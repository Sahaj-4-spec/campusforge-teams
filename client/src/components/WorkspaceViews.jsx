import { useEffect, useMemo, useState } from "react";
import {
  Activity, ArrowRight, ArrowUpRight, BarChart3, CalendarDays, CheckCircle2,
  CircleAlert, Clock3, FolderKanban, ListTodo, Plus, Users, Zap
} from "lucide-react";
import { analyticsApi } from "../lib/api.js";

const safeArray = (value) => Array.isArray(value) ? value : [];
const todayKey = () => new Date().toISOString().slice(0, 10);
const isDoneList = (list) => /^(done|completed|complete)$/i.test((list?.name || "").trim());
const listName = (task, lists) => lists.find((list) => list.id === task.list_id)?.name || "Unsorted";
const initials = (value) => (value || "Member").trim().split(/\s+/).slice(0, 2).map((part) => part[0]).join("").toUpperCase();
const shortDate = (value) => value ? new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" }).format(new Date(value + "T00:00:00")) : "No due date";
const relativeTime = (value) => {
  const elapsed = Math.max(0, Date.now() - new Date(value).getTime());
  const minutes = Math.floor(elapsed / 60000);
  if (minutes < 1) return "Just now";
  if (minutes < 60) return minutes + "m ago";
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return hours + "h ago";
  return Math.floor(hours / 24) + "d ago";
};
const eventDate = (value) => new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric", year: "numeric" }).format(new Date(value));
const isOverdue = (task, doneIds, today = todayKey()) => Boolean(task.due_date && task.due_date < today && !doneIds.has(task.id));

function getWorkspaceMetrics(data) {
  const tasks = safeArray(data.tasks);
  const lists = safeArray(data.lists);
  const boards = safeArray(data.boards);
  const users = safeArray(data.users);
  const assignments = safeArray(data.task_assignees);
  const events = safeArray(data.activity_events);
  const doneListIds = new Set(lists.filter(isDoneList).map((list) => list.id));
  const doneTasks = tasks.filter((task) => doneListIds.has(task.list_id));
  const doneIds = new Set(doneTasks.map((task) => task.id));
  const openTasks = tasks.filter((task) => !doneIds.has(task.id));
  const overdueTasks = openTasks.filter((task) => isOverdue(task, doneIds));
  const today = todayKey();
  const week = new Date();
  week.setDate(week.getDate() + 7);
  const dueSoon = openTasks.filter((task) => task.due_date && task.due_date >= today && task.due_date <= week.toISOString().slice(0, 10));
  const inProgress = openTasks.filter((task) => /progress|doing|active/i.test(listName(task, lists))).length;
  const completion = tasks.length ? Math.round(doneTasks.length / tasks.length * 100) : 0;
  return { tasks, lists, boards, users, assignments, events, doneListIds, doneTasks, doneIds, openTasks, overdueTasks, dueSoon, inProgress, completion };
}

function Avatar({ name, small = false }) {
  return <span className={"ref-avatar" + (small ? " ref-avatar-small" : "")} aria-label={name || "Team member"}>{initials(name)}</span>;
}

function PageHeading({ eyebrow, title, subtitle, action }) {
  return (
    <div className="ref-page-heading">
      <div>
        <p className="ref-eyebrow">{eyebrow}</p>
        <h1>{title}</h1>
        <p className="ref-subtitle">{subtitle}</p>
      </div>
      {action}
    </div>
  );
}

function StatCard({ icon: Icon, label, value, note, tone = "lime" }) {
  return (
    <article className={"ref-stat-card ref-stat-" + tone}>
      <div className="ref-stat-top"><span className="ref-stat-icon"><Icon size={17} /></span><span className="ref-stat-note">{note}</span></div>
      <strong>{value}</strong>
      <span className="ref-stat-label">{label}</span>
    </article>
  );
}

function PanelTitle({ icon: Icon, title, action }) {
  return <div className="ref-panel-title"><h2><Icon size={16} />{title}</h2>{action}</div>;
}

function ProgressBar({ value, tone = "lime" }) {
  return <span className={"ref-progress ref-progress-" + tone}><i style={{ width: Math.max(0, Math.min(100, value)) + "%" }} /></span>;
}

