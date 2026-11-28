# CampusForge Teams — Integrated Release Acceptance

**Status date:** 28 November 2026  
**Scope:** Track A collaboration product plus optional Track B AI assistant and human-approved task proposals.  
**Merged commit:** `bced1af502303b86bfb17d0eabdac18ea2985dc1`  
**Release status:** Integrated Track A/B source is merged to `main`; the Render frontend preview and Rust API deployment are live. Production acceptance is still incomplete.

## 1. What the candidate contains

CampusForge Teams is built on React/Vite, Rust/Axum, PostgreSQL and SQLx. The candidate consolidates:

- Verified Google, GitHub and Discord OAuth sign-up plus existing-account password login.
- Workspaces, project boards, lists, task management, assignment, comments, invitations and activity.
- Calendar, milestones, advanced task controls, notifications, files, chat, meeting notes, analytics and the encrypted Save Locker.
- Track A session hardening: random JWT `jti` identifiers, PostgreSQL-backed active-session checks, logout revocation and WebSocket session revalidation.
- Track B: feature-flagged AI assistant, Project Pulse, structured task proposals and explicit human review/approval.

The AI assistant remains disabled by default. Provider calls require an explicit data-sharing acknowledgement. Proposals store structured task suggestions and metadata-only audit events, not prompts or raw model completions. Model output cannot autonomously mutate projects.

## 2. Verified integration state

### GitHub

