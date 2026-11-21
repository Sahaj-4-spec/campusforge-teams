export const STORAGE_KEY = "campusforge-teams-demo-v1";

const ids = {
  workspace: "10000000-0000-4000-8000-000000000001",
  currentUser: "20000000-0000-4000-8000-000000000001",
  board: "30000000-0000-4000-8000-000000000001",
  backlog: "40000000-0000-4000-8000-000000000001",
  progress: "40000000-0000-4000-8000-000000000002",
  review: "40000000-0000-4000-8000-000000000003",
  done: "40000000-0000-4000-8000-000000000004"
};

export function createId() {
  if (typeof crypto !== "undefined" && crypto.randomUUID) {
    return crypto.randomUUID();
  }
  return "demo-" + Date.now().toString(36) + "-" + Math.random().toString(36).slice(2, 9);
}

// The demo records mirror the PostgreSQL entities in docs/ARCHITECTURE.md.
// They remain in localStorage until the authenticated API is implemented.
export function createDemoData() {
  return {
    current_user_id: ids.currentUser,
    current_workspace_id: ids.workspace,
    workspaces: [
      {
        id: ids.workspace,
        name: "CampusForge Workspace",
        slug: "campusforge",
        created_at: "2026-10-01T09:00:00.000Z"
      }
    ],
    users: [
      { id: ids.currentUser, full_name: "You", email: "you@example.local", avatar_url: null },
      { id: "20000000-0000-4000-8000-000000000002", full_name: "Aarav Mehta", email: "aarav@example.local", avatar_url: null },
      { id: "20000000-0000-4000-8000-000000000003", full_name: "Riya Sharma", email: "riya@example.local", avatar_url: null },
      { id: "20000000-0000-4000-8000-000000000004", full_name: "Kabir Singh", email: "kabir@example.local", avatar_url: null }
    ],
    workspace_members: [
      { workspace_id: ids.workspace, user_id: ids.currentUser, role: "owner" },
      { workspace_id: ids.workspace, user_id: "20000000-0000-4000-8000-000000000002", role: "member" },
      { workspace_id: ids.workspace, user_id: "20000000-0000-4000-8000-000000000003", role: "member" },
      { workspace_id: ids.workspace, user_id: "20000000-0000-4000-8000-000000000004", role: "member" }
    ],
    boards: [
      {
        id: ids.board,
        workspace_id: ids.workspace,
        name: "CampusForge Teams",
        description: "Plan, build, review, and ship the team collaboration platform.",
        visibility: "workspace",
        created_by: ids.currentUser,
        created_at: "2026-10-04T10:00:00.000Z"
      },
      {
        id: "30000000-0000-4000-8000-000000000002",
        workspace_id: ids.workspace,
        name: "ALTA Builder 30",
        description: "Challenge milestones, deliverables, and review checkpoints.",
        visibility: "workspace",
        created_by: ids.currentUser,
        created_at: "2026-10-07T10:00:00.000Z"
      }
    ],
    board_members: [
      { board_id: ids.board, user_id: ids.currentUser, role: "admin" },
      { board_id: ids.board, user_id: "20000000-0000-4000-8000-000000000002", role: "member" },
      { board_id: ids.board, user_id: "20000000-0000-4000-8000-000000000003", role: "member" },
      { board_id: ids.board, user_id: "20000000-0000-4000-8000-000000000004", role: "member" }
    ],
    lists: [
      { id: ids.backlog, board_id: ids.board, name: "To do", position: 1 },
      { id: ids.progress, board_id: ids.board, name: "In progress", position: 2 },
      { id: ids.review, board_id: ids.board, name: "In review", position: 3 },
      { id: ids.done, board_id: ids.board, name: "Done", position: 4 }
    ],
    tasks: [
      {
        id: "50000000-0000-4000-8000-000000000001",
        board_id: ids.board,
        list_id: ids.backlog,
        title: "Map the onboarding experience",
        description: "Document the first-time experience from workspace creation through the first team invitation.",
        priority: "high",
        due_date: "2026-11-24",
        position: 1,
        created_by: ids.currentUser,
        created_at: "2026-11-10T09:00:00.000Z",
        updated_at: "2026-11-18T09:00:00.000Z"
      },
      {
        id: "50000000-0000-4000-8000-000000000002",
        board_id: ids.board,
        list_id: ids.backlog,
        title: "Create empty states for boards",
        description: "Add helpful first-run guidance when a board or list has no tasks.",
        priority: "medium",
        due_date: "2026-11-27",
        position: 2,
        created_by: ids.currentUser,
        created_at: "2026-11-12T09:00:00.000Z",
        updated_at: "2026-11-12T09:00:00.000Z"
      },
      {
        id: "50000000-0000-4000-8000-000000000003",
        board_id: ids.board,
        list_id: ids.progress,
        title: "Build the responsive workspace shell",
        description: "Implement navigation, dashboard summary, responsive layouts, and global task actions.",
        priority: "high",
        due_date: "2026-11-23",
        position: 1,
        created_by: ids.currentUser,
        created_at: "2026-11-05T09:00:00.000Z",
        updated_at: "2026-11-20T09:00:00.000Z"
      },
      {
        id: "50000000-0000-4000-8000-000000000004",
        board_id: ids.board,
        list_id: ids.progress,
        title: "Connect task interactions to the data model",
        description: "Keep task, list, assignee, comment, and activity records aligned with the planned relational schema.",
        priority: "medium",
        due_date: "2026-11-26",
        position: 2,
        created_by: ids.currentUser,
        created_at: "2026-11-13T09:00:00.000Z",
        updated_at: "2026-11-19T09:00:00.000Z"
      },
      {
        id: "50000000-0000-4000-8000-000000000005",
        board_id: ids.board,
        list_id: ids.review,
        title: "Review the PostgreSQL entity relationships",
        description: "Check workspace membership, board access, task assignments, comments, and activity history.",
        priority: "medium",
        due_date: "2026-11-22",
        position: 1,
        created_by: ids.currentUser,
        created_at: "2026-11-14T09:00:00.000Z",
        updated_at: "2026-11-21T09:00:00.000Z"
      },
      {
        id: "50000000-0000-4000-8000-000000000006",
        board_id: ids.board,
        list_id: ids.done,
        title: "Choose the application architecture",
        description: "Confirm React, Vite, Tailwind, Rust, Axum, PostgreSQL, and SQLx.",
        priority: "low",
        due_date: "2026-11-18",
        position: 1,
        created_by: ids.currentUser,
        created_at: "2026-11-02T09:00:00.000Z",
        updated_at: "2026-11-18T16:00:00.000Z"
      }
    ],
    task_assignees: [
      { task_id: "50000000-0000-4000-8000-000000000001", user_id: "20000000-0000-4000-8000-000000000003" },
      { task_id: "50000000-0000-4000-8000-000000000003", user_id: ids.currentUser },
      { task_id: "50000000-0000-4000-8000-000000000003", user_id: "20000000-0000-4000-8000-000000000002" },
      { task_id: "50000000-0000-4000-8000-000000000004", user_id: "20000000-0000-4000-8000-000000000004" },
      { task_id: "50000000-0000-4000-8000-000000000005", user_id: "20000000-0000-4000-8000-000000000002" }
    ],
    comments: [
      {
        id: "60000000-0000-4000-8000-000000000001",
        task_id: "50000000-0000-4000-8000-000000000005",
        author_id: "20000000-0000-4000-8000-000000000002",
        body: "The board and task relationships look consistent. I will check the invitation flow next.",
        created_at: "2026-11-21T12:30:00.000Z"
      }
    ],
    activity_events: [
      {
        id: "70000000-0000-4000-8000-000000000001",
        board_id: ids.board,
        actor_id: "20000000-0000-4000-8000-000000000003",
        event_type: "task_moved",
        description: "moved a task into In review",
        created_at: "2026-11-21T14:20:00.000Z"
      },
      {
        id: "70000000-0000-4000-8000-000000000002",
        board_id: ids.board,
        actor_id: ids.currentUser,
        event_type: "task_created",
        description: "created the responsive workspace shell task",
        created_at: "2026-11-20T10:10:00.000Z"
      },
      {
        id: "70000000-0000-4000-8000-000000000003",
        board_id: ids.board,
        actor_id: "20000000-0000-4000-8000-000000000004",
        event_type: "comment_added",
        description: "commented on the data model review",
        created_at: "2026-11-19T16:40:00.000Z"
      }
    ]
  };
}

export function loadDemoData() {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (saved) {
      const parsed = JSON.parse(saved);
      if (parsed && Array.isArray(parsed.tasks) && Array.isArray(parsed.lists)) {
        return parsed;
      }
    }
  } catch (error) {
    console.warn("CampusForge demo data could not be loaded. Starting fresh.", error);
  }

  return createDemoData();
}
