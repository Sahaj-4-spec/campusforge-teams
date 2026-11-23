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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rollback_compatible_runner_ignores_unknown_forward_migration_versions() {
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            eprintln!("Skipping migration rollback compatibility test: DATABASE_URL is not configured");
            return;
        };

        let pool = connect_and_migrate(&database_url)
            .await
            .expect("the test database should migrate successfully");

        // Simulate a newer release having recorded a forward migration that this
        // older binary does not embed. The compatibility runner must still start.
        let future_version = i64::MAX;
        sqlx::query(
            "INSERT INTO _sqlx_migrations
                (version, description, success, checksum, execution_time)
             VALUES ($1, 'simulated future migration', TRUE, $2, 0)",
        )
        .bind(future_version)
        .bind(vec![0_u8; 32])
        .execute(&pool)
        .await
        .expect("the simulated future migration should be recorded");

        let rollback_result = connect_and_migrate(&database_url).await;

        sqlx::query("DELETE FROM _sqlx_migrations WHERE version = $1")
            .bind(future_version)
            .execute(&pool)
            .await
            .expect("the simulated future migration row should be cleaned up");

        rollback_result.expect("older binary should tolerate an unknown forward migration version");
        pool.close().await;
    }
}
