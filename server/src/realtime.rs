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
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use tokio::time::{interval, timeout, Instant};
use uuid::Uuid;

use crate::{
    auth::{frontend_origin_matches, session_token_from_headers, verify_token},
    state::AppState,
};

#[derive(Debug, Deserialize)]
struct AuthenticateMessage {
    #[serde(rename = "type")]
    message_type: String,
    workspace_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct ClientWorkspaceMessage {
    #[serde(rename = "type")]
    message_type: String,
    #[serde(default)]
    board_id: Option<Uuid>,
    #[serde(default)]
    task_id: Option<Uuid>,
    #[serde(default)]
    editing: Option<bool>,
}

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

async fn handle_socket(mut socket: WebSocket, state: AppState, user_id: Uuid, expires_at: usize) {
    let first_message = timeout(Duration::from_secs(10), socket.recv()).await;
    let message = match first_message {
        Ok(Some(Ok(Message::Text(text)))) => text,
        _ => {
            send_error(&mut socket, "Authenticate within 10 seconds to join the workspace").await;
            return;
        }
    };

    let authentication: AuthenticateMessage = match serde_json::from_str::<AuthenticateMessage>(&message) {
        Ok(value) if value.message_type == "authenticate" => value,
        _ => {
            send_error(&mut socket, "Invalid WebSocket authentication message").await;
            return;
        }
    };

    let is_member = match sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM workspace_members WHERE user_id=$1 AND workspace_id=$2)",
    )
    .bind(user_id)
    .bind(authentication.workspace_id)
    .fetch_one(&state.db)
    .await {
        Ok(value) => value,
        Err(error) => {
            tracing::error!(%error, "Could not verify WebSocket workspace membership");
            send_error(&mut socket, "Unable to join the workspace right now").await;
            return;
        }
    };
    if !is_member {
        send_error(&mut socket, "You are not a member of the selected workspace").await;
        return;
    }
    let workspace_id = authentication.workspace_id;
    let connection_id = Uuid::new_v4();
    if let Err(error) = sqlx::query(
        "INSERT INTO workspace_presence(connection_id,workspace_id,user_id) VALUES($1,$2,$3)",
    )
    .bind(connection_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(&state.db)
    .await {
        tracing::error!(%error, "Could not register workspace presence");
        send_error(&mut socket, "Unable to establish workspace presence right now").await;
        return;
    }

    let mut events = state.workspace_events.subscribe();
    let now = chrono::Utc::now().timestamp().max(0) as usize;
    let remaining_seconds = expires_at.saturating_sub(now) as u64;
    let session_expiry = tokio::time::sleep(Duration::from_secs(remaining_seconds));
    tokio::pin!(session_expiry);
    let mut heartbeat = interval(Duration::from_secs(30));
    heartbeat.tick().await;
    let mut last_typing = None::<Instant>;

    state.publish_user_workspace_event(
        workspace_id,
        None,
        Some(user_id),
        "presence_joined",
        "A workspace member came online",
    );
    let ready = json!({ "type": "ready" }).to_string();
    if socket.send(Message::Text(ready.into())).await.is_err() {
        let _ = sqlx::query("DELETE FROM workspace_presence WHERE connection_id=$1")
            .bind(connection_id).execute(&state.db).await;
        state.publish_user_workspace_event(workspace_id,None,Some(user_id),"presence_left","A workspace member went offline");
        return;
    }

    loop {
        tokio::select! {
            _ = &mut session_expiry => {
                send_error(&mut socket, "Your session has expired; sign in again").await;
                break;
            }
            _ = heartbeat.tick() => {
                if let Err(error) = sqlx::query("UPDATE workspace_presence SET last_seen_at=NOW() WHERE connection_id=$1")
                    .bind(connection_id).execute(&state.db).await
                {
                    tracing::warn!(%error, "Could not refresh workspace presence lease");
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Ping(payload))) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Text(text))) if text.len() <= 1024 => {
                        if let Ok(client_event) = serde_json::from_str::<ClientWorkspaceMessage>(&text) {
                            if client_event.message_type == "board_view" {
                                let valid_board = if let Some(board_id) = client_event.board_id {
                                    sqlx::query_scalar::<_,bool>(
                                        "SELECT EXISTS(SELECT 1 FROM boards WHERE id=$1 AND workspace_id=$2)",
                                    ).bind(board_id).bind(workspace_id).fetch_one(&state.db).await.unwrap_or(false)
                                } else {
                                    true
                                };
                                if valid_board {
                                    let previous_board: Option<Uuid> = sqlx::query_scalar(
                                        "SELECT board_id FROM workspace_presence WHERE connection_id=$1",
                                    ).bind(connection_id).fetch_optional(&state.db).await.ok().flatten();
                                    if previous_board != client_event.board_id {
                                        if let Err(error) = sqlx::query(
                                            "UPDATE workspace_presence SET board_id=$1,last_seen_at=NOW() WHERE connection_id=$2",
                                        ).bind(client_event.board_id).bind(connection_id).execute(&state.db).await {
                                            tracing::warn!(%error, "Could not update active board presence");
                                        } else {
                                            if let Some(previous_board) = previous_board {
                                                state.publish_user_workspace_event(
                                                    workspace_id, Some(previous_board), Some(user_id),
                                                    "presence_left", "A teammate left this board",
                                                );
                                            }
                                            if let Some(board_id) = client_event.board_id {
                                                state.publish_user_workspace_event(
                                                    workspace_id, Some(board_id), Some(user_id),
                                                    "presence_joined", "A teammate is viewing this board",
                                                );
                                            }
                                        }
                                    }
                                }
                            } else if client_event.message_type == "card_editing" {
                                if let (Some(board_id), Some(task_id), Some(editing)) =
                                    (client_event.board_id, client_event.task_id, client_event.editing)
                                {
                                    let is_viewing_board: bool = sqlx::query_scalar(
                                        "SELECT EXISTS(SELECT 1 FROM workspace_presence
                                         WHERE connection_id=$1 AND workspace_id=$2 AND board_id=$3)",
                                    ).bind(connection_id).bind(workspace_id).bind(board_id)
                                        .fetch_one(&state.db).await.unwrap_or(false);
                                    let task_on_board: bool = sqlx::query_scalar(
                                        "SELECT EXISTS(SELECT 1 FROM tasks t
                                         JOIN lists l ON l.id=t.list_id
                                         WHERE t.id=$1 AND l.board_id=$2)",
                                    ).bind(task_id).bind(board_id).fetch_one(&state.db).await.unwrap_or(false);
                                    if is_viewing_board && task_on_board {
                                        state.publish_task_workspace_event(
                                            workspace_id,
                                            board_id,
                                            task_id,
                                            Some(user_id),
                                            if editing { "card_editing_started" } else { "card_editing_stopped" },
                                            if editing { "A teammate is editing this card" } else { "A teammate stopped editing this card" },
                                        );
                                    }
                                }
                            } else if client_event.message_type == "typing" {
                                let now = Instant::now();
                                let should_publish = last_typing
                                    .map(|last| now.duration_since(last) >= Duration::from_millis(1500))
                                    .unwrap_or(true);
                                if should_publish {
                                    let valid_board = if let Some(board_id) = client_event.board_id {
                                        sqlx::query_scalar::<_,bool>(
                                            "SELECT EXISTS(SELECT 1 FROM boards WHERE id=$1 AND workspace_id=$2)",
                                        ).bind(board_id).bind(workspace_id).fetch_one(&state.db).await.unwrap_or(false)
                                    } else {
                                        true
                                    };
                                    if valid_board {
                                        state.publish_user_workspace_event(
                                            workspace_id,
                                            client_event.board_id,
                                            Some(user_id),
                                            "typing_indicator",
                                            "A workspace member is typing",
                                        );
                                        last_typing = Some(now);
                                    }
                                }
                            }
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
                    Ok(event) if event.workspace_id == workspace_id
                        && event.recipient_user_id.map_or(true, |recipient| recipient == user_id) => {
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

    let active_board: Option<Uuid> = sqlx::query_scalar(
        "SELECT board_id FROM workspace_presence WHERE connection_id=$1",
    ).bind(connection_id).fetch_optional(&state.db).await.ok().flatten();
    if let Err(error) = sqlx::query("DELETE FROM workspace_presence WHERE connection_id=$1")
        .bind(connection_id)
        .execute(&state.db)
        .await
    {
        tracing::warn!(%error, "Could not remove workspace presence lease");
    }
    if let Some(board_id) = active_board {
        state.publish_user_workspace_event(
            workspace_id, Some(board_id), Some(user_id),
            "presence_left", "A teammate left this board",
        );
    }
    state.publish_user_workspace_event(
        workspace_id,
        None,
        Some(user_id),
        "presence_left",
        "A workspace member went offline",
    );
}

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(websocket_handler))
}
