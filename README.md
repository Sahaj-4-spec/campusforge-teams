# 🚀 CampusForge Teams

### A Real-Time Project Management Platform for Student Teams

<p align="center">
  <strong>Plan Better. Collaborate Smarter. Build Together.</strong>
</p>

CampusForge Teams is a project management and collaboration platform for college students and development teams. Its deployed preview supports project boards, task assignment and tracking, comments, workspace invitations, and activity updates in one shared workspace.

This project is being developed as part of the **ALTA BUILDER 30 Challenge — Track A (Full-Stack Product)**.

> **Development status:** The React dashboard is connected to an authenticated Rust/Axum API for workspace loading, project and task management, column management, task assignments, comments, invitations, and persisted drag-and-drop ordering. Authentication uses Argon2 password hashing and 24-hour JWT sessions. Invitation tokens are random, stored as SHA-256 hashes, email-bound, single-use, and expire after seven days. Workspace owners/admins manage projects, columns, and invitations. Browser end-to-end and multi-session WebSocket acceptance tests remain.

---

## 🎯 Problem Statement

Student teams often manage projects through disconnected messaging applications, spreadsheets, and informal discussions. This can lead to unclear responsibilities, missed deadlines, poor visibility into progress, and difficulty coordinating work.

CampusForge Teams aims to bring project planning and collaboration into one structured workspace.

## 💡 Proposed Solution

CampusForge Teams is a Trello-inspired Kanban application. Team members can create projects, organize tasks into columns, assign work, discuss tasks, invite collaborators, and receive workspace-scoped live updates.

## ✨ Implemented Core Features

### 1. Authentication and User Profiles
- New accounts are created only through Google, GitHub, or Discord OAuth after the provider confirms the email address is verified.
- Existing email/password accounts can still sign in; password-only registration is disabled.
- Provider identities are validated server-side and are not silently merged with existing unrelated accounts.
- The account's provider-verified email timestamp is recorded in PostgreSQL.
- Authenticated and authorized API access, with user profiles and avatars.

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

### 6. Collaboration Suite
The collaboration suite in merged PR #12 includes:
- Team calendar with month, week, and day views, task deadlines, meetings, milestones, exams, and submissions.
- Advanced task tools: subtasks, checklists, labels, same-project dependencies, effort estimates, and recurring task rules.
- In-app notifications with read state, category preferences, and deadline reminders.
- Shared workspace/project/task files with authenticated upload, download, and delete.
- Server-backed productivity analytics for completion, overdue work, unassigned tasks, project progress, and member workload.
- Workspace/project chat and structured meeting notes with agenda and decisions.

### 7. Additional non-AI collaboration capabilities
The merged Track A non-AI implementation includes:
- Editable calendar events, event-type/project/attendee filters, timezone-aware all-day events, attendee invitations and responses, and one-time event reminders.
- Server-backed My Tasks filters for project, status, priority, effort range, blockers, assignee, and due-date range.
- Cursor-paginated notification and chat histories, workspace-invitation notifications, and mention notifications from task comments and shared channels.
- Workspace storage quotas and reservations, file content-signature checks, optional ClamAV scanning, authenticated PDF/image preview, file audit history, and admin quota controls.
- Daily productivity snapshots refreshed throughout the day, history filters, and CSV export.
- Private direct messages, message edit/delete controls, pinned team announcements, live presence and typing indicators, and recipient-scoped private-message events.
- Meeting-note action items with assignees, due dates, completion state, and optional task links.
- PostgreSQL LISTEN/NOTIFY to relay workspace change signals across API instances, plus expiring presence leases.
- An idempotency ledger for recurring task generation.

The non-AI collaboration implementation from PR #21 and subsequent improvements in PRs #23, #24, #26, and #30–#37 are merged into `main`. The hosted Render preview may lag this source branch and is a test environment, not a production release. The AI project assistant is intentionally excluded from Track A. Public release still requires provider OAuth configuration, durable Neon PostgreSQL and private object storage, mandatory ClamAV scanning, verified email delivery, backup/restore drills, and full multi-user acceptance. The Track A finalization branch also adds database-backed session revocation; that hardening has passed CI; independent review and runtime acceptance remain pending before merge. See the [roadmap](docs/ROADMAP.md), [architecture](docs/ARCHITECTURE.md), [operations](docs/OPERATIONS.md), and [social sign-in setup](docs/SOCIAL_AUTH_SETUP.md).

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

