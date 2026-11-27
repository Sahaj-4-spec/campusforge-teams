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
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};
use tokio::time::{interval, Duration as TokioDuration};
use uuid::Uuid;

use crate::{auth::{AuthUser, WorkspaceUser}, roadmap, state::AppState};

const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024;

struct TemporaryUpload {
    path: PathBuf,
}

#[cfg(unix)]
fn create_private_upload_file(path: &PathBuf) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path)
}

#[cfg(not(unix))]
fn create_private_upload_file(path: &PathBuf) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new().write(true).create_new(true).open(path)
}

async fn scan_upload(path: &PathBuf) -> Result<&'static str, String> {
    let scanner = std::env::var("CAMPUSFORGE_CLAMAV_ADDR")
        .ok().filter(|value| !value.trim().is_empty());
    let require_scan = std::env::var("CAMPUSFORGE_REQUIRE_MALWARE_SCAN")
        .ok().is_some_and(|value| value.eq_ignore_ascii_case("true"));
    let Some(address) = scanner else {
        return if require_scan {
            Err("Malware scanning is required but CAMPUSFORGE_CLAMAV_ADDR is not configured".to_string())
        } else {
            Ok("unscanned")
        };
    };

    let mut stream = tokio::time::timeout(std::time::Duration::from_secs(5), TcpStream::connect(&address))
        .await.map_err(|_| "Malware scanner connection timed out".to_string())?
        .map_err(|error| format!("Could not connect to malware scanner: {error}"))?;
    stream.write_all(b"zINSTREAM\0").await.map_err(|error| error.to_string())?;
    let mut file = tokio::fs::File::open(path).await.map_err(|error| error.to_string())?;
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).await.map_err(|error| error.to_string())?;
        if read == 0 { break; }
        let length = (read as u32).to_be_bytes();
        stream.write_all(&length).await.map_err(|error| error.to_string())?;
        stream.write_all(&buffer[..read]).await.map_err(|error| error.to_string())?;
    }
    stream.write_all(&0_u32.to_be_bytes()).await.map_err(|error| error.to_string())?;
    let mut response = [0_u8; 1024];
    let read = tokio::time::timeout(std::time::Duration::from_secs(20), stream.read(&mut response))
        .await.map_err(|_| "Malware scanner response timed out".to_string())?
        .map_err(|error| format!("Could not read malware scanner response: {error}"))?;
    let response = String::from_utf8_lossy(&response[..read]);
    if response.contains(" FOUND") {
        return Err("The uploaded file was flagged as malware".to_string());
    }
    if response.contains(" OK") {
        return Ok("clean");
    }
    Err("Malware scanner returned an unexpected response".to_string())
}

async fn validate_upload_content(path: &PathBuf, extension: &str) -> Result<(), String> {
    let bytes = tokio::fs::read(path).await.map_err(|error| error.to_string())?;
    let matches = match extension {
        "pdf" => bytes.starts_with(b"%PDF-"),
        "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpg" | "jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        "zip" | "docx" | "xlsx" | "pptx" => bytes.starts_with(b"PK\x03\x04"),
        "doc" | "xls" | "ppt" => bytes.starts_with(&[0xd0,0xcf,0x11,0xe0,0xa1,0xb1,0x1a,0xe1]),
        _ => std::str::from_utf8(&bytes).is_ok(),
    };
    if matches { Ok(()) } else { Err("The file content does not match its extension".to_string()) }
}

async fn reserve_workspace_storage(
    state: &AppState,
    workspace_id: Uuid,
    size_bytes: i64,
) -> ApiResult<Uuid> {
    let mut tx = state.db.begin().await.map_err(|error| internal(error, "Could not begin storage reservation"))?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
        .bind(workspace_id.to_string()).execute(&mut *tx).await
        .map_err(|error| internal(error, "Could not lock workspace storage quota"))?;
    sqlx::query("INSERT INTO workspace_storage_quotas(workspace_id,quota_bytes) VALUES($1,104857600) ON CONFLICT DO NOTHING")
        .bind(workspace_id).execute(&mut *tx).await
        .map_err(|error| internal(error, "Could not initialize workspace storage quota"))?;
    let (used,reserved,quota):(i64,i64,i64)=sqlx::query_as(
        "SELECT
            COALESCE((SELECT SUM(size_bytes) FROM workspace_files WHERE workspace_id=$1 AND is_deleting=FALSE),0)::bigint,
            COALESCE((SELECT SUM(size_bytes) FROM workspace_storage_reservations WHERE workspace_id=$1),0)::bigint,
            q.quota_bytes
         FROM workspace_storage_quotas q WHERE q.workspace_id=$1",
    ).bind(workspace_id).fetch_one(&mut *tx).await
      .map_err(|error| internal(error, "Could not calculate workspace storage usage"))?;
    if used.saturating_add(reserved).saturating_add(size_bytes)>quota {
        return Err(fail(StatusCode::PAYLOAD_TOO_LARGE,"Workspace storage quota would be exceeded"));
    }
    let reservation_id:Uuid=sqlx::query_scalar(
        "INSERT INTO workspace_storage_reservations(workspace_id,size_bytes) VALUES($1,$2) RETURNING id",
    ).bind(workspace_id).bind(size_bytes).fetch_one(&mut *tx).await
      .map_err(|error| internal(error, "Could not reserve workspace storage"))?;
    tx.commit().await.map_err(|error| internal(error, "Could not commit storage reservation"))?;
    Ok(reservation_id)
}

