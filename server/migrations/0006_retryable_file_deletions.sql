-- Retryable workspace object deletion: retain metadata until object bytes are removed.
ALTER TABLE workspace_files
    ADD COLUMN is_deleting BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN delete_after TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ADD COLUMN delete_attempts INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN delete_error TEXT;

CREATE INDEX idx_workspace_files_pending_delete
    ON workspace_files(delete_after, created_at)
    WHERE is_deleting = TRUE;
