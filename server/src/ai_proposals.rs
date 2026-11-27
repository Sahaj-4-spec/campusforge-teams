use axum::{
    extract::{Path, State},
    http::{header::CACHE_CONTROL, HeaderMap, HeaderValue, StatusCode},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

use crate::{auth::WorkspaceUser, state::AppState};

#[derive(Debug, Serialize)]
struct ApiError {
    error: &'static str,
}

type ApiResult<T> = Result<T, (StatusCode, Json<ApiError>)>;

fn fail(status: StatusCode, message: &'static str) -> (StatusCode, Json<ApiError>) {
    (status, Json(ApiError { error: message }))
}

fn no_store_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers
}

fn internal_error<E: std::fmt::Display>(
    cause: E,
    context: &'static str,
) -> (StatusCode, Json<ApiError>) {
    tracing::error!(error = %cause, "{context}");
    fail(StatusCode::INTERNAL_SERVER_ERROR, "The proposal request could not be completed")
}

async fn check_proposal_rate_limit(
    state: &AppState,
    user_id: Uuid,
    workspace_id: Uuid,
    action: &'static str,
    limit: i32,
) -> ApiResult<()> {
    let key = format!("ai-proposal:{action}:{workspace_id}:{user_id}");
    let allowed = state.check_rate_limit(&key, limit, 60).await
        .map_err(|cause| internal_error(cause, "Could not enforce AI proposal rate limit"))?;
    if !allowed {
        return Err(fail(StatusCode::TOO_MANY_REQUESTS, "Too many proposal requests. Please wait and try again."));
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ProposedTask {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default = "default_priority")]
    priority: String,
    #[serde(default)]
    due_at: Option<DateTime<Utc>>,
    #[serde(default)]
    estimated_minutes: Option<i32>,
    #[serde(default)]
    assignee_ids: Vec<Uuid>,
    #[serde(default)]
    source_refs: Vec<String>,
}

fn default_priority() -> String {
    "medium".to_owned()
}

#[derive(Debug, Deserialize)]
struct CreateProposalRequest {
    board_id: Uuid,
    target_list_id: Uuid,
    tasks: Vec<ProposedTask>,
}

#[derive(Debug, FromRow)]
struct ProposalRow {
    id: Uuid,
    board_id: Uuid,
    target_list_id: Uuid,
    created_by: Uuid,
    status: String,
    tasks_json: String,
    source_fingerprint: String,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    reviewed_by: Option<Uuid>,
    reviewed_at: Option<DateTime<Utc>>,
    applied_task_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
struct ProposalView {
    id: Uuid,
    board_id: Uuid,
    target_list_id: Uuid,
    created_by: Uuid,
    status: String,
    tasks: Vec<ProposedTask>,
    expires_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
    reviewed_by: Option<Uuid>,
    reviewed_at: Option<DateTime<Utc>>,
    applied_task_ids: Vec<Uuid>,
}

impl TryFrom<ProposalRow> for ProposalView {
    type Error = serde_json::Error;

