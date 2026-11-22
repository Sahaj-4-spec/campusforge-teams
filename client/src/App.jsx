import { useEffect, useMemo, useRef, useState } from "react";
import {
  Activity,
  BarChart3,
  ArrowRight,
  ArrowUpRight,
  Bell,
  CalendarDays,
  Check,
  CheckCircle2,
  ChevronDown,
  Copy,
  MailPlus,
  XCircle,
  CircleHelp,
  Clock3,
  Command,
  Filter,
  FolderKanban,
  LayoutDashboard,
  ListTodo,
  FileText,
  MessageSquare,
  LogOut,
  MoreHorizontal,
  Plus,
  Search,
  Settings,
  SlidersHorizontal,
  Sparkles,
  Users,
  X,
  Zap
} from "lucide-react";
import BoardView from "./components/BoardView.jsx";
import TaskDialog from "./components/TaskDialog.jsx";
import AuthScreen from "./components/AuthScreen.jsx";
import ProjectDialog from "./components/ProjectDialog.jsx";
import { DashboardOverview, WorkspaceTimeline, WorkspaceReports } from "./components/WorkspaceViews.jsx";
import { CalendarPage, NotificationsPage, FilesPage, ChatAndMeetingsPage } from "./components/WorkspaceFeatures.jsx";
import {
  authApi, boardApi, clearStoredWorkspaceId, createWorkspaceSocket,
  getStoredWorkspaceId, setStoredWorkspaceId, workspaceApi, taskApi, listApi, invitationApi, notificationApi
} from "./lib/api.js";

const navigation = [
  { id: "Overview", label: "Dashboard", icon: LayoutDashboard },
  { id: "Projects", label: "Projects", icon: FolderKanban },
  { id: "My Tasks", label: "My tasks", icon: ListTodo },
  { id: "Calendar", label: "Calendar", icon: CalendarDays },
  { id: "Notifications", label: "Notifications", icon: Bell },
  { id: "Files", label: "Files & docs", icon: FileText },
  { id: "Chat", label: "Chat & notes", icon: MessageSquare },
  { id: "Team", label: "Team", icon: Users },
  { id: "Activity", label: "Timeline", icon: Activity },
  { id: "Reports", label: "Analytics", icon: BarChart3 }
];

function initials(name) {
  return (name || "U").split(" ").map((part) => part[0]).join("").slice(0, 2).toUpperCase();
}

function formatDate(value, options = { month: "short", day: "numeric" }) {
  if (!value) return "No date";
  return new Intl.DateTimeFormat(undefined, options).format(new Date(value + "T00:00:00"));
}

function formatRelative(value) {
  const difference = Math.max(0, Date.now() - new Date(value).getTime());
  const minutes = Math.floor(difference / 60000);
  if (minutes < 1) return "Just now";
  if (minutes < 60) return minutes + "m ago";
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return hours + "h ago";
  return Math.floor(hours / 24) + "d ago";
}

function getListName(task, lists) {
  return lists.find((list) => list.id === task.list_id)?.name || "Unsorted";
}

function Avatar({ name, small = false }) {
  return <span className={"avatar" + (small ? " avatar-small" : "")} aria-label={name}>{initials(name)}</span>;
}

function EmptyState({ title, description, action, onAction }) {
  return (
    <div className="empty-state">
      <span className="empty-state-icon"><Sparkles size={20} /></span>
      <h3>{title}</h3>
      <p>{description}</p>
      {action && <button className="secondary-button" type="button" onClick={onAction}>{action}</button>}
    </div>
  );
}

function MetricCard({ icon: Icon, tone, label, value, note }) {
  return (
    <article className="metric-card">
      <div className="metric-card-top">
        <span className={"metric-icon metric-" + tone}><Icon size={18} /></span>
        <span className="metric-note">{note}</span>
      </div>
      <strong className="metric-value">{value}</strong>
      <span className="metric-label">{label}</span>
    </article>
  );
}

function SectionHeading({ eyebrow, title, action, onAction }) {
  return (
    <div className="section-heading">
      <div>
        {eyebrow && <p className="eyebrow">{eyebrow}</p>}
        <h2>{title}</h2>
      </div>
      {action && <button className="text-button" type="button" onClick={onAction}>{action} <ArrowUpRight size={15} /></button>}
    </div>
  );
}

function ActivityList({ events, users }) {
  if (!events.length) {
    return <p className="muted-copy">No activity yet. Changes to this board will appear here.</p>;
  }

  return (
    <div className="activity-list">
      {events.map((event) => {
        const actor = users.find((user) => user.id === event.actor_id);
        return (
          <article className="activity-item" key={event.id}>
            <Avatar name={actor?.full_name || "Member"} small />
            <div className="activity-copy">
              <p><strong>{actor?.full_name || "A team member"}</strong> {event.description}</p>
              <time>{formatRelative(event.created_at)}</time>
            </div>
          </article>
        );
      })}
    </div>
  );
}

