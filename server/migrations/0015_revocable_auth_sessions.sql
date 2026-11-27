-- Server-side revocation for signed CampusForge sessions.
-- Session identifiers are carried as JWT jti claims; no raw session tokens are stored.
CREATE TABLE auth_sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT auth_sessions_expiry_after_creation CHECK (expires_at > created_at)
);

CREATE INDEX idx_auth_sessions_user_active
    ON auth_sessions(user_id, expires_at DESC)
    WHERE revoked_at IS NULL;

CREATE INDEX idx_auth_sessions_expiry
    ON auth_sessions(expires_at);

-- The FK cascade must also find revoked rows when a user account is deleted.
CREATE INDEX idx_auth_sessions_user_id
    ON auth_sessions(user_id);

-- The cleanup worker also deletes rows whose revocation timestamp is old.
CREATE INDEX idx_auth_sessions_revoked_cleanup
    ON auth_sessions(revoked_at)
    WHERE revoked_at IS NOT NULL;
