use reqwest::Client;
use serde::Serialize;
use sqlx::FromRow;
use std::{env, time::Duration as StdDuration};
use tokio::time::interval;
use uuid::Uuid;

use crate::state::AppState;

pub fn email_delivery_available() -> bool {
    env::var("RESEND_API_KEY").is_ok_and(|value| !value.trim().is_empty())
        && env::var("EMAIL_FROM").is_ok_and(|value| !value.trim().is_empty())
}

#[derive(Debug, FromRow)]
struct EmailJob {
    id: Uuid,
    notification_id: Uuid,
    recipient_user_id: Uuid,
    attempts: i32,
}

#[derive(Debug, FromRow)]
struct EmailContent {
    recipient_email: String,
    title: String,
    body: String,
    email_enabled: bool,
}

#[derive(Debug, Serialize)]
struct ResendEmail<'a> {
    from: &'a str,
    to: [&'a str; 1],
    subject: &'a str,
    text: &'a str,
}

async fn claim_email_jobs(state: &AppState) -> Vec<EmailJob> {
    let mut transaction = match state.db.begin().await {
        Ok(transaction) => transaction,
        Err(error) => {
            tracing::error!(%error, "Could not begin notification email batch");
            return Vec::new();
        }
    };

    let stale_ambiguous = sqlx::query(
        "UPDATE notification_email_outbox SET status='reconciliation', locked_at=NULL,
                last_error='Provider idempotency window expired; reconcile delivery before retrying', updated_at=NOW()
          WHERE status IN ('queued','processing')
            AND first_provider_attempt_at < NOW() - INTERVAL '24 hours'
            AND (status='queued' OR locked_at < NOW() - INTERVAL '5 minutes')"
    ).execute(&mut *transaction).await;
    if let Err(error) = stale_ambiguous { tracing::error!(%error, "Could not quarantine ambiguous notification jobs"); return Vec::new(); }

    let exhausted = sqlx::query(
        "UPDATE notification_email_outbox SET status='failed', locked_at=NULL, last_error='Delivery attempt limit exhausted', updated_at=NOW()
          WHERE status='processing' AND locked_at < NOW() - INTERVAL '5 minutes' AND attempts >= 5"
    ).execute(&mut *transaction).await;
    if let Err(error) = exhausted { tracing::error!(%error, "Could not finalize exhausted notification jobs"); return Vec::new(); }

    let rows = match sqlx::query_as::<_, EmailJob>(
        "WITH next_jobs AS (
            SELECT id
              FROM notification_email_outbox
             WHERE (status = 'queued' AND available_at <= NOW() AND attempts < 5
                    AND (first_provider_attempt_at IS NULL OR first_provider_attempt_at >= NOW() - INTERVAL '24 hours'))
                OR (status = 'processing' AND locked_at < NOW() - INTERVAL '5 minutes' AND attempts < 5
                    AND (first_provider_attempt_at IS NULL OR first_provider_attempt_at >= NOW() - INTERVAL '24 hours'))
             ORDER BY available_at, created_at
             FOR UPDATE SKIP LOCKED
             LIMIT 10
         )
         UPDATE notification_email_outbox outbox
            SET status = 'processing',
                attempts = attempts + 1,
                locked_at = NOW(),
                first_provider_attempt_at = COALESCE(first_provider_attempt_at, NOW()),
                updated_at = NOW()
           FROM next_jobs
          WHERE outbox.id = next_jobs.id
         RETURNING outbox.id, outbox.notification_id, outbox.recipient_user_id, outbox.attempts",
    )
    .fetch_all(&mut *transaction)
    .await {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!(%error, "Could not claim notification email jobs");
            return Vec::new();
        }
    };

    if let Err(error) = transaction.commit().await {
        tracing::error!(%error, "Could not commit notification email claims");
        return Vec::new();
    }
    rows
}

