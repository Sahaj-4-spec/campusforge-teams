-- Complete the non-AI collaboration roadmap: calendar invitations/reminders,
-- file governance, historical analytics, direct messages, and structured actions.

ALTER TABLE calendar_events
    ADD COLUMN reminder_minutes INTEGER,
    ADD COLUMN all_day_start_date DATE,
    ADD COLUMN all_day_end_date DATE,
    ADD CONSTRAINT calendar_reminder_minutes_valid
        CHECK (reminder_minutes IS NULL OR reminder_minutes IN (0, 5, 10, 15, 30, 60, 1440)),
    ADD CONSTRAINT calendar_all_day_range_valid
        CHECK (NOT all_day OR (all_day_start_date IS NOT NULL AND all_day_end_date IS NOT NULL AND all_day_end_date >= all_day_start_date));

UPDATE calendar_events
   SET all_day_start_date = (start_at AT TIME ZONE COALESCE(NULLIF(time_zone,''),'UTC'))::date,
       all_day_end_date = ((end_at - INTERVAL '1 second') AT TIME ZONE COALESCE(NULLIF(time_zone,''),'UTC'))::date
 WHERE all_day;

CREATE TABLE calendar_event_attendees (
    event_id UUID NOT NULL REFERENCES calendar_events(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    response TEXT NOT NULL DEFAULT 'invited',
    reminder_sent_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (event_id, user_id),
    CONSTRAINT calendar_attendee_response_valid
        CHECK (response IN ('invited', 'accepted', 'declined'))
);
CREATE INDEX idx_calendar_attendees_user ON calendar_event_attendees(user_id, event_id);

ALTER TABLE workspace_files
    ADD COLUMN scan_status TEXT NOT NULL DEFAULT 'unscanned',
    ADD CONSTRAINT workspace_file_scan_status_valid
        CHECK (scan_status IN ('pending', 'clean', 'infected', 'unscanned'));

CREATE TABLE workspace_storage_quotas (
    workspace_id UUID PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
    quota_bytes BIGINT NOT NULL DEFAULT 104857600,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT workspace_storage_quota_positive CHECK (quota_bytes BETWEEN 10485760 AND 107374182400)
);
INSERT INTO workspace_storage_quotas(workspace_id, quota_bytes)
SELECT id, 104857600 FROM workspaces
ON CONFLICT (workspace_id) DO NOTHING;

CREATE TABLE workspace_storage_reservations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    size_bytes BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT workspace_storage_reservation_size CHECK (size_bytes BETWEEN 1 AND 10485760)
);
CREATE INDEX idx_workspace_storage_reservations_workspace
    ON workspace_storage_reservations(workspace_id, created_at);

CREATE TABLE file_audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    file_id UUID,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    action TEXT NOT NULL,
    original_name TEXT NOT NULL,
    size_bytes BIGINT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT file_audit_action_valid
        CHECK (action IN ('uploaded', 'downloaded', 'previewed', 'deletion_queued', 'deleted', 'scan_rejected'))
);
CREATE INDEX idx_file_audit_workspace_created ON file_audit_events(workspace_id, created_at DESC);

ALTER TABLE chat_messages
    ADD COLUMN conversation_id UUID,
    ADD COLUMN is_pinned BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN edited_at TIMESTAMPTZ,
    ADD COLUMN deleted_at TIMESTAMPTZ,
    ADD CONSTRAINT chat_message_channel_xor
        CHECK (NOT (board_id IS NOT NULL AND conversation_id IS NOT NULL));

CREATE TABLE chat_conversations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    participant_low UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    participant_high UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT direct_message_distinct_participants CHECK (participant_low < participant_high),
    UNIQUE (workspace_id, participant_low, participant_high)
);
ALTER TABLE chat_messages
    ADD CONSTRAINT chat_message_conversation_fk
    FOREIGN KEY (conversation_id) REFERENCES chat_conversations(id) ON DELETE CASCADE;
CREATE INDEX idx_chat_messages_conversation_created
    ON chat_messages(conversation_id, created_at DESC) WHERE conversation_id IS NOT NULL;
CREATE INDEX idx_chat_messages_pinned
    ON chat_messages(workspace_id, board_id, created_at DESC) WHERE is_pinned;

CREATE TABLE chat_message_mentions (
    message_id UUID NOT NULL REFERENCES chat_messages(id) ON DELETE CASCADE,
    mentioned_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (message_id, mentioned_user_id)
);
CREATE INDEX idx_chat_mentions_user_created
    ON chat_message_mentions(mentioned_user_id, created_at DESC);

CREATE TABLE meeting_note_action_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    meeting_note_id UUID NOT NULL REFERENCES meeting_notes(id) ON DELETE CASCADE,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    assigned_to UUID REFERENCES users(id) ON DELETE SET NULL,
    due_at TIMESTAMPTZ,
    linked_task_id UUID REFERENCES tasks(id) ON DELETE SET NULL,
    is_done BOOLEAN NOT NULL DEFAULT FALSE,
    position INTEGER NOT NULL DEFAULT 0,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT meeting_action_title_nonempty CHECK (length(trim(title)) > 0)
);
CREATE INDEX idx_meeting_action_note_position
    ON meeting_note_action_items(meeting_note_id, position);

CREATE TABLE workspace_analytics_daily (
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    snapshot_date DATE NOT NULL,
    total_tasks BIGINT NOT NULL,
    completed_tasks BIGINT NOT NULL,
    overdue_tasks BIGINT NOT NULL,
    unassigned_tasks BIGINT NOT NULL,
    active_projects BIGINT NOT NULL,
    completed_projects BIGINT NOT NULL,
    captured_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (workspace_id, snapshot_date),
    CONSTRAINT analytics_counts_nonnegative CHECK (
        total_tasks >= 0 AND completed_tasks >= 0 AND overdue_tasks >= 0
        AND unassigned_tasks >= 0 AND active_projects >= 0 AND completed_projects >= 0
    )
);
CREATE INDEX idx_workspace_analytics_date
    ON workspace_analytics_daily(snapshot_date DESC, workspace_id);

CREATE TABLE task_recurrence_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    recurrence_rule_id UUID NOT NULL REFERENCES task_recurrence_rules(id) ON DELETE CASCADE,
    scheduled_for TIMESTAMPTZ NOT NULL,
    generated_task_id UUID NOT NULL UNIQUE REFERENCES tasks(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (recurrence_rule_id, scheduled_for)
);

-- Ephemeral presence leases. The connection ID keeps multiple sessions on
-- different devices/tabs from incorrectly marking a member offline.
CREATE TABLE workspace_presence (
    connection_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_workspace_presence_active
    ON workspace_presence(workspace_id, last_seen_at DESC, user_id);
