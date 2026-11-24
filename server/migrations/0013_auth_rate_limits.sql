-- Shared, atomic fixed-window limits for authentication endpoints.
CREATE TABLE api_rate_limits (
    bucket_key TEXT PRIMARY KEY CHECK (length(bucket_key) = 64),
    window_started_at TIMESTAMPTZ NOT NULL,
    request_count INTEGER NOT NULL CHECK (request_count >= 1)
);

CREATE INDEX idx_api_rate_limits_window_started_at ON api_rate_limits(window_started_at);
