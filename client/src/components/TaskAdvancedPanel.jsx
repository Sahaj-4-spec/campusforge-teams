import { useEffect, useRef, useState } from "react";
import { Check, Plus, Repeat2, Tag, Trash2, X } from "lucide-react";
import { advancedTaskApi } from "../lib/api.js";

export default function TaskAdvancedPanel({ task, lists, tasks, onChanged }) {
  const [details, setDetails] = useState({ subtasks: [], checklist: [], labels: [], dependencies: [], recurrence: null });
  const [availableLabels, setAvailableLabels] = useState([]);
  const [subtaskTitle, setSubtaskTitle] = useState("");
  const [checklistTitle, setChecklistTitle] = useState("");
  const [labelName, setLabelName] = useState("");
  const [labelColor, setLabelColor] = useState("#7771d9");
  const [dependencyId, setDependencyId] = useState("");
  const [intervalDays, setIntervalDays] = useState("");
  const [targetListId, setTargetListId] = useState("");
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const loadRequestRef = useRef(0);

  async function load() {
    if (!task?.id) return false;
    const requestId = ++loadRequestRef.current;
    setLoading(true);
    setError("");
    try {
      const [advanced, labels] = await Promise.all([advancedTaskApi.details(task.id), advancedTaskApi.labels()]);
      if (requestId !== loadRequestRef.current) return false;
      setDetails(advanced);
      setAvailableLabels(labels);
      setIntervalDays(advanced.recurrence?.interval_days ? String(advanced.recurrence.interval_days) : "");
      setTargetListId(advanced.recurrence?.target_list_id || lists[0]?.id || "");
      return true;
    } catch (requestError) {
      if (requestId === loadRequestRef.current) {
        setError(requestError.message || "Could not load advanced task details.");
      }
      return false;
    } finally {
      if (requestId === loadRequestRef.current) setLoading(false);
    }
  }

  useEffect(() => {
    load();
    return () => { loadRequestRef.current += 1; };
  }, [task?.id]);

  async function run(action, successMessage) {
    setBusy(true);
    setError("");
    setMessage("");
    try {
      await action();
      setMessage(successMessage);
      await load();
      onChanged?.();
      return true;
    } catch (requestError) {
      setError(requestError.message || "The task update could not be saved.");
      return false;
    } finally {
      setBusy(false);
    }
  }

  async function createSubtask(event) {
    event.preventDefault();
    if (!subtaskTitle.trim()) return;
    const succeeded = await run(() => advancedTaskApi.createSubtask(task.id, { title: subtaskTitle.trim() }), "Subtask added");
    if (succeeded) setSubtaskTitle("");
  }

  async function createChecklistItem(event) {
    event.preventDefault();
    if (!checklistTitle.trim()) return;
    const succeeded = await run(() => advancedTaskApi.addChecklistItem(task.id, checklistTitle.trim()), "Checklist item added");
    if (succeeded) setChecklistTitle("");
  }

  async function createLabel(event) {
    event.preventDefault();
    if (!labelName.trim()) return;
    const succeeded = await run(() => advancedTaskApi.createLabel({ name: labelName.trim(), color: labelColor }), "Label created");
    if (succeeded) setLabelName("");
  }

  async function saveRecurrence(event) {
    event.preventDefault();
    await run(() => advancedTaskApi.updateRecurrence(task.id, intervalDays ? {
      interval_days: Number(intervalDays),
      target_list_id: targetListId,
      time_zone: Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC"
    } : { interval_days: null }), intervalDays ? "Repeat rule saved" : "Repeat rule removed");
  }

  const usedLabelIds = new Set(details.labels.map((label) => label.id));
  const unusedLabels = availableLabels.filter((label) => !usedLabelIds.has(label.id));
  const dependencyCandidates = tasks.filter((item) => item.id !== task.id && item.board_id === task.board_id && !details.dependencies.some((dep) => dep.depends_on_task_id === item.id));

  return (
    <section className="feature-task-advanced">
      <div className="feature-task-advanced-heading"><div><p className="eyebrow">ADVANCED TASKS</p><h3>Break work into manageable steps</h3></div><span className="feature-chip">Task tools</span></div>
      {loading && <p className="feature-muted">Loading task details…</p>}
      {error && <p className="auth-error" role="alert">{error}</p>}
      {message && <p className="feature-success" role="status">{message}</p>}

      <div className="feature-advanced-grid">
        <section className="feature-advanced-card">
          <div className="feature-advanced-title"><div><strong>Subtasks</strong><small>Split this task into smaller pieces</small></div><span>{details.subtasks.length}</span></div>
          <form className="feature-inline-add" onSubmit={createSubtask}><input value={subtaskTitle} onChange={(event) => setSubtaskTitle(event.target.value)} placeholder="Add a subtask…" maxLength={180} /><button type="submit" disabled={busy || !subtaskTitle.trim()} aria-label="Add subtask"><Plus size={16} /></button></form>
          <div className="feature-advanced-list">
            {details.subtasks.map((subtask) => <div className="feature-advanced-row" key={subtask.id}>
              <input type="checkbox" checked={subtask.is_done} onChange={(event) => run(() => advancedTaskApi.updateSubtask(subtask.id, { is_done: event.target.checked }), "Subtask updated")} aria-label={"Complete " + subtask.title} />
              <span className={subtask.is_done ? "completed" : ""}>{subtask.title}</span>
              <button type="button" onClick={() => run(() => advancedTaskApi.removeSubtask(subtask.id), "Subtask removed")} aria-label={"Delete " + subtask.title}><Trash2 size={13} /></button>
            </div>)}
            {!details.subtasks.length && <p className="feature-empty-line">No subtasks yet.</p>}
          </div>
        </section>

        <section className="feature-advanced-card">
          <div className="feature-advanced-title"><div><strong>Checklist</strong><small>Track small completion steps</small></div><span>{details.checklist.filter((item) => item.is_done).length}/{details.checklist.length}</span></div>
          <form className="feature-inline-add" onSubmit={createChecklistItem}><input value={checklistTitle} onChange={(event) => setChecklistTitle(event.target.value)} placeholder="Add a checklist item…" maxLength={240} /><button type="submit" disabled={busy || !checklistTitle.trim()} aria-label="Add checklist item"><Plus size={16} /></button></form>
          <div className="feature-advanced-list">
            {details.checklist.map((item) => <div className="feature-advanced-row" key={item.id}>
              <input type="checkbox" checked={item.is_done} onChange={(event) => run(() => advancedTaskApi.updateChecklistItem(item.id, { is_done: event.target.checked }), "Checklist updated")} aria-label={"Complete " + item.title} />
              <span className={item.is_done ? "completed" : ""}>{item.title}</span>
              <button type="button" onClick={() => run(() => advancedTaskApi.removeChecklistItem(item.id), "Checklist item removed")} aria-label={"Delete " + item.title}><Trash2 size={13} /></button>
            </div>)}
            {!details.checklist.length && <p className="feature-empty-line">No checklist items yet.</p>}
          </div>
        </section>

        <section className="feature-advanced-card">
          <div className="feature-advanced-title"><div><strong>Labels</strong><small>Organize tasks by topic or type</small></div><Tag size={16} /></div>
          <div className="feature-label-list">
            {details.labels.map((label) => <button key={label.id} type="button" className="feature-label-pill" style={{ "--label-color": label.color }} onClick={() => run(() => advancedTaskApi.removeLabel(task.id, label.id), "Label removed")}>{label.name}<X size={12} /></button>)}
            {!details.labels.length && <p className="feature-empty-line">No labels attached.</p>}
          </div>
          {unusedLabels.length > 0 && <div className="feature-inline-add"><select aria-label="Add existing label" value="" onChange={(event) => { const id = event.target.value; if (id) run(() => advancedTaskApi.addLabel(task.id, id), "Label added"); }}><option value="">Add existing label…</option>{unusedLabels.map((label) => <option key={label.id} value={label.id}>{label.name}</option>)}</select></div>}
          <form className="feature-label-create" onSubmit={createLabel}><input value={labelName} onChange={(event) => setLabelName(event.target.value)} placeholder="New label" maxLength={50} /><input type="color" value={labelColor} onChange={(event) => setLabelColor(event.target.value)} aria-label="Label color" /><button type="submit" disabled={busy || !labelName.trim()}><Plus size={14} /> Create</button></form>
        </section>

        <section className="feature-advanced-card">
          <div className="feature-advanced-title"><div><strong>Dependencies</strong><small>Show what must be finished first</small></div><span>{details.dependencies.length}</span></div>
          <div className="feature-advanced-list">
            {details.dependencies.map((dependency) => <div className="feature-dependency-row" key={dependency.depends_on_task_id}><span><strong>{dependency.depends_on_title}</strong><small>{dependency.depends_on_status}</small></span><button type="button" onClick={() => run(() => advancedTaskApi.removeDependency(task.id, dependency.depends_on_task_id), "Dependency removed")} aria-label="Remove dependency"><X size={13} /></button></div>)}
            {!details.dependencies.length && <p className="feature-empty-line">No task dependencies.</p>}
          </div>
          {dependencyCandidates.length > 0 && <div className="feature-inline-add"><select aria-label="Select blocking task" value={dependencyId} onChange={(event) => setDependencyId(event.target.value)}><option value="">Select a blocking task…</option>{dependencyCandidates.map((item) => <option key={item.id} value={item.id}>{item.title}</option>)}</select><button type="button" disabled={busy || !dependencyId} onClick={() => run(() => advancedTaskApi.addDependency(task.id, dependencyId), "Dependency added")} aria-label="Add dependency"><Plus size={16} /></button></div>}
        </section>

        <section className="feature-advanced-card feature-recurring-card">
          <div className="feature-advanced-title"><div><strong>Recurring task</strong><small>Create the next occurrence after this task is completed</small></div><Repeat2 size={17} /></div>
          <form className="feature-form" onSubmit={saveRecurrence}>
            <label>Repeat every<select value={intervalDays} onChange={(event) => setIntervalDays(event.target.value)}><option value="">Does not repeat</option><option value="1">Every day</option><option value="7">Every week</option><option value="14">Every 2 weeks</option><option value="30">Every month (30 days)</option><option value="90">Every 3 months (90 days)</option></select></label>
            {intervalDays && <label>New task destination<select value={targetListId} onChange={(event) => setTargetListId(event.target.value)}>{lists.map((list) => <option key={list.id} value={list.id}>{list.name}</option>)}</select></label>}
            <button className="secondary-button" type="submit" disabled={busy}>{intervalDays ? "Save repeat rule" : "Remove repeat rule"}</button>
            {details.recurrence && <small className="feature-muted">Next occurrence check: {new Date(details.recurrence.next_run_at).toLocaleString()}</small>}
          </form>
        </section>
      </div>
    </section>
  );
}
