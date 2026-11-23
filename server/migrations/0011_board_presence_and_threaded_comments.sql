-- Board-level collaboration signals and threaded task discussions.
-- Presence remains connection-scoped so multiple tabs/devices for one member
-- do not incorrectly take each other offline.
ALTER TABLE workspace_presence
    ADD COLUMN board_id UUID REFERENCES boards(id) ON DELETE CASCADE;

CREATE INDEX idx_workspace_presence_board_active
    ON workspace_presence(board_id, last_seen_at DESC, user_id)
    WHERE board_id IS NOT NULL;

ALTER TABLE comments
    ADD COLUMN parent_comment_id UUID REFERENCES comments(id) ON DELETE CASCADE,
    ADD CONSTRAINT comments_cannot_reply_to_self
        CHECK (parent_comment_id IS NULL OR parent_comment_id <> id);

CREATE INDEX idx_comments_parent_created
    ON comments(parent_comment_id, created_at)
    WHERE parent_comment_id IS NOT NULL;