function BoardSummary({ board, index, metrics, onOpenBoard }) {
  const boardTasks = metrics.tasks.filter((task) => task.board_id === board.id);
  const completed = boardTasks.filter((task) => metrics.doneIds.has(task.id)).length;
  const progress = boardTasks.length ? Math.round(completed / boardTasks.length * 100) : 0;
  const palette = ["lime", "violet", "blue", "orange"];
  const tone = palette[index % palette.length];
  return (
    <button className="ref-project-row" type="button" onClick={() => onOpenBoard(board)}>
      <span className={"ref-project-mark ref-mark-" + tone}><FolderKanban size={18} /></span>
      <span className="ref-project-main">
        <strong>{board.name}</strong>
        <small>{board.description || (boardTasks.length + " tasks in this project")}</small>
        <ProgressBar value={progress} tone={tone} />
      </span>
      <span className="ref-project-result"><strong>{progress}%</strong><small>{completed}/{boardTasks.length} done</small></span>
      <ArrowRight size={16} className="ref-row-arrow" />
    </button>
  );
}

function TaskLine({ task, lists, onOpenTask }) {
  return (
    <button type="button" className="ref-task-row" onClick={() => onOpenTask(task)}>
      <span className={"ref-priority-dot ref-priority-" + (task.priority || "normal")} />
      <span className="ref-task-copy"><strong>{task.title}</strong><small>{listName(task, lists)} · {shortDate(task.due_date)}</small></span>
      <ArrowUpRight size={15} className="ref-row-arrow" />
    </button>
  );
}

function ActivityLine({ event, users, compact = false }) {
  const actor = users.find((user) => user.id === event.actor_id);
  return (
    <article className={"ref-activity-row" + (compact ? " ref-activity-compact" : "")}>
      <Avatar name={actor?.full_name || "Team member"} small />
      <div className="ref-activity-content">
        <p><strong>{actor?.full_name || "A team member"}</strong> {event.description || "updated the workspace"}</p>
        <time>{relativeTime(event.created_at)}</time>
      </div>
    </article>
  );
}

function WorkloadPanel({ metrics, title = "Team workload" }) {
  const counts = metrics.users.map((user) => {
    const assigned = new Set(metrics.assignments.filter((row) => row.user_id === user.id).map((row) => row.task_id));
    const count = metrics.openTasks.filter((task) => assigned.has(task.id)).length;
    return { user, count };
  }).sort((a, b) => b.count - a.count).slice(0, 5);
  const max = Math.max(1, ...counts.map((item) => item.count));
  return (
    <section className="ref-panel">
      <PanelTitle icon={Users} title={title} />
      {counts.length ? <div className="ref-workload-list">{counts.map(({ user, count }) => (
        <div className="ref-workload-row" key={user.id}>
          <Avatar name={user.full_name} small />
          <span className="ref-workload-name">{user.full_name}</span>
          <ProgressBar value={count / max * 100} />
          <strong>{count}</strong>
        </div>
      ))}</div> : <p className="ref-empty-copy">Team workload will appear when members are added.</p>}
      <p className="ref-panel-footnote">Open tasks assigned to each member</p>
    </section>
  );
}

