-- Reliable notification email queue and database-enforced task hierarchy boundaries.

CREATE TABLE notification_email_outbox (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    notification_id UUID NOT NULL UNIQUE REFERENCES notifications(id) ON DELETE CASCADE,
    recipient_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'queued',
    attempts INTEGER NOT NULL DEFAULT 0,
    available_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    locked_at TIMESTAMPTZ,
    sent_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT email_outbox_status_valid CHECK (status IN ('queued', 'processing', 'sent', 'failed')),
    CONSTRAINT email_outbox_attempts_valid CHECK (attempts BETWEEN 0 AND 10)
);
CREATE INDEX idx_email_outbox_ready
    ON notification_email_outbox(available_at, created_at)
    WHERE status = 'queued';

CREATE OR REPLACE FUNCTION campusforge_validate_task_parent()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    child_board_id UUID;
    parent_board_id UUID;
    creates_cycle BOOLEAN;
BEGIN
    IF NEW.parent_task_id IS NULL THEN
        RETURN NEW;
    END IF;

    IF NEW.parent_task_id = NEW.id THEN
        RAISE EXCEPTION 'A task cannot be its own parent'
            USING ERRCODE = '23514';
    END IF;

    SELECT board_id INTO child_board_id FROM lists WHERE id = NEW.list_id;
    SELECT l.board_id INTO parent_board_id
      FROM tasks parent_task
      JOIN lists l ON l.id = parent_task.list_id
     WHERE parent_task.id = NEW.parent_task_id;

    IF child_board_id IS NULL OR parent_board_id IS NULL OR child_board_id <> parent_board_id THEN
        RAISE EXCEPTION 'Parent and child tasks must belong to the same project'
            USING ERRCODE = '23514';
    END IF;

    WITH RECURSIVE parent_chain(task_id, parent_task_id) AS (
        SELECT id, parent_task_id
          FROM tasks
         WHERE id = NEW.parent_task_id
        UNION ALL
        SELECT parent.id, parent.parent_task_id
          FROM tasks parent
          JOIN parent_chain chain ON parent.id = chain.parent_task_id
         WHERE chain.parent_task_id IS NOT NULL
    )
    SELECT EXISTS(SELECT 1 FROM parent_chain WHERE task_id = NEW.id)
      INTO creates_cycle;

    IF creates_cycle THEN
        RAISE EXCEPTION 'Task hierarchy cannot contain cycles'
            USING ERRCODE = '23514';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_tasks_validate_parent
BEFORE INSERT OR UPDATE OF parent_task_id, list_id
ON tasks
FOR EACH ROW
EXECUTE FUNCTION campusforge_validate_task_parent();
