use serde::Serialize;
use sqlx::PgPool;
use tokio::sync::broadcast;
use uuid::Uuid;

/// An update emitted after a workspace change has been committed to PostgreSQL.
#[derive(Clone, Debug, Serialize)]
pub struct WorkspaceEvent {
    pub workspace_id: Uuid,
    pub board_id: Option<Uuid>,
    pub event_type: String,
    pub description: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Shared application state injected into Axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub jwt_secret: String,
    pub workspace_events: broadcast::Sender<WorkspaceEvent>,
}

impl AppState {
    pub fn new(db: PgPool, jwt_secret: String) -> Self {
        let (workspace_events, _) = broadcast::channel(256);
        Self {
            db,
            jwt_secret,
            workspace_events,
        }
    }

    pub fn publish_workspace_event(
        &self,
        workspace_id: Uuid,
        board_id: Option<Uuid>,
        event_type: impl Into<String>,
        description: impl Into<String>,
    ) {
        let _ = self.workspace_events.send(WorkspaceEvent {
            workspace_id,
            board_id,
            event_type: event_type.into(),
            description: description.into(),
            created_at: chrono::Utc::now(),
        });
    }
}
