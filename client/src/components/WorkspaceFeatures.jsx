import { useEffect, useMemo, useRef, useState } from "react";
import {
  Bell, CalendarDays, Check, CheckCheck, ChevronLeft, ChevronRight, Clock3, Download,
  FileText, Folder, MessageCircle, Plus, Send, Trash2, Upload, Users, X
} from "lucide-react";
import {
  calendarApi, chatApi, filesApi, meetingNotesApi, notificationApi
} from "../lib/api.js";

const dateKey = (date) => {
  const value = new Date(date);
  return value.getFullYear() + "-" + String(value.getMonth() + 1).padStart(2, "0") + "-" + String(value.getDate()).padStart(2, "0");
};
const displayDate = (value, options = { month: "short", day: "numeric" }) =>
  value ? new Intl.DateTimeFormat(undefined, options).format(new Date(value)) : "No date";
const formatBytes = (value) => {
  if (!value) return "0 B";
  if (value < 1024) return value + " B";
  if (value < 1048576) return (value / 1024).toFixed(1) + " KB";
  return (value / 1048576).toFixed(1) + " MB";
};
const localDateTimeValue = (date, hour) => dateKey(date) + "T" + hour;

function PageHeading({ eyebrow, title, subtitle, action }) {
  return (
    <div className="feature-page-heading">
      <div><p className="ref-eyebrow">{eyebrow}</p><h1>{title}</h1><p className="ref-subtitle">{subtitle}</p></div>
      {action}
    </div>
  );
}

