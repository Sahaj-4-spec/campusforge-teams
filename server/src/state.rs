use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::{postgres::PgListener, PgPool};
use tokio::sync::broadcast;
use uuid::Uuid;

/// An update emitted after a workspace change has been committed to PostgreSQL.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkspaceEvent {
    pub workspace_id: Uuid,
    pub board_id: Option<Uuid>,
    #[serde(default)]
    pub actor_user_id: Option<Uuid>,
    #[serde(default)]
    pub recipient_user_id: Option<Uuid>,
    #[serde(default)]
    pub task_id: Option<Uuid>,
    pub event_type: String,
    pub description: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct WorkspaceEventEnvelope {
    origin_instance_id: Uuid,
    event: WorkspaceEvent,
}

/// Shared application state injected into Axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub jwt_secret: String,
    pub storage: crate::storage::StorageService,
    pub workspace_events: broadcast::Sender<WorkspaceEvent>,
    pub instance_id: Uuid,
}

impl AppState {
    pub fn new(db: PgPool, jwt_secret: String, storage: crate::storage::StorageService) -> Self {
        let (workspace_events, _) = broadcast::channel(256);
        Self {
            db,
            jwt_secret,
            storage,
            workspace_events,
            instance_id: Uuid::new_v4(),
        }
    }

    /// Atomically enforce a fixed-window limit shared by every application instance.
    pub async fn check_rate_limit(&self, key: &str, limit: i32, window_seconds: i32) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, bool>(
            "INSERT INTO api_rate_limits (bucket_key, window_started_at, request_count)
             VALUES ($1, NOW(), 1)
             ON CONFLICT (bucket_key) DO UPDATE SET
               window_started_at = CASE
                 WHEN api_rate_limits.window_started_at + make_interval(secs => $3) <= NOW() THEN NOW()
                 ELSE api_rate_limits.window_started_at END,
               request_count = CASE
                 WHEN api_rate_limits.window_started_at + make_interval(secs => $3) <= NOW() THEN 1
                 ELSE api_rate_limits.request_count + 1 END
             RETURNING request_count <= $2"
        )
        .bind(key)
        .bind(limit)
        .bind(window_seconds)
        .fetch_one(&self.db)
        .await
    }

    pub fn publish_workspace_event(
        &self,
        workspace_id: Uuid,
        board_id: Option<Uuid>,
        event_type: impl Into<String>,
        description: impl Into<String>,
    ) {
        self.publish_event(workspace_id,board_id,None,None,None,event_type,description);
    }

    pub fn publish_user_workspace_event(
        &self,
        workspace_id: Uuid,
        board_id: Option<Uuid>,
        actor_user_id: Option<Uuid>,
        event_type: impl Into<String>,
        description: impl Into<String>,
    ) {
        self.publish_event(workspace_id,board_id,actor_user_id,None,None,event_type,description);
    }

    /// Publish an event that is delivered only to one workspace member.
    /// Used for private direct-message notifications so other workspace
    /// members cannot infer private conversation activity.
    pub fn publish_private_workspace_event(
        &self,
        workspace_id: Uuid,
        recipient_user_id: Uuid,
        actor_user_id: Option<Uuid>,
        event_type: impl Into<String>,
        description: impl Into<String>,
    ) {
        self.publish_event(workspace_id,None,actor_user_id,Some(recipient_user_id),None,event_type,description);
    }

    /// Publish an ephemeral task-level event, such as a teammate editing a card.
    pub fn publish_task_workspace_event(
        &self,
        workspace_id: Uuid,
        board_id: Uuid,
        task_id: Uuid,
        actor_user_id: Option<Uuid>,
        event_type: impl Into<String>,
        description: impl Into<String>,
    ) {
        self.publish_event(workspace_id,Some(board_id),actor_user_id,None,Some(task_id),event_type,description);
    }

    fn publish_event(
        &self,
        workspace_id: Uuid,
        board_id: Option<Uuid>,
        actor_user_id: Option<Uuid>,
        recipient_user_id: Option<Uuid>,
        task_id: Option<Uuid>,
        event_type: impl Into<String>,
        description: impl Into<String>,
    ) {
        let event = WorkspaceEvent {
            workspace_id,
            board_id,
            actor_user_id,
            recipient_user_id,
            task_id,
            event_type: event_type.into(),
            description: description.into(),
            created_at: chrono::Utc::now(),
        };
        let _ = self.workspace_events.send(event.clone());

        let db = self.db.clone();
        let envelope = WorkspaceEventEnvelopeOwned {
            origin_instance_id: self.instance_id,
            event,
        };
        let Ok(payload) = serde_json::to_string(&envelope) else { return; };
        tokio::spawn(async move {
            if let Err(error) = sqlx::query("SELECT pg_notify('campusforge_workspace_events',$1)")
                .bind(payload)
                .execute(&db)
                .await
            {
                tracing::warn!(%error, "Could not publish workspace event through PostgreSQL");
            }
        });
    }
}

#[derive(Serialize)]
struct WorkspaceEventEnvelopeOwned {
    origin_instance_id: Uuid,
    event: WorkspaceEvent,
}

/// Relay cross-instance workspace notifications into the local WebSocket
/// broadcast channel. PostgreSQL NOTIFY is a signal only; persistent data
/// remains in PostgreSQL and clients reload through the authorized REST API.
pub async fn run_postgres_workspace_listener(state: AppState) {
    let mut listener = loop {
        match PgListener::connect_with(&state.db).await {
            Ok(mut listener) => {
                match listener.listen("campusforge_workspace_events").await {
                    Ok(()) => break listener,
                    Err(error) => {
                        tracing::error!(%error, "Could not subscribe to workspace events");
                    }
                }
            }
            Err(error) => tracing::error!(%error, "Could not connect workspace event listener"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    };

    loop {
        match listener.recv().await {
            Ok(message) => {
                let envelope = match serde_json::from_str::<WorkspaceEventEnvelope>(message.payload()) {
                    Ok(envelope) => envelope,
                    Err(error) => {
                        tracing::warn!(%error, "Ignoring malformed workspace event notification");
                        continue;
                    }
                };
                if envelope.origin_instance_id != state.instance_id {
                    let _ = state.workspace_events.send(envelope.event);
                }
            }
            Err(error) => {
                tracing::warn!(%error, "Workspace event listener disconnected; reconnecting");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                loop {
                    match PgListener::connect_with(&state.db).await {
                        Ok(mut next) => match next.listen("campusforge_workspace_events").await {
                            Ok(()) => { listener = next; break; }
                            Err(error) => tracing::error!(%error, "Could not restore workspace event subscription"),
                        },
                        Err(error) => tracing::error!(%error, "Could not reconnect workspace event listener"),
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                }
            }
        }
    }
}

/// Remove expired rate-limit keys periodically so the shared table remains bounded.
pub async fn run_rate_limit_cleanup(db: PgPool) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(900)).await;
        if let Err(error) = sqlx::query("DELETE FROM api_rate_limits WHERE window_started_at < NOW() - INTERVAL '1 day'")
            .execute(&db).await {
            tracing::warn!(%error, "Could not clean expired authentication rate-limit keys");
        }
    }
}