impl Drop for TemporaryUpload {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

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

async fn board_access(
    state: &AppState, user_id: Uuid, board_id: Uuid, workspace_id: Uuid, write: bool,
) -> ApiResult<()> {
    let role = crate::auth::board_role(&state.db, user_id, workspace_id, board_id)
        .await.map_err(|error| internal(error, "Could not verify board permissions"))?;
    if crate::auth::board_role_allows(role.as_deref(), write) {
        Ok(())
    } else if role.as_deref() == Some("viewer") && write {
        Err(fail(StatusCode::FORBIDDEN, "This project is read-only for your account"))
    } else {
        Err(fail(StatusCode::NOT_FOUND, "Project not found in this workspace"))
    }
}
async fn board_in_workspace(state: &AppState, user_id: Uuid, board_id: Uuid, workspace_id: Uuid) -> ApiResult<()> {
    board_access(state, user_id, board_id, workspace_id, false).await
}
async fn board_write_access(state: &AppState, user_id: Uuid, board_id: Uuid, workspace_id: Uuid) -> ApiResult<()> {
    board_access(state, user_id, board_id, workspace_id, true).await
}
async fn task_in_workspace(
    state: &AppState, user_id: Uuid, task_id: Uuid, workspace_id: Uuid, write: bool,
) -> ApiResult<()> {
    let board_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT b.id FROM tasks t JOIN lists l ON l.id=t.list_id
         JOIN boards b ON b.id=l.board_id WHERE t.id=$1 AND b.workspace_id=$2",
    )
    .bind(task_id).bind(workspace_id).fetch_optional(&state.db).await
    .map_err(|error| internal(error, "Could not verify task access"))?;
    let Some(board_id) = board_id else {
        return Err(fail(StatusCode::NOT_FOUND, "Task not found in this workspace"));
    };
    board_access(state, user_id, board_id, workspace_id, write).await
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
    reminder_minutes: Option<i32>,
    all_day_start_date: Option<NaiveDate>,
    all_day_end_date: Option<NaiveDate>,
    attendee_ids: Vec<Uuid>,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct CalendarTask {
    id: Uuid,
    board_id: Uuid,
    title: String,
    due_date: String,
    priority: String,
    list_name: String,
}

#[derive(Debug, Deserialize)]
struct CalendarRange {
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    start_at: Option<chrono::DateTime<Utc>>,
    end_at: Option<chrono::DateTime<Utc>>,
    board_id: Option<Uuid>,
    event_type: Option<String>,
    attendee_id: Option<Uuid>,
}

async fn calendar_list(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Query(range): Query<CalendarRange>,
) -> ApiResult<Json<Value>> {
    if let Some(board_id)=range.board_id { board_in_workspace(&state,user_id,board_id,workspace_id).await?; }
    let today = Utc::now().date_naive();
    let from = range.from.unwrap_or(today - Duration::days(30));
    let to = range.to.unwrap_or(today + Duration::days(60));
    if to < from || (to - from).num_days() > 400 {
        return Err(fail(StatusCode::BAD_REQUEST, "Calendar range must be within 400 days"));
    }
    let next_day = to.succ_opt()
        .ok_or_else(|| fail(StatusCode::BAD_REQUEST, "Calendar end date is out of range"))?;
    let task_start = from.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let task_end = next_day.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let start = range.start_at.unwrap_or(task_start);
    let end = range.end_at.unwrap_or(task_end);
    if end <= start || end - start > Duration::days(401) {
        return Err(fail(StatusCode::BAD_REQUEST, "Calendar instant range is invalid or exceeds 401 days"));
    }

    let events = sqlx::query_as::<_, CalendarEvent>(
        "SELECT id, workspace_id, board_id, created_by, title, description, event_type,
                start_at, end_at, time_zone, all_day, reminder_minutes,
                all_day_start_date, all_day_end_date,
                COALESCE((SELECT array_agg(a.user_id ORDER BY a.user_id) FROM calendar_event_attendees a WHERE a.event_id=calendar_events.id), ARRAY[]::uuid[]) AS attendee_ids,
                created_at
         FROM calendar_events
         WHERE workspace_id = $1
           AND (
                (all_day=TRUE AND all_day_start_date <= $3::date AND all_day_end_date >= $2::date)
                OR (all_day=FALSE AND start_at < $5 AND end_at > $4)
           )
           AND ($6::uuid IS NULL OR board_id=$6)
           AND (board_id IS NULL OR EXISTS(SELECT 1 FROM board_members bm WHERE bm.board_id=calendar_events.board_id AND bm.user_id=$9)
                OR EXISTS(SELECT 1 FROM workspace_members wm WHERE wm.workspace_id=calendar_events.workspace_id AND wm.user_id=$9 AND wm.role IN ('owner','admin')))
           AND ($7::text IS NULL OR event_type=$7)
           AND ($8::uuid IS NULL OR EXISTS(
                SELECT 1 FROM calendar_event_attendees a WHERE a.event_id=calendar_events.id AND a.user_id=$8
           ))
         ORDER BY CASE WHEN all_day THEN all_day_start_date::timestamp AT TIME ZONE time_zone ELSE start_at END",
    )
    .bind(workspace_id)
    .bind(from)
    .bind(to)
    .bind(start)
    .bind(end)
    .bind(range.board_id)
    .bind(range.event_type.as_deref())
    .bind(range.attendee_id)
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load calendar events"))?;

    let tasks = sqlx::query_as::<_, CalendarTask>(
        "SELECT t.id, b.id AS board_id, t.title, to_char(t.due_at AT TIME ZONE 'UTC', 'YYYY-MM-DD') AS due_date, t.priority, l.name AS list_name
         FROM tasks t
         JOIN lists l ON l.id = t.list_id
         JOIN boards b ON b.id = l.board_id
         WHERE b.workspace_id = $1 AND t.due_at >= $2 AND t.due_at < $3
           AND ($4::uuid IS NULL OR b.id=$4)
           AND (EXISTS(SELECT 1 FROM board_members bm WHERE bm.board_id=b.id AND bm.user_id=$5)
                OR EXISTS(SELECT 1 FROM workspace_members wm WHERE wm.workspace_id=b.workspace_id AND wm.user_id=$5 AND wm.role IN ('owner','admin')))
         ORDER BY t.due_at",
    )
    .bind(workspace_id)
    .bind(task_start)
    .bind(task_end)
    .bind(range.board_id)
    .bind(user_id)
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
    all_day_start_date: Option<NaiveDate>,
    all_day_end_date: Option<NaiveDate>,
    reminder_minutes: Option<i32>,
    attendee_ids: Option<Vec<Uuid>>,
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
    if input.time_zone.as_ref().is_some_and(|zone| zone.trim().is_empty() || zone.len() > 80 || zone.chars().any(char::is_control)) {
        return Err(fail(StatusCode::BAD_REQUEST, "Time zone is invalid"));
    }
    if input.reminder_minutes.is_some_and(|minutes| ![0,5,10,15,30,60,1440].contains(&minutes)) {
        return Err(fail(StatusCode::BAD_REQUEST, "Reminder must be 0, 5, 10, 15, 30, 60, or 1440 minutes"));
    }
    if input.all_day.unwrap_or(false) {
        let (Some(start),Some(end))=(input.all_day_start_date,input.all_day_end_date) else {
            return Err(fail(StatusCode::BAD_REQUEST, "All-day events require start and end dates"));
        };
        if end < start { return Err(fail(StatusCode::BAD_REQUEST, "All-day event end date cannot be before its start date")); }
    }
    if input.attendee_ids.as_ref().is_some_and(|ids| ids.len()>100 || ids.iter().copied().collect::<std::collections::HashSet<_>>().len()!=ids.len()) {
        return Err(fail(StatusCode::BAD_REQUEST, "Attendee list is invalid"));
    }
    Ok(())
}

async fn validate_calendar_timezone(state: &AppState, time_zone: Option<&str>) -> ApiResult<()> {
    let Some(time_zone) = time_zone else { return Ok(()); };
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_timezone_names WHERE name=$1)",
    )
    .bind(time_zone)
    .fetch_one(&state.db)
    .await
    .map_err(|error| internal(error, "Could not validate event time zone"))?;
    if !exists {
        return Err(fail(StatusCode::BAD_REQUEST, "Choose a valid IANA time zone"));
    }
    Ok(())
}

async fn validate_calendar_attendees(state: &AppState, workspace_id: Uuid, attendee_ids: &[Uuid]) -> ApiResult<()> {
    for attendee_id in attendee_ids {
        let is_member: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM workspace_members WHERE workspace_id=$1 AND user_id=$2)",
        ).bind(workspace_id).bind(attendee_id).fetch_one(&state.db).await
         .map_err(|error| internal(error, "Could not validate calendar attendee"))?;
        if !is_member { return Err(fail(StatusCode::BAD_REQUEST, "Every event attendee must be a workspace member")); }
    }
    Ok(())
}

