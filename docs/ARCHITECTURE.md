# CampusForge Teams — Architecture and Project Decisions

**Document status:** Approved design decisions and planned target architecture.  
**Implementation status:** The existing baseline is implemented to varying degrees; the five roadmap features below remain planned. This document is not evidence that those features already exist in the running application.

## 1. Product direction

CampusForge Teams is a collaborative project workspace for student teams. Its foundation is a shared workspace containing members, boards, task lists, tasks, assignments, comments, invitations, activity history, and real-time updates.

The next product increment adds:
1. Team calendar and deadline planner.
2. Advanced tasks and subtasks.
3. Smart notification centre.
4. Shared files and documents.
5. AI project assistant.

The approved delivery order is recorded in docs/ROADMAP.md. Advanced task relationships come first because calendar, notifications, and AI depend on reliable task and assignment data.

## 2. Current architecture decisions

| Area | Selected decision | Rationale / boundary |
|---|---|---|
| Frontend | React + Vite | Existing client and build workflow |
| UI styling | Tailwind CSS plus existing component styles | Keep the current responsive interface and avoid a second UI framework |
| Backend | Rust + Axum | Typed, async HTTP and WebSocket API |
| Database | PostgreSQL | Relational workspace, membership, task, and event data |
| Database access | SQLx | Parameterized queries, connection pooling, and migrations |
| Authentication | Argon2 password hashing and signed JWT sessions; Google, GitHub, Discord OAuth is being added on the feature branch | Provider secrets stay on the server |
| Real-time transport | Native WebSockets through Axum | No Socket.IO service or separate Node backend |
| Version control | GitHub | Branch-based work, pull requests, and CI checks |

Node.js/Express, MongoDB/Mongoose, and Socket.IO are not the selected backend stack. The current architecture remains React + Vite, Rust + Axum, PostgreSQL, SQLx, and native WebSockets.

## 3. Current relational model

The existing PostgreSQL baseline includes these main entities:

| Table | Responsibility |
|---|---|
| users | Account identity, email, name, avatar, and password hash (nullable for provider-only accounts after social-auth migration) |
| workspaces | Team workspace and owner |
| workspace_members | User membership and workspace role |
| boards | Projects within a workspace |
| board_members | Board-scoped membership/role records |
| lists | Ordered workflow columns for a board |
| tasks | Task title, description, priority, due timestamp, position, creator, and list |
| task_assignees | Many-to-many task assignment |
| comments | Task discussion |
| activity_events | Workspace/board/task activity history |
| invitations | Email-bound, expiring, single-use workspace invitations |
| oauth_accounts | External Google/GitHub/Discord identity linked to a CampusForge user |
| oauth_states | Hashed, short-lived OAuth state values consumed once by the callback |

The active API checks workspace membership for current operations. Fine-grained per-board authorization remains a known gap and must be designed before board-level private visibility is introduced.

## 4. Decisions for the five approved features

### ADR-005 — Advanced tasks and subtasks

**Decision:** Extend the existing task entity and add relational child tables; do not introduce a separate task database or replace the existing task API.

Planned schema additions (not yet migrated):

| Entity / field | Planned purpose |
|---|---|
| tasks.parent_task_id | Nullable self-reference to a parent task |
| tasks.estimated_minutes | Optional effort estimate |
| tasks.recurrence_rule_id | Optional link to a recurring-task rule |
| task_checklist_items | Ordered checklist items, completion timestamp, and completing user |
| labels | Workspace-scoped reusable labels |
| task_labels | Many-to-many link between tasks and labels |
| task_dependencies | Directed dependency edges between tasks |
| task_recurrence_rules | Recurrence rule, time zone, next occurrence, and template task |

Constraints and behavior:
- A parent and child must belong to the same board and workspace.
- Prevent self-parenting and cyclic parent/child relationships.
- Prevent dependency cycles and dependencies across inaccessible workspaces.
- Use transaction-safe ordering for checklist items.
- A parent task shows derived child completion progress; child completion does not silently complete the parent.
- Recurrence creates a new occurrence and preserves completed-task history. Retries must not create duplicate occurrences.
- Existing task IDs, assignees, comments, due dates, and drag-and-drop behavior remain compatible.

