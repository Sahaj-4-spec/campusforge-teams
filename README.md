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
The current non-AI completion branch adds:
- Editable calendar events, event-type/project/attendee filters, timezone-aware all-day events, attendee invitations and responses, and one-time event reminders.
- Server-backed My Tasks filters for project, status, priority, effort range, blockers, assignee, and due-date range.
- Cursor-paginated notification and chat histories, workspace-invitation notifications, and mention notifications from task comments and shared channels.
- Workspace storage quotas and reservations, file content-signature checks, optional ClamAV scanning, authenticated PDF/image preview, file audit history, and admin quota controls.
- Daily productivity snapshots refreshed throughout the day, history filters, and CSV export.
- Private direct messages, message edit/delete controls, pinned team announcements, live presence and typing indicators, and recipient-scoped private-message events.
- Meeting-note action items with assignees, due dates, completion state, and optional task links.
- PostgreSQL LISTEN/NOTIFY to relay workspace change signals across API instances, plus expiring presence leases.
- An idempotency ledger for recurring task generation.

These features are merged into `main` through PR #41. The integrated Render preview uses a separate Neon child-branch database, a DML-only runtime role, and a private S3-compatible bucket; startup verified PUT/GET/DELETE access. Track B includes a read-only AI assistant, Project Pulse, and human-approved proposal review. AI and Project Pulse remain disabled by default until an administrator configures a compatible provider and approves privacy, retention, and spending controls. Production still requires its own API service, production-scoped storage credential, private malware scanner, OAuth/email provider credentials, and backup/restore acceptance. The public Node frontend host is being corrected in PR #47; do not treat the preview API or preview database as production. See the [roadmap](docs/ROADMAP.md), [architecture](docs/ARCHITECTURE.md), [operations](docs/OPERATIONS.md), and [social sign-in setup](docs/SOCIAL_AUTH_SETUP.md).

### 8. Optional AI assistant (Track B)

The integrated candidate includes a provider-neutral, workspace/board-scoped AI assistant for questions, summaries, task plans, risk reviews, and human-approved task proposals. Project Pulse is provider-independent and summarizes permitted project metrics; it has its own API/frontend feature flag and remains hidden and blocked by default until explicitly enabled. Every provider request requires an explicit data-sharing acknowledgement; provider access is restricted to bounded workspace context. Proposed task mutations are saved for review and revalidated at approval time; the model cannot autonomously write to the project. The assistant is disabled by default and must remain off until provider credentials, privacy/retention terms, and spending limits are approved. See [Track B implementation and safety guide](docs/track-b/README.md).

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
| `CAMPUSFORGE_AI_ENABLED` | Track B assistant kill switch; defaults to `false` |
| `CAMPUSFORGE_AI_API_BASE_URL` / `CAMPUSFORGE_AI_MODEL` | Server-only OpenAI-compatible chat-completions endpoint and model |
| `CAMPUSFORGE_AI_API_KEY` | Server-only provider credential; never use a `VITE_*` variable or commit a real key |

> For Neon, use the pooled or direct PostgreSQL connection string supplied by your Neon project and retain `sslmode=require`. Keep secrets in environment files or the hosting provider's secret store; never commit them.

Run the frontend and Rust backend concurrently from the repository root:

```bash
npm run dev
```

- Frontend: `http://localhost:5173`
- Backend health check: `http://localhost:5000/api/health`

Build the React frontend for static hosting:

```bash
npm run build
```

Build the Rust API separately:

```bash
npm run build:server
```

Run the Rust API in release mode (with the required server environment variables configured):

```bash
cargo run --release --manifest-path server/Cargo.toml
```

The Node-based Render frontend host serves only the files in `client/dist`; it does not start the API. For a local development session, use `npm run dev` to start Vite and the Rust API together. For a compiled API binary, build it with `npm run build:server` and run `./server/target/release/campusforge-server`.

## 🖥️ Frontend Milestone

The frontend milestone is connected to the authenticated Rust API.

- Overview dashboard with project and task summaries loaded from PostgreSQL.
- Project directory and Kanban boards, including creation of a project with four starter columns.
- Create, edit, and delete task flows with priority, due date, list, and assignee fields.
- Task comments and a recent activity timeline loaded from the API.
- Workspace team directory, task search, and filters.
- Responsive layout for desktop, tablet, and mobile screens.
- JWT-backed sign-in and registration; the API sends the session JWT only in an HttpOnly cookie, which browser JavaScript cannot read or persist.
- Authenticated workspace WebSocket connections use the HttpOnly cookie during the handshake; JavaScript does not send the session token.
- Browser API requests include credentials; the server validates the frontend Origin on state-changing cookie-authenticated requests.

The active React application reads workspace, project, task, assignment, comment, and activity records from the authenticated Rust API backed by PostgreSQL. `client/src/data/demoData.js` remains as an unused legacy sample-data reference and is not the active application data source.

