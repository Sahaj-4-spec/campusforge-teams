import { useEffect, useMemo, useRef, useState } from "react";
import {
  Activity, BarChart3, Bell, CalendarDays, FileText, FolderKanban,
  LayoutDashboard, ListTodo, MessageSquare, Plus, Search, Users
} from "lucide-react";
import { searchCommandItems } from "../lib/commandSearch.js";

const NAVIGATION = [
  { id: "overview", label: "Dashboard", description: "Workspace overview and project pulse", category: "Navigate", icon: LayoutDashboard, keywords: ["home", "overview"], view: "Overview" },
  { id: "projects", label: "Projects", description: "Browse all project boards", category: "Navigate", icon: FolderKanban, keywords: ["boards"], view: "Projects" },
  { id: "my-tasks", label: "My tasks", description: "Review assigned work and filters", category: "Navigate", icon: ListTodo, keywords: ["personal", "todo"], view: "My Tasks" },
  { id: "calendar", label: "Calendar", description: "Deadlines, meetings, and milestones", category: "Navigate", icon: CalendarDays, keywords: ["schedule", "events"], view: "Calendar" },
  { id: "notifications", label: "Notifications", description: "Updates, mentions, and reminders", category: "Navigate", icon: Bell, keywords: ["inbox", "alerts"], view: "Notifications" },
  { id: "files", label: "Files & docs", description: "Shared project resources", category: "Navigate", icon: FileText, keywords: ["documents", "attachments"], view: "Files" },
  { id: "chat", label: "Chat & notes", description: "Team conversations and meeting notes", category: "Navigate", icon: MessageSquare, keywords: ["messages", "meetings"], view: "Chat" },
  { id: "team", label: "Team", description: "Members, invitations, and roles", category: "Navigate", icon: Users, keywords: ["people", "members"], view: "Team" },
  { id: "activity", label: "Timeline", description: "Recent project and task changes", category: "Navigate", icon: Activity, keywords: ["history", "audit"], view: "Activity" },
  { id: "reports", label: "Analytics", description: "Completion, progress, and workload trends", category: "Navigate", icon: BarChart3, keywords: ["reports", "metrics"], view: "Reports" }
];

export default function CommandPalette({
  open,
  onClose,
  onNavigate,
  onOpenBoard,
  onOpenTask,
  onCreateTask,
  onCreateProject,
  onSearchTasks,
  boards = [],
  tasks = []
}) {
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef(null);

  useEffect(() => {
    if (!open) return undefined;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    inputRef.current?.focus();
    return () => {
      document.body.style.overflow = previousOverflow;
    };
  }, [open]);

  const results = useMemo(() => {
    const actions = [
      {
        id: "create-task",
        label: "Create a task",
        description: "Add work to the current project",
        category: "Quick action",
        icon: Plus,
        keywords: ["new card", "add task", "todo"],
        run: onCreateTask
      },
      {
        id: "create-project",
        label: "Create a project",
        description: "Start a new team project board",
        category: "Quick action",
        icon: FolderKanban,
        keywords: ["new board", "add project"],
        run: onCreateProject
      }
    ];
    const navigation = NAVIGATION.map((item) => ({
      ...item,
      run: () => onNavigate(item.view)
    }));
    const projectItems = boards.map((board) => ({
      id: "board-" + board.id,
      label: board.name,
      description: board.description || "Open project board",
      category: "Project",
      icon: FolderKanban,
      keywords: ["project", "board"],
      run: () => onOpenBoard(board)
    }));
    const taskItems = query.trim() ? tasks.map((task) => {
      const board = boards.find((item) => item.id === task.board_id);
      return {
        id: "task-" + task.id,
        label: task.title,
        description: [board?.name, task.list_name || "Task", task.priority ? task.priority + " priority" : ""].filter(Boolean).join(" · "),
        category: "Task",
        icon: ListTodo,
        keywords: [task.description || "", board?.name || "", task.list_name || ""],
        run: () => onOpenTask(task)
      };
    }) : [];
    const searchAll = query.trim() ? [{
      id: "search-all-tasks",
      label: "Search all tasks for “" + query.trim() + "”",
      description: "Open My tasks with this search applied",
      category: "Search",
      icon: Search,
      keywords: ["all tasks", "filter"],
      run: () => onSearchTasks(query.trim())
    }] : [];

    return searchCommandItems([...actions, ...navigation, ...projectItems, ...taskItems, ...searchAll], query, 12);
  }, [query, boards, tasks, onNavigate, onOpenBoard, onOpenTask, onCreateTask, onCreateProject, onSearchTasks]);

  useEffect(() => {
    setActiveIndex(0);
  }, [query, results.length]);

  if (!open) return null;

  function choose(item) {
    if (!item) return;
    item.run?.();
    onClose();
  }

  function handleKeyDown(event) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((index) => results.length ? (index + 1) % results.length : 0);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((index) => results.length ? (index - 1 + results.length) % results.length : 0);
    } else if (event.key === "Enter") {
      event.preventDefault();
      choose(results[activeIndex]);
    } else if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    }
  }

  return (
    <div
      className="command-palette-backdrop"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section
        className="command-palette"
        role="dialog"
        aria-modal="true"
        aria-labelledby="command-palette-title"
      >
        <h2 id="command-palette-title" className="sr-only">Search CampusForge Teams</h2>
        <div className="command-palette-search">
          <Search size={19} aria-hidden="true" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Search tasks, projects, or jump to a page…"
            aria-label="Search tasks, projects, and commands"
            aria-autocomplete="list"
            aria-controls="command-palette-results"
            aria-expanded="true"
            aria-activedescendant={results[activeIndex] ? "command-result-" + activeIndex : undefined}
          />
          <kbd>ESC</kbd>
        </div>
        <div className="command-palette-results" id="command-palette-results" role="listbox" aria-label="Search results">
          {results.length ? results.map((item, index) => {
            const Icon = item.icon || Search;
            return (
              <button
                id={"command-result-" + index}
                key={item.id}
                className={"command-palette-option" + (activeIndex === index ? " is-active" : "")}
                type="button"
                role="option"
                aria-selected={activeIndex === index}
                onMouseEnter={() => setActiveIndex(index)}
                onClick={() => choose(item)}
              >
                <span className="command-palette-option-icon"><Icon size={17} /></span>
                <span className="command-palette-option-copy">
                  <strong>{item.label}</strong>
                  <small>{item.description}</small>
                </span>
                <span className="command-palette-category">{item.category}</span>
              </button>
            );
          }) : (
            <div className="command-palette-empty">
              <Search size={20} />
              <strong>No matching commands or tasks</strong>
              <span>Try a project name, teammate workflow, or task keyword.</span>
            </div>
          )}
        </div>
        <footer className="command-palette-footer">
          <span><kbd>↑</kbd><kbd>↓</kbd> Navigate</span>
          <span><kbd>↵</kbd> Open</span>
          <span><kbd>ESC</kbd> Close</span>
        </footer>
      </section>
    </div>
  );
}