- Default branch `main` contains integrated commit `bced1af502303b86bfb17d0eabdac18ea2985dc1` (PR #41, squash-merged on 28 November 2026).
- PRs #38, #39, #40 and #42 were closed as superseded because their changes are included in the integrated commit; they were not merged separately.
- Integrated frontend build, Rust backend check, dependency vulnerability audit, CodeQL and Dependabot checks passed on the merged commit.
- A follow-up security PR #43 separates runtime database privileges from schema migrations; it is not part of the merged commit until its checks pass and it is merged.

### Database

- Neon project `neon-rose-book`; default branch `main`; application database `campusforge_app`.
- SQLx journal versions 0001–0017 were verified successful. Versions 0014–0017 are respectively AI proposals, revocable auth sessions, board-scoped notifications and proposal-history retention. Each recorded migration is successful and has the expected 48-byte SQLx SHA-384 checksum.
- The merged API started against `campusforge_app` and applied pending migrations 0015–0017 at startup. The `auth_sessions` migration is therefore applied; do not rerun or rename prior versions.
- The running Render API was observed connecting as `neondb_owner`, a role with broad database/role creation and RLS-bypass capabilities. A DML-only `campusforge_app_runtime` role has been created and verified against the 43 application tables, but the Render API connection string had not yet been switched at the time of this update.
- The isolated Neon branch `campusforge-track-a-b-preview` and database `campusforge_release_preview` remain separate from the default branch.
- Backup/PITR configuration and an isolated restore drill are not evidenced.

### Render

- Frontend preview: `campusforge-preview` — `https://campusforge-preview.onrender.com`. It has been rerouted to the isolated `campusforge-track-a-api-preview` service.
- Track A isolated API preview: `https://campusforge-track-a-api-preview.onrender.com`, confirmed against Neon database `campusforge_track_a_preview` using role `campusforge_track_a_app`; it uses local development file storage and is not a production service.
- Rust service `campusforge-api-test` — `https://campusforge-api-test.onrender.com` — is live on the integrated commit, but its actual Neon target is the default `campusforge_app` database and it uses the `neondb_owner` role. Despite its name, it is production-connected and must not be treated as a disposable test API.
- The integrated API startup log confirmed the database migration journal, private S3-compatible bucket configuration and a successful storage PUT/GET/DELETE check.
- The legacy `campusforge-teams` Node service still fails its main-branch deploy because its build invokes `vite` without installing the client dependencies (`vite: not found`). Its Render runtime/build configuration has not been corrected.
- End-to-end account, OAuth, cross-account, mobile and accessibility acceptance remains outstanding.

## 3. Migration and release ordering

SQLx stores SHA-384 migration checksums (48 bytes). The live SQLx journal was read after rollout and reports versions 0001–0017 successful. The integrated API automatically applied 0015–0017 on startup before migration execution was separated into a one-off release operation in follow-up PR #43.

1. Keep `0014_ai_action_proposals.sql` unchanged; it is already recorded in Neon SQLx migration history.
2. Treat migrations 0015–0017 as applied on the current `campusforge_app` database; do not reapply or manually rewrite the journal.
3. Verify a Neon backup/PITR point and complete an isolated restore drill before any further schema change.
4. For future schema changes, use the protected migration command (`npm run db:migrate` with `MIGRATION_DATABASE_URL`) after the migration/runtime separation PR is merged. Do not run schema changes through the long-lived DML-only API role.
5. Verify password login, all configured OAuth providers, Bearer and cookie session revocation, WebSocket closure after revocation, and cleanup behavior with two separate accounts.
6. Do not roll back to a binary that ignores `auth_sessions` after migration 0015 has been applied. Follow the rollback procedure in [Database migration release order](DATABASE_MIGRATION_RELEASE_ORDER.md).

JWTs issued before session migration 0015 do not contain `jti`; users will need to authenticate again after rollout.

## 4. Production acceptance checklist

| Area | Status | Required evidence |
|---|---|---|
| Track A application source | **Merged to `main`** | PR #41 merged as `bced1af`; integrated build and security checks passed. |
| Track B assistant/proposals | **Merged; disabled by default** | Provider configuration, privacy/retention review, spend limits and authorization acceptance are still release gates. Project Pulse and AI assistant remain disabled by default. |
| Integrated CI/security | **Passed on merged commit** | Frontend build, Rust backend check, dependency vulnerability audit, CodeQL and Dependabot checks passed. |
| Neon schema 0001–0017 | **Verified** | SQLx journal reports all versions successful with 48-byte checksums. |
| Neon migration 0015 | **Applied** | `auth_sessions` migration recorded successful on the live application database. |
| Neon migration 0016 | **Applied** | Notification board-scope migration recorded successful. |
| Neon migration 0017 | **Applied** | Proposal-history retention migration recorded successful; data-level acceptance remains outstanding. |
| Database backups/restore | **Not verified** | Configure retention/PITR and complete a restore into an isolated environment. |
| Google/GitHub/Discord OAuth | **Not configured/verified** | Supply client IDs/secrets, register exact HTTPS callback URLs, and pass verified-email sign-up tests for each provider. |
| Private object storage | **Startup I/O verified; policy review pending** | The integrated API reports a configured private S3-compatible bucket and successful PUT/GET/DELETE verification. Still verify bucket ACL/IAM scope, encryption, lifecycle/versioning, backups and restoration. |
| Malware scanning | **Not configured/verified** | Provision reachable ClamAV, require scanning in production, and prove infected/unscanned uploads and downloads are blocked. |
| Email delivery | **Not configured/verified** | Configure Resend API key, verified sender, SPF/DKIM/DMARC and test delivery, bounce/suppression and retry handling. |
| Perimeter protection | **Not configured/verified** | Configure trusted proxy/edge rate limits and validate forwarded-client-IP handling. |
| Two-account authorization | **Pending** | Test member/viewer/admin boundaries, board visibility, invitations, files, chat/DM isolation, comments, calendar and task mutations. |
| Browser/mobile/accessibility | **Pending** | Test supported browsers and real mobile devices, keyboard, screen reader, focus, contrast and error/empty/loading states. |
| Privacy and retention | **Pending** | Approve policies for personal data, files, messages, meeting notes, AI provider processing, export and deletion. |
| Fine-grained authorization | **Enforced in candidate; acceptance pending** | Project snapshots, task mutations, advanced task actions, files, calendar, chat, meeting notes, analytics, AI proposals and task/board notifications check board membership/roles; viewers are read-only. Database-backed board-role tests and two-account end-to-end negative tests must pass before release. Workspace-level channels remain workspace-scoped. |
| Account recovery/provider linking | **Not complete** | Add or document supported recovery and secure account-linking paths before broad launch. |
| Production hosting | **Not accepted** | The canonical `campusforge-teams` Node service still fails its Render build; a correct production domain/runtime/monitoring and operational acceptance are outstanding. |
| Production release | **Partially deployed; not accepted as launch-ready** | The integrated source is merged and an API connected to the production database is live, but the legacy site fails and security/operations acceptance gates remain open. |

## 5. Current release decision

**Source integration:** Merged into `main` as `bced1af`; merged CI/security checks passed.  
**Database:** Migrations 0001–0017 are recorded successful on `campusforge_app`.  
**Preview:** `campusforge-preview` is live and routes to the isolated Track A API preview.  
**Integrated API:** `campusforge-api-test` is live but connected to the production Neon database; its owner-role connection still needs to be replaced by the DML-only runtime role through follow-up PR #43.  
**Legacy public service:** `campusforge-teams` Node deploy is failing (`vite: not found`).  
**Production acceptance:** **Not complete**.  
**Launch decision:** **Not production-ready** until the runtime-role switch, legacy service correction, OAuth configuration, backup/restore proof, malware scanning, provider/email verification, two-account authorization and mobile/accessibility acceptance are complete.

Do not describe the application as production-ready before each applicable gate has evidence. The notification authorization change (migration 0016) is already applied to the live application database; a running API binary must match that schema.
