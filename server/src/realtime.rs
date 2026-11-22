use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

use crate::{auth::verify_token, state::AppState};

#[derive(Debug, Deserialize)]
struct AuthenticateMessage {
    #[serde(rename = "type")]
    message_type: String,
    token: String,
}

async fn websocket_handler(
    State(state): State<AppState>,
    websocket: WebSocketUpgrade,
) -> impl IntoResponse {
    websocket.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn send_error(socket: &mut WebSocket, message: &'static str) {
    let payload = json!({ "type": "error", "error": message }).to_string();
    let _ = socket.send(Message::Text(payload.into())).await;
    let _ = socket.send(Message::Close(None)).await;
}

async fn user_workspace(pool: &PgPool, user_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT workspace_id FROM workspace_members          WHERE user_id = $1 ORDER BY joined_at ASC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let first_message = timeout(Duration::from_secs(10), socket.recv()).await;
    let message = match first_message {
        Ok(Some(Ok(Message::Text(text)))) => text,
        _ => {
            send_error(&mut socket, "Authenticate within 10 seconds to join the workspace").await;
            return;
        }
    };

    let authentication: AuthenticateMessage = match serde_json::from_str(&message) {
        Ok(value) if value.message_type == "authenticate" => value,
        _ => {
            send_error(&mut socket, "Invalid WebSocket authentication message").await;
            return;
        }
    };

    let user_id = match verify_token(&authentication.token, &state.jwt_secret) {
        Ok(id) => id,
        Err(_) => {
            send_error(&mut socket, "Your session is invalid or has expired").await;
            return;
        }
    };

    let workspace_id = match user_workspace(&state.db, user_id).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            send_error(&mut socket, "No workspace is available for this account").await;
            return;
        }
        Err(error) => {
            tracing::error!(%error, "Could not resolve WebSocket workspace");
            send_error(&mut socket, "Unable to join the workspace right now").await;
            return;
        }
    };

    let mut events = state.workspace_events.subscribe();
    let ready = json!({ "type": "ready" }).to_string();
    if socket.send(Message::Text(ready.into())).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Ping(payload))) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(error)) => {
                        tracing::debug!(%error, "WebSocket receive ended");
                        break;
                    }
                    _ => {}
                }
            }
            event = events.recv() => {
                match event {
                    Ok(event) if event.workspace_id == workspace_id => {
                        let payload = match serde_json::to_string(&json!({
                            "type": "workspace_event",
                            "event": event
                        })) {
                            Ok(payload) => payload,
                            Err(error) => {
                                tracing::error!(%error, "Could not serialize workspace event");
                                continue;
                            }
                        };
                        if socket.send(Message::Text(payload.into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        let payload = json!({ "type": "refresh_required" }).to_string();
                        if socket.send(Message::Text(payload.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(websocket_handler))
}
