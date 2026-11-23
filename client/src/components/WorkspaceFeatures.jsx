import { useEffect, useMemo, useRef, useState } from "react";
import {
  Bell, CalendarDays, Check, CheckCheck, ChevronDown, ChevronLeft, ChevronRight, Clock3, Download,
  Code2, Eye, FileText, Folder, KeyRound, Link2, Lock, LockKeyhole, ListChecks, MessageCircle, Pencil, Pin, Plus, Search, Send, ShieldCheck, Trash2, Upload, Users, X
} from "lucide-react";
import {
  calendarApi, chatApi, filesApi, lockerApi, meetingNotesApi, notificationApi, presenceApi,
  sendWorkspaceSocketMessage, subscribeWorkspaceSocket
} from "../lib/api.js";
import { base64ToBytes, bytesToBase64, createLockerKeyCheck, decryptLockerPayload, deriveLockerKey, encryptLockerPayload, LOCKER_KDF_ITERATIONS, randomBytes, verifyLockerKey } from "../lib/lockerCrypto.js";

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
  const [editingId, setEditingId] = useState(null);
  const [eventTypeFilter, setEventTypeFilter] = useState("");
  const [projectFilter, setProjectFilter] = useState("");
  const [attendeeFilter, setAttendeeFilter] = useState("");
  const [form, setForm] = useState({
    title: "", event_type: "meeting", date: dateKey(new Date()), endDate: dateKey(new Date()),
    start: localDateTimeValue(new Date(), "10:00"), end: localDateTimeValue(new Date(), "11:00"),
    board_id: "", description: "", all_day: false, reminder_minutes: "", attendee_ids: []
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
      const result = await calendarApi.list(range.from, range.to, {
        event_type: eventTypeFilter,
        board_id: projectFilter,
        attendee_id: attendeeFilter
      });
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
  }, [range.from, range.to, eventTypeFilter, projectFilter, attendeeFilter, realtimeVersion]);

  useEffect(() => {
    const dialog = eventDialogRef.current;
    if (!dialog) return;
    if (showForm && !dialog.open) dialog.showModal();
    else if (!showForm && dialog.open) dialog.close();
  }, [showForm]);

  const itemsByDate = useMemo(() => {
    const map = new Map();
    for (const event of calendar.events) {
      const startValue = event.all_day && event.all_day_start_date
        ? event.all_day_start_date + "T00:00:00"
        : event.start_at;
      const endValue = event.all_day && event.all_day_end_date
        ? event.all_day_end_date + "T23:59:59"
        : event.end_at;
      const eventStart = new Date(startValue);
      const eventEnd = new Date(endValue);
      if (!Number.isFinite(eventStart.getTime()) || !Number.isFinite(eventEnd.getTime()) || eventEnd < range.start || eventStart > new Date(range.end.getTime() + 86400000)) continue;
      const cursor = new Date(Math.max(eventStart.getTime(), range.start.getTime()));
      cursor.setHours(0, 0, 0, 0);
      const endBoundary = new Date(Math.min(eventEnd.getTime(), new Date(range.end.getTime() + 86400000).getTime() - 1));
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
  }, [calendar, range]);

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
    setEditingId(null);
    setForm({
      title: "", event_type: "meeting", date: day, endDate: day,
      start: day + "T10:00", end: day + "T11:00",
      board_id: "", description: "", all_day: false, reminder_minutes: "", attendee_ids: []
    });
    setShowForm(true);
  }

  function openEditEvent(event) {
    setEditingId(event.id);
    const date = event.all_day_start_date || dateKey(new Date(event.start_at));
    const endDate = event.all_day_end_date || date;
    const toLocalInput = (value) => {
      const parsed = new Date(value);
      const local = new Date(parsed.getTime() - parsed.getTimezoneOffset() * 60000);
      return local.toISOString().slice(0, 16);
    };
    setForm({
      title: event.title,
      event_type: event.event_type,
      date,
      endDate,
      start: toLocalInput(event.start_at),
      end: toLocalInput(event.end_at),
      board_id: event.board_id || "",
      description: event.description || "",
      all_day: event.all_day === true,
      reminder_minutes: event.reminder_minutes == null ? "" : String(event.reminder_minutes),
      attendee_ids: event.attendee_ids || []
    });
    setShowForm(true);
  }

  async function saveEvent(event) {
    event.preventDefault();
    if (!form.title.trim()) return;
    setSaving(true);
    setError("");
    const endDateObject = new Date(form.endDate + "T00:00:00");
    endDateObject.setDate(endDateObject.getDate() + 1);
    const allDayEndBoundary = dateKey(endDateObject);
    const payload = {
      title: form.title.trim(),
      event_type: form.event_type,
      description: form.description,
      board_id: form.board_id || null,
      start_at: new Date(form.all_day ? form.date + "T00:00" : form.start).toISOString(),
      end_at: new Date(form.all_day ? allDayEndBoundary + "T00:00" : form.end).toISOString(),
      time_zone: Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC",
      all_day: form.all_day,
      all_day_start_date: form.all_day ? form.date : null,
      all_day_end_date: form.all_day ? form.endDate : null,
      reminder_minutes: form.reminder_minutes === "" ? null : Number(form.reminder_minutes),
      attendee_ids: form.attendee_ids
    };
    try {
      if (editingId) await calendarApi.update(editingId, payload);
      else await calendarApi.create(payload);
      setShowForm(false);
      await loadCalendar();
    } catch (requestError) {
      setError(requestError.message || "Could not save calendar event.");
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
        <button className="feature-calendar-event-open" type="button" onClick={() => openEditEvent(item)} title="Edit event"><i /> <span>{item.title}</span></button>
        <span className="feature-event-controls">
          {item.reminder_minutes != null && <Clock3 size={11} title="Reminder set" />}
          {viewMode !== "month" && <button type="button" onClick={() => removeEvent(item.id)} aria-label={"Delete " + item.title}><X size={12} /></button>}
        </span>
      </div>
    ));
  }

  const weekDays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

  return (
    <div className="feature-page">
      <PageHeading eyebrow="PLAN THE WORK" title="Team calendar" subtitle="Filter events, invite workspace members, set reminders and reschedule events alongside task deadlines."
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
        <div className="feature-calendar-filters">
          <label>Type<select value={eventTypeFilter} onChange={(event) => setEventTypeFilter(event.target.value)}><option value="">All event types</option><option value="meeting">Meetings</option><option value="milestone">Milestones</option><option value="exam">Exams</option><option value="submission">Submissions</option><option value="focus_block">Focus blocks</option></select></label>
          <label>Project<select value={projectFilter} onChange={(event) => setProjectFilter(event.target.value)}><option value="">All projects</option>{data.boards.map((board) => <option key={board.id} value={board.id}>{board.name}</option>)}</select></label>
          <label>Attendee<select value={attendeeFilter} onChange={(event) => setAttendeeFilter(event.target.value)}><option value="">All attendees</option>{data.users.map((user) => <option key={user.id} value={user.id}>{user.full_name}</option>)}</select></label>
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
        <div className="feature-modal-heading"><div><p className="ref-eyebrow">CALENDAR</p><h2 id="event-form-title">{editingId ? "Edit event" : "Add an event"}</h2></div><button className="feature-icon-button" type="button" onClick={() => setShowForm(false)} aria-label="Close"><X size={17} /></button></div>
        <form className="feature-form" onSubmit={saveEvent}>
          <label>Title<input value={form.title} onChange={(e) => setForm({ ...form, title: e.target.value })} placeholder="Project review meeting" maxLength={180} required /></label>
          <div className="feature-form-grid">
            <label>Event type<select value={form.event_type} onChange={(e) => setForm({ ...form, event_type: e.target.value })}><option value="meeting">Meeting</option><option value="milestone">Project milestone</option><option value="exam">Exam</option><option value="submission">Submission</option><option value="focus_block">Focus block</option></select></label>
            <label>Project (optional)<select value={form.board_id} onChange={(e) => setForm({ ...form, board_id: e.target.value })}><option value="">Workspace-wide</option>{data.boards.map((board) => <option key={board.id} value={board.id}>{board.name}</option>)}</select></label>
          </div>
          <label>Description<textarea rows={3} value={form.description} onChange={(e) => setForm({ ...form, description: e.target.value })} placeholder="Agenda, preparation, or details…" maxLength={4000} /></label>
          <label className="feature-inline-check"><input type="checkbox" checked={form.all_day} onChange={(e) => setForm({ ...form, all_day: e.target.checked })} /> All-day event</label>
          {form.all_day ? <div className="feature-form-grid"><label>Starts<input type="date" value={form.date} onChange={(e) => setForm({ ...form, date: e.target.value })} required /></label><label>Ends<input type="date" value={form.endDate} min={form.date} onChange={(e) => setForm({ ...form, endDate: e.target.value })} required /></label></div> : <div className="feature-form-grid"><label>Starts<input type="datetime-local" value={form.start} onChange={(e) => setForm({ ...form, start: e.target.value })} required /></label><label>Ends<input type="datetime-local" value={form.end} onChange={(e) => setForm({ ...form, end: e.target.value })} required /></label></div>}
          <div className="feature-form-grid">
            <label>Reminder<select value={form.reminder_minutes} onChange={(e) => setForm({ ...form, reminder_minutes: e.target.value })}><option value="">No reminder</option><option value="0">At event time</option><option value="5">5 minutes before</option><option value="10">10 minutes before</option><option value="15">15 minutes before</option><option value="30">30 minutes before</option><option value="60">1 hour before</option><option value="1440">1 day before</option></select></label>
            <label>Attendees<select multiple size={4} value={form.attendee_ids} onChange={(e) => setForm({ ...form, attendee_ids: Array.from(e.target.selectedOptions, (option) => option.value) })}>{data.users.map((user) => <option key={user.id} value={user.id}>{user.full_name}</option>)}</select></label>
          </div>
          <p className="feature-muted">Hold Ctrl (Windows) or Command (Mac) to select multiple attendees.</p>
          <div className="feature-form-actions"><button className="secondary-button" type="button" onClick={() => setShowForm(false)}>Cancel</button>{editingId && <button className="secondary-button danger-outline" type="button" onClick={() => { setShowForm(false); removeEvent(editingId); }}>Delete</button>}<button className="ref-primary-button" type="submit" disabled={saving}>{saving ? "Saving…" : editingId ? "Save changes" : "Save event"}</button></div>
        </form>
      </dialog>
    </div>
  );
}