export function CalendarPage({ data, onOpenTask, realtimeVersion = 0 }) {
  const [selectedDate, setSelectedDate] = useState(() => new Date());
  const [viewMode, setViewMode] = useState("month");
  const [calendar, setCalendar] = useState({ events: [], tasks: [] });
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [showForm, setShowForm] = useState(false);
  const [saving, setSaving] = useState(false);
  const [form, setForm] = useState({
    title: "", event_type: "meeting", date: dateKey(new Date()),
    start: localDateTimeValue(new Date(), "10:00"), end: localDateTimeValue(new Date(), "11:00"),
    board_id: "", description: "", all_day: false
  });
  const calendarRequestRef = useRef(0);
  const eventDialogRef = useRef(null);

  const range = useMemo(() => {
    const anchor = new Date(selectedDate.getFullYear(), selectedDate.getMonth(), selectedDate.getDate());
    let start;
    let end;
    if (viewMode === "day") {
      start = anchor;
      end = anchor;
    } else if (viewMode === "week") {
      start = new Date(anchor);
      start.setDate(anchor.getDate() - ((anchor.getDay() + 6) % 7));
      end = new Date(start);
      end.setDate(start.getDate() + 6);
    } else {
      const first = new Date(anchor.getFullYear(), anchor.getMonth(), 1);
      const offset = (first.getDay() + 6) % 7;
      start = new Date(first);
      start.setDate(1 - offset);
      end = new Date(start);
      end.setDate(start.getDate() + 41);
    }
    return { from: dateKey(start), to: dateKey(end), start, end };
  }, [selectedDate, viewMode]);

  async function loadCalendar() {
    const requestId = ++calendarRequestRef.current;
    setLoading(true);
    setError("");
    try {
      const result = await calendarApi.list(range.from, range.to);
      if (requestId === calendarRequestRef.current) {
        setCalendar({ events: result.events || [], tasks: result.tasks || [] });
      }
    } catch (requestError) {
      if (requestId === calendarRequestRef.current) {
        setError(requestError.message || "Could not load the calendar.");
      }
    } finally {
      if (requestId === calendarRequestRef.current) setLoading(false);
    }
  }

  useEffect(() => {
    loadCalendar();
    return () => { calendarRequestRef.current += 1; };
  }, [range.from, range.to, realtimeVersion]);

  useEffect(() => {
    const dialog = eventDialogRef.current;
    if (!dialog) return;
    if (showForm && !dialog.open) dialog.showModal();
    else if (!showForm && dialog.open) dialog.close();
  }, [showForm]);

  const itemsByDate = useMemo(() => {
    const map = new Map();
    for (const event of calendar.events) {
      const cursor = new Date(event.start_at);
      cursor.setHours(0, 0, 0, 0);
      const endBoundary = new Date(new Date(event.end_at).getTime() - 1);
      endBoundary.setHours(0, 0, 0, 0);
      while (cursor <= endBoundary) {
        const key = dateKey(cursor);
        if (!map.has(key)) map.set(key, []);
        map.get(key).push({ ...event, kind: "event" });
        cursor.setDate(cursor.getDate() + 1);
      }
    }
    for (const task of calendar.tasks) {
      const key = task.due_date;
      if (!map.has(key)) map.set(key, []);
      map.get(key).push({ ...task, kind: "task" });
    }
    for (const items of map.values()) {
      items.sort((a, b) => new Date(a.start_at || a.due_date + "T00:00:00") - new Date(b.start_at || b.due_date + "T00:00:00"));
    }
    return map;
  }, [calendar]);

  const gridDays = useMemo(() => {
    if (viewMode === "month") {
      return Array.from({ length: 42 }, (_, index) => {
        const day = new Date(range.start);
        day.setDate(range.start.getDate() + index);
        return day;
      });
    }
    const days = [];
    const current = new Date(range.start);
    while (current <= range.end) {
      days.push(new Date(current));
      current.setDate(current.getDate() + 1);
    }
    return days;
  }, [range, viewMode]);

  const title = viewMode === "day"
    ? displayDate(selectedDate, { weekday: "long", month: "long", day: "numeric", year: "numeric" })
    : viewMode === "week"
      ? displayDate(range.start, { month: "short", day: "numeric" }) + " – " + displayDate(range.end, { month: "short", day: "numeric", year: "numeric" })
      : displayDate(selectedDate, { month: "long", year: "numeric" });

  function movePeriod(direction) {
    const next = new Date(selectedDate);
    if (viewMode === "day") next.setDate(next.getDate() + direction);
    else if (viewMode === "week") next.setDate(next.getDate() + direction * 7);
    else {
      const originalDay = next.getDate();
      const targetMonth = new Date(next.getFullYear(), next.getMonth() + direction, 1);
      const lastDay = new Date(targetMonth.getFullYear(), targetMonth.getMonth() + 1, 0).getDate();
      targetMonth.setDate(Math.min(originalDay, lastDay));
      next.setTime(targetMonth.getTime());
    }
    setSelectedDate(next);
  }

  function openCreateEvent() {
    const day = dateKey(selectedDate);
    setForm({
      title: "", event_type: "meeting", date: day,
      start: day + "T10:00", end: day + "T11:00",
      board_id: "", description: "", all_day: false
    });
    setShowForm(true);
  }

  async function createEvent(event) {
    event.preventDefault();
    if (!form.title.trim()) return;
    setSaving(true);
    setError("");
    try {
      await calendarApi.create({
        title: form.title.trim(),
        event_type: form.event_type,
        description: form.description,
        board_id: form.board_id || null,
        start_at: new Date(form.all_day ? form.date + "T00:00" : form.start).toISOString(),
        end_at: new Date(form.all_day ? form.date + "T23:59" : form.end).toISOString(),
        time_zone: Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC",
        all_day: form.all_day
      });
      setShowForm(false);
      await loadCalendar();
    } catch (requestError) {
      setError(requestError.message || "Could not create calendar event.");
    } finally {
      setSaving(false);
    }
  }

  async function removeEvent(eventId) {
    if (!window.confirm("Delete this calendar event?")) return;
    try {
      await calendarApi.remove(eventId);
      await loadCalendar();
    } catch (requestError) {
      setError(requestError.message || "Could not delete calendar event.");
    }
  }

  function renderItems(day) {
    const key = dateKey(day);
    const items = itemsByDate.get(key) || [];
    return items.slice(0, viewMode === "month" ? 3 : 30).map((item) => item.kind === "task" ? (
      <button className="feature-calendar-item feature-calendar-task" key={"task-" + item.id} type="button" onClick={() => onOpenTask(data.tasks.find((task) => task.id === item.id))}>
        <i />{item.title}
      </button>
    ) : (
      <div className={"feature-calendar-item feature-event-" + item.event_type} key={"event-" + item.id}>
        <i /> <span>{item.title}</span>
        {viewMode !== "month" && <button type="button" onClick={() => removeEvent(item.id)} aria-label={"Delete " + item.title}><X size={12} /></button>}
      </div>
    ));
  }

  const weekDays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

  return (
    <div className="feature-page">
      <PageHeading eyebrow="PLAN THE WORK" title="Team calendar" subtitle="Task deadlines, project milestones, exams, submissions, and team meetings in one shared view."
        action={<button className="ref-primary-button" type="button" onClick={openCreateEvent}><Plus size={16} /> Add event</button>} />
      <section className="ref-panel feature-calendar-panel">
        <div className="feature-calendar-toolbar">
          <div className="feature-calendar-nav">
            <button className="feature-icon-button" type="button" onClick={() => movePeriod(-1)} aria-label="Previous period"><ChevronLeft size={17} /></button>
            <button className="feature-icon-button" type="button" onClick={() => movePeriod(1)} aria-label="Next period"><ChevronRight size={17} /></button>
            <strong>{title}</strong>
            <button className="feature-today-button" type="button" onClick={() => setSelectedDate(new Date())}>Today</button>
          </div>
          <div className="feature-segmented">
            {["month", "week", "day"].map((mode) => <button type="button" key={mode} className={viewMode === mode ? "selected" : ""} onClick={() => setViewMode(mode)}>{mode[0].toUpperCase() + mode.slice(1)}</button>)}
          </div>
        </div>
        <div className="feature-calendar-legend">
          <span><i className="legend-task" /> Task deadline</span>
          <span><i className="legend-meeting" /> Meeting</span>
          <span><i className="legend-milestone" /> Milestone</span>
          <span><i className="legend-exam" /> Exam / submission</span>
        </div>
        {viewMode === "month" && <div className="feature-calendar-weekdays">{weekDays.map((day) => <span key={day}>{day}</span>)}</div>}
        <div className={"feature-calendar-grid mode-" + viewMode}>
          {gridDays.map((day) => {
            const key = dateKey(day);
            const selected = key === dateKey(selectedDate);
            const outside = viewMode === "month" && day.getMonth() !== selectedDate.getMonth();
            const items = itemsByDate.get(key) || [];
            return (
              <div className={"feature-calendar-day" + (selected ? " selected" : "") + (outside ? " outside" : "")} key={key}>
                <button className="feature-calendar-day-number" type="button" onClick={() => { setSelectedDate(day); if (viewMode === "month" && items.length) setViewMode("day"); }} aria-label={displayDate(day, { weekday: "long", month: "long", day: "numeric" })}>{day.getDate()}</button>
                <span className="feature-calendar-items">{renderItems(day)}</span>
                {viewMode === "month" && items.length > 3 && <small>+{items.length - 3} more</small>}
              </div>
            );
          })}
        </div>
        {loading && <p className="feature-muted">Loading events and deadlines…</p>}
        {error && <p className="auth-error" role="alert">{error}</p>}
      </section>

      <dialog
        ref={eventDialogRef}
        className="feature-modal"
        aria-labelledby="event-form-title"
        onCancel={() => setShowForm(false)}
        onClose={() => { if (showForm) setShowForm(false); }}
        onClick={(event) => { if (event.target === event.currentTarget) setShowForm(false); }}
      >
            <div className="feature-modal-heading"><div><p className="ref-eyebrow">CALENDAR</p><h2 id="event-form-title">Add an event</h2></div><button className="feature-icon-button" type="button" onClick={() => setShowForm(false)} aria-label="Close"><X size={17} /></button></div>
            <form className="feature-form" onSubmit={createEvent}>
              <label>Title<input value={form.title} onChange={(e) => setForm({ ...form, title: e.target.value })} placeholder="Project review meeting" maxLength={180} required /></label>
              <div className="feature-form-grid">
                <label>Event type<select value={form.event_type} onChange={(e) => setForm({ ...form, event_type: e.target.value })}><option value="meeting">Meeting</option><option value="milestone">Project milestone</option><option value="exam">Exam</option><option value="submission">Submission</option><option value="focus_block">Focus block</option></select></label>
                <label>Project (optional)<select value={form.board_id} onChange={(e) => setForm({ ...form, board_id: e.target.value })}><option value="">Workspace-wide</option>{data.boards.map((board) => <option key={board.id} value={board.id}>{board.name}</option>)}</select></label>
              </div>
              <label>Description<textarea rows={3} value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} placeholder="Agenda, preparation, or details…" maxLength={4000} /></label>
              <label className="feature-inline-check"><input type="checkbox" checked={form.all_day} onChange={(e) => setForm({ ...form, all_day: e.target.checked })} /> All-day event</label>
              {form.all_day ? <label>Date<input type="date" value={form.date} onChange={(e) => setForm({ ...form, date: e.target.value })} required /></label> : <div className="feature-form-grid"><label>Starts<input type="datetime-local" value={form.start} onChange={(e) => setForm({ ...form, start: e.target.value })} required /></label><label>Ends<input type="datetime-local" value={form.end} onChange={(e) => setForm({ ...form, end: e.target.value })} required /></label></div>}
              <div className="feature-form-actions"><button className="secondary-button" type="button" onClick={() => setShowForm(false)}>Cancel</button><button className="ref-primary-button" type="submit" disabled={saving}>{saving ? "Saving…" : "Save event"}</button></div>
            </form>
      </dialog>
    </div>
  );
}

