# 🚀 CampusForge Teams

### A Real-Time Project Management Platform for Student Teams

<p align="center">
  <strong>Plan Better. Collaborate Smarter. Build Together.</strong>
</p>

CampusForge Teams is a project management and collaboration platform being built for college students and development teams. It is designed to help teams create project boards, organize tasks, assign responsibilities, track deadlines, exchange comments, and follow board activity from one workspace.

This project is being developed as part of the **ALTA BUILDER 30 Challenge — Track A (Full-Stack Product)**.

> **Development status:** The React dashboard is connected to an authenticated Rust/Axum API for workspace loading, task creation/editing/deletion, task assignment, comments, and persisted drag-and-drop ordering. Authentication uses Argon2 password hashing and 24-hour JWT sessions. PostgreSQL access and SQLx migrations are implemented. Board creation, invitations, richer role administration, and real-time WebSocket synchronization remain to be completed and tested.

---

## 🎯 Problem Statement

Student teams often manage projects through disconnected messaging applications, spreadsheets, and informal discussions. This can lead to unclear responsibilities, missed deadlines, poor visibility into progress, and difficulty coordinating work.

CampusForge Teams aims to bring project planning and collaboration into one structured workspace.

## 💡 Proposed Solution

CampusForge Teams is planned as a Trello-inspired Kanban application. Team members will be able to create boards, invite collaborators, organize work into lists, assign tasks, discuss cards, and see board changes in real time.

## ✨ Planned Features

### 1. Authentication and User Profiles
- User registration and login.
- Secure password hashing.
- Authenticated and authorized API access.
- User profiles with names and avatars.

### 2. Project and Board Management
- Create, edit, and manage project boards.
- Invite teammates and manage membership.
- Join boards through controlled invitations.
- Restrict board access to authorized members.

### 3. Task and List Management
- Create, edit, and delete lists and cards.
- Add descriptions and due dates.
- Assign tasks to team members.
- Move cards between lists and reorder cards.
- Persist card positions in PostgreSQL.

### 4. Comments and Activity History
- Add comments to task cards.
- Record important board and task activity.
- Display a chronological activity feed.

### 5. Advanced Feature: Drag and Drop with Real-Time Updates
- Drag cards between lists.
- Reorder cards within a list.
- Persist the new order in PostgreSQL.
- Broadcast authorized board changes to connected members through native WebSockets.

### 6. Additional Feature: Deadline Risk Radar
- Identify overdue tasks and upcoming deadlines.
- Display project completion statistics.
- Highlight tasks without an assigned member.

## 🛠️ Technology Stack

| Technology | Purpose |
|---|---|
| React + Vite | Frontend application |
| Tailwind CSS | Responsive interface styling |
| Rust | Backend programming language |
| Axum | HTTP API and WebSocket server |
| PostgreSQL | Relational database |
| SQLx | Asynchronous, typed SQL access and migrations |
| Native WebSockets (Axum) | Real-time collaboration |
| Argon2 | Password hashing |
| JSON Web Token (JWT) | Token-based authentication |
| Git and GitHub | Version control |

**Architecture decision:** The backend is Rust + Axum, and the database is PostgreSQL accessed through SQLx. Node.js/Express, MongoDB/Mongoose, and Socket.IO are not part of the selected backend architecture.

## 🏗️ Planned Architecture

```text
                 CAMPUSFORGE TEAMS
                         |
             +-----------+-----------+
             |                       |
         FRONTEND                  BACKEND
             |                       |
       React + Vite              Rust + Axum
             |                       |
       Tailwind CSS          REST API + WebSocket
             |                       |
             +------ HTTP/WS --------+
                         |
                       SQLx
                         |
                    PostgreSQL
```

### Communication
- `POST /api/auth/register` and `POST /api/auth/login` create and authenticate accounts.
- `GET /api/workspace` returns the signed-in user's workspace, members, boards, lists, tasks, assignments, comments, and activity.
- `POST /api/tasks`, `PATCH /api/tasks/:id`, and `DELETE /api/tasks/:id` persist task changes.
- `POST /api/tasks/:id/comments` saves discussion comments; `PATCH /api/tasks/:id/move` persists drag-and-drop order.
- Protected routes require an `Authorization: Bearer <token>` header. SQLx provides asynchronous access to PostgreSQL.
- The Vite development server proxies `/api` and `/ws` to the backend on port `5000`. For a separately hosted frontend, set `VITE_API_BASE_URL` to the API service's HTTPS origin.

## 📁 Repository Structure

Current files and the initial backend scaffold:

```text
campusforge-teams/
├── .env.example
├── .gitignore
├── README.md
├── package.json
│
├── client/
│   ├── index.html
│   ├── package.json
│   ├── vite.config.js
│   └── src/
│       ├── App.jsx
│       ├── components/
│       │   └── AuthScreen.jsx
│       ├── lib/
│       │   └── api.js
│       ├── index.css
│       ├── main.jsx
│       ├── components/
│       │   ├── BoardView.jsx
│       │   └── TaskDialog.jsx
│       └── data/
│           └── demoData.js
│
└── server/
    ├── Cargo.toml
    └── src/
        └── main.rs
```

As implementation progresses, the backend will be expanded into modules for database access, models, routes, handlers, middleware, validation, and WebSocket events. The frontend will be expanded with reusable components, pages, context providers, hooks, and API services.

## ⚙️ Prerequisites

Install the following tools:

