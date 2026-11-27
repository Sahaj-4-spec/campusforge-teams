# CampusForge Teams — Architecture and Project Decisions

**Document status:** Architecture decisions for the merged Track A non-AI implementation plus the finalization branch.  
**Status date:** 28 November 2026.  
**Release status:** `release/campusforge-integrated-candidate` consolidates the current `main` source with Track A session hardening and Track B AI assistant/proposal work. It is a review candidate, not `main`; PRs #38–#40 remain separate draft PRs. The combined branch must pass fresh CI and runtime migration/session checks before review. Render currently hosts preview/test services; the API's latest environment update started a rollout that was still reported `update_in_progress` at the last check. No production deployment or merge is authorized.

## 1. Product direction

CampusForge Teams is a shared collaboration workspace for student teams. Its existing foundation contains accounts, workspaces, members, project boards, lists, tasks, assignees, comments, invitations, activity history, and workspace-scoped WebSocket updates.

The merged Track A implementation includes:
1. Team calendar and deadline planner.
2. Advanced tasks and subtasks.
3. Smart notification centre.
4. Shared files and documents.
5. Team productivity analytics.
6. Team chat and meeting notes.

AI project assistance is a separately scoped, optional Track B capability. The integrated candidate includes a feature-flagged read-only assistant, Project Pulse insights, and human-approved task proposals. The feature remains disabled until an administrator configures a supported provider and completes privacy, retention, and cost reviews. Delivery status and acceptance gates are recorded in docs/ROADMAP.md, docs/track-b/README.md, and docs/TRACK_A_FINAL_ACCEPTANCE.md.

## 2. Current architecture decisions

| Area | Selected decision | Boundary |
|---|---|---|
| Frontend | React + Vite | Existing client and build workflow |
| UI styling | Tailwind CSS plus existing component styles | Keep current responsive design system |
| Backend | Rust + Axum | Typed asynchronous HTTP and WebSocket API |
| Database | PostgreSQL | System of record for workspace/product data |
| Database access | SQLx | Parameterized SQL, pooling, migrations |
| Authentication | Argon2 verification for existing password accounts; provider-verified Google, GitHub, and Discord sign-up; HttpOnly JWT cookies | Provider secrets remain server-side; Track A finalization adds PostgreSQL-backed session revocation |
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

The schema is applied through ordered SQLx migrations in `server/migrations/`. The Neon `campusforge_app` database has successful journal entries 0001–0014; 0014 is the durable AI proposal/audit schema. The integrated candidate adds 0015 for revocable authentication sessions. Never edit an applied migration; add a new migration instead.

## 4. Feature architecture decisions

### ADR-005 — Advanced tasks and subtasks

**Decision:** Extend existing tasks rather than introducing a separate task database.

The merged Track A implementation supports:
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

Database triggers enforce same-project hierarchy scope and cycle rules; recurrence generation uses a PostgreSQL idempotency ledger. Release acceptance still needs multi-worker/concurrent retry tests with realistic PostgreSQL data and browser regression coverage.

### ADR-006 — Team calendar and deadline planner

**Decision:** Read task deadlines directly from tasks.due_at; use a separate calendar event table for meetings and milestones. Do not duplicate task deadlines.

calendar_events stores:
- id, workspace_id, optional board_id, created_by.
- title, description, event_type.
- start_at and end_at as UTC timestamps.
- time_zone and all_day.
- created/updated timestamps.

The merged UI/API supports month, week, and day views; task deadlines; event create/edit/reschedule/delete; type/project/attendee filters; all-day date/time-zone handling; attendee invitations/responses; and deduplicated reminders.

Remaining acceptance: two-user attendee invitation/reminder tests and timezone/daylight-saving boundary tests for supported regions. External Google/Outlook calendar synchronization is outside Track A.

### ADR-007 — Smart notification centre

**Decision:** Notifications persist in PostgreSQL; WebSockets are only the delivery/update signal. A reconnecting client reloads authorized records from REST.

notifications stores workspace and recipient, optional actor, type/title/body, resource type/id, optional dedupe key, read timestamp, and creation timestamp.

notification_preferences stores in-app and email flags by user/category. Email stays disabled.

The merged implementation includes:
- Inbox and unread count.
- Mark-one/read-all behavior.
- Per-category in-app and email preferences.
- Task/comment activity notifications.
- Background reminders for assigned tasks due within 24 hours and tasks that have recently become overdue.
- A PostgreSQL email outbox, optional Resend API sender, notification-specific idempotency key, and retry/backoff. The sender is inactive until RESEND_API_KEY and EMAIL_FROM are configured.

Remaining external setup: verify the Resend account and sender domain, configure SPF/DKIM/DMARC, test delivery/bounce handling, and approve suppression, unsubscribe, privacy, and retention policies. Mentions, invitation notifications, and cursor pagination are implemented.

### ADR-008 — Shared files and documents

**Decision:** Store metadata and authorization in PostgreSQL; do not store file bytes in database rows. File bytes currently go to a private local filesystem path for development.

