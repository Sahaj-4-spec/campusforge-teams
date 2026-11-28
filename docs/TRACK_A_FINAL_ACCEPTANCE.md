# CampusForge Teams — Integrated Release Acceptance

**Status date:** 28 November 2026  
**Scope:** Track A collaboration product plus optional Track B AI assistant and human-approved task proposals.  
**Integrated source commit:** `bced1af502303b86bfb17d0eabdac18ea2985dc1` (PR #41)  
**Current main commit:** `4f27ee69e4d90ea9292859090c3f435235d66715` (PR #53; production Blueprint only)  
**Release status:** Track A/B source, Track B hardening, secure password recovery and Resend-backed invitation delivery are merged to `main`. The canonical Render frontend and Rust API preview are deployed `live` on the current main commit. Production acceptance is still incomplete.

## 1. What the candidate contains

CampusForge Teams is built on React/Vite, Rust/Axum, PostgreSQL and SQLx. The candidate consolidates:

- Verified Google, GitHub and Discord OAuth sign-up plus existing-account password login.
- Workspaces, project boards, lists, task management, assignment, comments, invitations and activity.
- Calendar, milestones, advanced task controls, notifications, files, chat, meeting notes, analytics and the encrypted Save Locker.
- Track A session hardening: random JWT `jti` identifiers, PostgreSQL-backed active-session checks, logout revocation and WebSocket session revalidation.
- Track A account recovery: SHA-256-hashed, one-time password-reset tokens with 30-minute expiry, Argon2 password updates and revocation of active sessions. Migration 0018 is applied to the isolated preview database; production migration remains pending.
- Track A invitations: email delivery through the server-side Resend transport when configured, with a manual-link fallback and per-actor/per-recipient rate limits. Email delivery remains disabled until a verified Resend sender is configured.
- Track B: feature-flagged AI assistant, Project Pulse, structured task proposals and explicit human review/approval.

The AI assistant remains disabled by default. Provider calls require an explicit data-sharing acknowledgement. Proposals store structured task suggestions and metadata-only audit events, not prompts or raw model completions. Model output cannot autonomously mutate projects.

## 2. Verified integration state

### GitHub

- Default branch `main` contains integrated commit `bced1af502303b86bfb17d0eabdac18ea2985dc1` (PR #41, squash-merged on 28 November 2026).
- PRs #38, #39, #40 and #42 were closed as superseded because their changes are included in the integrated commit; they were not merged separately.
- PR #49 passed frontend build, Rust backend compilation/unit tests, dependency vulnerability audits and CodeQL, then was merged as `39fc486`.
- PR #52 added password recovery and migration 0018; frontend build, Rust compile/tests, dependency audits and CodeQL passed. It was merged as `5fd2bdb`, and the preview API/frontend reached `live`.
- PR #54 added invitation email delivery/rate limiting; frontend build/tests, Rust compilation/unit tests, dependency audits and CodeQL passed. It was merged as `fdfaf80`. PR #55 refreshed release documentation, and PR #53 merged the isolated production Render Blueprint. The current main commit is `4f27ee6`.
- PR #43 (`2bfa2a105d6986fa9ac79679fc15614360edf9d1`) is merged. It separates runtime database privileges from schema migrations, adds a protected one-off SQLx migration runner, and defaults production API startup to no DDL.

### Database

- Neon project `neon-rose-book`; default branch `main`; application database `campusforge_app`.
- Production SQLx journal versions 0001–0017 were verified successful. Integrated preview journal versions 0001–0018 are successful; migration 0018 adds `password_reset_tokens` with 48-byte SQLx SHA-384 checksum `48ba867f4a253f2f3568ee053269b9fec2f6099d64059d35924e766abcdaf6457e322e82d3cb6597e97b83537c12cee8`.
- The merged API started against `campusforge_app` and applied pending migrations 0015–0017 at startup. The `auth_sessions` migration is therefore applied; do not rerun or rename prior versions.
- The initial post-PR #41 rollout temporarily connected `campusforge-api-test` to `campusforge_app` as `neondb_owner` and applied migrations 0015–0017 there. It was subsequently moved to isolated child branch `campusforge-track-a-b-preview` and database `campusforge_release_preview`. Its DML-only `campusforge_release_runtime` role has been verified against all 43 preview tables and lacks superuser, role-creation, database-creation, RLS-bypass and schema-CREATE privileges. The production database has a separate `campusforge_app_runtime` role. A direct connection as that role was verified against `campusforge_app`; it can read and write all 42 application tables, cannot TRUNCATE, has schema USAGE but no schema CREATE, and is not superuser, CREATEDB, CREATEROLE or BYPASSRLS. Default table/sequence privileges are configured for future owner-created objects. No production Rust API service is attached to that role yet.
- The isolated Neon branch `campusforge-track-a-b-preview` and database `campusforge_release_preview` remain separate from the default branch.
- A manual Neon snapshot `snap-orange-grass-bbu0vwvb` (`campusforge-prelaunch-2026-11-28`) was created from the production `main` branch on 28 November 2026 and restored without finalizing into branch `campusforge-restore-drill-2026-11-28` (`br-bold-math-bbmdqptf`). The restored `campusforge_app` contains all 43 tables and successful SQLx migrations 0001–0017 with 48-byte checksums. This verifies schema/journal restore only: selected core tables (`users`, `workspaces`, `boards`, `tasks`, `notifications`, `auth_sessions`, `ai_action_proposals`) contain zero rows in the restored database; the legacy `neondb` core tables are also empty. A populated-user-data restore has therefore not been proven. No recurring snapshot schedule is configured, and the restore branch could not be marked protected because the current Neon plan has reached its protected-branch limit.

### Render

- Integrated frontend preview: `campusforge-preview` — `https://campusforge-preview.onrender.com`; it is now configured to call the integrated API preview at `https://campusforge-api-test.onrender.com`, with AI and Project Pulse frontend flags disabled.
- Integrated Rust API preview: `campusforge-api-test` — `https://campusforge-api-test.onrender.com`; on main commit `4f27ee6` it uses the isolated Neon child branch database `campusforge_release_preview`, whose SQLx journal contains successful migrations 0001–0018. The DML-only `campusforge_release_runtime` role is configured for the service; automatic migrations are disabled.
- Track A-only API preview: `https://campusforge-track-a-api-preview.onrender.com`, backed by Neon `campusforge_track_a_preview` and role `campusforge_track_a_app`; it remains separate from both the production database and integrated preview database.
- Neon Object Storage is enabled on both branches and the bucket `campusforge-private-uploads` is private. The integrated API preview uses a child-branch-only `storage:read`/`storage:write` credential, path-style requests and `CAMPUSFORGE_REQUIRE_REMOTE_STORAGE=true`. Render deploy `dep-dc5c0e4o3iic73d8k5g0` is live and its startup log confirms successful S3 PUT/GET/DELETE verification. This is preview evidence only; no production API is using a production-branch storage credential. No private production ClamAV service is configured.
- The older `campusforge-teams` Node service completed a main-branch deploy on commit `39fc48635d2d063ce016bb99d0d03637cbd2d46f` after the root package scripts and client build were corrected; its current Render deploy is `live`, so the previous `vite: not found` failure is no longer the current deployment state. It has not been accepted as the production topology or independently verified as the canonical frontend/API pair. Keep the separate static frontend (`campusforge-preview`) and Rust API (`campusforge-api-test`) preview as the documented test pair until a dedicated production service topology is provisioned.
- End-to-end account, OAuth, cross-account, mobile and accessibility acceptance remains outstanding. Password-reset and invitation email flows are implemented in source, but real email delivery is unverified until Resend credentials and a verified sender are configured.

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
| Invitation and password-reset email | **Code merged; delivery not configured** | Invitation email and account reset email use server-side Resend when configured; add Resend key, verified sender and SPF/DKIM/DMARC, then test delivery and bounce/retry handling. Manual invitation-link fallback remains available. |
| Perimeter protection | **Not configured/verified** | Configure trusted proxy/edge rate limits and validate forwarded-client-IP handling. |
| Two-account authorization | **Pending** | Test member/viewer/admin boundaries, board visibility, invitations, files, chat/DM isolation, comments, calendar and task mutations. |
| Browser/mobile/accessibility | **Pending** | Test supported browsers and real mobile devices, keyboard, screen reader, focus, contrast and error/empty/loading states. |
| Privacy and retention | **Pending** | Approve policies for personal data, files, messages, meeting notes, AI provider processing, export and deletion. |
| Fine-grained authorization | **Enforced in candidate; acceptance pending** | Project snapshots, task mutations, advanced task actions, files, calendar, chat, meeting notes, analytics, AI proposals and task/board notifications check board membership/roles; viewers are read-only. Database-backed board-role tests and two-account end-to-end negative tests must pass before release. Workspace-level channels remain workspace-scoped. |
| Account recovery/provider linking | **Password recovery implemented; production rollout pending** | Preview migration 0018 and API/frontend recovery routes are live. Apply 0018 to production through the protected migration workflow, configure Resend, test one-time expiry/revocation flows, and complete secure provider-linking acceptance. |
| Production hosting | **Not accepted** | Main-branch frontend and Rust API previews are live on `4f27ee6`. The isolated production Blueprint is merged and registered with auto-sync disabled; its first plan failed with `need_payment_info` for the private ClamAV service and standard API plan. No production resources were created. Dedicated production domains, monitoring and operational acceptance remain open. |
| Production release | **Preview deployed; production not accepted** | The integrated source is merged and the preview API is live on an isolated Neon child database. No production API service is deployed; production OAuth/email, malware scanning, backup/restore and multi-account acceptance gates remain open. |

## 5. Current release decision

**Source integration:** Track A/B application source and the latest security, recovery, invitation, and release-infrastructure changes are merged to `main` through PRs #49, #52, #54 and #53. CI/security checks passed for PRs #49, #52, #54 and #55. The latest main commit is `4f27ee69e4d90ea9292859090c3f435235d66715`; PR #53 changes the production Blueprint, not application behavior.  
**Frontend preview:** `campusforge-preview` is `live` on main commit `4f27ee6`; its `VITE_API_BASE_URL` is configured to call `campusforge-api-test`.  
**Rust API preview:** `campusforge-api-test` is `live` on main commit `4f27ee6`, with `/api/health` configured as Render's health-check path. Latest startup logs confirm the isolated Neon `campusforge_release_preview` database and `campusforge_release_runtime` role, automatic SQLx migrations disabled, and private object-storage PUT/GET/DELETE verification successful.  
**Production database:** Neon `campusforge_app` has SQLx migrations 0001–0017 successful. The separate `campusforge_app_runtime` role has verified DML-only permissions; the production API has not been provisioned. Migration 0018 remains preview-only.  
**Production Blueprint:** `CampusForge Production (isolated)` is registered against `main`/`render.production.yaml`, with auto-sync disabled and no resources created. Its initial plan failed because Render requires billing information for the private ClamAV and standard API services. No approval or paid deployment was performed.  
**Backups:** A manual production snapshot was restored into an isolated branch and validated for schema/journal only. Core user/workspace/board/task tables in the restored dataset were empty; no populated-data restore or recurring snapshot schedule is configured.  
**Production acceptance:** **Not complete**.  
**Launch decision:** **Not production-ready** until the production Blueprint can be provisioned with reviewed costs, a dedicated API is attached to `campusforge_app_runtime`, production frontend/API origins and custom domain are verified, production private storage and private ClamAV scanning pass, Google/GitHub/Discord OAuth and email delivery credentials are configured/tested, migration 0018 is applied through the protected migration operation, recurring backup/PITR costs and retention are approved and populated-data restore evidence exists, and two-account, mobile and accessibility acceptance pass.

Do not describe the application as production-ready before each applicable gate has evidence. The notification authorization change (migration 0016) is already applied to the live application database; a running API binary must match that schema.
