import { CheckCircle2, Clock3, FileCheck2, LoaderCircle, X } from "lucide-react";

function prettyDate(value) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "Unknown expiry" : date.toLocaleString();
}

function statusLabel(status) {
  return String(status || "pending").replaceAll("_", " ");
}

export default function ProposalReviewDrawer({
  proposals = [],
  auditEvents = [],
  selectedProposalId,
  onSelect,
  onClose,
  onApply,
  onReject,
  working = false,
  error = ""
}) {
  const selected = proposals.find((item) => item.id === selectedProposalId) || null;
  const pendingCount = proposals.filter((item) => item.status === "pending").length;

  return (
    <div
      className="ai-proposal-review-backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !working) onClose?.();
      }}
    >
      <section className="ai-proposal-review-panel" role="dialog" aria-modal="true" aria-labelledby="ai-proposal-review-title">
        <header className="ai-proposal-review-header">
          <div>
            <p className="eyebrow">HUMAN APPROVAL</p>
            <h2 id="ai-proposal-review-title">Review task proposals</h2>
            <p>{pendingCount} awaiting review · proposals expire after 24 hours</p>
          </div>
          <button type="button" className="ai-close-button" onClick={onClose} disabled={working} aria-label="Close proposal review">
            <X size={18} />
          </button>
        </header>

        <div className="ai-proposal-review-layout">
          <aside className="ai-proposal-review-list" aria-label="Saved proposals">
            {proposals.length === 0 ? (
              <div className="ai-proposal-review-empty">No saved proposals yet. Generate a task plan and save it for approval.</div>
            ) : proposals.map((proposal) => (
              <button
                type="button"
                key={proposal.id}
                className={"ai-proposal-review-list-item" + (selectedProposalId === proposal.id ? " active" : "")}
                onClick={() => onSelect?.(proposal.id)}
                aria-pressed={selectedProposalId === proposal.id}
              >
                <span className="ai-proposal-review-list-title">{proposal.tasks?.length || 0} proposed tasks</span>
                <span className={"ai-proposal-status status-" + proposal.status}>{statusLabel(proposal.status)}</span>
                <small>Created {prettyDate(proposal.created_at)}</small>
              </button>
            ))}
          </aside>

          <div className="ai-proposal-review-detail">
            {!selected ? (
              <div className="ai-proposal-review-empty">Select a saved proposal to inspect its task details.</div>
            ) : (
              <>
                <div className="ai-proposal-review-detail-heading">
                  <div>
                    <h3>Proposed work</h3>
                    <p>Project ID: {selected.board_id}</p>
                  </div>
                  <span className={"ai-proposal-status status-" + selected.status}>{statusLabel(selected.status)}</span>
                </div>
                <p className="ai-proposal-review-expiry"><Clock3 size={14} /> Expires {prettyDate(selected.expires_at)}</p>
                <div className="ai-proposal-review-tasks">
                  {(selected.tasks || []).map((task, index) => (
                    <article className="ai-proposal-review-task" key={selected.id + "-" + index}>
                      <div className="ai-proposal-review-task-heading">
                        <strong>{index + 1}. {task.title}</strong>
                        <span className={"ai-priority ai-priority-" + (["low", "medium", "high"].includes(String(task.priority).toLowerCase()) ? String(task.priority).toLowerCase() : "medium")}>{task.priority}</span>
                      </div>
                      {task.description && <p>{task.description}</p>}
                      <div className="ai-proposal-meta">
                        {task.estimated_minutes && <span>{task.estimated_minutes} min estimate</span>}
                        {task.source_refs?.length > 0 && <span>Evidence: {task.source_refs.slice(0, 4).join(", ")}</span>}
                        {task.assignee_ids?.length > 0 && <span>{task.assignee_ids.length} proposed assignee(s)</span>}
                      </div>
                    </article>
                  ))}
                </div>
                {selected.status === "pending" && (
                  <div className="ai-proposal-review-actions">
                    <button type="button" className="ai-proposal-reject" onClick={() => onReject?.(selected.id)} disabled={working}>
                      Reject proposal
                    </button>
                    <button type="button" className="ai-proposal-approve" onClick={() => onApply?.(selected.id)} disabled={working}>
                      {working ? <><LoaderCircle size={15} className="ai-spin" /> Applying…</> : <><CheckCircle2 size={15} /> Approve and create {selected.tasks?.length || 0} tasks</>}
                    </button>
                  </div>
                )}
                {selected.status === "applied" && (
                  <p className="ai-proposal-review-success"><CheckCircle2 size={15} /> This proposal has been applied. {selected.applied_task_ids?.length || 0} task records were created.</p>
                )}
                <p className="ai-proposal-review-safety"><FileCheck2 size={14} /> Approval rechecks your current permissions, project membership, destination column, assignees, and board state. All tasks are created in one database transaction.</p>
                <section className="ai-proposal-audit" aria-label="AI proposal audit history">
                  <h4>Recent approval history</h4>
                  {auditEvents.filter((event) => event.proposal_id === selected.id).length === 0 ? (
                    <p>No review events recorded yet.</p>
                  ) : auditEvents.filter((event) => event.proposal_id === selected.id).slice(0, 8).map((event) => (
                    <div className="ai-proposal-audit-event" key={event.id}>
                      <strong>{statusLabel(event.event_type)}</strong>
                      <span>{prettyDate(event.created_at)}</span>
                      {event.metadata?.task_count != null && <small>{event.metadata.task_count} task(s)</small>}
                    </div>
                  ))}
                </section>
              </>
            )}
            {error && <p className="ai-proposal-review-error" role="alert">{error}</p>}
          </div>
        </div>
      </section>
    </div>
  );
}
