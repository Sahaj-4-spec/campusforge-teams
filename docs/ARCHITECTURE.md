# CampusForge Teams — Architecture and Project Decisions

**Document status:** Selected technology decisions and current draft-branch implementation notes.  
**Release status:** The collaboration-suite implementation is on draft PR #12. It is not merged into the main branch or deployed. Some items remain local-development-only or incomplete.

## 1. Product direction

CampusForge Teams is a shared collaboration workspace for student teams. Its existing foundation contains accounts, workspaces, members, project boards, lists, tasks, assignees, comments, invitations, activity history, and workspace-scoped WebSocket updates.

The collaboration-suite branch adds:
1. Team calendar and deadline planner.
2. Advanced tasks and subtasks.
3. Smart notification centre.
4. Shared files and documents.
5. Team productivity analytics.
6. Team chat and meeting notes.

AI project assistance remains an approved future roadmap item. Delivery status and remaining work are recorded in docs/ROADMAP.md.

## 2. Current architecture decisions

| Area | Selected decision | Boundary |
|---|---|---|
| Frontend | React + Vite | Existing client and build workflow |
| UI styling | Tailwind CSS plus existing component styles | Keep current responsive design system |
| Backend | Rust + Axum | Typed asynchronous HTTP and WebSocket API |
| Database | PostgreSQL | System of record for workspace/product data |
| Database access | SQLx | Parameterized SQL, pooling, migrations |
| Authentication | Argon2 and signed JWT sessions; Google, GitHub, Discord OAuth on the social-auth feature branch | Provider secrets remain server-side |
| Real-time transport | Native Axum WebSockets | Not Socket.IO |
| File bytes | Private local filesystem directory for local development | Production object storage is not configured |
| Version control | GitHub | Branch-based work, draft PRs, CI |

Node.js/Express, MongoDB/Mongoose, and Socket.IO are not the selected backend architecture.

## 3. Current relational model

The baseline schema includes users, workspaces, workspace_members, boards, board_members, lists, tasks, task_assignees, comments, activity_events, invitations, oauth_accounts, and oauth_states.

The collaboration-suite migration adds:

| Table / columns | Responsibility |
|---|---|
| tasks.parent_task_id | Parent task/subtask relationship |
| tasks.estimated_minutes | Optional effort estimate |
| task_checklist_items | Ordered checklist and completion metadata |
| labels, task_labels | Workspace-scoped labels and task assignments |
| task_dependencies | Directed same-project dependency records |
| task_recurrence_rules | Recurrence interval, time zone, destination, and next run |
| calendar_events | Meetings, milestones, exams, submissions, and focus blocks |
| notifications | Recipient-scoped persistent in-app notifications |
| notification_preferences | Per-user notification category preferences |
| workspace_files | File metadata, scope, uploader, checksum and storage key |
| chat_messages | Workspace-wide or project-specific message history |
| meeting_notes | Meeting title, time, attendees, agenda, notes and action items |

The new SQL migration is server/migrations/0004_collaboration_suite.sql.

## 4. Feature architecture decisions

### ADR-005 — Advanced tasks and subtasks

**Decision:** Extend existing tasks rather than introducing a separate task database.

The draft branch supports:
- Parent task references and subtask creation.
- Ordered checklist items with completion state.
- Workspace labels and task-label assignments.
- Same-project dependency links with recursive cycle checks.
- Estimated effort stored in minutes.
- Recurrence rules processed by a background worker after the current occurrence is complete and due.

Important invariants:
- A subtask is created in the same list/board as its parent.
- A task cannot depend on itself or a task in another project.
- The API rejects dependency cycles.
- Subtask completion does not automatically complete its parent.
- Recurrence creates a new task occurrence rather than resetting the completed task.

Remaining validation: database-level hierarchy scope enforcement, recursive parent-cycle protection, concurrent recurrence idempotency, and browser regression coverage.

### ADR-006 — Team calendar and deadline planner

**Decision:** Read task deadlines directly from tasks.due_at; use a separate calendar event table for meetings and milestones. Do not duplicate task deadlines.

calendar_events stores:
- id, workspace_id, optional board_id, created_by.
- title, description, event_type.
- start_at and end_at as UTC timestamps.
- time_zone and all_day.
- created/updated timestamps.

Draft UI/API supports month, week, and day views; task deadline display; and creating/deleting meeting, milestone, exam, submission, and focus-block events.

Remaining: attendee links, full rescheduling/editor controls, time-zone/daylight-saving acceptance tests, richer filters, and reminder settings.

### ADR-007 — Smart notification centre

**Decision:** Notifications persist in PostgreSQL; WebSockets are only the delivery/update signal. A reconnecting client reloads authorized records from REST.