async fn replace_calendar_attendees(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    workspace_id: Uuid,
    event_id: Uuid,
    actor_id: Uuid,
    attendee_ids: &[Uuid],
) -> Result<(), sqlx::Error> {
    let mut unique=HashSet::new();
    unique.insert(actor_id);
    unique.extend(attendee_ids.iter().copied());
    let mut attendees:Vec<Uuid>=unique.into_iter().collect();
    attendees.sort_unstable();

    // Keep accepted/declined responses and reminder state for existing
    // attendees. Only genuinely removed attendees are deleted; a later
    // re-invitation then starts with a fresh response and reminder.
    sqlx::query(
        "DELETE FROM calendar_event_attendees
          WHERE event_id=$1 AND NOT (user_id=ANY($2))",
    )
    .bind(event_id)
    .bind(&attendees)
    .execute(&mut **tx)
    .await?;

    for attendee_id in attendees {
        let inserted=sqlx::query(
            "INSERT INTO calendar_event_attendees(event_id,user_id)
             VALUES($1,$2) ON CONFLICT(event_id,user_id) DO NOTHING",
        )
        .bind(event_id)
        .bind(attendee_id)
        .execute(&mut **tx)
        .await?;
        if inserted.rows_affected()>0 && attendee_id != actor_id {
            roadmap::create_notification_in_transaction(
                tx,workspace_id,attendee_id,Some(actor_id),"calendar_invitation",
                "Calendar invitation","You were invited to a calendar event.",
                Some("calendar_event"),Some(event_id),
                Some(&format!("calendar-invite:{event_id}:{attendee_id}")),
            ).await?;
        }
    }
    Ok(())
}

async fn calendar_create(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Json(input): Json<CalendarEventInput>,
) -> ApiResult<(StatusCode, Json<CalendarEvent>)> {
    validate_calendar_input(&input)?;
    validate_calendar_timezone(&state, input.time_zone.as_deref()).await?;
    if let Some(board_id) = input.board_id {
        board_write_access(&state,user_id,board_id,workspace_id).await?;
    }
    if let Some(attendees)=input.attendee_ids.as_deref() {
        validate_calendar_attendees(&state,workspace_id,attendees).await?;
    }
    let mut tx=state.db.begin().await.map_err(|error|internal(error,"Could not begin calendar event"))?;
    let event = sqlx::query_as::<_, CalendarEvent>(
        "INSERT INTO calendar_events
            (workspace_id, board_id, created_by, title, description, event_type, start_at, end_at,
             time_zone, all_day, reminder_minutes, all_day_start_date, all_day_end_date)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
         RETURNING id, workspace_id, board_id, created_by, title, description, event_type,
                   start_at, end_at, time_zone, all_day, reminder_minutes,
                   all_day_start_date, all_day_end_date,
                   COALESCE((SELECT array_agg(a.user_id ORDER BY a.user_id) FROM calendar_event_attendees a WHERE a.event_id=calendar_events.id), ARRAY[]::uuid[]) AS attendee_ids,
                   created_at",
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
    .bind(input.reminder_minutes)
    .bind(input.all_day_start_date)
    .bind(input.all_day_end_date)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| internal(error, "Could not create calendar event"))?;
    if let Some(attendees)=input.attendee_ids.as_deref() {
        replace_calendar_attendees(&mut tx,workspace_id,event.id,user_id,attendees)
            .await.map_err(|error|internal(error,"Could not save calendar attendees"))?;
    }
    tx.commit().await.map_err(|error|internal(error,"Could not save calendar event"))?;
    state.publish_workspace_event(workspace_id, event.board_id, "calendar_event_created", format!("created calendar event {}", event.title));
    Ok((StatusCode::CREATED, Json(event)))
}

async fn calendar_update(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(event_id): Path<Uuid>,
    Json(input): Json<CalendarEventInput>,
) -> ApiResult<StatusCode> {
    validate_calendar_input(&input)?;
    if let Some(board_id) = input.board_id {
        board_write_access(&state,user_id,board_id,workspace_id).await?;
    }
    if let Some(attendees)=input.attendee_ids.as_deref() {
        validate_calendar_attendees(&state,workspace_id,attendees).await?;
    }
    let mut tx=state.db.begin().await.map_err(|error|internal(error,"Could not begin calendar event update"))?;
    let event: Option<(Uuid,Option<Uuid>)>=sqlx::query_as(
        "SELECT created_by,board_id FROM calendar_events WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
    ).bind(event_id).bind(workspace_id).fetch_optional(&mut *tx).await
      .map_err(|error|internal(error,"Could not verify calendar event"))?;
    let Some((creator,existing_board_id))=event else { return Err(fail(StatusCode::NOT_FOUND,"Calendar event not found")); };
    if let Some(board_id)=existing_board_id { board_write_access(&state,user_id,board_id,workspace_id).await?; }
    if creator!=user_id && !crate::roadmap::workspace_admin(&state,user_id,workspace_id).await
        .map_err(|error|internal(error,"Could not verify calendar permissions"))? {
        return Err(fail(StatusCode::FORBIDDEN,"Only the event creator or a workspace admin can edit this event"));
    }
    let result = sqlx::query(
        "UPDATE calendar_events SET board_id=$1, title=$2, description=$3, event_type=$4,
            start_at=$5, end_at=$6, time_zone=$7, all_day=$8, reminder_minutes=$9,
            all_day_start_date=$10, all_day_end_date=$11, updated_at=NOW()
         WHERE id=$12 AND workspace_id=$13",
    )
    .bind(input.board_id)
    .bind(input.title.trim())
    .bind(input.description.as_deref().map(str::trim).filter(|value| !value.is_empty()))
    .bind(&input.event_type)
    .bind(input.start_at)
    .bind(input.end_at)
    .bind(input.time_zone.as_deref().unwrap_or("UTC"))
    .bind(input.all_day.unwrap_or(false))
    .bind(input.reminder_minutes)
    .bind(input.all_day_start_date)
    .bind(input.all_day_end_date)
    .bind(event_id)
    .bind(workspace_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| internal(error, "Could not update calendar event"))?;
    if result.rows_affected() == 0 {
        return Err(fail(StatusCode::NOT_FOUND, "Calendar event not found"));
    }
    if let Some(attendees)=input.attendee_ids.as_deref() {
        replace_calendar_attendees(&mut tx,workspace_id,event_id,user_id,attendees)
            .await.map_err(|error|internal(error,"Could not update calendar attendees"))?;
    }
    tx.commit().await.map_err(|error|internal(error,"Could not commit calendar event update"))?;
    state.publish_workspace_event(workspace_id, input.board_id, "calendar_event_updated", "updated a calendar event");
    Ok(StatusCode::NO_CONTENT)
}