    fn try_from(row: ProposalRow) -> Result<Self, Self::Error> {
        let mut status = row.status;
        if status == "pending" && row.expires_at <= Utc::now() {
            status = "expired".to_owned();
        }
        Ok(Self {
            id: row.id,
            board_id: row.board_id,
            target_list_id: row.target_list_id,
            created_by: row.created_by,
            status,
            tasks: serde_json::from_str(&row.tasks_json)?,
            expires_at: row.expires_at,
            created_at: row.created_at,
            reviewed_by: row.reviewed_by,
            reviewed_at: row.reviewed_at,
            applied_task_ids: row.applied_task_ids,
        })
    }
}

#[derive(Debug, FromRow, Serialize)]
struct AuditView {
    id: Uuid,
    board_id: Uuid,
    proposal_id: Option<Uuid>,
    actor_id: Option<Uuid>,
    event_type: String,
    metadata_json: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct AuditResponse {
    id: Uuid,
    board_id: Uuid,
    proposal_id: Option<Uuid>,
    actor_id: Option<Uuid>,
    event_type: String,
    metadata: serde_json::Value,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ApplyResponse {
    proposal_id: Uuid,
    status: &'static str,
    task_ids: Vec<Uuid>,
}

fn validate_tasks(tasks: &mut [ProposedTask]) -> Result<(), (StatusCode, Json<ApiError>)> {
    if tasks.is_empty() || tasks.len() > 8 {
        return Err(fail(StatusCode::BAD_REQUEST, "A proposal must contain between 1 and 8 tasks"));
    }

    for task in tasks.iter_mut() {
        task.title = task.title.trim().to_owned();
        if task.title.is_empty() || task.title.chars().count() > 160 || task.title.contains('\0') {
            return Err(fail(StatusCode::BAD_REQUEST, "Proposed task titles must be 1 to 160 characters"));
        }
        task.description = task.description.as_ref().map(|value| value.trim().to_owned());
        if task.description.as_ref().is_some_and(|value| value.chars().count() > 4000 || value.contains('\0')) {
            return Err(fail(StatusCode::BAD_REQUEST, "A proposed task description is too long"));
        }
        task.priority = task.priority.trim().to_ascii_lowercase();
        if !matches!(task.priority.as_str(), "low" | "medium" | "high" | "urgent") {
            return Err(fail(StatusCode::BAD_REQUEST, "A proposed task has an invalid priority"));
        }
        if task.estimated_minutes.is_some_and(|value| !(1..=100000).contains(&value)) {
            return Err(fail(StatusCode::BAD_REQUEST, "A proposed task estimate is outside the allowed range"));
        }
        if task.assignee_ids.len() > 20 {
            return Err(fail(StatusCode::BAD_REQUEST, "A proposed task has too many assignees"));
        }
        task.assignee_ids.sort_unstable();
        task.assignee_ids.dedup();
        if task.source_refs.len() > 8
            || task.source_refs.iter().any(|value| value.trim().is_empty() || value.chars().count() > 80 || value.contains('\0'))
        {
            return Err(fail(StatusCode::BAD_REQUEST, "A proposed task contains invalid source references"));
        }
    }
    Ok(())
}

/// A member with the viewer role can read proposals but cannot create or apply them.
/// Lock the authorization rows while an approval transaction is in progress.
async fn can_write_board(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    workspace_id: Uuid,
    board_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let board: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM boards WHERE id = $1 AND workspace_id = $2 FOR KEY SHARE",
    )
    .bind(board_id)
    .bind(workspace_id)
    .fetch_optional(&mut **tx)
    .await?;
    if board.is_none() {
        return Ok(false);
    }

    let workspace_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM workspace_members
          WHERE workspace_id = $1 AND user_id = $2
          FOR SHARE",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some(workspace_role) = workspace_role else {
        return Ok(false);
    };
    if matches!(workspace_role.as_str(), "owner" | "admin") {
        return Ok(true);
    }

