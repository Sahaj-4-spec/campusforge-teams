-- Make email opt-out effective for jobs already in the queue.
ALTER TABLE notification_email_outbox
    DROP CONSTRAINT email_outbox_status_valid;
ALTER TABLE notification_email_outbox
    ADD CONSTRAINT email_outbox_status_valid
    CHECK (status IN ('queued', 'processing', 'sent', 'failed', 'suppressed'));

-- Keep object keys durable when workspace/board/task cascades delete file metadata.
ALTER TABLE workspace_files
    ADD COLUMN object_delete_confirmed BOOLEAN NOT NULL DEFAULT FALSE;

CREATE TABLE object_deletion_outbox (
    storage_key TEXT PRIMARY KEY,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT object_deletion_attempts_nonnegative CHECK (attempts >= 0)
);

CREATE INDEX idx_object_deletion_outbox_due
    ON object_deletion_outbox(next_attempt_at, created_at);

CREATE OR REPLACE FUNCTION campusforge_queue_file_object_deletion()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF NOT OLD.object_delete_confirmed THEN
        INSERT INTO object_deletion_outbox(storage_key)
        VALUES (OLD.storage_key)
        ON CONFLICT (storage_key) DO NOTHING;
    END IF;
    RETURN OLD;
END;
$$;

CREATE TRIGGER trg_workspace_files_queue_object_delete
AFTER DELETE ON workspace_files
FOR EACH ROW
EXECUTE FUNCTION campusforge_queue_file_object_deletion();
