-- CampusForge collaboration suite
-- Adds advanced task fields, checklist/labels/dependencies/recurrence,
-- calendar events, notifications, private file metadata, chat and meeting notes.

ALTER TABLE tasks
    ADD COLUMN IF NOT EXISTS parent_task_id UUID REFERENCES tasks(id) ON DELETE CASCADE,
    ADD COLUMN IF NOT EXISTS estimated_minutes INTEGER,
    ADD CONSTRAINT tasks_estimated_minutes_positive
        CHECK (estimated_minutes IS NULL OR estimated_minutes BETWEEN 1 AND 100000);

CREATE INDEX IF NOT EXISTS idx_tasks_parent
    ON tasks(parent_task_id) WHERE parent_task_id IS NOT NULL;

CREATE TABLE task_checklist_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    is_done BOOLEAN NOT NULL DEFAULT FALSE,
    completed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT checklist_title_nonempty CHECK (length(trim(title)) > 0),
    UNIQUE (task_id, position)
);
CREATE INDEX idx_task_checklist_task_position
    ON task_checklist_items(task_id, position);

CREATE TABLE labels (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT '#7771d9',
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT labels_name_nonempty CHECK (length(trim(name)) > 0),
    CONSTRAINT labels_color_format CHECK (color ~ '^#[0-9A-Fa-f]{6}$')
);
CREATE UNIQUE INDEX idx_labels_workspace_name
    ON labels(workspace_id, lower(name));

CREATE TABLE task_labels (
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    label_id UUID NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (task_id, label_id)
);
CREATE INDEX idx_task_labels_label ON task_labels(label_id);

CREATE TABLE task_dependencies (
    task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    depends_on_task_id UUID NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (task_id, depends_on_task_id),
    CONSTRAINT task_dependency_not_self CHECK (task_id <> depends_on_task_id)
);
CREATE INDEX idx_task_dependencies_depends_on
    ON task_dependencies(depends_on_task_id);

CREATE TABLE task_recurrence_rules (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_task_id UUID NOT NULL UNIQUE REFERENCES tasks(id) ON DELETE CASCADE,
    target_list_id UUID NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
    interval_days INTEGER NOT NULL,
    next_run_at TIMESTAMPTZ NOT NULL,
    time_zone TEXT NOT NULL DEFAULT 'UTC',
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT recurrence_interval_valid CHECK (interval_days BETWEEN 1 AND 365)
);
CREATE INDEX idx_task_recurrence_due
    ON task_recurrence_rules(next_run_at) WHERE is_active;

CREATE TABLE calendar_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    board_id UUID REFERENCES boards(id) ON DELETE CASCADE,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    title TEXT NOT NULL,
    description TEXT,
    event_type TEXT NOT NULL DEFAULT 'meeting',
    start_at TIMESTAMPTZ NOT NULL,
    end_at TIMESTAMPTZ NOT NULL,
    time_zone TEXT NOT NULL DEFAULT 'UTC',
    all_day BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT calendar_title_nonempty CHECK (length(trim(title)) > 0),
    CONSTRAINT calendar_type_valid CHECK (
        event_type IN ('meeting', 'milestone', 'exam', 'submission', 'focus_block')
    ),
    CONSTRAINT calendar_range_valid CHECK (end_at > start_at)
);
CREATE INDEX idx_calendar_workspace_start
    ON calendar_events(workspace_id, start_at);
CREATE INDEX idx_calendar_board_start
    ON calendar_events(board_id, start_at) WHERE board_id IS NOT NULL;

CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    recipient_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    actor_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    notification_type TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    resource_type TEXT,
    resource_id UUID,
    dedupe_key TEXT UNIQUE,
    read_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT notification_title_nonempty CHECK (length(trim(title)) > 0)
);
CREATE INDEX idx_notifications_recipient_created
    ON notifications(recipient_user_id, created_at DESC);
CREATE INDEX idx_notifications_unread
    ON notifications(recipient_user_id, created_at DESC) WHERE read_at IS NULL;

CREATE TABLE notification_preferences (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    notification_type TEXT NOT NULL,
    in_app_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    email_enabled BOOLEAN NOT NULL DEFAULT FALSE,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, notification_type)
);

CREATE TABLE workspace_files (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    board_id UUID REFERENCES boards(id) ON DELETE CASCADE,
    task_id UUID REFERENCES tasks(id) ON DELETE CASCADE,
    uploaded_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    original_name TEXT NOT NULL,
    storage_key TEXT NOT NULL UNIQUE,
    media_type TEXT NOT NULL,
    size_bytes BIGINT NOT NULL,
    checksum_sha256 TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT workspace_file_name_nonempty CHECK (length(trim(original_name)) > 0),
    CONSTRAINT workspace_file_size_valid CHECK (size_bytes BETWEEN 1 AND 10485760),
    CONSTRAINT workspace_file_scope CHECK (num_nonnulls(board_id, task_id) <= 1)
);
CREATE INDEX idx_workspace_files_workspace_created
    ON workspace_files(workspace_id, created_at DESC);
CREATE INDEX idx_workspace_files_task
    ON workspace_files(task_id) WHERE task_id IS NOT NULL;
CREATE INDEX idx_workspace_files_board
    ON workspace_files(board_id) WHERE board_id IS NOT NULL;

CREATE TABLE chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    board_id UUID REFERENCES boards(id) ON DELETE CASCADE,
    author_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    body TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT chat_message_nonempty CHECK (length(trim(body)) > 0),
    CONSTRAINT chat_message_size CHECK (length(body) <= 4000)
);
CREATE INDEX idx_chat_workspace_created
    ON chat_messages(workspace_id, created_at DESC);
CREATE INDEX idx_chat_board_created
    ON chat_messages(board_id, created_at DESC) WHERE board_id IS NOT NULL;

CREATE TABLE meeting_notes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    board_id UUID REFERENCES boards(id) ON DELETE CASCADE,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    title TEXT NOT NULL,
    meeting_at TIMESTAMPTZ,
    attendees TEXT NOT NULL DEFAULT '',
    agenda TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '',
    action_items TEXT NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT meeting_notes_title_nonempty CHECK (length(trim(title)) > 0)
);
CREATE INDEX idx_meeting_notes_workspace_date
    ON meeting_notes(workspace_id, meeting_at DESC NULLS LAST);
