use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Transaction};
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

#[derive(Debug, Deserialize)]
struct ListInput {
    name: String,
}

#[derive(Debug, Serialize, FromRow)]
struct ListCreated {
    id: Uuid,
}

async fn add_activity(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    board_id: Uuid,
    actor_id: Uuid,
    event_type: &'static str,
    description: String,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO activity_events (workspace_id, board_id, actor_id, event_type, payload)
         VALUES ($1, $2, $3, $4, jsonb_build_object('description', $5))",
    )
    .bind(workspace_id)
    .bind(board_id)
    .bind(actor_id)
    .bind(event_type)
    .bind(description)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn validate_name(name: &str) -> Result<&str, (StatusCode, Json<ErrorBody>)> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 {
        return Err(error(StatusCode::BAD_REQUEST, "Column name must be between 1 and 60 characters"));
    }
    Ok(name)
}

async fn create_list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(board_id): Path<Uuid>,
    Json(input): Json<ListInput>,
) -> ApiResult<(StatusCode, Json<ListCreated>)> {
    let name = validate_name(&input.name)?;
    let mut tx = state.db.begin().await.map_err(|cause| internal_error(cause, "Could not begin column creation"))?;

    let scope: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT b.workspace_id, wm.role FROM boards b
         JOIN workspace_members wm ON wm.workspace_id = b.workspace_id
         JOIN board_members bm ON bm.board_id=b.id AND bm.user_id=wm.user_id AND bm.role='admin'
         WHERE b.id = $1 AND wm.user_id = $2
         FOR UPDATE OF b",
    )
    .bind(board_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not verify project access"))?;

    let Some((workspace_id, role)) = scope else {
        return Err(error(StatusCode::NOT_FOUND, "Project not found"));
    };
    if role != "owner" && role != "admin" {
        return Err(error(StatusCode::FORBIDDEN, "Only workspace owners and admins can manage columns"));
    }

    let position: i32 = sqlx::query_scalar("SELECT COALESCE(MAX(position), 0) + 1 FROM lists WHERE board_id = $1")
        .bind(board_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not allocate column position"))?;

    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO lists (board_id, name, position) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(board_id)
    .bind(name)
    .bind(position)
    .fetch_one(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not create column"))?;

    add_activity(&mut tx, workspace_id, board_id, user_id, "list_created", format!("created column {name}"))
        .await
        .map_err(|cause| internal_error(cause, "Could not record column activity"))?;
    tx.commit().await.map_err(|cause| internal_error(cause, "Could not save column"))?;
    state.publish_workspace_event(workspace_id, Some(board_id), "list_created", format!("created column {name}"));

    Ok((StatusCode::CREATED, Json(ListCreated { id })))
}

async fn update_list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(list_id): Path<Uuid>,
    Json(input): Json<ListInput>,
) -> ApiResult<StatusCode> {
    let name = validate_name(&input.name)?;
    let mut tx = state.db.begin().await.map_err(|cause| internal_error(cause, "Could not begin column update"))?;
    let scope: Option<(Uuid, Uuid, String, String)> = sqlx::query_as(
        "SELECT b.workspace_id, b.id, wm.role, l.name FROM lists l
         JOIN boards b ON b.id = l.board_id
         JOIN workspace_members wm ON wm.workspace_id = b.workspace_id
         JOIN board_members bm ON bm.board_id=b.id AND bm.user_id=wm.user_id AND bm.role='admin'
         WHERE l.id = $1 AND wm.user_id = $2
         FOR UPDATE OF b, l",
    )
    .bind(list_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not load column"))?;
    let Some((workspace_id, board_id, role, old_name)) = scope else {
        return Err(error(StatusCode::NOT_FOUND, "Column not found"));
    };
    if role != "owner" && role != "admin" {
        return Err(error(StatusCode::FORBIDDEN, "Only workspace owners and admins can manage columns"));
    }

    sqlx::query("UPDATE lists SET name = $1, updated_at = NOW() WHERE id = $2")
        .bind(name)
        .bind(list_id)
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not rename column"))?;

    add_activity(&mut tx, workspace_id, board_id, user_id, "list_updated", format!("renamed column {old_name} to {name}"))
        .await
        .map_err(|cause| internal_error(cause, "Could not record column activity"))?;
    tx.commit().await.map_err(|cause| internal_error(cause, "Could not save column update"))?;
    state.publish_workspace_event(workspace_id, Some(board_id), "list_updated", format!("renamed column {old_name} to {name}"));
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(list_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.begin().await.map_err(|cause| internal_error(cause, "Could not begin column deletion"))?;
    let scope: Option<(Uuid, Uuid, String, String)> = sqlx::query_as(
        "SELECT b.workspace_id, b.id, wm.role, l.name FROM lists l
         JOIN boards b ON b.id = l.board_id
         JOIN workspace_members wm ON wm.workspace_id = b.workspace_id
         JOIN board_members bm ON bm.board_id=b.id AND bm.user_id=wm.user_id AND bm.role='admin'
         WHERE l.id = $1 AND wm.user_id = $2
         FOR UPDATE OF b, l",
    )
    .bind(list_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not load column before deletion"))?;
    let Some((workspace_id, board_id, role, name)) = scope else {
        return Err(error(StatusCode::NOT_FOUND, "Column not found"));
    };
    if role != "owner" && role != "admin" {
        return Err(error(StatusCode::FORBIDDEN, "Only workspace owners and admins can manage columns"));
    }

    let list_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM lists WHERE board_id = $1")
        .bind(board_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not count project columns"))?;
    if list_count <= 1 {
        return Err(error(StatusCode::CONFLICT, "A project must keep at least one column"));
    }

    let task_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE list_id = $1")
        .bind(list_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not check column tasks"))?;
    if task_count > 0 {
        return Err(error(StatusCode::CONFLICT, "Move or remove the tasks in this column before deleting it"));
    }

    // Preserve durable AI proposal history and its audit linkage even after
    // approved tasks are later removed from this column.
    let has_proposal_history: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ai_action_proposals WHERE target_list_id=$1)",
    )
    .bind(list_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|cause| internal_error(cause, "Could not check proposal history before column deletion"))?;
    if has_proposal_history {
        return Err(error(StatusCode::CONFLICT, "This column has AI proposal history and cannot be deleted"));
    }

    let ordered_ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM lists WHERE board_id = $1 AND id <> $2 ORDER BY position")
        .bind(board_id)
        .bind(list_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not read remaining columns"))?;

    sqlx::query("DELETE FROM lists WHERE id = $1")
        .bind(list_id)
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not delete column"))?;

    // Move all positions to a temporary negative range before assigning dense positions.
    sqlx::query("UPDATE lists SET position = -position - 1 WHERE board_id = $1")
        .bind(board_id)
        .execute(&mut *tx)
        .await
        .map_err(|cause| internal_error(cause, "Could not reserve column positions"))?;
    for (index, id) in ordered_ids.iter().enumerate() {
        sqlx::query("UPDATE lists SET position = $1 WHERE id = $2")
            .bind((index + 1) as i32)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|cause| internal_error(cause, "Could not normalize column positions"))?;
    }

    add_activity(&mut tx, workspace_id, board_id, user_id, "list_deleted", format!("deleted column {name}"))
        .await
        .map_err(|cause| internal_error(cause, "Could not record column activity"))?;
    tx.commit().await.map_err(|cause| internal_error(cause, "Could not save column deletion"))?;
    state.publish_workspace_event(workspace_id, Some(board_id), "list_deleted", format!("deleted column {name}"));
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_name_is_trimmed_and_limited() {
        assert_eq!(validate_name("  In progress  ").unwrap(), "In progress");
        assert!(validate_name("   ").is_err());
        assert!(validate_name(&"x".repeat(61)).is_err());
        assert!(validate_name(&"x".repeat(60)).is_ok());
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/boards/{board_id}/lists", post(create_list))
        .route("/lists/{list_id}", patch(update_list).delete(delete_list))
}