const notificationCategories = [
  ["task_assigned", "Task activity", "Tasks created, moved, or updated"],
  ["comment", "Comments", "New task discussion comments"],
  ["mention", "Mentions", "When a teammate mentions you"],
  ["workspace_invitation", "Workspace invitations", "Invitations to join a workspace"],
  ["calendar_invitation", "Calendar invitations", "When you are invited to an event"],
  ["calendar_reminder", "Calendar reminders", "Reminders before scheduled events"],
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
  const [savingPreference, setSavingPreference] = useState(null);
  const [preferencesExpanded, setPreferencesExpanded] = useState(true);
  const [nextBefore, setNextBefore] = useState(null);
  const [loadingOlder, setLoadingOlder] = useState(false);

  async function load() {
    setLoading(true);
    setError("");
    try {
      const [result, settings] = await Promise.all([notificationApi.list(), notificationApi.preferences()]);
      setItems(result.items || []);
      setUnread(result.unread_count || 0);
      setNextBefore(result.next_before || null);
      setPreferences(settings.items || []);
      setEmailDeliveryAvailable(settings.email_delivery_available === true);
    } catch (requestError) {
      setError(requestError.message || "Could not load notifications.");
    } finally {
      setLoading(false);
    }
  }
  useEffect(() => { load(); }, [realtimeVersion]);

  async function loadOlder() {
    if (!nextBefore || loadingOlder) return;
    setLoadingOlder(true);
    try {
      const result = await notificationApi.list(nextBefore, 50);
      setItems((current) => {
        const seen = new Set(current.map((item) => item.id));
        return [...current, ...(result.items || []).filter((item) => !seen.has(item.id))];
      });
      setNextBefore(result.next_before || null);
      setUnread(result.unread_count || 0);
    } catch (requestError) {
      setError(requestError.message || "Could not load older notifications.");
    } finally {
      setLoadingOlder(false);
    }
  }

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
    if (savingPreference) return;
    setSavingPreference(type);
    try {
      await notificationApi.updatePreference(type, inAppEnabled, emailEnabled);
      await load();
      onChanged?.();
    } catch (requestError) { setError(requestError.message || "Could not update preferences."); }
    finally { setSavingPreference(null); }
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
          )) : <div className="feature-empty"><Bell size={24} /><strong>You're all caught up</strong><span>New assignments, comments, mentions, invitations and deadline reminders will appear here.</span></div>}
          {nextBefore && <button className="secondary-button feature-load-older" type="button" onClick={loadOlder} disabled={loadingOlder}>{loadingOlder ? "Loading older updates…" : "Load older notifications"}</button>}
        </section>
        <section className={"ref-panel feature-preferences-panel" + (preferencesExpanded ? " is-expanded" : " is-collapsed")}>
          <div className="ref-panel-title feature-preferences-heading">
            <button
              className="feature-preferences-toggle"
              type="button"
              id="notification-preferences-toggle"
              aria-expanded={preferencesExpanded}
              aria-controls="notification-preferences-content"
              onClick={() => setPreferencesExpanded((expanded) => !expanded)}
            >
              <span className="feature-preferences-label"><Check size={16} /> Notification preferences</span>
              <ChevronDown size={19} aria-hidden="true" />
            </button>
          </div>
          <div
            className={"feature-preferences-content" + (preferencesExpanded ? "" : " is-collapsed")}
            id="notification-preferences-content"
            role="region"
            aria-labelledby="notification-preferences-toggle"
            aria-hidden={!preferencesExpanded}
            inert={!preferencesExpanded}
          >
            <div className="feature-preferences-content-inner">
              <p className="feature-panel-description">Choose which updates appear in your in-app inbox. Email delivery can be enabled after an email provider is configured.</p>
              <div className="feature-preference-list">
            {notificationCategories.map(([type, title, description]) => {
              const preference = preferences.find((row) => row.notification_type === type);
              const inAppEnabled = preference?.in_app_enabled ?? true;
              const emailEnabled = preference?.email_enabled ?? false;
              return <div className="feature-preference" key={type}>
                <span className="feature-preference-copy"><strong>{title}</strong><small>{description}</small></span>
                <label className="feature-pref-toggle"><span>In-app</span><input type="checkbox" checked={inAppEnabled} disabled={savingPreference === type} onChange={(event) => togglePreference(type, event.target.checked, emailEnabled)} /></label>
                <label className="feature-pref-toggle"><span>Email</span><input type="checkbox" checked={emailEnabled} disabled={!emailDeliveryAvailable || savingPreference === type} onChange={(event) => togglePreference(type, inAppEnabled, event.target.checked)} /></label>
              </div>;
            })}
              </div>
              <p className="feature-muted">{emailDeliveryAvailable ? "Email delivery is configured. Enable email per notification category to receive messages." : "Email delivery is not configured on the server. Add RESEND_API_KEY and EMAIL_FROM to enable the email preferences."}</p>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}

