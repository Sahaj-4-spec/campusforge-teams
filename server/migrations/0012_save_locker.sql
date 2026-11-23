-- Personal Save Locker: only client-encrypted payloads are persisted.
CREATE TABLE save_locker_vaults (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    salt TEXT NOT NULL CHECK (length(salt) = 24),
    kdf_iterations INTEGER NOT NULL CHECK (kdf_iterations >= 600000),
    key_check_iv TEXT NOT NULL CHECK (length(key_check_iv) = 16),
    key_check_ciphertext TEXT NOT NULL CHECK (length(key_check_ciphertext) BETWEEN 24 AND 512),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE save_locker_items (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES save_locker_vaults(user_id) ON DELETE CASCADE,
    iv TEXT NOT NULL CHECK (length(iv) = 16),
    ciphertext TEXT NOT NULL CHECK (length(ciphertext) BETWEEN 24 AND 9437184),
    size_bytes BIGINT NOT NULL CHECK (size_bytes BETWEEN 24 AND 9437184),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_save_locker_items_user_updated
    ON save_locker_items(user_id, updated_at DESC);