async fn update_email_job(
    state: &AppState,
    job: &EmailJob,
    success: bool,
    error_message: Option<String>,
) {
    if success {
        if let Err(error) = sqlx::query(
            "UPDATE notification_email_outbox
                SET status='sent', sent_at=NOW(), locked_at=NULL, last_error=NULL, updated_at=NOW()
              WHERE id=$1 AND status='processing' AND attempts=$2",
        )
        .bind(job.id)
        .bind(job.attempts)
        .execute(&state.db)
        .await {
            tracing::error!(%error, job_id=%job.id, "Could not mark notification email as sent");
        }
        return;
    }

    let terminal = job.attempts >= 5;
    let retry_seconds = (60_i64 * 2_i64.pow((job.attempts.saturating_sub(1)).clamp(0, 6) as u32)).min(3600);
    let status = if terminal { "failed" } else { "queued" };
    if let Err(error) = sqlx::query(
        "UPDATE notification_email_outbox
            SET status=$2,
                available_at=CASE WHEN $2='queued' THEN NOW()+make_interval(secs=>$3) ELSE available_at END,
                locked_at=NULL,
                last_error=$4,
                updated_at=NOW()
          WHERE id=$1 AND status='processing' AND attempts=$5",
    )
    .bind(job.id)
    .bind(status)
    .bind(retry_seconds as f64)
    .bind(error_message.unwrap_or_else(|| "Email provider request failed".to_string()))
    .bind(job.attempts)
    .execute(&state.db)
    .await {
        tracing::error!(%error, job_id=%job.id, "Could not update failed notification email");
    }
}

async fn suppress_email_job(state: &AppState, job: &EmailJob, reason: &'static str) {
    if let Err(error) = sqlx::query(
        "UPDATE notification_email_outbox
            SET status='suppressed', locked_at=NULL, last_error=$2, updated_at=NOW()
          WHERE id=$1 AND status='processing' AND attempts=$3",
    )
    .bind(job.id)
    .bind(reason)
    .bind(job.attempts)
    .execute(&state.db)
    .await {
        tracing::error!(%error, job_id=%job.id, "Could not suppress notification email");
    }
}

