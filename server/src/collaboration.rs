use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    routing::{delete, get, patch, post},
    Json, Router,
};
use chrono::{Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use std::path::PathBuf;
use uuid::Uuid;

use crate::{auth::AuthUser, state::AppState};

const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

type ApiResult<T> = Result<T, (StatusCode, Json<ErrorBody>)>;

fn fail(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<ErrorBody>) {
    (status, Json(ErrorBody { error: message.into() }))
}

fn internal<E: std::fmt::Display>(error: E, context: &'static str) -> (StatusCode, Json<ErrorBody>) {
    tracing::error!(%error, "{context}");
    fail(StatusCode::INTERNAL_SERVER_ERROR, "The request could not be completed")
}

async fn workspace_for_user(state: &AppState, user_id: Uuid) -> ApiResult<Uuid> {
    sqlx::query_scalar(
        "SELECT workspace_id FROM workspace_members WHERE user_id = $1 ORDER BY joined_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| internal(error, "Could not resolve user's workspace"))?
    .ok_or_else(|| fail(StatusCode::NOT_FOUND, "Workspace not found"))
}

async fn board_in_workspace(state: &AppState, board_id: Uuid, workspace_id: Uuid) -> ApiResult<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM boards WHERE id = $1 AND workspace_id = $2)",
    )
    .bind(board_id)
    .bind(workspace_id)
    .fetch_one(&state.db)
    .await
    .map_err(|error| internal(error, "Could not verify project access"))?;
    if exists { Ok(()) } else { Err(fail(StatusCode::NOT_FOUND, "Project not found in this workspace")) }
}

async fn task_in_workspace(state: &AppState, task_id: Uuid, workspace_id: Uuid) -> ApiResult<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM tasks t
            JOIN lists l ON l.id = t.list_id
            JOIN boards b ON b.id = l.board_id
            WHERE t.id = $1 AND b.workspace_id = $2
        )",
    )
    .bind(task_id)
    .bind(workspace_id)
    .fetch_one(&state.db)
    .await
    .map_err(|error| internal(error, "Could not verify task access"))?;
    if exists { Ok(()) } else { Err(fail(StatusCode::NOT_FOUND, "Task not found in this workspace")) }
}

#[derive(Debug, Serialize, FromRow)]
struct CalendarEvent {
    id: Uuid,
    workspace_id: Uuid,
    board_id: Option<Uuid>,
    created_by: Uuid,
    title: String,
    description: Option<String>,
    event_type: String,
    start_at: chrono::DateTime<Utc>,
    end_at: chrono::DateTime<Utc>,
    time_zone: String,
    all_day: bool,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct CalendarTask {
    id: Uuid,
    board_id: Uuid,
    title: String,
    due_at: chrono::DateTime<Utc>,
    priority: String,
    list_name: String,
}

#[derive(Debug, Deserialize)]
struct CalendarRange {
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
}

async fn calendar_list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Query(range): Query<CalendarRange>,
) -> ApiResult<Json<Value>> {
    let workspace_id = workspace_for_user(&state, user_id).await?;
    let today = Utc::now().date_naive();
    let from = range.from.unwrap_or(today - Duration::days(30));
    let to = range.to.unwrap_or(today + Duration::days(60));
    if to < from || (to - from).num_days() > 400 {
        return Err(fail(StatusCode::BAD_REQUEST, "Calendar range must be within 400 days"));
    }
    let start = from.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let end = (to + Duration::days(1)).and_hms_opt(0, 0, 0).unwrap().and_utc();

    let events = sqlx::query_as::<_, CalendarEvent>(
        "SELECT id, workspace_id, board_id, created_by, title, description, event_type,
                start_at, end_at, time_zone, all_day, created_at
         FROM calendar_events
         WHERE workspace_id = $1 AND start_at < $3 AND end_at >= $2
         ORDER BY start_at",
    )
    .bind(workspace_id)
    .bind(start)
    .bind(end)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load calendar events"))?;

    let tasks = sqlx::query_as::<_, CalendarTask>(
        "SELECT t.id, b.id AS board_id, t.title, t.due_at, t.priority, l.name AS list_name
         FROM tasks t
         JOIN lists l ON l.id = t.list_id
         JOIN boards b ON b.id = l.board_id
         WHERE b.workspace_id = $1 AND t.due_at >= $2 AND t.due_at < $3
         ORDER BY t.due_at",
    )
    .bind(workspace_id)
    .bind(start)
    .bind(end)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load task deadlines"))?;

    Ok(Json(json!({ "events": events, "tasks": tasks, "from": from, "to": to })))
}