export function DashboardOverview({ data, currentUser, onOpenBoard, onOpenTask, onCreateTask, onNavigate }) {
  const metrics = getWorkspaceMetrics(data);
  const myTaskIds = new Set(metrics.assignments.filter((assignment) => assignment.user_id === data.current_user_id).map((assignment) => assignment.task_id));
  const recentTasks = metrics.openTasks.filter((task) => myTaskIds.has(task.id)).sort((a, b) => (a.due_date || "9999").localeCompare(b.due_date || "9999")).slice(0, 4);
  const events = [...metrics.events].sort((a, b) => new Date(b.created_at) - new Date(a.created_at)).slice(0, 4);
  const firstName = (currentUser?.full_name || "there").split(/\s+/)[0];
  const today = new Intl.DateTimeFormat(undefined, { weekday: "long", month: "long", day: "numeric" }).format(new Date());

  return (
    <div className="ref-page">
      <PageHeading
        eyebrow={today.toUpperCase() + "  ·  " + (data.workspaces?.find((item) => item.id === data.current_workspace_id)?.name || "YOUR WORKSPACE").toUpperCase()}
        title="Project pulse"
        subtitle={"Good to see you, " + firstName + ". Here's the latest across your projects and team."}
        action={<button className="ref-primary-button" type="button" onClick={onCreateTask}><Plus size={16} /> New task</button>}
      />

      <section className="ref-highlight-grid" aria-label="Workspace highlights">
        <article className="ref-highlight-card ref-highlight-lime">
          <div><span className="ref-highlight-label"><Zap size={14} /> Delivery pace</span><ArrowUpRight size={16} /></div>
          <strong>{metrics.doneTasks.length}</strong>
          <p>Tasks completed across your workspace</p>
          <span className="ref-highlight-foot"><CheckCircle2 size={13} /> {metrics.completion}% overall completion</span>
        </article>
        <article className="ref-highlight-card ref-highlight-dark">
          <div><span className="ref-highlight-label"><CalendarDays size={14} /> Coming up</span><ArrowUpRight size={16} /></div>
          <strong>{metrics.dueSoon.length}</strong>
          <p>Open tasks due in the next seven days</p>
          <span className="ref-highlight-foot"><Clock3 size={13} /> Keep the next milestones visible</span>
        </article>
        <article className="ref-highlight-card ref-highlight-soft">
          <div><span className="ref-highlight-label"><CircleAlert size={14} /> Needs attention</span><ArrowUpRight size={16} /></div>
          <strong>{metrics.overdueTasks.length}</strong>
          <p>Overdue tasks that may need a follow-up</p>
          <span className="ref-highlight-foot">{metrics.overdueTasks.length ? "Review overdue work with the team" : "No overdue tasks right now"}</span>
        </article>
      </section>

      <section className="ref-stat-grid" aria-label="Project analytics">
        <StatCard icon={CheckCircle2} label="Tasks completed" value={metrics.doneTasks.length} note={metrics.completion + "% complete"} tone="green" />
        <StatCard icon={ListTodo} label="Open tasks" value={metrics.openTasks.length} note={metrics.inProgress + " in progress"} tone="ink" />
        <StatCard icon={FolderKanban} label="Active projects" value={metrics.boards.length} note="Workspace boards" tone="violet" />
        <StatCard icon={Users} label="Team members" value={metrics.users.length} note="Active workspace" tone="blue" />
      </section>

      <div className="ref-content-grid">
        <section className="ref-panel ref-projects-panel">
          <PanelTitle icon={FolderKanban} title="Your projects" action={<button type="button" className="ref-text-action" onClick={() => onNavigate("Projects")}>View all <ArrowUpRight size={14} /></button>} />
          {metrics.boards.length ? <div className="ref-project-list">{metrics.boards.map((board, index) => <BoardSummary key={board.id} board={board} index={index} metrics={metrics} onOpenBoard={onOpenBoard} />)}</div> : <div className="ref-empty"><FolderKanban size={22} /><strong>No projects yet</strong><span>Create your first project board to start planning work.</span><button className="ref-outline-button" type="button" onClick={() => onNavigate("Projects")}>Go to projects</button></div>}
        </section>

        <section className="ref-panel">
          <PanelTitle icon={ListTodo} title="My tasks" action={<button className="ref-text-action" type="button" onClick={() => onNavigate("My Tasks")}>Open list <ArrowUpRight size={14} /></button>} />
          {recentTasks.length ? <div className="ref-task-list">{recentTasks.map((task) => <TaskLine key={task.id} task={task} lists={metrics.lists} onOpenTask={onOpenTask} />)}</div> : <p className="ref-empty-copy">You’re all caught up. New open tasks will show here.</p>}
        </section>

        <WorkloadPanel metrics={metrics} />
        <section className="ref-panel">
          <PanelTitle icon={Activity} title="Recent activity" action={<button className="ref-text-action" type="button" onClick={() => onNavigate("Activity")}>Timeline <ArrowUpRight size={14} /></button>} />
          {events.length ? <div className="ref-activity-list">{events.map((event) => <ActivityLine key={event.id} event={event} users={metrics.users} compact />)}</div> : <p className="ref-empty-copy">Project and task updates will appear here as your team works.</p>}
        </section>
      </div>
    </div>
  );
}

