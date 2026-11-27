# CampusForge Teams — Product Roadmap

## Product objective

CampusForge Teams is a project and collaboration workspace for student teams and small delivery groups. The product combines project boards, task planning, coordination, shared resources, calendar workflows, team communications, and measurable project progress.

**AI project assistance is a separate optional Track B scope.** The integrated candidate adds a read-only assistant, Project Pulse, and durable human-approved task proposals. AI stays disabled by default pending provider/privacy/cost approvals.

## Delivery status — 28 November 2026

The non-AI feature implementation described in Phases 1–8 is merged into `main`. The release history is:

- **PR #12:** collaboration suite — merged.
- **PR #20:** SQLx rollback-compatible migration runner — merged.
- **PR #21:** provider-verified sign-up plus remaining non-AI collaboration roadmap features — merged.
- **PR #23:** live board presence, active card editing indicators, and threaded task discussion — merged.
- **PR #24:** owner-only workspace teammate role editing — merged.
- **PR #26:** encrypted personal Save Locker — merged.
- **PR #30:** optimistic Kanban drag-and-drop with rollback — merged.
- **PR #32–#34:** auth/Kanban UI polish, command palette/performance, and mobile navigation — merged.
- **PR #35–#36:** Neon database/object-storage verification and fail-closed production startup guards — merged.
- **PR #37:** shared PostgreSQL-backed rate limits for password login and OAuth initiation — merged.

PR #19 was closed without merging; its verified-provider sign-up implementation was included in PR #21. PR #20 and #21 are no longer pending. The historical order remains important: the SQLx compatibility bridge was merged before the provider-sign-up migrations were introduced.

The latest recorded CI and security workflows for main commit `2b0c5bf` passed on 24 November 2026. The separate Track A/Track B component PRs also reported green frontend build, Rust, dependency and CodeQL checks. Draft PR #41 now runs the integrated checks; its CI state is tracked separately and must pass before merge. Browser-based two-account acceptance, accessibility review, backup/restore exercises, and production integration tests are separate release gates.

The integrated candidate `release/campusforge-integrated-candidate` combines the Track A session-revocation code with Track B assistant and proposal approval work. Migration 0014 is the AI proposal/audit migration already present in Neon; revocable sessions are migration 0015. The separate feature branches have green component checks, but the integrated candidate still needs fresh CI and runtime PostgreSQL session-revocation acceptance. Pre-0015 JWTs lack a session ID and require fresh sign-in after rollout.

**Hosting status:** the Render preview and API are test services. The repository's `render.yaml` currently tracks `main` with commit-triggered auto-deploy. Therefore, do not merge finalization changes to `main` until the user explicitly authorizes the corresponding deployment and the external production gates below are met. The current source branch being complete does not prove that the deployed database or services have been upgraded.

**External release gates:** configure production Google/GitHub/Discord OAuth applications and exact HTTPS callbacks; migrate to durable Neon PostgreSQL and verify row counts, backups, and restoration; provision private Neon Object Storage; configure a private ClamAV service and require clean scans; verify Resend sender-domain authentication and delivery; configure trusted ingress/WAF controls; finish two-user/browser/accessibility tests; approve privacy, data-retention, export, and deletion policies; and close the documented per-board/channel authorization and account-recovery gaps.

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
- Shared PostgreSQL-backed fixed-window limits protect password login by client IP and normalized email, and OAuth initiation by client IP.
- The integrated candidate adds server-recorded JWT session IDs, active-session checks on protected API requests, logout revocation, WebSocket revalidation, and scheduled session cleanup. Its combined CI and PostgreSQL runtime acceptance remain pending.
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
- PR #20's SQLx migration runner compatibility release is already merged before migrations 0009–0013. Preserve its rollback-compatible behavior for future additive migrations.
- Run the PostgreSQL-backed migration tests, frontend build, Rust tests, dependency audit and CodeQL on the final Track A hardening PR head.
- Complete browser, accessibility, multi-user, backup/restore and operational acceptance tests before public release.
- Confirm that production ingress overwrites trusted client-IP headers. Application login/OAuth limits are now PostgreSQL-backed (login: 30 per IP/15 minutes and 10 per normalized email/15 minutes; OAuth initiation: 30 per IP/10 minutes), but this is not a substitute for perimeter/WAF protection.

## Deployment gate

The non-AI implementation is merged into `main`; production deployment is still withheld. The current Render preview uses a temporary free PostgreSQL database (documented expiry: 22 December 2026) and must not be treated as durable production storage. Check the actual deployed migration version before any release; source migrations being present in GitHub does not mean they have been applied to the running database.

The integrated candidate adds migration 0015 for revocable sessions after migration 0014 for AI proposal persistence; the combined branch must pass CI/review and runtime database checks before release. Because `render.yaml` uses commit-triggered auto-deploy from `main`, do not merge it to `main` until the user has explicitly authorized the release and the production configuration is prepared. No deployment is authorized by the roadmap or this branch.

The release owner must sign off every item in [Track A Final Acceptance](TRACK_A_FINAL_ACCEPTANCE.md): durable database and restore drill, private object storage, mandatory malware scanning, OAuth, email delivery, edge protection, privacy/retention, two-account end-to-end tests, accessibility, and operational monitoring. The AI project assistant remains optional, feature-flagged, and disabled until provider/privacy/cost controls are approved.


### Integrated candidate state — 28 November 2026

- Code is consolidated on `release/campusforge-integrated-candidate`; it is not merged into `main`.
- The Neon `campusforge_app` SQLx journal records successful migrations 0001–0014, including the AI proposal tables. Migration 0015 adds revocable sessions and is awaiting integrated validation before application.
- Render still identifies the Rust API and frontend as preview/test services. The most recent API deploy built successfully but was still reported `update_in_progress` at the last observation; do not assume the new database connection is serving traffic.
- No production release is authorized. Merge/deploy remains gated on the release owner’s explicit approval and the checklist in `TRACK_A_FINAL_ACCEPTANCE.md`.
