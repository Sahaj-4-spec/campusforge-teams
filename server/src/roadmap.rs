use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post, put},
    Json, Router,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, Postgres, Transaction};
use std::collections::HashSet;
use tokio::time::{interval, Duration as TokioDuration};
use uuid::Uuid;

use crate::{auth::{AuthUser, WorkspaceUser}, state::AppState};

#[derive(Debug, Serialize)]
struct ErrorBody { error: String }
type ApiResult<T> = Result<T, (StatusCode, Json<ErrorBody>)>;

fn fail(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<ErrorBody>) {
    (status, Json(ErrorBody { error: message.into() }))
}

fn internal<E: std::fmt::Display>(cause: E, context: &'static str) -> (StatusCode, Json<ErrorBody>) {
    tracing::error!(%cause, "{context}");
    fail(StatusCode::INTERNAL_SERVER_ERROR, "The request could not be completed")
}

pub(crate) async fn workspace_admin(state: &AppState, user_id: Uuid, workspace_id: Uuid) -> Result<bool, sqlx::Error> {
    let role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM workspace_members WHERE workspace_id=$1 AND user_id=$2",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(matches!(role.as_deref(), Some("owner" | "admin")))
}

pub(crate) async fn create_notification_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    recipient_user_id: Uuid,
    actor_user_id: Option<Uuid>,
    notification_type: &str,
    title: &str,
    body: &str,
    resource_type: Option<&str>,
    resource_id: Option<Uuid>,
    dedupe_key: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "WITH inserted AS (
            INSERT INTO notifications
                (workspace_id, board_id, recipient_user_id, actor_user_id, notification_type,
                 title, body, resource_type, resource_id, dedupe_key)
            VALUES (
                $1,
                CASE
                    WHEN $7 = 'calendar_event' THEN (
                        SELECT e.board_id FROM calendar_events e
                         WHERE e.id=$8 AND e.workspace_id=$1
                    )
                    WHEN $7 = 'meeting_action_item' THEN (
                        SELECT mn.board_id
                          FROM meeting_note_action_items ai
                          JOIN meeting_notes mn ON mn.id=ai.meeting_note_id
                         WHERE ai.id=$8 AND ai.workspace_id=$1 AND mn.workspace_id=$1
                    )
                    WHEN $7 = 'chat_message' THEN (
                        SELECT cm.board_id FROM chat_messages cm
                         WHERE cm.id=$8 AND cm.workspace_id=$1 AND cm.conversation_id IS NULL
                    )
                    ELSE NULL::uuid
                END,
                $2,$3,$4,$5,$6,$7,$8,$9
            )
            ON CONFLICT (dedupe_key) DO NOTHING
            RETURNING id, recipient_user_id
         )
         INSERT INTO notification_email_outbox(notification_id, recipient_user_id)
         SELECT i.id, i.recipient_user_id
           FROM inserted i
           JOIN notification_preferences p
             ON p.user_id=i.recipient_user_id
            AND p.notification_type=$4
            AND p.email_enabled=TRUE
         ON CONFLICT (notification_id) DO NOTHING",
    )
    .bind(workspace_id)
    .bind(recipient_user_id)
    .bind(actor_user_id)
    .bind(notification_type)
    .bind(title)
    .bind(body)
    .bind(resource_type)
    .bind(resource_id)
    .bind(dedupe_key)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn mentioned_user_ids(body: &str) -> HashSet<Uuid> {
    let mut result = HashSet::new();
    let mut remaining = body;
    while let Some(start) = remaining.find("@{") {
        let after = &remaining[start + 2..];
        let Some(end) = after.find('}') else { break; };
        if let Ok(user_id) = Uuid::parse_str(after[..end].trim()) {
            result.insert(user_id);
        }
        remaining = &after[end + 1..];
    }
    result
}

