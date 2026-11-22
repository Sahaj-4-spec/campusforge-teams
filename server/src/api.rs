use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, patch, post},
    Json, Router,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use std::collections::HashSet;
use uuid::Uuid;

use crate::{auth::AuthUser, state::AppState};

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: &'static str,
}

type ApiResult<T> = Result<T, (StatusCode, Json<ErrorBody>)>;

fn error(status: StatusCode, message: &'static str) -> (StatusCode, Json<ErrorBody>) {
    (status, Json(ErrorBody { error: message }))
}

fn internal_error<E: std::fmt::Display>(cause: E, context: &'static str) -> (StatusCode, Json<ErrorBody>) {
    tracing::error!(error = %cause, "{context}");
    error(StatusCode::INTERNAL_SERVER_ERROR, "The request could not be completed")
}

#[derive(Debug, Serialize, FromRow)]
struct WorkspaceRow {
    id: Uuid,
    name: String,
    description: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct UserRow {
    id: Uuid,
    email: String,
    full_name: String,
    avatar_url: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
struct WorkspaceMemberRow {
    workspace_id: Uuid,
    user_id: Uuid,
    role: String,
}

#[derive(Debug, Serialize, FromRow)]
struct BoardRow {
    id: Uuid,
    workspace_id: Uuid,
    name: String,
    description: Option<String>,
    created_by: Uuid,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct BoardMemberRow {
    board_id: Uuid,
    user_id: Uuid,
    role: String,
}

#[derive(Debug, Serialize, FromRow)]
struct ListRow {
    id: Uuid,
    board_id: Uuid,
    name: String,
    position: i32,
}

#[derive(Debug, Serialize, FromRow)]
struct TaskRow {
    id: Uuid,
    board_id: Uuid,
    list_id: Uuid,
    title: String,
    description: String,
    priority: String,
    due_date: Option<String>,
    position: i32,
    created_by: Uuid,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct TaskAssigneeRow {
    task_id: Uuid,
    user_id: Uuid,
}

#[derive(Debug, Serialize, FromRow)]
struct CommentRow {
    id: Uuid,
    task_id: Uuid,
    author_id: Uuid,
    body: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct ActivityRow {
    id: Uuid,
    board_id: Option<Uuid>,
    actor_id: Option<Uuid>,
    event_type: String,
    description: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct WorkspaceSnapshot {
    current_user_id: Uuid,
    current_workspace_id: Uuid,
    workspaces: Vec<WorkspaceRow>,
    users: Vec<UserRow>,
    workspace_members: Vec<WorkspaceMemberRow>,
    boards: Vec<BoardRow>,
    board_members: Vec<BoardMemberRow>,
    lists: Vec<ListRow>,
    tasks: Vec<TaskRow>,
    task_assignees: Vec<TaskAssigneeRow>,
    comments: Vec<CommentRow>,
    activity_events: Vec<ActivityRow>,
}

/// Ensure newly registered and older accounts both have a usable first workspace.
async fn ensure_default_workspace(pool: &PgPool, user_id: Uuid) -> Result<(), sqlx::Error> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM workspace_members WHERE user_id = $1)",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    if exists {
        return Ok(());
    }

    let full_name: String = sqlx::query_scalar("SELECT full_name FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;

    let mut tx = pool.begin().await?;
    let workspace = sqlx::query_as::<_, WorkspaceRow>(
        "INSERT INTO workspaces (name, description, owner_id)          VALUES ($1, $2, $3) RETURNING id, name, description, created_at",
    )
    .bind(format!("{full_name}'s Workspace"))
    .bind("A shared space for your team's projects and tasks.")
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO workspace_members (workspace_id, user_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(workspace.id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    let board_id: Uuid = sqlx::query_scalar(
        "INSERT INTO boards (workspace_id, name, description, created_by)          VALUES ($1, 'My First Project', 'Organize your first team project here.', $2)          RETURNING id",
    )
    .bind(workspace.id)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO board_members (board_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(board_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;

    for (position, name) in ["To do", "In progress", "In review", "Done"]
        .iter()
        .enumerate()
    {
        sqlx::query("INSERT INTO lists (board_id, name, position) VALUES ($1, $2, $3)")
            .bind(board_id)
            .bind(name)
            .bind((position + 1) as i32)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    Ok(())
}

async fn get_workspace(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
) -> ApiResult<Json<WorkspaceSnapshot>> {
    ensure_default_workspace(&state.db, user_id)
        .await
        .map_err(|cause| internal_error(cause, "Could not initialize the user's workspace"))?;

    let workspace = sqlx::query_as::<_, WorkspaceRow>(
        "SELECT w.id, w.name, w.description, w.created_at          FROM workspaces w          JOIN workspace_members wm ON wm.workspace_id = w.id          WHERE wm.user_id = $1 ORDER BY wm.joined_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await
    .map_err(|cause| internal_error(cause, "Could not load workspace"))?;

    let users = sqlx::query_as::<_, UserRow>(
        "SELECT u.id, u.email, u.full_name, u.avatar_url          FROM users u JOIN workspace_members wm ON wm.user_id = u.id          WHERE wm.workspace_id = $1 ORDER BY u.full_name",
    )
    .bind(workspace.id)
    .fetch_all(&state.db)
    .await
    .map_err(|cause| internal_error(cause, "Could not load workspace users"))?;

    let workspace_members = sqlx::query_as::<_, WorkspaceMemberRow>(
        "SELECT workspace_id, user_id, role FROM workspace_members WHERE workspace_id = $1",
    )
    .bind(workspace.id)
    .fetch_all(&state.db)
    .await
    .map_err(|cause| internal_error(cause, "Could not load workspace membership"))?;

    let boards = sqlx::query_as::<_, BoardRow>(
        "SELECT id, workspace_id, name, description, created_by, created_at          FROM boards WHERE workspace_id = $1 ORDER BY created_at",
    )
    .bind(workspace.id)
    .fetch_all(&state.db)
    .await
    .map_err(|cause| internal_error(cause, "Could not load boards"))?;

    let board_members = sqlx::query_as::<_, BoardMemberRow>(
        "SELECT bm.board_id, bm.user_id, bm.role          FROM board_members bm JOIN boards b ON b.id = bm.board_id          WHERE b.workspace_id = $1",
    )
    .bind(workspace.id)
    .fetch_all(&state.db)
    .await
    .map_err(|cause| internal_error(cause, "Could not load board membership"))?;

    let lists = sqlx::query_as::<_, ListRow>(
        "SELECT l.id, l.board_id, l.name, l.position          FROM lists l JOIN boards b ON b.id = l.board_id          WHERE b.workspace_id = $1 ORDER BY l.board_id, l.position",
    )
    .bind(workspace.id)
    .fetch_all(&state.db)
    .await
    .map_err(|cause| internal_error(cause, "Could not load lists"))?;

    let tasks = sqlx::query_as::<_, TaskRow>(
        "SELECT t.id, l.board_id, t.list_id, t.title, COALESCE(t.description, '') AS description,          t.priority, CASE WHEN t.due_at IS NULL THEN NULL          ELSE to_char(t.due_at AT TIME ZONE 'UTC', 'YYYY-MM-DD') END AS due_date,          t.position, t.created_by, t.created_at, t.updated_at          FROM tasks t JOIN lists l ON l.id = t.list_id          JOIN boards b ON b.id = l.board_id          WHERE b.workspace_id = $1 ORDER BY l.board_id, t.list_id, t.position",
    )
    .bind(workspace.id)
    .fetch_all(&state.db)
    .await
    .map_err(|cause| internal_error(cause, "Could not load tasks"))?;

    let task_assignees = sqlx::query_as::<_, TaskAssigneeRow>(
        "SELECT ta.task_id, ta.user_id FROM task_assignees ta          JOIN tasks t ON t.id = ta.task_id JOIN lists l ON l.id = t.list_id          JOIN boards b ON b.id = l.board_id WHERE b.workspace_id = $1",
    )
    .bind(workspace.id)
    .fetch_all(&state.db)
    .await
    .map_err(|cause| internal_error(cause, "Could not load task assignees"))?;

    let comments = sqlx::query_as::<_, CommentRow>(
        "SELECT c.id, c.task_id, c.author_id, c.body, c.created_at FROM comments c          JOIN tasks t ON t.id = c.task_id JOIN lists l ON l.id = t.list_id          JOIN boards b ON b.id = l.board_id WHERE b.workspace_id = $1          ORDER BY c.created_at",
    )
    .bind(workspace.id)
    .fetch_all(&state.db)
    .await
    .map_err(|cause| internal_error(cause, "Could not load comments"))?;

    let activity_events = sqlx::query_as::<_, ActivityRow>(
        "SELECT id, board_id, actor_id, event_type,          COALESCE(payload->>'description', event_type) AS description, created_at          FROM activity_events WHERE workspace_id = $1 ORDER BY created_at DESC LIMIT 200",
    )
    .bind(workspace.id)
    .fetch_all(&state.db)
    .await
    .map_err(|cause| internal_error(cause, "Could not load activity"))?;

    Ok(Json(WorkspaceSnapshot {
        current_user_id: user_id,
        current_workspace_id: workspace.id,
        workspaces: vec![workspace],
        users,
        workspace_members,
        boards,
        board_members,
        lists,
        tasks,
        task_assignees,
        comments,
        activity_events,
    }))
}

#[derive(Debug, Deserialize)]
struct BoardInput {
    name: String,
    description: Option<String>,
}

async fn create_board(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(input): Json<BoardInput>,
) -> ApiResult<StatusCode> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(error(StatusCode::BAD_REQUEST, "Project name must be between 1 and 80 characters"));
    }
    if input.description.as_ref().is_some_and(|value| value.chars().count() > 1000) {
        return Err(error(StatusCode::BAD_REQUEST, "Project description is too long"));
    }

    let mut tx = state.db.begin().await.map_err(|cause| internal_error(cause, "Could not begin project creation"))?;
    let membership: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT workspace_id, role FROM workspace_members WHERE user_id = $1 ORDER BY joined_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not find the user's workspace"))?;
    let Some((workspace_id, workspace_role)) = membership else {
        return Err(error(StatusCode::NOT_FOUND, "Workspace not found"));
    };
    if workspace_role != "owner" && workspace_role != "admin" {
        return Err(error(StatusCode::FORBIDDEN, "Only workspace owners and admins can create projects"));
    }

    let board_id: Uuid = sqlx::query_scalar(
        "INSERT INTO boards (workspace_id, name, description, created_by)          VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(workspace_id)
    .bind(name)
    .bind(input.description.as_deref().filter(|value| !value.trim().is_empty()))
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not create project"))?;

    sqlx::query("INSERT INTO board_members (board_id, user_id, role) VALUES ($1, $2, 'admin')")
        .bind(board_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not add project owner"))?;

    for (position, list_name) in ["To do", "In progress", "In review", "Done"].iter().enumerate() {
        sqlx::query("INSERT INTO lists (board_id, name, position) VALUES ($1, $2, $3)")
            .bind(board_id)
            .bind(list_name)
            .bind((position + 1) as i32)
            .execute(&mut *tx)
            .await
            .map_err(|cause| internal_error(cause, "Could not create starter project columns"))?;
    }

    add_activity(&mut tx, workspace_id, board_id, None, user_id, "board_created", format!("created project {name}"))
        .await.map_err(|cause| internal_error(cause, "Could not record project activity"))?;
    tx.commit().await.map_err(|cause| internal_error(cause, "Could not save project"))?;
    state.publish_workspace_event(workspace_id, Some(board_id), "board_created", format!("created project {name}"));
    Ok(StatusCode::CREATED)
}

#[derive(Debug, Deserialize)]
struct TaskInput {
    title: String,
    description: Option<String>,
    priority: Option<String>,
    due_date: Option<NaiveDate>,
    list_id: Uuid,
    assignee_id: Option<Uuid>,
}

fn validate_task(input: &TaskInput) -> ApiResult<&str> {
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > 180 {
        return Err(error(StatusCode::BAD_REQUEST, "Task title must be between 1 and 180 characters"));
    }
    if input.description.as_ref().is_some_and(|value| value.chars().count() > 4000) {
        return Err(error(StatusCode::BAD_REQUEST, "Task description is too long"));
    }
    let priority = input.priority.as_deref().unwrap_or("medium");
    if !["low", "medium", "high", "urgent"].contains(&priority) {
        return Err(error(StatusCode::BAD_REQUEST, "Task priority is invalid"));
    }
    Ok(title)
}

async fn add_activity(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    board_id: Uuid,
    task_id: Option<Uuid>,
    actor_id: Uuid,
    event_type: &'static str,
    description: String,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO activity_events (workspace_id, board_id, task_id, actor_id, event_type, payload)          VALUES ($1, $2, $3, $4, $5, jsonb_build_object('description', $6))",
    )
    .bind(workspace_id)
    .bind(board_id)
    .bind(task_id)
    .bind(actor_id)
    .bind(event_type)
    .bind(description)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn ensure_assignee(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    assignee_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM workspace_members WHERE workspace_id = $1 AND user_id = $2)",
    )
    .bind(workspace_id)
    .bind(assignee_id)
    .fetch_one(&mut **tx)
    .await
}

async fn create_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Json(input): Json<TaskInput>,
) -> ApiResult<StatusCode> {
    let title = validate_task(&input)?;
    let priority = input.priority.as_deref().unwrap_or("medium");
    let description = input.description.as_deref().filter(|value| !value.trim().is_empty());
    let due_at: Option<DateTime<Utc>> = input.due_date.map(|date| date.and_hms_opt(0, 0, 0).unwrap().and_utc());

    let mut tx = state.db.begin().await.map_err(|cause| internal_error(cause, "Could not begin task creation"))?;
    let scope: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT b.workspace_id, b.id FROM lists l JOIN boards b ON b.id = l.board_id          JOIN workspace_members wm ON wm.workspace_id = b.workspace_id          WHERE l.id = $1 AND wm.user_id = $2",
    )
    .bind(input.list_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not verify task list"))?;
    let Some((workspace_id, board_id)) = scope else {
        return Err(error(StatusCode::NOT_FOUND, "The selected list was not found"));
    };

    if let Some(assignee_id) = input.assignee_id {
        let is_member = ensure_assignee(&mut tx, workspace_id, assignee_id)
            .await.map_err(|cause| internal_error(cause, "Could not verify assignee"))?;
        if !is_member {
            return Err(error(StatusCode::BAD_REQUEST, "Assignee must be a workspace member"));
        }
    }

    let position: i32 = sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 1 FROM tasks WHERE list_id = $1")
        .bind(input.list_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not find task position"))?;

    let task_id: Uuid = sqlx::query_scalar(
        "INSERT INTO tasks (list_id, title, description, priority, due_at, position, created_by)          VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(input.list_id)
    .bind(title)
    .bind(description)
    .bind(priority)
    .bind(due_at)
    .bind(position)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not create task"))?;

    if let Some(assignee_id) = input.assignee_id {
        sqlx::query("INSERT INTO task_assignees (task_id, user_id) VALUES ($1, $2)")
            .bind(task_id)
            .bind(assignee_id)
            .execute(&mut *tx)
            .await
            .map_err(|cause| internal_error(cause, "Could not assign task"))?;
    }

    add_activity(&mut tx, workspace_id, board_id, Some(task_id), user_id, "task_created", format!("created {title}"))
        .await.map_err(|cause| internal_error(cause, "Could not record task activity"))?;
    tx.commit().await.map_err(|cause| internal_error(cause, "Could not save task"))?;
    state.publish_workspace_event(workspace_id, Some(board_id), "task_created", format!("created {title}"));
    Ok(StatusCode::CREATED)
}

async fn update_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(task_id): Path<Uuid>,
    Json(input): Json<TaskInput>,
) -> ApiResult<StatusCode> {
    let title = validate_task(&input)?;
    let priority = input.priority.as_deref().unwrap_or("medium");
    let description = input.description.as_deref().filter(|value| !value.trim().is_empty());
    let due_at: Option<DateTime<Utc>> = input.due_date.map(|date| date.and_hms_opt(0, 0, 0).unwrap().and_utc());

    let mut tx = state.db.begin().await.map_err(|cause| internal_error(cause, "Could not begin task update"))?;
    let existing: Option<(Uuid, Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT b.workspace_id, b.id, t.list_id, t.title FROM tasks t          JOIN lists l ON l.id = t.list_id JOIN boards b ON b.id = l.board_id          JOIN workspace_members wm ON wm.workspace_id = b.workspace_id          WHERE t.id = $1 AND wm.user_id = $2",
    )
    .bind(task_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not load task"))?;
    let Some((workspace_id, board_id, old_list_id, old_title)) = existing else {
        return Err(error(StatusCode::NOT_FOUND, "Task not found"));
    };

    let target_is_valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM lists l JOIN boards b ON b.id = l.board_id
         WHERE l.id = $1 AND b.workspace_id = $2 AND b.id = $3)",
    )
    .bind(input.list_id)
    .bind(workspace_id)
    .bind(board_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not verify destination list"))?;
    if !target_is_valid {
        return Err(error(StatusCode::BAD_REQUEST, "Destination list is not in this workspace"));
    }

    if let Some(assignee_id) = input.assignee_id {
        let is_member = ensure_assignee(&mut tx, workspace_id, assignee_id)
            .await.map_err(|cause| internal_error(cause, "Could not verify assignee"))?;
        if !is_member {
            return Err(error(StatusCode::BAD_REQUEST, "Assignee must be a workspace member"));
        }
    }

    if old_list_id != input.list_id {
        let position: i32 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM tasks WHERE list_id = $1",
        )
        .bind(input.list_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not find task position"))?;

        sqlx::query("UPDATE tasks SET list_id = $1, position = $2 WHERE id = $3")
            .bind(input.list_id)
            .bind(position)
            .bind(task_id)
            .execute(&mut *tx)
            .await
            .map_err(|cause| internal_error(cause, "Could not move task"))?;

        let source_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM tasks WHERE list_id = $1 ORDER BY position",
        )
        .bind(old_list_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not reorder source list"))?;
        for (index, id) in source_ids.iter().enumerate() {
            sqlx::query("UPDATE tasks SET position = $1 WHERE id = $2")
                .bind((index + 1) as i32)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|cause| internal_error(cause, "Could not normalize source positions"))?;
        }
    }

    sqlx::query(
        "UPDATE tasks SET title = $1, description = $2, priority = $3, due_at = $4, updated_at = NOW() WHERE id = $5",
    )
    .bind(title)
    .bind(description)
    .bind(priority)
    .bind(due_at)
    .bind(task_id)
    .execute(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not save task details"))?;

    sqlx::query("DELETE FROM task_assignees WHERE task_id = $1")
        .bind(task_id)
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not update task assignees"))?;
    if let Some(assignee_id) = input.assignee_id {
        sqlx::query("INSERT INTO task_assignees (task_id, user_id) VALUES ($1, $2)")
            .bind(task_id)
            .bind(assignee_id)
            .execute(&mut *tx)
            .await
            .map_err(|cause| internal_error(cause, "Could not assign task"))?;
    }

    let moved = old_list_id != input.list_id;
    let description = if moved {
        format!("moved {old_title} to another list")
    } else {
        format!("updated {title}")
    };
    add_activity(&mut tx, workspace_id, board_id, Some(task_id), user_id, if moved { "task_moved" } else { "task_updated" }, description)
        .await.map_err(|cause| internal_error(cause, "Could not record task activity"))?;
    tx.commit().await.map_err(|cause| internal_error(cause, "Could not save task update"))?;
    state.publish_workspace_event(
        workspace_id,
        Some(board_id),
        if moved { "task_moved" } else { "task_updated" },
        if moved { format!("moved {old_title} to another list") } else { format!("updated {title}") },
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.begin().await.map_err(|cause| internal_error(cause, "Could not begin task deletion"))?;
    let existing: Option<(Uuid, Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT b.workspace_id, b.id, t.list_id, t.title FROM tasks t          JOIN lists l ON l.id = t.list_id JOIN boards b ON b.id = l.board_id          JOIN workspace_members wm ON wm.workspace_id = b.workspace_id          WHERE t.id = $1 AND wm.user_id = $2",
    )
    .bind(task_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not load task before deletion"))?;
    let Some((workspace_id, board_id, _list_id, title)) = existing else {
        return Err(error(StatusCode::NOT_FOUND, "Task not found"));
    };

    sqlx::query("DELETE FROM tasks WHERE id = $1")
        .bind(task_id)
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not delete task"))?;
    add_activity(&mut tx, workspace_id, board_id, None, user_id, "task_deleted", format!("deleted {title}"))
        .await.map_err(|cause| internal_error(cause, "Could not record task deletion"))?;
    tx.commit().await.map_err(|cause| internal_error(cause, "Could not save task deletion"))?;
    state.publish_workspace_event(workspace_id, Some(board_id), "task_deleted", format!("deleted {title}"));
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct CommentInput {
    body: String,
}

async fn add_comment(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(task_id): Path<Uuid>,
    Json(input): Json<CommentInput>,
) -> ApiResult<StatusCode> {
    let body = input.body.trim();
    if body.is_empty() || body.chars().count() > 2000 {
        return Err(error(StatusCode::BAD_REQUEST, "Comment must be between 1 and 2000 characters"));
    }

    let mut tx = state.db.begin().await.map_err(|cause| internal_error(cause, "Could not begin comment"))?;
    let scope: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT b.workspace_id, b.id FROM tasks t JOIN lists l ON l.id = t.list_id          JOIN boards b ON b.id = l.board_id          JOIN workspace_members wm ON wm.workspace_id = b.workspace_id          WHERE t.id = $1 AND wm.user_id = $2",
    )
    .bind(task_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not verify comment access"))?;
    let Some((workspace_id, board_id)) = scope else {
        return Err(error(StatusCode::NOT_FOUND, "Task not found"));
    };

    sqlx::query("INSERT INTO comments (task_id, author_id, body) VALUES ($1, $2, $3)")
        .bind(task_id)
        .bind(user_id)
        .bind(body)
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not add comment"))?;
    add_activity(&mut tx, workspace_id, board_id, Some(task_id), user_id, "comment_added", "added a comment to a task".to_string())
        .await.map_err(|cause| internal_error(cause, "Could not record comment activity"))?;
    tx.commit().await.map_err(|cause| internal_error(cause, "Could not save comment"))?;
    state.publish_workspace_event(workspace_id, Some(board_id), "comment_added", "added a comment to a task");
    Ok(StatusCode::CREATED)
}

#[derive(Debug, Deserialize)]
struct MoveTaskInput {
    destination_list_id: Uuid,
    ordered_task_ids: Vec<Uuid>,
}

async fn move_task(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(task_id): Path<Uuid>,
    Json(input): Json<MoveTaskInput>,
) -> ApiResult<StatusCode> {
    if input.ordered_task_ids.is_empty() || input.ordered_task_ids.len() > 1000 {
        return Err(error(StatusCode::BAD_REQUEST, "Task order is invalid"));
    }

    let mut tx = state.db.begin().await.map_err(|cause| internal_error(cause, "Could not begin task move"))?;
    let source: Option<(Uuid, Uuid, Uuid)> = sqlx::query_as(
        "SELECT t.list_id, l.board_id, b.workspace_id FROM tasks t          JOIN lists l ON l.id = t.list_id JOIN boards b ON b.id = l.board_id          JOIN workspace_members wm ON wm.workspace_id = b.workspace_id          WHERE t.id = $1 AND wm.user_id = $2",
    )
    .bind(task_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not locate task to move"))?;
    let Some((source_list_id, board_id, workspace_id)) = source else {
        return Err(error(StatusCode::NOT_FOUND, "Task not found"));
    };

    let destination_valid: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM lists WHERE id = $1 AND board_id = $2)",
    )
    .bind(input.destination_list_id)
    .bind(board_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not verify destination list"))?;
    if !destination_valid {
        return Err(error(StatusCode::BAD_REQUEST, "Destination list is not on this board"));
    }

    let destination_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM tasks WHERE list_id = $1 AND id <> $2 ORDER BY position",
    )
    .bind(input.destination_list_id)
    .bind(task_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not read destination tasks"))?;
    let expected: HashSet<Uuid> = destination_ids.into_iter().chain(std::iter::once(task_id)).collect();
    let submitted: HashSet<Uuid> = input.ordered_task_ids.iter().copied().collect();
    if expected != submitted || submitted.len() != input.ordered_task_ids.len() {
        return Err(error(StatusCode::BAD_REQUEST, "Task order does not match the destination list"));
    }

    let source_ids: Vec<Uuid> = if source_list_id == input.destination_list_id {
        Vec::new()
    } else {
        sqlx::query_scalar("SELECT id FROM tasks WHERE list_id = $1 AND id <> $2 ORDER BY position")
            .bind(source_list_id)
            .bind(task_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|cause| internal_error(cause, "Could not read source tasks"))?
    };

    if source_list_id == input.destination_list_id {
        sqlx::query("UPDATE tasks SET position = -position - 1 WHERE list_id = $1")
            .bind(source_list_id)
            .execute(&mut *tx)
            .await
            .map_err(|cause| internal_error(cause, "Could not reserve task order"))?;
    } else {
        for list_id in [source_list_id, input.destination_list_id] {
            sqlx::query("UPDATE tasks SET position = -position - 1 WHERE list_id = $1")
                .bind(list_id)
                .execute(&mut *tx)
                .await
                .map_err(|cause| internal_error(cause, "Could not reserve task order"))?;
        }
    }

    for (index, id) in input.ordered_task_ids.iter().enumerate() {
        sqlx::query("UPDATE tasks SET list_id = $1, position = $2, updated_at = NOW() WHERE id = $3")
            .bind(input.destination_list_id)
            .bind((index + 1) as i32)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|cause| internal_error(cause, "Could not persist task order"))?;
    }

    for (index, id) in source_ids.iter().enumerate() {
        sqlx::query("UPDATE tasks SET position = $1 WHERE id = $2")
            .bind((index + 1) as i32)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|cause| internal_error(cause, "Could not normalize source task order"))?;
    }

    add_activity(&mut tx, workspace_id, board_id, Some(task_id), user_id, "task_moved", "moved a task to another list or position".to_string())
        .await.map_err(|cause| internal_error(cause, "Could not record task move"))?;
    tx.commit().await.map_err(|cause| internal_error(cause, "Could not save task order"))?;
    state.publish_workspace_event(workspace_id, Some(board_id), "task_moved", "moved a task to another list or position");
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/workspace", get(get_workspace))
        .route("/boards", post(create_board))
        .route("/tasks", post(create_task))
        .route("/tasks/{task_id}", patch(update_task).delete(delete_task))
        .route("/tasks/{task_id}/comments", post(add_comment))
        .route("/tasks/{task_id}/move", patch(move_task))
}