export function WorkspaceTimeline({ data }) {
  const [filter, setFilter] = useState("all");
  const metrics = getWorkspaceMetrics(data);
  const events = useMemo(() => [...metrics.events].sort((a, b) => new Date(b.created_at) - new Date(a.created_at)), [metrics.events]);
  const filtered = events.filter((event) => {
    if (filter === "today") return new Date(event.created_at).toDateString() === new Date().toDateString();
    if (filter === "week") return Date.now() - new Date(event.created_at).getTime() <= 7 * 86400000;
    return true;
  });
  const grouped = filtered.reduce((groups, event) => {
    const key = new Date(event.created_at).toDateString();
    if (!groups[key]) groups[key] = [];
    groups[key].push(event);
    return groups;
  }, {});
  return (
    <div className="ref-page">
      <PageHeading eyebrow="WORKSPACE HISTORY" title="Activity timeline" subtitle="A shared view of the changes, decisions, and progress happening across your projects." />
      <section className="ref-panel ref-timeline-panel">
        <div className="ref-timeline-toolbar">
          <div><span className="ref-section-label">LATEST UPDATES</span><strong>{filtered.length} updates</strong></div>
          <div className="ref-filter-group" aria-label="Filter activity">
            {[["all", "All activity"], ["today", "Today"], ["week", "Last 7 days"]].map(([value, label]) => <button className={filter === value ? "selected" : ""} key={value} type="button" onClick={() => setFilter(value)}>{label}</button>)}
          </div>
        </div>
        {Object.keys(grouped).length ? <div className="ref-timeline">{Object.entries(grouped).map(([day, dayEvents]) => (
          <section className="ref-timeline-day" key={day}>
            <div className="ref-timeline-date"><span />{eventDate(dayEvents[0].created_at)}</div>
            <div className="ref-timeline-events">
              {dayEvents.map((event) => {
                const actor = metrics.users.find((user) => user.id === event.actor_id);
                return <article className="ref-timeline-event" key={event.id}>
                  <span className="ref-timeline-node" />
                  <Avatar name={actor?.full_name || "Team member"} />
                  <div className="ref-timeline-card">
                    <div><p><strong>{actor?.full_name || "A team member"}</strong> {event.description || "updated the workspace"}</p><time>{new Date(event.created_at).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}</time></div>
                    <small>Workspace activity</small>
                  </div>
                </article>;
              })}
            </div>
          </section>
        ))}</div> : <div className="ref-empty"><Activity size={24} /><strong>No activity in this view</strong><span>Try another time filter or check back after your team makes an update.</span></div>}
      </section>
    </div>
  );
}