const notificationCategories = [
  ["task_assigned", "Task activity", "Tasks created, moved, or updated"],
  ["comment", "Comments", "New task discussion comments"],
  ["project_update", "Project updates", "Changes across your workspace"],
  ["deadline_approaching", "Upcoming deadlines", "Tasks due within 24 hours"],
  ["overdue_task", "Overdue tasks", "Tasks that have passed their due date"]
];

export function NotificationsPage({ onChanged, realtimeVersion = 0 }) {
  const [items, setItems] = useState([]);
  const [preferences, setPreferences] = useState([]);
  const [emailDeliveryAvailable, setEmailDeliveryAvailable] = useState(false);
  const [unread, setUnread] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");

  async function load() {
    setLoading(true);
    setError("");
    try {
      const [result, settings] = await Promise.all([notificationApi.list(), notificationApi.preferences()]);
      setItems(result.items || []);
      setUnread(result.unread_count || 0);
      setPreferences(settings.items || []);
      setEmailDeliveryAvailable(settings.email_delivery_available === true);
    } catch (requestError) {
      setError(requestError.message || "Could not load notifications.");
    } finally {
      setLoading(false);
    }
  }
  useEffect(() => { load(); }, [realtimeVersion]);

  async function markRead(item) {
    try {
      await notificationApi.read(item.id);
      setItems((current) => current.map((row) => row.id === item.id ? { ...row, read_at: new Date().toISOString() } : row));
      setUnread((value) => Math.max(0, value - (item.read_at ? 0 : 1)));
      onChanged?.();
    } catch (requestError) { setError(requestError.message || "Could not update notification."); }
  }

  async function markAllRead() {
    try {
      await notificationApi.readAll();
      setItems((current) => current.map((row) => ({ ...row, read_at: row.read_at || new Date().toISOString() })));
      setUnread(0);
      onChanged?.();
    } catch (requestError) { setError(requestError.message || "Could not update notifications."); }
  }

  async function togglePreference(type, inAppEnabled, emailEnabled) {
    try {
      await notificationApi.updatePreference(type, inAppEnabled, emailEnabled);
      setPreferences((current) => {
        const found = current.some((row) => row.notification_type === type);
        return found ? current.map((row) => row.notification_type === type
          ? { ...row, in_app_enabled: inAppEnabled, email_enabled: emailEnabled }
          : row)
          : [...current, { notification_type: type, in_app_enabled: inAppEnabled, email_enabled: emailEnabled }];
      });
    } catch (requestError) { setError(requestError.message || "Could not update preferences."); }
  }

  return (
    <div className="feature-page">
      <PageHeading eyebrow="STAY IN THE LOOP" title="Notification centre" subtitle={unread + " unread updates for your workspace."}
        action={<button className="secondary-button" type="button" onClick={markAllRead} disabled={!unread}><CheckCheck size={15} /> Mark all as read</button>} />
      {error && <p className="auth-error" role="alert">{error}</p>}
      <div className="feature-notification-layout">
        <section className="ref-panel feature-notifications">
          <div className="ref-panel-title"><h2><Bell size={16} /> Your notifications</h2><span className="feature-count">{items.length}</span></div>
          {loading ? <p className="feature-muted">Loading notifications…</p> : items.length ? items.map((item) => (
            <article className={"feature-notification" + (!item.read_at ? " unread" : "")} key={item.id}>
              <span className="feature-notification-icon"><Bell size={16} /></span>
              <div className="feature-notification-copy"><div><strong>{item.title}</strong><time>{displayDate(item.created_at, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })}</time></div><p>{item.body}</p><small>{item.notification_type.replaceAll("_", " ")}</small></div>
              {!item.read_at && <button className="feature-read-button" type="button" onClick={() => markRead(item)} aria-label="Mark as read"><Check size={15} /></button>}
            </article>
          )) : <div className="feature-empty"><Bell size={24} /><strong>You're all caught up</strong><span>New assignments, comments, and deadline reminders will appear here.</span></div>}
        </section>
        <section className="ref-panel">
          <div className="ref-panel-title"><h2><Check size={16} /> Notification preferences</h2></div>
          <p className="feature-panel-description">Choose which updates appear in your in-app inbox. Email delivery can be enabled after an email provider is configured.</p>
          <div className="feature-preference-list">
            {notificationCategories.map(([type, title, description]) => {
              const preference = preferences.find((row) => row.notification_type === type);
              const inAppEnabled = preference?.in_app_enabled ?? true;
              const emailEnabled = preference?.email_enabled ?? false;
              return <div className="feature-preference" key={type}>
                <span className="feature-preference-copy"><strong>{title}</strong><small>{description}</small></span>
                <label className="feature-pref-toggle"><span>In-app</span><input type="checkbox" checked={inAppEnabled} onChange={(event) => togglePreference(type, event.target.checked, emailEnabled)} /></label>
                <label className="feature-pref-toggle"><span>Email</span><input type="checkbox" checked={emailEnabled} disabled={!emailDeliveryAvailable} onChange={(event) => togglePreference(type, inAppEnabled, event.target.checked)} /></label>
              </div>;
            })}
          </div>
          <p className="feature-muted">{emailDeliveryAvailable ? "Email delivery is configured. Enable email per notification category to receive messages." : "Email delivery is not configured on the server. Add RESEND_API_KEY and EMAIL_FROM to enable the email preferences."}</p>
        </section>
      </div>
    </div>
  );
}

