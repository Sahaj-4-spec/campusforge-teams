# CampusForge Teams — Product Roadmap

## Product objective

CampusForge Teams is a project and collaboration workspace for student teams and small delivery groups. The product combines project boards, task planning, coordination, shared resources, calendar workflows, team communications, and measurable project progress.

**AI project assistance is explicitly excluded from this delivery.** No AI assistant, AI provider integration, or AI-generated project action is part of the current scope.

## Delivery status

- **PR #12 — collaboration suite:** merged into `main`. It delivered the team calendar, advanced task controls, notification centre, shared files, productivity analytics, chat, and meeting notes.
- **PR #19 — provider-verified sign-up:** contains the Google, GitHub, and Discord-only self-service account creation policy while retaining email/password login for existing accounts.
- **PR #20 — SQLx rollback bridge:** must be merged and deployed before the provider-sign-up migration is applied.
- **PR #21 — non-AI roadmap completion:** bundles the PR #19 provider sign-up change with the remaining non-AI collaboration enhancements described below. It is a draft implementation branch and is not deployed.

**Release sequence:** merge and deploy PR #20 first; then merge and deploy PR #21. PR #21 contains the provider-verified sign-up changes from PR #19, so PR #19 is superseded by the combined PR #21 change set. Do not deploy PR #21 before the PR #20 migration-runner compatibility release is active on every API instance.

## Phase 1 — Core workspaces and project boards

Implemented:
- Workspace membership, member roles, starter workspace and project.
- Project boards, task lists, tasks, task assignments and comments.
- Persisted task ordering and workspace activity history.
- Invitation creation, revocation, expiry and email-bound acceptance.
- Protected Rust/Axum routes backed by PostgreSQL and SQLx.

## Phase 2 — Authentication and account security

Implemented in PR #19 and included in PR #21:
- New self-service accounts must use Google, GitHub or Discord OAuth.
- A new account is created only after the provider response confirms its email is verified.
- Google uses the `email_verified` claim, GitHub uses the verified email list, and Discord uses its verified-email flag.
- Existing email/password accounts can continue to log in; password-only registration returns `403 Forbidden`.
- OAuth state is one-time, hashed in PostgreSQL, and bound to a short-lived HttpOnly browser cookie.
- Provider identities are not silently linked to existing accounts solely because the email matches.
- PostgreSQL records `email_verified_at`; migration 0009 includes database-side guards so password-only user inserts from old API instances are rejected.

External setup still required:
- Create the three provider applications.
- Register the exact production HTTPS callback URLs.
- Store provider client IDs and secrets only in the backend environment.
- Complete OAuth consent/branding/privacy policy requirements.

## Phase 3 — Team calendar and deadline planner

Implemented:
- Month, week and day views for project and workspace events.
- Task due dates displayed with event deadlines.
- Event types for meetings, milestones, exams, submissions and focus blocks.
- Create, edit, reschedule and delete events.
- Filters by event type, project and attendee.
- All-day events retain their selected calendar dates and time zone.
- Workspace-member attendee invitations and accepted/declined responses.
- One-time attendee reminders with durable notification deduplication.

External/acceptance gates:
- Configure a deployment time zone policy and test daylight-saving transitions for the supported regions.
- Complete two-user attendee invitation and reminder acceptance tests.
- Calendar events are not integrated with external Google Calendar/Outlook calendars.

## Phase 4 — Advanced task management

Implemented:
- Subtasks, checklists, workspace labels, estimates and dependency links.
- Same-project dependency validation and cycle checks in the API.
- Recurring task rules and a durable `task_recurrence_runs` idempotency ledger.
- Atomic recurrence generation so task creation, copied assignments, ledger insertion, and rule advancement commit together.
- Server-backed My Tasks filters for project, status, priority, assignee, effort range, blockers and due-date range.

Acceptance gates:
- Test recurring jobs with multiple workers, delayed schedules and concurrent retries against PostgreSQL.
- Verify task hierarchy and dependency behavior with real multi-user project data.
- Define how reopened, cancelled, and archived tasks affect reporting metrics.

## Phase 5 — Smart notification centre