function OverviewPage({ data, onOpenBoard, onOpenTask, onCreateTask, onNavigate }) {
  const doneListIds = data.lists.filter((list) => list.name.toLowerCase() === "done").map((list) => list.id);
  const completed = data.tasks.filter((task) => doneListIds.includes(task.list_id)).length;
  const openTasks = data.tasks.length - completed;
  const today = new Date().toISOString().slice(0, 10);
  const nextWeek = new Date();
  nextWeek.setDate(nextWeek.getDate() + 7);
  const dueSoon = data.tasks.filter((task) =>
    task.due_date && task.due_date >= today &&
    task.due_date <= nextWeek.toISOString().slice(0, 10) &&
    !doneListIds.includes(task.list_id)
  ).length;
  const recentTasks = [...data.tasks]
    .filter((task) => !doneListIds.includes(task.list_id))
    .sort((a, b) => (a.due_date || "9999").localeCompare(b.due_date || "9999"))
    .slice(0, 4);
  const recentEvents = [...data.activity_events]
    .sort((a, b) => new Date(b.created_at) - new Date(a.created_at))
    .slice(0, 4);

  return (
    <div className="page-stack">
      <section className="welcome-panel">
        <div className="welcome-copy">
          <p className="eyebrow"><span className="eyebrow-sparkle"><Sparkles size={12} /></span> YOUR TEAM WORKSPACE</p>
          <h1>Good work starts with a clear plan.</h1>
          <p>Bring your team's projects, tasks, and conversations into one shared space.</p>
          <button className="primary-button" type="button" onClick={() => onCreateTask()}>
            <Plus size={17} /> Create a task
          </button>
        </div>
        <div className="welcome-visual" aria-hidden="true">
          <div className="welcome-orbit orbit-one" />
          <div className="welcome-orbit orbit-two" />
          <div className="welcome-glass-card">
            <div className="visual-card-top"><span /><span /><span /></div>
            <div className="visual-line line-long" />
            <div className="visual-line line-short" />
            <div className="visual-checks"><i><Check size={12} /></i><i><Check size={12} /></i><i /></div>
          </div>
          <span className="visual-spark visual-spark-one">✦</span>
          <span className="visual-spark visual-spark-two">✧</span>
        </div>
      </section>

      <section className="metrics-grid" aria-label="Workspace summary">
        <MetricCard icon={FolderKanban} tone="purple" label="Projects" value={data.boards.length} note="In this workspace" />
        <MetricCard icon={CheckCircle2} tone="green" label="Completed tasks" value={completed} note="Across all boards" />
        <MetricCard icon={Clock3} tone="orange" label="Open tasks" value={openTasks} note="Need attention" />
        <MetricCard icon={CalendarDays} tone="blue" label="Due this week" value={dueSoon} note="Next 7 days" />
      </section>

      <div className="overview-grid">
        <section className="surface-panel projects-panel">
          <SectionHeading eyebrow="YOUR WORK" title="Projects" action="View all" onAction={() => onNavigate("Projects")} />
          <div className="project-card-list">
            {data.boards.map((board, index) => {
              const boardTasks = data.tasks.filter((task) => task.board_id === board.id);
              const doneCount = boardTasks.filter((task) => doneListIds.includes(task.list_id)).length;
              const progress = boardTasks.length ? Math.round((doneCount / boardTasks.length) * 100) : 0;
              const tones = ["violet", "blue", "orange"];
              return (
                <button className="project-summary-card" type="button" key={board.id} onClick={() => onOpenBoard(board)}>
                  <span className={"project-summary-icon " + tones[index % tones.length]}><FolderKanban size={19} /></span>
                  <span className="project-summary-main">
                    <strong>{board.name}</strong>
                    <small>{board.description || "Team project board"}</small>
                    <span className="project-progress"><i style={{ width: progress + "%" }} /></span>
                  </span>
                  <span className="project-summary-meta"><strong>{progress}%</strong><small>{boardTasks.length} tasks</small></span>
                  <ArrowRight className="project-arrow" size={17} />
                </button>
              );
            })}
            {data.boards.length === 0 && <EmptyState title="No projects yet" description="Create a project board to start organizing your team's work." />}
          </div>
        </section>

        <section className="surface-panel due-panel">
          <SectionHeading eyebrow="KEEP MOVING" title="Upcoming tasks" action="View all" onAction={() => onNavigate("My Tasks")} />
          {recentTasks.length ? (
            <div className="upcoming-list">
              {recentTasks.map((task) => (
                <button className="upcoming-task" type="button" key={task.id} onClick={() => onOpenTask(task)}>
                  <span className={"upcoming-priority priority-" + task.priority} />
                  <span className="upcoming-task-copy">
                    <strong>{task.title}</strong>
                    <small>{getListName(task, data.lists)} · {task.due_date ? formatDate(task.due_date) : "No due date"}</small>
                  </span>
                  <ArrowUpRight size={15} />
                </button>
              ))}
            </div>
          ) : (
            <EmptyState title="All caught up" description="There are no open tasks right now." />
          )}
        </section>

        <section className="surface-panel activity-panel-home">
          <SectionHeading eyebrow="TEAM PULSE" title="Recent activity" action="See activity" onAction={() => onNavigate("Activity")} />
          <ActivityList events={recentEvents} users={data.users} />
        </section>
        <section className="quote-panel">
          <div className="quote-mark">“</div>
          <p>Great teams don't just get things done. They make progress visible.</p>
          <span>THE CAMPUSFORGE WAY</span>
          <div className="quote-decoration"><Zap size={48} /></div>
        </section>
      </div>
    </div>
  );
}

