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
        "public.calendar_event_attendees",
        "public.workspace_storage_quotas",
        "public.workspace_storage_reservations",
        "public.file_audit_events",
        "public.chat_conversations",
        "public.chat_message_mentions",
        "public.meeting_note_action_items",
        "public.workspace_analytics_daily",
        "public.task_recurrence_runs",
        "public.workspace_presence",
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

    let reconciliation_status_supported: bool = sqlx::query_scalar(
        "SELECT position('reconciliation' in pg_get_constraintdef(oid)) > 0
           FROM pg_constraint WHERE conname='email_outbox_status_valid'",
    )
    .fetch_one(&pool)
    .await
    .expect("email outbox reconciliation status should be readable");
    assert!(reconciliation_status_supported);

    let required_constraint_count:i64=sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM pg_constraint
          WHERE conname IN (
              'calendar_attendee_response_valid',
              'workspace_storage_quota_positive',
              'workspace_file_scan_status_valid',
              'chat_message_channel_xor',
              'direct_message_distinct_participants',
              'calendar_all_day_range_valid'
          )",
    ).fetch_one(&pool).await.expect("roadmap constraints should be queryable");
    assert_eq!(required_constraint_count,6,"all non-AI roadmap integrity constraints should be installed");

    let presence_connection_index:bool=sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_indexes WHERE schemaname='public' AND indexname='idx_workspace_presence_active')",
    ).fetch_one(&pool).await.expect("presence index should be queryable");
    assert!(presence_connection_index,"workspace presence should be indexed for active-member lookup");

    let board_presence_index: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_indexes
          WHERE schemaname='public' AND indexname='idx_workspace_presence_board_active')",
    ).fetch_one(&pool).await.expect("board presence index should be queryable");
    assert!(board_presence_index, "board-scoped presence should be indexed");

    let presence_board_column: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM information_schema.columns
          WHERE table_schema='public' AND table_name='workspace_presence'
            AND column_name='board_id' AND is_nullable='YES')",
    ).fetch_one(&pool).await.expect("board presence column should be readable");
    assert!(presence_board_column, "presence rows should store the current board");

    let comment_parent_column: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM information_schema.columns
          WHERE table_schema='public' AND table_name='comments'
            AND column_name='parent_comment_id' AND is_nullable='YES')",
    ).fetch_one(&pool).await.expect("threaded comment parent column should be readable");
    assert!(comment_parent_column, "comments should support root posts and replies");


    let first_attempt_timestamp_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM information_schema.columns
          WHERE table_schema='public' AND table_name='notification_email_outbox'
            AND column_name='first_provider_attempt_at' AND is_nullable='YES')",
    )
    .fetch_one(&pool)
    .await
    .expect("first provider attempt timestamp should be readable");
    assert!(first_attempt_timestamp_exists);

    // Database-level policy must block password-only inserts even when an old API
    // instance still exposes the legacy registration handler.
    let rejected_user_id = uuid::Uuid::new_v4();
    let rejected_email = format!("{}@example.test", uuid::Uuid::new_v4());
    let password_insert_error = sqlx::query(
        "INSERT INTO users(id,email,password_hash,full_name)
         VALUES($1,$2,'legacy-password-hash','Rejected Password User')",
    )
    .bind(rejected_user_id)
    .bind(rejected_email)
    .execute(&pool)
    .await
    .expect_err("password-only user inserts must be rejected by the database");
    assert_eq!(
        password_insert_error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("23514"),
        "password-only user insert should fail with a check-constraint SQLSTATE"
    );

    // A passwordless row is not enough by itself: require an OAuth identity in
    // the same transaction before it can commit.
    let unlinked_user_id = uuid::Uuid::new_v4();
    let unlinked_email = format!("{}@example.test", uuid::Uuid::new_v4());
    let mut unlinked_tx = pool.begin().await.expect("unlinked user transaction should start");
    sqlx::query(
        "INSERT INTO users(id,email,password_hash,full_name)
         VALUES($1,$2,NULL,'Unlinked Test User')",
    )
    .bind(unlinked_user_id)
    .bind(unlinked_email)
    .execute(&mut *unlinked_tx)
    .await
    .expect("passwordless insert should be deferred until transaction commit");
    let unlinked_commit_error = unlinked_tx
        .commit()
        .await
        .expect_err("passwordless users without OAuth identities must not commit");
    assert_eq!(
        unlinked_commit_error
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("23514"),
        "missing OAuth identity should fail with a check-constraint SQLSTATE"
    );

    // A verified OAuth identity may be inserted after its user in the same
    // transaction, matching the existing server flow and older OAuth binaries.
    let verified_user_id = uuid::Uuid::new_v4();
    let verified_email = format!("{}@example.test", uuid::Uuid::new_v4());
    let provider_user_id = format!("migration-test-{}", uuid::Uuid::new_v4());
    let mut verified_tx = pool.begin().await.expect("verified OAuth transaction should start");
    sqlx::query(
        "INSERT INTO users(id,email,password_hash,full_name)
         VALUES($1,$2,NULL,'Verified Provider Test User')",
    )
    .bind(verified_user_id)
    .bind(&verified_email)
    .execute(&mut *verified_tx)
    .await
    .expect("provider account user should be inserted");
    sqlx::query(
        "INSERT INTO oauth_accounts(provider,provider_user_id,user_id,email)
         VALUES('google',$1,$2,$3)",
    )
    .bind(&provider_user_id)
    .bind(verified_user_id)
    .bind(&verified_email)
    .execute(&mut *verified_tx)
    .await
    .expect("provider identity should be attached in the same transaction");
    verified_tx
        .commit()
        .await
        .expect("verified OAuth user should commit");

    let verified_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT email_verified_at FROM users WHERE id=$1")
            .bind(verified_user_id)
            .fetch_one(&pool)
            .await
            .expect("verified timestamp should be readable");
    assert!(verified_at.is_some(), "database guard should timestamp verified provider users");

    sqlx::query("DELETE FROM users WHERE id=$1")
        .bind(verified_user_id)
        .execute(&pool)
        .await
        .expect("verified OAuth migration fixture should be removed");

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

    // Email-only preferences must not leak into the in-app inbox or unread
    // count, while the independent email outbox remains eligible for delivery.
    let notification_id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO notification_preferences(user_id,notification_type,in_app_enabled,email_enabled)
         VALUES($1,'task_assigned',FALSE,TRUE)",
    )
    .bind(user_id)
    .execute(&mut *transaction)
    .await
    .expect("email-only preference should be inserted");
    sqlx::query(
        "INSERT INTO notifications(id,workspace_id,recipient_user_id,notification_type,title,body)
         VALUES($1,$2,$3,'task_assigned','Email-only update','This update should not appear in the in-app inbox')",
    )
    .bind(notification_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await
    .expect("email-only notification should be inserted");
    sqlx::query(
        "INSERT INTO notification_email_outbox(notification_id,recipient_user_id)
         VALUES($1,$2)",
    )
    .bind(notification_id)
    .bind(user_id)
    .execute(&mut *transaction)
    .await
    .expect("email-only outbox job should be inserted");

    let inbox_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM notifications n
         JOIN workspace_members wm ON wm.workspace_id=n.workspace_id AND wm.user_id=n.recipient_user_id
         WHERE n.recipient_user_id=$1
           AND COALESCE((SELECT np.in_app_enabled FROM notification_preferences np
                         WHERE np.user_id=n.recipient_user_id AND np.notification_type=n.notification_type),TRUE)",
    )
    .bind(user_id)
    .fetch_one(&mut *transaction)
    .await
    .expect("in-app inbox query should succeed");
    let unread_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM notifications n
         JOIN workspace_members wm ON wm.workspace_id=n.workspace_id AND wm.user_id=n.recipient_user_id
         WHERE n.recipient_user_id=$1 AND n.read_at IS NULL
           AND COALESCE((SELECT np.in_app_enabled FROM notification_preferences np
                         WHERE np.user_id=n.recipient_user_id AND np.notification_type=n.notification_type),TRUE)",
    )
    .bind(user_id)
    .fetch_one(&mut *transaction)
    .await
    .expect("in-app unread query should succeed");
    let email_job_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM notification_email_outbox
         WHERE notification_id=$1 AND recipient_user_id=$2 AND status='queued'",
    )
    .bind(notification_id)
    .bind(user_id)
    .fetch_one(&mut *transaction)
    .await
    .expect("email outbox should remain queryable");
    assert_eq!(inbox_count, 0, "email-only notifications must not appear in the in-app inbox");
    assert_eq!(unread_count, 0, "email-only notifications must not affect the unread count");
    assert_eq!(email_job_count, 1, "email-only notification delivery must remain queued");

    // The rollback-bridge version of the old chat query must continue to hide
    // private message rows after migration 0010 adds conversation_id.
    let other_user_id=uuid::Uuid::new_v4();
    let other_email=format!("{}@example.test",uuid::Uuid::new_v4());
    sqlx::query(
        "INSERT INTO users(id,email,password_hash,full_name)
         VALUES($1,$2,NULL,'Direct Message Fixture')",
    ).bind(other_user_id).bind(other_email)
     .execute(&mut *transaction).await
     .expect("direct-message participant should be created");
    sqlx::query(
        "INSERT INTO workspace_members(workspace_id,user_id,role) VALUES($1,$2,'member')",
    ).bind(workspace_id).bind(other_user_id)
     .execute(&mut *transaction).await
     .expect("direct-message participant should join the workspace");
    let (participant_low,participant_high)=if user_id<other_user_id {
        (user_id,other_user_id)
    } else {
        (other_user_id,user_id)
    };
    let conversation_id=uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO chat_conversations(id,workspace_id,participant_low,participant_high,created_by)
         VALUES($1,$2,$3,$4,$5)",
    ).bind(conversation_id).bind(workspace_id).bind(participant_low).bind(participant_high).bind(user_id)
     .execute(&mut *transaction).await
     .expect("private conversation should be inserted");
    sqlx::query(
        "INSERT INTO chat_messages(workspace_id,author_id,body)
         VALUES($1,$2,'Public channel fixture')",
    ).bind(workspace_id).bind(user_id)
     .execute(&mut *transaction).await
     .expect("public channel message should be inserted");
    sqlx::query(
        "INSERT INTO chat_messages(workspace_id,author_id,conversation_id,body)
         VALUES($1,$2,$3,'Private direct-message fixture')",
    ).bind(workspace_id).bind(user_id).bind(conversation_id)
     .execute(&mut *transaction).await
     .expect("private direct message should be inserted");
    let legacy_channel_count:i64=sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM chat_messages m
          WHERE m.workspace_id=$1
            AND COALESCE(to_jsonb(m)->>'conversation_id','')=''",
    ).bind(workspace_id).fetch_one(&mut *transaction).await
     .expect("rollback-compatible workspace channel filter should execute");
    assert_eq!(legacy_channel_count,1,"legacy workspace chat must not expose direct messages");

    transaction.rollback().await.expect("cascade fixture should roll back");

    pool.close().await;
}
