use sqlx::postgres::PgPoolOptions;
use std::env;

#[tokio::test]
async fn applies_all_postgres_migrations_and_creates_collaboration_tables() {
    let database_url = match env::var("DATABASE_URL") {
        Ok(value) => value,
        Err(_) => {
            eprintln!("Skipping database migration smoke test: DATABASE_URL is not configured");
            return;
        }
    };

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("test PostgreSQL service should accept connections");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("all CampusForge migrations should apply cleanly");

    for table in [
        "public.task_checklist_items",
        "public.labels",
        "public.task_labels",
        "public.task_dependencies",
        "public.task_recurrence_rules",
        "public.calendar_events",
        "public.notifications",
        "public.notification_preferences",
        "public.notification_email_outbox",
        "public.object_deletion_outbox",
        "public.workspace_files",
        "public.chat_messages",
        "public.meeting_notes",
    ] {
        let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
            .bind(table)
            .fetch_one(&pool)
            .await
            .expect("table lookup should succeed");
        assert!(exists, "expected migration to create {table}");
    }

    let storage_key_is_opaque_and_unique: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM information_schema.columns
             WHERE table_schema='public'
               AND table_name='workspace_files'
               AND column_name='storage_key'
               AND is_nullable='NO'
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("workspace file metadata schema should be readable");
    assert!(storage_key_is_opaque_and_unique);

    let object_deletion_trigger_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_trigger WHERE tgname='trg_workspace_files_queue_object_delete' AND NOT tgisinternal)",
    )
    .fetch_one(&pool)
    .await
    .expect("object deletion trigger should be installed");
    assert!(object_deletion_trigger_exists);

    let suppressed_email_jobs_supported: bool = sqlx::query_scalar(
        "SELECT position('suppressed' in pg_get_constraintdef(oid)) > 0
           FROM pg_constraint
          WHERE conname='email_outbox_status_valid'",
    )
    .fetch_one(&pool)
    .await
    .expect("email outbox status constraint should be readable");
    assert!(suppressed_email_jobs_supported);

    pool.close().await;
}
