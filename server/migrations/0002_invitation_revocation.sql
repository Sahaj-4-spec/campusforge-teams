-- Invitation revocation and one active invitation per workspace/email.
ALTER TABLE invitations
    ADD COLUMN revoked_at TIMESTAMPTZ;

CREATE UNIQUE INDEX idx_invitations_one_pending_per_email
    ON invitations (workspace_id, lower(email))
    WHERE accepted_at IS NULL AND revoked_at IS NULL;
