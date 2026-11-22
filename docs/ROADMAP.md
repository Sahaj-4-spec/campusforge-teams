# CampusForge Teams — Product Roadmap

**Status:** Six collaboration areas have implementation work on draft PR #12. They are not merged into the main branch or deployed. AI project assistance remains an approved future capability.  
**Release rule:** Work on feature branches, test locally, review security, and obtain explicit approval before merging or deploying.

## Product direction

CampusForge Teams is a shared workspace for student teams. The existing foundation includes authentication, project boards, task assignment, comments, invitations, activity history, and workspace-scoped real-time updates.

The collaboration-suite branch adds calendar planning, advanced task controls, notifications, files, team analytics, chat, and meeting notes. The delivery status below reflects what is in the draft branch, not a production release.

## Feature delivery status

| Area | Draft-branch implementation | Remaining before production |
|---|---|---|
| Team calendar and deadline planner | Month/week/day views, task deadlines, manual meeting/milestone/exam/submission events | Event attendees, edit/reschedule, stronger date-only/time-zone handling and browser tests |
| Advanced task management | Estimates, subtasks, checklists, labels, same-project dependencies, recurring rules and scheduled occurrence processing | Parent/child cycle constraints, dependency and recurrence concurrency tests, full filtering and subtask ordering |
| Smart notification centre | Persistent inbox, read state, category preferences, task/comment notifications, due-soon and overdue reminders | Mentions, invitation-specific recipients, pagination, email delivery, retention and multi-session tests |
| Shared files and documents | Workspace library, project/task attachment, authenticated upload/list/download/delete, 10 MB limit | Durable private object storage, malware scanning, backups, retention, preview, lifecycle cleanup and production hardening |
| Team productivity analytics | Server-side completion, overdue and unassigned counts, project progress and member workload | Historical trends, date filters, export, metric verification and privacy review |
| Team chat and meeting notes | Workspace/project message history and saved notes with attendees, agenda, discussion notes and action items | Direct messages, mentions, presence/typing, chat real-time delivery, edit/delete policy and browser acceptance |
| AI project assistant | Approved future roadmap item; not implemented in this branch | Provider review, server adapter, permission-aware context, suggestions-only workflow, quotas and AI safety tests |

## Phase 0 — Release foundations

- Keep React + Vite, Rust + Axum, PostgreSQL, SQLx, and native WebSockets.
- Add migration, API authorization, database relationship, and browser tests.
- Test two distinct workspace members in separate browser sessions.
- Make background processing retry-safe and observable.
- Confirm local development does not require production storage or email credentials.

**Exit criteria:** frontend build, Rust compile/tests, migration tests, API integration tests, authorization checks, accessibility checks, and core browser flows pass.

## Phase 1 — Advanced tasks and subtasks

### Draft-branch scope
- Parent tasks and subtasks.
- Ordered checklist items with per-item completion.
- Workspace-scoped labels.
- Same-project task dependencies with cycle checks.
- Estimated effort in minutes.
- Recurrence rules processed by a background worker after the current occurrence is completed and the rule is due.

### Remaining work
- Add database-level or transaction-protected invariants for parent/child project scope.
- Add recursive cycle checks for task hierarchy.
- Make recurrence creation fully idempotent under concurrent workers.
- Improve status transitions without silently completing a parent when a child completes.
- Add filtering by labels, dependency state, estimate, and parent/child structure.

## Phase 2 — Team calendar and deadline planner

### Draft-branch scope
- Month, week, and day views.
- Task deadlines read from existing task records, not duplicated.
- Workspace/project events for meetings, milestones, exams, submissions, and focus blocks.
- Create/delete events and query by date range.

### Remaining work
- Event attendees and invitations.
- Full edit/reschedule controls and reminders.
- Preserve all-day event dates across time zones.
- Test daylight-saving and local-time transitions.
- Add filters by project, type, and member.

## Phase 3 — Smart notification centre

### Draft-branch scope
- Persistent in-app inbox, unread count, mark-one/read-all.
- Per-user in-app category preferences.
- Task/comment activity notifications.
- Scheduled reminders for assigned tasks due within 24 hours and tasks newly overdue.

### Remaining work
- Mention parsing and targeted recipients.
- Invitation-specific notifications.
- Cursor pagination and retention controls.
- Email delivery is disabled until a provider, opt-in, unsubscribe, retry, and bounce policy are approved.
- Add idempotency and background-job concurrency tests.

## Phase 4 — Shared files and documents

### Draft-branch scope
- Workspace file listing and authenticated upload/download.
- Optional project or task attachment.
- Uploader-only delete.
- 10 MB limit, opaque server-generated storage key, SHA-256 checksum, safe content-disposition, and nosniff response header.

### Local-only storage boundary
The current branch stores file bytes in a private local server directory, configurable through CAMPUSFORGE_UPLOAD_DIR and defaulting to server/uploads. This is for local development only. It is not durable across host replacement or multiple API instances and is not suitable for production.

### Required production work
- Choose private object storage after reviewing region, cost, durability, access controls, retention, backup, and deletion requirements.
- Use short-lived signed transfer URLs or equivalent server-mediated transfers.
- Add malware scanning, file-type allowlists, quotas, abandoned-upload cleanup, audit history, and backup/restore tests.
- Never expose raw storage keys or treat client-provided URLs as authorization.

## Phase 5 — Team chat and meeting notes

### Draft-branch scope
- Workspace-wide and project-specific message channels.
- Persisted message history.
- Meeting notes with title, date/time, project, attendees, agenda, discussion notes, and action items.

### Remaining work
- Real-time chat delivery and pagination.
- Direct messages, mentions, presence/typing, pinned announcements, and message edit/delete policy.
- Meeting-note permissions and collaborative editing.
- Link action items to tasks and dates.
- Add accessibility and two-user live acceptance tests.

## Phase 6 — Team productivity analytics

### Draft-branch scope
- Server-calculated total, completed, overdue, and unassigned task counts.
- Per-project completion statistics.
- Per-member open and completed assignment counts.
- Analytics panel displays actual API metrics.

### Remaining work
- Historical snapshots for trend charts rather than inferring history from current task state.
- Date-range filters and export.
- Metric definitions for reopened/cancelled tasks and shared assignments.
- Review privacy and fairness before individual rankings or contribution scores.

## Phase 7 — AI project assistant

### Approved future scope
- Draft a project plan from a user brief.
- Suggest subtasks and milestone dates.
- Explain workload and dependency risks.
- Summarize explicitly selected boards, discussions, or documents.

### Decisions
- Keep provider credentials and the AI adapter server-side.
- Select a provider only after privacy, retention, region, cost, latency, and quality review.
- AI output is advisory and cannot mutate project data without explicit user approval.
- Show which resources are included; verify access server-side.
- Treat comments and uploaded files as untrusted content.
- Default to no raw prompt/document/response retention.

## Cross-feature release requirements

- Enforce workspace/resource authorization on every request and file operation.
- Add migration and rollback guidance for every schema change.
- Add unit, API integration, migration, browser, accessibility, and multi-user acceptance tests.
- Add observability for background jobs, notification retries, file transfer failures, and WebSocket disconnects.
- Define export, deletion, privacy, retention, and backup behavior before public launch.
- Use shared pub/sub before running more than one API instance.

## Deployment gate

PR #12 is a draft feature branch. Its work is not in the main branch and has not been deployed. Do not merge or deploy until implementation, tests, security review, object-storage decisions, provider configuration, and production-readiness checks are complete and the user explicitly authorizes release.