pub(crate) async fn create_mentions_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    actor_user_id: Uuid,
    resource_type: &str,
    resource_id: Uuid,
    body: &str,
) -> Result<(), sqlx::Error> {
    for mentioned_user_id in mentioned_user_ids(body) {
        if mentioned_user_id == actor_user_id {
            continue;
        }
        let is_member: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM workspace_members WHERE workspace_id=$1 AND user_id=$2)",
        )
        .bind(workspace_id)
        .bind(mentioned_user_id)
        .fetch_one(&mut **tx)
        .await?;
        if !is_member {
            continue;
        }
        create_notification_in_transaction(
            tx,
            workspace_id,
            mentioned_user_id,
            Some(actor_user_id),
            "mention",
            "You were mentioned",
            "A teammate mentioned you in a workspace conversation.",
            Some(resource_type),
            Some(resource_id),
            Some(&format!("mention:{resource_type}:{resource_id}:{mentioned_user_id}")),
        )
        .await?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct TaskSearchQuery {
    board_id: Option<Uuid>,
    label_id: Option<Uuid>,
    parent_task_id: Option<Uuid>,
    min_estimated_minutes: Option<i32>,
    max_estimated_minutes: Option<i32>,
    has_blockers: Option<bool>,
    status: Option<String>,
    assignee_id: Option<Uuid>,
    priority: Option<String>,
    due_after: Option<NaiveDate>,
    due_before: Option<NaiveDate>,
    q: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
struct SearchTaskRow {
    id: Uuid,
    board_id: Uuid,
    list_id: Uuid,
    list_name: String,
    title: String,
    description: String,
    priority: String,
    due_date: Option<String>,
    parent_task_id: Option<Uuid>,
    estimated_minutes: Option<i32>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

async fn search_tasks(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Query(query): Query<TaskSearchQuery>,
) -> ApiResult<Json<Vec<SearchTaskRow>>> {
    if query.min_estimated_minutes.is_some_and(|v| v < 1)
        || query.max_estimated_minutes.is_some_and(|v| v < 1)
        || matches!((query.min_estimated_minutes, query.max_estimated_minutes), (Some(a), Some(b)) if a > b)
    {
        return Err(fail(StatusCode::BAD_REQUEST, "Estimated-time filter range is invalid"));
    }
    if let Some(board_id) = query.board_id {
        let allowed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM boards b
             WHERE b.id=$1 AND b.workspace_id=$2
               AND (EXISTS(SELECT 1 FROM board_members bm WHERE bm.board_id=b.id AND bm.user_id=$3)
                    OR EXISTS(SELECT 1 FROM workspace_members wm WHERE wm.workspace_id=b.workspace_id AND wm.user_id=$3 AND wm.role IN ('owner','admin')))",
        )
        .bind(board_id).bind(workspace_id).bind(user_id)
        .fetch_one(&state.db).await
        .map_err(|error| internal(error, "Could not verify task search project"))?;
        if !allowed { return Err(fail(StatusCode::NOT_FOUND, "Project not found")); }
    }
    let status = query.status.as_deref().map(str::trim).filter(|value| !value.is_empty());
    if status.is_some_and(|value| value.chars().count() > 80) {
        return Err(fail(StatusCode::BAD_REQUEST, "Task status filter is invalid"));
    }
    let priority = query.priority.as_deref().map(str::trim).filter(|value| !value.is_empty());
    if priority.is_some_and(|value| !["low","medium","high"].contains(&value)) {
        return Err(fail(StatusCode::BAD_REQUEST, "Priority filter is invalid"));
    }
    if matches!((query.due_after,query.due_before),(Some(from),Some(to)) if from>to) {
        return Err(fail(StatusCode::BAD_REQUEST, "Due date filter range is invalid"));
    }
    let search = query.q.as_deref().map(str::trim).filter(|value| !value.is_empty());
    if search.is_some_and(|value| value.chars().count() > 100) {
        return Err(fail(StatusCode::BAD_REQUEST, "Search text is too long"));
    }
    let rows = sqlx::query_as::<_, SearchTaskRow>(
        "SELECT t.id, b.id AS board_id, t.list_id, l.name AS list_name,
                t.title, COALESCE(t.description,'') AS description, t.priority,
                CASE WHEN t.due_at IS NULL THEN NULL ELSE to_char(t.due_at AT TIME ZONE 'UTC','YYYY-MM-DD') END AS due_date,
                t.parent_task_id, t.estimated_minutes, t.created_at, t.updated_at
           FROM tasks t
           JOIN lists l ON l.id=t.list_id
           JOIN boards b ON b.id=l.board_id
          WHERE b.workspace_id=$1
            AND ($2::uuid IS NULL OR b.id=$2)
            AND ($3::uuid IS NULL OR EXISTS(SELECT 1 FROM task_labels tl WHERE tl.task_id=t.id AND tl.label_id=$3))
            AND ($4::uuid IS NULL OR t.parent_task_id=$4)
            AND ($5::int IS NULL OR t.estimated_minutes >= $5)
            AND ($6::int IS NULL OR t.estimated_minutes <= $6)
            AND ($7::bool IS NULL OR EXISTS(SELECT 1 FROM task_dependencies td WHERE td.task_id=t.id)=$7)
            AND ($8::text IS NULL OR lower(l.name)=lower($8))
            AND ($9::uuid IS NULL OR EXISTS(SELECT 1 FROM task_assignees ta WHERE ta.task_id=t.id AND ta.user_id=$9))
            AND ($10::text IS NULL OR t.priority=$10)
            AND ($11::date IS NULL OR t.due_at::date >= $11)
            AND ($12::date IS NULL OR t.due_at::date <= $12)
            AND ($13::text IS NULL OR t.title ILIKE '%'||$13||'%' OR COALESCE(t.description,'') ILIKE '%'||$13||'%')
            AND (EXISTS(SELECT 1 FROM board_members bm WHERE bm.board_id=b.id AND bm.user_id=$14)
                 OR EXISTS(SELECT 1 FROM workspace_members wm WHERE wm.workspace_id=b.workspace_id AND wm.user_id=$14 AND wm.role IN ('owner','admin')))
          ORDER BY t.due_at NULLS LAST, t.updated_at DESC
          LIMIT 250",
    )
    .bind(workspace_id)
    .bind(query.board_id)
    .bind(query.label_id)
    .bind(query.parent_task_id)
    .bind(query.min_estimated_minutes)
    .bind(query.max_estimated_minutes)
    .bind(query.has_blockers)
    .bind(status)
    .bind(query.assignee_id)
    .bind(priority)
    .bind(query.due_after)
    .bind(query.due_before)
    .bind(search)
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not search workspace tasks"))?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
struct AnalyticsRange {
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
}

#[derive(Debug, Serialize, FromRow)]
struct AnalyticsSnapshot {
    snapshot_date: NaiveDate,
    total_tasks: i64,
    completed_tasks: i64,
    overdue_tasks: i64,
    unassigned_tasks: i64,
    active_projects: i64,
    completed_projects: i64,
    captured_at: DateTime<Utc>,
}

fn analytics_range(range: &AnalyticsRange) -> ApiResult<(NaiveDate, NaiveDate)> {
    let today = Utc::now().date_naive();
    let from = range.from.unwrap_or(today - chrono::Duration::days(30));
    let to = range.to.unwrap_or(today);
    if to < from || (to - from).num_days() > 366 {
        return Err(fail(StatusCode::BAD_REQUEST, "Analytics range must be within 366 days"));
    }
    Ok((from, to))
}

async fn analytics_history(
    State(state): State<AppState>,
    WorkspaceUser(_user_id, workspace_id): WorkspaceUser,
    Query(range): Query<AnalyticsRange>,
) -> ApiResult<Json<Vec<AnalyticsSnapshot>>> {
    let (from, to) = analytics_range(&range)?;
    let rows = sqlx::query_as::<_, AnalyticsSnapshot>(
        "SELECT snapshot_date,total_tasks,completed_tasks,overdue_tasks,unassigned_tasks,
                active_projects,completed_projects,captured_at
           FROM workspace_analytics_daily
          WHERE workspace_id=$1 AND snapshot_date BETWEEN $2 AND $3
          ORDER BY snapshot_date",
    )
    .bind(workspace_id).bind(from).bind(to)
    .fetch_all(&state.db).await
    .map_err(|error| internal(error, "Could not load analytics history"))?;
    Ok(Json(rows))
}

async fn analytics_export(
    State(state): State<AppState>,
    WorkspaceUser(_user_id, workspace_id): WorkspaceUser,
    Query(range): Query<AnalyticsRange>,
) -> ApiResult<Response> {
    let (from, to) = analytics_range(&range)?;
    let rows = sqlx::query_as::<_, AnalyticsSnapshot>(
        "SELECT snapshot_date,total_tasks,completed_tasks,overdue_tasks,unassigned_tasks,
                active_projects,completed_projects,captured_at
           FROM workspace_analytics_daily
          WHERE workspace_id=$1 AND snapshot_date BETWEEN $2 AND $3
          ORDER BY snapshot_date",
    )
    .bind(workspace_id).bind(from).bind(to)
    .fetch_all(&state.db).await
    .map_err(|error| internal(error, "Could not export analytics history"))?;
    let mut csv = String::from("date,total_tasks,completed_tasks,overdue_tasks,unassigned_tasks,active_projects,completed_projects,captured_at\n");
    for row in rows {
        csv.push_str(&format!("{},{},{},{},{},{},{},{}\n",
            row.snapshot_date, row.total_tasks, row.completed_tasks, row.overdue_tasks,
            row.unassigned_tasks, row.active_projects, row.completed_projects, row.captured_at.to_rfc3339()));
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(header::CONTENT_DISPOSITION, "attachment; filename=\"campusforge-analytics.csv\"")
        .header(header::CACHE_CONTROL, "no-store")
        .body(axum::body::Body::from(csv))
        .map_err(|error| internal(error, "Could not prepare analytics export"))
}

pub async fn capture_daily_analytics(state: &AppState) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO workspace_analytics_daily
            (workspace_id,snapshot_date,total_tasks,completed_tasks,overdue_tasks,
             unassigned_tasks,active_projects,completed_projects,captured_at)
         SELECT w.id, CURRENT_DATE,
                COUNT(t.id)::bigint,
                COUNT(t.id) FILTER (WHERE lower(l.name) IN ('done','complete','completed'))::bigint,
                COUNT(t.id) FILTER (WHERE t.due_at < NOW() AND lower(l.name) NOT IN ('done','complete','completed'))::bigint,
                COUNT(t.id) FILTER (WHERE t.id IS NOT NULL AND NOT EXISTS(
                    SELECT 1 FROM task_assignees ta WHERE ta.task_id=t.id
                ))::bigint,
                COUNT(DISTINCT b.id)::bigint,
                COUNT(DISTINCT b.id) FILTER (WHERE COALESCE(project_counts.total,0)>0 AND project_counts.completed=project_counts.total)::bigint,
                NOW()
           FROM workspaces w
           LEFT JOIN boards b ON b.workspace_id=w.id
           LEFT JOIN lists l ON l.board_id=b.id
           LEFT JOIN tasks t ON t.list_id=l.id
           LEFT JOIN LATERAL (
               SELECT COUNT(t2.id)::bigint AS total,
                      COUNT(t2.id) FILTER (WHERE lower(l2.name) IN ('done','complete','completed'))::bigint AS completed
                 FROM tasks t2 JOIN lists l2 ON l2.id=t2.list_id
                WHERE l2.board_id=b.id
           ) project_counts ON TRUE
          GROUP BY w.id
         ON CONFLICT (workspace_id,snapshot_date) DO UPDATE SET
            total_tasks=EXCLUDED.total_tasks,
            completed_tasks=EXCLUDED.completed_tasks,
            overdue_tasks=EXCLUDED.overdue_tasks,
            unassigned_tasks=EXCLUDED.unassigned_tasks,
            active_projects=EXCLUDED.active_projects,
            completed_projects=EXCLUDED.completed_projects,
            captured_at=NOW()",
    ).execute(&state.db).await?;
    Ok(result.rows_affected())
}

#[derive(Debug, Serialize, FromRow)]
struct DirectConversation {
    id: Uuid,
    workspace_id: Uuid,
    other_user_id: Uuid,
    other_name: String,
    updated_at: DateTime<Utc>,
    last_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DirectConversationInput { other_user_id: Uuid }

async fn direct_conversations(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
) -> ApiResult<Json<Vec<DirectConversation>>> {
    let rows = sqlx::query_as::<_, DirectConversation>(
        "SELECT c.id,c.workspace_id,
                CASE WHEN c.participant_low=$1 THEN c.participant_high ELSE c.participant_low END AS other_user_id,
                u.full_name AS other_name,c.updated_at,
                (SELECT m.body FROM chat_messages m WHERE m.conversation_id=c.id AND m.deleted_at IS NULL ORDER BY m.created_at DESC LIMIT 1) AS last_message
           FROM chat_conversations c
           JOIN users u ON u.id=CASE WHEN c.participant_low=$1 THEN c.participant_high ELSE c.participant_low END
          WHERE c.workspace_id=$2 AND (c.participant_low=$1 OR c.participant_high=$1)
          ORDER BY c.updated_at DESC",
    )
    .bind(user_id).bind(workspace_id)
    .fetch_all(&state.db).await
    .map_err(|error| internal(error, "Could not list direct conversations"))?;
    Ok(Json(rows))
}

async fn direct_conversation_create(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Json(input): Json<DirectConversationInput>,
) -> ApiResult<(StatusCode, Json<DirectConversation>)> {
    if input.other_user_id == user_id {
        return Err(fail(StatusCode::BAD_REQUEST, "Choose another workspace member"));
    }
    let other_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM workspace_members WHERE workspace_id=$1 AND user_id=$2)",
    )
    .bind(workspace_id).bind(input.other_user_id)
    .fetch_one(&state.db).await
    .map_err(|error| internal(error, "Could not validate direct-message participant"))?;
    if !other_exists { return Err(fail(StatusCode::NOT_FOUND, "That person is not in this workspace")); }
    let (low, high) = if user_id < input.other_user_id { (user_id, input.other_user_id) } else { (input.other_user_id, user_id) };
    let conversation_id: Uuid = sqlx::query_scalar(
        "INSERT INTO chat_conversations(workspace_id,participant_low,participant_high,created_by)
         VALUES($1,$2,$3,$4)
         ON CONFLICT(workspace_id,participant_low,participant_high)
         DO UPDATE SET updated_at=NOW()
         RETURNING id",
    )
    .bind(workspace_id).bind(low).bind(high).bind(user_id)
    .fetch_one(&state.db).await
    .map_err(|error| internal(error, "Could not create direct conversation"))?;
    let row = sqlx::query_as::<_, DirectConversation>(
        "SELECT c.id,c.workspace_id,
                CASE WHEN c.participant_low=$1 THEN c.participant_high ELSE c.participant_low END AS other_user_id,
                u.full_name AS other_name,c.updated_at,
                (SELECT m.body FROM chat_messages m WHERE m.conversation_id=c.id AND m.deleted_at IS NULL ORDER BY m.created_at DESC LIMIT 1) AS last_message
           FROM chat_conversations c
           JOIN users u ON u.id=CASE WHEN c.participant_low=$1 THEN c.participant_high ELSE c.participant_low END
          WHERE c.id=$2",
    )
    .bind(user_id).bind(conversation_id)
    .fetch_one(&state.db).await
    .map_err(|error| internal(error, "Could not load direct conversation"))?;

    Ok((StatusCode::CREATED,Json(row)))
}

