use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, patch, post, put},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use tokio::time::{interval, Duration as TokioDuration};
use uuid::Uuid;

use crate::{auth::{AuthUser, WorkspaceUser}, state::AppState};

#[derive(Debug, Serialize)]
struct ErrorBody { error: String }
type ApiResult<T> = Result<T, (StatusCode, Json<ErrorBody>)>;

fn fail(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<ErrorBody>) {
    (status, Json(ErrorBody { error: message.into() }))
}
fn internal<E: std::fmt::Display>(error: E, context: &'static str) -> (StatusCode, Json<ErrorBody>) {
    tracing::error!(%error, "{context}");
    fail(StatusCode::INTERNAL_SERVER_ERROR, "The request could not be completed")
}

async fn task_scope(state: &AppState, user_id: Uuid, task_id: Uuid) -> ApiResult<(Uuid, Uuid, Uuid)> {
    sqlx::query_as(
        "SELECT b.workspace_id, b.id, t.list_id
         FROM tasks t JOIN lists l ON l.id=t.list_id JOIN boards b ON b.id=l.board_id
         JOIN workspace_members wm ON wm.workspace_id=b.workspace_id AND wm.user_id=$2
         LEFT JOIN board_members bm ON bm.board_id=b.id AND bm.user_id=wm.user_id
         WHERE t.id=$1
           AND (wm.role IN ('owner','admin') OR bm.user_id IS NOT NULL)",
    )
    .bind(task_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| internal(error, "Could not verify task access"))?
    .ok_or_else(|| fail(StatusCode::NOT_FOUND, "Task not found"))
}

async fn task_write_scope(state: &AppState, user_id: Uuid, task_id: Uuid) -> ApiResult<(Uuid, Uuid, Uuid)> {
    let scope = task_scope(state, user_id, task_id).await?;
    let role = crate::auth::board_role(&state.db, user_id, scope.0, scope.1)
        .await
        .map_err(|error| internal(error, "Could not verify board write permissions"))?;
    if crate::auth::board_role_allows(role.as_deref(), true) {
        Ok(scope)
    } else if role.as_deref() == Some("viewer") {
        Err(fail(StatusCode::FORBIDDEN, "This project is read-only for your account"))
    } else {
        Err(fail(StatusCode::NOT_FOUND, "Task not found"))
    }
}

async fn task_in_workspace(
    state: &AppState, user_id: Uuid, task_id: Uuid, workspace_id: Uuid,
) -> ApiResult<()> {
    let (_, board_id, _) = task_scope(state, user_id, task_id).await
        .map_err(|_| fail(StatusCode::BAD_REQUEST, "The selected task is outside an accessible project"))?;
    let role = crate::auth::board_role(&state.db, user_id, workspace_id, board_id)
        .await
        .map_err(|error| internal(error, "Could not validate dependency board permissions"))?;
    if crate::auth::board_role_allows(role.as_deref(), true) {
        Ok(())
    } else {
        Err(fail(StatusCode::BAD_REQUEST, "The selected task is outside a writable project"))
    }
}

#[derive(Debug, Serialize, FromRow)]
struct SubtaskRow {
    id: Uuid,
    parent_task_id: Uuid,
    title: String,
    description: String,
    priority: String,
    due_date: Option<String>,
    estimated_minutes: Option<i32>,
    list_name: String,
    is_done: bool,
}

