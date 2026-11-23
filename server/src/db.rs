use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;

/// Create a PostgreSQL connection pool and apply all pending SQLx migrations.
///
/// Missing migration versions are tolerated so an older, additive-schema-compatible
/// application binary can be rolled back after a newer release has applied a forward
/// migration. SQLx still checks checksums for migration versions present in this binary.
pub async fn connect_and_migrate(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect(database_url)
        .await?;

    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_ignore_missing(true);
    migrator.run(&pool).await?;

    Ok(pool)
}
