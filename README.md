# 🚀 CampusForge Teams

### A Real-Time Project Management Platform for Student Teams

<p align="center">
  <strong>Plan Better. Collaborate Smarter. Build Together.</strong>
</p>

CampusForge Teams is a project management and collaboration platform being built for college students and development teams. It is designed to help teams create project boards, organize tasks, assign responsibilities, track deadlines, exchange comments, and follow board activity from one workspace.

This project is being developed as part of the **ALTA BUILDER 30 Challenge — Track A (Full-Stack Product)**.

> **Development status:** The React dashboard is connected to an authenticated Rust/Axum API for workspace loading, project and task management, column management, task assignments, comments, invitations, and persisted drag-and-drop ordering. Authentication uses Argon2 password hashing and 24-hour JWT sessions. Invitation tokens are random, stored as SHA-256 hashes, email-bound, single-use, and expire after seven days. Workspace owners/admins manage projects, columns, and invitations. Browser end-to-end and multi-session WebSocket acceptance tests remain.

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
- Create and manage project boards.
- Invite teammates with member or admin roles using expiring, single-use invitation links.
- Accept invitations only while signed in with the invited email address.
- Revoke pending invitations and inspect their expiry.
- Restrict project and column management to workspace owners and admins.

### 3. Task and List Management
- Create, rename, and delete empty columns; every project must retain at least one column.
- Create, edit, and delete tasks.
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
- `POST /api/boards/:board_id/lists`, `PATCH /api/lists/:id`, and `DELETE /api/lists/:id` manage project columns.
- `GET/POST /api/invitations/`, `DELETE /api/invitations/:id`, and `POST /api/invitations/accept` manage workspace invitations.
- Protected routes require an `Authorization: Bearer <token>` header. SQLx provides asynchronous access to PostgreSQL.
- The Vite development server proxies `/api` and `/ws` to the backend on port `5000`. For a separately hosted frontend, set `VITE_API_BASE_URL` to the API service's HTTPS origin. Invitation links use the frontend root query string (`?invite=...`) and are accepted after the invitee signs in.

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

The backend is split into Axum API, authentication, invitation, column-management, database, real-time, and shared-state modules. The frontend includes the workspace shell, board/task dialogs, authentication, invitation controls, and a shared API client.

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

The frontend milestone is connected to the authenticated Rust API.

- Overview dashboard with project and task summaries loaded from PostgreSQL.
- Project directory and Kanban boards, including creation of a project with four starter columns.
- Create, edit, and delete task flows with priority, due date, list, and assignee fields.
- Task comments and a recent activity timeline loaded from the API.
- Workspace team directory, task search, and filters.
- Responsive layout for desktop, tablet, and mobile screens.
- JWT-backed sign-in and registration; the browser stores the session token and sends it with API requests.
- Authenticated workspace WebSocket connection with reconnect handling and workspace snapshot refreshes after remote changes.

The frontend uses sample records shaped around the planned relational entities: `workspaces`, `workspace_members`, `users`, `boards`, `board_members`, `lists`, `tasks`, `task_assignees`, `comments`, and `activity_events`. This is a UI prototype data layer, not a replacement for PostgreSQL.

**Connected:** account registration/sign-in, authenticated workspace loading, project creation, task CRUD, assignee persistence, comments, database-backed task reordering, column creation/rename/deletion, invitation creation/acceptance/revocation, workspace owner/admin checks, and workspace-scoped WebSocket event delivery. **Still pending:** automated browser end-to-end tests, live two-browser WebSocket acceptance tests, full workspace switching, and granular per-board permissions.

## 🎨 UI Design System

The frontend uses a Cal.com-inspired neutral SaaS visual language adapted for CampusForge Teams. The full design reference and reusable tokens are documented in the root `DESIGN.md`.

- Theme implementation: `client/src/styles/cal-theme.css`.
- Theme entry point: imported by `client/src/main.jsx` after the base stylesheet.
- Typography: Inter for display and interface text; JetBrains Mono is available for code-oriented UI.
- Visual rules: white canvas, black primary actions, subtle gray surfaces and borders, 8–12px control/card radii, and restrained semantic colors for task status.
- No additional runtime UI library is required. Existing React components, navigation, drag-and-drop, and dialogs now use authenticated API-backed data.

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

**Status:** Core frontend/API integration and local/hosted database wiring are implemented. Full production acceptance testing is still required.

The React client authenticates against the Rust API and loads workspace records from PostgreSQL. Task creation, editing, deletion, assignments, comments, project creation, column management, invitation workflows, and drag-and-drop ordering call protected Axum endpoints; they no longer rely on browser-local demo records. New accounts receive a starter workspace and project board when the workspace endpoint is first loaded. Authenticated WebSocket sessions receive workspace-scoped events after committed mutations, and the frontend reloads the snapshot when remote changes arrive.

**Not yet complete:** automated browser end-to-end tests, live multi-user WebSocket acceptance testing, full workspace switching, and granular per-board permissions. GitHub Actions checks the frontend build and Rust backend; dependency audits and CodeQL analysis are also configured. The production API has connected to PostgreSQL and applied SQLx migrations. The real-time broadcast channel is process-local, so the current single-instance deployment is intentional; multiple API instances require shared pub/sub before scaling horizontally.

## 🎓 Challenge Information

- **Challenge:** ALTA BUILDER 30
- **Track:** A — Full-Stack Product
- **Project:** Project Board (Trello-style)
- **Duration:** 15 Days

---

<p align="center">
  <strong>CampusForge Teams — Build Together. Achieve Together.</strong>
</p>