Potential API shape:
- GET/POST /api/tasks/:task_id/subtasks
- PATCH/DELETE /api/subtasks/:subtask_id
- GET/POST /api/tasks/:task_id/checklist
- PATCH/DELETE /api/checklist-items/:item_id
- GET/POST /api/workspace/labels
- POST/DELETE /api/tasks/:task_id/labels/:label_id
- GET/POST /api/tasks/:task_id/dependencies
- DELETE /api/task-dependencies/:dependency_id
- PUT /api/tasks/:task_id/recurrence

The exact route naming may be refined during implementation, but all routes must enforce workspace scope and cycle checks on the server.

### ADR-006 — Team calendar and deadline planner

**Decision:** Build one calendar view over existing task deadlines plus a separate calendar event entity. Do not copy task deadlines into duplicate calendar rows.

Planned schema:

**calendar_events**
- id UUID primary key
- workspace_id UUID foreign key
- board_id UUID nullable foreign key
- created_by UUID foreign key
- title and description
- event_type (meeting, milestone, or focus_block)
- start_at and end_at (UTC timestamps for timed events)
- time_zone (IANA identifier)
- all_day boolean
- created_at and updated_at

A future attendee relation may connect events to workspace members. The first implementation should keep event ownership and workspace visibility simple and explicit.

Task due dates continue to come from tasks.due_at. The calendar query combines task deadlines and calendar_events at read time. A migration may be considered later if the current timestamp-based task due date cannot preserve date-only semantics in all user time zones; do not duplicate deadlines as a workaround.

Potential API shape:
- GET /api/calendar?from=...&to=...&board_id=...
- GET/POST /api/calendar/events
- PATCH/DELETE /api/calendar/events/:event_id

Timed events are stored in UTC and displayed in the user's selected time zone. All-day events must retain their intended local calendar date. Every query is restricted to the authenticated workspace and requested range.

Deadline-risk radar is an explainable calendar/dashboard layer, not a separate source of task truth. Indicators may include overdue, due soon, unassigned, blocked by dependency, or milestone at risk.

### ADR-007 — Smart notification centre

**Decision:** Persist in-app notifications in PostgreSQL first, then use WebSockets for live delivery. Do not use WebSocket messages as the only copy of a notification.

Planned schema:

**notifications**
- id UUID primary key
- workspace_id UUID foreign key
- recipient_user_id UUID foreign key
- actor_user_id UUID nullable foreign key
- notification_type
- title and body
- resource_type and resource_id
- dedupe_key nullable unique
- read_at nullable timestamp
- created_at timestamp

**notification_preferences**
- user_id UUID foreign key
- notification_type
- in_app_enabled boolean
- email_enabled boolean (kept disabled until email delivery is implemented and approved)
- unique(user_id, notification_type)

Initial notification types include assignment, mention, comment, invitation, dependency change, deadline approaching, overdue task, and calendar event change.

Potential API shape:
- GET /api/notifications?cursor=...
- PATCH /api/notifications/:notification_id/read
- POST /api/notifications/read-all
- GET/PUT /api/notification-preferences

Notifications are created in the same database transaction as the domain change where practical. Background deadline reminders use idempotency keys and retry-safe jobs. WebSocket events contain a minimal notification ID/update signal; the client retrieves the notification through an authorized endpoint. Recipient resolution is based on task assignees, explicit mentions, and relevant workspace membership.

Email notifications are not part of the first implementation. They require an approved mail provider, opt-in settings, unsubscribe handling, retry policy, and delivery observability.

### ADR-008 — Shared files and documents

**Decision:** Keep file metadata and authorization in PostgreSQL, while storing file bytes in an object-storage adapter. Do not store uploaded binaries in PostgreSQL or trust client-provided object URLs.

Planned schema:

**workspace_files**
- id UUID primary key
- workspace_id UUID foreign key
- uploaded_by UUID foreign key
- original_name
- storage_key (opaque server-generated key)
- media_type
- size_bytes
- checksum_sha256
- created_at and deleted_at

