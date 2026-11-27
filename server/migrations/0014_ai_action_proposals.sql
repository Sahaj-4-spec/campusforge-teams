-- Durable human-approved AI task proposals.
-- Deliberately stores structured proposed tasks only; never persist prompts or model responses.

CREATE TABLE ai_action_proposals (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    board_id UUID NOT NULL,
    target_list_id UUID NOT NULL,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    status TEXT NOT NULL DEFAULT 'pending',
    tasks JSONB NOT NULL,
    source_fingerprint TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '24 hours'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    reviewed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    reviewed_at TIMESTAMPTZ,
    applied_task_ids UUID[] NOT NULL DEFAULT '{}'::UUID[],
    CONSTRAINT ai_action_proposals_status_valid
        CHECK (status IN ('pending', 'applied', 'rejected', 'stale', 'expired')),
    CONSTRAINT ai_action_proposals_tasks_valid
        CHECK (
            CASE
                WHEN jsonb_typeof(tasks) = 'array'
                THEN jsonb_array_length(tasks) BETWEEN 1 AND 8
                ELSE FALSE
            END
        ),
    CONSTRAINT ai_action_proposals_fingerprint_valid
        CHECK (source_fingerprint ~ '^[0-9a-f]{64}$'),
    CONSTRAINT ai_action_proposals_expiry_valid
        CHECK (expires_at > created_at),
    CONSTRAINT ai_action_proposals_board_scope_fk
        FOREIGN KEY (board_id, workspace_id)
        REFERENCES boards(id, workspace_id) ON DELETE CASCADE,
    CONSTRAINT ai_action_proposals_target_list_fk
        FOREIGN KEY (target_list_id, board_id)
        REFERENCES lists(id, board_id) ON DELETE CASCADE
);

CREATE INDEX idx_ai_action_proposals_workspace_created
    ON ai_action_proposals(workspace_id, created_at DESC);
CREATE INDEX idx_ai_action_proposals_board_status_created
    ON ai_action_proposals(board_id, status, created_at DESC);
CREATE INDEX idx_ai_action_proposals_expiry
    ON ai_action_proposals(expires_at) WHERE status = 'pending';

CREATE TABLE ai_action_audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    board_id UUID NOT NULL,
    proposal_id UUID REFERENCES ai_action_proposals(id) ON DELETE SET NULL,
    actor_id UUID REFERENCES users(id) ON DELETE SET NULL,
    event_type TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT ai_action_audit_event_type_valid
        CHECK (event_type IN ('created', 'applied', 'rejected', 'stale', 'expired')),
    CONSTRAINT ai_action_audit_metadata_object
        CHECK (jsonb_typeof(metadata) = 'object'),
    CONSTRAINT ai_action_audit_board_scope_fk
        FOREIGN KEY (board_id, workspace_id)
        REFERENCES boards(id, workspace_id) ON DELETE CASCADE
);

CREATE INDEX idx_ai_action_audit_workspace_created
    ON ai_action_audit_events(workspace_id, created_at DESC);
CREATE INDEX idx_ai_action_audit_board_created
    ON ai_action_audit_events(board_id, created_at DESC);
CREATE INDEX idx_ai_action_audit_proposal_created
    ON ai_action_audit_events(proposal_id, created_at DESC);
