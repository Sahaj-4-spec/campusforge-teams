-- Retain applied/rejected/expired proposal records after a target column is removed.
-- Only the destination reference is cleared; board and proposal/audit history remain.
ALTER TABLE ai_action_proposals
    ALTER COLUMN target_list_id DROP NOT NULL;

ALTER TABLE ai_action_proposals
    DROP CONSTRAINT ai_action_proposals_target_list_fk;

ALTER TABLE ai_action_proposals
    ADD CONSTRAINT ai_action_proposals_target_list_fk
    FOREIGN KEY (target_list_id, board_id)
    REFERENCES lists(id, board_id)
    ON DELETE SET NULL (target_list_id);
