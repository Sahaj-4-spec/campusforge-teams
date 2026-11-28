# CampusForge Teams — Product Roadmap

## Product objective

CampusForge Teams is a project and collaboration workspace for student teams and small delivery groups. The product combines project boards, task planning, coordination, shared resources, calendar workflows, team communications, and measurable project progress.

**AI project assistance is a separate optional Track B scope.** The merged Track B release includes a read-only assistant, Project Pulse, and durable human-approved task proposals. The features remain disabled by default pending provider/privacy/cost approvals.

## Delivery status — 28 November 2026

Track A's non-AI collaboration features and Track B's optional AI assistant/proposal implementation are merged into `main`. The latest security, recovery, invitation, release-infrastructure and documentation updates have also been merged.

- **PR #41:** integrated Track A/Track B release — merged.
- **PR #43:** DML-only runtime role and protected migration/runtime separation — merged.
- **PR #49:** static-host path/symlink hardening, Track B quota/permission hardening — merged; frontend build, Rust checks/tests, dependency audit and CodeQL passed.
- **PR #52:** single-use password-reset tokens, session invalidation and password-recovery UI — merged; frontend build, Rust checks/tests, dependency audit and CodeQL passed.
- **PR #53:** isolated production Render Blueprint with paid Rust API and private ClamAV scanner — merged; Blueprint registered with auto-sync disabled.
- **PR #54:** Resend-backed workspace invitation delivery, throttling and manual-link fallback — merged; frontend build, Rust checks/tests, dependency audit and CodeQL passed.
- **PR #55 and #56:** release documentation corrections — merged.

The latest main commit is `205ab6bec8da3cf31824798bd943147e46c35738`; it is a documentation-only commit after the application/production-Blueprint commit `4f27ee69e4d90ea9292859090c3f435235d66715`. The integrated Render frontend and Rust API preview are separate from production. The frontend's deployment of the documentation commit reached `live`; at the last API check, Render had completed the build and was still applying the documentation-only revision. The previous application API revision on `4f27ee6` was confirmed live. The API configuration remains connected to the isolated Neon preview database, not production.

Direct Neon journal checks on 28 November 2026 show production `campusforge_app` at migrations 0001–0017 and isolated preview `campusforge_release_preview` at 0001–0018. Both journals report zero failed migrations. Migration 0018 is the password-reset token table and is preview-only; production migration must be run through the protected one-off migration process after backup verification.

**Production release status:** the separate `CampusForge Production (isolated)` Render Blueprint is registered against `main` and `render.production.yaml`, with auto-sync disabled and no resources created. Its first plan failed with `need_payment_info` for the paid Rust API and private ClamAV services. No paid production services have been created and the preview must not be treated as production.

**External release gates:** provision the dedicated production API/frontend after reviewing Render costs; configure the production Neon DML-only runtime URL and separate private object-storage credential; provision private ClamAV scanning; configure Google/GitHub/Discord OAuth credentials and exact HTTPS callbacks; configure Resend, a verified sender, SPF/DKIM/DMARC; apply migration 0018 through the protected release operation; establish recurring backups/PITR and verify a populated-data restore; configure custom domain, trusted ingress/WAF, monitoring and incident response; and complete two-account authorization, WebSocket, browser/mobile, accessibility and privacy/retention acceptance. Password recovery is implemented in source/preview; provider linking through account settings remains a separate unimplemented enhancement.

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
- The merged main release implements server-recorded JWT session IDs, active-session checks on protected API requests, logout revocation, WebSocket revalidation, and scheduled session cleanup. Migrations 0015–0017 are applied in production and preview. Two-account runtime acceptance remains pending.
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
- Per-board/channel authorization is implemented in the merged main release; two-account negative tests remain a release-acceptance item.

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

The integrated Track A/Track B source is merged into `main` and deployed to the isolated Render preview. The main-branch preview auto-deploys on commits; a successful build or preview deployment is not production approval.

The production stack is defined separately in `render.production.yaml`. Its registered Blueprint has auto-sync disabled and has no resources. The first plan is blocked because Render requires billing information for the paid API and private scanner. Do not connect the public frontend to the preview API or production database until the production stack has been provisioned, its required credentials are configured through protected provider dashboards, and the checklist in [Track A Final Acceptance](TRACK_A_FINAL_ACCEPTANCE.md) is evidenced.

The API must keep `CAMPUSFORGE_AUTO_MIGRATE=false` and use the DML-only runtime role. Production migration 0018 must run through the protected one-off `MIGRATION_DATABASE_URL` release procedure only after a current backup/restore point is confirmed. Do not copy production secrets into Git or chat.

The release owner must sign off every applicable item in [Track A Final Acceptance](TRACK_A_FINAL_ACCEPTANCE.md): production infrastructure, OAuth/email, private storage and required malware scanning, migration 0018, backup/PITR and populated-data restore, privacy/retention, two-account end-to-end tests, accessibility, monitoring and incident response. The AI project assistant and Project Pulse remain optional, feature-flagged, and disabled until provider/privacy/cost controls are approved.

### Board-scoped notification authorization

The integrated candidate now records the board on each activity notification and applies the same board access rule when selecting recipients, listing inbox items, updating read state, and dispatching email. Migration 0016 adds the required column/index and backfills task notifications when the task still exists. Workspace invitations keep their separate invitation validation.