#[derive(Debug, Serialize, FromRow)]
struct RoadmapChatMessage {
    id: Uuid,
    workspace_id: Uuid,
    board_id: Option<Uuid>,
    conversation_id: Option<Uuid>,
    author_id: Uuid,
    author_name: String,
    body: String,
    is_pinned: bool,
    edited_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct DirectMessageQuery {
    before: Option<DateTime<Utc>>,
    limit: Option<i64>,
}

async fn direct_messages_list(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(conversation_id): Path<Uuid>,
    Query(query): Query<DirectMessageQuery>,
) -> ApiResult<Json<Vec<RoadmapChatMessage>>> {
    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM chat_conversations WHERE id=$1 AND workspace_id=$2
          AND (participant_low=$3 OR participant_high=$3))",
    )
    .bind(conversation_id).bind(workspace_id).bind(user_id)
    .fetch_one(&state.db).await
    .map_err(|error| internal(error, "Could not verify direct conversation access"))?;
    if !allowed { return Err(fail(StatusCode::NOT_FOUND, "Direct conversation not found")); }
    let limit = query.limit.unwrap_or(50).clamp(1,100);
    let rows = sqlx::query_as::<_, RoadmapChatMessage>(
        "SELECT m.id,m.workspace_id,m.board_id,m.conversation_id,m.author_id,u.full_name AS author_name,
                CASE WHEN m.deleted_at IS NULL THEN m.body ELSE 'This message was deleted' END AS body,
                m.is_pinned,m.edited_at,m.created_at
           FROM chat_messages m JOIN users u ON u.id=m.author_id
          WHERE m.conversation_id=$1 AND ($2::timestamptz IS NULL OR m.created_at<$2)
          ORDER BY m.created_at DESC LIMIT $3",
    )
    .bind(conversation_id).bind(query.before).bind(limit)
    .fetch_all(&state.db).await
    .map_err(|error| internal(error, "Could not load direct messages"))?;
    Ok(Json(rows.into_iter().rev().collect()))
}

#[derive(Debug, Deserialize)]
struct DirectMessageInput { body: String }

async fn direct_message_send(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(conversation_id): Path<Uuid>,
    Json(input): Json<DirectMessageInput>,
) -> ApiResult<(StatusCode, Json<RoadmapChatMessage>)> {
    let body = input.body.trim();
    if body.is_empty() || body.chars().count()>4000 {
        return Err(fail(StatusCode::BAD_REQUEST,"Message must be between 1 and 4000 characters"));
    }
    let mut tx=state.db.begin().await.map_err(|e|internal(e,"Could not begin direct message"))?;
    let participants:Option<(Uuid,Uuid)>=sqlx::query_as(
        "SELECT participant_low,participant_high FROM chat_conversations
          WHERE id=$1 AND workspace_id=$2 AND (participant_low=$3 OR participant_high=$3)
          FOR UPDATE",
    )
    .bind(conversation_id).bind(workspace_id).bind(user_id)
    .fetch_optional(&mut *tx).await.map_err(|e|internal(e,"Could not verify direct conversation"))?;
    let Some((participant_low,participant_high))=participants else {
        return Err(fail(StatusCode::NOT_FOUND,"Direct conversation not found"));
    };
    let other_user_id=if participant_low==user_id { participant_high } else { participant_low };
    let message=sqlx::query_as::<_,RoadmapChatMessage>(
        "INSERT INTO chat_messages(workspace_id,author_id,conversation_id,body)
         VALUES($1,$2,$3,$4)
         RETURNING id,workspace_id,board_id,conversation_id,author_id,
           (SELECT full_name FROM users WHERE id=$2) AS author_name,body,is_pinned,edited_at,created_at",
    )
    .bind(workspace_id).bind(user_id).bind(conversation_id).bind(body)
    .fetch_one(&mut *tx).await.map_err(|e|internal(e,"Could not save direct message"))?;
    sqlx::query("UPDATE chat_conversations SET updated_at=NOW() WHERE id=$1")
        .bind(conversation_id).execute(&mut *tx).await.map_err(|e|internal(e,"Could not update conversation time"))?;
    tx.commit().await.map_err(|e|internal(e,"Could not commit direct message"))?;
    state.publish_private_workspace_event(
        workspace_id,other_user_id,Some(user_id),"direct_message_created","A teammate sent you a direct message",
    );
    Ok((StatusCode::CREATED,Json(message)))
}

