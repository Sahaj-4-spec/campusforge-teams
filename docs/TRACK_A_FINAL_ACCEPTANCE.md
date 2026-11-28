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
- PR #43 (runtime DB/migration separation), PR #44 (acceptance/runbook correction) and PR #46 (verified TLS, private ClamAV health gate and preview Blueprint) are also merged.
- The final merged commit `1639c8f133ab4c77b1ce8c85c14928e1b39a4341` passed frontend build, Rust backend check/unit tests, dependency vulnerability audit and CodeQL.
- PR #43 (`2bfa2a105d6986fa9ac79679fc15614360edf9d1`) is merged. It separates runtime database privileges from schema migrations, adds a protected one-off SQLx migration runner, and defaults production API startup to no DDL.

### Database

- Neon project `neon-rose-book`; default branch `main`; application database `campusforge_app`.
- SQLx journal versions 0001–0017 were verified successful. Versions 0014–0017 are respectively AI proposals, revocable auth sessions, board-scoped notifications and proposal-history retention. Each recorded migration is successful and has the expected 48-byte SQLx SHA-384 checksum.
- The merged API started against `campusforge_app` and applied pending migrations 0015–0017 at startup. The `auth_sessions` migration is therefore applied; do not rerun or rename prior versions.
- The initial post-PR #41 rollout temporarily connected `campusforge-api-test` to `campusforge_app` as `neondb_owner` and applied migrations 0015–0017 there. It was subsequently moved to isolated child branch `campusforge-track-a-b-preview` and database `campusforge_release_preview`. Its DML-only `campusforge_release_runtime` role has been verified against all 43 preview tables and lacks superuser, role-creation, database-creation, RLS-bypass and schema-CREATE privileges. The production database has a separate `campusforge_app_runtime` role with the same DML-only profile, and default table/sequence privileges are configured for future owner-created objects. No production Rust API service is attached to that production role yet.
- The isolated Neon branch `campusforge-track-a-b-preview` and database `campusforge_release_preview` remain separate from the default branch.
- A manual Neon snapshot `snap-orange-grass-bbu0vwvb` (`campusforge-prelaunch-2026-11-28`) was created from the production `main` branch on 28 November 2026 and restored without finalizing into branch `campusforge-restore-drill-2026-11-28` (`br-bold-math-bbmdqptf`). The restored `campusforge_app` contains all 43 tables and successful SQLx migrations 0001–0017 with 48-byte checksums. This verifies schema/journal restore only: selected core tables (`users`, `workspaces`, `boards`, `tasks`, `notifications`, `auth_sessions`, `ai_action_proposals`) contain zero rows in the restored database; the legacy `neondb` core tables are also empty. A populated-user-data restore has therefore not been proven. No recurring snapshot schedule is configured, and the restore branch could not be marked protected because the current Neon plan has reached its protected-branch limit.

### Render

- Integrated frontend preview: `campusforge-preview` — `https://campusforge-preview.onrender.com`; Render deploy `dep-dc5ch8ui0phs73d8l2q0` is live on commit `1639c8f`. Its `VITE_API_BASE_URL` is explicitly `https://campusforge-api-test.onrender.com`; AI and Project Pulse frontend flags are both set to `false`.
- Integrated Rust API preview: `campusforge-api-test` — `https://campusforge-api-test.onrender.com`; Render deploy `dep-dc5cf6utlv9c7397ei90` is live on commit `1639c8f`. It connects to Neon child branch database `campusforge_release_preview` as `campusforge_release_runtime`, uses `sslmode=verify-full`, and has automatic SQLx migrations disabled. Migrations 0001–0017 are successful.
- Track A-only API preview: `https://campusforge-track-a-api-preview.onrender.com`, backed by Neon `campusforge_track_a_preview` and role `campusforge_track_a_app`; it remains separate from both the production database and integrated preview database.
- Neon Object Storage is enabled on both branches and the bucket `campusforge-private-uploads` is private. The integrated API preview uses a child-branch-only `storage:read`/`storage:write` credential, path-style requests and `CAMPUSFORGE_REQUIRE_REMOTE_STORAGE=true`. Deploy `dep-dc5cf6utlv9c7397ei90` confirms S3 PUT/GET/DELETE verification on startup. This is preview evidence only; no production API uses a production-branch storage credential. PR #46 adds a mandatory private ClamAV PING/PONG startup probe and private-IP-only scanner connection; no production ClamAV service is provisioned.
- The legacy `campusforge-teams` Node service still fails its main-branch deploy: `yarn install` runs at repository root, then the root build invokes `npm --prefix client run build` without installing the client package's Vite dependency (`vite: not found`). Its start command is also `yarn start`, while the root start script launches Cargo; the Node runtime is not a valid host configuration for this monorepo API/frontend split. The existing service configuration must be replaced with a static frontend service plus a separate Rust API service, or converted to a supported Docker setup. The available Render connector cannot edit that existing service's build/start command directly.
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
| Database backups/restore | **Schema restore verified; data restore/retention pending** | A manual production-branch snapshot restored successfully to a separate branch with 43 tables and SQLx migrations 0001–0017. Restored core user/workspace/board/task tables had zero rows; there was no populated application dataset to validate. No recurring snapshot schedule is configured. |
| Google/GitHub/Discord OAuth | **Not configured/verified** | Supply client IDs/secrets, register exact HTTPS callback URLs, and pass verified-email sign-up tests for each provider. |
| Private object storage | **Preview I/O verified; production pending** | Integrated preview passes private Neon Object Storage PUT/GET/DELETE using a branch-scoped credential. Production API, production-branch credential and real user upload/download/deletion acceptance remain pending. Confirm provider retention/restore features before promising versioning/lifecycle behavior. |
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
**Preview:** `campusforge-preview` is live on commit `1639c8f` and explicitly targets the integrated API preview; AI and Project Pulse remain hidden.  
**Integrated API:** `campusforge-api-test` is live on commit `1639c8f`, isolated to `campusforge_release_preview`, connected as `campusforge_release_runtime` with verified TLS and automatic migrations disabled, and passes private Neon Object Storage PUT/GET/DELETE verification.  
**Legacy public service:** `campusforge-teams` Node deploy is still misconfigured (`vite: not found`; its `yarn start` invokes Cargo under a Node runtime).  
**Production acceptance:** **Not complete**.  
**Launch decision:** **Not production-ready** until a dedicated production Rust API uses `campusforge_app_runtime`, a supported production frontend/API topology replaces the failing legacy Node service, production-branch S3 credentials and a private reachable ClamAV service are configured and tested, Google/GitHub/Discord OAuth app credentials and Resend email sender/DNS are verified, a recurring backup/retention policy plus populated-data restore evidence exist, and two-account authorization plus mobile/accessibility acceptance pass.

Do not describe the application as production-ready before each applicable gate has evidence. The notification authorization change (migration 0016) is already applied to the live application database; a running API binary must match that schema.