    let board_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM board_members
          WHERE board_id = $1 AND user_id = $2
          FOR SHARE",
    )
    .bind(board_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(matches!(board_role.as_deref(), Some("admin" | "member")))
}

/// Hash only the current board/list/task state. Raw content is never written to the audit log.
async fn board_fingerprint(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    board_id: Uuid,
) -> Result<String, sqlx::Error> {
    let snapshot: Option<String> = sqlx::query_scalar(
        "SELECT jsonb_build_object(
            'board', jsonb_build_object(
                'id', b.id, 'name', b.name, 'description', b.description
            ),
            'lists', COALESCE((
                SELECT jsonb_agg(
                    jsonb_build_object(
                        'id', l.id, 'name', l.name, 'position', l.position,
                        'tasks', COALESCE((
                            SELECT jsonb_agg(
                                jsonb_build_object(
                                    'id', t.id, 'title', t.title,
                                    'description', t.description, 'priority', t.priority,
                                    'due_at', t.due_at, 'position', t.position,
                                    'updated_at', t.updated_at,
                                    'estimated_minutes', t.estimated_minutes,
                                    'parent_task_id', t.parent_task_id,
                                    'assignees', COALESCE((
                                        SELECT jsonb_agg(ta.user_id ORDER BY ta.user_id)
                                          FROM task_assignees ta
                                         WHERE ta.task_id = t.id
                                    ), '[]'::jsonb)
                                ) ORDER BY t.position, t.id
                            )
                              FROM tasks t
                             WHERE t.list_id = l.id
                        ), '[]'::jsonb)
                    ) ORDER BY l.position, l.id
                )
                  FROM lists l
                 WHERE l.board_id = b.id
            ), '[]'::jsonb)
        )::text
          FROM boards b
         WHERE b.id = $1 AND b.workspace_id = $2",
    )
    .bind(board_id)
    .bind(workspace_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(snapshot) = snapshot else {
        return Ok(String::new());
    };
    let digest = Sha256::digest(snapshot.as_bytes());
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

async fn validate_assignees(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    board_id: Uuid,
    tasks: &[ProposedTask],
) -> Result<bool, sqlx::Error> {
    for task in tasks {
        if task.assignee_ids.is_empty() {
            continue;
        }

        // Lock both workspace and board membership records so access cannot be
        // revoked between this check and the task/assignee inserts.
        let owners: Vec<Uuid> = sqlx::query_scalar(
            "SELECT user_id FROM workspace_members
              WHERE workspace_id = $1 AND user_id = ANY($2::uuid[])
                AND role IN ('owner', 'admin')
              FOR SHARE",
        )
        .bind(workspace_id)
        .bind(&task.assignee_ids)
        .fetch_all(&mut **tx)
        .await?;

        let board_members: Vec<Uuid> = sqlx::query_scalar(
            "SELECT bm.user_id
               FROM board_members bm
               JOIN workspace_members wm
                 ON wm.workspace_id = $2 AND wm.user_id = bm.user_id
              WHERE bm.board_id = $1 AND bm.user_id = ANY($3::uuid[])
                AND bm.role IN ('admin', 'member')
              FOR SHARE OF bm, wm",
        )
        .bind(board_id)
        .bind(workspace_id)
        .bind(&task.assignee_ids)
        .fetch_all(&mut **tx)
        .await?;

        let mut eligible = owners;
        eligible.extend(board_members);
        eligible.sort_unstable();
        eligible.dedup();
        if eligible.len() != task.assignee_ids.len() {
            return Ok(false);
        }
    }
    Ok(true)
}

async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    board_id: Uuid,
    proposal_id: Uuid,
    actor_id: Uuid,
    event_type: &'static str,
    metadata: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO ai_action_audit_events
            (workspace_id, board_id, proposal_id, actor_id, event_type, metadata)
         VALUES ($1, $2, $3, $4, $5, $6::jsonb)",
    )
    .bind(workspace_id)
    .bind(board_id)
    .bind(proposal_id)
    .bind(actor_id)
    .bind(event_type)
    .bind(metadata.to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn list_proposals(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
) -> ApiResult<(HeaderMap, Json<Vec<ProposalView>>)> {
    check_proposal_rate_limit(&state, user_id, workspace_id, "read", 60).await?;
    let rows = sqlx::query_as::<_, ProposalRow>(
        "SELECT p.id, p.board_id, p.target_list_id, p.created_by, p.status,
                p.tasks::text AS tasks_json, p.source_fingerprint, p.expires_at,
                p.created_at, p.reviewed_by, p.reviewed_at, p.applied_task_ids
           FROM ai_action_proposals p
          WHERE p.workspace_id = $1
            AND (
                EXISTS (SELECT 1 FROM workspace_members wm
                         WHERE wm.workspace_id = p.workspace_id AND wm.user_id = $2
                           AND wm.role IN ('owner', 'admin'))
                OR EXISTS (SELECT 1 FROM board_members bm
                            WHERE bm.board_id = p.board_id AND bm.user_id = $2)
            )
          ORDER BY p.created_at DESC
          LIMIT 50",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|cause| internal_error(cause, "Could not list AI action proposals"))?;

    let mut proposals = Vec::with_capacity(rows.len());
    for row in rows {
        proposals.push(ProposalView::try_from(row).map_err(|cause| {
            internal_error(cause, "Could not decode a stored AI action proposal")
        })?);
    }
    Ok((no_store_headers(), Json(proposals)))
}

async fn create_proposal(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Json(mut request): Json<CreateProposalRequest>,
) -> ApiResult<(StatusCode, Json<ProposalView>)> {
    check_proposal_rate_limit(&state, user_id, workspace_id, "write", 10).await?;
    validate_tasks(&mut request.tasks)?;

    let mut tx = state.db.begin().await
        .map_err(|cause| internal_error(cause, "Could not begin proposal creation"))?;

    if !can_write_board(&mut tx, user_id, workspace_id, request.board_id).await
        .map_err(|cause| internal_error(cause, "Could not verify proposal permissions"))?
    {
        return Err(fail(StatusCode::FORBIDDEN, "You cannot create proposals for this project"));
    }

    let list_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM lists WHERE id = $1 AND board_id = $2)",
    )
    .bind(request.target_list_id)
    .bind(request.board_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not validate the proposal destination"))?;
    if !list_exists {
        return Err(fail(StatusCode::BAD_REQUEST, "Choose a valid column in the selected project"));
    }

    if !validate_assignees(&mut tx, workspace_id, request.board_id, &request.tasks).await
        .map_err(|cause| internal_error(cause, "Could not validate proposed assignees"))?
    {
        return Err(fail(StatusCode::BAD_REQUEST, "Every proposed assignee must be a member of this project"));
    }

    let fingerprint = board_fingerprint(&mut tx, workspace_id, request.board_id).await
        .map_err(|cause| internal_error(cause, "Could not capture the project state"))?;
    if fingerprint.is_empty() {
        return Err(fail(StatusCode::NOT_FOUND, "Project not found"));
    }

    let tasks_json = serde_json::to_string(&request.tasks)
        .map_err(|cause| internal_error(cause, "Could not encode the task proposal"))?;
    let row = sqlx::query_as::<_, ProposalRow>(
        "INSERT INTO ai_action_proposals
            (workspace_id, board_id, target_list_id, created_by, tasks, source_fingerprint, expires_at)
         VALUES ($1, $2, $3, $4, $5::jsonb, $6, $7)
         RETURNING id, board_id, target_list_id, created_by, status,
                   tasks::text AS tasks_json, source_fingerprint, expires_at,
                   created_at, reviewed_by, reviewed_at, applied_task_ids",
    )
    .bind(workspace_id)
    .bind(request.board_id)
    .bind(request.target_list_id)
    .bind(user_id)
    .bind(tasks_json)
    .bind(fingerprint)
    .bind(Utc::now() + Duration::hours(24))
    .fetch_one(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not save the task proposal"))?;

    insert_audit(
        &mut tx, workspace_id, request.board_id, row.id, user_id, "created",
        json!({ "task_count": request.tasks.len(), "target_list_id": request.target_list_id }),
    )
    .await
    .map_err(|cause| internal_error(cause, "Could not record proposal creation"))?;

    tx.commit().await
        .map_err(|cause| internal_error(cause, "Could not commit the task proposal"))?;

    let view = ProposalView::try_from(row)
        .map_err(|cause| internal_error(cause, "Could not encode the saved proposal"))?;
    Ok((StatusCode::CREATED, Json(view)))
}

async fn apply_proposal(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(proposal_id): Path<Uuid>,
) -> ApiResult<Json<ApplyResponse>> {
    check_proposal_rate_limit(&state, user_id, workspace_id, "write", 10).await?;
    let mut tx = state.db.begin().await
        .map_err(|cause| internal_error(cause, "Could not begin proposal approval"))?;
    // Serializable isolation prevents a concurrent board/task write from slipping
    // between the drift check and the approved task inserts.
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not establish safe proposal approval isolation"))?;

    let row = sqlx::query_as::<_, ProposalRow>(
        "SELECT id, board_id, target_list_id, created_by, status,
                tasks::text AS tasks_json, source_fingerprint, expires_at,
                created_at, reviewed_by, reviewed_at, applied_task_ids
           FROM ai_action_proposals
          WHERE id = $1 AND workspace_id = $2
          FOR UPDATE",
    )
    .bind(proposal_id)
    .bind(workspace_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|cause| {
        if cause.as_database_error().and_then(|error| error.code()).as_deref() == Some("40001") {
            fail(StatusCode::CONFLICT, "This proposal was reviewed concurrently. Refresh the proposal list and try again.")
        } else {
            internal_error(cause, "Could not load the task proposal")
        }
    })?;
    let Some(row) = row else {
        return Err(fail(StatusCode::NOT_FOUND, "Proposal not found"));
    };

    if row.status != "pending" {
        return Err(fail(StatusCode::CONFLICT, "This proposal has already been reviewed"));
    }
    if !can_write_board(&mut tx, user_id, workspace_id, row.board_id).await
        .map_err(|cause| internal_error(cause, "Could not recheck project permissions"))?
    {
        return Err(fail(StatusCode::FORBIDDEN, "You no longer have permission to approve this proposal"));
    }

    if row.expires_at <= Utc::now() {
        sqlx::query(
            "UPDATE ai_action_proposals SET status = 'expired', reviewed_by = $1, reviewed_at = NOW()
              WHERE id = $2",
        )
        .bind(user_id)
        .bind(proposal_id)
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not expire the proposal"))?;
        insert_audit(&mut tx, workspace_id, row.board_id, proposal_id, user_id, "expired", json!({}))
            .await
            .map_err(|cause| internal_error(cause, "Could not record proposal expiry"))?;
        tx.commit().await.map_err(|cause| internal_error(cause, "Could not save proposal expiry"))?;
        return Err(fail(StatusCode::CONFLICT, "This proposal has expired and cannot be applied"));
    }

    let current_fingerprint = board_fingerprint(&mut tx, workspace_id, row.board_id).await
        .map_err(|cause| internal_error(cause, "Could not recheck current project state"))?;
    if current_fingerprint != row.source_fingerprint {
        sqlx::query(
            "UPDATE ai_action_proposals SET status = 'stale', reviewed_by = $1, reviewed_at = NOW()
              WHERE id = $2",
        )
        .bind(user_id)
        .bind(proposal_id)
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not mark the proposal stale"))?;
        insert_audit(&mut tx, workspace_id, row.board_id, proposal_id, user_id, "stale", json!({}))
            .await
            .map_err(|cause| internal_error(cause, "Could not record proposal drift"))?;
        tx.commit().await.map_err(|cause| internal_error(cause, "Could not save proposal state"))?;
        return Err(fail(StatusCode::CONFLICT, "The project changed after this proposal was created. Generate a fresh proposal."));
    }

    let mut tasks: Vec<ProposedTask> = serde_json::from_str(&row.tasks_json)
        .map_err(|cause| internal_error(cause, "Stored proposal data is invalid"))?;
    validate_tasks(&mut tasks).map_err(|_| fail(StatusCode::CONFLICT, "Stored proposal validation failed"))?;
    if !validate_assignees(&mut tx, workspace_id, row.board_id, &tasks).await
        .map_err(|cause| internal_error(cause, "Could not recheck proposed assignees"))?
    {
        sqlx::query(
            "UPDATE ai_action_proposals SET status = 'stale', reviewed_by = $1, reviewed_at = NOW()
              WHERE id = $2",
        )
        .bind(user_id)
        .bind(proposal_id)
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not mark the proposal stale"))?;
        insert_audit(&mut tx, workspace_id, row.board_id, proposal_id, user_id, "stale", json!({ "reason": "assignee_membership_changed" }))
            .await
            .map_err(|cause| internal_error(cause, "Could not record proposal drift"))?;
        tx.commit().await.map_err(|cause| internal_error(cause, "Could not save proposal state"))?;
        return Err(fail(StatusCode::CONFLICT, "A proposed assignee is no longer a member of this project. Generate a fresh proposal."));
    }

    let list_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM lists WHERE id = $1 AND board_id = $2)",
    )
    .bind(row.target_list_id)
    .bind(row.board_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not recheck the task destination"))?;
    if !list_exists {
        return Err(fail(StatusCode::CONFLICT, "The target column no longer exists"));
    }

    // Serialize proposal task inserts with other inserts into the same list.
    sqlx::query("SELECT id FROM lists WHERE id = $1 FOR UPDATE")
        .bind(row.target_list_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not lock the destination column"))?;

    // The list lock can block long enough for a proposal to expire.
    // Re-check the deadline immediately before the first task write.
    if row.expires_at <= Utc::now() {
        sqlx::query(
            "UPDATE ai_action_proposals SET status = 'expired', reviewed_by = $1, reviewed_at = NOW()
              WHERE id = $2 AND status = 'pending'",
        )
        .bind(user_id)
        .bind(proposal_id)
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not expire the locked proposal"))?;
        insert_audit(&mut tx, workspace_id, row.board_id, proposal_id, user_id, "expired", json!({}))
            .await
            .map_err(|cause| internal_error(cause, "Could not audit the expired proposal"))?;
        tx.commit().await.map_err(|cause| internal_error(cause, "Could not save proposal expiry"))?;
        return Err(fail(StatusCode::CONFLICT, "This proposal expired while it was being reviewed"));
    }

    let mut next_position: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM tasks WHERE list_id = $1",
    )
    .bind(row.target_list_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not allocate task positions"))?;

    let mut task_ids = Vec::with_capacity(tasks.len());
    for task in &tasks {
        let task_id: Uuid = sqlx::query_scalar(
            "INSERT INTO tasks
                (list_id, title, description, priority, due_at, position, created_by, estimated_minutes)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING id",
        )
        .bind(row.target_list_id)
        .bind(&task.title)
        .bind(&task.description)
        .bind(&task.priority)
        .bind(task.due_at.clone())
        .bind(next_position)
        .bind(user_id)
        .bind(task.estimated_minutes)
        .fetch_one(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not create an approved task"))?;

        for assignee_id in &task.assignee_ids {
            sqlx::query("INSERT INTO task_assignees (task_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(task_id)
                .bind(assignee_id)
                .execute(&mut *tx)
                .await
                .map_err(|cause| internal_error(cause, "Could not assign an approved task"))?;
        }
        task_ids.push(task_id);
        next_position += 1;
    }

    let task_ids_json = serde_json::to_value(&task_ids)
        .map_err(|cause| internal_error(cause, "Could not encode approved task IDs"))?;
    sqlx::query(
        "INSERT INTO activity_events (workspace_id, board_id, actor_id, event_type, payload)
         VALUES ($1, $2, $3, 'ai_proposal_applied', $4::jsonb)",
    )
    .bind(workspace_id)
    .bind(row.board_id)
    .bind(user_id)
    .bind(json!({ "proposal_id": proposal_id, "task_ids": task_ids_json }).to_string())
    .execute(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not record task creation activity"))?;

    sqlx::query(
        "UPDATE ai_action_proposals
            SET status = 'applied', reviewed_by = $1, reviewed_at = NOW(), applied_task_ids = $2
          WHERE id = $3 AND status = 'pending'",
    )
    .bind(user_id)
    .bind(&task_ids)
    .bind(proposal_id)
    .execute(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not finalize the task proposal"))?;

    insert_audit(
        &mut tx, workspace_id, row.board_id, proposal_id, user_id, "applied",
        json!({ "task_count": task_ids.len(), "task_ids": task_ids }),
    )
    .await
    .map_err(|cause| internal_error(cause, "Could not record proposal approval"))?;

    tx.commit().await
        .map_err(|cause| internal_error(cause, "Could not commit approved tasks"))?;
    state.publish_workspace_event(workspace_id, Some(row.board_id), "ai_proposal_applied", "Approved AI task proposal");
    Ok(Json(ApplyResponse { proposal_id, status: "applied", task_ids }))
}

async fn reject_proposal(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(proposal_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    check_proposal_rate_limit(&state, user_id, workspace_id, "write", 10).await?;
    let mut tx = state.db.begin().await
        .map_err(|cause| internal_error(cause, "Could not begin proposal rejection"))?;
    let row = sqlx::query_as::<_, ProposalRow>(
        "SELECT id, board_id, target_list_id, created_by, status,
                tasks::text AS tasks_json, source_fingerprint, expires_at,
                created_at, reviewed_by, reviewed_at, applied_task_ids
           FROM ai_action_proposals
          WHERE id = $1 AND workspace_id = $2
          FOR UPDATE",
    )
    .bind(proposal_id)
    .bind(workspace_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not load the task proposal"))?;
    let Some(row) = row else {
        return Err(fail(StatusCode::NOT_FOUND, "Proposal not found"));
    };
    if row.status != "pending" {
        return Err(fail(StatusCode::CONFLICT, "This proposal has already been reviewed"));
    }
    if !can_write_board(&mut tx, user_id, workspace_id, row.board_id).await
        .map_err(|cause| internal_error(cause, "Could not recheck project permissions"))?
    {
        return Err(fail(StatusCode::FORBIDDEN, "You no longer have permission to reject this proposal"));
    }

    if row.expires_at <= Utc::now() {
        sqlx::query(
            "UPDATE ai_action_proposals SET status = 'expired', reviewed_by = $1, reviewed_at = NOW()
              WHERE id = $2 AND status = 'pending'",
        )
        .bind(user_id)
        .bind(proposal_id)
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not expire the rejected proposal"))?;
        insert_audit(&mut tx, workspace_id, row.board_id, proposal_id, user_id, "expired", json!({}))
            .await
            .map_err(|cause| internal_error(cause, "Could not audit the expired proposal"))?;
        tx.commit().await.map_err(|cause| internal_error(cause, "Could not save proposal expiry"))?;
        return Err(fail(StatusCode::CONFLICT, "This proposal has expired and cannot be rejected"));
    }

    sqlx::query(
        "UPDATE ai_action_proposals SET status = 'rejected', reviewed_by = $1, reviewed_at = NOW()
          WHERE id = $2 AND status = 'pending'",
    )
    .bind(user_id)
    .bind(proposal_id)
    .execute(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not reject the proposal"))?;
    insert_audit(&mut tx, workspace_id, row.board_id, proposal_id, user_id, "rejected", json!({}))
        .await
        .map_err(|cause| internal_error(cause, "Could not record proposal rejection"))?;

    tx.commit().await.map_err(|cause| internal_error(cause, "Could not save proposal rejection"))?;
    state.publish_workspace_event(workspace_id, Some(row.board_id), "ai_proposal_rejected", "An AI task proposal was rejected");
    Ok(StatusCode::NO_CONTENT)
}

async fn list_audit(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
) -> ApiResult<(HeaderMap, Json<Vec<AuditResponse>>)> {
    check_proposal_rate_limit(&state, user_id, workspace_id, "read", 60).await?;
    let rows = sqlx::query_as::<_, AuditView>(
        "SELECT e.id, e.board_id, e.proposal_id, e.actor_id, e.event_type,
                e.metadata::text AS metadata_json, e.created_at
           FROM ai_action_audit_events e
           JOIN workspace_members wm ON wm.workspace_id = e.workspace_id AND wm.user_id = $2
          WHERE e.workspace_id = $1
            AND (
                wm.role IN ('owner', 'admin')
                OR EXISTS (SELECT 1 FROM board_members bm
                            WHERE bm.board_id = e.board_id AND bm.user_id = $2)
            )
          ORDER BY e.created_at DESC
          LIMIT 100",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|cause| internal_error(cause, "Could not load the AI action audit log"))?;

    let mut events = Vec::with_capacity(rows.len());
    for row in rows {
        let metadata = serde_json::from_str(&row.metadata_json)
            .map_err(|cause| internal_error(cause, "Could not decode an audit event"))?;
        events.push(AuditResponse {
            id: row.id,
            board_id: row.board_id,
            proposal_id: row.proposal_id,
            actor_id: row.actor_id,
            event_type: row.event_type,
            metadata,
            created_at: row.created_at,
        });
    }
    Ok((no_store_headers(), Json(events)))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/ai/proposals", get(list_proposals).post(create_proposal))
        .route("/ai/proposals/audit", get(list_audit))
        .route("/ai/proposals/{proposal_id}/apply", post(apply_proposal))
        .route("/ai/proposals/{proposal_id}/reject", post(reject_proposal))
}


#[cfg(test)]
mod tests {
    use super::*;

    fn task() -> ProposedTask {
        ProposedTask {
            title: "  Implement proposal review  ".to_owned(),
            description: Some(" Add explicit human approval ".to_owned()),
            priority: "HIGH".to_owned(),
            due_at: None,
            estimated_minutes: Some(60),
            assignee_ids: vec![Uuid::nil(), Uuid::nil()],
            source_refs: vec!["T-12".to_owned()],
        }
    }

    #[test]
    fn validates_and_normalizes_proposed_task_fields() {
        let mut tasks = vec![task()];
        validate_tasks(&mut tasks).expect("task should be valid");
        assert_eq!(tasks[0].title, "Implement proposal review");
        assert_eq!(tasks[0].description.as_deref(), Some("Add explicit human approval"));
        assert_eq!(tasks[0].priority, "high");
        assert_eq!(tasks[0].assignee_ids.len(), 1);
    }

    #[test]
    fn rejects_empty_and_oversized_proposals() {
        let mut empty: Vec<ProposedTask> = Vec::new();
        assert!(validate_tasks(&mut empty).is_err());
        let mut tasks = vec![task(); 9];
        assert!(validate_tasks(&mut tasks).is_err());
    }

    #[test]
    fn rejects_invalid_task_priorities_and_estimates() {
        let mut invalid_priority = vec![task()];
        invalid_priority[0].priority = "critical-now".to_owned();
        assert!(validate_tasks(&mut invalid_priority).is_err());

        let mut invalid_estimate = vec![task()];
        invalid_estimate[0].estimated_minutes = Some(0);
        assert!(validate_tasks(&mut invalid_estimate).is_err());
    }
}