async fn send_one(
    state: &AppState,
    client: &Client,
    api_key: &str,
    sender: &str,
    job: &EmailJob,
) {
    // Hold the outbox row lock through the provider request. Preference changes
    // suppress this row in the same transaction, so they cannot race between the
    // final eligibility check and dispatch. A request already in flight is allowed
    // to finish before the preference change can commit.
    let mut transaction = match state.db.begin().await {
        Ok(transaction) => transaction,
        Err(error) => {
            tracing::error!(%error, notification_id=%job.notification_id, "Could not begin email dispatch");
            update_email_job(state, job, false, Some("Could not begin email dispatch".to_string())).await;
            return;
        }
    };

    let content = sqlx::query_as::<_, EmailContent>(
        "SELECT u.email AS recipient_email, n.title, n.body,
                COALESCE(np.email_enabled,FALSE) AS email_enabled
           FROM notifications n
           JOIN users u ON u.id=n.recipient_user_id
           JOIN workspace_members wm
             ON wm.workspace_id=n.workspace_id AND wm.user_id=n.recipient_user_id
           LEFT JOIN notification_preferences np
             ON np.user_id=n.recipient_user_id AND np.notification_type=n.notification_type
           JOIN notification_email_outbox outbox
             ON outbox.notification_id=n.id AND outbox.recipient_user_id=n.recipient_user_id
          WHERE n.id=$1 AND n.recipient_user_id=$2
            AND outbox.id=$3 AND outbox.status='processing' AND outbox.attempts=$4
          FOR UPDATE OF outbox",
    )
    .bind(job.notification_id)
    .bind(job.recipient_user_id)
    .bind(job.id)
    .bind(job.attempts)
    .fetch_optional(&mut *transaction)
    .await;

    let content = match content {
        Ok(Some(content)) => content,
        Ok(None) => {
            if let Err(error) = sqlx::query(
                "UPDATE notification_email_outbox
                    SET status='suppressed', locked_at=NULL,
                        last_error='Notification or current workspace membership no longer exists', updated_at=NOW()
                  WHERE id=$1 AND status='processing' AND attempts=$2",
            )
            .bind(job.id)
            .bind(job.attempts)
            .execute(&mut *transaction)
            .await {
                tracing::error!(%error, job_id=%job.id, "Could not suppress ineligible email job");
            }
            if let Err(error) = transaction.commit().await {
                tracing::error!(%error, job_id=%job.id, "Could not commit email suppression");
            }
            return;
        }
        Err(error) => {
            drop(transaction);
            tracing::error!(%error, notification_id=%job.notification_id, "Could not load email notification");
            update_email_job(state, job, false, Some("Could not load notification".to_string())).await;
            return;
        }
    };

    if !content.email_enabled {
        if let Err(error) = sqlx::query(
            "UPDATE notification_email_outbox
                SET status='suppressed', locked_at=NULL,
                    last_error='Recipient disabled email notifications', updated_at=NOW()
              WHERE id=$1 AND status='processing' AND attempts=$2",
        )
        .bind(job.id)
        .bind(job.attempts)
        .execute(&mut *transaction)
        .await {
            tracing::error!(%error, job_id=%job.id, "Could not suppress disabled email job");
        }
        if let Err(error) = transaction.commit().await {
            tracing::error!(%error, job_id=%job.id, "Could not commit disabled email suppression");
        }
        return;
    }

    let body = format!(
        "{}\\n\\n{}\\n\\nOpen CampusForge Teams to review this update.",
        content.title, content.body
    );
    let request = ResendEmail {
        from: sender,
        to: [&content.recipient_email],
        subject: &content.title,
        text: &body,
    };
    let response = client
        .post("https://api.resend.com/emails")
        .bearer_auth(api_key)
        .header("Idempotency-Key", format!("campusforge-notification-{}", job.notification_id))
        .json(&request)
        .send()
        .await;

    let update = match response {
        Ok(response) if response.status().is_success() => {
            sqlx::query(
                "UPDATE notification_email_outbox
                    SET status='sent', sent_at=NOW(), locked_at=NULL, last_error=NULL, updated_at=NOW()
                  WHERE id=$1 AND status='processing' AND attempts=$2",
            )
            .bind(job.id)
            .bind(job.attempts)
            .execute(&mut *transaction)
            .await
        }
        Ok(response) => {
            let status = response.status();
            tracing::warn!(%status, notification_id=%job.notification_id, attempt=job.attempts, "Email provider rejected notification");
            let terminal = job.attempts >= 5;
            let next_status = if terminal { "failed" } else { "queued" };
            let retry_seconds = (60_i64 * 2_i64.pow((job.attempts.saturating_sub(1)).clamp(0, 6) as u32)).min(3600);
            sqlx::query(
                "UPDATE notification_email_outbox
                    SET status=$2,
                        available_at=CASE WHEN $2='queued' THEN statement_timestamp()+make_interval(secs=>$3) ELSE available_at END,
                        locked_at=NULL, last_error=$4, updated_at=NOW()
                  WHERE id=$1 AND status='processing' AND attempts=$5",
            )
            .bind(job.id)
            .bind(next_status)
            .bind(retry_seconds as f64)
            .bind(format!("Email provider returned HTTP {status}"))
            .bind(job.attempts)
            .execute(&mut *transaction)
            .await
        }
        Err(error) => {
            tracing::warn!(%error, notification_id=%job.notification_id, attempt=job.attempts, "Email provider request failed");
            let terminal = job.attempts >= 5;
            let next_status = if terminal { "failed" } else { "queued" };
            let retry_seconds = (60_i64 * 2_i64.pow((job.attempts.saturating_sub(1)).clamp(0, 6) as u32)).min(3600);
            sqlx::query(
                "UPDATE notification_email_outbox
                    SET status=$2,
                        available_at=CASE WHEN $2='queued' THEN statement_timestamp()+make_interval(secs=>$3) ELSE available_at END,
                        locked_at=NULL, last_error='Email provider request failed', updated_at=NOW()
                  WHERE id=$1 AND status='processing' AND attempts=$4",
            )
            .bind(job.id)
            .bind(next_status)
            .bind(retry_seconds as f64)
            .bind(job.attempts)
            .execute(&mut *transaction)
            .await
        }
    };

    if let Err(error) = update {
        tracing::error!(%error, job_id=%job.id, "Could not persist email delivery result");
        return;
    }
    if let Err(error) = transaction.commit().await {
        tracing::error!(%error, job_id=%job.id, "Could not commit email delivery result");
    }
}

async fn deliver_batch(state: &AppState, client: &Client, api_key: &str, sender: &str) {
    let jobs = claim_email_jobs(state).await;
    for job in jobs {
        send_one(state, client, api_key, sender, &job).await;
    }
}

pub async fn run_email_worker(state: AppState) {
    let mut ticker = interval(StdDuration::from_secs(20));
    let client = match Client::builder()
        .timeout(StdDuration::from_secs(15))
        .user_agent("CampusForgeTeams/1.0")
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            tracing::error!(%error, "Could not initialize email HTTP client");
            return;
        }
    };

    loop {
        ticker.tick().await;
        let api_key = match env::var("RESEND_API_KEY") {
            Ok(value) if !value.trim().is_empty() => value,
            _ => continue,
        };
        let sender = match env::var("EMAIL_FROM") {
            Ok(value) if !value.trim().is_empty() => value,
            _ => continue,
        };
        deliver_batch(&state, &client, &api_key, &sender).await;
    }
}
