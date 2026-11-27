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