async fn calendar_delete(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(event_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let event:Option<(Uuid,Option<Uuid>)>=sqlx::query_as(
        "SELECT created_by,board_id FROM calendar_events WHERE id=$1 AND workspace_id=$2",
    ).bind(event_id).bind(workspace_id).fetch_optional(&state.db).await
      .map_err(|error|internal(error,"Could not verify calendar delete permission"))?;
    let Some((creator,board_id))=event else { return Err(fail(StatusCode::NOT_FOUND,"Calendar event not found")); };
    if let Some(board_id)=board_id { board_write_access(&state,user_id,board_id,workspace_id).await?; }
    if creator!=user_id && !crate::roadmap::workspace_admin(&state,user_id,workspace_id).await
        .map_err(|error|internal(error,"Could not verify calendar delete permission"))? {
        return Err(fail(StatusCode::FORBIDDEN,"Only the event creator or a workspace admin can delete this event"));
    }
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

#[derive(Debug, Deserialize)]
struct NotificationQuery {
    before: Option<chrono::DateTime<Utc>>,
    limit: Option<i64>,
}

async fn notifications_list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Query(query): Query<NotificationQuery>,
) -> ApiResult<Json<Value>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let mut transaction = state.db.begin().await
        .map_err(|error| internal(error, "Could not begin notification read"))?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *transaction)
        .await
        .map_err(|error| internal(error, "Could not set notification snapshot"))?;
    let rows = sqlx::query_as::<_, NotificationRow>(
        "SELECT n.id, n.workspace_id, n.actor_user_id, n.notification_type, n.title, n.body,
                n.resource_type, n.resource_id, n.read_at, n.created_at
         FROM notifications n
         LEFT JOIN workspace_members wm
           ON wm.workspace_id=n.workspace_id AND wm.user_id=n.recipient_user_id
         WHERE n.recipient_user_id=$1
           AND (wm.user_id IS NOT NULL OR (
                n.notification_type='workspace_invitation' AND EXISTS(
                    SELECT 1 FROM invitations i JOIN users u ON lower(u.email)=lower(i.email)
                     WHERE i.id=n.resource_id AND i.workspace_id=n.workspace_id
                       AND u.id=$1 AND i.revoked_at IS NULL AND i.expires_at>NOW()
                )
           ))
           AND ($2::timestamptz IS NULL OR n.created_at<$2)
           AND COALESCE((
                SELECT np.in_app_enabled
                  FROM notification_preferences np
                 WHERE np.user_id=n.recipient_user_id
                   AND np.notification_type=n.notification_type
           ), TRUE)
         ORDER BY n.created_at DESC LIMIT $3",
    )
    .bind(user_id)
    .bind(query.before)
    .bind(limit)
    .fetch_all(&mut *transaction)
    .await
    .map_err(|error| internal(error, "Could not load notifications"))?;
    let unread: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM notifications n
         WHERE n.recipient_user_id=$1 AND n.read_at IS NULL
           AND (
                EXISTS(SELECT 1 FROM workspace_members wm
                       WHERE wm.workspace_id=n.workspace_id AND wm.user_id=n.recipient_user_id)
                OR (n.notification_type='workspace_invitation' AND EXISTS(
                    SELECT 1 FROM invitations i JOIN users u ON lower(u.email)=lower(i.email)
                     WHERE i.id=n.resource_id AND i.workspace_id=n.workspace_id
                       AND u.id=$1 AND i.revoked_at IS NULL AND i.expires_at>NOW()
                ))
           )
           AND COALESCE((
                SELECT np.in_app_enabled
                  FROM notification_preferences np
                 WHERE np.user_id=n.recipient_user_id
                   AND np.notification_type=n.notification_type
           ), TRUE)",
    )
    .bind(user_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|error| internal(error, "Could not count unread notifications"))?;
    transaction.commit().await
        .map_err(|error| internal(error, "Could not finish notification read"))?;
    let next_before = if rows.len() as i64 == limit {
        rows.last().map(|row| row.created_at)
    } else {
        None
    };
    Ok(Json(json!({ "items": rows, "unread_count": unread, "next_before": next_before })))
}

async fn notification_read(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(notification_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let result = sqlx::query(
        "UPDATE notifications n
            SET read_at=COALESCE(n.read_at,NOW())
          WHERE n.id=$1 AND n.recipient_user_id=$2
            AND (
                EXISTS(SELECT 1 FROM workspace_members wm WHERE wm.workspace_id=n.workspace_id AND wm.user_id=$2)
                OR (n.notification_type='workspace_invitation' AND EXISTS(
                    SELECT 1 FROM invitations i JOIN users u ON lower(u.email)=lower(i.email)
                     WHERE i.id=n.resource_id AND i.workspace_id=n.workspace_id AND u.id=$2
                       AND i.revoked_at IS NULL AND i.expires_at>NOW()
                ))
            )
            AND COALESCE((
                SELECT np.in_app_enabled FROM notification_preferences np
                 WHERE np.user_id=n.recipient_user_id
                   AND np.notification_type=n.notification_type
            ), TRUE)"
    )
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
    sqlx::query(
        "UPDATE notifications n SET read_at=NOW()
          WHERE n.recipient_user_id=$1 AND n.read_at IS NULL
            AND (
                EXISTS(SELECT 1 FROM workspace_members wm WHERE wm.workspace_id=n.workspace_id AND wm.user_id=$1)
                OR (n.notification_type='workspace_invitation' AND EXISTS(
                    SELECT 1 FROM invitations i JOIN users u ON lower(u.email)=lower(i.email)
                     WHERE i.id=n.resource_id AND i.workspace_id=n.workspace_id AND u.id=$1
                       AND i.revoked_at IS NULL AND i.expires_at>NOW()
                ))
            )
            AND COALESCE((
                SELECT np.in_app_enabled FROM notification_preferences np
                 WHERE np.user_id=n.recipient_user_id
                   AND np.notification_type=n.notification_type
            ), TRUE)"
    )
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
    let mut transaction = state.db.begin().await
        .map_err(|error| internal(error, "Could not begin notification preference update"))?;
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
    .execute(&mut *transaction)
    .await
    .map_err(|error| internal(error, "Could not update notification preference"))?;
    if input.email_enabled == Some(false) {
        sqlx::query(
            "UPDATE notification_email_outbox outbox
                SET status='suppressed',
                    locked_at=NULL,
                    last_error='Recipient disabled email notifications',
                    updated_at=NOW()
               FROM notifications n
              WHERE outbox.notification_id=n.id
                AND outbox.recipient_user_id=$1
                AND n.notification_type=$2
                AND outbox.status IN ('queued','processing')",
        )
        .bind(user_id)
        .bind(kind)
        .execute(&mut *transaction)
        .await
        .map_err(|error| internal(error, "Could not suppress queued notification emails"))?;
    }
    transaction.commit().await
        .map_err(|error| internal(error, "Could not commit notification preference update"))?;
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
    scan_status: String,
    created_at: chrono::DateTime<Utc>,
}

async fn files_list(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
) -> ApiResult<Json<Vec<WorkspaceFile>>> {
    let rows = sqlx::query_as::<_, WorkspaceFile>(
        "SELECT id, workspace_id, board_id, task_id, uploaded_by, original_name, media_type,
                size_bytes, checksum_sha256, scan_status, created_at
         FROM workspace_files wf WHERE workspace_id=$1 AND is_deleting=FALSE
           AND (board_id IS NULL OR EXISTS(SELECT 1 FROM board_members bm WHERE bm.board_id=wf.board_id AND bm.user_id=$2)
             OR EXISTS(SELECT 1 FROM workspace_members wm WHERE wm.workspace_id=wf.workspace_id AND wm.user_id=$2 AND wm.role IN ('owner','admin')))
           AND (task_id IS NULL OR EXISTS(SELECT 1 FROM tasks t JOIN lists l ON l.id=t.list_id JOIN board_members bm ON bm.board_id=l.board_id WHERE t.id=wf.task_id AND bm.user_id=$2)
             OR EXISTS(SELECT 1 FROM tasks t JOIN lists l ON l.id=t.list_id JOIN boards b ON b.id=l.board_id JOIN workspace_members wm ON wm.workspace_id=b.workspace_id WHERE t.id=wf.task_id AND wm.user_id=$2 AND wm.role IN ('owner','admin')))
         ORDER BY created_at DESC",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load workspace files"))?;
    Ok(Json(rows))
}

