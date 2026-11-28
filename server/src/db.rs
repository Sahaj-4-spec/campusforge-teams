use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::time::Duration;

/// Open a PostgreSQL pool for normal application queries without schema-owner privileges.
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect(database_url)
        .await
}

/// Apply pending SQLx migrations using a protected release credential.
/// The long-running API should use `connect` with a DML-only runtime role.
pub async fn connect_and_migrate(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = connect(database_url).await?;
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

/// Non-secret connection identity for verifying the database target used by this API instance.
#[derive(Debug)]
pub struct DatabaseIdentity {
    pub database_name: String,
    pub database_user: String,
    pub server_address: Option<String>,
    pub server_port: Option<i32>,
    pub server_version: String,
}

/// Read non-secret identity fields from the active PostgreSQL pool.
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
