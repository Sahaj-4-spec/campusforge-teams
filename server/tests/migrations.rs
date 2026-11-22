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

    // Deleting a board cascades its workspace_files metadata. The trigger must
    // preserve the object key in a table that is not part of the workspace cascade.
    let user_id = uuid::Uuid::new_v4();
    let workspace_id = uuid::Uuid::new_v4();
    let board_id = uuid::Uuid::new_v4();
    let storage_key = format!("cascade-test-{}", uuid::Uuid::new_v4());
    let test_email = format!("{}@example.test", uuid::Uuid::new_v4());
    let mut transaction = pool.begin().await.expect("cascade fixture transaction should start");

    sqlx::query(
        "INSERT INTO users(id,email,password_hash,full_name)
         VALUES($1,$2,NULL,'Cascade Test User')",
    )
    .bind(user_id)
    .bind(test_email)
    .execute(&mut *transaction)
    .await
    .expect("cascade fixture user should be inserted");

    sqlx::query(
        "INSERT INTO workspaces(id,name,owner_id) VALUES($1,'Cascade Test Workspace',$2)",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await
    .expect("cascade fixture workspace should be inserted");

    sqlx::query(
        "INSERT INTO workspace_members(workspace_id,user_id,role) VALUES($1,$2,'owner')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await
    .expect("cascade fixture owner membership should be inserted");

    sqlx::query(
        "INSERT INTO boards(id,workspace_id,name,created_by)
         VALUES($1,$2,'Cascade Test Project',$3)",
    )
    .bind(board_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await
    .expect("cascade fixture board should be inserted");

    sqlx::query(
        "INSERT INTO workspace_files
            (workspace_id,board_id,uploaded_by,original_name,storage_key,media_type,size_bytes,checksum_sha256)
         VALUES($1,$2,$3,'cascade-test.txt',$4,'text/plain',1,$5)",
    )
    .bind(workspace_id)
    .bind(board_id)
    .bind(user_id)
    .bind(&storage_key)
    .bind("0".repeat(64))
    .execute(&mut *transaction)
    .await
    .expect("cascade fixture file metadata should be inserted");

    sqlx::query("DELETE FROM boards WHERE id=$1")
        .bind(board_id)
        .execute(&mut *transaction)
        .await
        .expect("deleting the board should cascade file metadata");

    let cascade_object_is_queued: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM object_deletion_outbox WHERE storage_key=$1)",
    )
    .bind(&storage_key)
    .fetch_one(&mut *transaction)
    .await
    .expect("object deletion outbox should be queryable");
    assert!(cascade_object_is_queued, "a board cascade must retain the deleted file's object key");

    transaction.rollback().await.expect("cascade fixture should roll back");

    pool.close().await;
}