## 🏗️ Architecture

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
- `POST /api/auth/register` remains for compatibility but returns `403 Forbidden`; new accounts must use `GET /api/auth/google/start`, `/api/auth/github/start`, or `/api/auth/discord/start`, each of which requires a provider-verified email.
- `POST /api/auth/login` remains available for existing email/password accounts.
- `GET /api/workspace` returns the signed-in user's workspace, members, boards, lists, tasks, assignments, comments, and activity.
- `POST /api/tasks`, `PATCH /api/tasks/:id`, and `DELETE /api/tasks/:id` persist task changes.
- `POST /api/tasks/:id/comments` saves discussion comments; `PATCH /api/tasks/:id/move` persists drag-and-drop order.
- `POST /api/boards/:board_id/lists`, `PATCH /api/lists/:id`, and `DELETE /api/lists/:id` manage project columns.
- `GET/POST /api/invitations/`, `DELETE /api/invitations/:id`, and `POST /api/invitations/accept` manage workspace invitations.
- Browser sessions use an API-issued HttpOnly cookie; protected routes also retain Bearer authentication for non-browser clients. SQLx provides asynchronous access to PostgreSQL.
- The Vite development server proxies `/api` and `/ws` to the backend on port `5000`. For a separately hosted frontend, set `VITE_API_BASE_URL` to the API service's HTTPS origin. Invitation links use the frontend root query string (`?invite=...`) and are accepted after the invitee signs in.

## 📁 Repository Structure

Current application structure:

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
| `CAMPUSFORGE_STORAGE_MODE` | `local` for development or `s3` for a private S3-compatible bucket |
| `CAMPUSFORGE_UPLOAD_DIR` | Local-mode upload directory (default: `server/uploads`) |
| `CAMPUSFORGE_CLAMAV_ADDR` | Private TCP address of the ClamAV daemon |
| `CAMPUSFORGE_REQUIRE_MALWARE_SCAN` | Set to `true` to reject uploads/downloads unless a clean scan is recorded |
| `CAMPUSFORGE_S3_BUCKET` / `CAMPUSFORGE_S3_REGION` | Private bucket name and region when storage mode is `s3` |
| `CAMPUSFORGE_S3_ENDPOINT_URL` | Optional S3-compatible endpoint for R2/MinIO-style providers |
| `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` | Optional server-side credentials; use a dedicated S3 access key and secret in backend secrets |
| `RESEND_API_KEY` / `EMAIL_FROM` | Optional server-side email provider key and verified sender; leave blank to keep email delivery disabled |

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
- Provider-verified Google/GitHub/Discord sign-up and existing-account password sign-in; direct password registration is disabled. The API sends the JWT only in an HttpOnly cookie.
- Authenticated workspace WebSocket connections use the HttpOnly cookie during the handshake; JavaScript does not send the session token.
- Browser API requests include credentials; the server validates the frontend Origin on state-changing cookie-authenticated requests.

The active React application reads workspace, project, task, assignment, comment, and activity records from the authenticated Rust API backed by PostgreSQL. `client/src/data/demoData.js` remains as an unused legacy sample-data reference and is not the active application data source.

**Connected in the merged non-AI Track A implementation:** calendar editing, filters, attendee invitations and reminders; advanced task filters, subtasks, checklists, labels, dependencies and idempotent recurrence; paginated notifications and task/channel mentions; shared file preview, quotas, content checks, optional malware scanning and audit; daily analytics snapshots and CSV export; team chat, direct messages, pins, edits/deletes, presence, typing, meeting notes and structured action items. Cross-instance workspace change signals use PostgreSQL LISTEN/NOTIFY. PR #21 and subsequent non-AI additions are merged into `main`; the hosted preview may not yet run this source. **Still pending before public release:** configure and test Google/GitHub/Discord OAuth applications and callback URLs, provision durable Neon PostgreSQL and private object storage, configure ClamAV and require scans, verify the Resend sender domain, complete browser/two-user acceptance and backup/restore tests, approve retention/privacy policies, and complete fine-grained per-board/channel authorization.

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
- Authenticate API requests and authorize workspace membership on the server; fine-grained per-board permissions are not implemented yet.
- Validate all incoming request data.
- Use parameterized SQL queries through SQLx.
- Keep S3-compatible buckets private and stream file downloads only after workspace authorization.
- Store notification email jobs in PostgreSQL and send only for categories explicitly enabled by the member.
- Restrict CORS to the configured frontend origin.
- Authenticate WebSocket connections and scope broadcast events to the signed-in user's current workspace.
- Never commit `.env`, database credentials, tokens, or private user files.

## 📖 What I Built and What I Understood

Track A implements a complete full-stack collaboration workflow around React, Rust/Axum, PostgreSQL, and SQLx:

1. A responsive React application communicates with authenticated Axum REST and WebSocket endpoints.
2. PostgreSQL stores workspace, membership, board, task, collaboration, calendar, notification, file, analytics, chat, and activity data.
3. SQLx applies versioned, additive migrations and performs parameterized asynchronous database access.
4. Authentication supports provider-verified OAuth account creation, Argon2 verification for existing password accounts, HttpOnly browser sessions, and workspace authorization.
5. Kanban changes persist to PostgreSQL, with optimistic drag-and-drop rollback and real-time workspace updates.
6. Collaboration features include task discussions, calendar events, mentions, notifications, files, chat, direct messages, meeting action items, analytics, and an encrypted personal Save Locker.

The current delivery status, external integrations, known limitations, and final acceptance gates are maintained in [Track A Final Acceptance](docs/TRACK_A_FINAL_ACCEPTANCE.md).

## 🤖 AI Tools and Development Assistance