#[derive(Debug, Deserialize)]
struct CalendarEventInput {
    board_id: Option<Uuid>,
    title: String,
    description: Option<String>,
    event_type: String,
    start_at: chrono::DateTime<Utc>,
    end_at: chrono::DateTime<Utc>,
    time_zone: Option<String>,
    all_day: Option<bool>,
}

fn validate_calendar_input(input: &CalendarEventInput) -> ApiResult<()> {
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > 180 {
        return Err(fail(StatusCode::BAD_REQUEST, "Event title must be between 1 and 180 characters"));
    }
    if input.description.as_ref().is_some_and(|value| value.chars().count() > 4000) {
        return Err(fail(StatusCode::BAD_REQUEST, "Event description is too long"));
    }
    if input.end_at <= input.start_at {
        return Err(fail(StatusCode::BAD_REQUEST, "Event end must be after its start"));
    }
    if !["meeting", "milestone", "exam", "submission", "focus_block"].contains(&input.event_type.as_str()) {
        return Err(fail(StatusCode::BAD_REQUEST, "Event type is invalid"));
    }
    if input.time_zone.as_ref().is_some_and(|zone| zone.trim().is_empty() || zone.len() > 80) {
        return Err(fail(StatusCode::BAD_REQUEST, "Time zone is invalid"));
    }
    Ok(())
}

async fn calendar_create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(input): Json<CalendarEventInput>,
) -> ApiResult<(StatusCode, Json<CalendarEvent>)> {
    validate_calendar_input(&input)?;
    let workspace_id = workspace_for_user(&state, user_id).await?;
    if let Some(board_id) = input.board_id {
        board_in_workspace(&state, board_id, workspace_id).await?;
    }
    let event = sqlx::query_as::<_, CalendarEvent>(
        "INSERT INTO calendar_events
            (workspace_id, board_id, created_by, title, description, event_type, start_at, end_at, time_zone, all_day)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
         RETURNING id, workspace_id, board_id, created_by, title, description, event_type,
                   start_at, end_at, time_zone, all_day, created_at",
    )
    .bind(workspace_id)
    .bind(input.board_id)
    .bind(user_id)
    .bind(input.title.trim())
    .bind(input.description.as_deref().map(str::trim).filter(|value| !value.is_empty()))
    .bind(&input.event_type)
    .bind(input.start_at)
    .bind(input.end_at)
    .bind(input.time_zone.as_deref().unwrap_or("UTC"))
    .bind(input.all_day.unwrap_or(false))
    .fetch_one(&state.db)
    .await
    .map_err(|error| internal(error, "Could not create calendar event"))?;

    state.publish_workspace_event(workspace_id, event.board_id, "calendar_event_created", format!("created calendar event {}", event.title));
    Ok((StatusCode::CREATED, Json(event)))
}

async fn calendar_update(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(event_id): Path<Uuid>,
    Json(input): Json<CalendarEventInput>,
) -> ApiResult<StatusCode> {
    validate_calendar_input(&input)?;
    let workspace_id = workspace_for_user(&state, user_id).await?;
    if let Some(board_id) = input.board_id {
        board_in_workspace(&state, board_id, workspace_id).await?;
    }
    let result = sqlx::query(
        "UPDATE calendar_events SET board_id=$1, title=$2, description=$3, event_type=$4,
            start_at=$5, end_at=$6, time_zone=$7, all_day=$8, updated_at=NOW()
         WHERE id=$9 AND workspace_id=$10",
    )
    .bind(input.board_id)
    .bind(input.title.trim())
    .bind(input.description.as_deref().map(str::trim).filter(|value| !value.is_empty()))
    .bind(input.event_type)
    .bind(input.start_at)
    .bind(input.end_at)
    .bind(input.time_zone.as_deref().unwrap_or("UTC"))
    .bind(input.all_day.unwrap_or(false))
    .bind(event_id)
    .bind(workspace_id)
    .execute(&state.db)
    .await
    .map_err(|error| internal(error, "Could not update calendar event"))?;
    if result.rows_affected() == 0 {
        return Err(fail(StatusCode::NOT_FOUND, "Calendar event not found"));
    }
    state.publish_workspace_event(workspace_id, input.board_id, "calendar_event_updated", "updated a calendar event");
    Ok(StatusCode::NO_CONTENT)
}