export function WorkspaceReports({ data }) {
  const metrics = getWorkspaceMetrics(data);
  const [serverAnalytics, setServerAnalytics] = useState(null);
  useEffect(() => {
    let cancelled = false;
    analyticsApi.get().then((result) => { if (!cancelled) setServerAnalytics(result); }).catch(() => {});
    return () => { cancelled = true; };
  }, []);
  const projectRows = metrics.boards.map((board) => {
    const tasks = metrics.tasks.filter((task) => task.board_id === board.id);
    const done = tasks.filter((task) => metrics.doneIds.has(task.id)).length;
    const overdue = tasks.filter((task) => isOverdue(task, metrics.doneIds)).length;
    const progress = tasks.length ? Math.round(done / tasks.length * 100) : 0;
    const status = !tasks.length ? "No tasks" : overdue ? "At risk" : progress >= 70 ? "On track" : "In progress";
    return { board, tasks, done, overdue, progress, status };
  });
  const weekly = Array.from({ length: 7 }, (_, index) => {
    const date = new Date();
    date.setHours(0, 0, 0, 0);
    date.setDate(date.getDate() - (6 - index));
    const key = date.toDateString();
    const count = metrics.events.filter((event) => new Date(event.created_at).toDateString() === key).length;
    return { day: new Intl.DateTimeFormat(undefined, { weekday: "short" }).format(date), date: key, count };
  });
  const weeklyMax = Math.max(1, ...weekly.map((item) => item.count));
  const assignedOpen = metrics.assignments.filter((assignment) => metrics.openTasks.some((task) => task.id === assignment.task_id));
  const totalAssigned = assignedOpen.length;
  const workload = metrics.users.map((user) => ({
    user,
    count: assignedOpen.filter((assignment) => assignment.user_id === user.id).length
  })).sort((a, b) => b.count - a.count).slice(0, 5);
  const healthCount = projectRows.filter((row) => row.status === "On track").length;
  const overdueCount = metrics.overdueTasks.length;
  const averageCompletion = projectRows.length ? Math.round(projectRows.reduce((sum, row) => sum + row.progress, 0) / projectRows.length) : 0;

  return (
    <div className="ref-page">
      <PageHeading eyebrow="WORKSPACE PERFORMANCE" title="Project analytics" subtitle="A live summary calculated from your projects, tasks, assignments, and activity." action={<span className="ref-live-indicator"><i /> Live workspace data</span>} />
      <section className="ref-stat-grid ref-report-stats">
        <StatCard icon={CheckCircle2} label="Tasks completed" value={metrics.doneTasks.length} note={metrics.completion + "% of all tasks"} tone="green" />
        <StatCard icon={BarChart3} label="Average project progress" value={averageCompletion + "%"} note={healthCount + " projects on track"} tone="violet" />
        <StatCard icon={CircleAlert} label="Overdue tasks" value={overdueCount} note="Across all projects" tone="orange" />
        <StatCard icon={Users} label="Team members" value={metrics.users.length} note={totalAssigned + " active assignments"} tone="blue" />
      </section>

      {serverAnalytics && (
        <section className="ref-panel">
          <PanelTitle icon={Users} title="Server-verified team productivity" />
          <div className="feature-analytics-summary">
            <div><strong>{serverAnalytics.unassigned_tasks}</strong><span>Unassigned tasks</span></div>
            <div><strong>{serverAnalytics.overdue_tasks}</strong><span>Overdue open tasks</span></div>
            <div><strong>{serverAnalytics.completion_percent}%</strong><span>Completed across workspace</span></div>
          </div>
          {serverAnalytics.members?.length ? <div className="ref-health-table feature-analytics-members">
            <div className="ref-health-head"><span>Team member</span><span>Open tasks</span><span>Completed</span><span>Activity</span></div>
            {serverAnalytics.members.map((member) => <div className="ref-health-row" key={member.user_id}>
              <strong>{member.full_name}</strong><span>{member.open_tasks}</span><span>{member.completed_tasks}</span>
              <span className="ref-health-task-count">{member.open_tasks + member.completed_tasks} assigned</span>
            </div>)}
          </div> : <p className="ref-empty-copy">No assigned team work yet.</p>}
        </section>
      )}
      <section className="ref-panel">
        <PanelTitle icon={FolderKanban} title="Project health overview" />
        {projectRows.length ? <div className="ref-health-table">
          <div className="ref-health-head"><span>Project</span><span>Progress</span><span>Status</span><span>Tasks</span></div>
          {projectRows.map((row) => <div className="ref-health-row" key={row.board.id}>
            <strong>{row.board.name}</strong>
            <div className="ref-health-progress"><ProgressBar value={row.progress} /><span>{row.progress}%</span></div>
            <span className={"ref-health-status " + row.status.toLowerCase().replace(/\s+/g, "-")}>{row.status}</span>
            <span className="ref-health-task-count">{row.done}/{row.tasks.length} complete{row.overdue ? " · " + row.overdue + " overdue" : ""}</span>
          </div>)}
        </div> : <p className="ref-empty-copy">Create a project to start tracking workspace health.</p>}
      </section>

      <div className="ref-report-grid">
        <section className="ref-panel">
          <PanelTitle icon={Activity} title="Weekly workspace activity" />
          <p className="ref-chart-note">Recorded workspace updates over the last seven days</p>
          <div className="ref-week-chart">{weekly.map((item) => <div className="ref-week-column" key={item.date}><strong>{item.count}</strong><span className="ref-week-track"><i style={{ height: (item.count / weeklyMax * 100) + "%" }} /></span><small>{item.day}</small></div>)}</div>
        </section>
        <section className="ref-panel">
          <PanelTitle icon={Users} title="Workload distribution" />
          <p className="ref-chart-note">Open task assignments by team member</p>
          {workload.length ? <div className="ref-workload-list ref-report-workload">{workload.map(({ user, count }) => <div className="ref-workload-row" key={user.id}><Avatar name={user.full_name} small /><span className="ref-workload-name">{user.full_name}</span><ProgressBar value={totalAssigned ? count / totalAssigned * 100 : 0} /><strong>{count}</strong></div>)}</div> : <p className="ref-empty-copy">No open task assignments yet.</p>}
        </section>
      </div>
      <p className="ref-panel-footnote ref-report-footnote">Reports use live workspace records. No estimated revenue, time tracking, or forecast figures are shown.</p>
    </div>
  );
}
