-- Social sign-in accounts and one-time OAuth state tokens.
-- Passwords remain available for email/password accounts; social-only users have NULL password_hash.
ALTER TABLE users ALTER COLUMN password_hash DROP NOT NULL;

CREATE TABLE oauth_accounts (
    provider TEXT NOT NULL,
    provider_user_id TEXT NOT NULL,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (provider, provider_user_id),
    UNIQUE (user_id, provider),
    CONSTRAINT oauth_accounts_provider CHECK (provider IN ('google', 'github', 'discord')),
    CONSTRAINT oauth_accounts_email_nonempty CHECK (length(trim(email)) > 0)
);

CREATE INDEX idx_oauth_accounts_user ON oauth_accounts(user_id);

CREATE TABLE oauth_states (
    state_hash BYTEA PRIMARY KEY,
    provider TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT oauth_states_provider CHECK (provider IN ('google', 'github', 'discord'))
);

CREATE INDEX idx_oauth_states_expiry ON oauth_states(expires_at);