Implemented:
- Persistent inbox, unread counts, mark-read and mark-all-read.
- Cursor-based older-notification pagination.
- Per-category in-app and email preferences.
- Deadline and calendar reminders.
- Workspace invitation notices for already registered invitees.
- Mention notifications from task comments and shared team channels.
- Notification deduplication using stable keys.
- Email outbox retry, attempt fencing and suppression controls from the earlier collaboration work.

External gates:
- Configure Resend API credentials and a verified sender domain.
- Validate SPF, DKIM, DMARC, bounce handling, unsubscribe/suppression and privacy/retention policy before enabling email delivery.

## Phase 6 — Shared files and documents

Implemented:
- Authenticated workspace, project and task file upload, listing, download and deletion.
- Opaque server-generated storage keys and SHA-256 checksums.
- File content-signature checks against supported extensions.
- Per-workspace storage quota, upload reservations and quota-overrun prevention.
- Admin quota management and file audit history for upload, preview, download, queued deletion and completed deletion.
- Authenticated inline PDF and raster-image preview.
- Optional ClamAV `INSTREAM` scanning.
- When `CAMPUSFORGE_REQUIRE_MALWARE_SCAN=true`, unscanned files cannot be downloaded or previewed; infected files are blocked.

External gates:
- Provision a private S3-compatible bucket, encryption, least-privilege access keys, backup and lifecycle rules.
- Run a ClamAV service reachable from the API and configure `CAMPUSFORGE_CLAMAV_ADDR`; set the require-scan flag before public use.
- Complete malware test-file, oversized file, interrupted-upload, delete/restore and quota-race tests.
- Approve retention and deletion policies. Local filesystem storage is for localhost development, not production durability.

## Phase 7 — Team chat and meeting notes

Implemented:
- Workspace-wide and project channels with cursor-paginated message history.
- Direct messages stored in private conversations with participant-only REST authorization.
- Recipient-scoped private-message WebSocket events; private message activity is not broadcast to every workspace member.
- Shared-channel mentions, pinned announcements, author edits/deletes and workspace-admin moderation.
- Live presence leases, typing indicators, and PostgreSQL LISTEN/NOTIFY relay across API instances.
- Meeting notes with title, meeting time, project, attendees, agenda and discussion notes.
- Structured meeting action items with assignee, due date, completion and optional linked task.

Acceptance gates:
- Run multi-browser/two-account tests for private-message isolation, member removal, presence reconnect and typing behavior.
- Add end-user-facing retention/export/delete rules for chat and meeting records.
- Fine-grained per-board/channel authorization remains a separate permission-hardening item.

## Phase 8 — Team productivity analytics

Implemented:
- Live workspace metrics for task completion, overdue work, unassigned work, project progress and member workload.
- Daily snapshots stored in PostgreSQL and refreshed during the day.
- Date-range history, trend visualization and CSV export.
- Workspace-scoped analytics access.

Acceptance gates:
- Agree on metric definitions for reopened, cancelled, archived and multi-assignee tasks.
- Review privacy and fairness before showing comparative individual-contribution rankings.
- Retain/aggregate historical data according to an approved policy.

## Non-AI out-of-scope items

The following items are intentionally not part of the current delivery:
- AI project assistant, AI summaries, AI task generation or AI provider integrations.
- External Google/Outlook calendar synchronization.
- Full video meetings, call recording or meeting transcription.
- External email delivery until Resend is configured and verified.
- Full rich-text collaborative document editing.

## Cross-feature engineering and security requirements

- PostgreSQL is the source of truth; mutations are committed before change events are published.
- Every resource endpoint verifies workspace/resource scope and the authenticated user.
- File storage remains private and server-mediated.
- Email jobs require explicit opt-in.
- Private direct-message events are sent only to the intended participant.
- Database changes are additive and versioned. Never edit applied migrations.
- Use PR #20's SQLx migration runner compatibility release before applying migration 0009 and newer schema versions.
- Run the PostgreSQL-backed migration tests, frontend build, Rust tests, dependency audit and CodeQL on the final PR head.
- Complete browser, accessibility, multi-user, backup/restore and operational acceptance tests before public release.

## Deployment gate

The non-AI completion work is in draft PR #21 and has not been merged or deployed. Render remains unchanged. Before release, verify provider OAuth applications, the private S3 bucket, ClamAV, Resend, database backups, retention, privacy, and the two-user acceptance tests. Deploy only in the order described above and only after the user explicitly authorizes release.