async fn calendar_delete(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(event_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let workspace_id = workspace_for_user(&state, user_id).await?;
    let result = sqlx::query("DELETE FROM calendar_events WHERE id=$1 AND workspace_id=$2")
        .bind(event_id)
        .bind(workspace_id)
        .execute(&state.db)
        .await
        .map_err(|error| internal(error, "Could not delete calendar event"))?;
    if result.rows_affected() == 0 {
        return Err(fail(StatusCode::NOT_FOUND, "Calendar event not found"));
    }
    state.publish_workspace_event(workspace_id, None, "calendar_event_deleted", "deleted a calendar event");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, FromRow)]
struct NotificationRow {
    id: Uuid,
    workspace_id: Uuid,
    actor_user_id: Option<Uuid>,
    notification_type: String,
    title: String,
    body: String,
    resource_type: Option<String>,
    resource_id: Option<Uuid>,
    read_at: Option<chrono::DateTime<Utc>>,
    created_at: chrono::DateTime<Utc>,
}

async fn notifications_list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> ApiResult<Json<Value>> {
    let rows = sqlx::query_as::<_, NotificationRow>(
        "SELECT id, workspace_id, actor_user_id, notification_type, title, body,
                resource_type, resource_id, read_at, created_at
         FROM notifications WHERE recipient_user_id=$1
         ORDER BY created_at DESC LIMIT 100",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load notifications"))?;
    let unread = rows.iter().filter(|row| row.read_at.is_none()).count();
    Ok(Json(json!({ "items": rows, "unread_count": unread })))
}

