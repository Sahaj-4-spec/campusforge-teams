use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::time::Duration;

/// Create a PostgreSQL connection pool and apply all pending SQLx migrations.
pub async fn connect_and_migrate(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect(database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}


/// Return non-secret identity fields for the database reached by the live pool.
/// This lets operators verify the active target without exposing DATABASE_URL.
#[derive(Debug)]
pub struct DatabaseIdentity {
    pub database_name: String,
    pub database_user: String,
    pub server_address: Option<String>,
    pub server_port: Option<i32>,
    pub server_version: String,
}

pub async fn identify(pool: &PgPool) -> Result<DatabaseIdentity, sqlx::Error> {
    let row = sqlx::query(
        "SELECT
            current_database() AS database_name,
            current_user AS database_user,
            inet_server_addr()::text AS server_address,
            inet_server_port() AS server_port,
            current_setting('server_version') AS server_version"
    )
    .fetch_one(pool)
    .await?;

    Ok(DatabaseIdentity {
        database_name: row.try_get("database_name")?,
        database_user: row.try_get("database_user")?,
        server_address: row.try_get("server_address")?,
        server_port: row.try_get("server_port")?,
        server_version: row.try_get("server_version")?,
    })
}