notifications stores workspace and recipient, optional actor, type/title/body, resource type/id, optional dedupe key, read timestamp, and creation timestamp.

notification_preferences stores in-app and email flags by user/category. Email stays disabled.

Draft implementation includes:
- Inbox and unread count.
- Mark-one/read-all behavior.
- Per-category in-app preferences.
- Task/comment activity notifications.
- Background reminders for assigned tasks due within 24 hours and tasks that have recently become overdue.

Remaining: mentions, invitation-specific recipients, pagination, delivery retry/idempotency tests, retention controls, and optional email integration.

### ADR-008 — Shared files and documents

**Decision:** Store metadata and authorization in PostgreSQL; do not store file bytes in database rows. File bytes currently go to a private local filesystem path for development.

workspace_files stores workspace, optional board/task attachment, uploader, original name, opaque server-generated storage key, media type, size, SHA-256 checksum, and timestamp.

Draft implementation includes an authenticated workspace file listing, upload, download, and uploader-only delete route, with a 10 MB request limit and safe download headers.

**Production blocker:** Local disk is not durable across server replacement or multiple instances. Before public use, replace it with private object storage, short-lived signed transfers or server-mediated transfers, malware scanning, file type/size policy, quotas, backups, retention/deletion, and orphan cleanup. A client-provided storage key/URL is never an authorization credential.

### ADR-009 — AI project assistant (future)

**Decision:** Keep AI integration behind a server-side provider adapter and keep responses advisory.

Potential functions include project-plan drafting, task breakdown, milestone suggestions, workload/dependency explanations, and summaries of explicitly selected resources. Do not select a provider until privacy, data region, retention, cost, and quality are reviewed. Show the user what context is sent; verify access server-side; treat uploaded files and comments as untrusted; require explicit user approval before applying any suggestion. Default to no raw prompt/document/response retention.

### ADR-010 — Team productivity analytics

**Decision:** Calculate live operational counts from PostgreSQL and be explicit about what each metric means. Do not present current totals as historical trends.

Draft API GET /api/analytics returns workspace totals for tasks, completed tasks, completion percentage, overdue tasks, unassigned tasks, project completion counts, and per-member open/completed assignments. The Reports/Analytics view displays these server-calculated values alongside existing activity/project reports.

Remaining: historical snapshots, date filters, export, metric definitions for reopened/cancelled tasks, and privacy review before member ranking or contribution scoring.

### ADR-011 — Team chat and meeting notes

**Decision:** Keep workspace/project message history and meeting notes as separate PostgreSQL resources scoped to the same workspace.

- chat_messages supports workspace-wide or board-specific channels with author and timestamp.
- meeting_notes supports title, meeting time, attendees, agenda, discussion notes, and action items.

Draft UI/API supports sending/listing messages and creating/listing/updating/deleting meeting notes (delete is limited to the creator). This is not yet a full messenger: direct messages, typing/presence, pins, and mention parsing are not included. Real-time message fan-out and two-browser acceptance tests remain.

## 5. API and real-time conventions

REST routes require a valid bearer session unless explicitly public. Every resource lookup validates the authenticated user's workspace membership and the relationship between referenced IDs.

Draft route groups include:
- /api/calendar and /api/calendar/events/:id
- /api/tasks/:id/advanced, /subtasks, /checklist, /labels, /dependencies, /estimate, and /recurrence
- /api/notifications and /notification-preferences
- /api/files and /api/files/:id/download
- /api/chat/messages
- /api/meeting-notes
- /api/analytics

PostgreSQL is the source of truth. Mutations use SQLx; background jobs must be observable and retry-safe. WebSocket payloads should contain minimal update signals; the client reloads authorized data from REST. The current broadcast channel is in-process and requires shared pub/sub before horizontal scaling.

## 6. Security, privacy, and operations

- Use parameterized SQL through SQLx.
- Keep OAuth, storage, and future AI secrets on the server.
- Check workspace membership and referenced resource relationships on every request.
- Keep files private; use server-generated opaque storage keys and workspace-scoped downloads.
- Limit file sizes and return safe content-disposition and nosniff headers.
- Do not enable email delivery without opt-in, unsubscribe, retry, and provider configuration.
- Do not send unauthorized workspace data to future AI services.
- Define backups, retention, export, and deletion before public launch.
- Fine-grained per-board authorization and shared WebSocket pub/sub are known scale/security gaps.

## 7. Deployment decision

PR #12 is a draft feature branch. No new collaboration-suite feature is in the live preview until it is merged and deployed by explicit authorization. Before that, the branch needs completed CI/security checks, database migration review, browser/multi-user testing, storage vendor configuration, production retention/backup design, and security review.

Do not merge or deploy until the user explicitly authorizes release.