function MyTasksPage({ data, onOpenTask, search }) {
  const [filter, setFilter] = useState("all");
  const assignedTaskIds = new Set(
    data.task_assignees
      .filter((assignment) => assignment.user_id === data.current_user_id)
      .map((assignment) => assignment.task_id)
  );
  const tasks = data.tasks.filter((task) => {
    const assignedToCurrentUser = assignedTaskIds.has(task.id);
    const queryMatch = !search || (task.title + " " + task.description).toLowerCase().includes(search.toLowerCase());
    const filterMatch = filter === "all" || task.priority === filter || getListName(task, data.lists).toLowerCase() === filter;
    return assignedToCurrentUser && queryMatch && filterMatch;
  }).sort((a, b) => (a.due_date || "9999").localeCompare(b.due_date || "9999"));

  return (
    <div className="page-stack">
      <div className="page-title-row">
        <div><p className="eyebrow">YOUR PERSONAL QUEUE</p><h1>My tasks</h1><p className="page-subtitle">A focused view of the work happening across your boards.</p></div>
        <span className="count-chip">{tasks.length} tasks</span>
      </div>
      <section className="surface-panel task-table-panel">
        <div className="filter-row">
          <div className="filter-label"><Filter size={15} /> Filter by</div>
          <div className="filter-chips">
            {[
              ["all", "All tasks"],
              ["high", "High priority"],
              ["in progress", "In progress"],
              ["done", "Done"]
            ].map(([value, label]) => (
              <button key={value} type="button" className={"filter-chip" + (filter === value ? " selected" : "")} onClick={() => setFilter(value)}>{label}</button>
            ))}
          </div>
        </div>
        <div className="task-table">
          <div className="task-table-head"><span>Task</span><span>Project</span><span>Status</span><span>Due date</span><span>Priority</span></div>
          {tasks.map((task) => {
            const board = data.boards.find((item) => item.id === task.board_id);
            return (
              <button className="task-table-row" type="button" key={task.id} onClick={() => onOpenTask(task)}>
                <span className="task-table-title"><i className={"priority-dot priority-" + task.priority} /><strong>{task.title}</strong></span>
                <span className="table-muted">{board?.name || "Project"}</span>
                <span><span className="status-pill">{getListName(task, data.lists)}</span></span>
                <span className="table-muted">{task.due_date ? formatDate(task.due_date) : "Not set"}</span>
                <span className={"priority-text priority-" + task.priority}>{task.priority}</span>
              </button>
            );
          })}
          {tasks.length === 0 && <EmptyState title="No tasks found" description="Try a different filter or search term." />}
        </div>
      </section>
    </div>
  );
}

function TeamPage({ data, invitations, canManageInvites, onInvite, onRevoke }) {
  const [email, setEmail] = useState("");
  const [role, setRole] = useState("member");
  const [inviteLink, setInviteLink] = useState("");
  const [inviteError, setInviteError] = useState("");
  const [sending, setSending] = useState(false);
  const [copied, setCopied] = useState(false);

  async function submitInvite(event) {
    event.preventDefault();
    if (!email.trim()) return;
    setSending(true);
    setInviteError("");
    setInviteLink("");
    setCopied(false);
    try {
      const link = await onInvite({ email: email.trim(), role });
      setInviteLink(link);
      setEmail("");
    } catch (error) {
      setInviteError(error.message || "Could not create invitation");
    } finally {
      setSending(false);
    }
  }

  async function copyInviteLink() {
    try {
      await navigator.clipboard.writeText(inviteLink);
      setCopied(true);
    } catch {
      setInviteError("Clipboard access is unavailable. Copy the invitation link from the field below.");
    }
  }

  return (
    <div className="page-stack">
      <div className="page-title-row">
        <div><p className="eyebrow">YOUR PEOPLE</p><h1>Team members</h1><p className="page-subtitle">The people collaborating across this workspace.</p></div>
        <span className="count-chip">{data.users.length} members</span>
      </div>
      {canManageInvites && (
        <section className="surface-panel team-panel">
          <div className="team-panel-heading">
            <div><h2>Invite a teammate</h2><p>Send a secure, single-use link that expires in seven days.</p></div>
            <span className="role-badge"><MailPlus size={14} /> Invite</span>
          </div>
          <form className="task-form" onSubmit={submitInvite}>
            <div className="field-grid">
              <label className="field-group">
                <span>Teammate email</span>
                <input type="email" value={email} onChange={(event) => setEmail(event.target.value)} placeholder="name@college.edu" autoComplete="email" required />
              </label>
              <label className="field-group">
                <span>Workspace role</span>
                <select value={role} onChange={(event) => setRole(event.target.value)}>
                  <option value="member">Member</option>
                  <option value="admin">Admin</option>
                </select>
              </label>
            </div>
            <div className="dialog-actions">
              <span className="muted-copy">The invite must be accepted by an account using this email.</span>
              <button className="primary-button" type="submit" disabled={sending || !email.trim()}>{sending ? "Creating…" : "Create invite link"}</button>
            </div>
            {inviteError && <p className="auth-error" role="alert">{inviteError}</p>}
            {inviteLink && (
              <div className="settings-row">
                <div className="field-group" style={{ flex: 1, minWidth: 0 }}>
                  <span>Share this invitation link</span>
                  <input aria-label="Invitation link" readOnly value={inviteLink} onFocus={(event) => event.target.select()} />
                </div>
                <button className="secondary-button" type="button" onClick={copyInviteLink}><Copy size={15} /> {copied ? "Copied" : "Copy link"}</button>
              </div>
            )}
          </form>
        </section>
      )}
      {canManageInvites && (
        <section className="surface-panel team-panel">
          <div className="team-panel-heading"><div><h2>Pending invitations</h2><p>Links that have not yet been accepted.</p></div><span className="role-badge">{invitations.length} pending</span></div>
          {invitations.length ? (
            <div className="team-list">
              {invitations.map((invitation) => (
                <article className="team-member-row" key={invitation.id}>
                  <span className="avatar avatar-small"><MailPlus size={16} /></span>
                  <div className="team-member-copy"><strong>{invitation.email}</strong><span>Expires {new Date(invitation.expires_at).toLocaleDateString()}</span></div>
                  <span className="member-role">{invitation.role}</span>
                  <button className="secondary-button danger-outline" type="button" onClick={() => onRevoke(invitation.id)} aria-label={"Revoke invitation for " + invitation.email}><XCircle size={15} /> Revoke</button>
                </article>
              ))}
            </div>
          ) : <p className="muted-copy">No pending invitations.</p>}
        </section>
      )}
      <section className="surface-panel team-panel">
        <div className="team-panel-heading"><div><h2>Workspace members</h2><p>Member roles and profile details</p></div><span className="role-badge"><Users size={14} /> {data.users.length} active</span></div>
        <div className="team-list">
          {data.users.map((user) => {
            const membership = data.workspace_members.find((member) => member.user_id === user.id);
            return (
              <article className="team-member-row" key={user.id}>
                <Avatar name={user.full_name} />
                <div className="team-member-copy"><strong>{user.full_name}</strong><span>{user.email}</span></div>
                <span className={"member-role " + (membership?.role === "owner" ? "role-owner" : "")}>{membership?.role || "member"}</span>
                <span className="member-status"><i /> Active</span>
              </article>
            );
          })}
        </div>
        <p className="demo-note">Workspace members and roles are loaded from the shared database.</p>
      </section>
    </div>
  );
}

