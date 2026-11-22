# CampusForge Teams — Product Roadmap

**Status:** Approved for planning; the five features below are not implemented yet.  
**Delivery rule:** Build and test locally first. Do not merge to main or deploy without explicit approval.

## Product direction

CampusForge Teams is intended to become a shared planning and collaboration workspace for student teams, project groups, hackathons, research teams, and campus organizations. The current authenticated Kanban workspace remains the foundation. The next roadmap expands it into a connected place to plan deadlines, coordinate work, share project material, and receive assistance.

## Approved feature priorities

The five features below were selected for the next development roadmap.

| Priority | Feature | Planned scope | Depends on |
|---|---|---|---|
| 1 | Advanced tasks and subtasks | Parent/child tasks, checklists, labels, dependencies, estimates, and recurring work | Existing task and assignment model |
| 2 | Team calendar and deadline planner | Calendar views, task deadlines, project milestones, meetings, and deadline-risk indicators | Existing due dates; calendar events |
| 3 | Smart notification centre | Persistent in-app alerts, unread counts, read state, preferences, and real-time delivery | Workspace membership and domain events |
| 4 | Shared files and documents | Workspace document library, task/board attachments, secure upload/download, and metadata | Workspace authorization and storage adapter |
| 5 | AI project assistant | Project planning, task breakdown, workload/deadline suggestions, and summaries | Permission-aware task, calendar, and file context |

The implementation order intentionally puts the advanced task model first. Calendar reminders, notifications, and AI suggestions depend on reliable task relationships, assignment data, and task events.

## Phase 0 — Foundation and quality gates

Before adding substantial UI, preserve the current application baseline and establish reliable tests around it.

- Keep the existing React + Vite client, Rust + Axum API, PostgreSQL database, SQLx data access, and native WebSocket transport.
- Add migration and API test coverage for the task, workspace, and membership invariants.
- Add browser end-to-end coverage for sign-in, board navigation, task create/edit/move, invitations, and two-user collaboration.
- Confirm every new endpoint checks authenticated workspace membership and validates IDs against the same workspace.
- Keep the database as the source of truth; WebSockets are a delivery mechanism, not a persistence layer.

**Exit criteria:** frontend production build, Rust compilation/tests, database migration tests, authorization tests, and core browser flows pass.

## Phase 1 — Advanced tasks and subtasks

### Scope
- Parent tasks and nested subtasks, with clear completion and progress roll-up.
- Ordered checklist items inside a task.
- Workspace-scoped labels and task-label assignment.
- Task dependencies, such as “blocked by” and “blocks”.
- Estimated effort in minutes or hours.
- Recurring task rules with time-zone-aware scheduling.
- Keep existing assignees, priorities, comments, task ordering, and due dates working.

### Key product decisions
- A subtask belongs to the same board and workspace as its parent.
- Prevent self-parenting, circular parent chains, and circular task dependencies.
- A parent task is not automatically marked complete merely because one child is complete. The UI should show derived subtask progress; any bulk-complete behavior must be explicit.
- Recurring work creates a new task occurrence from a rule; it does not silently reset the history of a completed task.
- Existing task IDs and current task workflows remain compatible.

### Acceptance criteria
- Users can create, edit, reorder, and complete subtasks and checklist items.
- Dependencies are visible and cannot form cycles.
- Labels can be filtered without exposing labels from another workspace.
- Recurrence respects the configured time zone and does not create duplicate occurrences after retries.
- Existing task CRUD, assignees, comments, and drag-and-drop regression tests still pass.

## Phase 2 — Team calendar and deadline planner

### Scope
- Calendar views for month, week, and agenda.
- Show task deadlines from the existing task records.
- Add project milestones and team meetings as separate calendar events.
- Filter by workspace, project/board, event type, and assignee where applicable.
- Highlight overdue tasks, deadlines approaching soon, unassigned work, and project milestones at risk.
- Provide links from a calendar item to its source task, board, or event.

### Key product decisions
- Do not duplicate a task deadline into a second event row. The task remains the source of truth for its due date.
- Store timed events as UTC timestamps and retain the event time zone for correct display.
- Represent all-day events distinctly from timed meetings.
- Calendar queries must be constrained to the authenticated workspace and requested date range.
- Deadline-risk indicators are advisory and explain the underlying reason (for example, overdue, unassigned, or dependency blocked).

### Acceptance criteria
- Calendar and agenda views agree on the same event/deadline data.
- Changing a task due date updates the calendar without creating duplicate events.
- All-day events remain on the intended local date across time zones.
- Users cannot view or change calendar events outside their workspace.
- The calendar remains usable on mobile screens.