workspace_files stores workspace, optional board/task attachment, uploader, original name, opaque server-generated storage key, media type, size, SHA-256 checksum, and timestamp.

The merged implementation includes authenticated workspace/project/task file listing, upload, download, preview, and deletion, with a 10 MB request limit, extension allowlist, signature checks, safe headers, quotas/reservations, audit events, and durable object-deletion cleanup. A storage adapter supports local-development disk and an S3-compatible backend using AWS Signature Version 4 over Reqwest. Transfers are server-mediated; the bucket remains private and the API checks authorization before reading/deleting an object.

**Production blocker:** The S3 adapter is code-complete but no private bucket/credentials are configured. Before public use, provision and verify the chosen private bucket, IAM policy, encryption, backups/versioning, retention/deletion, malware scanning, quotas, object/database reconciliation, and restore procedures. A client-provided storage key/URL is never an authorization credential.

### ADR-009 — AI project assistant

**Decision:** Excluded from the Track A baseline; separately implemented behind Track B feature controls.

AI assistant and model-driven project support are excluded from the Track A baseline. Track B is separately scoped and optional; the current integrated candidate adds a feature-flagged assistant and human-approved proposal flow, with no autonomous mutations.

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

Protected REST routes require a valid bearer token or HttpOnly cookie session unless explicitly public. Requests validate the authenticated user's workspace membership and the relationship between referenced IDs; finalization branch adds an active server-side session record check.

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
- Fine-grained per-board/channel authorization is a known permission-hardening gap. PostgreSQL LISTEN/NOTIFY is implemented for cross-instance workspace update signals; presence, typing, and other ephemeral connection state still require operational multi-instance verification.

## 7. Deployment decision

The non-AI Track A implementation is merged into `main`. PR #20's SQLx rollback bridge was merged before PR #21 introduced migrations 0009 and 0010; subsequent migrations are also committed on main. PR #37 adds shared PostgreSQL-backed authentication request limits.

The repository's Render Blueprint tracks `main` and uses commit-triggered auto-deploy. Therefore, do not merge this finalization branch to `main` or alter production infrastructure until the user explicitly authorizes release. The live preview is a test environment; verify its deployed source revision and database migration state before any rollout. A migration file in source control does not prove that a hosted database has applied it.

Before public release, configure provider OAuth applications and callbacks, durable Neon PostgreSQL, private Neon Object Storage, ClamAV with scan enforcement, Resend sender-domain credentials, trusted ingress/WAF controls, backups and restore procedures, privacy/retention policies, and two-account/browser/accessibility acceptance. See [Track A Final Acceptance](TRACK_A_FINAL_ACCEPTANCE.md) and [Operations](OPERATIONS.md).

### ADR-012 — Calendar attendees, time zones and reminders

**Decision:** Store all-day start/end dates independently from instants and retain the event's IANA time-zone identifier. Store attendees as workspace-member foreign keys with explicit invited/accepted/declined responses. A reminder is one-shot per attendee and is deduplicated in the notification store.

### ADR-013 — Shared file governance and scanning

**Decision:** Keep file bytes in a private storage adapter and metadata, quotas, reservations and audit events in PostgreSQL. Check file signatures before storage. ClamAV scanning is optional for local development, but public deployments must set `CAMPUSFORGE_REQUIRE_MALWARE_SCAN=true` and configure a reachable ClamAV service. Preview routes are workspace-authorized, no-store and restricted to PDF and raster images.

### ADR-014 — Idempotent recurring task generation

**Decision:** Every recurrence run records `(recurrence_rule_id, scheduled_for, generated_task_id)` in `task_recurrence_runs`. Task creation, assignment copy, ledger insert and rule advancement commit in one transaction. The unique key prevents duplicate generation for the same scheduled occurrence.

### ADR-015 — Recipient-scoped private real-time events

**Decision:** Shared workspace changes may be relayed to workspace members, but direct-message signals carry a recipient user ID and are filtered at the WebSocket connection. Direct-message body content is never put in workspace broadcast payloads.



### ADR-016 — Revocable server-side authentication sessions

**Decision:** Keep browser authentication in an HttpOnly JWT cookie, but make each JWT reference a database-recorded session ID through its `jti` claim. Never store the raw JWT in PostgreSQL.

- The finalization branch adds `auth_sessions` with a UUID session ID, owning user, expiry, and optional revocation timestamp.
- Password and OAuth sign-in persist a session row before returning the session cookie.
- Protected HTTP requests require a matching session row that is not revoked and has not expired.
- Logout revokes the corresponding row before clearing the cookie.
- WebSocket handshakes validate the row, and active connections re-check it during their 30-second heartbeat.
- A background cleanup removes expired/revoked rows after a seven-day buffer.
- JWTs issued before this migration have no `jti` and must be renewed by signing in again after rollout.

Migration 0015 and its application changes are in `release/campusforge-integrated-candidate`; it is not yet merged into `main` or live on the hosted service. The feature-flagged Track B assistant remains disabled until provider and privacy controls are approved.