async fn notification_read(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(notification_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let result = sqlx::query("UPDATE notifications SET read_at=COALESCE(read_at,NOW()) WHERE id=$1 AND recipient_user_id=$2")
        .bind(notification_id)
        .bind(user_id)
        .execute(&state.db)
        .await
        .map_err(|error| internal(error, "Could not mark notification as read"))?;
    if result.rows_affected() == 0 {
        return Err(fail(StatusCode::NOT_FOUND, "Notification not found"));
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn notifications_read_all(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> ApiResult<StatusCode> {
    sqlx::query("UPDATE notifications SET read_at=NOW() WHERE recipient_user_id=$1 AND read_at IS NULL")
        .bind(user_id)
        .execute(&state.db)
        .await
        .map_err(|error| internal(error, "Could not mark notifications as read"))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, FromRow)]
struct NotificationPreference {
    notification_type: String,
    in_app_enabled: bool,
    email_enabled: bool,
}

async fn notification_preferences(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> ApiResult<Json<Value>> {
    let rows = sqlx::query_as::<_, NotificationPreference>(
        "SELECT notification_type, in_app_enabled, email_enabled
         FROM notification_preferences WHERE user_id=$1 ORDER BY notification_type",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load notification preferences"))?;
    Ok(Json(json!({
        "items": rows,
        "email_delivery_available": crate::notifications::email_delivery_available()
    })))
}

#[derive(Debug, Deserialize)]
struct PreferenceInput {
    notification_type: String,
    in_app_enabled: bool,
    email_enabled: Option<bool>,
}

async fn notification_preference_update(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(input): Json<PreferenceInput>,
) -> ApiResult<StatusCode> {
    let kind = input.notification_type.trim();
    if kind.is_empty() || kind.len() > 60 {
        return Err(fail(StatusCode::BAD_REQUEST, "Notification category is invalid"));
    }
    if input.email_enabled == Some(true) && !crate::notifications::email_delivery_available() {
        return Err(fail(StatusCode::SERVICE_UNAVAILABLE, "Email delivery is not configured on this server"));
    }
    sqlx::query(
        "INSERT INTO notification_preferences (user_id, notification_type, in_app_enabled, email_enabled)
         VALUES ($1,$2,$3,COALESCE($4,FALSE))
         ON CONFLICT (user_id, notification_type)
         DO UPDATE SET
            in_app_enabled=EXCLUDED.in_app_enabled,
            email_enabled=COALESCE($4,notification_preferences.email_enabled),
            updated_at=NOW()",
    )
    .bind(user_id)
    .bind(kind)
    .bind(input.in_app_enabled)
    .bind(input.email_enabled)
    .execute(&state.db)
    .await
    .map_err(|error| internal(error, "Could not update notification preference"))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, FromRow)]
struct WorkspaceFile {
    id: Uuid,
    workspace_id: Uuid,
    board_id: Option<Uuid>,
    task_id: Option<Uuid>,
    uploaded_by: Uuid,
    original_name: String,
    media_type: String,
    size_bytes: i64,
    checksum_sha256: String,
    created_at: chrono::DateTime<Utc>,
}

async fn files_list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> ApiResult<Json<Vec<WorkspaceFile>>> {
    let workspace_id = workspace_for_user(&state, user_id).await?;
    let rows = sqlx::query_as::<_, WorkspaceFile>(
        "SELECT id, workspace_id, board_id, task_id, uploaded_by, original_name, media_type,
                size_bytes, checksum_sha256, created_at
         FROM workspace_files WHERE workspace_id=$1 ORDER BY created_at DESC",
    )
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load workspace files"))?;
    Ok(Json(rows))
}

async fn files_upload(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    mut multipart: Multipart,
) -> ApiResult<(StatusCode, Json<WorkspaceFile>)> {
    let workspace_id = workspace_for_user(&state, user_id).await?;
    let mut board_id: Option<Uuid> = None;
    let mut task_id: Option<Uuid> = None;
    let mut original_name = String::new();
    let mut media_type = "application/octet-stream".to_string();
    let mut file_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await
        .map_err(|error| fail(StatusCode::BAD_REQUEST, format!("Invalid upload form: {error}")))? {
        let field_name = field.name().unwrap_or("").to_owned();
        match field_name.as_str() {
            "board_id" => {
                let value = field.text().await.map_err(|_| fail(StatusCode::BAD_REQUEST, "Invalid project reference"))?;
                board_id = Some(Uuid::parse_str(value.trim()).map_err(|_| fail(StatusCode::BAD_REQUEST, "Invalid project reference"))?);
            }
            "task_id" => {
                let value = field.text().await.map_err(|_| fail(StatusCode::BAD_REQUEST, "Invalid task reference"))?;
                task_id = Some(Uuid::parse_str(value.trim()).map_err(|_| fail(StatusCode::BAD_REQUEST, "Invalid task reference"))?);
            }
            "file" => {
                original_name = field.file_name().unwrap_or("upload.bin").to_owned();
                media_type = field.content_type().unwrap_or("application/octet-stream").to_owned();
                let bytes = field.bytes().await.map_err(|_| fail(StatusCode::BAD_REQUEST, "Could not read uploaded file"))?;
                if bytes.is_empty() || bytes.len() > MAX_UPLOAD_BYTES {
                    return Err(fail(StatusCode::BAD_REQUEST, "Files must be between 1 byte and 10 MB"));
                }
                file_bytes = Some(bytes.to_vec());
            }
            _ => {}
        }
    }

    if board_id.is_some() && task_id.is_some() {
        return Err(fail(StatusCode::BAD_REQUEST, "Attach a file to a project or a task, not both"));
    }
    if let Some(board_id) = board_id {
        board_in_workspace(&state, board_id, workspace_id).await?;
    }
    if let Some(task_id) = task_id {
        task_in_workspace(&state, task_id, workspace_id).await?;
    }
    let bytes = file_bytes.ok_or_else(|| fail(StatusCode::BAD_REQUEST, "Choose a file to upload"))?;
    if original_name.trim().is_empty() || original_name.chars().count() > 240 || media_type.len() > 160 {
        return Err(fail(StatusCode::BAD_REQUEST, "File name or media type is invalid"));
    }

    let checksum = format!("{:x}", Sha256::digest(&bytes));
    let storage_key = Uuid::new_v4().to_string();
    state.storage.put(&storage_key, &bytes, &media_type)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Could not store workspace file");
            fail(StatusCode::INTERNAL_SERVER_ERROR, "Could not store uploaded file")
        })?;

    let file = sqlx::query_as::<_, WorkspaceFile>(
        "INSERT INTO workspace_files
            (workspace_id, board_id, task_id, uploaded_by, original_name, storage_key, media_type, size_bytes, checksum_sha256)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
         RETURNING id, workspace_id, board_id, task_id, uploaded_by, original_name, media_type,
                   size_bytes, checksum_sha256, created_at",
    )
    .bind(workspace_id)
    .bind(board_id)
    .bind(task_id)
    .bind(user_id)
    .bind(original_name.trim())
    .bind(&storage_key)
    .bind(&media_type)
    .bind(bytes.len() as i64)
    .bind(&checksum)
    .fetch_one(&state.db)
    .await
    .map_err(|error| {
        let storage = state.storage.clone();
        let key = storage_key.clone();
        tokio::spawn(async move {
            if let Err(cleanup_error) = storage.delete(&key).await {
                tracing::error!(%cleanup_error, "Could not clean up file after metadata insert failed");
            }
        });
        internal(error, "Could not save uploaded file metadata")
    })?;

    state.publish_workspace_event(workspace_id, board_id, "file_uploaded", format!("uploaded {}", file.original_name));
    Ok((StatusCode::CREATED, Json(file)))
}

