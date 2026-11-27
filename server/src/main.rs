mod api;
mod advanced_tasks;
mod ai;
mod ai_proposals;
mod collaboration;
mod auth;
mod db;
mod invitations;
mod lists;
mod oauth;
mod notifications;
mod realtime;
mod roadmap;
mod state;
mod storage;

use axum::{
    extract::State,
    http::{header, HeaderName, HeaderValue, Method, StatusCode},
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

    // Explicit production mode fails closed unless durable private storage and
    // mandatory malware scanning are configured. Keep previews/local development usable.
    if std::env::var("CAMPUSFORGE_ENV").is_ok_and(|value| value.eq_ignore_ascii_case("production")) {
        let storage_mode = std::env::var("CAMPUSFORGE_STORAGE_MODE").unwrap_or_default();
        let require_remote = std::env::var("CAMPUSFORGE_REQUIRE_REMOTE_STORAGE")
            .is_ok_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes"));
        let require_scan = std::env::var("CAMPUSFORGE_REQUIRE_MALWARE_SCAN")
            .is_ok_and(|value| value.eq_ignore_ascii_case("true"));
        let scanner_configured = std::env::var("CAMPUSFORGE_CLAMAV_ADDR")
            .ok().is_some_and(|value| !value.trim().is_empty());
        if storage_mode != "s3" || !require_remote || !require_scan || !scanner_configured {
            return Err("Production startup blocked: configure S3 storage, CAMPUSFORGE_REQUIRE_REMOTE_STORAGE=true, CAMPUSFORGE_REQUIRE_MALWARE_SCAN=true, and CAMPUSFORGE_CLAMAV_ADDR before setting CAMPUSFORGE_ENV=production".into());
        }
    }

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

    // Log only non-secret database identity fields so the live target can be verified
    // without ever exposing DATABASE_URL or its credentials.
    let database_host = url::Url::parse(&database_url)
        .ok()
        .and_then(|value| value.host_str().map(str::to_owned))
        .unwrap_or_else(|| "unparsed-database-url".to_string());
    let pool = db::connect_and_migrate(&database_url).await?;
    let database_identity = db::identify(&pool).await?;
    tracing::info!(
        database_host = %database_host,
        database_name = %database_identity.database_name,
        database_user = %database_identity.database_user,
        server_address = ?database_identity.server_address,
        server_port = ?database_identity.server_port,
        server_version = %database_identity.server_version,
        "Verified active PostgreSQL connection target; migrations applied"
    );

    let storage = storage::StorageService::from_env().await?;
    if storage.verify_remote().await.map_err(|error| {
        std::io::Error::new(std::io::ErrorKind::Other, error)
    })? {
        tracing::info!("S3-compatible object storage PUT/GET/DELETE verification passed");
    }
    let state = AppState::new(pool, jwt_secret, storage);
    let rate_limit_db = state.db.clone();
    tokio::spawn(async move { state::run_rate_limit_cleanup(rate_limit_db).await; });
    let background_state = state.clone();
    tokio::spawn(async move {
        advanced_tasks::run_background_jobs(background_state).await;
    });
    let email_state = state.clone();
    tokio::spawn(async move {
        notifications::run_email_worker(email_state).await;
    });
    let file_cleanup_state = state.clone();
    tokio::spawn(async move {
        collaboration::run_file_cleanup(file_cleanup_state).await;
    });
    let roadmap_state = state.clone();
    tokio::spawn(async move {
        roadmap::run_roadmap_jobs(roadmap_state).await;
    });
    let realtime_state = state.clone();
    tokio::spawn(async move {
        state::run_postgres_workspace_listener(realtime_state).await;
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
        .allow_credentials(true)
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            HeaderName::from_static("x-workspace-id"),
        ]);

    let api_router = Router::new()
        .merge(ai::router())
        .merge(ai_proposals::router())
        .merge(api::router())
        .merge(lists::router())
        .merge(advanced_tasks::router())
        .merge(collaboration::router())
        .merge(roadmap::router());
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
