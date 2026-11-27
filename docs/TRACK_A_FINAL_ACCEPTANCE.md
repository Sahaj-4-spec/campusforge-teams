# CampusForge Teams — Integrated Release Acceptance

**Status date:** 28 November 2026  
**Scope:** Track A collaboration product plus optional Track B AI assistant and human-approved task proposals.  
**Candidate:** `release/campusforge-integrated-candidate`  
**Release status:** Review candidate only. Not merged to `main`, not deployed as an integrated release, and not approved for production.

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

- Default branch reviewed: `main`, commit `2b0c5bf41225f1d27f079e7bd591761a0e972018`.
- Existing draft PRs: #38 (Track A session hardening), #39 (Track B assistant), and #40 (persistent proposal approval).
- Each component PR reports clean/mergeable status and its own frontend, Rust, dependency/security and CodeQL checks have passed.
- Integrated candidate branch: `release/campusforge-integrated-candidate`. A fresh combined-branch CI run is still required before review. Component PR checks are not a substitute for an integrated build/test.

### Database

- Neon project: `neon-rose-book`; branch: `main`; application database: `campusforge_app`.
- SQLx journal versions 0001–0014 were verified successful; version 14 is `ai_action_proposals`.
- AI proposal and audit tables are present.
- The session-revocation table `auth_sessions` is not yet present. Candidate migration `0015_revocable_auth_sessions.sql` adds it; do not rename or overwrite version 14.
- The legacy `neondb` database was preserved. The new `campusforge_app` database contained no user/proposal rows at the last check.
- A verified backup/PITR configuration and isolated restore drill have not been evidenced.

### Render

- Preview frontend: `campusforge-preview` — `https://campusforge-preview.onrender.com`.
- Rust API test service: `campusforge-api-test` — `https://campusforge-api-test.onrender.com`.
- Both services use the free/test setup and are not a production deployment.
- The API's `DATABASE_URL` environment setting was changed to the Neon `campusforge_app` database. Render unexpectedly started a deploy; the last observed status was `update_in_progress` after the build succeeded. Do not assume the service started or is healthy until the deployment and health endpoint are confirmed.
- The static frontend still targets the API test service. End-to-end live sign-in and workspace acceptance are not verified.

## 3. Migration and release ordering

1. Keep `0014_ai_action_proposals.sql` unchanged; it is already recorded in Neon SQLx migration history.
2. Run the candidate through integrated CI and review.
3. Take/verify a database backup and run the session migration in a controlled release window.
4. Apply `0015_revocable_auth_sessions.sql` with the matching session-aware API binary.
5. Verify password login, all configured OAuth providers, Bearer and cookie session revocation, WebSocket closure after revocation, and cleanup behavior.
6. Do not roll back to a binary that ignores `auth_sessions` after migration 0015 has been applied. Follow the rollback procedure in [Database migration release order](DATABASE_MIGRATION_RELEASE_ORDER.md).

JWTs issued before session migration 0015 do not contain `jti`; users will need to authenticate again after rollout.

## 4. Production acceptance checklist

| Area | Status | Required evidence |
|---|---|---|
| Track A application source | **Implemented in candidate** | Candidate includes the current Track A source and session hardening. |
| Track B assistant/proposals | **Implemented; disabled by default** | Provider configuration, privacy/retention review, spend limits, data-sharing acknowledgement and proposal authorization acceptance. |
| Integrated CI/security | **Pending** | Fresh checks on the integrated candidate, including frontend tests/build, Rust checks/tests, migration smoke test, dependency audit and CodeQL. |
| Neon schema 0001–0014 | **Verified** | SQLx journal success and AI proposal tables present. |
| Neon migration 0015 | **Pending** | Apply after candidate checks and backup/restore sign-off; verify `auth_sessions`. |
| Database backups/restore | **Not verified** | Configure retention/PITR and complete a restore into an isolated environment. |
| Google/GitHub/Discord OAuth | **Not configured/verified** | Supply client IDs/secrets, register exact HTTPS callback URLs, and pass verified-email sign-up tests for each provider. |
| Private object storage | **Not configured/verified** | Provision private S3-compatible bucket, encryption, least-privilege credentials, lifecycle/versioning and PUT/GET/DELETE verification. |
| Malware scanning | **Not configured/verified** | Provision reachable ClamAV, require scanning in production, and prove infected/unscanned uploads and downloads are blocked. |
| Email delivery | **Not configured/verified** | Configure Resend API key, verified sender, SPF/DKIM/DMARC and test delivery, bounce/suppression and retry handling. |
| Perimeter protection | **Not configured/verified** | Configure trusted proxy/edge rate limits and validate forwarded-client-IP handling. |
| Two-account authorization | **Pending** | Test member/viewer/admin boundaries, board visibility, invitations, files, chat/DM isolation, comments, calendar and task mutations. |
| Browser/mobile/accessibility | **Pending** | Test supported browsers and real mobile devices, keyboard, screen reader, focus, contrast and error/empty/loading states. |
| Privacy and retention | **Pending** | Approve policies for personal data, files, messages, meeting notes, AI provider processing, export and deletion. |
| Fine-grained authorization | **Known gap to verify** | Complete per-board/channel permission hardening and negative access tests. |
| Account recovery/provider linking | **Not complete** | Add or document supported recovery and secure account-linking paths before broad launch. |
| Production hosting | **Not provisioned** | Replace preview/test services with an appropriately sized production environment, domain, monitoring and operational runbooks. |
| Production release | **Withheld** | No merge to `main`, Blueprint sync or deployment without explicit release authorization. |

## 5. Current release decision

**Source integration:** Candidate assembled; combined CI pending.  
**Database:** Versions 1–14 verified; migration 15 pending.  
**Preview:** Existing Render API rollout not yet confirmed healthy.  
**Production acceptance:** **Not complete**.  
**Production release:** **Not approved / not deployed**.

The candidate can be considered release-ready only after every applicable checklist item has verifiable evidence and the release owner authorizes merge/deployment. Do not describe the application as production-ready before those gates pass.