async fn files_upload(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    mut multipart: Multipart,
) -> ApiResult<(StatusCode, Json<WorkspaceFile>)> {
    let mut board_id: Option<Uuid> = None;
    let mut task_id: Option<Uuid> = None;
    let mut original_name = String::new();
    let mut media_type = "application/octet-stream".to_string();
    let mut temp_upload: Option<TemporaryUpload> = None;
    let mut size_bytes: i64 = 0;
    let mut checksum = String::new();

    while let Some(mut field) = multipart.next_field().await
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
                if temp_upload.is_some() {
                    return Err(fail(StatusCode::BAD_REQUEST, "Upload one file at a time"));
                }
                original_name = field.file_name().unwrap_or("upload.bin").to_owned();
                media_type = field.content_type().unwrap_or("application/octet-stream").to_owned();

                let temporary = TemporaryUpload {
                    path: std::env::temp_dir().join(format!("campusforge-upload-{}", Uuid::new_v4())),
                };
                let output_file = create_private_upload_file(&temporary.path)
                    .map_err(|error| internal(error, "Could not create private temporary upload"))?;
                let mut output = tokio::fs::File::from_std(output_file);
                let mut hasher = Sha256::new();
                let mut total_bytes: usize = 0;

                while let Some(chunk) = field.chunk().await
                    .map_err(|_| fail(StatusCode::BAD_REQUEST, "Could not read uploaded file"))? {
                    total_bytes = total_bytes.saturating_add(chunk.len());
                    if total_bytes > MAX_UPLOAD_BYTES {
                        return Err(fail(StatusCode::BAD_REQUEST, "Files must not exceed 10 MB"));
                    }
                    hasher.update(&chunk);
                    output.write_all(&chunk)
                        .await
                        .map_err(|error| internal(error, "Could not write temporary upload"))?;
                }

                if total_bytes == 0 {
                    return Err(fail(StatusCode::BAD_REQUEST, "Files must not be empty"));
                }
                output.flush()
                    .await
                    .map_err(|error| internal(error, "Could not finish temporary upload"))?;
                drop(output);

                size_bytes = total_bytes as i64;
                checksum = format!("{:x}", hasher.finalize());
                temp_upload = Some(temporary);
            }
            _ => {}
        }
    }

    if board_id.is_some() && task_id.is_some() {
        return Err(fail(StatusCode::BAD_REQUEST, "Attach a file to a project or a task, not both"));
    }
    if let Some(board_id) = board_id {
        board_write_access(&state,user_id,board_id,workspace_id).await?;
    }
    if let Some(task_id) = task_id {
        task_in_workspace(&state,user_id,task_id,workspace_id,true).await?;
    }
    let temporary = temp_upload.as_ref()
        .ok_or_else(|| fail(StatusCode::BAD_REQUEST, "Choose a file to upload"))?;
    if original_name.trim().is_empty() || original_name.chars().count() > 240 || media_type.len() > 160 {
        return Err(fail(StatusCode::BAD_REQUEST, "File name or media type is invalid"));
    }

    let extension = original_name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    let normalized_media_type = match extension.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "zip" => "application/zip",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "doc" => "application/msword",
        "xls" => "application/vnd.ms-excel",
        "ppt" => "application/vnd.ms-powerpoint",
        "txt" | "md" | "csv" | "json" | "rs" | "py" | "js" | "ts" | "jsx" | "tsx" |
        "java" | "c" | "cpp" | "h" | "css" | "sql" | "go" | "html" | "xml" | "yaml" | "yml" =>
            "text/plain; charset=utf-8",
        _ => return Err(fail(StatusCode::BAD_REQUEST, "This file type is not supported")),
    };

    validate_upload_content(&temporary.path, &extension).await
        .map_err(|message| fail(StatusCode::BAD_REQUEST, message))?;
    let scan_status = scan_upload(&temporary.path).await.map_err(|message| {
        tracing::warn!(workspace_id=%workspace_id, %message, "Workspace upload did not pass malware scanning");
        fail(StatusCode::UNPROCESSABLE_ENTITY, message)
    })?;
    let reservation_id = reserve_workspace_storage(&state, workspace_id, size_bytes).await?;

    let storage_key = Uuid::new_v4().to_string();
    // Persist cleanup ownership before the external object write. If the process
    // stops or the provider times out after accepting the PUT, the outbox can
    // still remove the object after this upload intent expires.
    sqlx::query(
        "INSERT INTO object_deletion_outbox(storage_key, next_attempt_at)
         VALUES ($1, NOW() + INTERVAL '30 minutes')
         ON CONFLICT (storage_key) DO NOTHING",
    )
    .bind(&storage_key)
    .execute(&state.db)
    .await
    .map_err(|error| internal(error, "Could not register upload cleanup intent"))?;

    state.storage.put_file(&storage_key, &temporary.path, normalized_media_type, &checksum, size_bytes as u64)
        .await
        .map_err(|error| {
            tracing::error!(%error, "Could not store workspace file; cleanup intent retained");
            fail(StatusCode::INTERNAL_SERVER_ERROR, "Could not store uploaded file")
        })?;

    let mut upload_tx = state.db.begin().await
        .map_err(|error| internal(error, "Could not begin file metadata transaction"))?;
    let file = sqlx::query_as::<_, WorkspaceFile>(
        "INSERT INTO workspace_files
            (workspace_id, board_id, task_id, uploaded_by, original_name, storage_key, media_type, size_bytes, checksum_sha256, scan_status)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
         RETURNING id, workspace_id, board_id, task_id, uploaded_by, original_name, media_type,
                   size_bytes, checksum_sha256, scan_status, created_at",
    )
    .bind(workspace_id)
    .bind(board_id)
    .bind(task_id)
    .bind(user_id)
    .bind(original_name.trim())
    .bind(&storage_key)
    .bind(normalized_media_type)
    .bind(size_bytes)
    .bind(&checksum)
    .bind(scan_status)
    .fetch_one(&mut *upload_tx)
    .await
    .map_err(|error| internal(error, "Could not save uploaded file metadata"))?;

    sqlx::query("DELETE FROM workspace_storage_reservations WHERE id=$1")
        .bind(reservation_id)
        .execute(&mut *upload_tx)
        .await
        .map_err(|error| internal(error, "Could not release storage reservation"))?;

    sqlx::query("INSERT INTO file_audit_events(workspace_id,file_id,actor_user_id,action,original_name,size_bytes,metadata) VALUES($1,$2,$3,'uploaded',$4,$5,jsonb_build_object('scan_status',$6))")
        .bind(workspace_id).bind(file.id).bind(user_id).bind(&file.original_name).bind(file.size_bytes).bind(&file.scan_status)
        .execute(&mut *upload_tx).await.map_err(|error| internal(error, "Could not record file upload audit"))?;

    sqlx::query("DELETE FROM object_deletion_outbox WHERE storage_key=$1")
        .bind(&storage_key)
        .execute(&mut *upload_tx)
        .await
        .map_err(|error| internal(error, "Could not finalize upload cleanup intent"))?;
    upload_tx.commit().await
        .map_err(|error| internal(error, "Could not commit uploaded file metadata"))?;

    state.publish_workspace_event(workspace_id, board_id, "file_uploaded", format!("uploaded {}", file.original_name));
    Ok((StatusCode::CREATED, Json(file)))
}

