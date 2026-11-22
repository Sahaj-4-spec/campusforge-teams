use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde_json::json;
use sqlx::PgPool;
use std::time::Duration;
use uuid::Uuid;

use crate::{
    auth::{frontend_origin_matches, session_token_from_headers, verify_token},
    state::AppState,
};

async fn websocket_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    websocket: WebSocketUpgrade,
) -> Response {
    if !frontend_origin_matches(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(token) = session_token_from_headers(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let (user_id, expires_at) = match verify_token(&token, &state.jwt_secret) {
        Ok(identity) => identity,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    websocket
        .on_upgrade(move |socket| handle_socket(socket, state, user_id, expires_at))
        .into_response()
}

async fn send_error(socket: &mut WebSocket, message: &'static str) {
    let payload = json!({ "type": "error", "error": message }).to_string();
    let _ = socket.send(Message::Text(payload.into())).await;
    let _ = socket.send(Message::Close(None)).await;
}

async fn user_workspace(pool: &PgPool, user_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT workspace_id FROM workspace_members          WHERE user_id = $1 ORDER BY joined_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

async fn handle_socket(mut socket: WebSocket, state: AppState, user_id: Uuid, expires_at: usize) {
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
    let now = chrono::Utc::now().timestamp().max(0) as usize;
    let remaining_seconds = expires_at.saturating_sub(now) as u64;
    let session_expiry = tokio::time::sleep(Duration::from_secs(remaining_seconds));
    tokio::pin!(session_expiry);
    let ready = json!({ "type": "ready" }).to_string();
    if socket.send(Message::Text(ready.into())).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            _ = &mut session_expiry => {
                send_error(&mut socket, "Your session has expired; sign in again").await;
                break;
            }
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