**Included in the integrated collaboration candidate:** calendar editing, filters, attendee invitations and reminders; advanced task filters, subtasks, checklists, labels, dependencies and idempotent recurrence; paginated notifications and task/channel mentions; shared file preview, quotas, content checks, optional malware scanning and audit; daily analytics snapshots and CSV export; team chat, direct messages, pins, edits/deletes, presence, typing, meeting notes and structured action items. Cross-instance workspace change signals use PostgreSQL LISTEN/NOTIFY. The integrated candidate still requires fresh combined-branch checks and is not live on the hosted preview. **Still pending before public release:** configure and test Google/GitHub/Discord OAuth applications and callback URLs, provision a private S3-compatible bucket and credentials, configure ClamAV and require scans, verify the Resend sender domain, complete browser/two-user acceptance and backup/restore tests, approve retention/privacy policies, and finish two-account negative tests for the implemented per-board authorization rules.

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
- Authenticate API requests on the server. The integrated candidate enforces board membership and role checks across board-scoped reads and writes; workspace owners/admins retain elevated access and board viewers are read-only. Two-account negative authorization acceptance remains pending.
- Validate all incoming request data.
- Use parameterized SQL queries through SQLx.
- Keep S3-compatible buckets private and stream file downloads only after workspace authorization.
- Store notification email jobs in PostgreSQL and send only for categories explicitly enabled by the member.
- Restrict CORS to the configured frontend origin.
- Authenticate WebSocket connections and scope broadcast events to the signed-in user's current workspace.
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

**Status:** Track A and Track B are merged into `main` through PR #41; database runtime/migration separation is merged through PR #43. The integrated Render preview is deployed against the isolated Neon child branch and its restricted runtime role, with private object-storage I/O verified. The Node static frontend host and build scripts are present in the repository; PR #49 adds canonical-path/symlink protection and keeps API/WebSocket paths separate from SPA fallback, alongside Track B quota hardening. PR #49 is awaiting final CI/security validation and merge. The Render service settings and production topology still require inspection before a release. This preview is not a production release. Production OAuth credentials, email delivery, mandatory private malware scanning, a production API using the production DML-only database role and storage credential, recurring backups or a paid retention plan, populated-data restore verification, two-account acceptance and accessibility testing remain launch gates.

The React client authenticates against the Rust API and loads workspace records from PostgreSQL. Task creation, editing, deletion, assignments, comments, project creation, column management, invitation workflows, and drag-and-drop ordering call protected Axum endpoints; they no longer rely on browser-local demo records. New accounts receive a starter workspace and project board when the workspace endpoint is first loaded. Authenticated WebSocket sessions receive workspace-scoped events after committed mutations, and the frontend reloads the snapshot when remote changes arrive.

**Known release limitations:** browser end-to-end tests and a two-account WebSocket acceptance test have not been completed; invitation links are created for manual sharing and are not emailed; the workspace selector is not yet a functional workspace switcher; fine-grained per-board roles are enforced in the integrated candidate; two-account negative authorization acceptance remains pending. WebSocket events use an in-process broadcast channel, so run one API instance until shared pub/sub is added. The current Render database is a temporary free test database and expires on December 22, 2026; migrate to durable database hosting and configure backups before production use.

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

The integrated candidate adds Track B's optional AI assistant and human-approved proposal workflow without enabling provider calls by default. Track A's session hardening is migration 0015; Track B's durable proposal/audit schema is migration 0014 and already exists on Neon `campusforge_app`. The board-level authorization policy now filters project snapshots and analytics, denies writes to board viewers, restricts task/board notifications and email to authorized recipients, and validates task assignees against board membership. Do not merge or deploy until the combined CI, migration/session checks, privacy and operational acceptance gates pass and the release owner explicitly authorizes release.


## Track B — AI project assistant

Track B is included in the integrated review candidate in [draft PR #41](https://github.com/Sahaj-4-spec/campusforge-teams/pull/41), with source work preserved in [PR #39](https://github.com/Sahaj-4-spec/campusforge-teams/pull/39) and [PR #40](https://github.com/Sahaj-4-spec/campusforge-teams/pull/40). It includes workspace/board-scoped context, four response modes (ask, summary, task breakdown, risk review), source references, prompt-injection boundaries, bounded context, and shared rate limits. Task plans are stored as proposals; applying them requires explicit human approval and a fresh permission/resource-state check.

The API uses an OpenAI-compatible `/chat/completions` endpoint configured only on the Rust server. `CAMPUSFORGE_AI_ENABLED=false` keeps the feature disabled until provider terms, privacy controls, and production settings are approved. See [Track B implementation notes](docs/track-b/README.md) and the root `.env.example`. Conversation history is held in the current browser panel only; it is not persisted to PostgreSQL. Structured task proposals and metadata-only approval/rejection audit events are persisted; original prompts and raw model completions are not.


Production configuration and release steps: see [docs/PRODUCTION_SETUP.md](docs/PRODUCTION_SETUP.md) and [the final acceptance checklist](docs/TRACK_A_FINAL_ACCEPTANCE.md).


**Notification privacy:** task and project activity notifications are board-scoped. Only board members and workspace owners/admins can view the inbox item or receive its email; workspace invitation notifications use a separate invitation check. Migration 0016 adds the persisted board scope required for this enforcement.