#[derive(Debug, Deserialize)]
struct MessageEditInput { body: String }
async fn chat_message_update(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(message_id): Path<Uuid>,
    Json(input): Json<MessageEditInput>,
) -> ApiResult<StatusCode> {
    let body=input.body.trim();
    if body.is_empty()||body.chars().count()>4000 { return Err(fail(StatusCode::BAD_REQUEST,"Message must be between 1 and 4000 characters")); }
    let mut tx=state.db.begin().await.map_err(|e|internal(e,"Could not begin message edit"))?;
    let row: Option<(Option<Uuid>,Option<Uuid>,Uuid)> = sqlx::query_as(
        "SELECT board_id,conversation_id,author_id FROM chat_messages WHERE id=$1 AND workspace_id=$2 AND deleted_at IS NULL FOR UPDATE",
    ).bind(message_id).bind(workspace_id).fetch_optional(&mut *tx).await
      .map_err(|e|internal(e,"Could not load message"))?;
    let Some((board_id,conversation_id,author_id))=row else { return Err(fail(StatusCode::NOT_FOUND,"Message not found")); };
    if author_id!=user_id { return Err(fail(StatusCode::FORBIDDEN,"Only the message author can edit this message")); }
    let private_recipient = if let Some(conversation_id)=conversation_id {
        let participants:Option<(Uuid,Uuid)>=sqlx::query_as(
            "SELECT participant_low,participant_high FROM chat_conversations WHERE id=$1 AND workspace_id=$2
              AND (participant_low=$3 OR participant_high=$3)",
        ).bind(conversation_id).bind(workspace_id).bind(user_id)
          .fetch_optional(&mut *tx).await.map_err(|e|internal(e,"Could not verify direct message membership"))?;
        let Some((low,high))=participants else { return Err(fail(StatusCode::FORBIDDEN,"You cannot edit this message")); };
        Some(if low==user_id { high } else { low })
    } else {
        None
    };
    sqlx::query("UPDATE chat_messages SET body=$1,edited_at=NOW() WHERE id=$2")
        .bind(body).bind(message_id).execute(&mut *tx).await.map_err(|e|internal(e,"Could not edit message"))?;
    if private_recipient.is_none() {
        sqlx::query("DELETE FROM chat_message_mentions WHERE message_id=$1").bind(message_id)
            .execute(&mut *tx).await.map_err(|e|internal(e,"Could not refresh message mentions"))?;
        create_mentions_in_transaction(&mut tx,workspace_id,user_id,"chat_message",message_id,body)
            .await.map_err(|e|internal(e,"Could not create mention notifications"))?;
    }
    tx.commit().await.map_err(|e|internal(e,"Could not commit message edit"))?;
    if let Some(recipient)=private_recipient {
        state.publish_private_workspace_event(workspace_id,recipient,Some(user_id),"direct_message_updated","A private message was edited");
    } else {
        state.publish_workspace_event(workspace_id,board_id,"chat_message_updated","a message was edited");
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn chat_message_delete(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(message_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut tx=state.db.begin().await.map_err(|e|internal(e,"Could not begin message deletion"))?;
    let row:Option<(Option<Uuid>,Option<Uuid>,Uuid)>=sqlx::query_as(
        "SELECT board_id,conversation_id,author_id FROM chat_messages
          WHERE id=$1 AND workspace_id=$2 AND deleted_at IS NULL FOR UPDATE",
    ).bind(message_id).bind(workspace_id).fetch_optional(&mut *tx).await
      .map_err(|e|internal(e,"Could not load message before deletion"))?;
    let Some((board_id,conversation_id,author_id))=row else {
        return Err(fail(StatusCode::NOT_FOUND,"Message not found"));
    };
    let role:Option<String>=sqlx::query_scalar(
        "SELECT role FROM workspace_members WHERE workspace_id=$1 AND user_id=$2",
    ).bind(workspace_id).bind(user_id).fetch_optional(&mut *tx).await
      .map_err(|e|internal(e,"Could not verify workspace role"))?;
    let private_recipient = if let Some(conversation_id)=conversation_id {
        let participants:Option<(Uuid,Uuid)>=sqlx::query_as(
            "SELECT participant_low,participant_high FROM chat_conversations WHERE id=$1 AND workspace_id=$2
              AND (participant_low=$3 OR participant_high=$3)",
        ).bind(conversation_id).bind(workspace_id).bind(user_id).fetch_optional(&mut *tx).await
          .map_err(|e|internal(e,"Could not verify direct conversation access"))?;
        let Some((low,high))=participants else { return Err(fail(StatusCode::FORBIDDEN,"You cannot delete this message")); };
        if author_id!=user_id { return Err(fail(StatusCode::FORBIDDEN,"You cannot delete this message")); }
        Some(if low==user_id { high } else { low })
    } else {
        if author_id!=user_id && !matches!(role.as_deref(),Some("owner"|"admin")) {
            return Err(fail(StatusCode::FORBIDDEN,"You cannot delete this message"));
        }
        None
    };
    sqlx::query("UPDATE chat_messages SET deleted_at=NOW(),body='This message was deleted',is_pinned=FALSE WHERE id=$1")
        .bind(message_id).execute(&mut *tx).await.map_err(|e|internal(e,"Could not delete message"))?;
    tx.commit().await.map_err(|e|internal(e,"Could not commit message deletion"))?;
    if let Some(recipient)=private_recipient {
        state.publish_private_workspace_event(workspace_id,recipient,Some(user_id),"direct_message_deleted","A private message was deleted");
    } else {
        state.publish_workspace_event(workspace_id,board_id,"chat_message_deleted","a message was deleted");
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct PinInput { pinned: bool }
async fn chat_message_pin(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(message_id): Path<Uuid>,
    Json(input): Json<PinInput>,
) -> ApiResult<StatusCode> {
    let role:Option<String>=sqlx::query_scalar(
        "SELECT role FROM workspace_members WHERE workspace_id=$1 AND user_id=$2",
    ).bind(workspace_id).bind(user_id).fetch_optional(&state.db).await
      .map_err(|e|internal(e,"Could not verify workspace role"))?;
    let result=sqlx::query(
        "UPDATE chat_messages SET is_pinned=$1
          WHERE id=$2 AND workspace_id=$3 AND conversation_id IS NULL AND deleted_at IS NULL
            AND (author_id=$4 OR $5 IN ('owner','admin'))",
    ).bind(input.pinned).bind(message_id).bind(workspace_id).bind(user_id).bind(role.as_deref().unwrap_or(""))
     .execute(&state.db).await.map_err(|e|internal(e,"Could not pin message"))?;
    if result.rows_affected()==0 { return Err(fail(StatusCode::NOT_FOUND,"Message not found or you cannot pin it")); }
    state.publish_workspace_event(workspace_id,None,"chat_message_pinned","a message pin was updated");
    Ok(StatusCode::NO_CONTENT)
}

async fn pinned_messages(
    State(state): State<AppState>,
    WorkspaceUser(_user_id, workspace_id): WorkspaceUser,
    Query(query): Query<BoardQuery>,
) -> ApiResult<Json<Vec<RoadmapChatMessage>>> {
    let rows=sqlx::query_as::<_,RoadmapChatMessage>(
        "SELECT m.id,m.workspace_id,m.board_id,m.conversation_id,m.author_id,u.full_name AS author_name,
                m.body,m.is_pinned,m.edited_at,m.created_at
           FROM chat_messages m JOIN users u ON u.id=m.author_id
          WHERE m.workspace_id=$1 AND m.is_pinned=TRUE AND m.deleted_at IS NULL
            AND (($2::uuid IS NULL AND m.board_id IS NULL AND m.conversation_id IS NULL) OR m.board_id=$2)
          ORDER BY m.created_at DESC LIMIT 100",
    ).bind(workspace_id).bind(query.board_id).fetch_all(&state.db).await
      .map_err(|e|internal(e,"Could not load pinned messages"))?;
    Ok(Json(rows))
}
#[derive(Debug,Deserialize)]
struct BoardQuery { board_id: Option<Uuid> }

#[derive(Debug, Serialize, FromRow)]
struct ActionItemRow {
    id: Uuid,
    meeting_note_id: Uuid,
    title: String,
    assigned_to: Option<Uuid>,
    due_at: Option<DateTime<Utc>>,
    linked_task_id: Option<Uuid>,
    is_done: bool,
    position: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct ActionItemInput {
    title: String,
    assigned_to: Option<Uuid>,
    due_at: Option<DateTime<Utc>>,
    linked_task_id: Option<Uuid>,
    is_done: Option<bool>,
}

async fn action_items_list(
    State(state): State<AppState>,
    WorkspaceUser(_user_id, workspace_id): WorkspaceUser,
    Path(note_id): Path<Uuid>,
) -> ApiResult<Json<Vec<ActionItemRow>>> {
    let rows=sqlx::query_as::<_,ActionItemRow>(
        "SELECT id,meeting_note_id,title,assigned_to,due_at,linked_task_id,is_done,position,created_at,updated_at
           FROM meeting_note_action_items WHERE meeting_note_id=$1 AND workspace_id=$2 ORDER BY position,created_at",
    ).bind(note_id).bind(workspace_id).fetch_all(&state.db).await
      .map_err(|e|internal(e,"Could not load meeting action items"))?;
    Ok(Json(rows))
}

async fn action_item_create(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(note_id): Path<Uuid>,
    Json(input): Json<ActionItemInput>,
) -> ApiResult<(StatusCode,Json<ActionItemRow>)> {
    let title=input.title.trim();
    if title.is_empty()||title.chars().count()>500 { return Err(fail(StatusCode::BAD_REQUEST,"Action item title must be between 1 and 500 characters")); }
    if let Some(assigned)=input.assigned_to {
        let member:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspace_members WHERE workspace_id=$1 AND user_id=$2)")
            .bind(workspace_id).bind(assigned).fetch_one(&state.db).await.map_err(|e|internal(e,"Could not validate action owner"))?;
        if !member { return Err(fail(StatusCode::BAD_REQUEST,"Action owner must be a workspace member")); }
    }
    if let Some(task_id)=input.linked_task_id {
        let exists:bool=sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM tasks t JOIN lists l ON l.id=t.list_id JOIN boards b ON b.id=l.board_id WHERE t.id=$1 AND b.workspace_id=$2)",
        ).bind(task_id).bind(workspace_id).fetch_one(&state.db).await.map_err(|e|internal(e,"Could not validate linked task"))?;
        if !exists { return Err(fail(StatusCode::BAD_REQUEST,"Linked task must belong to this workspace")); }
    }
    let mut tx=state.db.begin().await.map_err(|e|internal(e,"Could not begin action item"))?;
    let note_exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM meeting_notes WHERE id=$1 AND workspace_id=$2)")
        .bind(note_id).bind(workspace_id).fetch_one(&mut *tx).await.map_err(|e|internal(e,"Could not verify meeting notes"))?;
    if !note_exists { return Err(fail(StatusCode::NOT_FOUND,"Meeting notes not found")); }
    let position:i32=sqlx::query_scalar("SELECT COALESCE(MAX(position),0)+1 FROM meeting_note_action_items WHERE meeting_note_id=$1")
        .bind(note_id).fetch_one(&mut *tx).await.map_err(|e|internal(e,"Could not order action items"))?;
    let row=sqlx::query_as::<_,ActionItemRow>(
        "INSERT INTO meeting_note_action_items(meeting_note_id,workspace_id,title,assigned_to,due_at,linked_task_id,is_done,position,created_by)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9)
         RETURNING id,meeting_note_id,title,assigned_to,due_at,linked_task_id,is_done,position,created_at,updated_at",
    ).bind(note_id).bind(workspace_id).bind(title).bind(input.assigned_to).bind(input.due_at).bind(input.linked_task_id)
     .bind(input.is_done.unwrap_or(false)).bind(position).bind(user_id)
     .fetch_one(&mut *tx).await.map_err(|e|internal(e,"Could not create meeting action item"))?;
    if let Some(assigned)=input.assigned_to {
        if assigned!=user_id {
            create_notification_in_transaction(&mut tx,workspace_id,assigned,Some(user_id),"task_assigned","Meeting action assigned",
                "A meeting action item was assigned to you.",Some("meeting_action_item"),Some(row.id),
                Some(&format!("meeting-action:{}:{}",row.id,assigned))).await.map_err(|e|internal(e,"Could not notify action owner"))?;
        }
    }
    tx.commit().await.map_err(|e|internal(e,"Could not save meeting action item"))?;
    state.publish_workspace_event(workspace_id,None,"meeting_action_created","a meeting action item was created");
    Ok((StatusCode::CREATED,Json(row)))
}

async fn action_item_update(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(item_id): Path<Uuid>,
    Json(input): Json<ActionItemInput>,
) -> ApiResult<StatusCode> {
    let title=input.title.trim();
    if title.is_empty()||title.chars().count()>500 { return Err(fail(StatusCode::BAD_REQUEST,"Action item title must be between 1 and 500 characters")); }
    if let Some(assigned)=input.assigned_to {
        let member:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspace_members WHERE workspace_id=$1 AND user_id=$2)")
            .bind(workspace_id).bind(assigned).fetch_one(&state.db).await.map_err(|e|internal(e,"Could not validate action owner"))?;
        if !member { return Err(fail(StatusCode::BAD_REQUEST,"Action owner must be a workspace member")); }
    }
    if let Some(task_id)=input.linked_task_id {
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tasks t JOIN lists l ON l.id=t.list_id JOIN boards b ON b.id=l.board_id WHERE t.id=$1 AND b.workspace_id=$2)")
            .bind(task_id).bind(workspace_id).fetch_one(&state.db).await.map_err(|e|internal(e,"Could not validate linked task"))?;
        if !exists { return Err(fail(StatusCode::BAD_REQUEST,"Linked task must belong to this workspace")); }
    }
    let result=sqlx::query(
        "UPDATE meeting_note_action_items SET title=$1,assigned_to=$2,due_at=$3,linked_task_id=$4,
              is_done=COALESCE($5,is_done),updated_at=NOW()
         WHERE id=$6 AND workspace_id=$7",
    ).bind(title).bind(input.assigned_to).bind(input.due_at).bind(input.linked_task_id).bind(input.is_done)
     .bind(item_id).bind(workspace_id).execute(&state.db).await.map_err(|e|internal(e,"Could not update meeting action item"))?;
    if result.rows_affected()==0 { return Err(fail(StatusCode::NOT_FOUND,"Meeting action item not found")); }
    state.publish_workspace_event(workspace_id,None,"meeting_action_updated","a meeting action item was updated");
    Ok(StatusCode::NO_CONTENT)
}

