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
| File bytes | Storage adapter with local-development and S3-compatible backends | Production S3 bucket, credentials/IAM role, backups and malware scanning still require setup |
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
- Parent task references and subtask creation, with a database trigger enforcing same-project parent/child scope and rejecting hierarchy cycles.
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
- Per-category in-app and email preferences.
- Task/comment activity notifications.
- Background reminders for assigned tasks due within 24 hours and tasks that have recently become overdue.
- A PostgreSQL email outbox, optional Resend API sender, notification-specific idempotency key, and retry/backoff. The sender is inactive until RESEND_API_KEY and EMAIL_FROM are configured.

Remaining: verify the sender domain and provider account, test actual delivery and bounce handling, add mentions, invitation-specific recipients, pagination, retention controls, and unsubscribe/suppression policy.

### ADR-008 — Shared files and documents

**Decision:** Store metadata and authorization in PostgreSQL; do not store file bytes in database rows. File bytes currently go to a private local filesystem path for development.

workspace_files stores workspace, optional board/task attachment, uploader, original name, opaque server-generated storage key, media type, size, SHA-256 checksum, and timestamp.

Draft implementation includes an authenticated workspace file listing, upload, download, and uploader-only delete route, with a 10 MB request limit, extension allowlist, normalized content types, and safe download headers. A storage adapter supports local-development disk and an S3-compatible backend using AWS Signature Version 4 over Reqwest. Transfers are server-mediated; the bucket remains private and the API checks authorization before reading/deleting an object.

**Production blocker:** The S3 adapter is code-complete but no private bucket/credentials are configured. Before public use, provision and verify the chosen private bucket, IAM policy, encryption, backups/versioning, retention/deletion, malware scanning, quotas, object/database reconciliation, and restore procedures. A client-provided storage key/URL is never an authorization credential.

### ADR-009 — AI project assistant

**Decision:** Explicitly excluded from the current delivery.

No AI assistant, AI provider integration, AI-generated tasks, AI-generated summaries or model-driven project actions are being implemented in this release. Revisit only as a separately approved product scope and architecture decision.

### ADR-010 — Team productivity analytics

**Decision:** Keep operational metrics server-calculated and store explicit daily snapshots for trend reporting.

- `GET /api/analytics` returns workspace-scoped current metrics.
- `GET /api/analytics/history` returns daily snapshots for a bounded date range.
- `GET /api/analytics/export.csv` exports the selected history range.
- A background job upserts the current day's metrics during the day so the snapshot is refreshed as work changes.
- The UI exposes date filters, a completion trend and CSV export.

Metric definitions for reopened/cancelled/archived tasks and fairness/privacy review remain required before individual rankings are introduced. No individual leaderboard is part of this scope.

### ADR-011 — Team chat and meeting notes

**Decision:** Store shared channels, private conversations, meeting notes and action items in PostgreSQL. Keep direct-message reads and change events private to the two participants.

- `chat_messages` stores workspace/project channel messages and direct-conversation messages.
- Direct conversations use a normalized participant pair and enforce access on every REST read/write.
- Shared channels support mentions, pinning, edits, soft deletes and workspace-admin moderation.
- Direct-message events carry a recipient user ID and are delivered only to that participant; they are not broadcast to the workspace.
- Meeting notes have a separate `meeting_note_action_items` table with assignee, due date, completion state and optional task link.
- Cursor pagination is used for both team and direct message history.

Workspace presence and typing use expiring per-connection leases. PostgreSQL LISTEN/NOTIFY forwards committed workspace change signals to other API instances; WebSocket events remain small update signals, and the client reloads authorized data from REST.

## 5. API and real-time conventions

REST routes require a valid bearer session unless explicitly public. Every resource lookup validates the authenticated user's workspace membership and the relationship between referenced IDs.

Implemented route groups include:
- Calendar events, attendee management, attendee responses and all-day date ranges.
- Advanced task details, subtasks, checklists, labels, dependencies, estimates, recurrence and filtered task search.
- Notification inbox, cursor pagination, read state and category preferences.
- Workspace files, upload/download/delete, previews, storage usage/quota, audit history and scan gating.
- Team chat, direct conversations, cursor-paginated messages, pins, mentions and message moderation.
- Meeting notes and structured action items.
- Current analytics, historical snapshots, and CSV export.
- Workspace presence lookup and authenticated WebSocket sessions.

PostgreSQL is the source of truth. Mutations use SQLx; recurrence, reminder, analytics and file-cleanup jobs use database-backed claiming or idempotency records. PostgreSQL LISTEN/NOTIFY relays workspace change events across API instances. Direct-message events include a recipient and are filtered before delivery.

## 6. Security, privacy, and operations

- Use parameterized SQL through SQLx.
- Keep OAuth, storage, and future AI secrets on the server.
- Check workspace membership and referenced resource relationships on every request.
- Keep S3-compatible buckets private; use server-generated opaque storage keys and workspace-scoped, server-mediated downloads.
- Keep Resend credentials and sender configuration server-side; only create email jobs for categories a member explicitly enabled.
- Limit file sizes and return safe content-disposition and nosniff headers.
- Do not enable email delivery without opt-in, unsubscribe, retry, and provider configuration.
- Do not send unauthorized workspace data to future AI services.
- Define backups, retention, export, and deletion before public launch.
- Fine-grained per-board authorization and shared WebSocket pub/sub are known scale/security gaps.

## 7. Deployment decision

PR #12's collaboration suite is merged into main. Provider-verified sign-up and the remaining non-AI enhancements are in draft PR #21, which stacks the PR #19 changes. PR #20 is the SQLx rollback-compatibility prerequisite.

Release order: deploy PR #20 first, then PR #21. After PR #21, configure provider OAuth applications, private S3-compatible storage, ClamAV (with scan enforcement), Resend sender-domain credentials, backups, retention and privacy controls. Complete browser/two-user acceptance tests before public release. The existing Render preview has not been updated by these changes.

Operational configuration instructions are in docs/OPERATIONS.md. No merge or deployment should happen until the user explicitly authorizes release.

### ADR-012 — Calendar attendees, time zones and reminders

**Decision:** Store all-day start/end dates independently from instants and retain the event's IANA time-zone identifier. Store attendees as workspace-member foreign keys with explicit invited/accepted/declined responses. A reminder is one-shot per attendee and is deduplicated in the notification store.

### ADR-013 — Shared file governance and scanning

**Decision:** Keep file bytes in a private storage adapter and metadata, quotas, reservations and audit events in PostgreSQL. Check file signatures before storage. ClamAV scanning is optional for local development, but public deployments must set `CAMPUSFORGE_REQUIRE_MALWARE_SCAN=true` and configure a reachable ClamAV service. Preview routes are workspace-authorized, no-store and restricted to PDF and raster images.

### ADR-014 — Idempotent recurring task generation

**Decision:** Every recurrence run records `(recurrence_rule_id, scheduled_for, generated_task_id)` in `task_recurrence_runs`. Task creation, assignment copy, ledger insert and rule advancement commit in one transaction. The unique key prevents duplicate generation for the same scheduled occurrence.

### ADR-015 — Recipient-scoped private real-time events

**Decision:** Shared workspace changes may be relayed to workspace members, but direct-message signals carry a recipient user ID and are filtered at the WebSocket connection. Direct-message body content is never put in workspace broadcast payloads.

