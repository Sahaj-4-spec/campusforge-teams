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

fn parse_frontend_origins(raw: &str, require_https: bool) -> Result<Vec<HeaderValue>, Box<dyn std::error::Error>> {
    let mut origins = Vec::new();

    for entry in raw.split(',').map(str::trim).filter(|entry| !entry.is_empty()) {
        if entry == "*" {
            return Err("FRONTEND_ORIGIN cannot use '*' when credentialed CORS is enabled".into());
        }

        let parsed = url::Url::parse(entry)?;
        if !matches!(parsed.scheme(), "http" | "https")
            || (require_https && parsed.scheme() != "https")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !matches!(parsed.path(), "" | "/")
        {
            return Err(format!("FRONTEND_ORIGIN entry must be a plain HTTP(S) origin: {entry}").into());
        }

        let serialized = parsed.origin().ascii_serialization();
        if serialized == "null" {
            return Err("FRONTEND_ORIGIN contains an opaque origin".into());
        }
        let header_value = serialized.parse::<HeaderValue>()?;
        if !origins.contains(&header_value) {
            origins.push(header_value);
        }
    }

    if origins.is_empty() {
        return Err("FRONTEND_ORIGIN must contain at least one valid HTTP(S) origin".into());
    }
    Ok(origins)
}

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

fn should_run_migrations(configured: Option<&str>, production_mode: bool) -> Result<bool, &'static str> {
    match configured.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        Some("true" | "1" | "yes") => Ok(true),
        Some("false" | "0" | "no") => Ok(false),
        Some(_) => Err("CAMPUSFORGE_AUTO_MIGRATE must be true or false"),
        None => Ok(!production_mode),
    }
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

    // Comma-separated exact origins allow isolated preview domains without
    // weakening credentialed CORS to a wildcard.
    let frontend_origin = std::env::var("FRONTEND_ORIGIN")
        .unwrap_or_else(|_| "http://localhost:5173".to_string());
    let production_mode = std::env::var("CAMPUSFORGE_ENV")
        .is_ok_and(|value| value.eq_ignore_ascii_case("production"));
    let allowed_frontend_origins = parse_frontend_origins(&frontend_origin, production_mode)?;

    // Log only non-secret database identity fields so the live target can be verified
    // without ever exposing DATABASE_URL or its credentials.
    let database_host = url::Url::parse(&database_url)
        .ok()
        .and_then(|value| value.host_str().map(str::to_owned))
        .unwrap_or_else(|| "unparsed-database-url".to_string());
    // Keep DDL out of the long-running API process when a least-privilege runtime role is used.
    // In production, migrations are a separate release operation unless explicitly overridden.
    let auto_migrate = should_run_migrations(
        std::env::var("CAMPUSFORGE_AUTO_MIGRATE").ok().as_deref(),
        production_mode,
    )?;
    let pool = if auto_migrate {
        tracing::warn!("Automatic SQLx migrations are enabled for this API process");
        db::connect_and_migrate(&database_url).await?
    } else {
        tracing::info!("Automatic SQLx migrations disabled; connecting with runtime database privileges");
        db::connect(&database_url).await?
    };
    let database_identity = db::identify(&pool).await?;
    tracing::info!(
        database_host = %database_host,
        database_name = %database_identity.database_name,
        database_user = %database_identity.database_user,
        server_address = ?database_identity.server_address,
        server_port = ?database_identity.server_port,
        server_version = %database_identity.server_version,
        "Verified active PostgreSQL connection target"
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
    let session_cleanup_db = state.db.clone();
    tokio::spawn(async move { auth::run_session_cleanup(session_cleanup_db).await; });
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
        .allow_origin(allowed_frontend_origins)
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


#[cfg(test)]
mod cors_origin_tests {
    use super::parse_frontend_origins;

    #[test]
    fn accepts_multiple_exact_origins_and_deduplicates() {
        let origins = parse_frontend_origins(
            "https://app.example.com, https://preview.example.com/, https://app.example.com",
            true,
        ).expect("valid origins");
        assert_eq!(origins.len(), 2);
        assert_eq!(origins[0].to_str().unwrap(), "https://app.example.com");
        assert_eq!(origins[1].to_str().unwrap(), "https://preview.example.com");
    }

    #[test]
    fn rejects_wildcard_for_credentialed_cors() {
        assert!(parse_frontend_origins("*", false).is_err());
    }

    #[test]
    fn rejects_paths_and_non_http_schemes() {
        assert!(parse_frontend_origins("https://example.com/dashboard", false).is_err());
        assert!(parse_frontend_origins("javascript:alert(1)", false).is_err());
        assert!(parse_frontend_origins("http://example.com", true).is_err());
    }

    #[test]
    fn rejects_an_empty_origin_list() {
        assert!(parse_frontend_origins(" , , ", false).is_err());
    }
}

#[cfg(test)]
mod migration_configuration_tests {
    use super::should_run_migrations;

    #[test]
    fn production_defaults_to_manual_migrations() {
        assert!(!should_run_migrations(None, true).unwrap());
    }

    #[test]
    fn non_production_keeps_local_auto_migrations_by_default() {
        assert!(should_run_migrations(None, false).unwrap());
    }

    #[test]
    fn explicit_configuration_overrides_default() {
        assert!(should_run_migrations(Some(" true "), true).unwrap());
        assert!(!should_run_migrations(Some("false"), false).unwrap());
    }

    #[test]
    fn invalid_configuration_fails_closed() {
        assert!(should_run_migrations(Some("sometimes"), true).is_err());
    }
}