async fn action_item_delete(
    State(state): State<AppState>,
    WorkspaceUser(_user_id, workspace_id): WorkspaceUser,
    Path(item_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let result=sqlx::query("DELETE FROM meeting_note_action_items WHERE id=$1 AND workspace_id=$2")
        .bind(item_id).bind(workspace_id).execute(&state.db).await.map_err(|e|internal(e,"Could not delete meeting action item"))?;
    if result.rows_affected()==0 { return Err(fail(StatusCode::NOT_FOUND,"Meeting action item not found")); }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, FromRow)]
struct FileUsage {
    used_bytes: i64,
    reserved_bytes: i64,
    quota_bytes: i64,
    file_count: i64,
}

async fn file_usage(
    State(state): State<AppState>,
    WorkspaceUser(_user_id, workspace_id): WorkspaceUser,
) -> ApiResult<Json<FileUsage>> {
    sqlx::query("INSERT INTO workspace_storage_quotas(workspace_id,quota_bytes) VALUES($1,104857600) ON CONFLICT DO NOTHING")
        .bind(workspace_id).execute(&state.db).await.map_err(|e|internal(e,"Could not initialize workspace storage quota"))?;
    let row=sqlx::query_as::<_,FileUsage>(
        "SELECT COALESCE((SELECT SUM(size_bytes) FROM workspace_files WHERE workspace_id=$1 AND is_deleting=FALSE),0)::bigint AS used_bytes,
                COALESCE((SELECT SUM(size_bytes) FROM workspace_storage_reservations WHERE workspace_id=$1),0)::bigint AS reserved_bytes,
                q.quota_bytes,
                (SELECT COUNT(*) FROM workspace_files WHERE workspace_id=$1 AND is_deleting=FALSE)::bigint AS file_count
           FROM workspace_storage_quotas q WHERE q.workspace_id=$1",
    ).bind(workspace_id).fetch_one(&state.db).await.map_err(|e|internal(e,"Could not load workspace storage usage"))?;
    Ok(Json(row))
}

#[derive(Debug,Deserialize)]
struct FileQuotaInput { quota_bytes: i64 }
async fn file_quota_update(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Json(input): Json<FileQuotaInput>,
) -> ApiResult<StatusCode> {
    if !(10*1024*1024..=100*1024*1024*1024).contains(&input.quota_bytes) {
        return Err(fail(StatusCode::BAD_REQUEST,"Workspace quota must be between 10 MB and 100 GB"));
    }
    if !workspace_admin(&state,user_id,workspace_id).await.map_err(|e|internal(e,"Could not verify quota permission"))? {
        return Err(fail(StatusCode::FORBIDDEN,"Only workspace owners and admins can change storage quotas"));
    }
    let mut tx=state.db.begin().await.map_err(|e|internal(e,"Could not begin storage quota update"))?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(workspace_id.to_string()).execute(&mut *tx).await
        .map_err(|e|internal(e,"Could not lock workspace storage quota"))?;
    let (used,reserved):(i64,i64)=sqlx::query_as(
        "SELECT
            COALESCE((SELECT SUM(size_bytes) FROM workspace_files WHERE workspace_id=$1 AND is_deleting=FALSE),0)::bigint,
            COALESCE((SELECT SUM(size_bytes) FROM workspace_storage_reservations WHERE workspace_id=$1),0)::bigint",
    ).bind(workspace_id).fetch_one(&mut *tx).await
      .map_err(|e|internal(e,"Could not calculate storage usage"))?;
    if used.saturating_add(reserved)>input.quota_bytes {
        return Err(fail(StatusCode::CONFLICT,"The new quota cannot be lower than current file usage and pending uploads"));
    }
    sqlx::query(
        "INSERT INTO workspace_storage_quotas(workspace_id,quota_bytes,updated_by,updated_at)
         VALUES($1,$2,$3,NOW()) ON CONFLICT(workspace_id) DO UPDATE
         SET quota_bytes=EXCLUDED.quota_bytes,updated_by=EXCLUDED.updated_by,updated_at=NOW()",
    ).bind(workspace_id).bind(input.quota_bytes).bind(user_id)
     .execute(&mut *tx).await.map_err(|e|internal(e,"Could not update storage quota"))?;
    tx.commit().await.map_err(|e|internal(e,"Could not save storage quota"))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug,Serialize,FromRow)]