export function FilesPage({ data, realtimeVersion = 0 }) {
  const [files, setFiles] = useState([]);
  const [usage, setUsage] = useState(null);
  const [audit, setAudit] = useState([]);
  const [auditOpen, setAuditOpen] = useState(false);
  const [quotaInput, setQuotaInput] = useState("");
  const [boardId, setBoardId] = useState("");
  const [taskId, setTaskId] = useState("");
  const [selectedFile, setSelectedFile] = useState(null);
  const [preview, setPreview] = useState(null);
  const [uploading, setUploading] = useState(false);
  const [savingQuota, setSavingQuota] = useState(false);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const currentRole = data.workspace_members?.find((member) => member.user_id === data.current_user_id)?.role;
  const canManageStorage = currentRole === "owner" || currentRole === "admin";

  async function load() {
    setLoading(true);
    setError("");
    try {
      const [fileRows, storageUsage] = await Promise.all([filesApi.list(), filesApi.usage()]);
      setFiles(fileRows);
      setUsage(storageUsage);
      setQuotaInput(String(storageUsage.quota_bytes));
    } catch (requestError) {
      setError(requestError.message || "Could not load workspace storage.");
    } finally {
      setLoading(false);
    }
  }
  useEffect(() => { load(); }, [realtimeVersion]);

  useEffect(() => () => {
    if (preview?.url) URL.revokeObjectURL(preview.url);
  }, [preview?.url]);

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

  async function openPreview(file) {
    setError("");
    try {
      const url = await filesApi.preview(file.id);
      setPreview({ url, name: file.original_name, type: file.media_type });
    } catch (requestError) {
      setError(requestError.message || "Preview is unavailable. The file may need a malware scan.");
    }
  }

  async function remove(file) {
    if (!window.confirm("Delete " + file.original_name + "?")) return;
    try { await filesApi.remove(file.id); await load(); }
    catch (requestError) { setError(requestError.message || "Could not delete file."); }
  }

  async function saveQuota(event) {
    event.preventDefault();
    setSavingQuota(true);
    setError("");
    try {
      await filesApi.setQuota(Number(quotaInput));
      await load();
    } catch (requestError) { setError(requestError.message || "Could not update the workspace storage quota."); }
    finally { setSavingQuota(false); }
  }

  async function toggleAudit() {
    if (auditOpen) { setAuditOpen(false); return; }
    try {
      setAudit(await filesApi.audit());
      setAuditOpen(true);
    } catch (requestError) { setError(requestError.message || "Only workspace admins can view file audit history."); }
  }

  const usedPercent = usage?.quota_bytes ? Math.min(100, Math.round((usage.used_bytes + usage.reserved_bytes) / usage.quota_bytes * 100)) : 0;

  return (
    <div className="feature-page">
      <PageHeading eyebrow="SHARED KNOWLEDGE" title="Files & documents" subtitle="Keep project presentations, PDFs, source files, and team resources in one shared workspace." />
      <section className="ref-panel feature-storage-panel">
        <div className="ref-panel-title"><h2><Folder size={16} /> Workspace storage</h2><span className="feature-count">{usage?.file_count ?? files.length} files</span></div>
        {usage && <>
          <div className="feature-storage-meter"><span style={{ width: usedPercent + "%" }} /></div>
          <div className="feature-storage-metadata"><strong>{formatBytes(usage.used_bytes + usage.reserved_bytes)} used</strong><span>{formatBytes(usage.quota_bytes)} total · {usedPercent}%</span></div>
          <p className="feature-muted">{formatBytes(usage.reserved_bytes)} currently reserved for uploads in progress.</p>
        </>}
        {canManageStorage && usage && <form className="feature-quota-form" onSubmit={saveQuota}>
          <label>Workspace quota (MB)<input type="number" min="10" max="102400" step="10" value={Math.round(Number(quotaInput || usage.quota_bytes) / 1048576)} onChange={(event) => setQuotaInput(String(Number(event.target.value) * 1048576))} /></label>
          <button className="secondary-button" type="submit" disabled={savingQuota}>{savingQuota ? "Saving…" : "Update quota"}</button>
          <small>Workspace owners and admins can set between 10 MB and 100 GB.</small>
        </form>}
      </section>
      <section className="ref-panel feature-upload-panel">
        <div className="ref-panel-title"><h2><Upload size={16} /> Upload a file</h2><span className="feature-file-limit">10 MB max per file</span></div>
        <form className="feature-upload-form" onSubmit={upload}>
          <label className="feature-file-picker"><input type="file" onChange={(event) => setSelectedFile(event.target.files?.[0] || null)} required /><span><Upload size={20} /><strong>{selectedFile?.name || "Choose a file from your device"}</strong><small>PDF, images, presentations, documents, code, and more</small></span></label>
          <div className="feature-form-grid"><label>Attach to project<select value={boardId} onChange={(event) => { setBoardId(event.target.value); setTaskId(""); }} disabled={Boolean(taskId)}><option value="">General workspace file</option>{data.boards.map((board) => <option value={board.id} key={board.id}>{board.name}</option>)}</select></label>
          <label>Or attach to task<select value={taskId} onChange={(event) => { setTaskId(event.target.value); if (event.target.value) setBoardId(""); }}><option value="">No task attachment</option>{data.tasks.map((task) => <option value={task.id} key={task.id}>{task.title}</option>)}</select></label></div>
          <button className="ref-primary-button" type="submit" disabled={!selectedFile || uploading || (usage && usage.used_bytes + usage.reserved_bytes + selectedFile?.size > usage.quota_bytes)}><Upload size={15} /> {uploading ? "Uploading…" : "Upload to workspace"}</button>
          {usage && selectedFile && usage.used_bytes + usage.reserved_bytes + selectedFile.size > usage.quota_bytes && <p className="auth-error" role="alert">This file would exceed your workspace storage quota.</p>}
        </form>
        {error && <p className="auth-error" role="alert">{error}</p>}
      </section>
      <section className="ref-panel">
        <div className="ref-panel-title"><h2><Folder size={16} /> Workspace library</h2><span className="feature-count">{files.length} files</span></div>
        {loading ? <p className="feature-muted">Loading your workspace library…</p> : files.length ? <div className="feature-file-list">
          {files.map((file) => <article className="feature-file-row" key={file.id}>
            <span className="feature-file-icon"><FileText size={19} /></span>
            <div className="feature-file-copy"><strong>{file.original_name}</strong><small>{formatBytes(file.size_bytes)} · {file.media_type} · {displayDate(file.created_at, { month: "short", day: "numeric", year: "numeric" })}</small><small>{file.task_id ? "Attached to task" : file.board_id ? "Attached to project" : "Workspace file"} · {file.scan_status === "clean" ? "Scan passed" : file.scan_status === "infected" ? "Blocked" : "Not scanned"}</small></div>
            {["application/pdf","image/png","image/jpeg","image/gif","image/webp"].includes(file.media_type) && <button className="feature-icon-button" type="button" onClick={() => openPreview(file)} aria-label={"Preview " + file.original_name}><Eye size={16} /></button>}
            <button className="feature-icon-button" type="button" onClick={() => downloadFile(file)} aria-label={"Download " + file.original_name}><Download size={16} /></button>
            {(file.uploaded_by === data.current_user_id || canManageStorage) && <button className="feature-icon-button feature-danger-button" type="button" onClick={() => remove(file)} aria-label={"Delete " + file.original_name}><Trash2 size={16} /></button>}
          </article>)}
        </div> : <div className="feature-empty"><Folder size={24} /><strong>Your library is ready</strong><span>Upload your first shared project resource.</span></div>}
      </section>
      {canManageStorage && <section className="ref-panel">
        <div className="ref-panel-title"><h2><ShieldCheck size={16} /> File audit history</h2><button className="secondary-button" type="button" onClick={toggleAudit}>{auditOpen ? "Hide audit" : "View audit"}</button></div>
        {auditOpen && (audit.length ? <div className="feature-audit-list">{audit.map((entry) => <div className="feature-audit-row" key={entry.id}><span><strong>{entry.original_name}</strong><small>{entry.action.replaceAll("_", " ")} · {displayDate(entry.created_at, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })}</small></span><small>{entry.size_bytes ? formatBytes(entry.size_bytes) : "—"}</small></div>)}</div> : <p className="feature-muted">No file activity is recorded yet.</p>)}
      </section>}
      {preview && <div className="feature-preview-backdrop" role="presentation" onClick={() => setPreview(null)}>
        <section className="feature-preview-panel" role="dialog" aria-modal="true" aria-label={"Preview " + preview.name} onClick={(event) => event.stopPropagation()}>
          <div className="feature-modal-heading"><strong>{preview.name}</strong><button className="feature-icon-button" type="button" onClick={() => setPreview(null)} aria-label="Close preview"><X size={17} /></button></div>
          {preview.type === "application/pdf" ? <iframe title={"Preview " + preview.name} src={preview.url} /> : <img src={preview.url} alt={preview.name} />}
        </section>
      </div>}
      <p className="feature-muted">Production storage requires a private S3-compatible bucket. Configure ClamAV and set CAMPUSFORGE_REQUIRE_MALWARE_SCAN=true to block unscanned downloads.</p>
    </div>
  );
}

