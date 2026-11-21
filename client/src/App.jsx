import { useEffect, useMemo, useState } from "react";
import {
  Activity,
  ArrowRight,
  ArrowUpRight,
  Bell,
  CalendarDays,
  Check,
  CheckCircle2,
  ChevronDown,
  CircleHelp,
  Clock3,
  Command,
  Filter,
  FolderKanban,
  LayoutDashboard,
  ListTodo,
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
import { createId, createDemoData, loadDemoData, STORAGE_KEY } from "./data/demoData.js";

const navigation = [
  { id: "Overview", label: "Overview", icon: LayoutDashboard },
  { id: "My Tasks", label: "My tasks", icon: ListTodo },
  { id: "Team", label: "Team", icon: Users },
  { id: "Activity", label: "Activity", icon: Activity }
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
  const tasks = data.tasks.filter((task) => {
    const queryMatch = !search || (task.title + " " + task.description).toLowerCase().includes(search.toLowerCase());
    const filterMatch = filter === "all" || task.priority === filter || getListName(task, data.lists).toLowerCase() === filter;
    return queryMatch && filterMatch;
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

function TeamPage({ data }) {
  return (
    <div className="page-stack">
      <div className="page-title-row">
        <div><p className="eyebrow">YOUR PEOPLE</p><h1>Team members</h1><p className="page-subtitle">The people collaborating across this workspace.</p></div>
        <span className="count-chip">{data.users.length} members</span>
      </div>
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
        <p className="demo-note">Invitations and role changes will be connected to the backend after authentication and workspace APIs are complete.</p>
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

function ProjectsPage({ data, onOpenBoard }) {
  return (
    <div className="page-stack">
      <div className="page-title-row"><div><p className="eyebrow">WORKSPACE LIBRARY</p><h1>Projects</h1><p className="page-subtitle">Choose a board to see its tasks and keep work moving.</p></div></div>
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

function AppSidebar({ activeView, onNavigate, onOpenBoard, boards, currentUser }) {
  return (
    <aside className="sidebar">
      <a className="brand" href="#" onClick={(event) => { event.preventDefault(); onNavigate("Overview"); }} aria-label="CampusForge Teams home">
        <span className="brand-mark"><Zap size={21} fill="currentColor" /></span>
        <span className="brand-name">CampusForge<small>TEAMS WORKSPACE</small></span>
      </a>
      <div className="workspace-switcher">
        <span className="workspace-avatar">C</span>
        <span><strong>CampusForge</strong><small>Free workspace</small></span>
        <ChevronDown size={15} />
      </div>
      <div className="sidebar-section-label">WORKSPACE</div>
      <nav className="sidebar-nav" aria-label="Main navigation">
        {navigation.map(({ id, label, icon: Icon }) => (
          <button className={"nav-item" + (activeView === id ? " active" : "")} key={id} onClick={() => onNavigate(id)} type="button">
            <Icon size={18} /><span>{label}</span>{id === "Activity" && <span className="nav-count">{Math.min(9, 3)}</span>}
          </button>
        ))}
      </nav>
      <div className="sidebar-section-row"><span className="sidebar-section-label">YOUR PROJECTS</span><span className="sidebar-plus">+</span></div>
      <div className="sidebar-projects">
        {boards.map((board, index) => (
          <button className={"sidebar-project" + (activeView === "Board" && index === 0 ? " selected" : "")} key={board.id} onClick={() => onOpenBoard(board)} type="button">
            <span className={"project-dot dot-color-" + index % 3} />{board.name}
          </button>
        ))}
      </div>
      <div className="sidebar-bottom">
        <button className={"nav-item" + (activeView === "Help" ? " active" : "")} type="button" onClick={() => onNavigate("Help")}><CircleHelp size={18} /><span>Help & support</span></button>
        <button className={"nav-item" + (activeView === "Settings" ? " active" : "")} type="button" onClick={() => onNavigate("Settings")}><Settings size={18} /><span>Settings</span></button>
        <div className="profile-card">
          <Avatar name={currentUser?.full_name || "You"} />
          <div className="profile-copy"><strong>{currentUser?.full_name || "Workspace member"}</strong><small>Demo account</small></div>
          <MoreHorizontal size={18} />
        </div>
      </div>
    </aside>
  );
}

export default function App() {
  const [data, setData] = useState(() => loadDemoData());
  const [activeView, setActiveView] = useState("Overview");
  const [activeBoardId, setActiveBoardId] = useState(() => loadDemoData().boards[0]?.id || "");
  const [search, setSearch] = useState("");
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingTaskId, setEditingTaskId] = useState(null);
  const [defaultListId, setDefaultListId] = useState("");
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
  const [notice, setNotice] = useState("");

  const currentUser = data.users.find((user) => user.id === data.current_user_id);
  const activeBoard = data.boards.find((board) => board.id === activeBoardId) || data.boards[0];
  const activeLists = data.lists.filter((list) => list.board_id === activeBoard?.id).sort((a, b) => a.position - b.position);
  const boardTasks = data.tasks.filter((task) => task.board_id === activeBoard?.id);
  const editingTask = data.tasks.find((task) => task.id === editingTaskId) || null;
  const editingAssignees = editingTask
    ? data.task_assignees.filter((item) => item.task_id === editingTask.id).map((item) => item.user_id)
    : [];

  const filteredBoardTasks = useMemo(() => {
    const query = search.trim().toLowerCase();
    if (!query) return boardTasks;
    return boardTasks.filter((task) => (task.title + " " + task.description).toLowerCase().includes(query));
  }, [boardTasks, search]);

  useEffect(() => {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
    } catch (error) {
      console.warn("CampusForge demo changes could not be saved.", error);
    }
  }, [data]);

  useEffect(() => {
    if (!notice) return undefined;
    const timeout = window.setTimeout(() => setNotice(""), 2600);
    return () => window.clearTimeout(timeout);
  }, [notice]);

  function updateData(updater) {
    setData((current) => updater(current));
  }

  function addActivity(description, eventType, boardId = activeBoard?.id) {
    return {
      id: createId(),
      board_id: boardId,
      actor_id: data.current_user_id,
      event_type: eventType,
      description,
      created_at: new Date().toISOString()
    };
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

  function saveTask(values) {
    const now = new Date().toISOString();
    if (editingTask) {
      updateData((current) => {
        const previousList = current.lists.find((item) => item.id === editingTask.list_id);
        const nextList = current.lists.find((item) => item.id === values.list_id);
        const updatedTasks = current.tasks.map((task) => task.id === editingTask.id
          ? { ...task, ...values, updated_at: now }
          : task);
        const assignments = current.task_assignees.filter((item) => item.task_id !== editingTask.id);
        if (values.assignee_id) assignments.push({ task_id: editingTask.id, user_id: values.assignee_id });
        const changedList = previousList?.id !== nextList?.id;
        const event = {
          id: createId(),
          board_id: editingTask.board_id,
          actor_id: current.current_user_id,
          event_type: changedList ? "task_moved" : "task_updated",
          description: changedList ? "moved a task to " + (nextList?.name || "another list") : "updated a task",
          created_at: now
        };
        return { ...current, tasks: updatedTasks, task_assignees: assignments, activity_events: [event, ...current.activity_events] };
      });
      setNotice("Task updated");
    } else {
      const newTask = {
        id: createId(),
        board_id: activeBoard?.id,
        list_id: values.list_id,
        title: values.title,
        description: values.description,
        priority: values.priority,
        due_date: values.due_date,
        position: boardTasks.filter((task) => task.list_id === values.list_id).length + 1,
        created_by: data.current_user_id,
        created_at: now,
        updated_at: now
      };
      updateData((current) => ({
        ...current,
        tasks: [...current.tasks, newTask],
        task_assignees: values.assignee_id
          ? [...current.task_assignees, { task_id: newTask.id, user_id: values.assignee_id }]
          : current.task_assignees,
        activity_events: [{
          id: createId(), board_id: newTask.board_id, actor_id: current.current_user_id,
          event_type: "task_created", description: "created " + newTask.title, created_at: now
        }, ...current.activity_events]
      }));
      setNotice("Task created");
    }
    closeDialog();
  }

  function deleteTask(taskId) {
    const task = data.tasks.find((item) => item.id === taskId);
    updateData((current) => ({
      ...current,
      tasks: current.tasks.filter((item) => item.id !== taskId),
      task_assignees: current.task_assignees.filter((item) => item.task_id !== taskId),
      comments: current.comments.filter((item) => item.task_id !== taskId),
      activity_events: [{
        id: createId(), board_id: task?.board_id, actor_id: current.current_user_id,
        event_type: "task_deleted", description: "deleted " + (task?.title || "a task"), created_at: new Date().toISOString()
      }, ...current.activity_events]
    }));
    closeDialog();
    setNotice("Task deleted");
  }

  function addComment(taskId, body) {
    const now = new Date().toISOString();
    updateData((current) => ({
      ...current,
      comments: [...current.comments, {
        id: createId(), task_id: taskId, author_id: current.current_user_id, body, created_at: now
      }],
      activity_events: [{
        id: createId(), board_id: current.tasks.find((task) => task.id === taskId)?.board_id,
        actor_id: current.current_user_id, event_type: "comment_added",
        description: "added a comment to a task", created_at: now
      }, ...current.activity_events]
    }));
    setNotice("Comment added");
  }

  function moveTask(taskId, destinationListId, overId) {
    const task = data.tasks.find((item) => item.id === taskId);
    if (!task || !destinationListId) return;
    const now = new Date().toISOString();
    updateData((current) => {
      const moving = current.tasks.find((item) => item.id === taskId);
      const destinationTasks = current.tasks
        .filter((item) => item.list_id === destinationListId && item.id !== taskId)
        .sort((a, b) => a.position - b.position);
      const overTaskIndex = destinationTasks.findIndex((item) => item.id === overId);
      const insertAt = overTaskIndex >= 0 ? overTaskIndex : destinationTasks.length;
      destinationTasks.splice(insertAt, 0, { ...moving, list_id: destinationListId });
      const positionById = new Map(destinationTasks.map((item, index) => [item.id, index + 1]));
      const nextTasks = current.tasks.map((item) => {
        if (item.id === taskId) return { ...item, list_id: destinationListId, position: positionById.get(item.id), updated_at: now };
        if (item.list_id === destinationListId) return { ...item, position: positionById.get(item.id) };
        return item;
      });
      const listName = current.lists.find((item) => item.id === destinationListId)?.name || "another list";
      return {
        ...current,
        tasks: nextTasks,
        activity_events: [{
          id: createId(), board_id: moving.board_id, actor_id: current.current_user_id,
          event_type: "task_moved", description: "moved " + moving.title + " to " + listName, created_at: now
        }, ...current.activity_events]
      };
    });
    setNotice("Task moved");
  }

  function resetDemo() {
    const fresh = createDemoData();
    setData(fresh);
    setActiveBoardId(fresh.boards[0]?.id || "");
    setActiveView("Overview");
    setSearch("");
    setNotice("Demo workspace reset");
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
          />
        ) : <EmptyState title="No project selected" description="Choose a project from the sidebar." />;
      case "My Tasks":
        return <MyTasksPage data={data} onOpenTask={openTask} search={search} />;
      case "Team":
        return <TeamPage data={data} />;
      case "Activity":
        return <ActivityPage data={data} />;
      case "Projects":
        return <ProjectsPage data={data} onOpenBoard={openBoard} />;
      case "Settings":
        return (
          <div className="page-stack">
            <div className="page-title-row"><div><p className="eyebrow">PREFERENCES</p><h1>Settings</h1><p className="page-subtitle">Manage your local demo workspace.</p></div></div>
            <section className="surface-panel settings-panel">
              <div className="settings-row"><div><strong>Demo workspace data</strong><p>Reset local demo data to its original sample state. This only affects this browser.</p></div><button className="secondary-button danger-outline" type="button" onClick={resetDemo}>Reset demo data</button></div>
              <div className="settings-row"><div><strong>Backend connection</strong><p>The Rust/Axum API is not connected to the frontend yet. No account or remote data is being used.</p></div><span className="connection-badge"><i /> Demo mode</span></div>
            </section>
          </div>
        );
      case "Help":
        return (
          <div className="page-stack">
            <div className="page-title-row"><div><p className="eyebrow">WE'RE HERE TO HELP</p><h1>Help & support</h1><p className="page-subtitle">A quick guide to the current frontend preview.</p></div></div>
            <section className="surface-panel help-panel"><h2>Working with the preview</h2><p>Create tasks, move them between board columns, assign team members, add comments, and search your task list. Changes persist in this browser using local storage.</p><p>This is a frontend-only demo. Sign-in, invitations, server persistence, and real-time multi-user updates will be connected after the backend is implemented.</p></section>
          </div>
        );
      default:
        return (
          <OverviewPage
            data={data}
            onOpenBoard={openBoard}
            onOpenTask={openTask}
            onCreateTask={() => openNewTask()}
            onNavigate={navigate}
          />
        );
    }
  }

  const pageTitle = activeView === "Board" ? activeBoard?.name || "Project board" : activeView;
  const searchPlaceholder = activeView === "Board" ? "Search cards..." : "Search tasks...";
  const showSearch = ["Board", "My Tasks"].includes(activeView);

  return (
    <div className="app-shell">
      <AppSidebar
        activeView={activeView}
        onNavigate={navigate}
        onOpenBoard={openBoard}
        boards={data.boards}
        currentUser={currentUser}
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
            <span className="preview-status"><i /> Local preview</span>
            <button className="icon-button notification-button" type="button" aria-label="Notifications" onClick={() => navigate("Activity")}><Bell size={18} /><span className="notification-dot" /></button>
            <Avatar name={currentUser?.full_name || "You"} />
          </div>
        </header>

        {mobileMenuOpen && (
          <div className="mobile-nav-overlay">
            <div className="mobile-nav-card">
              <div className="mobile-nav-heading"><strong>CampusForge</strong><button type="button" onClick={() => setMobileMenuOpen(false)} aria-label="Close navigation"><X size={18} /></button></div>
              {navigation.map(({ id, label, icon: Icon }) => <button key={id} type="button" className="nav-item" onClick={() => navigate(id)}><Icon size={18} />{label}</button>)}
              <button type="button" className="nav-item" onClick={() => navigate("Projects")}><FolderKanban size={18} />Projects</button>
            </div>
          </div>
        )}

        <main className="page-content">{renderPage()}</main>
        <footer className="page-footer"><span>CampusForge Teams</span><span>Plan better. Collaborate smarter. Build together.</span></footer>
      </div>

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
      />
      {notice && <div className="toast-message" role="status"><CheckCircle2 size={16} />{notice}</div>}
    </div>
  );
}