- Node.js and npm.
- Rust stable toolchain and Cargo.
- PostgreSQL.
- Git.

## 🚀 Local Development Setup

Clone the repository:

```bash
git clone https://github.com/Sahaj-4-spec/campusforge-teams.git
cd campusforge-teams
```

Install root and frontend dependencies:

```bash
npm install
npm --prefix client install
```

Create the local backend and frontend environment files:

```bash
cp .env.example .env
cp client/.env.example client/.env
```

Edit `.env` and replace placeholder values with your local configuration. Never commit `.env` or real credentials.

| Variable | Purpose |
|---|---|
| `PORT` | Axum API port (default: `5000`) |
| `DATABASE_URL` | PostgreSQL connection string |
| `JWT_SECRET` | Secret reserved for token signing |
| `FRONTEND_ORIGIN` | Exact frontend origin allowed by API CORS (local: `http://localhost:5173`) |
| `VITE_API_BASE_URL` | API origin for a separately hosted frontend; leave blank locally to use the Vite proxy |

> For Neon, use the pooled or direct PostgreSQL connection string supplied by your Neon project and retain `sslmode=require`. Keep secrets in environment files or the hosting provider's secret store; never commit them.

Run the frontend and Rust backend concurrently from the repository root:

```bash
npm run dev
```

- Frontend: `http://localhost:5173`
- Backend health check: `http://localhost:5000/api/health`

Build both applications:

```bash
npm run build
```

Run the Rust backend in release mode:

```bash
npm start
```

## 🖥️ Frontend Milestone

The first frontend milestone is implemented with the selected React + Vite stack.

- Overview dashboard with project and task summaries.
- Project directory and Kanban board with drag-and-drop task movement.
- Create, edit, and delete task flows with priority, due date, list, and assignee fields.
- Task comments and a recent activity timeline.
- Workspace team directory, task search, and filters.
- Responsive layout for desktop, tablet, and mobile screens.
- Browser-local demo persistence so interactions can be explored before the API is ready.

The frontend uses sample records shaped around the planned relational entities: `workspaces`, `workspace_members`, `users`, `boards`, `board_members`, `lists`, `tasks`, `task_assignees`, `comments`, and `activity_events`. This is a UI prototype data layer, not a replacement for PostgreSQL.

**Connected:** account registration/sign-in, authenticated workspace loading, task CRUD, assignee persistence, comments, and database-backed task reordering. **Still pending:** invitations, board/list creation and administration, granular per-board permissions, native WebSocket synchronization, and full automated integration tests.

## 🎨 UI Design System

The frontend uses a Cal.com-inspired neutral SaaS visual language adapted for CampusForge Teams. The full design reference and reusable tokens are documented in the root `DESIGN.md`.

- Theme implementation: `client/src/styles/cal-theme.css`.
- Theme entry point: imported by `client/src/main.jsx` after the base stylesheet.
- Typography: Inter for display and interface text; JetBrains Mono is available for code-oriented UI.
- Visual rules: white canvas, black primary actions, subtle gray surfaces and borders, 8–12px control/card radii, and restrained semantic colors for task status.
- No additional runtime UI library is required. Existing React components, navigation, drag-and-drop, dialogs, and local demo behavior are retained.

The Cal.com reference is an independent visual analysis and is not affiliated with or endorsed by Cal.com; source attribution is recorded in `DESIGN.md`.

## 🔒 Security Considerations

- Store passwords using Argon2 password hashing.
- Keep secrets in environment variables.
- Authenticate API requests and authorize board membership on the server.
- Validate all incoming request data.
- Use parameterized SQL queries through SQLx.
- Restrict CORS to the configured frontend origin.
- Authorize WebSocket connections and board-room membership.
- Never commit `.env`, database credentials, tokens, or private user files.

## 📖 What I Built and What I Understood

This section will be updated as implementation is completed. The learning goals include:

1. How React communicates with a Rust backend through HTTP APIs.
2. How Axum routes requests and manages application state.
3. How PostgreSQL models users, boards, lists, cards, comments, and activities.
4. How SQLx performs asynchronous, parameterized database queries.
5. How authentication and authorization protect application data.
6. How drag-and-drop interactions update persistent card ordering.
7. How native WebSockets synchronize changes between authorized team members.

## 🤖 AI Tools and Development Assistance

AI-assisted tools may be used to understand concepts, explore implementation approaches, debug issues, and improve documentation. AI-assisted code will be reviewed, tested, and understood before it is incorporated into the project. Specific tools and contributions will be documented accurately as development progresses.

## 🚧 Current Project Status

**Status:** Core frontend/API integration is in progress.

The React client now authenticates against the Rust API and loads workspace records from PostgreSQL. Task creation, editing, deletion, assignments, comments, and drag-and-drop ordering call protected Axum endpoints; they no longer rely on browser-local demo records. New accounts receive a starter workspace and project board when the workspace endpoint is first loaded.

**Not yet complete:** board/list management, invitation acceptance, granular role administration, WebSocket live collaboration, automated end-to-end tests, and deployment configuration validation. The code must pass CI and be tested against a real Neon database before it is considered ready for production.

## 🎓 Challenge Information

- **Challenge:** ALTA BUILDER 30
- **Track:** A — Full-Stack Product
- **Project:** Project Board (Trello-style)
- **Duration:** 15 Days

---

<p align="center">
  <strong>CampusForge Teams — Build Together. Achieve Together.</strong>
</p>