#[derive(Debug, Serialize, FromRow)]
struct ChecklistItem {
    id: Uuid,
    task_id: Uuid,
    title: String,
    position: i32,
    is_done: bool,
    completed_by: Option<Uuid>,
    completed_at: Option<chrono::DateTime<Utc>>,
    created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
struct TaskLabel {
    id: Uuid,
    workspace_id: Uuid,
    name: String,
    color: String,
}

#[derive(Debug, Serialize, FromRow)]
struct DependencyRow {
    task_id: Uuid,
    depends_on_task_id: Uuid,
    depends_on_title: String,
    depends_on_status: String,
}

#[derive(Debug, Serialize, FromRow)]
struct RecurrenceRule {
    id: Uuid,
    template_task_id: Uuid,
    target_list_id: Uuid,
    interval_days: i32,
    next_run_at: chrono::DateTime<Utc>,
    time_zone: String,
    is_active: bool,
}

async fn advanced_details(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let (workspace_id, _board_id, _list_id) = task_scope(&state, user_id, task_id).await?;
    let subtasks = sqlx::query_as::<_, SubtaskRow>(
        "SELECT t.id, t.parent_task_id, t.title, COALESCE(t.description,'') AS description,
           t.priority, CASE WHEN t.due_at IS NULL THEN NULL ELSE to_char(t.due_at AT TIME ZONE 'UTC','YYYY-MM-DD') END AS due_date,
           t.estimated_minutes, l.name AS list_name,
           (lower(l.name) IN ('done','complete','completed')) AS is_done
         FROM tasks t JOIN lists l ON l.id=t.list_id
         WHERE t.parent_task_id=$1 ORDER BY t.position",
    )
    .bind(task_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load subtasks"))?;

    let checklist = sqlx::query_as::<_, ChecklistItem>(
        "SELECT id, task_id, title, position, is_done, completed_by, completed_at, created_at
         FROM task_checklist_items WHERE task_id=$1 ORDER BY position",
    )
    .bind(task_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load task checklist"))?;

    let labels = sqlx::query_as::<_, TaskLabel>(
        "SELECT l.id, l.workspace_id, l.name, l.color
         FROM labels l JOIN task_labels tl ON tl.label_id=l.id
         WHERE tl.task_id=$1 AND l.workspace_id=$2 ORDER BY l.name",
    )
    .bind(task_id)
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load task labels"))?;

    let dependencies = sqlx::query_as::<_, DependencyRow>(
        "SELECT td.task_id, td.depends_on_task_id, dep.title AS depends_on_title, dep_list.name AS depends_on_status
         FROM task_dependencies td
         JOIN tasks dep ON dep.id=td.depends_on_task_id
         JOIN lists dep_list ON dep_list.id=dep.list_id
         WHERE td.task_id=$1 ORDER BY dep.title",
    )
    .bind(task_id)
    .fetch_all(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load task dependencies"))?;

    let recurrence = sqlx::query_as::<_, RecurrenceRule>(
        "SELECT id, template_task_id, target_list_id, interval_days, next_run_at, time_zone, is_active
         FROM task_recurrence_rules WHERE template_task_id=$1",
    )
    .bind(task_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|error| internal(error, "Could not load recurrence rule"))?;

    Ok(Json(json!({ "subtasks": subtasks, "checklist": checklist, "labels": labels, "dependencies": dependencies, "recurrence": recurrence })))
}

#[derive(Debug, Deserialize)]
struct SubtaskInput {
    title: String,
    description: Option<String>,
    due_date: Option<chrono::NaiveDate>,
    estimated_minutes: Option<i32>,
}

async fn subtask_create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(parent_task_id): Path<Uuid>,
    Json(input): Json<SubtaskInput>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let (workspace_id, board_id, list_id) = task_write_scope(&state, user_id, parent_task_id).await?;
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > 180 {
        return Err(fail(StatusCode::BAD_REQUEST, "Subtask title must be between 1 and 180 characters"));
    }
    if input.estimated_minutes.is_some_and(|minutes| minutes < 1 || minutes > 100000) {
        return Err(fail(StatusCode::BAD_REQUEST, "Estimated time must be between 1 and 100000 minutes"));
    }
    if input.description.as_ref().is_some_and(|description| description.chars().count() > 4000) {
        return Err(fail(StatusCode::BAD_REQUEST, "Subtask description is too long"));
    }
    let due_at = input.due_date.map(|date| date.and_hms_opt(0,0,0).unwrap().and_utc());
    let mut tx = state.db.begin().await.map_err(|error| internal(error, "Could not begin subtask creation"))?;
    let position: i32 = sqlx::query_scalar("SELECT COALESCE(MAX(position),0)+1 FROM tasks WHERE list_id=$1")
        .bind(list_id).fetch_one(&mut *tx).await
        .map_err(|error| internal(error, "Could not get subtask position"))?;
    let subtask_id: Uuid = sqlx::query_scalar(
        "INSERT INTO tasks (list_id,title,description,priority,due_at,position,created_by,parent_task_id,estimated_minutes)
         VALUES ($1,$2,$3,'medium',$4,$5,$6,$7,$8) RETURNING id",
    )
    .bind(list_id).bind(title)
    .bind(input.description.as_deref())
    .bind(due_at).bind(position).bind(user_id).bind(parent_task_id).bind(input.estimated_minutes)
    .fetch_one(&mut *tx).await
    .map_err(|error| internal(error, "Could not create subtask"))?;
    sqlx::query(
        "INSERT INTO activity_events (workspace_id,board_id,task_id,actor_id,event_type,payload)
         VALUES ($1,$2,$3,$4,'subtask_created',jsonb_build_object('description',$5))",
    )
    .bind(workspace_id).bind(board_id).bind(subtask_id).bind(user_id).bind(format!("created subtask {title}"))
    .execute(&mut *tx).await.map_err(|error| internal(error, "Could not record subtask activity"))?;
    tx.commit().await.map_err(|error| internal(error, "Could not save subtask"))?;
    state.publish_workspace_event(workspace_id, Some(board_id), "subtask_created", format!("created subtask {title}"));
    Ok((StatusCode::CREATED, Json(json!({ "id": subtask_id, "title": title, "parent_task_id": parent_task_id }))))
}

#[derive(Debug, Deserialize)]
struct SubtaskUpdate {
    title: Option<String>,
    is_done: Option<bool>,
}

async fn subtask_update(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(subtask_id): Path<Uuid>,
    Json(input): Json<SubtaskUpdate>,
) -> ApiResult<StatusCode> {
    let (workspace_id, board_id, _) = task_write_scope(&state, user_id, subtask_id).await?;
    let is_child: bool = sqlx::query_scalar("SELECT parent_task_id IS NOT NULL FROM tasks WHERE id=$1")
        .bind(subtask_id).fetch_one(&state.db).await
        .map_err(|error| internal(error, "Could not verify subtask"))?;
    if !is_child { return Err(fail(StatusCode::BAD_REQUEST, "This task is not a subtask")); }
    let title = input.title.as_deref().map(str::trim);
    if title.is_some_and(|value| value.is_empty() || value.chars().count() > 180) {
        return Err(fail(StatusCode::BAD_REQUEST, "Subtask title must be between 1 and 180 characters"));
    }
    let mut target_list: Option<Uuid> = None;
    if let Some(is_done) = input.is_done {
        let desired: Vec<&str> = if is_done { vec!["done", "complete", "completed"] } else { vec!["to do", "todo", "backlog"] };
        target_list = sqlx::query_scalar(
            "SELECT id FROM lists WHERE board_id=$1 AND lower(name)=ANY($2) ORDER BY position LIMIT 1",
        )
        .bind(board_id)
        .bind(&desired)
        .fetch_optional(&state.db)
        .await
        .map_err(|error| internal(error, "Could not find target subtask status"))?;
        if target_list.is_none() {
            return Err(fail(StatusCode::BAD_REQUEST, "Add a matching To do or Done column before changing subtask status"));
        }
    }
    let mut tx = state.db.begin().await.map_err(|error| internal(error, "Could not begin subtask update"))?;
    if let Some(destination) = target_list {
        let position: i32 = sqlx::query_scalar("SELECT COALESCE(MAX(position),0)+1 FROM tasks WHERE list_id=$1")
            .bind(destination).fetch_one(&mut *tx).await
            .map_err(|error| internal(error, "Could not determine subtask position"))?;
        sqlx::query("UPDATE tasks SET list_id=$1,position=$2 WHERE id=$3")
            .bind(destination).bind(position).bind(subtask_id).execute(&mut *tx).await
            .map_err(|error| internal(error, "Could not update subtask status"))?;
    }
    sqlx::query("UPDATE tasks SET title=COALESCE($1,title), updated_at=NOW() WHERE id=$2")
        .bind(title).bind(subtask_id).execute(&mut *tx).await
        .map_err(|error| internal(error, "Could not update subtask"))?;
    tx.commit().await.map_err(|error| internal(error, "Could not save subtask"))?;
    state.publish_workspace_event(workspace_id, Some(board_id), "subtask_updated", "updated a subtask");
    Ok(StatusCode::NO_CONTENT)
}

async fn subtask_delete(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(subtask_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let (workspace_id, board_id, _) = task_write_scope(&state, user_id, subtask_id).await?;
    let is_child: bool = sqlx::query_scalar("SELECT parent_task_id IS NOT NULL FROM tasks WHERE id=$1")
        .bind(subtask_id).fetch_one(&state.db).await
        .map_err(|error| internal(error, "Could not verify subtask"))?;
    if !is_child { return Err(fail(StatusCode::BAD_REQUEST, "This task is not a subtask")); }
    sqlx::query("DELETE FROM tasks WHERE id=$1").bind(subtask_id).execute(&state.db).await
        .map_err(|error| internal(error, "Could not delete subtask"))?;
    state.publish_workspace_event(workspace_id, Some(board_id), "subtask_deleted", "deleted a subtask");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct ChecklistInput { title: String }

async fn checklist_list(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(task_id): Path<Uuid>,
) -> ApiResult<Json<Vec<ChecklistItem>>> {
    task_scope(&state, user_id, task_id).await?;
    let rows = sqlx::query_as::<_, ChecklistItem>(
        "SELECT id,task_id,title,position,is_done,completed_by,completed_at,created_at
         FROM task_checklist_items WHERE task_id=$1 ORDER BY position",
    )
    .bind(task_id).fetch_all(&state.db).await
    .map_err(|error| internal(error, "Could not load checklist"))?;
    Ok(Json(rows))
}

async fn checklist_create(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(task_id): Path<Uuid>,
    Json(input): Json<ChecklistInput>,
) -> ApiResult<(StatusCode, Json<ChecklistItem>)> {
    let (workspace_id, board_id, _) = task_write_scope(&state, user_id, task_id).await?;
    let title = input.title.trim();
    if title.is_empty() || title.chars().count()>240 {
        return Err(fail(StatusCode::BAD_REQUEST,"Checklist item must be between 1 and 240 characters"));
    }
    let mut tx=state.db.begin().await.map_err(|error| internal(error,"Could not begin checklist update"))?;
    sqlx::query("SELECT id FROM tasks WHERE id=$1 FOR UPDATE")
        .bind(task_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| internal(error,"Could not lock parent task for checklist update"))?;
    let position:i32=sqlx::query_scalar("SELECT COALESCE(MAX(position),0)+1 FROM task_checklist_items WHERE task_id=$1")
        .bind(task_id).fetch_one(&mut *tx).await.map_err(|error| internal(error,"Could not determine checklist position"))?;
    let row=sqlx::query_as::<_,ChecklistItem>(
        "INSERT INTO task_checklist_items(task_id,title,position) VALUES($1,$2,$3)
         RETURNING id,task_id,title,position,is_done,completed_by,completed_at,created_at")
        .bind(task_id).bind(title).bind(position).fetch_one(&mut *tx).await
        .map_err(|error| internal(error,"Could not create checklist item"))?;
    tx.commit().await.map_err(|error| internal(error,"Could not save checklist item"))?;
    state.publish_workspace_event(workspace_id,Some(board_id),"checklist_item_created","added a checklist item");
    Ok((StatusCode::CREATED,Json(row)))
}

#[derive(Debug, Deserialize)]
struct ChecklistUpdate { title: Option<String>, is_done: Option<bool> }

async fn checklist_update(
    State(state):State<AppState>,
    AuthUser(user_id):AuthUser,
    Path(item_id):Path<Uuid>,
    Json(input):Json<ChecklistUpdate>,
)->ApiResult<StatusCode>{
    let title=input.title.as_deref().map(str::trim);
    if title.is_some_and(|value|value.is_empty()||value.chars().count()>240){
        return Err(fail(StatusCode::BAD_REQUEST,"Checklist item title is invalid"));
    }
    let scope:Option<(Uuid,Uuid)>=sqlx::query_as(
        "UPDATE task_checklist_items c
            SET title=COALESCE($1,title),
                is_done=COALESCE($2,is_done),
                completed_by=CASE WHEN $2 IS TRUE THEN $3 WHEN $2 IS FALSE THEN NULL ELSE completed_by END,
                completed_at=CASE WHEN $2 IS TRUE THEN NOW() WHEN $2 IS FALSE THEN NULL ELSE completed_at END
           FROM tasks t
           JOIN lists l ON l.id=t.list_id
           JOIN boards b ON b.id=l.board_id
           JOIN workspace_members wm ON wm.workspace_id=b.workspace_id AND wm.user_id=$3
          WHERE c.id=$4 AND c.task_id=t.id
            AND (
                wm.role IN ('owner','admin')
                OR EXISTS (
                    SELECT 1 FROM board_members bm
                     WHERE bm.board_id=b.id AND bm.user_id=$3 AND bm.role IN ('admin','member')
                )
            )
          RETURNING b.workspace_id,b.id")
        .bind(title).bind(input.is_done).bind(user_id).bind(item_id)
        .fetch_optional(&state.db).await
        .map_err(|error|internal(error,"Could not update checklist item"))?;
    let Some((workspace_id,board_id))=scope else{return Err(fail(StatusCode::NOT_FOUND,"Checklist item not found"));};
    state.publish_workspace_event(workspace_id,Some(board_id),"checklist_item_updated","updated a checklist item");
    Ok(StatusCode::NO_CONTENT)
}

async fn checklist_delete(
    State(state):State<AppState>,AuthUser(user_id):AuthUser,Path(item_id):Path<Uuid>,
)->ApiResult<StatusCode>{
    let scope:Option<(Uuid,Uuid)>=sqlx::query_as(
        "DELETE FROM task_checklist_items c
          USING tasks t
          JOIN lists l ON l.id=t.list_id
          JOIN boards b ON b.id=l.board_id
          JOIN workspace_members wm ON wm.workspace_id=b.workspace_id AND wm.user_id=$2
         WHERE c.id=$1 AND c.task_id=t.id
           AND (
                wm.role IN ('owner','admin')
                OR EXISTS (
                    SELECT 1 FROM board_members bm
                     WHERE bm.board_id=b.id AND bm.user_id=$2 AND bm.role IN ('admin','member')
                )
           )
         RETURNING b.workspace_id,b.id")
        .bind(item_id).bind(user_id).fetch_optional(&state.db).await
        .map_err(|error|internal(error,"Could not delete checklist item"))?;
    let Some((workspace_id,board_id))=scope else{return Err(fail(StatusCode::NOT_FOUND,"Checklist item not found"));};
    state.publish_workspace_event(workspace_id,Some(board_id),"checklist_item_deleted","deleted a checklist item");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug,Deserialize)]
struct LabelInput { name:String, color:Option<String> }

async fn labels_list(
    State(state):State<AppState>,WorkspaceUser(user_id,workspace_id):WorkspaceUser,
)->ApiResult<Json<Vec<TaskLabel>>>{
    let rows=sqlx::query_as::<_,TaskLabel>("SELECT id,workspace_id,name,color FROM labels WHERE workspace_id=$1 ORDER BY name")
        .bind(workspace_id).fetch_all(&state.db).await.map_err(|error|internal(error,"Could not load labels"))?;
    Ok(Json(rows))
}

async fn label_create(
    State(state):State<AppState>,WorkspaceUser(user_id,workspace_id):WorkspaceUser,Json(input):Json<LabelInput>,
)->ApiResult<(StatusCode,Json<TaskLabel>)>{
    let name=input.name.trim();
    let color=input.color.as_deref().unwrap_or("#7771d9");
    if name.is_empty()||name.chars().count()>50||!color.starts_with('#')||color.len()!=7||
       !color[1..].chars().all(|c|c.is_ascii_hexdigit()){
        return Err(fail(StatusCode::BAD_REQUEST,"Label name or color is invalid"));
    }
    let row=sqlx::query_as::<_,TaskLabel>(
        "INSERT INTO labels(workspace_id,name,color,created_by) VALUES($1,$2,$3,$4)
         RETURNING id,workspace_id,name,color")
        .bind(workspace_id).bind(name).bind(color).bind(user_id).fetch_one(&state.db).await
        .map_err(|error|{
            if let sqlx::Error::Database(db)=&error{
                if db.code().as_deref()==Some("23505"){return fail(StatusCode::CONFLICT,"A label with this name already exists");}
            }
            internal(error,"Could not create label")
        })?;
    Ok((StatusCode::CREATED,Json(row)))
}

async fn task_label_add(
    State(state):State<AppState>,AuthUser(user_id):AuthUser,
    Path((task_id,label_id)):Path<(Uuid,Uuid)>,
)->ApiResult<StatusCode>{
    let (workspace_id,board_id,_)=task_write_scope(&state,user_id,task_id).await?;
    let label_valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM labels WHERE id=$1 AND workspace_id=$2)")
        .bind(label_id).bind(workspace_id).fetch_one(&state.db).await.map_err(|error|internal(error,"Could not validate label"))?;
    if !label_valid{return Err(fail(StatusCode::BAD_REQUEST,"Label is outside this workspace"));}
    sqlx::query("INSERT INTO task_labels(task_id,label_id) VALUES($1,$2) ON CONFLICT DO NOTHING")
        .bind(task_id).bind(label_id).execute(&state.db).await.map_err(|error|internal(error,"Could not apply label"))?;
    state.publish_workspace_event(workspace_id,Some(board_id),"task_label_added","added a task label");
    Ok(StatusCode::NO_CONTENT)
}

async fn task_label_remove(
    State(state):State<AppState>,AuthUser(user_id):AuthUser,
    Path((task_id,label_id)):Path<(Uuid,Uuid)>,
)->ApiResult<StatusCode>{
    let (workspace_id,board_id,_)=task_write_scope(&state,user_id,task_id).await?;
    sqlx::query("DELETE FROM task_labels WHERE task_id=$1 AND label_id=$2")
        .bind(task_id).bind(label_id).execute(&state.db).await.map_err(|error|internal(error,"Could not remove label"))?;
    state.publish_workspace_event(workspace_id,Some(board_id),"task_label_removed","removed a task label");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug,Deserialize)]
struct DependencyInput { depends_on_task_id:Uuid }

async fn dependency_add(
    State(state): State<AppState>,
    AuthUser(user_id): AuthUser,
    Path(task_id): Path<Uuid>,
    Json(input): Json<DependencyInput>,
) -> ApiResult<StatusCode> {
    let (workspace_id, board_id, _) = task_write_scope(&state, user_id, task_id).await?;
    task_in_workspace(&state, user_id, input.depends_on_task_id, workspace_id).await?;
    if task_id == input.depends_on_task_id {
        return Err(fail(StatusCode::BAD_REQUEST, "A task cannot depend on itself"));
    }

    let mut transaction = state.db.begin().await
        .map_err(|error| internal(error, "Could not begin dependency update"))?;
    sqlx::query("SELECT pg_advisory_xact_lock(7712345)")
        .execute(&mut *transaction)
        .await
        .map_err(|error| internal(error, "Could not lock dependency graph"))?;

    let same_board: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1
              FROM tasks a
              JOIN lists la ON la.id=a.list_id
              JOIN boards ba ON ba.id=la.board_id
              JOIN tasks b ON b.id=$2
              JOIN lists lb ON lb.id=b.list_id
              JOIN boards bb ON bb.id=lb.board_id
             WHERE a.id=$1 AND ba.id=bb.id
         )",
    )
    .bind(task_id)
    .bind(input.depends_on_task_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|error| internal(error, "Could not compare dependency projects"))?;
    if !same_board {
        return Err(fail(StatusCode::BAD_REQUEST, "Dependencies must be within the same project"));
    }

    let creates_cycle: bool = sqlx::query_scalar(
        "WITH RECURSIVE dependency_path(task_id) AS (
            SELECT depends_on_task_id FROM task_dependencies WHERE task_id=$1
            UNION
            SELECT td.depends_on_task_id
              FROM task_dependencies td
              JOIN dependency_path dp ON td.task_id=dp.task_id
         )
         SELECT EXISTS(SELECT 1 FROM dependency_path WHERE task_id=$2)",
    )
    .bind(input.depends_on_task_id)
    .bind(task_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|error| internal(error, "Could not validate dependency graph"))?;
    if creates_cycle {
        return Err(fail(StatusCode::CONFLICT, "This dependency would create a cycle"));
    }

    sqlx::query(
        "INSERT INTO task_dependencies(task_id,depends_on_task_id,created_by)
         VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
    )
    .bind(task_id)
    .bind(input.depends_on_task_id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await
    .map_err(|error| internal(error, "Could not add task dependency"))?;

    transaction.commit().await
        .map_err(|error| internal(error, "Could not save task dependency"))?;
    state.publish_workspace_event(workspace_id, Some(board_id), "task_dependency_added", "added a task dependency");
    Ok(StatusCode::CREATED)
}

async fn dependency_remove(
    State(state):State<AppState>,AuthUser(user_id):AuthUser,
    Path((task_id,depends_on_task_id)):Path<(Uuid,Uuid)>,
)->ApiResult<StatusCode>{
    let (workspace_id,board_id,_)=task_write_scope(&state,user_id,task_id).await?;
    sqlx::query("DELETE FROM task_dependencies WHERE task_id=$1 AND depends_on_task_id=$2")
        .bind(task_id).bind(depends_on_task_id).execute(&state.db).await
        .map_err(|error|internal(error,"Could not remove task dependency"))?;
    state.publish_workspace_event(workspace_id,Some(board_id),"task_dependency_removed","removed a task dependency");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug,Deserialize)]
struct EstimateInput { estimated_minutes:Option<i32> }

async fn estimate_update(
    State(state):State<AppState>,AuthUser(user_id):AuthUser,
    Path(task_id):Path<Uuid>,Json(input):Json<EstimateInput>,
)->ApiResult<StatusCode>{
    let (workspace_id,board_id,_)=task_write_scope(&state,user_id,task_id).await?;
    if input.estimated_minutes.is_some_and(|value|value<1||value>100000){
        return Err(fail(StatusCode::BAD_REQUEST,"Estimated time must be between 1 and 100000 minutes"));
    }
    sqlx::query("UPDATE tasks SET estimated_minutes=$1,updated_at=NOW() WHERE id=$2")
        .bind(input.estimated_minutes).bind(task_id).execute(&state.db).await
        .map_err(|error|internal(error,"Could not update task estimate"))?;
    state.publish_workspace_event(workspace_id,Some(board_id),"task_estimate_updated","updated a task estimate");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug,Deserialize)]
struct RecurrenceInput {
    interval_days:Option<i32>,
    target_list_id:Option<Uuid>,
    time_zone:Option<String>,
}

async fn recurrence_update(
    State(state):State<AppState>,AuthUser(user_id):AuthUser,
    Path(task_id):Path<Uuid>,Json(input):Json<RecurrenceInput>,
)->ApiResult<StatusCode>{
    let (workspace_id,board_id,_)=task_write_scope(&state,user_id,task_id).await?;
    match input.interval_days {
        None=>{
            sqlx::query("DELETE FROM task_recurrence_rules WHERE template_task_id=$1").bind(task_id).execute(&state.db).await
                .map_err(|error|internal(error,"Could not remove recurrence rule"))?;
        }
        Some(days)=>{
            if !(1..=365).contains(&days){return Err(fail(StatusCode::BAD_REQUEST,"Repeat interval must be between 1 and 365 days"));}
            let target_list=input.target_list_id.ok_or_else(||fail(StatusCode::BAD_REQUEST,"Choose a destination list for repeated tasks"))?;
            let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM lists l JOIN boards b ON b.id=l.board_id WHERE l.id=$1 AND b.workspace_id=$2 AND b.id=$3)")
                .bind(target_list).bind(workspace_id).bind(board_id).fetch_one(&state.db).await
                .map_err(|error|internal(error,"Could not validate recurrence destination"))?;
            if !valid{return Err(fail(StatusCode::BAD_REQUEST,"Repeat destination must be in the same project"));}
            let zone=input.time_zone.as_deref().unwrap_or("UTC");
            if zone.trim().is_empty()||zone.len()>80{return Err(fail(StatusCode::BAD_REQUEST,"Time zone is invalid"));}
            sqlx::query(
                "INSERT INTO task_recurrence_rules(template_task_id,target_list_id,interval_days,next_run_at,time_zone,created_by)
                 VALUES($1,$2,$3,NOW()+make_interval(days=>$3),$4,$5)
                 ON CONFLICT(template_task_id) DO UPDATE SET target_list_id=EXCLUDED.target_list_id,
                   interval_days=EXCLUDED.interval_days,next_run_at=NOW()+make_interval(days=>EXCLUDED.interval_days),
                   time_zone=EXCLUDED.time_zone,is_active=TRUE,updated_at=NOW()")
                .bind(task_id).bind(target_list).bind(days).bind(zone).bind(user_id).execute(&state.db).await
                .map_err(|error|internal(error,"Could not save recurrence rule"))?;
        }
    }
    state.publish_workspace_event(workspace_id,Some(board_id),"task_recurrence_updated","updated a recurring task rule");
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug,FromRow)]
struct RecurrenceJob {
    id:Uuid,
    template_task_id:Uuid,
    target_list_id:Uuid,
    interval_days:i32,
    workspace_id:Uuid,
    board_id:Uuid,
    scheduled_for:chrono::DateTime<Utc>,
}

async fn process_recurrences(state:&AppState){
    let mut tx=match state.db.begin().await {
        Ok(tx)=>tx,
        Err(error)=>{tracing::error!(%error,"Could not begin recurrence job");return;}
    };
    let jobs=match sqlx::query_as::<_,RecurrenceJob>(
        "SELECT r.id,r.template_task_id,r.target_list_id,r.interval_days,
                b.workspace_id,b.id AS board_id,r.next_run_at AS scheduled_for
         FROM task_recurrence_rules r
         JOIN tasks t ON t.id=r.template_task_id
         JOIN lists status_list ON status_list.id=t.list_id
         JOIN lists target_list ON target_list.id=r.target_list_id
         JOIN boards b ON b.id=target_list.board_id
         WHERE r.is_active AND r.next_run_at<=NOW()
           AND lower(status_list.name) IN ('done','complete','completed')
         ORDER BY r.next_run_at
         LIMIT 20 FOR UPDATE OF r SKIP LOCKED")
        .fetch_all(&mut *tx).await {
            Ok(rows)=>rows,
            Err(error)=>{tracing::error!(%error,"Could not load due recurrence rules");return;}
        };
    let mut committed_events: Vec<(Uuid, Uuid, Uuid)> = Vec::new();

    for job in jobs {
        let already_generated:bool=match sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM task_recurrence_runs WHERE recurrence_rule_id=$1 AND scheduled_for=$2)",
        ).bind(job.id).bind(job.scheduled_for).fetch_one(&mut *tx).await {
            Ok(value)=>value,
            Err(error)=>{
                tracing::error!(%error,rule_id=%job.id,"Could not check recurrence idempotency ledger");
                let _=tx.rollback().await;
                return;
            }
        };
        if already_generated {
            if let Err(error)=sqlx::query(
                "UPDATE task_recurrence_rules
                    SET template_task_id=(
                            SELECT generated_task_id FROM task_recurrence_runs
                             WHERE recurrence_rule_id=$2 AND scheduled_for=$3
                        ),
                        next_run_at=NOW()+make_interval(days=>$1),updated_at=NOW()
                  WHERE id=$2",
            ).bind(job.interval_days).bind(job.id).bind(job.scheduled_for).execute(&mut *tx).await {
                tracing::error!(%error,rule_id=%job.id,"Could not advance already processed recurrence");
                let _=tx.rollback().await;
                return;
            }
            continue;
        }

        let template=match sqlx::query_as::<_,(String,Option<String>,String,Option<chrono::DateTime<Utc>>,Uuid,Option<Uuid>,Option<i32>)>(
            "SELECT t.title,t.description,t.priority,t.due_at,t.created_by,t.parent_task_id,t.estimated_minutes
             FROM tasks t WHERE t.id=$1")
            .bind(job.template_task_id).fetch_optional(&mut *tx).await {
                Ok(Some(row)) => row,
                Ok(None) => continue,
                Err(error) => {
                    tracing::error!(%error,"Could not load recurring task template");
                    let _=tx.rollback().await;
                    return;
                }
            };
        let position:i32=match sqlx::query_scalar("SELECT COALESCE(MAX(position),0)+1 FROM tasks WHERE list_id=$1")
            .bind(job.target_list_id).fetch_one(&mut *tx).await {
                Ok(value)=>value,
                Err(error)=>{
                    tracing::error!(%error,rule_id=%job.id,"Could not get recurrence task position");
                    let _=tx.rollback().await;
                    return;
                }
            };
        let new_id:Uuid=match sqlx::query_scalar(
            "INSERT INTO tasks(list_id,title,description,priority,due_at,position,created_by,parent_task_id,estimated_minutes)
             VALUES($1,$2,$3,$4,CASE WHEN $5::timestamptz IS NULL THEN NULL ELSE NOW()+make_interval(days=>$6) END,$7,$8,$9,$10)
             RETURNING id")
            .bind(job.target_list_id).bind(&template.0).bind(&template.1).bind(&template.2)
            .bind(template.3).bind(job.interval_days).bind(position).bind(template.4).bind(template.5).bind(template.6)
            .fetch_one(&mut *tx).await {
                Ok(value)=>value,
                Err(error)=>{
                    tracing::error!(%error,rule_id=%job.id,"Could not create recurring task");
                    let _=tx.rollback().await;
                    return;
                }
            };

        if let Err(error)=sqlx::query(
            "INSERT INTO task_assignees(task_id,user_id)
             SELECT $1,user_id FROM task_assignees WHERE task_id=$2 ON CONFLICT DO NOTHING")
            .bind(new_id).bind(job.template_task_id).execute(&mut *tx).await {
                tracing::error!(%error,task_id=%new_id,"Could not copy recurring task assignments");
                let _=tx.rollback().await;
                return;
        }

        if let Err(error)=sqlx::query(
            "INSERT INTO task_recurrence_runs(recurrence_rule_id,scheduled_for,generated_task_id)
             VALUES($1,$2,$3)",
        ).bind(job.id).bind(job.scheduled_for).bind(new_id).execute(&mut *tx).await {
            tracing::error!(%error,rule_id=%job.id,"Could not record generated recurrence");
            let _=tx.rollback().await;
            return;
        }

        if let Err(error)=sqlx::query(
            "UPDATE task_recurrence_rules
                SET template_task_id=$1,next_run_at=NOW()+make_interval(days=>$2),updated_at=NOW()
              WHERE id=$3",
        ).bind(new_id).bind(job.interval_days).bind(job.id).execute(&mut *tx).await {
            tracing::error!(%error,rule_id=%job.id,"Could not advance recurrence rule");
            let _=tx.rollback().await;
            return;
        }

        if let Err(error)=sqlx::query(
            "INSERT INTO activity_events(workspace_id,board_id,task_id,actor_id,event_type,payload)
             VALUES($1,$2,$3,NULL,'recurring_task_created',jsonb_build_object('description','created the next recurring task'))")
            .bind(job.workspace_id).bind(job.board_id).bind(new_id).execute(&mut *tx).await {
                tracing::error!(%error,task_id=%new_id,"Could not record recurring task activity");
                let _=tx.rollback().await;
                return;
        }
        committed_events.push((job.workspace_id, job.board_id, new_id));
    }

    match tx.commit().await {
        Ok(()) => {
            for (workspace_id, board_id, task_id) in committed_events {
                state.publish_workspace_event(
                    workspace_id,
                    Some(board_id),
                    "recurring_task_created",
                    format!("created recurring task {task_id}"),
                );
            }
        }
        Err(error) => tracing::error!(%error,"Could not commit recurrence jobs"),
    }
}

async fn process_deadline_notifications(state:&AppState){
    let approaching=sqlx::query_as::<_,(Uuid,Uuid,Uuid)>(
        "WITH inserted AS (
            INSERT INTO notifications(workspace_id,board_id,recipient_user_id,notification_type,title,body,resource_type,resource_id,dedupe_key)
            SELECT b.workspace_id,b.id,ta.user_id,'deadline_approaching','Deadline approaching',
                'A task assigned to you is due within the next 24 hours.','task',t.id,
                'deadline:'||t.id::text||':'||ta.user_id::text||':'||to_char(t.due_at AT TIME ZONE 'UTC','YYYY-MM-DD')
            FROM tasks t JOIN task_assignees ta ON ta.task_id=t.id
            JOIN lists l ON l.id=t.list_id JOIN boards b ON b.id=l.board_id
            JOIN workspace_members wm ON wm.workspace_id=b.workspace_id AND wm.user_id=ta.user_id
            LEFT JOIN notification_preferences p ON p.user_id=ta.user_id AND p.notification_type='deadline_approaching'
            WHERE t.due_at>NOW() AND t.due_at<=NOW()+INTERVAL '24 hours'
              AND lower(l.name) NOT IN ('done','complete','completed')
              AND (COALESCE(p.in_app_enabled,TRUE) OR COALESCE(p.email_enabled,FALSE))
              AND (wm.role IN ('owner','admin') OR EXISTS(
                    SELECT 1 FROM board_members bm WHERE bm.board_id=b.id AND bm.user_id=ta.user_id
              ))
            ON CONFLICT(dedupe_key) DO NOTHING
            RETURNING id,workspace_id,recipient_user_id,board_id
         ),
         queued_emails AS (
            INSERT INTO notification_email_outbox(notification_id,recipient_user_id)
            SELECT i.id,i.recipient_user_id FROM inserted i
            JOIN notification_preferences p ON p.user_id=i.recipient_user_id AND p.notification_type='deadline_approaching'
            WHERE p.email_enabled
            ON CONFLICT(notification_id) DO NOTHING
            RETURNING notification_id
         )
         SELECT workspace_id,recipient_user_id,board_id FROM inserted")
        .fetch_all(&state.db).await;
    if let Ok(rows)=approaching{
        for (workspace_id,_,board_id) in rows{state.publish_workspace_event(workspace_id,Some(board_id),"notification_created","a deadline reminder is available");}
    }
    let overdue=sqlx::query_as::<_,(Uuid,Uuid,Uuid)>(
        "WITH inserted AS (
            INSERT INTO notifications(workspace_id,board_id,recipient_user_id,notification_type,title,body,resource_type,resource_id,dedupe_key)
            SELECT b.workspace_id,b.id,ta.user_id,'overdue_task','Task overdue',
                'A task assigned to you is overdue.','task',t.id,
                'overdue:'||t.id::text||':'||ta.user_id::text||':'||to_char(t.due_at AT TIME ZONE 'UTC','YYYY-MM-DD')
            FROM tasks t JOIN task_assignees ta ON ta.task_id=t.id
            JOIN lists l ON l.id=t.list_id JOIN boards b ON b.id=l.board_id
            JOIN workspace_members wm ON wm.workspace_id=b.workspace_id AND wm.user_id=ta.user_id
            LEFT JOIN notification_preferences p ON p.user_id=ta.user_id AND p.notification_type='overdue_task'
            WHERE t.due_at<NOW() AND t.due_at>=NOW()-INTERVAL '24 hours'
              AND lower(l.name) NOT IN ('done','complete','completed')
              AND (COALESCE(p.in_app_enabled,TRUE) OR COALESCE(p.email_enabled,FALSE))
              AND (wm.role IN ('owner','admin') OR EXISTS(
                    SELECT 1 FROM board_members bm WHERE bm.board_id=b.id AND bm.user_id=ta.user_id
              ))
            ON CONFLICT(dedupe_key) DO NOTHING
            RETURNING id,workspace_id,recipient_user_id,board_id
         ),
         queued_emails AS (
            INSERT INTO notification_email_outbox(notification_id,recipient_user_id)
            SELECT i.id,i.recipient_user_id FROM inserted i
            JOIN notification_preferences p ON p.user_id=i.recipient_user_id AND p.notification_type='overdue_task'
            WHERE p.email_enabled
            ON CONFLICT(notification_id) DO NOTHING
            RETURNING notification_id
         )
         SELECT workspace_id,recipient_user_id,board_id FROM inserted")
        .fetch_all(&state.db).await;
    if let Ok(rows)=overdue{
        for (workspace_id,_,board_id) in rows{state.publish_workspace_event(workspace_id,Some(board_id),"notification_created","an overdue task reminder is available");}
    }
}

pub async fn run_background_jobs(state:AppState){
    let mut ticker=interval(TokioDuration::from_secs(60));
    loop{
        ticker.tick().await;
        process_recurrences(&state).await;
        process_deadline_notifications(&state).await;
    }
}

pub fn router()->Router<AppState>{
    Router::new()
        .route("/tasks/{task_id}/advanced",get(advanced_details))
        .route("/tasks/{task_id}/subtasks",post(subtask_create))
        .route("/subtasks/{subtask_id}",patch(subtask_update).delete(subtask_delete))
        .route("/tasks/{task_id}/checklist",get(checklist_list).post(checklist_create))
        .route("/checklist-items/{item_id}",patch(checklist_update).delete(checklist_delete))
        .route("/labels",get(labels_list).post(label_create))
        .route("/tasks/{task_id}/labels/{label_id}",post(task_label_add).delete(task_label_remove))
        .route("/tasks/{task_id}/dependencies",post(dependency_add))
        .route("/tasks/{task_id}/dependencies/{depends_on_task_id}",delete(dependency_remove))
        .route("/tasks/{task_id}/estimate",patch(estimate_update))
        .route("/tasks/{task_id}/recurrence",put(recurrence_update))
}