struct FileAuditRow {
    id: Uuid,
    file_id: Option<Uuid>,
    actor_user_id: Option<Uuid>,
    action: String,
    original_name: String,
    size_bytes: Option<i64>,
    metadata: serde_json::Value,
    created_at: DateTime<Utc>,
}
async fn file_audit_list(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
) -> ApiResult<Json<Vec<FileAuditRow>>> {
    if !workspace_admin(&state,user_id,workspace_id).await.map_err(|e|internal(e,"Could not verify file audit permission"))? {
        return Err(fail(StatusCode::FORBIDDEN,"Only workspace owners and admins can view file audit history"));
    }
    let rows=sqlx::query_as::<_,FileAuditRow>(
        "SELECT id,file_id,actor_user_id,action,original_name,size_bytes,metadata,created_at
           FROM file_audit_events WHERE workspace_id=$1 ORDER BY created_at DESC LIMIT 200",
    ).bind(workspace_id).fetch_all(&state.db).await.map_err(|e|internal(e,"Could not load file audit history"))?;
    Ok(Json(rows))
}

async fn file_preview(
    State(state): State<AppState>,
    WorkspaceUser(_user_id, workspace_id): WorkspaceUser,
    Path(file_id): Path<Uuid>,
) -> ApiResult<Response> {
    let row: Option<(String,String,String,String,i64)> = sqlx::query_as(
        "SELECT storage_key,original_name,media_type,scan_status,size_bytes FROM workspace_files
          WHERE id=$1 AND workspace_id=$2 AND is_deleting=FALSE",
    ).bind(file_id).bind(workspace_id).fetch_optional(&state.db).await
      .map_err(|e|internal(e,"Could not load preview metadata"))?;
    let Some((key,name,media_type,scan_status,size_bytes))=row else { return Err(fail(StatusCode::NOT_FOUND,"File not found")); };
    if scan_status=="infected" || (std::env::var("CAMPUSFORGE_REQUIRE_MALWARE_SCAN").ok().is_some_and(|v|v.eq_ignore_ascii_case("true")) && scan_status!="clean") {
        return Err(fail(StatusCode::LOCKED,"File preview is unavailable until the file passes malware scanning"));
    }
    if !matches!(media_type.as_str(),"application/pdf"|"image/png"|"image/jpeg"|"image/gif"|"image/webp") {
        return Err(fail(StatusCode::UNSUPPORTED_MEDIA_TYPE,"Only PDF and raster image files can be previewed"));
    }
    let bytes=state.storage.get(&key).await.map_err(|e|{tracing::error!(%e,"File preview read failed");fail(StatusCode::NOT_FOUND,"Stored file is unavailable")})?;
    let mut tx=state.db.begin().await.map_err(|e|internal(e,"Could not begin file preview audit"))?;
    sqlx::query("INSERT INTO file_audit_events(workspace_id,file_id,actor_user_id,action,original_name,size_bytes) VALUES($1,$2,$3,'previewed',$4,$5)")
        .bind(workspace_id).bind(file_id).bind(_user_id).bind(&name).bind(size_bytes)
        .execute(&mut *tx).await.map_err(|e|internal(e,"Could not record file preview"))?;
    tx.commit().await.map_err(|e|internal(e,"Could not save file preview audit"))?;
    let content_type=HeaderValue::from_str(&media_type).unwrap_or_else(|_|HeaderValue::from_static("application/octet-stream"));
    Response::builder().status(StatusCode::OK)
        .header(header::CONTENT_TYPE,content_type)
        .header(header::CONTENT_DISPOSITION,"inline")
        .header("x-content-type-options","nosniff")
        .header("content-security-policy","default-src 'none'; sandbox")
        .header(header::CACHE_CONTROL,"no-store")
        .body(axum::body::Body::from(bytes))
        .map_err(|e|internal(e,"Could not prepare file preview"))
}

async fn calendar_attendees_update(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(event_id): Path<Uuid>,
    Json(input): Json<EventAttendeesInput>,
) -> ApiResult<StatusCode> {
    if input.user_ids.len()>100 { return Err(fail(StatusCode::BAD_REQUEST,"An event can have at most 100 attendees")); }
    let unique: HashSet<Uuid>=input.user_ids.iter().copied().collect();
    if unique.len()!=input.user_ids.len() { return Err(fail(StatusCode::BAD_REQUEST,"Attendee list contains duplicates")); }
    let event: Option<(Uuid,Option<Uuid>,Uuid)>=sqlx::query_as(
        "SELECT id,board_id,created_by FROM calendar_events WHERE id=$1 AND workspace_id=$2",
    ).bind(event_id).bind(workspace_id).fetch_optional(&state.db).await
      .map_err(|e|internal(e,"Could not verify event attendee access"))?;
    let Some((_id,board_id,created_by))=event else { return Err(fail(StatusCode::NOT_FOUND,"Calendar event not found")); };
    if created_by!=user_id && !workspace_admin(&state,user_id,workspace_id).await.map_err(|e|internal(e,"Could not verify event permission"))? {
        return Err(fail(StatusCode::FORBIDDEN,"Only the event creator or a workspace admin can manage attendees"));
    }
    let mut tx=state.db.begin().await.map_err(|e|internal(e,"Could not begin attendee update"))?;
    for attendee in &input.user_ids {
        let member:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM workspace_members WHERE workspace_id=$1 AND user_id=$2)")
            .bind(workspace_id).bind(attendee).fetch_one(&mut *tx).await.map_err(|e|internal(e,"Could not validate event attendee"))?;
        if !member { return Err(fail(StatusCode::BAD_REQUEST,"Every event attendee must be a workspace member")); }
    }
    sqlx::query("DELETE FROM calendar_event_attendees WHERE event_id=$1 AND NOT(user_id=ANY($2))")
        .bind(event_id).bind(&input.user_ids).execute(&mut *tx).await.map_err(|e|internal(e,"Could not remove event attendees"))?;
    for attendee in &input.user_ids {
        let inserted=sqlx::query(
            "INSERT INTO calendar_event_attendees(event_id,user_id)
             VALUES($1,$2) ON CONFLICT(event_id,user_id) DO NOTHING",
        ).bind(event_id).bind(attendee).execute(&mut *tx).await.map_err(|e|internal(e,"Could not save event attendee"))?;
        if inserted.rows_affected()>0 && *attendee!=user_id {
            create_notification_in_transaction(&mut tx,workspace_id,*attendee,Some(user_id),
                "calendar_invitation","Calendar invitation","You were invited to a calendar event.",
                Some("calendar_event"),Some(event_id),Some(&format!("calendar-invite:{event_id}:{attendee}")))
                .await.map_err(|e|internal(e,"Could not notify calendar attendee"))?;
        }
    }
    tx.commit().await.map_err(|e|internal(e,"Could not save event attendees"))?;
    state.publish_workspace_event(workspace_id,board_id,"calendar_attendees_updated","calendar attendees were updated");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug,Deserialize)]
struct EventAttendeesInput { user_ids: Vec<Uuid> }
#[derive(Debug,Deserialize)]
struct EventAttendeeResponseInput { response: String }
async fn calendar_attendee_response(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(event_id): Path<Uuid>,
    Json(input): Json<EventAttendeeResponseInput>,
) -> ApiResult<StatusCode> {
    if !["accepted","declined"].contains(&input.response.as_str()) {
        return Err(fail(StatusCode::BAD_REQUEST,"Response must be accepted or declined"));
    }
    let result=sqlx::query(
        "UPDATE calendar_event_attendees a SET response=$1,updated_at=NOW()
           FROM calendar_events e
          WHERE a.event_id=e.id AND a.event_id=$2 AND a.user_id=$3 AND e.workspace_id=$4",
    ).bind(&input.response).bind(event_id).bind(user_id).bind(workspace_id)
     .execute(&state.db).await.map_err(|e|internal(e,"Could not respond to event invitation"))?;
    if result.rows_affected()==0 { return Err(fail(StatusCode::NOT_FOUND,"Event invitation not found")); }
    state.publish_workspace_event(workspace_id,None,"calendar_attendee_response","an event attendee responded");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug,FromRow)]