export function FilesPage({ data, realtimeVersion = 0 }) {
  const [files, setFiles] = useState([]);
  const [boardId, setBoardId] = useState("");
  const [taskId, setTaskId] = useState("");
  const [selectedFile, setSelectedFile] = useState(null);
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);

  async function load() {
    setLoading(true);
    try { setFiles(await filesApi.list()); }
    catch (requestError) { setError(requestError.message || "Could not load files."); }
    finally { setLoading(false); }
  }
  useEffect(() => { load(); }, [realtimeVersion]);

  async function upload(event) {
    event.preventDefault();
    if (!selectedFile) return;
    const formElement = event.currentTarget;
    setUploading(true);
    setError("");
    try {
      const form = new FormData();
      form.append("file", selectedFile);
      if (taskId) form.append("task_id", taskId);
      else if (boardId) form.append("board_id", boardId);
      await filesApi.upload(form);
      setSelectedFile(null);
      formElement.reset();
      await load();
    } catch (requestError) { setError(requestError.message || "File upload failed."); }
    finally { setUploading(false); }
  }

  async function downloadFile(file) {
    setError("");
    try {
      await filesApi.download(file.id, file.original_name);
    } catch (requestError) {
      setError(requestError.message || "Could not download this file. It may have been removed.");
    }
  }

  async function remove(file) {
    if (!window.confirm("Delete " + file.original_name + "?")) return;
    try { await filesApi.remove(file.id); await load(); }
    catch (requestError) { setError(requestError.message || "Could not delete file."); }
  }

  return (
    <div className="feature-page">
      <PageHeading eyebrow="SHARED KNOWLEDGE" title="Files & documents" subtitle="Keep project presentations, PDFs, source files, and team resources in one shared workspace." />
      <section className="ref-panel feature-upload-panel">
        <div className="ref-panel-title"><h2><Upload size={16} /> Upload a file</h2><span className="feature-file-limit">10 MB max</span></div>
        <form className="feature-upload-form" onSubmit={upload}>
          <label className="feature-file-picker"><input type="file" onChange={(event) => setSelectedFile(event.target.files?.[0] || null)} required /><span><Upload size={20} /><strong>{selectedFile?.name || "Choose a file from your device"}</strong><small>PDF, images, presentations, documents, code, and more</small></span></label>
          <div className="feature-form-grid"><label>Attach to project<select value={boardId} onChange={(event) => { setBoardId(event.target.value); setTaskId(""); }} disabled={Boolean(taskId)}><option value="">General workspace file</option>{data.boards.map((board) => <option value={board.id} key={board.id}>{board.name}</option>)}</select></label>
          <label>Or attach to task<select value={taskId} onChange={(event) => { setTaskId(event.target.value); if (event.target.value) setBoardId(""); }}><option value="">No task attachment</option>{data.tasks.map((task) => <option value={task.id} key={task.id}>{task.title}</option>)}</select></label></div>
          <button className="ref-primary-button" type="submit" disabled={!selectedFile || uploading}><Upload size={15} /> {uploading ? "Uploading…" : "Upload to workspace"}</button>
        </form>
        {error && <p className="auth-error" role="alert">{error}</p>}
      </section>
      <section className="ref-panel">
        <div className="ref-panel-title"><h2><Folder size={16} /> Workspace library</h2><span className="feature-count">{files.length} files</span></div>
        {loading ? <p className="feature-muted">Loading your workspace library…</p> : files.length ? <div className="feature-file-list">
          {files.map((file) => <article className="feature-file-row" key={file.id}>
            <span className="feature-file-icon"><FileText size={19} /></span>
            <div className="feature-file-copy"><strong>{file.original_name}</strong><small>{formatBytes(file.size_bytes)} · {file.media_type} · {displayDate(file.created_at, { month: "short", day: "numeric", year: "numeric" })}</small><small>{file.task_id ? "Attached to task" : file.board_id ? "Attached to project" : "Workspace file"}</small></div>
            <button className="feature-icon-button" type="button" onClick={() => downloadFile(file)} aria-label={"Download " + file.original_name}><Download size={16} /></button>
            {file.uploaded_by === data.current_user_id && <button className="feature-icon-button feature-danger-button" type="button" onClick={() => remove(file)} aria-label={"Delete " + file.original_name}><Trash2 size={16} /></button>}
          </article>)}
        </div> : <div className="feature-empty"><Folder size={24} /><strong>Your library is ready</strong><span>Upload your first shared project resource.</span></div>}
      </section>
      <p className="feature-muted">Local development storage is used on this branch. Configure durable object storage and backups before a production release.</p>
    </div>
  );
}