async fn file_download(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(file_id): Path<Uuid>,
) -> ApiResult<Response<axum::body::Body>> {
    let row: Option<(String, String, String, String, i64, Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "SELECT storage_key, original_name, media_type, scan_status, size_bytes, board_id, task_id FROM workspace_files WHERE id=$1 AND workspace_id=$2 AND is_deleting=FALSE",
    )
    .bind(file_id)
    .bind(workspace_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load file metadata"))?;
    let Some((storage_key, original_name, media_type, scan_status, size_bytes, board_id, task_id)) = row else {
        return Err(fail(StatusCode::NOT_FOUND, "File not found"));
    };

    if let Some(board_id)=board_id { board_in_workspace(&state,user_id,board_id,workspace_id).await?; }
    if let Some(task_id)=task_id { task_in_workspace(&state,user_id,task_id,workspace_id,false).await?; }
    if scan_status == "infected" || (std::env::var("CAMPUSFORGE_REQUIRE_MALWARE_SCAN").ok().is_some_and(|value| value.eq_ignore_ascii_case("true")) && scan_status != "clean") {
        return Err(fail(StatusCode::LOCKED, "File download is unavailable until malware scanning passes"));
    }
    let bytes = state.storage.get(&storage_key).await.map_err(|error| {
        tracing::error!(%error, "Could not read workspace file");
        fail(StatusCode::NOT_FOUND, "Stored file is unavailable")
    })?;
    let safe_name: String = original_name.chars()
        .map(|character| if character.is_ascii_alphanumeric() || "._- ".contains(character) { character } else { '_' })
        .collect();
    sqlx::query("INSERT INTO file_audit_events(workspace_id,file_id,actor_user_id,action,original_name,size_bytes) VALUES($1,$2,$3,'downloaded',$4,$5)")
        .bind(workspace_id).bind(file_id).bind(user_id).bind(&original_name).bind(size_bytes)
        .execute(&state.db).await.map_err(|error| internal(error, "Could not record file download audit"))?;
    let content_type = HeaderValue::from_str(&media_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    let disposition = HeaderValue::from_str(&format!("attachment; filename=\"{}\"", safe_name))
        .unwrap_or_else(|_| HeaderValue::from_static("attachment; filename=\"download\""));

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_DISPOSITION, disposition)
        .header("x-content-type-options", "nosniff")
        .header(header::CACHE_CONTROL, "no-store")
        .body(axum::body::Body::from(bytes))
        .map_err(|error| internal(error, "Could not prepare file download"))
}

async fn file_delete(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(file_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut tx=state.db.begin().await.map_err(|error|internal(error,"Could not begin file deletion"))?;
    let existing: Option<(bool,String,i64,Uuid,Option<Uuid>,Option<Uuid>)> = sqlx::query_as(
        "SELECT is_deleting,original_name,size_bytes,uploaded_by,board_id,task_id FROM workspace_files
         WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
    )
    .bind(file_id)
    .bind(workspace_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|error| internal(error, "Could not load file metadata"))?;
    let Some((is_deleting,original_name,size_bytes,uploaded_by,board_id,task_id)) = existing else {
        return Err(fail(StatusCode::NOT_FOUND, "File not found"));
    };
    if let Some(board_id)=board_id { board_write_access(&state,user_id,board_id,workspace_id).await?; }
    if let Some(task_id)=task_id { task_in_workspace(&state,user_id,task_id,workspace_id,true).await?; }
    if uploaded_by!=user_id && !crate::roadmap::workspace_admin(&state,user_id,workspace_id).await
        .map_err(|error|internal(error,"Could not verify file deletion permission"))? {
        return Err(fail(StatusCode::FORBIDDEN,"Only the uploader or a workspace admin can delete this file"));
    }
    if is_deleting {
        tx.rollback().await.map_err(|error|internal(error,"Could not finish file deletion request"))?;
        return Ok(StatusCode::ACCEPTED);
    }

    sqlx::query(
        "UPDATE workspace_files
            SET is_deleting=TRUE, delete_after=NOW(), delete_error=NULL
          WHERE id=$1 AND workspace_id=$2 AND is_deleting=FALSE",
    )
    .bind(file_id)
    .bind(workspace_id)
    .execute(&mut *tx)
    .await
    .map_err(|error| internal(error, "Could not schedule file deletion"))?;

    sqlx::query("INSERT INTO file_audit_events(workspace_id,file_id,actor_user_id,action,original_name,size_bytes) VALUES($1,$2,$3,'deletion_queued',$4,$5)")
        .bind(workspace_id).bind(file_id).bind(user_id).bind(original_name).bind(size_bytes)
        .execute(&mut *tx).await.map_err(|error|internal(error,"Could not record file deletion audit"))?;
    tx.commit().await.map_err(|error|internal(error,"Could not save file deletion request"))?;

    state.publish_workspace_event(workspace_id, None, "file_deletion_queued", "queued workspace file deletion");
    Ok(StatusCode::ACCEPTED)
}

#[derive(Debug, Serialize, FromRow)]
struct ChatMessage {
    id: Uuid,
    workspace_id: Uuid,
    board_id: Option<Uuid>,
    author_id: Uuid,
    author_name: String,
    body: String,
    is_pinned: bool,
    edited_at: Option<chrono::DateTime<Utc>>,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct ChatQuery {
    board_id: Option<Uuid>,
    before: Option<chrono::DateTime<Utc>>,
    limit: Option<i64>,
}

async fn chat_list(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Query(query): Query<ChatQuery>,
) -> ApiResult<Json<Vec<ChatMessage>>> {
    if let Some(board_id) = query.board_id {
        board_in_workspace(&state,user_id,board_id,workspace_id).await?;
    }
    let limit=query.limit.unwrap_or(50).clamp(1,100);
    let rows = sqlx::query_as::<_, ChatMessage>(
        "SELECT m.id, m.workspace_id, m.board_id, m.author_id, u.full_name AS author_name,
                CASE WHEN m.deleted_at IS NULL THEN m.body ELSE 'This message was deleted' END AS body,
                m.is_pinned, m.edited_at, m.created_at
         FROM chat_messages m JOIN users u ON u.id=m.author_id
         WHERE m.workspace_id=$1
           -- Use row JSON so this query works before migration 0010 adds conversation_id.
           AND COALESCE(to_jsonb(m)->>'conversation_id','')=''
           AND (($2::uuid IS NULL AND m.board_id IS NULL) OR m.board_id=$2)
           AND ($3::timestamptz IS NULL OR m.created_at<$3)
         ORDER BY m.created_at DESC LIMIT $4",
    )
    .bind(workspace_id)
    .bind(query.board_id)
    .bind(query.before)
    .bind(limit)
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
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Json(input): Json<ChatInput>,
) -> ApiResult<(StatusCode, Json<ChatMessage>)> {
    let body = input.body.trim();
    if body.is_empty() || body.chars().count() > 4000 {
        return Err(fail(StatusCode::BAD_REQUEST, "Message must be between 1 and 4000 characters"));
    }
    if let Some(board_id) = input.board_id {
        board_write_access(&state,user_id,board_id,workspace_id).await?;
    }
    let mut tx=state.db.begin().await.map_err(|error|internal(error,"Could not begin team message"))?;
    let message = sqlx::query_as::<_, ChatMessage>(
        "INSERT INTO chat_messages (workspace_id, board_id, author_id, body)
         VALUES ($1,$2,$3,$4)
         RETURNING id, workspace_id, board_id, author_id,
            (SELECT full_name FROM users WHERE id=$3) AS author_name, body,
            FALSE AS is_pinned, NULL::timestamptz AS edited_at, created_at",
    )
    .bind(workspace_id)
    .bind(input.board_id)
    .bind(user_id)
    .bind(body)
    .fetch_one(&mut *tx)
    .await
    .map_err(|error| internal(error, "Could not send team message"))?;
    roadmap::create_mentions_in_transaction(&mut tx,workspace_id,user_id,"chat_message",message.id,body)
        .await
        .map_err(|error|internal(error,"Could not create chat mention notifications"))?;
    tx.commit().await.map_err(|error|internal(error,"Could not commit team message"))?;
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
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
) -> ApiResult<Json<Vec<MeetingNote>>> {
    let rows = sqlx::query_as::<_, MeetingNote>(
        "SELECT id, workspace_id, board_id, created_by, title, meeting_at, attendees,
                agenda, notes, action_items, created_at, updated_at
         FROM meeting_notes mn WHERE workspace_id=$1
           AND (board_id IS NULL OR EXISTS(SELECT 1 FROM board_members bm WHERE bm.board_id=mn.board_id AND bm.user_id=$2)
             OR EXISTS(SELECT 1 FROM workspace_members wm WHERE wm.workspace_id=mn.workspace_id AND wm.user_id=$2 AND wm.role IN ('owner','admin')))
         ORDER BY meeting_at DESC NULLS LAST, created_at DESC",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load meeting notes"))?;
    Ok(Json(rows))
}

async fn meeting_note_create(
    State(state): State<AppState>,
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Json(input): Json<MeetingNoteInput>,
) -> ApiResult<(StatusCode, Json<MeetingNote>)> {
    validate_meeting_note(&input)?;
    if let Some(board_id) = input.board_id {
        board_write_access(&state,user_id,board_id,workspace_id).await?;
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
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(note_id): Path<Uuid>,
    Json(input): Json<MeetingNoteInput>,
) -> ApiResult<StatusCode> {
    validate_meeting_note(&input)?;
    let existing:Option<(Uuid,Option<Uuid>)>=sqlx::query_as(
        "SELECT created_by,board_id FROM meeting_notes WHERE id=$1 AND workspace_id=$2",
    ).bind(note_id).bind(workspace_id).fetch_optional(&state.db).await
      .map_err(|error|internal(error,"Could not verify meeting note permissions"))?;
    let Some((creator,existing_board_id))=existing else { return Err(fail(StatusCode::NOT_FOUND,"Meeting notes not found")); };
    if let Some(board_id)=existing_board_id { board_write_access(&state,user_id,board_id,workspace_id).await?; }
    if let Some(board_id)=input.board_id { board_write_access(&state,user_id,board_id,workspace_id).await?; }
    if creator!=user_id && !crate::roadmap::workspace_admin(&state,user_id,workspace_id).await
        .map_err(|error|internal(error,"Could not verify meeting note edit permission"))? {
        return Err(fail(StatusCode::FORBIDDEN,"Only the meeting notes creator or a workspace admin can edit them"));
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
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
    Path(note_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let note:Option<(Uuid,Option<Uuid>)>=sqlx::query_as("SELECT created_by,board_id FROM meeting_notes WHERE id=$1 AND workspace_id=$2")
        .bind(note_id).bind(workspace_id).fetch_optional(&state.db).await
        .map_err(|error|internal(error,"Could not verify meeting note permissions"))?;
    let Some((creator,board_id))=note else { return Err(fail(StatusCode::NOT_FOUND,"Meeting notes not found")); };
    if let Some(board_id)=board_id { board_write_access(&state,user_id,board_id,workspace_id).await?; }
    if creator!=user_id { return Err(fail(StatusCode::FORBIDDEN,"Only the meeting notes creator can delete them")); }
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
    WorkspaceUser(user_id, workspace_id): WorkspaceUser,
) -> ApiResult<Json<Value>> {
    let (total, completed, overdue, unassigned): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT
           COUNT(*)::bigint,
           COUNT(*) FILTER (WHERE lower(l.name) IN ('done','complete','completed'))::bigint,
           COUNT(*) FILTER (WHERE t.due_at < NOW() AND lower(l.name) NOT IN ('done','complete','completed'))::bigint,
           COUNT(*) FILTER (WHERE NOT EXISTS(SELECT 1 FROM task_assignees ta WHERE ta.task_id=t.id))::bigint
         FROM tasks t
         JOIN lists l ON l.id=t.list_id
         JOIN boards b ON b.id=l.board_id
         WHERE b.workspace_id=$1 AND (EXISTS(SELECT 1 FROM board_members bm WHERE bm.board_id=b.id AND bm.user_id=$2)
           OR EXISTS(SELECT 1 FROM workspace_members wm WHERE wm.workspace_id=b.workspace_id AND wm.user_id=$2 AND wm.role IN ('owner','admin')))",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    .map_err(|error| internal(error, "Could not calculate workspace metrics"))?;

    let members = sqlx::query_as::<_, (Uuid, String, i64, i64)>(
        "SELECT u.id, u.full_name,
           COUNT(DISTINCT t.id) FILTER (WHERE b.id IS NOT NULL AND lower(l.name) NOT IN ('done','complete','completed'))::bigint AS open_tasks,
           COUNT(DISTINCT t.id) FILTER (WHERE b.id IS NOT NULL AND lower(l.name) IN ('done','complete','completed'))::bigint AS completed_tasks
         FROM workspace_members wm
         JOIN users u ON u.id=wm.user_id
         LEFT JOIN task_assignees ta ON ta.user_id=u.id
         LEFT JOIN tasks t ON t.id=ta.task_id
         LEFT JOIN lists l ON l.id=t.list_id
         LEFT JOIN boards b ON b.id=l.board_id AND b.workspace_id=wm.workspace_id
          AND (EXISTS(SELECT 1 FROM board_members own WHERE own.board_id=b.id AND own.user_id=$2)
               OR wm.role IN ('owner','admin'))
         WHERE wm.workspace_id=$1
         GROUP BY u.id, u.full_name ORDER BY u.full_name",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not calculate member workload"))?;

    let project_rows = sqlx::query_as::<_, (Uuid, String, i64, i64)>(
        "SELECT b.id, b.name, COUNT(t.id)::bigint,
           COUNT(t.id) FILTER (WHERE lower(l.name) IN ('done','complete','completed'))::bigint
         FROM boards b
         LEFT JOIN lists l ON l.board_id=b.id
         LEFT JOIN tasks t ON t.list_id=l.id
         WHERE b.workspace_id=$1 AND (EXISTS(SELECT 1 FROM board_members own WHERE own.board_id=b.id AND own.user_id=$2)
           OR EXISTS(SELECT 1 FROM workspace_members wm WHERE wm.workspace_id=b.workspace_id AND wm.user_id=$2 AND wm.role IN ('owner','admin')))
         GROUP BY b.id,b.name ORDER BY b.name",
    )
    .bind(workspace_id)
    .bind(user_id)
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


#[derive(Debug, FromRow)]
struct PendingFileDeletion {
    id: Uuid,
    workspace_id: Uuid,
    original_name: String,
    size_bytes: i64,
    storage_key: String,
    attempts: i32,
}

async fn cleanup_pending_files(state: &AppState) {
    let mut transaction = match state.db.begin().await {
        Ok(transaction) => transaction,
        Err(error) => {
            tracing::error!(%error, "Could not begin file cleanup batch");
            return;
        }
    };

    let rows = match sqlx::query_as::<_, PendingFileDeletion>(
        "SELECT id, workspace_id, original_name, size_bytes, storage_key, delete_attempts AS attempts
           FROM workspace_files
          WHERE is_deleting=TRUE AND delete_after <= NOW()
          ORDER BY delete_after, created_at
          LIMIT 10
          FOR UPDATE SKIP LOCKED",
    )
    .fetch_all(&mut *transaction)
    .await {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!(%error, "Could not load pending file deletions");
            return;
        }
    };

    let mut changed_workspaces = Vec::new();
    for row in rows {
        match state.storage.delete(&row.storage_key).await {
            Ok(()) => {
                if let Err(error) = sqlx::query(
                    "INSERT INTO file_audit_events(workspace_id,file_id,action,original_name,size_bytes)
                     VALUES($1,$2,'deleted',$3,$4)",
                )
                .bind(row.workspace_id)
                .bind(row.id)
                .bind(&row.original_name)
                .bind(row.size_bytes)
                .execute(&mut *transaction)
                .await {
                    tracing::error!(%error, file_id=%row.id, "Could not record completed file deletion");
                    let _=transaction.rollback().await;
                    return;
                }
                if let Err(error) = sqlx::query(
                    "UPDATE workspace_files SET object_delete_confirmed=TRUE
                      WHERE id=$1 AND is_deleting=TRUE",
                )
                .bind(row.id)
                .execute(&mut *transaction)
                .await {
                    tracing::error!(%error, file_id=%row.id, "Could not confirm object deletion");
                    let _=transaction.rollback().await;
                    return;
                }
                match sqlx::query_scalar::<_, Uuid>(
                    "DELETE FROM workspace_files WHERE id=$1 AND is_deleting=TRUE RETURNING workspace_id",
                )
                .bind(row.id)
                .fetch_optional(&mut *transaction)
                .await {
                    Ok(Some(workspace_id)) => changed_workspaces.push(workspace_id),
                    Ok(None) => {}
                    Err(error) => {
                        tracing::error!(%error, file_id=%row.id, "Could not remove deleted file metadata");
                        let _=transaction.rollback().await;
                        return;
                    },
                }
            }
            Err(error) => {
                let backoff_seconds = (60_i64 * 2_i64.pow(row.attempts.clamp(0, 6) as u32)).min(3600);
                if let Err(update_error) = sqlx::query(
                    "UPDATE workspace_files
                        SET delete_attempts=delete_attempts+1,
                            delete_error=$2,
                            delete_after=statement_timestamp()+make_interval(secs=>$3)
                      WHERE id=$1 AND is_deleting=TRUE",
                )
                .bind(row.id)
                .bind(error)
                .bind(backoff_seconds as f64)
                .execute(&mut *transaction)
                .await {
                    tracing::error!(%update_error, file_id=%row.id, "Could not schedule file deletion retry");
                    let _=transaction.rollback().await;
                    return;
                }
            }
        }
    }

    match transaction.commit().await {
        Ok(()) => {
            for workspace_id in changed_workspaces {
                state.publish_workspace_event(workspace_id, None, "file_deleted", "completed workspace file deletion");
            }
        }
        Err(error) => tracing::error!(%error, "Could not commit workspace file cleanup batch"),
    }
}

#[derive(Debug, FromRow)]
struct PendingObjectDeletion {
    storage_key: String,
    attempts: i32,
}

/// Object keys are copied into this outbox by a DELETE trigger on workspace_files.
/// That trigger also runs for task, board, and workspace cascades, so metadata
/// removal never loses the last durable reference to the object bytes.
async fn cleanup_orphaned_objects(state: &AppState) {
    let mut transaction = match state.db.begin().await {
        Ok(transaction) => transaction,
        Err(error) => {
            tracing::error!(%error, "Could not begin orphaned-object cleanup batch");
            return;
        }
    };

    let rows = match sqlx::query_as::<_, PendingObjectDeletion>(
        "SELECT storage_key, attempts
           FROM object_deletion_outbox
          WHERE next_attempt_at <= NOW()
          ORDER BY next_attempt_at, created_at
          FOR UPDATE SKIP LOCKED
          LIMIT 20",
    )
    .fetch_all(&mut *transaction)
    .await {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!(%error, "Could not load orphaned-object cleanup jobs");
            return;
        }
    };

    for row in rows {
        match state.storage.delete(&row.storage_key).await {
            Ok(()) => {
                if let Err(error) = sqlx::query(
                    "DELETE FROM object_deletion_outbox WHERE storage_key=$1",
                )
                .bind(&row.storage_key)
                .execute(&mut *transaction)
                .await {
                    tracing::error!(%error, storage_key=%row.storage_key, "Could not clear completed object deletion");
                }
            }
            Err(error) => {
                let backoff_seconds = (60_i64 * 2_i64.pow(row.attempts.clamp(0, 8) as u32)).min(21600);
                if let Err(update_error) = sqlx::query(
                    "UPDATE object_deletion_outbox
                        SET attempts=attempts+1,
                            last_error=$2,
                            next_attempt_at=NOW()+make_interval(secs=>$3),
                            updated_at=NOW()
                      WHERE storage_key=$1",
                )
                .bind(&row.storage_key)
                .bind(error)
                .bind(backoff_seconds as f64)
                .execute(&mut *transaction)
                .await {
                    tracing::error!(%update_error, storage_key=%row.storage_key, "Could not schedule orphaned-object retry");
                }
            }
        }
    }

    if let Err(error) = transaction.commit().await {
        tracing::error!(%error, "Could not commit orphaned-object cleanup batch");
    }
}

pub async fn run_file_cleanup(state: AppState) {
    let mut ticker = interval(TokioDuration::from_secs(60));
    loop {
        ticker.tick().await;
        cleanup_pending_files(&state).await;
        cleanup_orphaned_objects(&state).await;
    }
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