struct DueEventReminder {
    event_id: Uuid,
    workspace_id: Uuid,
    user_id: Uuid,
    title: String,
}

async fn process_calendar_reminders(state: &AppState) {
    let mut tx=match state.db.begin().await {
        Ok(tx)=>tx,
        Err(error)=>{tracing::error!(%error,"Could not begin calendar reminder job");return;}
    };
    let due=match sqlx::query_as::<_,DueEventReminder>(
        "SELECT e.id AS event_id,e.workspace_id,a.user_id,e.title
           FROM calendar_event_attendees a
           JOIN calendar_events e ON e.id=a.event_id
          WHERE a.reminder_sent_at IS NULL
            AND e.reminder_minutes IS NOT NULL
            AND e.start_at - make_interval(mins => e.reminder_minutes) <= NOW()
            AND e.end_at > NOW()
            AND a.response <> 'declined'
          ORDER BY e.start_at
          LIMIT 100
          FOR UPDATE OF a SKIP LOCKED",
    ).fetch_all(&mut *tx).await {
        Ok(rows)=>rows,
        Err(error)=>{tracing::error!(%error,"Could not load event reminders");return;}
    };
    let mut notified=Vec::new();
    for item in due {
        if let Err(error)=create_notification_in_transaction(
            &mut tx,item.workspace_id,item.user_id,None,"calendar_reminder",
            "Upcoming calendar event",&format!("Reminder: {} is starting soon.",item.title),
            Some("calendar_event"),Some(item.event_id),
            Some(&format!("calendar-reminder:{}:{}",item.event_id,item.user_id)),
        ).await {
            tracing::error!(%error,event_id=%item.event_id,"Could not create calendar reminder");
            return;
        }
        if let Err(error)=sqlx::query(
            "UPDATE calendar_event_attendees SET reminder_sent_at=NOW(),updated_at=NOW()
             WHERE event_id=$1 AND user_id=$2 AND reminder_sent_at IS NULL",
        ).bind(item.event_id).bind(item.user_id).execute(&mut *tx).await {
            tracing::error!(%error,event_id=%item.event_id,"Could not complete calendar reminder");
            return;
        }
        notified.push(item.workspace_id);
    }
    if let Err(error)=tx.commit().await {
        tracing::error!(%error,"Could not commit calendar reminders");
        return;
    }
    for workspace_id in notified {
        state.publish_workspace_event(workspace_id,None,"notification_created","a calendar reminder is available");
    }
}

async fn cleanup_stale_storage_reservations(state: &AppState) {
    if let Err(error)=sqlx::query(
        "DELETE FROM workspace_storage_reservations WHERE created_at < NOW() - INTERVAL '1 hour'",
    ).execute(&state.db).await {
        tracing::error!(%error,"Could not clean stale file quota reservations");
    }

    if let Err(error)=sqlx::query(
        "DELETE FROM workspace_presence WHERE last_seen_at < NOW() - INTERVAL '90 seconds'",
    ).execute(&state.db).await {
        tracing::error!(%error,"Could not clean expired workspace presence leases");
    }
}

pub async fn run_roadmap_jobs(state: AppState) {
    if let Err(error)=capture_daily_analytics(&state).await {
        tracing::error!(%error,"Could not capture initial workspace analytics snapshot");
    }
    let mut tick=interval(TokioDuration::from_secs(60));
    let mut last_snapshot=Utc::now()-chrono::Duration::minutes(15);
    let mut last_reservation_cleanup=Utc::now();
    loop {
        tick.tick().await;
        process_calendar_reminders(&state).await;
        let now=Utc::now();
        if now-last_snapshot >= chrono::Duration::minutes(15) {
            if let Err(error)=capture_daily_analytics(&state).await {
                tracing::error!(%error,"Could not refresh daily workspace analytics snapshot");
            } else {
                last_snapshot=now;
            }
        }
        if now-last_reservation_cleanup >= chrono::Duration::minutes(15) {
            cleanup_stale_storage_reservations(&state).await;
            last_reservation_cleanup=now;
        }
    }
}

#[derive(Debug, Deserialize)]
struct PresenceQuery {
    board_id: Option<Uuid>,
}

#[derive(Debug, Serialize, FromRow)]
struct PresenceRow {
    user_id: Uuid,
    full_name: String,
    last_seen_at: DateTime<Utc>,
}

async fn workspace_presence(
    State(state): State<AppState>,
    WorkspaceUser(_user_id, workspace_id): WorkspaceUser,
    Query(query): Query<PresenceQuery>,
) -> ApiResult<Json<Vec<PresenceRow>>> {
    let rows=sqlx::query_as::<_,PresenceRow>(
        "SELECT p.user_id,u.full_name,MAX(p.last_seen_at) AS last_seen_at
           FROM workspace_presence p
           JOIN users u ON u.id=p.user_id
          WHERE p.workspace_id=$1
            AND p.last_seen_at >= NOW()-INTERVAL '90 seconds'
            AND ($2::uuid IS NULL OR p.board_id=$2)
          GROUP BY p.user_id,u.full_name
          ORDER BY u.full_name",
    ).bind(workspace_id).bind(query.board_id).fetch_all(&state.db).await
      .map_err(|error|internal(error,"Could not load workspace presence"))?;
    Ok(Json(rows))
}

const SAVE_LOCKER_MAX_ITEM_BYTES: usize = 9 * 1024 * 1024;
const SAVE_LOCKER_MAX_TOTAL_BYTES: i64 = 50 * 1024 * 1024;
const SAVE_LOCKER_MAX_ITEMS: i64 = 100;
const SAVE_LOCKER_KDF_ITERATIONS: i32 = 600_000;

fn valid_locker_base64(value: &str, min_len: usize, max_len: usize) -> bool {
    if value.len() < min_len || value.len() > max_len || value.len() % 4 != 0 {
        return false;
    }
    let padding_start = value.find('=').unwrap_or(value.len());
    let data = &value.as_bytes()[..padding_start];
    let padding = &value.as_bytes()[padding_start..];
    data.iter().all(|byte| byte.is_ascii_alphanumeric() || *byte == b'+' || *byte == b'/')
        && padding.len() <= 2
        && padding.iter().all(|byte| *byte == b'=')
}

fn validate_locker_item(iv: &str, ciphertext: &str) -> ApiResult<i64> {
    if !valid_locker_base64(iv, 16, 16) || iv.contains('=') {
        return Err(fail(StatusCode::BAD_REQUEST, "Save Locker item IV is invalid"));
    }
    if !valid_locker_base64(ciphertext, 24, SAVE_LOCKER_MAX_ITEM_BYTES) {
        return Err(fail(StatusCode::BAD_REQUEST, "Encrypted Save Locker item is invalid or exceeds the per-item limit"));
    }
    Ok(ciphertext.len() as i64)
}