export function ChatAndMeetingsPage({ data, realtimeVersion = 0 }) {
  const [tab, setTab] = useState("chat");
  const [boardId, setBoardId] = useState("");
  const [messages, setMessages] = useState([]);
  const [notes, setNotes] = useState([]);
  const [message, setMessage] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [sending, setSending] = useState(false);
  const [noteForm, setNoteForm] = useState({ title: "", meeting_at: "", board_id: "", attendees: "", agenda: "", notes: "", action_items: "" });
  const [savingNote, setSavingNote] = useState(false);
  const chatRequestRef = useRef(0);
  const notesRequestRef = useRef(0);
  const activeChannelRef = useRef(boardId);
  const activeTabRef = useRef(tab);
  activeChannelRef.current = boardId;
  activeTabRef.current = tab;

  async function loadChat(channel = boardId) {
    const requestId = ++chatRequestRef.current;
    setLoading(true);
    setError("");
    try {
      const rows = await chatApi.list(channel || null);
      if (requestId === chatRequestRef.current && activeChannelRef.current === channel && activeTabRef.current === "chat") {
        setMessages(rows);
      }
    } catch (requestError) {
      if (requestId === chatRequestRef.current && activeChannelRef.current === channel && activeTabRef.current === "chat") {
        setError(requestError.message || "Could not load team chat.");
      }
    } finally {
      if (requestId === chatRequestRef.current && activeChannelRef.current === channel && activeTabRef.current === "chat") {
        setLoading(false);
      }
    }
  }
  async function loadNotes() {
    const requestId = ++notesRequestRef.current;
    setLoading(true);
    setError("");
    try {
      const rows = await meetingNotesApi.list();
      if (requestId === notesRequestRef.current && activeTabRef.current === "notes") setNotes(rows);
    } catch (requestError) {
      if (requestId === notesRequestRef.current && activeTabRef.current === "notes") {
        setError(requestError.message || "Could not load meeting notes.");
      }
    } finally {
      if (requestId === notesRequestRef.current && activeTabRef.current === "notes") setLoading(false);
    }
  }
  useEffect(() => {
    if (tab === "chat") loadChat(boardId);
    else loadNotes();
    return () => {
      chatRequestRef.current += 1;
      notesRequestRef.current += 1;
    };
  }, [tab, boardId, realtimeVersion]);

  async function sendMessage(event) {
    event.preventDefault();
    if (!message.trim()) return;
    const channel = boardId;
    const body = message.trim();
    setSending(true);
    try {
      await chatApi.send({ board_id: channel || null, body });
      if (activeChannelRef.current === channel && activeTabRef.current === "chat") {
        setMessage("");
        await loadChat(channel);
      }
    } catch (requestError) {
      if (activeChannelRef.current === channel) setError(requestError.message || "Could not send message.");
    } finally {
      setSending(false);
    }
  }

  async function saveNote(event) {
    event.preventDefault();
    if (!noteForm.title.trim()) return;
    setSavingNote(true);
    try {
      await meetingNotesApi.create({
        ...noteForm,
        board_id: noteForm.board_id || null,
        meeting_at: noteForm.meeting_at ? new Date(noteForm.meeting_at).toISOString() : null
      });
      setNoteForm({ title: "", meeting_at: "", board_id: "", attendees: "", agenda: "", notes: "", action_items: "" });
      await loadNotes();
    } catch (requestError) { setError(requestError.message || "Could not save meeting notes."); }
    finally { setSavingNote(false); }
  }

  async function deleteNote(note) {
    if (!window.confirm("Delete meeting notes for " + note.title + "?")) return;
    try { await meetingNotesApi.remove(note.id); await loadNotes(); }
    catch (requestError) { setError(requestError.message || "Could not delete meeting notes."); }
  }

  return (
    <div className="feature-page">
      <PageHeading eyebrow="TEAM CONVERSATIONS" title="Chat & meeting notes" subtitle="Keep project conversations, agendas, decisions, and action items connected to your workspace." />
      <section className="ref-panel feature-collaboration-panel">
        <div className="feature-collab-toolbar">
          <div className="feature-segmented"><button type="button" className={tab === "chat" ? "selected" : ""} onClick={() => setTab("chat")}><MessageCircle size={14} /> Team chat</button><button type="button" className={tab === "notes" ? "selected" : ""} onClick={() => setTab("notes")}><FileText size={14} /> Meeting notes</button></div>
          {tab === "chat" && <select value={boardId} onChange={(event) => setBoardId(event.target.value)} aria-label="Choose chat channel"><option value="">General workspace</option>{data.boards.map((board) => <option value={board.id} key={board.id}>{board.name}</option>)}</select>}
        </div>
        {error && <p className="auth-error" role="alert">{error}</p>}
        {tab === "chat" ? (
          <div className="feature-chat">
            <div className="feature-chat-messages">
              {loading ? <p className="feature-muted">Loading conversation…</p> : messages.length ? messages.map((row) => <article className="feature-chat-message" key={row.id}><span className="ref-avatar">{(row.author_name || "M").split(/\s+/).map((part) => part[0]).slice(0,2).join("").toUpperCase()}</span><div><div className="feature-chat-meta"><strong>{row.author_name}</strong><time>{displayDate(row.created_at, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })}</time></div><p>{row.body}</p></div></article>) : <div className="feature-empty"><MessageCircle size={24} /><strong>No messages yet</strong><span>Start a conversation with your team.</span></div>}
            </div>
            <form className="feature-chat-composer" onSubmit={sendMessage}><input value={message} onChange={(event) => setMessage(event.target.value)} placeholder="Write a message to your team…" maxLength={4000} /><button className="ref-primary-button" type="submit" disabled={!message.trim() || sending}><Send size={15} /> Send</button></form>
          </div>
        ) : (
          <div className="feature-notes">
            <form className="feature-form feature-note-form" onSubmit={saveNote}>
              <h3><Plus size={16} /> New meeting notes</h3>
              <label>Meeting title<input value={noteForm.title} onChange={(e) => setNoteForm({ ...noteForm, title: e.target.value })} placeholder="Weekly project sync" maxLength={180} required /></label>
              <div className="feature-form-grid"><label>Date and time<input type="datetime-local" value={noteForm.meeting_at} onChange={(e) => setNoteForm({ ...noteForm, meeting_at: e.target.value })} /></label><label>Project<select value={noteForm.board_id} onChange={(e) => setNoteForm({ ...noteForm, board_id: e.target.value })}><option value="">Workspace-wide</option>{data.boards.map((board) => <option key={board.id} value={board.id}>{board.name}</option>)}</select></label></div>
              <label>Attendees<input value={noteForm.attendees} onChange={(e) => setNoteForm({ ...noteForm, attendees: e.target.value })} placeholder="Team members" maxLength={1000} /></label>
              <label>Agenda<textarea rows={2} value={noteForm.agenda} onChange={(e) => setNoteForm({ ...noteForm, agenda: e.target.value })} placeholder="Topics to discuss…" maxLength={12000} /></label>
              <label>Discussion notes<textarea rows={3} value={noteForm.notes} onChange={(e) => setNoteForm({ ...noteForm, notes: e.target.value })} placeholder="Decisions and discussion…" maxLength={12000} /></label>
              <label>Action items<textarea rows={2} value={noteForm.action_items} onChange={(e) => setNoteForm({ ...noteForm, action_items: e.target.value })} placeholder="Owners, actions, and due dates…" maxLength={12000} /></label>
              <button className="ref-primary-button" type="submit" disabled={savingNote}>{savingNote ? "Saving…" : "Save meeting notes"}</button>
            </form>
            <div className="feature-note-list">
              <h3>Saved notes</h3>
              {loading ? <p className="feature-muted">Loading meeting notes…</p> : notes.length ? notes.map((note) => <article className="feature-note-card" key={note.id}>
                <div className="feature-note-card-head"><div><strong>{note.title}</strong><small>{note.meeting_at ? displayDate(note.meeting_at, { weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit" }) : "No date set"}</small></div><button className="feature-icon-button feature-danger-button" type="button" onClick={() => deleteNote(note)} aria-label={"Delete " + note.title}><Trash2 size={15} /></button></div>
                {note.agenda && <p><b>Agenda:</b> {note.agenda}</p>}{note.notes && <p><b>Notes:</b> {note.notes}</p>}{note.action_items && <p><b>Action items:</b> {note.action_items}</p>}
              </article>) : <p className="feature-muted">No meeting notes have been saved yet.</p>}
            </div>
          </div>
        )}
      </section>
    </div>
  );
}