AI-assisted tools may be used to understand concepts, explore implementation approaches, debug issues, and improve documentation. AI-assisted code will be reviewed, tested, and understood before it is incorporated into the project. Specific tools and contributions will be documented accurately as development progresses.

## 🚧 Current Project Status

**Track:** ALTA BUILDER 30 — Track A (Full-Stack Product)  
**Status date:** 27 November 2026  
**AI scope:** Excluded from this delivery.

The planned non-AI product phases (core workspaces, authentication, calendar, advanced tasks, notifications, files, chat/meeting notes, analytics, and collaboration) are implemented in the repository. PR #20 (SQLx rollback compatibility), PR #21 (non-AI roadmap completion), and later UI, collaboration, storage, and authentication-hardening PRs #23–#37 are merged into `main`.

The latest recorded CI and security checks for `main` commit `2b0c5bf` passed on 24 November 2026: frontend production build and regression tests, Rust backend checks/tests, dependency vulnerability audit, and CodeQL. This does not replace browser-based, two-account, accessibility, backup/restore, or production-provider acceptance.

The current Track A finalization branch, `track-a/final-session-hardening-docs`, adds server-side authentication-session records and revocation:
- Login and OAuth create a random session ID recorded in PostgreSQL; the JWT contains only that ID, not a server-stored raw token.
- Protected API calls require an active, unrevoked session.
- Logout revokes the server-side session before clearing the HttpOnly cookie.
- WebSocket connections validate the session at handshake and re-check it on their heartbeat.
- Expired/revoked session rows are cleaned periodically.
- Migration `0014_revocable_auth_sessions.sql` and a PostgreSQL migration smoke-test assertion are included.

This finalization branch is not yet merged; its CI/review must pass before merge. **Do not merge to `main` until the release is explicitly authorized:** `render.yaml` currently configures both Render services to auto-deploy commits to `main`. No deployment is part of this Track A finalization.

**Public-release blockers remain:** create and verify Google/GitHub/Discord OAuth applications and production callback URLs; migrate from the temporary Render test database to durable Neon PostgreSQL and verify backups/restores; provision private Neon Object Storage and credentials; configure and test mandatory ClamAV scanning; verify Resend sender-domain authentication and delivery; configure trusted ingress/WAF rate limiting; complete two-user/browser and accessibility acceptance; approve privacy, retention, export, and deletion policies; and close the documented per-board/channel authorization and account-recovery gaps.

**Known product limitations:** password reset is not implemented; provider linking to an existing local account through account settings is not implemented; external Google/Outlook calendar synchronization is out of scope; per-board/channel permissions need a dedicated hardening pass; and the workspace navigation still needs verified multi-workspace switching before claiming that workflow as complete. See [Track A Final Acceptance](docs/TRACK_A_FINAL_ACCEPTANCE.md) for the release checklist.

## 🌐 Hosted Preview

- Frontend: https://campusforge-preview.onrender.com
- API health endpoint: https://campusforge-api-test.onrender.com/api/health
- API and frontend are separate Render services. The frontend is built with `VITE_API_BASE_URL`; the API CORS origin is set to the deployed frontend origin.

> This is a test preview hosted on Render's free service/database plans, not a production SLA or durable production database setup. The temporary test database expires on December 22, 2026.

## 🎓 Challenge Information

- **Challenge:** ALTA BUILDER 30
- **Track:** A — Full-Stack Product
- **Project:** Project Board (Trello-style)
- **Duration:** 15 Days

---

<p align="center">
  <strong>CampusForge Teams — Build Together. Achieve Together.</strong>
</p>

## Social sign-in

CampusForge creates new accounts only through server-side OAuth sign-in with Google, GitHub, or Discord using a provider-verified email. Existing email/password accounts can continue signing in. Configure provider client IDs/secrets and exact callback URLs on the backend only. See [Social Sign-in Setup](docs/SOCIAL_AUTH_SETUP.md).

OAuth provider secrets must remain on the Rust API server and must never be added to frontend variables or committed to Git. Social-only accounts are stored with a nullable password hash, while existing password accounts continue to use Argon2 password verification. New OAuth identities are accepted only when the provider returns a verified email address.

A CodeRabbit configuration is included in the repository root to guide pull-request reviews of OAuth handlers, auth code, migrations, and the sign-in UI. The CodeRabbit GitHub App must be installed and enabled for this repository before automated PR reviews can run.

## Product roadmap and architecture decisions

The product roadmap and the decisions for the target data model, API boundaries, security, and release order are documented here:

- [Product roadmap](docs/ROADMAP.md)
- [Architecture and project decisions](docs/ARCHITECTURE.md)
- [Storage and email operations](docs/OPERATIONS.md)

PR #20 (SQLx rollback compatibility) and PR #21 (verified-provider sign-up plus remaining non-AI features) are already merged, along with later hardening/UI/collaboration PRs #23–#37. The AI project assistant is intentionally excluded from Track A. The [Track A final acceptance checklist](docs/TRACK_A_FINAL_ACCEPTANCE.md) records the current source state, pending branch hardening, external integration gates, known limitations, and deployment hold.