#[derive(Debug, Serialize, FromRow)]
struct SaveLockerVaultRow {
    salt: String,
    kdf_iterations: i32,
    key_check_iv: String,
    key_check_ciphertext: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct SaveLockerVaultState {
    exists: bool,
    vault: Option<SaveLockerVaultRow>,
    item_count: i64,
    used_bytes: i64,
}

async fn save_locker_vault_get(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> ApiResult<Json<SaveLockerVaultState>> {
    let vault = sqlx::query_as::<_, SaveLockerVaultRow>(
        "SELECT salt,kdf_iterations,key_check_iv,key_check_ciphertext,created_at,updated_at
           FROM save_locker_vaults WHERE user_id=$1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load Save Locker metadata"))?;

    let (item_count, used_bytes) = if vault.is_some() {
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT COUNT(*)::bigint,COALESCE(SUM(size_bytes),0)::bigint
               FROM save_locker_items WHERE user_id=$1",
        )
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .map_err(|error| internal(error, "Could not load Save Locker usage"))?
    } else {
        (0, 0)
    };

    Ok(Json(SaveLockerVaultState {
        exists: vault.is_some(),
        vault,
        item_count,
        used_bytes,
    }))
}

#[derive(Debug, Deserialize)]
struct SaveLockerVaultCreateInput {
    salt: String,
    kdf_iterations: i32,
    key_check_iv: String,
    key_check_ciphertext: String,
}

async fn save_locker_vault_create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(input): Json<SaveLockerVaultCreateInput>,
) -> ApiResult<Json<SaveLockerVaultState>> {
    if input.kdf_iterations != SAVE_LOCKER_KDF_ITERATIONS
        || !valid_locker_base64(&input.salt, 24, 24)
        || !input.salt.ends_with("==")
        || !valid_locker_base64(&input.key_check_iv, 16, 16)
        || input.key_check_iv.contains('=')
        || !valid_locker_base64(&input.key_check_ciphertext, 24, 512)
    {
        return Err(fail(StatusCode::BAD_REQUEST, "Save Locker encryption parameters are invalid"));
    }

    let inserted = sqlx::query(
        "INSERT INTO save_locker_vaults(user_id,salt,kdf_iterations,key_check_iv,key_check_ciphertext)
         VALUES($1,$2,$3,$4,$5) ON CONFLICT(user_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(&input.salt)
    .bind(input.kdf_iterations)
    .bind(&input.key_check_iv)
    .bind(&input.key_check_ciphertext)
    .execute(&state.db)
    .await
    .map_err(|error| internal(error, "Could not create Save Locker"))?;

    if inserted.rows_affected() == 0 {
        return Err(fail(StatusCode::CONFLICT, "A Save Locker already exists for this account"));
    }
    save_locker_vault_get(State(state), AuthUser(user_id)).await
}

async fn save_locker_vault_delete(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM save_locker_vaults WHERE user_id=$1")
        .bind(user_id)
        .execute(&state.db)
        .await
        .map_err(|error| internal(error, "Could not delete Save Locker"))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, FromRow)]
struct SaveLockerItemRow {
    id: Uuid,
    iv: String,
    ciphertext: String,
    size_bytes: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

async fn save_locker_items_list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> ApiResult<Json<Vec<SaveLockerItemRow>>> {
    let rows = sqlx::query_as::<_, SaveLockerItemRow>(
        "SELECT id,iv,ciphertext,size_bytes,created_at,updated_at
           FROM save_locker_items WHERE user_id=$1 ORDER BY updated_at DESC LIMIT 100",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load encrypted Save Locker items"))?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
struct SaveLockerItemInput {
    iv: String,
    ciphertext: String,
}

#[derive(Debug, Deserialize)]
struct SaveLockerItemCreateInput {
    id: Uuid,
    iv: String,
    ciphertext: String,
}

async fn save_locker_item_create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(input): Json<SaveLockerItemCreateInput>,
) -> ApiResult<Json<SaveLockerItemRow>> {
    let size_bytes = validate_locker_item(&input.iv, &input.ciphertext)?;
    let mut tx = state.db.begin().await
        .map_err(|error| internal(error, "Could not begin Save Locker item creation"))?;

    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(user_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(|error| internal(error, "Could not lock Save Locker quota"))?;

    let vault_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM save_locker_vaults WHERE user_id=$1)",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| internal(error, "Could not verify Save Locker"))?;
    if !vault_exists {
        return Err(fail(StatusCode::CONFLICT, "Create your Save Locker before saving an item"));
    }

    let (item_count, used_bytes): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*)::bigint,COALESCE(SUM(size_bytes),0)::bigint
           FROM save_locker_items WHERE user_id=$1",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| internal(error, "Could not check Save Locker quota"))?;
    if item_count >= SAVE_LOCKER_MAX_ITEMS || used_bytes.saturating_add(size_bytes) > SAVE_LOCKER_MAX_TOTAL_BYTES {
        return Err(fail(StatusCode::PAYLOAD_TOO_LARGE, "Save Locker is full. Remove an item before saving more."));
    }

    let item_id = input.id;
    let row = sqlx::query_as::<_, SaveLockerItemRow>(
        "INSERT INTO save_locker_items(id,user_id,iv,ciphertext,size_bytes)
         VALUES($1,$2,$3,$4,$5)
         RETURNING id,iv,ciphertext,size_bytes,created_at,updated_at",
    )
    .bind(item_id)
    .bind(user_id)
    .bind(&input.iv)
    .bind(&input.ciphertext)
    .bind(size_bytes)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| internal(error, "Could not save encrypted Save Locker item"))?;

    tx.commit().await
        .map_err(|error| internal(error, "Could not commit Save Locker item"))?;
    Ok(Json(row))
}

async fn save_locker_item_update(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(item_id): Path<Uuid>,
    Json(input): Json<SaveLockerItemInput>,
) -> ApiResult<Json<SaveLockerItemRow>> {
    let size_bytes = validate_locker_item(&input.iv, &input.ciphertext)?;
    let mut tx = state.db.begin().await
        .map_err(|error| internal(error, "Could not begin Save Locker item update"))?;

    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(user_id.to_string())
        .execute(&mut *tx)
        .await
        .map_err(|error| internal(error, "Could not lock Save Locker quota"))?;

    let current: Option<i64> = sqlx::query_scalar(
        "SELECT size_bytes FROM save_locker_items WHERE id=$1 AND user_id=$2 FOR UPDATE",
    )
    .bind(item_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| internal(error, "Could not find Save Locker item"))?;
    if current.is_none() {
        return Err(fail(StatusCode::NOT_FOUND, "Save Locker item not found"));
    }

    let used_bytes: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(size_bytes),0)::bigint FROM save_locker_items
          WHERE user_id=$1 AND id<>$2",
    )
    .bind(user_id)
    .bind(item_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| internal(error, "Could not check Save Locker quota"))?;
    if used_bytes.saturating_add(size_bytes) > SAVE_LOCKER_MAX_TOTAL_BYTES {
        return Err(fail(StatusCode::PAYLOAD_TOO_LARGE, "Save Locker is full. Remove an item before updating this one."));
    }

    let row = sqlx::query_as::<_, SaveLockerItemRow>(
        "UPDATE save_locker_items SET iv=$3,ciphertext=$4,size_bytes=$5,updated_at=NOW()
          WHERE id=$1 AND user_id=$2
          RETURNING id,iv,ciphertext,size_bytes,created_at,updated_at",
    )
    .bind(item_id)
    .bind(user_id)
    .bind(&input.iv)
    .bind(&input.ciphertext)
    .bind(size_bytes)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| internal(error, "Could not update encrypted Save Locker item"))?;

    tx.commit().await
        .map_err(|error| internal(error, "Could not commit Save Locker item update"))?;
    Ok(Json(row))
}

async fn save_locker_item_delete(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(item_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let result = sqlx::query("DELETE FROM save_locker_items WHERE id=$1 AND user_id=$2")
        .bind(item_id)
        .bind(user_id)
        .execute(&state.db)
        .await
        .map_err(|error| internal(error, "Could not delete Save Locker item"))?;
    if result.rows_affected() == 0 {
        return Err(fail(StatusCode::NOT_FOUND, "Save Locker item not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/presence",get(workspace_presence))
        .merge(Router::new()
            .route("/locker/vault",get(save_locker_vault_get).post(save_locker_vault_create).delete(save_locker_vault_delete))
            .route("/locker/items",get(save_locker_items_list).post(save_locker_item_create))
            .route("/locker/items/{item_id}",put(save_locker_item_update).delete(save_locker_item_delete))
            .layer(DefaultBodyLimit::max(10 * 1024 * 1024)))
        .route("/tasks/search",get(search_tasks))
        .route("/analytics/history",get(analytics_history))
        .route("/analytics/export.csv",get(analytics_export))
        .route("/calendar/events/{event_id}/attendees",put(calendar_attendees_update))
        .route("/calendar/events/{event_id}/respond",post(calendar_attendee_response))
        .route("/files/usage",get(file_usage))
        .route("/files/quota",put(file_quota_update))
        .route("/files/audit",get(file_audit_list))
        .route("/files/{file_id}/preview",get(file_preview))
        .route("/chat/direct",get(direct_conversations).post(direct_conversation_create))
        .route("/chat/direct/{conversation_id}/messages",get(direct_messages_list).post(direct_message_send))
        .route("/chat/messages/{message_id}",patch(chat_message_update).delete(chat_message_delete))
        .route("/chat/messages/{message_id}/pin",patch(chat_message_pin))
        .route("/chat/pinned",get(pinned_messages))
        .route("/meeting-notes/{note_id}/action-items",get(action_items_list).post(action_item_create))
        .route("/meeting-action-items/{item_id}",patch(action_item_update).delete(action_item_delete))
}
