use reqwest::Client;
use serde::Serialize;
use sqlx::FromRow;
use std::{env, time::Duration as StdDuration};
use tokio::time::interval;
use uuid::Uuid;

use crate::state::AppState;

pub fn email_delivery_available() -> bool {
    env::var("RESEND_API_KEY").is_ok()
        && env::var("EMAIL_FROM").is_ok()
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

    let rows = match sqlx::query_as::<_, EmailJob>(
        "WITH next_jobs AS (
            SELECT id
              FROM notification_email_outbox
             WHERE status = 'queued' AND available_at <= NOW()
             ORDER BY available_at, created_at
             FOR UPDATE SKIP LOCKED
             LIMIT 10
         )
         UPDATE notification_email_outbox outbox
            SET status = 'processing',
                attempts = attempts + 1,
                locked_at = NOW(),
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
              WHERE id=$1 AND status='processing'",
        )
        .bind(job.id)
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
          WHERE id=$1 AND status='processing'",
    )
    .bind(job.id)
    .bind(status)
    .bind(retry_seconds as f64)
    .bind(error_message.unwrap_or_else(|| "Email provider request failed".to_string()))
    .execute(&state.db)
    .await {
        tracing::error!(%error, job_id=%job.id, "Could not update failed notification email");
    }
}

async fn send_one(
    state: &AppState,
    client: &Client,
    api_key: &str,
    sender: &str,
    job: &EmailJob,
) {
    let content = sqlx::query_as::<_, EmailContent>(
        "SELECT u.email AS recipient_email, n.title, n.body
           FROM notifications n
           JOIN users u ON u.id=n.recipient_user_id
          WHERE n.id=$1 AND n.recipient_user_id=$2",
    )
    .bind(job.notification_id)
    .bind(job.recipient_user_id)
    .fetch_optional(&state.db)
    .await;

    let content = match content {
        Ok(Some(content)) => content,
        Ok(None) => {
            update_email_job(state, job, false, Some("Notification recipient or notification no longer exists".to_string())).await;
            return;
        }
        Err(error) => {
            tracing::error!(%error, notification_id=%job.notification_id, "Could not load email notification");
            update_email_job(state, job, false, Some("Could not load notification".to_string())).await;
            return;
        }
    };

    let body = format!(
        "{}\n\n{}\n\nOpen CampusForge Teams to review this update.",
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

    match response {
        Ok(response) if response.status().is_success() => {
            update_email_job(state, job, true, None).await;
        }
        Ok(response) => {
            let status = response.status();
            tracing::warn!(%status, notification_id=%job.notification_id, attempt=job.attempts, "Email provider rejected notification");
            update_email_job(state, job, false, Some(format!("Email provider returned HTTP {status}"))).await;
        }
        Err(error) => {
            tracing::warn!(%error, notification_id=%job.notification_id, attempt=job.attempts, "Email provider request failed");
            update_email_job(state, job, false, Some("Email provider request failed".to_string())).await;
        }
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