function ActivityPage({ data }) {
  const events = [...data.activity_events].sort((a, b) => new Date(b.created_at) - new Date(a.created_at));
  return (
    <div className="page-stack">
      <div className="page-title-row"><div><p className="eyebrow">WHAT'S HAPPENING</p><h1>Activity</h1><p className="page-subtitle">A timeline of the latest updates across your workspace.</p></div></div>
      <section className="surface-panel activity-page-panel">
        {events.length ? <ActivityList events={events} users={data.users} /> : <EmptyState title="No activity yet" description="New task and board updates will show up here." />}
      </section>
    </div>
  );
}

function ProjectsPage({ data, onOpenBoard, onCreateBoard, canManageProjects }) {
  return (
    <div className="page-stack">
      <div className="page-title-row"><div><p className="eyebrow">WORKSPACE LIBRARY</p><h1>Projects</h1><p className="page-subtitle">Choose a board to see its tasks and keep work moving.</p></div>{canManageProjects && <button className="primary-button" type="button" onClick={onCreateBoard}><Plus size={16} /> New project</button>}</div>
      <div className="projects-grid-page">
        {data.boards.map((board) => {
          const count = data.tasks.filter((task) => task.board_id === board.id).length;
          const members = data.board_members.filter((member) => member.board_id === board.id).length;
          return (
            <button className="project-large-card" type="button" key={board.id} onClick={() => onOpenBoard(board)}>
              <div className="project-large-top"><span className="project-summary-icon violet"><FolderKanban size={20} /></span><MoreHorizontal size={19} /></div>
              <h2>{board.name}</h2><p>{board.description}</p>
              <div className="project-large-meta"><span><ListTodo size={14} /> {count} tasks</span><span><Users size={14} /> {members} members</span></div>
              <span className="project-open-link">Open project <ArrowUpRight size={15} /></span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

function AppSidebar({
  activeView, activeBoardId, onNavigate, onOpenBoard, onCreateBoard, boards, currentUser,
  workspaces, currentWorkspaceId, onWorkspaceChange, workspaceName, activityCount, canManageProjects
}) {
  return (
    <aside className="sidebar">
      <a className="brand" href="#" onClick={(event) => { event.preventDefault(); onNavigate("Overview"); }} aria-label="CampusForge Teams home">
        <span className="brand-mark"><img src="/campusforge-mark.svg" alt="" /></span>
        <span className="brand-name">CampusForge<small>TEAMS WORKSPACE</small></span>
      </a>
      <div className="workspace-switcher">
        <span className="workspace-avatar">{(workspaceName || "C").slice(0, 1).toUpperCase()}</span>
        <span className="workspace-switcher-copy"><strong>{workspaceName}</strong><small>Switch workspace</small></span>
        <select
          aria-label="Switch workspace"
          value={currentWorkspaceId || ""}
          onChange={(event) => onWorkspaceChange(event.target.value)}
          disabled={(workspaces || []).length < 2}
        >
          {(workspaces || []).map((workspace) => (
            <option key={workspace.id} value={workspace.id}>{workspace.name}</option>
          ))}
        </select>
      </div>
      <div className="sidebar-section-label">WORKSPACE</div>
      <nav className="sidebar-nav" aria-label="Main navigation">
        {navigation.map(({ id, label, icon: Icon }) => (
          <button className={"nav-item" + (activeView === id ? " active" : "")} key={id} onClick={() => onNavigate(id)} type="button">
            <Icon size={18} /><span>{label}</span>{id === "Activity" && activityCount > 0 && <span className="nav-count">{Math.min(9, activityCount)}</span>}
          </button>
        ))}
      </nav>
      <div className="sidebar-section-row"><span className="sidebar-section-label">YOUR PROJECTS</span>{canManageProjects && <button className="sidebar-plus" type="button" onClick={onCreateBoard} aria-label="Create project">+</button>}</div>
      <div className="sidebar-projects">
        {boards.map((board, index) => (
          <button className={"sidebar-project" + (activeView === "Board" && activeBoardId === board.id ? " selected" : "")} key={board.id} onClick={() => onOpenBoard(board)} type="button">
            <span className={"project-dot dot-color-" + index % 3} />{board.name}
          </button>
        ))}
      </div>
      <div className="sidebar-bottom">
        <button className={"nav-item" + (activeView === "Help" ? " active" : "")} type="button" onClick={() => onNavigate("Help")}><CircleHelp size={18} /><span>Help & support</span></button>
        <button className={"nav-item" + (activeView === "Settings" ? " active" : "")} type="button" onClick={() => onNavigate("Settings")}><Settings size={18} /><span>Settings</span></button>
        <div className="profile-card">
          <Avatar name={currentUser?.full_name || "You"} />
          <div className="profile-copy"><strong>{currentUser?.full_name || "Workspace member"}</strong><small>Signed-in account</small></div>
          <MoreHorizontal size={18} />
        </div>
      </div>
    </aside>
  );
}

export default function App() {
  const [isAuthenticated, setIsAuthenticated] = useState(true);
  const [data, setData] = useState(null);
  const [invitations, setInvitations] = useState([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState("");
  const [activeView, setActiveView] = useState("Overview");
  const [activeBoardId, setActiveBoardId] = useState("");
  const [search, setSearch] = useState("");
  const [dialogOpen, setDialogOpen] = useState(false);
  const [boardDialogOpen, setBoardDialogOpen] = useState(false);
  const [editingTaskId, setEditingTaskId] = useState(null);
  const [defaultListId, setDefaultListId] = useState("");
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const [notice, setNotice] = useState("");
  const [logoutError, setLogoutError] = useState("");
  const [isLoggingOut, setIsLoggingOut] = useState(false);
  const [unreadNotifications, setUnreadNotifications] = useState(0);
  const [realtimeVersion, setRealtimeVersion] = useState(0);
  const workspaceGeneration = useRef(0);

  useEffect(() => {
    if (!isAuthenticated) {
      setData(null);
      setLoading(false);
      return undefined;
    }

    let cancelled = false;
    const requestGeneration = workspaceGeneration.current;
    setLoading(true);
    setLoadError("");
    const pendingInvite = new URLSearchParams(window.location.search).get("invite");
    const workspaceRequest = pendingInvite
      ? invitationApi.accept(pendingInvite).then(() => {
          const inviteUrl = new URL(window.location.href);
          inviteUrl.searchParams.delete("invite");
          window.history.replaceState({}, "", inviteUrl.toString());
          setNotice("You joined the invited workspace");
          return workspaceApi.get(null);
        })
      : workspaceApi.get(getStoredWorkspaceId());

    workspaceRequest
      .then((workspace) => {
        if (cancelled || requestGeneration !== workspaceGeneration.current) return;
        setStoredWorkspaceId(workspace.current_workspace_id);
        setData(workspace);
        setActiveBoardId((current) =>
          workspace.boards.some((board) => board.id === current)
            ? current
            : (workspace.boards[0]?.id || "")
        );
      })
      .catch((error) => {
        if (cancelled) return;
        if (error.status === 401) {
          clearStoredWorkspaceId();
          setIsAuthenticated(false);
          setData(null);
        } else {
          if (error.status === 403 && getStoredWorkspaceId()) {
            clearStoredWorkspaceId();
            return workspaceApi.get(null).then((workspace) => {
              if (cancelled || requestGeneration !== workspaceGeneration.current) return;
              setStoredWorkspaceId(workspace.current_workspace_id);
              setData(workspace);
              setActiveBoardId(workspace.boards[0]?.id || "");
            }).catch((fallbackError) => {
              if (!cancelled && requestGeneration === workspaceGeneration.current) setLoadError(fallbackError.message || "Unable to load an accessible workspace.");
            });
          } else setLoadError(error.message || "Unable to load your workspace.");
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => { cancelled = true; };
  }, [isAuthenticated]);

  useEffect(() => {
    if (!isAuthenticated || !data?.current_workspace_id) return undefined;

    let stopped = false;
    let retryTimer = null;
    let socket = null;
    let attempts = 0;

    function connect() {
      if (stopped) return;
      try {
        socket = createWorkspaceSocket();
      } catch (error) {
        setNotice("Live updates are temporarily unavailable");
        retryTimer = window.setTimeout(connect, 5000);
        return;
      }

      socket.addEventListener("open", () => {
        attempts = 0;
      });

      socket.addEventListener("message", (message) => {
        try {
          const payload = JSON.parse(message.data);
          if (payload.type === "error" && /session.*(invalid|expired)|sign in again/i.test(payload.error || "")) {
            clearStoredWorkspaceId();
            setIsAuthenticated(false);
            setData(null);
            return;
          }
          if (payload.type === "workspace_event" || payload.type === "refresh_required") {
            setRealtimeVersion((version) => version + 1);
            notificationApi.list()
              .then((result) => { if (!stopped) setUnreadNotifications(result.unread_count || 0); })
              .catch(() => {});
            const requestGeneration = workspaceGeneration.current;
            const requestedWorkspaceId = data?.current_workspace_id;
            workspaceApi.get(requestedWorkspaceId)
              .then((snapshot) => {
                if (!stopped && requestGeneration === workspaceGeneration.current && snapshot.current_workspace_id === requestedWorkspaceId) {
                  setStoredWorkspaceId(snapshot.current_workspace_id);
                  setData(snapshot);
                  setActiveBoardId((current) => snapshot.boards.some((board) => board.id === current) ? current : (snapshot.boards[0]?.id || ""));
                }
              })
              .catch((error) => {
                if (error.status === 401 && !stopped) {
                  clearStoredWorkspaceId();
                  setIsAuthenticated(false);
                  setData(null);
                }
              });
          }
        } catch {
          // Ignore malformed messages and keep the session alive.
        }
      });

      socket.addEventListener("close", () => {
        if (stopped) return;
        attempts += 1;
        const delay = Math.min(30000, 1000 * (2 ** Math.min(attempts, 5)));
        retryTimer = window.setTimeout(connect, delay);
      });

      socket.addEventListener("error", () => {
        socket?.close();
      });
    }

    connect();
    return () => {
      stopped = true;
      if (retryTimer) window.clearTimeout(retryTimer);
      socket?.close();
    };
  }, [isAuthenticated, data?.current_workspace_id]);

  useEffect(() => {
    if (!isAuthenticated || !data) {
      setInvitations([]);
      return undefined;
    }
    const membership = data.workspace_members.find((member) => member.user_id === data.current_user_id);
    if (membership?.role !== "owner" && membership?.role !== "admin") {
      setInvitations([]);
      return undefined;
    }
    let cancelled = false;
    invitationApi.list()
      .then((items) => { if (!cancelled) setInvitations(items); })
      .catch((error) => {
        if (!cancelled) setNotice(error.message || "Could not load invitations");
      });
    return () => { cancelled = true; };
  }, [isAuthenticated, data]);

  useEffect(() => {
    if (!isAuthenticated) {
      setUnreadNotifications(0);
      return undefined;
    }
    let cancelled = false;
    notificationApi.list()
      .then((result) => { if (!cancelled) setUnreadNotifications(result.unread_count || 0); })
      .catch(() => { if (!cancelled) setUnreadNotifications(0); });
    return () => { cancelled = true; };
  }, [isAuthenticated, activeView]);

  useEffect(() => {
    if (!notice) return undefined;
    const timeout = window.setTimeout(() => setNotice(""), 3200);
    return () => window.clearTimeout(timeout);
  }, [notice]);

  const currentUser = data?.users.find((user) => user.id === data.current_user_id);
  const currentRole = data?.workspace_members.find((member) => member.user_id === data.current_user_id)?.role;
  const canManageWorkspace = currentRole === "owner" || currentRole === "admin";
  const workspaceName = data?.workspaces?.find((workspace) => workspace.id === data.current_workspace_id)?.name || "CampusForge";
  const activeBoard = data?.boards.find((board) => board.id === activeBoardId) || data?.boards[0];
  const activeLists = (data?.lists || [])
    .filter((list) => list.board_id === activeBoard?.id)
    .sort((a, b) => a.position - b.position);
  const boardTasks = (data?.tasks || []).filter((task) => task.board_id === activeBoard?.id);
  const editingTask = data?.tasks.find((task) => task.id === editingTaskId) || null;
  const editingAssignees = editingTask
    ? data.task_assignees.filter((item) => item.task_id === editingTask.id).map((item) => item.user_id)
    : [];

  const filteredBoardTasks = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return boardTasks;
    return boardTasks.filter((task) => (task.title + " " + task.description).toLowerCase().includes(query));
  }, [boardTasks, search]);

  async function refreshWorkspace() {
    const requestGeneration = workspaceGeneration.current;
    const requestedWorkspaceId = data?.current_workspace_id || getStoredWorkspaceId();
    const workspace = await workspaceApi.get(requestedWorkspaceId);
    if (requestGeneration !== workspaceGeneration.current || workspace.current_workspace_id !== requestedWorkspaceId) return workspace;
    setData(workspace);
    if (!workspace.boards.some((board) => board.id === activeBoardId)) {
      setActiveBoardId(workspace.boards[0]?.id || "");
    }
    return workspace;
  }

  async function switchWorkspace(workspaceId) {
    if (!workspaceId || workspaceId === data?.current_workspace_id) return;
    const requestGeneration = ++workspaceGeneration.current;
    setLoading(true);
    try {
      const workspace = await workspaceApi.get(workspaceId);
      if (requestGeneration !== workspaceGeneration.current) return;
      setStoredWorkspaceId(workspace.current_workspace_id);
      setData(workspace);
      setActiveBoardId(workspace.boards[0]?.id || "");
      setActiveView("Overview");
      setMobileMenuOpen(false);
      setRealtimeVersion((version) => version + 1);
      setNotice("Workspace switched");
    } catch (error) {
      setNotice(error.message || "Could not switch workspace");
    } finally {
      if (requestGeneration === workspaceGeneration.current) setLoading(false);
    }
  }

  function handleAuthenticated() {
    setLoading(true);
    setIsAuthenticated(true);
    setActiveView("Overview");
    setLoadError("");
  }

  async function logout() {
    if (isLoggingOut) return;

    setIsLoggingOut(true);
    setLogoutError("");
    try {
      // The server must confirm that it cleared the HttpOnly cookie before
      // the UI reports a signed-out state.
      await authApi.logout();
      clearStoredWorkspaceId();
      setIsAuthenticated(false);
      setData(null);
      setActiveView("Overview");
      setActiveBoardId("");
      setNotice("");
    } catch (error) {
      // Keep the current workspace visible: the HttpOnly cookie may still be valid.
      setLogoutError(
        "Sign-out failed. Your session is still active. Please retry. " +
        (error.message || "Check your connection and try again.")
      );
    } finally {
      setIsLoggingOut(false);
    }
  }

  function openBoard(board) {
    if (!board) return;
    setActiveBoardId(board.id);
    setActiveView("Board");
    setSearch("");
    setMobileMenuOpen(false);
  }

  function navigate(view) {
    setActiveView(view);
    setSearch("");
    setMobileMenuOpen(false);
  }

  function openNewBoard() {
    setBoardDialogOpen(true);
    setMobileMenuOpen(false);
  }

  async function saveBoard(values) {
    try {
      await boardApi.create(values, data.current_workspace_id);
      const workspace = await refreshWorkspace();
      const newestBoard = [...workspace.boards].sort((a, b) => new Date(b.created_at) - new Date(a.created_at))[0];
      setBoardDialogOpen(false);
      if (newestBoard) openBoard(newestBoard);
      setNotice("Project created");
    } catch (error) {
      setNotice(error.message || "Could not create project");
      throw error;
    }
  }

  async function createList() {
    if (!activeBoard) return;
    const name = window.prompt("Name this column");
    if (name === null || !name.trim()) return;
    try {
      await listApi.create(activeBoard.id, name.trim());
      await refreshWorkspace();
      setNotice("Column created");
    } catch (error) {
      setNotice(error.message || "Could not create column");
    }
  }

  async function renameList(list) {
    const name = window.prompt("Rename column", list.name);
    if (name === null || !name.trim() || name.trim() === list.name) return;
    try {
      await listApi.update(list.id, name.trim());
      await refreshWorkspace();
      setNotice("Column renamed");
    } catch (error) {
      setNotice(error.message || "Could not rename column");
    }
  }

  async function deleteList(list) {
    if (!window.confirm("Delete the empty column \"" + list.name + "\"? Columns must be empty before they can be deleted.")) return;
    try {
      await listApi.remove(list.id);
      await refreshWorkspace();
      setNotice("Column deleted");
    } catch (error) {
      setNotice(error.message || "Could not delete column");
    }
  }

  async function createInvitation(values) {
    const invitation = await invitationApi.create(values);
    const items = await invitationApi.list();
    setInvitations(items);
    setNotice("Invitation link created");
    return window.location.origin + "/?invite=" + encodeURIComponent(invitation.token);
  }

  async function revokeInvitation(invitationId) {
    try {
      await invitationApi.revoke(invitationId);
      setInvitations((current) => current.filter((item) => item.id !== invitationId));
      setNotice("Invitation revoked");
    } catch (error) {
      setNotice(error.message || "Could not revoke invitation");
    }
  }

  function openNewTask(listId = activeLists[0]?.id) {
    setEditingTaskId(null);
    setDefaultListId(listId || activeLists[0]?.id || "");
    setDialogOpen(true);
  }

  function openTask(task) {
    setEditingTaskId(task.id);
    setDefaultListId(task.list_id);
    setDialogOpen(true);
  }

  function closeDialog() {
    setDialogOpen(false);
    setEditingTaskId(null);
  }

  async function saveTask(values) {
    try {
      if (editingTask) {
        await taskApi.update(editingTask.id, values);
        setNotice("Task updated");
      } else {
        await taskApi.create(values);
        setNotice("Task created");
      }
      await refreshWorkspace();
      closeDialog();
    } catch (error) {
      setNotice(error.message || "Could not save task");
    }
  }

  async function deleteTask(taskId) {
    try {
      await taskApi.remove(taskId);
      await refreshWorkspace();
      closeDialog();
      setNotice("Task deleted");
    } catch (error) {
      setNotice(error.message || "Could not delete task");
    }
  }

  async function addComment(taskId, body) {
    try {
      await taskApi.addComment(taskId, body);
      await refreshWorkspace();
      setNotice("Comment added");
    } catch (error) {
      setNotice(error.message || "Could not add comment");
    }
  }

  async function moveTask(taskId, destinationListId, overId) {
    const movingTask = data?.tasks.find((task) => task.id === taskId);
    if (!movingTask || !destinationListId) return;

    const destinationTasks = data.tasks
      .filter((task) => task.list_id === destinationListId && task.id !== taskId)
      .sort((a, b) => a.position - b.position);
    const overIndex = destinationTasks.findIndex((task) => task.id === overId);
    const insertAt = overIndex >= 0 ? overIndex : destinationTasks.length;
    destinationTasks.splice(insertAt, 0, movingTask);

    try {
      await taskApi.move(taskId, destinationListId, destinationTasks.map((task) => task.id));
      await refreshWorkspace();
      setNotice("Task moved");
    } catch (error) {
      setNotice(error.message || "Could not move task");
    }
  }

  async function retryWorkspaceLoad() {
    setLoading(true);
    setLoadError("");
    try {
      await refreshWorkspace();
    } catch (error) {
      setLoadError(error.message || "Unable to load your workspace.");
    } finally {
      setLoading(false);
    }
  }

  function renderPage() {
    switch (activeView) {
      case "Board":
        return activeBoard ? (
          <BoardView
            board={activeBoard}
            lists={activeLists}
            tasks={filteredBoardTasks}
            users={data.users}
            taskAssignees={data.task_assignees}
            onOpenTask={openTask}
            onAddTask={openNewTask}
            onMoveTask={moveTask}
            onCreateList={createList}
            onRenameList={renameList}
            onDeleteList={deleteList}
            canManageLists={canManageWorkspace}
          />
        ) : <EmptyState title="No project selected" description="Choose a project from the sidebar." />;
      case "My Tasks":
        return <MyTasksPage data={data} onOpenTask={openTask} search={search} />;
      case "Team": {
        const memberRole = data.workspace_members.find((member) => member.user_id === data.current_user_id)?.role;
        const canManageInvites = memberRole === "owner" || memberRole === "admin";
        return <TeamPage data={data} invitations={invitations} canManageInvites={canManageInvites} onInvite={createInvitation} onRevoke={revokeInvitation} />;
      }
      case "Calendar":
        return <CalendarPage data={data} onOpenTask={openTask} realtimeVersion={realtimeVersion} />;
      case "Notifications":
        return <NotificationsPage realtimeVersion={realtimeVersion} onChanged={() => notificationApi.list().then((result) => setUnreadNotifications(result.unread_count || 0)).catch(() => {})} />;
      case "Files":
        return <FilesPage data={data} realtimeVersion={realtimeVersion} />;
      case "Chat":
        return <ChatAndMeetingsPage data={data} realtimeVersion={realtimeVersion} />;
      case "Activity":
        return <WorkspaceTimeline data={data} />;
      case "Reports":
        return <WorkspaceReports data={data} realtimeVersion={realtimeVersion} />;
      case "Projects":
        return <ProjectsPage data={data} onOpenBoard={openBoard} onCreateBoard={openNewBoard} canManageProjects={canManageWorkspace} />;
      case "Settings":
        return (
          <div className="page-stack">
            <div className="page-title-row"><div><p className="eyebrow">ACCOUNT & PREFERENCES</p><h1>Settings</h1><p className="page-subtitle">Manage your signed-in account and workspace session.</p></div></div>
            <section className="surface-panel settings-panel">
              <div className="settings-row"><div><strong>Signed-in account</strong><p>{currentUser?.full_name || "Workspace member"} · {currentUser?.email || ""}</p></div><span className="connection-badge"><i /> Authenticated</span></div>
              <div className="settings-row"><div><strong>Workspace data</strong><p>Boards, tasks, assignments, comments, and activity are loaded from the CampusForge API and saved in PostgreSQL.</p></div><span className="connection-badge"><i /> Connected</span></div>
              <div className="settings-row">
                <div>
                  <strong>Session</strong>
                  <p>Sign out on this device. Your saved workspace data remains on the server.</p>
                  {logoutError && <p className="auth-error logout-error" role="alert">{logoutError}</p>}
                </div>
                <button
                  className="secondary-button danger-outline"
                  type="button"
                  onClick={logout}
                  disabled={isLoggingOut}
                >
                  <LogOut size={15} /> {isLoggingOut ? "Signing out..." : logoutError ? "Retry sign out" : "Sign out"}
                </button>
              </div>
            </section>
          </div>
        );
      case "Help":
        return (
          <div className="page-stack">
            <div className="page-title-row"><div><p className="eyebrow">WE'RE HERE TO HELP</p><h1>Help & support</h1><p className="page-subtitle">A quick guide to your shared workspace.</p></div></div>
            <section className="surface-panel help-panel"><h2>Working with your workspace</h2><p>Create tasks, move them between board columns, assign workspace members, add comments, and search your task list. Changes are saved to the CampusForge API and PostgreSQL.</p><p>Your session is protected by a server-set HttpOnly cookie that JavaScript cannot read. Workspace records are returned only to authenticated workspace members.</p></section>
          </div>
        );
      case "Overview":
      default:
        return (
          <DashboardOverview
            data={data}
            currentUser={currentUser}
            onOpenBoard={openBoard}
            onOpenTask={openTask}
            onCreateTask={() => openNewTask()}
            onNavigate={navigate}
          />
        );
    }
  }

  if (loading) {
    return <main className="auth-screen"><section className="auth-card"><p className="eyebrow">CAMPUSFORGE TEAMS</p><h1>Loading your workspace…</h1><p>Connecting to your account and retrieving your projects.</p></section></main>;
  }

  if (!isAuthenticated) return <AuthScreen onAuthenticated={handleAuthenticated} />;

  if (loadError || !data) {
    return (
      <main className="auth-screen">
        <section className="auth-card">
          <p className="eyebrow">WORKSPACE CONNECTION</p>
          <h1>We couldn't load your workspace.</h1>
          <p>{loadError || "The API did not return workspace data."}</p>
          {logoutError && <p className="auth-error logout-error" role="alert">{logoutError}</p>}
          <div className="auth-form">
            <button className="primary-button auth-submit" type="button" onClick={retryWorkspaceLoad}>Try again</button>
            <button className="secondary-button" type="button" onClick={logout} disabled={isLoggingOut}>
              {isLoggingOut ? "Signing out..." : logoutError ? "Retry sign out" : "Sign out"}
            </button>
          </div>
        </section>
      </main>
    );
  }

  const pageTitle = activeView === "Board" ? activeBoard?.name || "Project board" : activeView === "Overview" ? "Dashboard" : activeView === "Activity" ? "Timeline" : activeView;
  const searchPlaceholder = activeView === "Board" ? "Search cards..." : "Search tasks...";
  const showSearch = ["Board", "My Tasks"].includes(activeView);

  return (
    <div className="app-shell">
      <AppSidebar
        activeView={activeView}
        activeBoardId={activeBoardId}
        workspaceName={workspaceName}
        workspaces={data.workspaces}
        currentWorkspaceId={data.current_workspace_id}
        onWorkspaceChange={switchWorkspace}
        activityCount={data.activity_events.length}
        onNavigate={navigate}
        onOpenBoard={openBoard}
        onCreateBoard={openNewBoard}
        boards={data.boards}
        currentUser={currentUser}
        canManageProjects={canManageWorkspace}
      />
      <div className="main-content">
        <header className="topbar">
          <div className="topbar-left">
            <button className="mobile-menu-button" type="button" aria-label="Open navigation" onClick={() => setMobileMenuOpen((value) => !value)}>
              <Command size={18} />
            </button>
            <div className="breadcrumbs"><span>Workspace</span><span className="breadcrumb-divider">/</span><strong>{pageTitle}</strong></div>
          </div>
          <div className="topbar-actions">
            {showSearch && (
              <label className="search-box">
                <Search size={16} />
                <input aria-label="Search tasks" value={search} onChange={(event) => setSearch(event.target.value)} placeholder={searchPlaceholder} />
                <kbd>⌘ K</kbd>
              </label>
            )}
            <span className="preview-status"><i /> Database-backed</span>
            <button className="icon-button notification-button" type="button" aria-label={"Notifications" + (unreadNotifications ? ", " + unreadNotifications + " unread" : "")} onClick={() => navigate("Notifications")}><Bell size={18} />{unreadNotifications > 0 && <span className="notification-count-badge">{Math.min(unreadNotifications, 99)}</span>}</button>
            <Avatar name={currentUser?.full_name || "You"} />
          </div>
        </header>

        {mobileMenuOpen && (
          <div className="mobile-nav-overlay">
            <div className="mobile-nav-card">
              <div className="mobile-nav-heading"><strong>CampusForge</strong><button type="button" onClick={() => setMobileMenuOpen(false)} aria-label="Close navigation"><X size={18} /></button></div>
              <label className="mobile-workspace-select">Current workspace<select aria-label="Switch workspace" value={data.current_workspace_id || ""} onChange={(event) => switchWorkspace(event.target.value)} disabled={(data.workspaces || []).length < 2}>{(data.workspaces || []).map((workspace) => <option key={workspace.id} value={workspace.id}>{workspace.name}</option>)}</select></label>
              {navigation.map(({ id, label, icon: Icon }) => <button key={id} type="button" className="nav-item" onClick={() => navigate(id)}><Icon size={18} />{label}</button>)}
              <button type="button" className="nav-item" onClick={() => navigate("Projects")}><FolderKanban size={18} />Projects</button>
            </div>
          </div>
        )}

        <main className="page-content">{renderPage()}</main>
        <footer className="page-footer"><span>CampusForge Teams</span><span>Plan better. Collaborate smarter. Build together.</span></footer>
      </div>

      <ProjectDialog open={boardDialogOpen} onClose={() => setBoardDialogOpen(false)} onSave={saveBoard} />
      <TaskDialog
        open={dialogOpen}
        task={editingTask}
        defaultListId={defaultListId}
        lists={data.lists.filter((list) => list.board_id === (editingTask?.board_id || activeBoard?.id))}
        users={data.users}
        assigneeIds={editingAssignees}
        comments={data.comments}
        currentUser={currentUser}
        onClose={closeDialog}
        onSave={saveTask}
        onDelete={deleteTask}
        onAddComment={addComment}
        allTasks={data.tasks}
        onAdvancedChanged={refreshWorkspace}
        refreshKey={realtimeVersion}
      />
      {notice && <div className="toast-message" role="status"><CheckCircle2 size={16} />{notice}</div>}
    </div>
  );
}