async fn file_download(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(file_id): Path<Uuid>,
) -> ApiResult<Response<axum::body::Body>> {
    let workspace_id = workspace_for_user(&state, user_id).await?;
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT storage_key, original_name, media_type FROM workspace_files WHERE id=$1 AND workspace_id=$2",
    )
    .bind(file_id)
    .bind(workspace_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load file metadata"))?;
    let Some((storage_key, original_name, media_type)) = row else {
        return Err(fail(StatusCode::NOT_FOUND, "File not found"));
    };

    let bytes = state.storage.get(&storage_key).await.map_err(|error| {
        tracing::error!(%error, "Could not read workspace file");
        fail(StatusCode::NOT_FOUND, "Stored file is unavailable")
    })?;
    let safe_name: String = original_name.chars()
        .map(|character| if character.is_ascii_alphanumeric() || "._- ".contains(character) { character } else { '_' })
        .collect();
    let content_type = HeaderValue::from_str(&media_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{}\"", safe_name))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment; filename=\"download\""));

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_DISPOSITION, disposition)
        .header("x-content-type-options", "nosniff")
        .body(axum::body::Body::from(bytes))
        .map_err(|error| internal(error, "Could not prepare file download"))
}

async fn file_delete(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(file_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let workspace_id = workspace_for_user(&state, user_id).await?;
    let storage_key: Option<String> = sqlx::query_scalar(
        "SELECT storage_key FROM workspace_files WHERE id=$1 AND workspace_id=$2 AND uploaded_by=$3",
    )
    .bind(file_id)
    .bind(workspace_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load file metadata"))?;
    let Some(storage_key) = storage_key else {
        return Err(fail(StatusCode::NOT_FOUND, "File not found or you did not upload it"));
    };
    state.storage.delete(&storage_key).await.map_err(|error| {
        tracing::error!(%error, "Could not remove workspace file from storage");
        fail(StatusCode::INTERNAL_SERVER_ERROR, "Could not delete stored file")
    })?;
    let result = sqlx::query(
        "DELETE FROM workspace_files WHERE id=$1 AND workspace_id=$2 AND uploaded_by=$3",
    )
    .bind(file_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(&state.db)
    .await
    .map_err(|error| internal(error, "Could not delete file metadata"))?;
    if result.rows_affected() == 0 {
        return Err(fail(StatusCode::CONFLICT, "File metadata changed before deletion completed"));
    }
    state.publish_workspace_event(workspace_id, None, "file_deleted", "deleted a workspace file");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, FromRow)]
struct ChatMessage {
    id: Uuid,
    workspace_id: Uuid,
    board_id: Option<Uuid>,
    author_id: Uuid,
    author_name: String,
    body: String,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct ChatQuery {
    board_id: Option<Uuid>,
}

async fn chat_list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Query(query): Query<ChatQuery>,
) -> ApiResult<Json<Vec<ChatMessage>>> {
    let workspace_id = workspace_for_user(&state, user_id).await?;
    if let Some(board_id) = query.board_id {
        board_in_workspace(&state, board_id, workspace_id).await?;
    }
    let rows = sqlx::query_as::<_, ChatMessage>(
        "SELECT m.id, m.workspace_id, m.board_id, m.author_id, u.full_name AS author_name,
                m.body, m.created_at
         FROM chat_messages m JOIN users u ON u.id=m.author_id
         WHERE m.workspace_id=$1 AND (
             ($2::uuid IS NULL AND m.board_id IS NULL) OR m.board_id=$2
         )
         ORDER BY m.created_at DESC LIMIT 100",
    )
    .bind(workspace_id)
    .bind(query.board_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load team chat"))?;
    Ok(Json(rows.into_iter().rev().collect()))
}

#[derive(Debug, Deserialize)]
struct ChatInput {
    board_id: Option<Uuid>,
    body: String,
}

async fn chat_send(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(input): Json<ChatInput>,
) -> ApiResult<(StatusCode, Json<ChatMessage>)> {
    let body = input.body.trim();
    if body.is_empty() || body.chars().count() > 4000 {
        return Err(fail(StatusCode::BAD_REQUEST, "Message must be between 1 and 4000 characters"));
    }
    let workspace_id = workspace_for_user(&state, user_id).await?;
    if let Some(board_id) = input.board_id {
        board_in_workspace(&state, board_id, workspace_id).await?;
    }
    let message = sqlx::query_as::<_, ChatMessage>(
        "INSERT INTO chat_messages (workspace_id, board_id, author_id, body)
         VALUES ($1,$2,$3,$4)
         RETURNING id, workspace_id, board_id, author_id,
            (SELECT full_name FROM users WHERE id=$3) AS author_name, body, created_at",
    )
    .bind(workspace_id)
    .bind(input.board_id)
    .bind(user_id)
    .bind(body)
    .fetch_one(&state.db)
    .await
    .map_err(|error| internal(error, "Could not send team message"))?;
    state.publish_workspace_event(workspace_id, input.board_id, "chat_message_created", "sent a team message");
    Ok((StatusCode::CREATED, Json(message)))
}

#[derive(Debug, Serialize, FromRow)]
struct MeetingNote {
    id: Uuid,
    workspace_id: Uuid,
    board_id: Option<Uuid>,
    created_by: Uuid,
    title: String,
    meeting_at: Option<chrono::DateTime<Utc>>,
    attendees: String,
    agenda: String,
    notes: String,
    action_items: String,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct MeetingNoteInput {
    board_id: Option<Uuid>,
    title: String,
    meeting_at: Option<chrono::DateTime<Utc>>,
    attendees: Option<String>,
    agenda: Option<String>,
    notes: Option<String>,
    action_items: Option<String>,
}

fn validate_meeting_note(input: &MeetingNoteInput) -> ApiResult<()> {
    if input.title.trim().is_empty() || input.title.chars().count() > 180 {
        return Err(fail(StatusCode::BAD_REQUEST, "Meeting title must be between 1 and 180 characters"));
    }
    for value in [&input.attendees, &input.agenda, &input.notes, &input.action_items] {
        if value.as_ref().is_some_and(|text| text.chars().count() > 12000) {
            return Err(fail(StatusCode::BAD_REQUEST, "Meeting notes are too long"));
        }
    }
    Ok(())
}

async fn meeting_notes_list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> ApiResult<Json<Vec<MeetingNote>>> {
    let workspace_id = workspace_for_user(&state, user_id).await?;
    let rows = sqlx::query_as::<_, MeetingNote>(
        "SELECT id, workspace_id, board_id, created_by, title, meeting_at, attendees,
                agenda, notes, action_items, created_at, updated_at
         FROM meeting_notes WHERE workspace_id=$1 ORDER BY meeting_at DESC NULLS LAST, created_at DESC",
    )
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load meeting notes"))?;
    Ok(Json(rows))
}

async fn meeting_note_create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(input): Json<MeetingNoteInput>,
) -> ApiResult<(StatusCode, Json<MeetingNote>)> {
    validate_meeting_note(&input)?;
    let workspace_id = workspace_for_user(&state, user_id).await?;
    if let Some(board_id) = input.board_id {
        board_in_workspace(&state, board_id, workspace_id).await?;
    }
    let note = sqlx::query_as::<_, MeetingNote>(
        "INSERT INTO meeting_notes
            (workspace_id, board_id, created_by, title, meeting_at, attendees, agenda, notes, action_items)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
         RETURNING id, workspace_id, board_id, created_by, title, meeting_at, attendees,
                   agenda, notes, action_items, created_at, updated_at",
    )
    .bind(workspace_id)
    .bind(input.board_id)
    .bind(user_id)
    .bind(input.title.trim())
    .bind(input.meeting_at)
    .bind(input.attendees.as_deref().unwrap_or(""))
    .bind(input.agenda.as_deref().unwrap_or(""))
    .bind(input.notes.as_deref().unwrap_or(""))
    .bind(input.action_items.as_deref().unwrap_or(""))
    .fetch_one(&state.db)
    .await
    .map_err(|error| internal(error, "Could not create meeting notes"))?;
    state.publish_workspace_event(workspace_id, input.board_id, "meeting_notes_created", format!("created meeting notes for {}", note.title));
    Ok((StatusCode::CREATED, Json(note)))
}

async fn meeting_note_update(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(note_id): Path<Uuid>,
    Json(input): Json<MeetingNoteInput>,
) -> ApiResult<StatusCode> {
    validate_meeting_note(&input)?;
    let workspace_id = workspace_for_user(&state, user_id).await?;
    if let Some(board_id) = input.board_id {
        board_in_workspace(&state, board_id, workspace_id).await?;
    }
    let result = sqlx::query(
        "UPDATE meeting_notes SET board_id=$1, title=$2, meeting_at=$3, attendees=$4,
            agenda=$5, notes=$6, action_items=$7, updated_at=NOW()
         WHERE id=$8 AND workspace_id=$9",
    )
    .bind(input.board_id)
    .bind(input.title.trim())
    .bind(input.meeting_at)
    .bind(input.attendees.as_deref().unwrap_or(""))
    .bind(input.agenda.as_deref().unwrap_or(""))
    .bind(input.notes.as_deref().unwrap_or(""))
    .bind(input.action_items.as_deref().unwrap_or(""))
    .bind(note_id)
    .bind(workspace_id)
    .execute(&state.db)
    .await
    .map_err(|error| internal(error, "Could not update meeting notes"))?;
    if result.rows_affected() == 0 {
        return Err(fail(StatusCode::NOT_FOUND, "Meeting notes not found"));
    }
    state.publish_workspace_event(workspace_id, input.board_id, "meeting_notes_updated", "updated meeting notes");
    Ok(StatusCode::NO_CONTENT)
}

async fn meeting_note_delete(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(note_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let workspace_id = workspace_for_user(&state, user_id).await?;
    let result = sqlx::query("DELETE FROM meeting_notes WHERE id=$1 AND workspace_id=$2 AND created_by=$3")
        .bind(note_id)
        .bind(workspace_id)
        .bind(user_id)
        .execute(&state.db)
        .await
        .map_err(|error| internal(error, "Could not delete meeting notes"))?;
    if result.rows_affected() == 0 {
        return Err(fail(StatusCode::NOT_FOUND, "Meeting notes not found or you did not create them"));
    }
    state.publish_workspace_event(workspace_id, None, "meeting_notes_deleted", "deleted meeting notes");
    Ok(StatusCode::NO_CONTENT)
}

async fn analytics(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> ApiResult<Json<Value>> {
    let workspace_id = workspace_for_user(&state, user_id).await?;
    let (total, completed, overdue, unassigned): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT
           COUNT(*)::bigint,
           COUNT(*) FILTER (WHERE lower(l.name) IN ('done','complete','completed'))::bigint,
           COUNT(*) FILTER (WHERE t.due_at < NOW() AND lower(l.name) NOT IN ('done','complete','completed'))::bigint,
           COUNT(*) FILTER (WHERE NOT EXISTS(SELECT 1 FROM task_assignees ta WHERE ta.task_id=t.id))::bigint
         FROM tasks t
         JOIN lists l ON l.id=t.list_id
         JOIN boards b ON b.id=l.board_id
         WHERE b.workspace_id=$1",
    )
    .bind(workspace_id)
    .fetch_one(&state.db)
    .await
    .map_err(|error| internal(error, "Could not calculate workspace metrics"))?;

    let members = sqlx::query_as::<_, (Uuid, String, i64, i64)>(
        "SELECT u.id, u.full_name,
           COUNT(DISTINCT t.id) FILTER (WHERE lower(l.name) NOT IN ('done','complete','completed'))::bigint AS open_tasks,
           COUNT(DISTINCT t.id) FILTER (WHERE lower(l.name) IN ('done','complete','completed'))::bigint AS completed_tasks
         FROM workspace_members wm
         JOIN users u ON u.id=wm.user_id
         LEFT JOIN task_assignees ta ON ta.user_id=u.id
         LEFT JOIN tasks t ON t.id=ta.task_id
         LEFT JOIN lists l ON l.id=t.list_id
         LEFT JOIN boards b ON b.id=l.board_id AND b.workspace_id=wm.workspace_id
         WHERE wm.workspace_id=$1 AND (t.id IS NULL OR b.id IS NOT NULL)
         GROUP BY u.id, u.full_name ORDER BY u.full_name",
    )
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not calculate member workload"))?;

    let project_rows = sqlx::query_as::<_, (Uuid, String, i64, i64)>(
        "SELECT b.id, b.name, COUNT(t.id)::bigint,
           COUNT(t.id) FILTER (WHERE lower(l.name) IN ('done','complete','completed'))::bigint
         FROM boards b LEFT JOIN lists l ON l.board_id=b.id
         LEFT JOIN tasks t ON t.list_id=l.id
         WHERE b.workspace_id=$1 GROUP BY b.id,b.name ORDER BY b.name",
    )
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not calculate project progress"))?;

    Ok(Json(json!({
        "total_tasks": total,
        "completed_tasks": completed,
        "completion_percent": if total > 0 { completed * 100 / total } else { 0 },
        "overdue_tasks": overdue,
        "unassigned_tasks": unassigned,
        "members": members.into_iter().map(|(id, name, open, done)| json!({
            "user_id": id, "full_name": name, "open_tasks": open, "completed_tasks": done
        })).collect::<Vec<_>>(),
        "projects": project_rows.into_iter().map(|(id, name, task_count, done_count)| json!({
            "board_id": id, "name": name, "task_count": task_count, "completed_count": done_count,
            "completion_percent": if task_count > 0 { done_count * 100 / task_count } else { 0 }
        })).collect::<Vec<_>>()
    })))
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/calendar", get(calendar_list).post(calendar_create))
        .route("/calendar/events/{event_id}", patch(calendar_update).delete(calendar_delete))
        .route("/notifications", get(notifications_list))
        .route("/notifications/read-all", post(notifications_read_all))
        .route("/notifications/{notification_id}/read", patch(notification_read))
        .route("/notification-preferences", get(notification_preferences).put(notification_preference_update))
        .route("/files", get(files_list).post(files_upload).layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES + 1024 * 1024)))
        .route("/files/{file_id}/download", get(file_download))
        .route("/files/{file_id}", delete(file_delete))
        .route("/chat/messages", get(chat_list).post(chat_send))
        .route("/meeting-notes", get(meeting_notes_list).post(meeting_note_create))
        .route("/meeting-notes/{note_id}", patch(meeting_note_update).delete(meeting_note_delete))
        .route("/analytics", get(analytics))
}
