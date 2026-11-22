-- Fence notification delivery attempts and retain ambiguous sends for manual reconciliation.
ALTER TABLE notification_email_outbox
    DROP CONSTRAINT email_outbox_status_valid;

ALTER TABLE notification_email_outbox
    ADD CONSTRAINT email_outbox_status_valid
    CHECK (status IN ('queued', 'processing', 'sent', 'failed', 'suppressed', 'reconciliation'));

ALTER TABLE notification_email_outbox
    ADD COLUMN first_provider_attempt_at TIMESTAMPTZ;

CREATE INDEX idx_email_outbox_expired_processing
    ON notification_email_outbox(locked_at)
    WHERE status = 'processing';