## Phase 3 — Smart notification centre

### Scope
- Persistent in-app notification inbox with unread count.
- Notifications for assignment, mention, comment, invitation, dependency change, approaching deadline, overdue task, and meeting/milestone updates.
- Mark one notification or all notifications as read.
- Per-user preferences by notification category.
- Real-time notification delivery to connected users.
- Optional email delivery can be added after an email provider and delivery policies are approved.

### Key product decisions
- Store notifications in PostgreSQL before publishing a WebSocket event.
- WebSocket messages should contain a notification identifier and minimal update signal; the client fetches the authorized notification record.
- Generate recipients from workspace membership, task assignees, and explicitly mentioned members.
- Use idempotency/deduplication keys for repeated events and deadline reminder jobs.
- Do not send email until a delivery provider, opt-in settings, unsubscribe handling, and retry policy are implemented.

### Acceptance criteria
- Notifications survive refresh, reconnect, and device changes.
- Read state and preferences are per user.
- Members only see notifications addressed to them and belonging to workspaces they can access.
- Repeated event processing does not create duplicate alerts.
- A disconnected client receives the correct inbox state on its next fetch.

## Phase 4 — Shared files and documents

### Scope
- Workspace-level document library.
- Attach files to a board or task.
- Search and filter by file name, type, uploader, project, and upload date.
- Show file details, preview supported formats, and maintain clear download permissions.
- Add folder organization and file version history in a later increment if the initial library needs them.

### Key product decisions
- Store file metadata and ownership in PostgreSQL; store file bytes in an object-storage adapter, not in PostgreSQL rows.
- Do not select a storage vendor until cost, region, access control, retention, and backup requirements are compared.
- Use short-lived signed upload/download URLs or equivalent server-mediated transfers.
- Validate file size, allowed media types, ownership, workspace scope, and object keys on the server.
- Never treat a client-supplied storage key or URL as proof of authorization.

### Acceptance criteria
- A member can upload, list, attach, preview (where supported), download, and delete files within their permitted workspace scope.
- Files attached to tasks/boards cannot be retrieved by users outside that workspace.
- Upload failures, abandoned uploads, and deleted metadata do not leave unbounded orphaned objects.
- File-size limits, MIME checks, and safe content-disposition behavior are tested.
- Backups and retention rules cover metadata and stored objects.

## Phase 5 — AI project assistant

### Scope
- Generate a first-draft project plan from a user's brief.
- Break a selected task or deliverable into suggested subtasks.
- Suggest milestone dates and flag workload/dependency risks.
- Summarize a selected board, task discussion, or explicitly selected documents.
- Explain recommendations and allow a user to accept, edit, or reject each suggested change.

### Key product decisions
- Keep the provider integration behind a server-side AI adapter; no provider API key is exposed to the browser.
- Do not select an AI vendor/model until cost, privacy, data-region, retention, and quality requirements have been reviewed.
- The assistant is advisory. It must not create, assign, move, delete, or change project data without an explicit user action.
- Only send context that the signed-in user is already authorized to access. The user must be able to see which workspace, board, task, or files are included in a request.
- Treat comments, uploaded files, and other workspace content as untrusted input; never execute instructions embedded in that content.
- Add usage limits, cost visibility, error handling, audit metadata, and user controls to clear stored assistant history.
- Do not retain raw prompts, document contents, or model responses by default. Persistent conversation history requires a separate, explicit retention and deletion decision.

### Acceptance criteria
- Suggestions are returned as structured data that the user can review before applying.
- Applying a suggestion calls the existing authenticated task/calendar APIs and records normal activity history.
- Tests prove that the assistant cannot retrieve another workspace's data.
- Secrets remain server-side and usage/cost limits are enforced.
- The user can see what context is sent and can clear any intentionally stored assistant history.

## Cross-feature release requirements

- Authorization is enforced on the server for every record and every file/object operation.
- Add migration up/down or rollback guidance for every new schema change.
- Add unit, API integration, migration, and browser tests for each feature.
- Add accessibility checks for keyboard navigation, screen readers, focus states, and responsive layouts.
- Add observability for failed jobs, notification retries, file transfers, AI provider errors, and background scheduling.
- Document data retention, export, deletion, privacy, and backup behavior before public launch.

## Deployment gate

This roadmap update changes documentation only. No application runtime code, hosting configuration, environment secrets, or deployment services are being changed. Keep the feature branch unmerged and do not deploy the application unless the user explicitly authorizes it after implementation, review, and release testing.
