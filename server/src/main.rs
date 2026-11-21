use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "campusforge-api",
    })
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

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/health", get(health))
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
