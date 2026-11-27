import { useEffect, useRef, useState } from "react";
import {
  Activity, AlertTriangle, ArrowUpRight, CalendarClock, CheckCircle2,
  Clock3, Gauge, RefreshCw, ShieldAlert, Users, X
} from "lucide-react";
import { aiApi } from "../lib/api.js";

const formatHours = (minutes) => (Number(minutes || 0) / 60).toFixed(1) + "h";
const scoreTone = (score) => score >= 85 ? "healthy" : score >= 70 ? "watch" : score >= 50 ? "risk" : "critical";
const humanize = (value) => String(value || "unknown").replaceAll("_", " ");

function MetricCard({ label, value, detail, icon: Icon, tone = "neutral" }) {
  return (
    <article className={"pulse-metric pulse-metric-" + tone}>
      <span className="pulse-metric-icon"><Icon size={16} /></span>
      <strong>{value}</strong>
      <span className="pulse-metric-label">{label}</span>
      <small>{detail}</small>
    </article>
  );
}

export default function ProjectPulseDashboard({ onClose, boards = [], onOpenTask, refreshKey = 0 }) {
  const [boardId, setBoardId] = useState("");
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const panelRef = useRef(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    const previousFocus = document.activeElement;
    const previousOverflow = document.body.style.overflow;
    const panel = panelRef.current;
    document.body.style.overflow = "hidden";

    const focusable = () => panel?.querySelectorAll(
      'button:not([disabled]), select:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])'
    ) || [];
    focusable()[0]?.focus();
    if (!focusable().length) panel?.focus();

    const onKeyDown = (event) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onCloseRef.current?.();
        return;
      }
      if (event.key !== "Tab") return;
      const items = Array.from(focusable());
      if (!items.length) {
        event.preventDefault();
        panel?.focus();
        return;
      }
      const first = items[0];
      const last = items[items.length - 1];
      if (event.shiftKey && (document.activeElement === first || !panel?.contains(document.activeElement))) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && (document.activeElement === last || !panel?.contains(document.activeElement))) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", onKeyDown, true);
      if (previousFocus instanceof HTMLElement && previousFocus.isConnected) previousFocus.focus();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError("");
    aiApi.insights(boardId || null)
      .then((result) => { if (!cancelled) setData(result); })
      .catch((requestError) => {
        if (!cancelled) {
          setData(null);
          setError(requestError.message || "Project Pulse could not load.");
        }
      })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [boardId, refreshKey]);

  const closeOnBackdrop = (event) => {
    if (event.target === event.currentTarget) onClose();
  };

  return (
    <div className="pulse-backdrop" onMouseDown={closeOnBackdrop}>
      <section
        className="pulse-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby="pulse-title"
        ref={panelRef}
        tabIndex={-1}
      >
        <header className="pulse-header">
          <div className="pulse-heading">
            <span className="pulse-heading-icon"><Activity size={19} /></span>
            <div>
              <p className="pulse-eyebrow">CAMPUSFORGE INTELLIGENCE</p>
              <h2 id="pulse-title">Project Pulse</h2>
              <p>Evidence-based delivery health and next actions</p>
            </div>
          </div>
          <div className="pulse-header-actions">
            <button className="pulse-refresh" type="button" onClick={() => setBoardId((current) => current)} disabled={loading} aria-label="Refresh Project Pulse">
              <RefreshCw size={16} />
            </button>
            <button className="pulse-close" type="button" onClick={onClose} aria-label="Close Project Pulse"><X size={19} /></button>
          </div>
        </header>

        <div className="pulse-toolbar">
          <label>
            <span>Project scope</span>
            <select value={boardId} onChange={(event) => setBoardId(event.target.value)}>
              <option value="">All accessible projects</option>
              {boards.map((board) => <option key={board.id} value={board.id}>{board.name}</option>)}
            </select>
          </label>
          <span className="pulse-live-note"><span /> Calculated from live workspace records</span>
        </div>

        <div className="pulse-content" aria-live="polite">
          {loading ? (
            <div className="pulse-state"><RefreshCw className="pulse-spin" size={22} /><strong>Calculating project health…</strong></div>
          ) : error ? (
            <div className="pulse-error" role="alert"><AlertTriangle size={18} /><div><strong>Could not load Project Pulse</strong><p>{error}</p><button type="button" onClick={() => setBoardId((current) => current)}>Try again</button></div></div>
          ) : data ? (
            <>
              <section className="pulse-overview">
                <div className={"pulse-score pulse-score-" + scoreTone(data.health_score)}>
                  <div className="pulse-score-ring"><strong>{data.health_score}</strong><span>/100</span></div>
                  <div><span className="pulse-section-kicker">PROJECT HEALTH</span><h3>{humanize(data.health_label)}</h3><p>{data.open_tasks} open tasks · {data.completed_tasks} completed of {data.total_tasks}</p></div>
                </div>
                <div className="pulse-overview-note">
                  <Gauge size={17} />
                  <span>{data.sample_capped ? "Large workspace: the score uses a capped sample. Narrow to one project for a full project-level view." : "The score is a transparent heuristic based on overdue work, blockers, priority ownership, stale tasks, deadlines and missing estimates."}</span>
                </div>
              </section>

              <section className="pulse-metrics-grid" aria-label="Project health signals">
                <MetricCard icon={CalendarClock} label="Overdue" value={data.overdue_tasks} detail="Past due date" tone={data.overdue_tasks ? "danger" : "good"} />
                <MetricCard icon={ShieldAlert} label="Blocked" value={data.blocked_tasks} detail="Blocked or on hold" tone={data.blocked_tasks ? "danger" : "good"} />
                <MetricCard icon={Clock3} label="Due soon" value={data.due_soon_tasks} detail="Next three days" tone={data.due_soon_tasks ? "warning" : "neutral"} />
                <MetricCard icon={Users} label="Unassigned priority" value={data.unassigned_urgent_tasks} detail="High / urgent without owner" tone={data.unassigned_urgent_tasks ? "warning" : "good"} />
                <MetricCard icon={Activity} label="Stale" value={data.stale_tasks} detail="No update in seven days" tone={data.stale_tasks ? "warning" : "good"} />
                <MetricCard icon={CheckCircle2} label="No estimate" value={data.unestimated_tasks} detail="Effort not estimated" tone={data.unestimated_tasks ? "warning" : "good"} />
              </section>

              <section className="pulse-section">
                <div className="pulse-section-heading"><div><span className="pulse-section-kicker">PRIORITIZED WORK</span><h3>Next best actions</h3></div><span className="pulse-count">{data.focus_queue.length} signals</span></div>
                {data.focus_queue.length ? (
                  <div className="pulse-focus-list">
                    {data.focus_queue.map((item, index) => (
                      <article className="pulse-focus-item" key={item.task_id}>
                        <span className="pulse-focus-rank">{index + 1}</span>
                        <div className="pulse-focus-main">
                          <div className="pulse-focus-title-row"><strong>{item.title}</strong><span className={"pulse-priority pulse-priority-" + String(item.priority).toLowerCase()}>{item.priority}</span></div>
                          <p className="pulse-focus-context">{item.board_name} · {item.list_name}{item.due_at ? " · Due " + new Date(item.due_at).toLocaleDateString() : ""}</p>
                          <p className="pulse-focus-reason">{item.reason}</p>
                          <p className="pulse-recommendation">{item.recommendation}</p>
                          {item.assignees?.length > 0 && <small className="pulse-assignees">Owners: {item.assignees.join(", ")}</small>}
                        </div>
                        <button type="button" className="pulse-open-task" onClick={() => onOpenTask?.(item.task_id)} aria-label={"Open task " + item.title}><ArrowUpRight size={16} /></button>
                      </article>
                    ))}
                  </div>
                ) : <p className="pulse-empty">No high-priority risk signals were found in this scope.</p>}
              </section>

              <section className="pulse-two-columns">
                <div className="pulse-section">
                  <div className="pulse-section-heading"><div><span className="pulse-section-kicker">CAPACITY</span><h3>Workload balance</h3></div></div>
                  {data.workload.length ? (
                    <div className="pulse-workload-list">
                      {data.workload.map((member) => (
                        <div className="pulse-workload-row" key={member.member}>
                          <div className="pulse-workload-person"><strong>{member.member}</strong><span>{member.open_tasks} open · {member.unestimated_tasks} unestimated</span></div>
                          <div className="pulse-workload-value"><strong>{formatHours(member.estimated_minutes)}</strong><span className={"pulse-workload-tag pulse-workload-" + member.workload}>{humanize(member.workload)}</span></div>
                        </div>
                      ))}
                    </div>
                  ) : <p className="pulse-empty">No assigned open work in this scope.</p>}
                  <div className="pulse-unassigned"><span>Unassigned work</span><strong>{data.unassigned_tasks} tasks · {formatHours(data.unassigned_estimated_minutes)} estimated</strong></div>
                  <p className="pulse-footnote">Effort is split across multiple assignees; missing estimates are shown separately, not counted as zero work.</p>
                </div>

                <div className="pulse-section">
                  <div className="pulse-section-heading"><div><span className="pulse-section-kicker">PORTFOLIO</span><h3>Board health comparison</h3></div></div>
                  {data.board_health.length ? (
                    <div className="pulse-board-list">
                      {data.board_health.map((board) => (
                        <div className="pulse-board-row" key={board.board_id}>
                          <div className="pulse-board-row-top"><strong>{board.name}</strong><span className={"pulse-board-score pulse-score-text-" + scoreTone(board.health_score)}>{board.health_score}</span></div>
                          <div className="pulse-board-track"><i style={{ width: board.health_score + "%" }} /></div>
                          <small>{humanize(board.health_label)} · {board.overdue_tasks} overdue · {board.blocked_tasks} blocked · {board.completed_tasks}/{board.total_tasks} done</small>
                        </div>
                      ))}
                    </div>
                  ) : <p className="pulse-empty">No accessible projects found.</p>}
                </div>
              </section>
              <p className="pulse-methodology">{data.methodology}</p>
              <p className="pulse-generated">Updated {new Date(data.generated_at).toLocaleString()} · {data.scope === "selected_project" ? "Selected project" : "Accessible workspace projects"}</p>
            </>
          ) : <div className="pulse-state">No project data is available.</div>}
        </div>
      </section>
    </div>
  );
}