export function ChatAndMeetingsPage({ data, realtimeVersion = 0 }) {
  const [tab, setTab] = useState("chat");
  const [chatMode, setChatMode] = useState("team");
  const [boardId, setBoardId] = useState("");
  const [messages, setMessages] = useState([]);
  const [hasOlderMessages, setHasOlderMessages] = useState(false);
  const [loadingOlderMessages, setLoadingOlderMessages] = useState(false);
  const [conversations, setConversations] = useState([]);
  const [conversationId, setConversationId] = useState("");
  const [newDirectUserId, setNewDirectUserId] = useState("");
  const [pinnedMessages, setPinnedMessages] = useState([]);
  const [pinnedExpanded, setPinnedExpanded] = useState(false);
  const [notes, setNotes] = useState([]);
  const [message, setMessage] = useState("");
  const [mentionUserId, setMentionUserId] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [sending, setSending] = useState(false);
  const [editingMessageId, setEditingMessageId] = useState("");
  const [editingMessageBody, setEditingMessageBody] = useState("");
  const [noteForm, setNoteForm] = useState({ title: "", meeting_at: "", board_id: "", attendees: "", agenda: "", notes: "", action_items: "" });
  const [editingNoteId, setEditingNoteId] = useState("");
  const [savingNote, setSavingNote] = useState(false);
  const [expandedNoteId, setExpandedNoteId] = useState("");
  const [actionItems, setActionItems] = useState([]);
  const [actionForm, setActionForm] = useState({ title: "", assigned_to: "", due_at: "", linked_task_id: "" });
  const [savingAction, setSavingAction] = useState(false);
  const [presenceUsers, setPresenceUsers] = useState([]);
  const [typingUserIds, setTypingUserIds] = useState([]);
  const chatRequestRef = useRef(0);
  const notesRequestRef = useRef(0);
  const chatAdmin = data.workspace_members?.find((member) => member.user_id === data.current_user_id)?.role;
  const canModerateChat = chatAdmin === "owner" || chatAdmin === "admin";

  function displayMentions(body) {
    return (body || "").replace(/@\{([0-9a-f-]{36})\}/gi, (_match, id) => {
      const user = data.users.find((row) => row.id === id);
      return "@" + (user?.full_name || "workspace member");
    });
  }

  async function loadChat() {
    const requestId = ++chatRequestRef.current;
    setLoading(true);
    setError("");
    try {
      if (chatMode === "team") {
        const [rows, pins] = await Promise.all([chatApi.list(boardId || null, null, 50), chatApi.pinned(boardId || null)]);
        if (requestId === chatRequestRef.current) {
          setMessages(rows || []);
          setHasOlderMessages((rows || []).length === 50);
          setPinnedMessages(pins || []);
          setConversations([]);
        }
      } else {
        const rows = await chatApi.directConversations();
        if (requestId !== chatRequestRef.current) return;
        setConversations(rows || []);
        const active = conversationId || rows?.[0]?.id || "";
        if (!conversationId && active) setConversationId(active);
        if (!active) {
          setMessages([]);
          setHasOlderMessages(false);
          return;
        }
        const dmRows = await chatApi.directMessages(active, null, 50);
        if (requestId === chatRequestRef.current) {
          setMessages(dmRows || []);
          setHasOlderMessages((dmRows || []).length === 50);
        }
      }
    } catch (requestError) {
      if (requestId === chatRequestRef.current) setError(requestError.message || "Could not load chat.");
    } finally {
      if (requestId === chatRequestRef.current) setLoading(false);
    }
  }

  async function loadOlderMessages() {
    if (!hasOlderMessages || loadingOlderMessages || messages.length === 0) return;
    setLoadingOlderMessages(true);
    setError("");
    try {
      const before = messages[0].created_at;
      const rows = chatMode === "team"
        ? await chatApi.list(boardId || null, before, 50)
        : await chatApi.directMessages(conversationId, before, 50);
      setMessages((current) => [...(rows || []), ...current]);
      setHasOlderMessages((rows || []).length === 50);
    } catch (requestError) {
      setError(requestError.message || "Could not load older messages.");
    } finally {
      setLoadingOlderMessages(false);
    }
  }

  async function loadNotes() {
    const requestId = ++notesRequestRef.current;
    setLoading(true);
    setError("");
    try {
      const rows = await meetingNotesApi.list();
      if (requestId === notesRequestRef.current) setNotes(rows || []);
    } catch (requestError) {
      if (requestId === notesRequestRef.current) setError(requestError.message || "Could not load meeting notes.");
    } finally {
      if (requestId === notesRequestRef.current) setLoading(false);
    }
  }

  useEffect(() => {
    if (tab === "chat") loadChat();
    else loadNotes();
    return () => {
      chatRequestRef.current += 1;
      notesRequestRef.current += 1;
    };
  }, [tab, chatMode, boardId, conversationId, realtimeVersion]);

  useEffect(() => {
    if (tab !== "chat") return undefined;
    let stopped = false;
    const refreshPresence = () => {
      presenceApi.list().then((rows) => {
        if (!stopped) setPresenceUsers(rows || []);
      }).catch(() => {});
    };
    refreshPresence();
    const typingTimers = new Map();
    const unsubscribe = subscribeWorkspaceSocket((payload) => {
      if (payload.type !== "workspace_event" || !payload.event) return;
      const event = payload.event;
      if (event.event_type === "presence_joined" || event.event_type === "presence_left") {
        refreshPresence();
      }
      if (event.event_type === "typing_indicator" &&
          event.actor_user_id !== data.current_user_id &&
          chatMode === "team" &&
          (event.board_id || null) === (boardId || null)) {
        const userId = event.actor_user_id;
        if (userId) {
          setTypingUserIds((ids) => ids.includes(userId) ? ids : [...ids, userId]);
          if (typingTimers.has(userId)) window.clearTimeout(typingTimers.get(userId));
          typingTimers.set(userId, window.setTimeout(() => {
            setTypingUserIds((ids) => ids.filter((id) => id !== userId));
            typingTimers.delete(userId);
          }, 3500));
        }
      }
    });
    return () => {
      stopped = true;
      unsubscribe();
      typingTimers.forEach((timer) => window.clearTimeout(timer));
      typingTimers.clear();
    };
  }, [tab, chatMode, boardId, data.current_user_id]);

  async function startDirectConversation(userId) {
    if (!userId) return;
    setError("");
    try {
      const row = await chatApi.openDirect(userId);
      setChatMode("direct");
      setConversationId(row.id);
      setNewDirectUserId("");
      setTab("chat");
    } catch (requestError) {
      setError(requestError.message || "Could not open direct conversation.");
    }
  }

  async function sendMessage(event) {
    event.preventDefault();
    if (!message.trim() || sending) return;
    const body = message.trim();
    setSending(true);
    setError("");
    try {
      if (chatMode === "direct") {
        await chatApi.sendDirect(conversationId, body);
      } else {
        await chatApi.send({ board_id: boardId || null, body });
      }
      setMessage("");
      setMentionUserId("");
      await loadChat();
    } catch (requestError) {
      setError(requestError.message || "Could not send message.");
    } finally {
      setSending(false);
    }
  }

  function insertMention(userId) {
    if (!userId) return;
    setMessage((current) => (current ? current + " " : "") + "@{" + userId + "} ");
    setMentionUserId("");
  }

  async function saveMessageEdit(messageId) {
    if (!editingMessageBody.trim()) return;
    try {
      await chatApi.edit(messageId, editingMessageBody.trim());
      setEditingMessageId("");
      setEditingMessageBody("");
      await loadChat();
    } catch (requestError) { setError(requestError.message || "Could not edit message."); }
  }

  async function deleteMessage(row) {
    if (!window.confirm("Delete this message?")) return;
    try { await chatApi.remove(row.id); await loadChat(); }
    catch (requestError) { setError(requestError.message || "Could not delete message."); }
  }

  async function togglePin(row) {
    try {
      await chatApi.pin(row.id, !row.is_pinned);
      await loadChat();
    } catch (requestError) { setError(requestError.message || "Could not update pinned message."); }
  }

  function resetNoteForm() {
    setNoteForm({ title: "", meeting_at: "", board_id: "", attendees: "", agenda: "", notes: "", action_items: "" });
    setEditingNoteId("");
  }

  async function saveNote(event) {
    event.preventDefault();
    if (!noteForm.title.trim()) return;
    setSavingNote(true);
    setError("");
    try {
      const payload = {
        ...noteForm,
        board_id: noteForm.board_id || null,
        meeting_at: noteForm.meeting_at ? new Date(noteForm.meeting_at).toISOString() : null
      };
      if (editingNoteId) await meetingNotesApi.update(editingNoteId, payload);
      else await meetingNotesApi.create(payload);
      resetNoteForm();
      await loadNotes();
    } catch (requestError) { setError(requestError.message || "Could not save meeting notes."); }
    finally { setSavingNote(false); }
  }

  function editNote(note) {
    let meetingAt = "";
    if (note.meeting_at) {
      const parsed = new Date(note.meeting_at);
      meetingAt = new Date(parsed.getTime() - parsed.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
    }
    setEditingNoteId(note.id);
    setNoteForm({
      title: note.title, meeting_at: meetingAt, board_id: note.board_id || "",
      attendees: note.attendees || "", agenda: note.agenda || "", notes: note.notes || "", action_items: note.action_items || ""
    });
    setExpandedNoteId(note.id);
  }

  async function deleteNote(note) {
    if (!window.confirm("Delete meeting notes for " + note.title + "?")) return;
    try {
      await meetingNotesApi.remove(note.id);
      if (editingNoteId === note.id) resetNoteForm();
      if (expandedNoteId === note.id) setExpandedNoteId("");
      await loadNotes();
    } catch (requestError) { setError(requestError.message || "Could not delete meeting notes."); }
  }

  async function toggleNoteActions(note) {
    if (expandedNoteId === note.id) {
      setExpandedNoteId("");
      return;
    }
    setExpandedNoteId(note.id);
    setActionForm({ title: "", assigned_to: "", due_at: "", linked_task_id: "" });
    try {
      setActionItems(await meetingNotesApi.actionItems(note.id));
    } catch (requestError) { setError(requestError.message || "Could not load meeting action items."); }
  }

  async function createActionItem(event, note) {
    event.preventDefault();
    if (!actionForm.title.trim()) return;
    setSavingAction(true);
    try {
      await meetingNotesApi.createActionItem(note.id, {
        title: actionForm.title.trim(),
        assigned_to: actionForm.assigned_to || null,
        due_at: actionForm.due_at ? new Date(actionForm.due_at + "T23:59").toISOString() : null,
        linked_task_id: actionForm.linked_task_id || null
      });
      setActionForm({ title: "", assigned_to: "", due_at: "", linked_task_id: "" });
      setActionItems(await meetingNotesApi.actionItems(note.id));
    } catch (requestError) { setError(requestError.message || "Could not create action item."); }
    finally { setSavingAction(false); }
  }

  async function toggleActionItem(item) {
    try {
      await meetingNotesApi.updateActionItem(item.id, {
        title: item.title, assigned_to: item.assigned_to, due_at: item.due_at,
        linked_task_id: item.linked_task_id, is_done: !item.is_done
      });
      setActionItems((rows) => rows.map((row) => row.id === item.id ? { ...row, is_done: !row.is_done } : row));
    } catch (requestError) { setError(requestError.message || "Could not update action item."); }
  }

  async function removeActionItem(item) {
    try {
      await meetingNotesApi.removeActionItem(item.id);
      setActionItems((rows) => rows.filter((row) => row.id !== item.id));
    } catch (requestError) { setError(requestError.message || "Could not remove action item."); }
  }

  const activeConversation = conversations.find((row) => row.id === conversationId);

  return (
    <div className="feature-page">
      <PageHeading eyebrow="TEAM CONVERSATIONS" title="Chat & meeting notes" subtitle="Workspace chat, direct messages, pinned updates, meeting decisions and assigned action items." />
      <section className="ref-panel feature-collaboration-panel">
        <div className="feature-collab-toolbar">
          <div className="feature-segmented">
            <button type="button" className={tab === "chat" ? "selected" : ""} onClick={() => setTab("chat")}><MessageCircle size={14} /> Chat</button>
            <button type="button" className={tab === "notes" ? "selected" : ""} onClick={() => setTab("notes")}><FileText size={14} /> Meeting notes</button>
          </div>
          {tab === "chat" && <div className="feature-chat-toolbar-controls">
            <div className="feature-segmented">
              <button type="button" className={chatMode === "team" ? "selected" : ""} onClick={() => setChatMode("team")}>Team channels</button>
              <button type="button" className={chatMode === "direct" ? "selected" : ""} onClick={() => setChatMode("direct")}>Direct messages</button>
            </div>
            {chatMode === "team" ? <select value={boardId} onChange={(event) => setBoardId(event.target.value)} aria-label="Choose chat channel"><option value="">General workspace</option>{data.boards.map((board) => <option value={board.id} key={board.id}>{board.name}</option>)}</select> : <>
              <select value={newDirectUserId} onChange={(event) => startDirectConversation(event.target.value)} aria-label="Start a direct message"><option value="">Start direct message…</option>{data.users.filter((user) => user.id !== data.current_user_id).map((user) => <option key={user.id} value={user.id}>{user.full_name}</option>)}</select>
              {conversations.length > 0 && <select value={conversationId} onChange={(event) => setConversationId(event.target.value)} aria-label="Choose direct conversation">{conversations.map((row) => <option key={row.id} value={row.id}>{row.other_name}</option>)}</select>}
            </>}
          </div>}
        </div>
        {error && <p className="auth-error" role="alert">{error}</p>}
        {tab === "chat" ? (
          <div className="feature-chat">
            {chatMode === "team" && <section className="feature-pinned-panel">
              <button className="feature-pinned-heading" type="button" onClick={() => setPinnedExpanded((value) => !value)}><Pin size={14} /> Pinned announcements ({pinnedMessages.length}) <ChevronDown size={15} /></button>
              {pinnedExpanded && (pinnedMessages.length ? pinnedMessages.map((row) => <p key={row.id}><strong>{row.author_name}:</strong> {displayMentions(row.body)}</p>) : <p className="feature-muted">No pinned messages in this channel yet.</p>)}
            </section>}
            {chatMode === "direct" && activeConversation && <p className="feature-muted">Private conversation with {activeConversation.other_name}. Only the two participants can read these messages.</p>}
            {chatMode === "team" && <div className="feature-presence-strip"><span className="feature-presence-indicator" /> {presenceUsers.length} online</div>}
            {typingUserIds.length > 0 && <p className="feature-typing-indicator">{typingUserIds.map((id) => data.users.find((user) => user.id === id)?.full_name || "A teammate").join(", ")} {typingUserIds.length === 1 ? "is" : "are"} typing…</p>}
            <div className="feature-chat-messages">
              {hasOlderMessages && <button className="secondary-button feature-load-older" type="button" onClick={loadOlderMessages} disabled={loadingOlderMessages}>{loadingOlderMessages ? "Loading older messages…" : "Load older messages"}</button>}
              {loading ? <p className="feature-muted">Loading conversation…</p> : messages.length ? messages.map((row) => {
                const isOwn = row.author_id === data.current_user_id;
                const canDelete = isOwn || (chatMode === "team" && canModerateChat);
                const canPin = chatMode === "team" && (isOwn || canModerateChat);
                return <article className={"feature-chat-message" + (row.deleted_at ? " is-deleted" : "")} key={row.id}>
                  <span className="ref-avatar">{(row.author_name || "M").split(/\s+/).map((part) => part[0]).slice(0,2).join("").toUpperCase()}</span>
                  <div className="feature-chat-message-body">
                    <div className="feature-chat-meta"><strong>{row.author_name}</strong><time>{displayDate(row.created_at, { month: "short", day: "numeric", hour: "numeric", minute: "2-digit" })}{row.edited_at ? " · edited" : ""}</time>{row.is_pinned && <span className="feature-pinned-badge"><Pin size={11} /> Pinned</span>}</div>
                    {editingMessageId === row.id ? <div className="feature-message-edit"><textarea rows={2} value={editingMessageBody} onChange={(event) => setEditingMessageBody(event.target.value)} maxLength={4000} /><div><button type="button" className="secondary-button" onClick={() => setEditingMessageId("")}>Cancel</button><button type="button" className="ref-primary-button" onClick={() => saveMessageEdit(row.id)}>Save</button></div></div> : <p>{displayMentions(row.body)}</p>}
                    {!row.body?.includes("This message was deleted") && editingMessageId !== row.id && <div className="feature-message-actions">
                      {isOwn && <button type="button" onClick={() => { setEditingMessageId(row.id); setEditingMessageBody(row.body); }}><Pencil size={12} /> Edit</button>}
                      {canDelete && <button type="button" onClick={() => deleteMessage(row)}><Trash2 size={12} /> Delete</button>}
                      {canPin && <button type="button" onClick={() => togglePin(row)}><Pin size={12} /> {row.is_pinned ? "Unpin" : "Pin"}</button>}
                    </div>}
                  </div>
                </article>;
              }) : <div className="feature-empty"><MessageCircle size={24} /><strong>{chatMode === "direct" && !activeConversation ? "Choose a teammate" : "No messages yet"}</strong><span>{chatMode === "direct" && !activeConversation ? "Start a private conversation from the member selector." : "Start a conversation with your team."}</span></div>}
            </div>
            <form className={"feature-chat-composer" + (chatMode === "direct" ? " is-direct" : "")} onSubmit={sendMessage}>
              {chatMode === "team" && <div className="feature-chat-compose-tools">
                <select value={mentionUserId} onChange={(event) => insertMention(event.target.value)} aria-label="Mention a teammate"><option value="">Mention teammate…</option>{data.users.filter((user) => user.id !== data.current_user_id).map((user) => <option key={user.id} value={user.id}>{user.full_name}</option>)}</select>
                <small>Mentions notify the selected team member.</small>
              </div>}
              <div><input value={message} onChange={(event) => { setMessage(event.target.value); if (chatMode === "team") sendWorkspaceSocketMessage({ type: "typing", board_id: boardId || null }); }} placeholder={chatMode === "direct" ? "Write a private message…" : "Write a message to your team…"} maxLength={4000} /><button className="ref-primary-button" type="submit" disabled={!message.trim() || sending || (chatMode === "direct" && !conversationId)}><Send size={15} /> Send</button></div>
            </form>
          </div>
        ) : (
          <div className="feature-notes">
            <form className="feature-form feature-note-form" onSubmit={saveNote}>
              <h3><Plus size={16} /> {editingNoteId ? "Edit meeting notes" : "New meeting notes"}</h3>
              <label>Meeting title<input value={noteForm.title} onChange={(e) => setNoteForm({ ...noteForm, title: e.target.value })} placeholder="Weekly project sync" maxLength={180} required /></label>
              <div className="feature-form-grid"><label>Date and time<input type="datetime-local" value={noteForm.meeting_at} onChange={(e) => setNoteForm({ ...noteForm, meeting_at: e.target.value })} /></label><label>Project<select value={noteForm.board_id} onChange={(e) => setNoteForm({ ...noteForm, board_id: e.target.value })}><option value="">Workspace-wide</option>{data.boards.map((board) => <option key={board.id} value={board.id}>{board.name}</option>)}</select></label></div>
              <label>Attendees<input value={noteForm.attendees} onChange={(e) => setNoteForm({ ...noteForm, attendees: e.target.value })} placeholder="Team members" maxLength={1000} /></label>
              <label>Agenda<textarea rows={2} value={noteForm.agenda} onChange={(e) => setNoteForm({ ...noteForm, agenda: e.target.value })} placeholder="Topics to discuss…" maxLength={12000} /></label>
              <label>Discussion notes<textarea rows={3} value={noteForm.notes} onChange={(e) => setNoteForm({ ...noteForm, notes: e.target.value })} placeholder="Decisions and discussion…" maxLength={12000} /></label>
              <label>Legacy action summary (optional)<textarea rows={2} value={noteForm.action_items} onChange={(e) => setNoteForm({ ...noteForm, action_items: e.target.value })} placeholder="Short summary…" maxLength={12000} /></label>
              <div className="feature-form-actions">{editingNoteId && <button className="secondary-button" type="button" onClick={resetNoteForm}>Cancel edit</button>}<button className="ref-primary-button" type="submit" disabled={savingNote}>{savingNote ? "Saving…" : editingNoteId ? "Update meeting notes" : "Save meeting notes"}</button></div>
            </form>
            <div className="feature-note-list">
              <h3>Saved notes</h3>
              {loading ? <p className="feature-muted">Loading meeting notes…</p> : notes.length ? notes.map((note) => <article className="feature-note-card" key={note.id}>
                <div className="feature-note-card-head"><div><strong>{note.title}</strong><small>{note.meeting_at ? displayDate(note.meeting_at, { weekday: "short", month: "short", day: "numeric", hour: "numeric", minute: "2-digit" }) : "No date set"}</small></div><div className="feature-note-actions"><button className="feature-icon-button" type="button" onClick={() => editNote(note)} aria-label={"Edit " + note.title}><Pencil size={15} /></button><button className="feature-icon-button feature-danger-button" type="button" onClick={() => deleteNote(note)} aria-label={"Delete " + note.title}><Trash2 size={15} /></button></div></div>
                {note.agenda && <p><b>Agenda:</b> {note.agenda}</p>}{note.notes && <p><b>Notes:</b> {note.notes}</p>}{note.action_items && <p><b>Action summary:</b> {note.action_items}</p>}
                <button className="feature-action-toggle" type="button" onClick={() => toggleNoteActions(note)}><ListChecks size={14} /> {expandedNoteId === note.id ? "Hide action items" : "Manage action items"}</button>
                {expandedNoteId === note.id && <div className="feature-action-items">
                  {actionItems.map((item) => <div className="feature-action-item" key={item.id}>
                    <label><input type="checkbox" checked={item.is_done} onChange={() => toggleActionItem(item)} /><span className={item.is_done ? "is-done" : ""}>{item.title}</span></label>
                    <small>{item.assigned_to ? data.users.find((user) => user.id === item.assigned_to)?.full_name || "Assigned member" : "Unassigned"}{item.due_at ? " · due " + displayDate(item.due_at) : ""}{item.linked_task_id ? " · linked task" : ""}</small>
                    <button className="feature-icon-button feature-danger-button" type="button" onClick={() => removeActionItem(item)} aria-label="Delete action item"><Trash2 size={13} /></button>
                  </div>)}
                  <form className="feature-action-form" onSubmit={(event) => createActionItem(event, note)}>
                    <label>Action title<input value={actionForm.title} onChange={(event) => setActionForm({ ...actionForm, title: event.target.value })} placeholder="Prepare presentation draft" maxLength={500} required /></label>
                    <div className="feature-form-grid"><label>Owner<select value={actionForm.assigned_to} onChange={(event) => setActionForm({ ...actionForm, assigned_to: event.target.value })}><option value="">Unassigned</option>{data.users.map((user) => <option key={user.id} value={user.id}>{user.full_name}</option>)}</select></label><label>Due date<input type="date" value={actionForm.due_at} onChange={(event) => setActionForm({ ...actionForm, due_at: event.target.value })} /></label></div>
                    <label>Link to task (optional)<select value={actionForm.linked_task_id} onChange={(event) => setActionForm({ ...actionForm, linked_task_id: event.target.value })}><option value="">No linked task</option>{data.tasks.map((task) => <option key={task.id} value={task.id}>{task.title}</option>)}</select></label>
                    <button className="secondary-button" type="submit" disabled={savingAction || !actionForm.title.trim()}>{savingAction ? "Adding…" : "Add action item"}</button>
                  </form>
                </div>}
              </article>) : <p className="feature-muted">No meeting notes have been saved yet.</p>}
            </div>
          </div>
        )}
      </section>
    </div>
  );
}


const LOCKER_FILE_LIMIT_BYTES = 4 * 1024 * 1024;

function lockerTypeIcon(type) {
  if (type === "file") return FileText;
  if (type === "link") return Link2;
  if (type === "code") return Code2;
  return FileText;
}

export function SaveLockerPage({ data }) {
  const [vaultState, setVaultState] = useState(null);
  const [unlockedKey, setUnlockedKey] = useState(null);
  const [items, setItems] = useState([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [confirmPassphrase, setConfirmPassphrase] = useState("");
  const [showPassphrase, setShowPassphrase] = useState(false);
  const [search, setSearch] = useState("");
  const [showForm, setShowForm] = useState(false);
  const [selectedFile, setSelectedFile] = useState(null);
  const [selectedItem, setSelectedItem] = useState(null);
  const [form, setForm] = useState({
    title: "", type: "note", projectId: "", url: "", content: "", language: "text"
  });

  useEffect(() => {
    let active = true;
    lockerApi.getVault()
      .then((result) => { if (active) setVaultState(result); })
      .catch((requestError) => { if (active) setError(requestError.message || "Could not load your Save Locker."); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, []);

  async function decryptItemRows(key, rows) {
    return Promise.all((rows || []).map(async (row) => {
      try {
        const payload = await decryptLockerPayload(key, row);
        return { ...row, payload, corrupt: false };
      } catch {
        return { ...row, payload: null, corrupt: true };
      }
    }));
  }

  async function refreshItems(key = unlockedKey) {
    const rows = await lockerApi.listItems();
    const decrypted = await decryptItemRows(key, rows);
    setItems(decrypted);
    setVaultState((current) => current ? {
      ...current,
      item_count: rows.length,
      used_bytes: rows.reduce((total, row) => total + Number(row.size_bytes || 0), 0)
    } : current);
  }

  async function createLocker(event) {
    event.preventDefault();
    setError("");
    setNotice("");
    if (passphrase.length < 12) {
      setError("Choose a unique locker passphrase with at least 12 characters.");
      return;
    }
    if (passphrase !== confirmPassphrase) {
      setError("The passphrases do not match.");
      return;
    }
    setBusy(true);
    try {
      const salt = randomBytes(16);
      const key = await deriveLockerKey(passphrase, salt);
      const verifier = await createLockerKeyCheck(key, data.current_user_id);
      const result = await lockerApi.createVault({
        salt: bytesToBase64(salt),
        kdf_iterations: LOCKER_KDF_ITERATIONS,
        key_check_iv: verifier.iv,
        key_check_ciphertext: verifier.ciphertext
      });
      setVaultState(result);
      setUnlockedKey(key);
      setPassphrase("");
      setConfirmPassphrase("");
      setNotice("Your encrypted Save Locker is ready.");
      setItems([]);
    } catch (requestError) {
      setError(requestError.message || "Could not create your Save Locker.");
    } finally {
      setBusy(false);
    }
  }

  async function unlockLocker(event) {
    event.preventDefault();
    setError("");
    setNotice("");
    if (!vaultState?.vault) return;
    setBusy(true);
    try {
      const salt = base64ToBytes(vaultState.vault.salt);
      const key = await deriveLockerKey(passphrase, salt);
      const valid = await verifyLockerKey(key, {
        iv: vaultState.vault.key_check_iv,
        ciphertext: vaultState.vault.key_check_ciphertext
      }, data.current_user_id);
      setPassphrase("");
      if (!valid) throw new Error("That locker passphrase is incorrect.");
      setUnlockedKey(key);
      await refreshItems(key);
      setNotice("Save Locker unlocked on this device.");
    } catch (requestError) {
      setError(requestError.message || "Could not unlock your Save Locker.");
    } finally {
      setBusy(false);
    }
  }

  function lockLocker() {
    setUnlockedKey(null);
    setItems([]);
    setPassphrase("");
    setConfirmPassphrase("");
    setSearch("");
    setSelectedItem(null);
    setShowForm(false);
    setNotice("Save Locker locked. Its key has been cleared from this page.");
  }

  async function resetLocker() {
    const confirmed = window.confirm(
      "Permanently delete this Save Locker and every encrypted item in it? This cannot be undone."
    );
    if (!confirmed) return;
    setBusy(true);
    setError("");
    try {
      await lockerApi.deleteVault();
      setUnlockedKey(null);
      setItems([]);
      setSelectedItem(null);
      setVaultState({ exists: false, vault: null, item_count: 0, used_bytes: 0 });
      setNotice("Save Locker deleted. You can create a new one with a new passphrase.");
    } catch (requestError) {
      setError(requestError.message || "Could not delete Save Locker.");
    } finally {
      setBusy(false);
    }
  }

  async function saveItem(event) {
    event.preventDefault();
    if (!unlockedKey) return;
    setError("");
    setNotice("");
    if (!form.title.trim()) {
      setError("Add a title for this item.");
      return;
    }
    if ((form.type === "note" || form.type === "code") && !form.content.trim()) {
      setError("Add some content before saving.");
      return;
    }
    if (form.type === "link") {
      try {
        const parsed = new URL(form.url);
        if (!["http:", "https:"].includes(parsed.protocol)) throw new Error("invalid protocol");
      } catch {
        setError("Enter a valid HTTP or HTTPS link.");
        return;
      }
    }
    if (form.type === "file" && !selectedFile) {
      setError("Choose a file to save.");
      return;
    }
    if (selectedFile && selectedFile.size > LOCKER_FILE_LIMIT_BYTES) {
      setError("Locker files are limited to 4 MB each.");
      return;
    }

    setBusy(true);
    try {
      const project = data.boards.find((board) => board.id === form.projectId);
      const payload = {
        version: 1,
        title: form.title.trim(),
        type: form.type,
        project: project ? { id: project.id, name: project.name } : null,
        content: form.content,
        url: form.type === "link" ? form.url.trim() : "",
        language: form.type === "code" ? form.language : "",
        file: form.type === "file" && selectedFile ? {
          name: selectedFile.name,
          media_type: selectedFile.type || "application/octet-stream",
          data: bytesToBase64(await selectedFile.arrayBuffer())
        } : null
      };
      const encrypted = await encryptLockerPayload(unlockedKey, payload);
      const saved = await lockerApi.createItem(encrypted);
      setItems((current) => [{ ...saved, payload, corrupt: false }, ...current]);
      setVaultState((current) => current ? {
        ...current,
        item_count: Number(current.item_count || 0) + 1,
        used_bytes: Number(current.used_bytes || 0) + Number(saved.size_bytes || encrypted.ciphertext.length)
      } : current);
      setForm({ title: "", type: "note", projectId: "", url: "", content: "", language: "text" });
      setSelectedFile(null);
      setShowForm(false);
      setNotice("Item encrypted and saved to your locker.");
    } catch (requestError) {
      setError(requestError.message || "Could not save this item.");
    } finally {
      setBusy(false);
    }
  }

  async function deleteItem(item) {
    const title = item.payload?.title || "this encrypted item";
    if (!window.confirm("Delete " + title + " permanently?")) return;
    setError("");
    try {
      await lockerApi.removeItem(item.id);
      setItems((current) => current.filter((row) => row.id !== item.id));
      if (selectedItem?.id === item.id) setSelectedItem(null);
      setVaultState((current) => current ? {
        ...current,
        item_count: Math.max(0, Number(current.item_count || 0) - 1),
        used_bytes: Math.max(0, Number(current.used_bytes || 0) - Number(item.size_bytes || 0))
      } : current);
      setNotice("Encrypted item deleted.");
    } catch (requestError) {
      setError(requestError.message || "Could not delete this item.");
    }
  }

  function downloadItem(item) {
    if (!item.payload?.file?.data) return;
    try {
      const bytes = base64ToBytes(item.payload.file.data);
      const blob = new Blob([bytes], { type: item.payload.file.media_type || "application/octet-stream" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = item.payload.file.name || item.payload.title || "locker-file";
      document.body.appendChild(anchor);
      anchor.click();
      anchor.remove();
      window.setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch {
      setError("This encrypted file could not be prepared for download.");
    }
  }

  const filteredItems = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return items;
    return items.filter((item) => {
      if (item.corrupt || !item.payload) return false;
      const payload = item.payload;
      return [payload.title, payload.project?.name, payload.content, payload.url]
        .filter(Boolean).join(" ").toLowerCase().includes(query);
    });
  }, [items, search]);

  if (loading) {
    return <div className="feature-page"><PageHeading eyebrow="PRIVATE STORAGE" title="Save Locker" subtitle="Loading your encrypted personal locker…" /><section className="ref-panel"><p className="feature-muted">Checking your locker status…</p></section></div>;
  }

  if (!vaultState?.exists) {
    return (
      <div className="feature-page">
        <PageHeading eyebrow="PRIVATE STORAGE" title="Save Locker" subtitle="A personal vault for important project notes, links, code snippets and files." />
        <section className="ref-panel locker-access-panel">
          <div className="locker-access-icon"><LockKeyhole size={25} /></div>
          <h2>Create your private locker</h2>
          <p>Choose a separate passphrase. CampusForge encrypts your items in this browser before anything is sent to the server.</p>
          <form className="locker-access-form" onSubmit={createLocker}>
            <label>Locker passphrase
              <input type="password" value={passphrase} onChange={(event) => setPassphrase(event.target.value)} autoComplete="new-password" minLength={12} required />
            </label>
            <label>Confirm locker passphrase
              <input type="password" value={confirmPassphrase} onChange={(event) => setConfirmPassphrase(event.target.value)} autoComplete="new-password" minLength={12} required />
            </label>
            {error && <p className="auth-error" role="alert">{error}</p>}
            <button className="ref-primary-button" type="submit" disabled={busy || passphrase.length < 12 || !confirmPassphrase}>
              <KeyRound size={15} /> {busy ? "Creating encrypted locker…" : "Create Save Locker"}
            </button>
          </form>
          <div className="locker-warning"><ShieldCheck size={16} /><span>Use a unique, long passphrase that you can remember. It is not your CampusForge login password.</span></div>
          <p className="locker-recovery-warning">There is no password reset or recovery key. If you forget this passphrase, your saved items cannot be decrypted. Keep a safe copy of the passphrase.</p>
        </section>
        {error && <p className="auth-error" role="alert">{error}</p>}
      </div>
    );
  }

  if (!unlockedKey) {
    return (
      <div className="feature-page">
        <PageHeading eyebrow="PRIVATE STORAGE" title="Save Locker" subtitle="Your items are encrypted. Unlock the locker to view them on this device." />
        <section className="ref-panel locker-access-panel">
          <div className="locker-access-icon"><Lock size={25} /></div>
          <h2>Unlock your locker</h2>
          <p>Enter the separate passphrase you used when this locker was created.</p>
          <form className="locker-access-form" onSubmit={unlockLocker}>
            <label>Locker passphrase
              <input type="password" value={passphrase} onChange={(event) => setPassphrase(event.target.value)} autoComplete="current-password" required />
            </label>
            {error && <p className="auth-error" role="alert">{error}</p>}
            <button className="ref-primary-button" type="submit" disabled={busy || !passphrase}>
              <LockKeyhole size={15} /> {busy ? "Unlocking…" : "Unlock Save Locker"}
            </button>
          </form>
          <div className="locker-warning"><ShieldCheck size={16} /><span>The decryption key stays in this page's memory and is not saved to your account or browser storage.</span></div>
          <button className="locker-reset-button" type="button" onClick={resetLocker} disabled={busy}>Forgot passphrase? Delete this locker and start again</button>
        </section>
      </div>
    );
  }

  return (
    <div className="feature-page">
      <PageHeading
        eyebrow="PRIVATE STORAGE"
        title="Save Locker"
        subtitle="Your personal project vault. Items are decrypted locally only while this locker is unlocked."
        action={<button className="secondary-button" type="button" onClick={lockLocker}><Lock size={15} /> Lock locker</button>}
      />
      <section className="locker-security-banner">
        <ShieldCheck size={19} />
        <div><strong>End-to-end encrypted</strong><span>Only this browser session holds the key. Server storage contains encrypted item data, not readable project content.</span></div>
        <span className="locker-encryption-label">AES-256-GCM</span>
      </section>
      <div className="locker-summary-row">
        <div className="ref-panel locker-summary"><strong>{items.length}</strong><span>Encrypted items</span></div>
        <div className="ref-panel locker-summary"><strong>{formatBytes(items.reduce((sum, item) => sum + Number(item.size_bytes || 0), 0))}</strong><span>Encrypted storage used</span></div>
        <div className="ref-panel locker-summary"><strong>100</strong><span>Item limit</span></div>
      </div>
      <div className="locker-toolbar">
        <label className="locker-search"><Search size={16} /><input aria-label="Search decrypted locker items" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search your saved items…" /></label>
        <button className="ref-primary-button" type="button" onClick={() => { setShowForm((value) => !value); setError(""); }}>
          <Plus size={15} /> {showForm ? "Close form" : "Save an item"}
        </button>
      </div>
      {notice && <p className="locker-notice" role="status">{notice}</p>}
      {error && <p className="auth-error" role="alert">{error}</p>}
      {showForm && (
        <section className="ref-panel locker-compose-panel">
          <div className="ref-panel-title"><h2><Plus size={16} /> Save a project item</h2><span className="feature-file-limit">Encrypted before upload</span></div>
          <form className="locker-compose-form" onSubmit={saveItem}>
            <div className="locker-form-grid">
              <label>Item title<input value={form.title} onChange={(event) => setForm((current) => ({ ...current, title: event.target.value }))} maxLength={160} required /></label>
              <label>Item type<select value={form.type} onChange={(event) => { setForm((current) => ({ ...current, type: event.target.value })); setSelectedFile(null); }}><option value="note">Note</option><option value="link">Link</option><option value="code">Code snippet</option><option value="file">File</option></select></label>
              <label className="locker-project-field">Related project (optional)<select value={form.projectId} onChange={(event) => setForm((current) => ({ ...current, projectId: event.target.value }))}><option value="">Personal / unassigned</option>{(data.boards || []).map((board) => <option value={board.id} key={board.id}>{board.name}</option>)}</select></label>
              {form.type === "link" && <label className="locker-project-field">URL<input type="url" value={form.url} onChange={(event) => setForm((current) => ({ ...current, url: event.target.value }))} placeholder="https://…" required /></label>}
              {form.type === "code" && <label>Language<input value={form.language} onChange={(event) => setForm((current) => ({ ...current, language: event.target.value }))} placeholder="Rust, JavaScript, SQL…" maxLength={40} /></label>}
            </div>
            {form.type === "file" ? (
              <label className="locker-file-picker"><Upload size={20} /><strong>{selectedFile?.name || "Choose a file to encrypt"}</strong><small>Files up to 4 MB. File name and contents are encrypted together.</small><input type="file" onChange={(event) => setSelectedFile(event.target.files?.[0] || null)} required /></label>
            ) : (
              <label className="locker-content-field">{form.type === "link" ? "Description (optional)" : form.type === "code" ? "Code" : "Note content"}
                <textarea rows={form.type === "code" ? 9 : 6} value={form.content} onChange={(event) => setForm((current) => ({ ...current, content: event.target.value }))} maxLength={200000} required={form.type !== "link"} placeholder={form.type === "code" ? "Paste your code snippet…" : form.type === "link" ? "Why did you save this link?" : "Write your private project note…"} />
              </label>
            )}
            <div className="dialog-actions">
              <span className="muted-copy">Titles, project names, links and file details are encrypted.</span>
              <button className="ref-primary-button" type="submit" disabled={busy || !form.title.trim()}><LockKeyhole size={15} /> {busy ? "Encrypting and saving…" : "Encrypt & save"}</button>
            </div>
          </form>
        </section>
      )}
      {filteredItems.length ? (
        <div className="locker-grid">
          {filteredItems.map((item) => {
            const Icon = lockerTypeIcon(item.payload?.type);
            return (
              <article className={"locker-item-card" + (item.corrupt ? " locker-item-corrupt" : "")} key={item.id}>
                <div className="locker-card-top"><span className="locker-card-icon"><Icon size={17} /></span><span className="locker-type">{item.corrupt ? "Unreadable" : item.payload?.type || "Item"}</span><button className="feature-icon-button" type="button" onClick={() => deleteItem(item)} aria-label="Delete encrypted item"><Trash2 size={15} /></button></div>
                <h3>{item.corrupt ? "Encrypted item could not be opened" : item.payload?.title || "Untitled item"}</h3>
                <p>{item.corrupt ? "This item may be damaged or encrypted with another key." : item.payload?.project?.name || "Personal item"}</p>
                <div className="locker-card-bottom"><small>{displayDate(item.updated_at, { month: "short", day: "numeric", year: "numeric" })}</small><button type="button" className="text-button" onClick={() => setSelectedItem(item)} disabled={item.corrupt}>Open <Eye size={14} /></button></div>
              </article>
            );
          })}
        </div>
      ) : (
        <div className="locker-empty"><LockKeyhole size={25} /><strong>{search ? "No matching saved items" : "Your locker is empty"}</strong><span>{search ? "Try a different search." : "Save a note, link, code snippet or project file. It is encrypted before it leaves your browser."}</span></div>
      )}
      <div className="locker-footer-actions">
        <span>Encrypted storage: {formatBytes(items.reduce((sum, item) => sum + Number(item.size_bytes || 0), 0))} of 50 MB</span>
        <button type="button" className="locker-reset-button" onClick={resetLocker} disabled={busy}>Delete locker and all items</button>
      </div>
      {selectedItem && !selectedItem.corrupt && (
        <div className="locker-modal-backdrop" role="presentation" onClick={() => setSelectedItem(null)}>
          <section className="locker-modal" role="dialog" aria-modal="true" aria-label={selectedItem.payload.title} onClick={(event) => event.stopPropagation()}>
            <div className="feature-modal-heading"><div><p className="ref-eyebrow">DECRYPTED LOCALLY</p><strong>{selectedItem.payload.title}</strong></div><button className="feature-icon-button" type="button" onClick={() => setSelectedItem(null)} aria-label="Close item"><X size={17} /></button></div>
            <div className="locker-modal-meta"><span>{selectedItem.payload.type}</span><span>{selectedItem.payload.project?.name || "Personal item"}</span></div>
            {selectedItem.payload.type === "link" && <p><a href={selectedItem.payload.url} target="_blank" rel="noopener noreferrer">{selectedItem.payload.url}</a></p>}
            {selectedItem.payload.type === "file" && (
              <div className="locker-file-download"><FileText size={22} /><strong>{selectedItem.payload.file?.name || selectedItem.payload.title}</strong><button className="ref-primary-button" type="button" onClick={() => downloadItem(selectedItem)}><Download size={15} /> Decrypt & download</button></div>
            )}
            {selectedItem.payload.content && <pre className={"locker-modal-content" + (selectedItem.payload.type === "code" ? " locker-code-content" : "")}>{selectedItem.payload.content}</pre>}
            {selectedItem.payload.type === "code" && selectedItem.payload.language && <small className="locker-language">Language: {selectedItem.payload.language}</small>}
            <div className="dialog-actions"><span className="muted-copy">This content is decrypted only in this browser session.</span><button className="secondary-button" type="button" onClick={() => deleteItem(selectedItem)}><Trash2 size={14} /> Delete item</button></div>
          </section>
        </div>
      )}
    </div>
  );
}
