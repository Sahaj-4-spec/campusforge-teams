import { useMemo, useState } from "react";
import {
  ArrowUpRight,
  Bell,
  CalendarDays,
  CheckCircle2,
  CircleHelp,
  Clock3,
  FolderKanban,
  LayoutDashboard,
  ListTodo,
  MoreHorizontal,
  Plus,
  Search,
  Settings,
  Users,
  Zap,
} from "lucide-react";

const navigation = [
  { label: "Overview", icon: LayoutDashboard },
  { label: "My Projects", icon: FolderKanban },
  { label: "My Tasks", icon: ListTodo },
  { label: "Team", icon: Users },
];

const initialTasks = [
  {
    id: 1,
    title: "Create project wireframes",
    project: "CampusForge Teams",
    due: "Today",
    status: "In progress",
    priority: "High",
  },
  {
    id: 2,
    title: "Set up authentication flow",
    project: "CampusForge Teams",
    due: "Tomorrow",
    status: "To do",
    priority: "High",
  },
  {
    id: 3,
    title: "Review database structure",
    project: "CampusForge Teams",
    due: "Nov 23",
    status: "Review",
    priority: "Medium",
  },
];

function statusClass(status) {
  return status.toLowerCase().replaceAll(" ", "-");
}

export default function App() {
  const [activePage, setActivePage] = useState("Overview");
  const [search, setSearch] = useState("");
  const [tasks, setTasks] = useState(initialTasks);

  const visibleTasks = useMemo(() => {
    const query = search.trim().toLowerCase();

    if (!query) return tasks;

    return tasks.filter(
      (task) =>
        task.title.toLowerCase().includes(query) ||
        task.project.toLowerCase().includes(query)
    );
  }, [search, tasks]);

  function addSampleTask() {
    setTasks((current) => [
      {
        id: Date.now(),
        title: "New task — add your task details",
        project: "CampusForge Teams",
        due: "Not set",
        status: "To do",
        priority: "Medium",
      },
      ...current,
    ]);
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a className="brand" href="/" aria-label="CampusForge Teams home">
          <span className="brand-mark">
            <Zap size={21} />
          </span>
          <span className="brand-name">
            CampusForge
            <small>TEAMS WORKSPACE</small>
          </span>
        </a>

        <div className="sidebar-section-label">WORKSPACE</div>
        <nav className="sidebar-nav" aria-label="Main navigation">
          {navigation.map(({ label, icon: Icon }) => (
            <button
              className={`nav-item ${activePage === label ? "active" : ""}`}
              key={label}
              onClick={() => setActivePage(label)}
              type="button"
            >
              <Icon size={18} />
              <span>{label}</span>
              {label === "My Tasks" && <span className="nav-count">{tasks.length}</span>}
            </button>
          ))}
        </nav>

        <div className="sidebar-section-label projects-label">YOUR PROJECTS</div>
        <button
          className="project-link"
          onClick={() => setActivePage("CampusForge Teams")}
          type="button"
        >
          <span className="project-dot violet" />
          <span>CampusForge Teams</span>
        </button>
        <button
          className="project-link"
          onClick={() => setActivePage("ALTA Builder 30")}
          type="button"
        >
          <span className="project-dot blue" />
          <span>ALTA Builder 30</span>
        </button>

        <div className="sidebar-bottom">
          <button className="nav-item" onClick={() => setActivePage("Help")} type="button">
            <CircleHelp size={18} />
            <span>Help & support</span>
          </button>
          <button className="nav-item" onClick={() => setActivePage("Settings")} type="button">
            <Settings size={18} />
            <span>Settings</span>
          </button>
          <div className="profile-card">
            <div className="avatar">S</div>
            <div className="profile-copy">
              <strong>Workspace member</strong>
              <small>Student account</small>
            </div>
            <MoreHorizontal size={18} />
          </div>
        </div>
      </aside>

      <main className="main-content">
        <header className="topbar">
          <div className="breadcrumbs">
            <span>Workspace</span>
            <span className="breadcrumb-divider">/</span>
            <strong>{activePage}</strong>
          </div>
          <div className="topbar-actions">
            <label className="search-box">
              <Search size={17} />
              <input
                aria-label="Search tasks"
                onChange={(event) => setSearch(event.target.value)}
                placeholder="Search tasks..."
                value={search}
              />
              <kbd>⌘ K</kbd>
            </label>
            <button className="icon-button" type="button" aria-label="Notifications">
              <Bell size={19} />
              <span className="notification-dot" />
            </button>
            <div className="avatar top-avatar" aria-label="Student profile">S</div>
          </div>
        </header>

        <div className="page-content">
          <section className="welcome-row">
            <div>
              <div className="eyebrow">YOUR TEAM WORKSPACE</div>
              <h1>Good work starts with a clear plan.</h1>
              <p>Organize your projects, stay on top of tasks, and build together.</p>
            </div>
            <button className="primary-button" onClick={addSampleTask} type="button">
              <Plus size={18} />
              Add sample task
            </button>
          </section>

          <section className="stats-grid" aria-label="Workspace summary">
            <article className="stat-card">
              <div className="stat-topline">
                <span className="stat-icon purple"><FolderKanban size={19} /></span>
                <span className="stat-note">Workspace</span>
              </div>
              <strong className="stat-value">02</strong>
              <span className="stat-label">Active projects</span>
            </article>
            <article className="stat-card">
              <div className="stat-topline">
                <span className="stat-icon green"><CheckCircle2 size={19} /></span>
                <span className="stat-note">Sample data</span>
              </div>
              <strong className="stat-value">08</strong>
              <span className="stat-label">Tasks completed</span>
            </article>
            <article className="stat-card">
              <div className="stat-topline">
                <span className="stat-icon orange"><Clock3 size={19} /></span>
                <span className="stat-note">Needs attention</span>
              </div>
              <strong className="stat-value">{tasks.length}</strong>
              <span className="stat-label">Open tasks shown</span>
            </article>
            <article className="stat-card">
              <div className="stat-topline">
                <span className="stat-icon blue"><Users size={19} /></span>
                <span className="stat-note">Team</span>
              </div>
              <strong className="stat-value">04</strong>
              <span className="stat-label">Team members</span>
            </article>
          </section>

          <section className="content-grid">
            <div className="primary-column">
              <article className="panel project-panel">
                <div className="panel-heading">
                  <div>
                    <div className="eyebrow">FEATURED PROJECT</div>
                    <h2>CampusForge Teams</h2>
                    <p>Your Trello-style collaboration workspace</p>
                  </div>
                  <button className="more-button" type="button" aria-label="Project options">
                    <MoreHorizontal size={20} />
                  </button>
                </div>
                <div className="project-progress-copy">
                  <span>Project setup</span>
                  <strong>Getting started</strong>
                </div>
                <div className="progress-track">
                  <div className="progress-value" />
                </div>
                <div className="project-footer">
                  <div className="member-stack" aria-label="Example team members">
                    <span className="mini-avatar avatar-one">A</span>
                    <span className="mini-avatar avatar-two">R</span>
                    <span className="mini-avatar avatar-three">P</span>
                    <span className="mini-avatar avatar-more">+1</span>
                  </div>
                  <span className="project-meta"><CalendarDays size={15} /> ALTA Builder 30</span>
                  <button
                    className="text-button"
                    onClick={() => setActivePage("CampusForge Teams")}
                    type="button"
                  >
                    Open project <ArrowUpRight size={16} />
                  </button>
                </div>
              </article>

              <article className="panel tasks-panel">
                <div className="panel-heading tasks-heading">
                  <div>
                    <div className="eyebrow">STAY ORGANIZED</div>
                    <h2>My tasks</h2>
                  </div>
                  <button className="text-button" onClick={() => setActivePage("My Tasks")} type="button">
                    View all <ArrowUpRight size={16} />
                  </button>
                </div>
                <div className="task-list">
                  {visibleTasks.length > 0 ? (
                    visibleTasks.map((task) => (
                      <div className="task-row" key={task.id}>
                        <span className="task-check" aria-hidden="true" />
                        <div className="task-copy">
                          <strong>{task.title}</strong>
                          <small>{task.project}</small>
                        </div>
                        <span className="task-due"><CalendarDays size={14} /> {task.due}</span>
                        <span className={`status-pill ${statusClass(task.status)}`}>{task.status}</span>
                        <span className={`priority priority-${task.priority.toLowerCase()}`}>{task.priority}</span>
                      </div>
                    ))
                  ) : (
                    <div className="empty-state">
                      No tasks match “{search}”. Try another search.
                    </div>
                  )}
                </div>
              </article>
            </div>

            <aside className="secondary-column">
              <article className="panel focus-panel">
                <div className="eyebrow">A LITTLE PROGRESS EVERY DAY</div>
                <div className="focus-illustration"><Zap size={27} /></div>
                <h2>Make today count.</h2>
                <p>Small, consistent improvements turn a project into a product.</p>
                <div className="focus-tip">
                  <strong>Today's focus</strong>
                  <span>Choose one task, finish it, and record what you learned.</span>
                </div>
              </article>

              <article className="panel activity-panel">
                <div className="panel-heading">
                  <h2>Quick overview</h2>
                  <MoreHorizontal size={19} />
                </div>
                <div className="overview-item">
                  <span className="overview-indicator green-indicator" />
                  <span>Completed</span>
                  <strong>08</strong>
                </div>
                <div className="overview-item">
                  <span className="overview-indicator orange-indicator" />
                  <span>In progress</span>
                  <strong>03</strong>
                </div>
                <div className="overview-item">
                  <span className="overview-indicator gray-indicator" />
                  <span>To do</span>
                  <strong>{tasks.length}</strong>
                </div>
              </article>
            </aside>
          </section>

          <footer className="page-footer">
            <span>CampusForge Teams</span>
            <span>Plan better. Collaborate smarter. Build together.</span>
          </footer>
        </div>
      </main>
    </div>
  );
}
