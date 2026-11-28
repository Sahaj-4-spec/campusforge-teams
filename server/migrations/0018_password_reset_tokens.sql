-- One-time, hashed password reset tokens. Raw bearer tokens are never stored.
CREATE TABLE password_reset_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT password_reset_tokens_hash_length CHECK (octet_length(token_hash) = 32),
    CONSTRAINT password_reset_tokens_expiry CHECK (expires_at > created_at)
);

CREATE INDEX idx_password_reset_tokens_user_active
    ON password_reset_tokens(user_id, expires_at DESC)
    WHERE used_at IS NULL;

CREATE INDEX idx_password_reset_tokens_expiry
    ON password_reset_tokens(expires_at);
