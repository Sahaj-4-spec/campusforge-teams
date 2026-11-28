use sqlx::postgres::PgPoolOptions;
use std::{env, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let database_url = env::var("MIGRATION_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| "MIGRATION_DATABASE_URL (or DATABASE_URL for local development) must be set")?;

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(15))
        .connect(&database_url)
        .await?;

    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_ignore_missing(true);
    migrator.run(&pool).await?;

    let identity: (String, String) = sqlx::query_as("SELECT current_database(), current_user")
        .fetch_one(&pool)
        .await?;

    println!(
        "SQLx migrations verified/applied for database '{}' using role '{}'.",
        identity.0, identity.1
    );
    pool.close().await;
    Ok(())
}
