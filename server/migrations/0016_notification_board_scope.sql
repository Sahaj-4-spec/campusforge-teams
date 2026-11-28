-- Preserve the board scope of notifications so board-restricted activity
-- cannot be read or emailed to unrelated workspace members.
ALTER TABLE notifications
    ADD COLUMN board_id UUID REFERENCES boards(id) ON DELETE CASCADE;

-- Recover board scope for existing task notifications where the task still exists.
UPDATE notifications n
   SET board_id = b.id
  FROM tasks t
  JOIN lists l ON l.id = t.list_id
  JOIN boards b ON b.id = l.board_id
 WHERE n.resource_type = 'task'
   AND n.resource_id = t.id
   AND n.workspace_id = b.workspace_id;

CREATE INDEX idx_notifications_recipient_board_created
    ON notifications(recipient_user_id, board_id, created_at DESC)
    WHERE board_id IS NOT NULL;