**file_links**
- id UUID primary key
- workspace_file_id UUID foreign key
- board_id UUID nullable foreign key
- task_id UUID nullable foreign key
- created_at timestamp
- constraint: exactly one of board_id or task_id is present

The API must verify that file, board, task, and signed-in user all belong to the same authorized workspace. Use short-lived signed upload/download URLs or equivalent server-mediated transfers. Validate size, allowed media types, ownership, object keys, and safe content disposition. Plan cleanup for abandoned uploads and deleted metadata.

Potential API shape:
- GET /api/files
- POST /api/files/upload-requests
- GET /api/files/:file_id/download
- DELETE /api/files/:file_id
- POST/DELETE /api/files/:file_id/links

Storage vendor selection is deliberately deferred until region, cost, access control, backup, retention, and lifecycle requirements are compared. Folder organization and file version history are later increments, not requirements for the first upload release.

### ADR-009 — AI project assistant

**Decision:** Add a server-side provider adapter and keep AI output advisory. The assistant never changes project data without explicit user confirmation.

Initial capabilities:
- Draft a project plan from a user-provided brief.
- Suggest subtasks for a selected task or deliverable.
- Suggest milestone dates and highlight workload/dependency risks.
- Summarize a selected board, task discussion, or explicitly selected documents.

Potential API shape:
- POST /api/ai/project-plan
- POST /api/ai/task-breakdown
- POST /api/ai/summarize
- GET /api/ai/usage

Provider selection is deferred. Evaluate privacy, retention, data region, cost, latency, quality, and contract terms before choosing a model/provider. Provider credentials remain on the Rust server and are never sent to the browser.

The client must show which workspace, board, task, or files are in context before sending a request. The server independently verifies every referenced record. Workspace comments and uploaded documents are untrusted content; prompt-injection instructions found in them must not be followed. AI-generated output is structured as suggestions, shown for review, and applied through existing authenticated APIs only after a user action.

Default privacy decision: do not retain raw prompts, document contents, or model responses. Record minimal usage metadata (user, workspace, feature, provider/model, token/cost counters when available, status, and timestamp). Persistent AI conversation history requires an explicit retention, export, and deletion design before implementation.

Add quotas and cost guardrails. Tests must verify cross-workspace isolation, safe error behavior, provider timeouts, and that generated content cannot bypass normal authorization.

## 5. Shared API and real-time conventions

- REST endpoints are authenticated by the existing bearer-token mechanism unless explicitly declared public.
- Every endpoint validates workspace membership and validates relationships between every referenced record.
- Mutations use SQLx transactions where multiple tables or activity/notification records must remain consistent.
- PostgreSQL is the system of record.
- Native WebSockets notify connected workspace users after committed mutations; a reconnecting client fetches authoritative state from REST endpoints.
- WebSocket payloads should carry minimal identifiers and event names, not become an alternate permission bypass.
- Background jobs must be idempotent and observable. Deadline reminders and recurrence generation must be safe to retry.

## 6. Security, privacy, and operational decisions

- Use parameterized SQL through SQLx.
- Keep API/provider/storage secrets in server environment variables or a secret store.
- Enforce membership and resource ownership on the server, not only in the UI.
- Treat uploaded files and AI context as untrusted input.
- Use short-lived file transfer credentials and server-generated storage keys.
- Apply file type/size limits and safe download headers.
- Keep notification visibility scoped to the recipient and workspace.
- Define backup, retention, export, account deletion, and file deletion semantics before public launch.
- The current in-process WebSocket broadcast channel is a single-instance design; shared pub/sub is required before scaling the API horizontally.
- Fine-grained per-board authorization remains a known gap and must be resolved before private board visibility is introduced.

## 7. Delivery and deployment decision

The five roadmap features are planned, not implemented by this documentation update. New SQL migrations, routes, UI components, background workers, storage integrations, and AI provider dependencies must be added in their relevant implementation phases and tested separately.

This update is documentation-only:
- No runtime code is changed.
- No database migration is applied.
- No provider, storage, mail, or AI secrets are added.
- No hosting configuration is changed.
- Keep the feature branch and draft PR unmerged.
- Do not deploy until the user explicitly authorizes deployment after implementation, security review, and end-to-end release checks.
