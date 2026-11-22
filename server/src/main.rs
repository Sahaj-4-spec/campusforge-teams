mod api;
mod advanced_tasks;
mod collaboration;
mod auth;
mod db;
mod invitations;
mod lists;
mod oauth;
mod notifications;
mod realtime;
mod state;
mod storage;

use axum::{
    extract::State,
    http::{header, HeaderValue, Method, StatusCode},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::state::AppState;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    database: &'static str,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

async fn health(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, (StatusCode, Json<ErrorResponse>)> {
    if let Err(error) = sqlx::query("SELECT 1").execute(&state.db).await {
        tracing::error!(%error, "Database health check failed");
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "Database unavailable",
            }),
        ));
    }

    Ok(Json(HealthResponse {
        status: "ok",
        service: "campusforge-api",
        database: "connected",
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .map_err(|_| "DATABASE_URL must be set before starting CampusForge API")?;
    let jwt_secret = std::env::var("JWT_SECRET")
        .map_err(|_| "JWT_SECRET must contain at least 32 characters")?;
    if jwt_secret.len() < 32 {
        return Err("JWT_SECRET must contain at least 32 characters".into());
    }

    let frontend_origin = std::env::var("FRONTEND_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:5173".to_string());

    let pool = db::connect_and_migrate(&database_url).await?;
    tracing::info!("PostgreSQL connected and migrations applied");

    let storage = storage::StorageService::from_env().await?;
    let state = AppState::new(pool, jwt_secret, storage);
    let background_state = state.clone();
    tokio::spawn(async move {
        advanced_tasks::run_background_jobs(background_state).await;
    });
    let email_state = state.clone();
    tokio::spawn(async move {
        notifications::run_email_worker(email_state).await;
    });
    let cors = CorsLayer::new()
        .allow_origin(frontend_origin.parse::<HeaderValue>()?)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

    let api_router = Router::new()
        .merge(api::router())
        .merge(lists::router())
        .merge(advanced_tasks::router())
        .merge(collaboration::router());
    let auth_router = auth::router().merge(oauth::router());

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/health", get(health))
        .nest("/api/auth", auth_router)
        .nest("/api/invitations", invitations::router())
        .nest("/api", api_router)
        .nest("/ws", realtime::router())
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "5000".to_string())
        .parse::<u16>()?;

    let address = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(address).await?;

    tracing::info!("CampusForge API listening on {address}");

    axum::serve(listener, app).await?;

    Ok(())
}
