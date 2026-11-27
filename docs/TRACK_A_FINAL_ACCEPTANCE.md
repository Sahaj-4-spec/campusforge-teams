# CampusForge Teams — Track A Final Acceptance

**Track:** ALTA BUILDER 30 — Track A (Full-Stack Product)  
**Status date:** 27 November 2026  
**Scope:** Non-AI collaboration product. The AI assistant, AI summaries, AI-generated tasks, and model-driven project actions are excluded.  
**Release status:** Source implementation is consolidated on `main`; final session hardening is on a review branch. Production acceptance is not yet signed off. No deployment is authorized.

## 1. What Track A includes

CampusForge Teams is a responsive project and collaboration workspace built on:

- **Frontend:** React, Vite, Tailwind CSS, and the existing component/design system.
- **Backend:** Rust and Axum with HTTP REST endpoints and native WebSockets.
- **Database:** PostgreSQL with SQLx migrations and parameterized queries.
- **Identity:** Verified Google, GitHub, and Discord OAuth for new accounts; Argon2 login remains available to existing password accounts.
- **Collaboration:** Workspaces, project boards, ordered lists/tasks, members, assignments, threaded comments, activity history, presence, active-card editing indicators, chat, and direct messages.
- **Planning:** Calendar and deadlines, task subtasks/checklists/labels/dependencies/recurrence, and structured meeting action items.
- **Team operations:** Notification inbox/preferences, file upload/download/preview/audit/quota controls, team analytics/history/CSV export, and meeting notes.
- **Personal utility:** End-to-end encrypted personal Save Locker.
- **Experience:** Responsive mobile navigation, optimistic drag-and-drop with rollback, global command palette, and performance/UI refinements.

The implementation is represented by merged PRs #12, #20–#21, and #23–#37. PR #19 was closed unmerged because its verified-provider sign-up work was included in PR #21. PR #20's rollback-compatible SQLx runner was merged before PR #21 introduced migrations 0009 and 0010.

## 2. Verified repository state

As reviewed on 27 November 2026:

- Default branch: `main`.
- Source HEAD reviewed: `2b0c5bf41225f1d27f079e7bd591761a0e972018` (24 November 2026).
- Latest recorded main-branch **CampusForge CI** and **CampusForge Security** workflows completed successfully on 24 November 2026.
- Recorded checks included frontend production build and client regression tests, Rust backend check/tests, PostgreSQL migration smoke tests, dependency vulnerability audit, and CodeQL JavaScript analysis.
- No open product-feature PR was found; the open PRs at the time of review were Dependabot dependency updates. Dependency PRs must be reviewed individually and must not be bulk-merged.
- The current `render.yaml` points the API and frontend services at `main` and uses commit-triggered auto-deploy. A push/merge to `main` can therefore trigger a deployment.

These are source/repository observations. A green CI run does not prove that OAuth providers, managed database, file storage, email, production backups, or end-user workflows are configured and accepted.

## 3. Final hardening branch

Branch: `track-a/final-session-hardening-docs`

This branch adds database-backed session revocation:

1. Migration `0014_revocable_auth_sessions.sql` creates an `auth_sessions` table containing a random session UUID, user ID, expiry, and optional revocation time. Raw JWTs are never stored.
2. Password login and OAuth login issue a JWT with a random `jti` and persist its session row before setting the cookie.
3. Protected HTTP requests require a valid JWT and a matching, unexpired, non-revoked database session.
4. Logout revokes the database session before clearing the HttpOnly cookie.
5. WebSocket handshakes validate the session; open sockets re-check the session during their 30-second presence heartbeat and close after revocation is detected.
6. A background cleanup removes expired/revoked records after a seven-day retention buffer.
7. The PostgreSQL migration smoke test asserts that `auth_sessions` is present.

**Review state:** Latest GitHub Actions CI and Security workflows passed on 27 November 2026 (Rust backend check and unit tests, frontend build, dependency audit, and CodeQL). A follow-up patch now ensures Bearer-token sessions are revoked by logout and rejects malformed provider email addresses; the follow-up checks must complete before merge. Independent maintainer review and runtime session-revocation acceptance against PostgreSQL remain pending. Do not call this hardening merged or released until review and runtime acceptance pass. Existing 24-hour JWTs issued before migration 0014 lack a `jti`; after rollout, users must authenticate again.

## 4. Final acceptance checklist

Status labels describe what can be verified from the repository alone. “Not verified” means no successful external configuration or acceptance evidence was available during this repository review.

| Area | Status | Acceptance requirement |
|---|---|---|
| Non-AI core feature implementation | **Merged** | PRs #12, #20–#21, and #23–#37 are in `main`. |
| Main CI/security baseline | **Passed on reviewed HEAD** | Latest recorded CI and security runs passed on 24 Nov 2026; re-run on finalization branch before merge. |
| Session revocation | **CI passed; follow-up checks pending** | Verify login, OAuth login, cookie and Bearer logout revocation, rejection of revoked sessions, WebSocket closure, and cleanup against PostgreSQL. |
| Deployed API/database revision | **Not verified** | Record the currently running API commit and exact `_sqlx_migrations` versions before any database rollout. |
| Dedicated CampusForge Neon PostgreSQL | **Blocked by account scope / not provisioned** | The connected Neon organization is Vercel-managed and rejected direct project creation. Its sole existing project has a different application schema (`account`, `board`, `card`, etc.) and was deliberately not reused. Provision a separate CampusForge project through the authorized Vercel-managed Neon integration/account, then configure pooled `DATABASE_URL`, least-privilege credentials, region, connection limits, backups/PITR, and a successful restore drill. |
| OAuth provider applications | **Not verified** | Configure Google, GitHub, and Discord client IDs/secrets, exact HTTPS callbacks, consent/branding, and verified-email behavior in the real provider consoles. |
| Private file object storage | **Not verified** | Provision the private S3-compatible bucket, encryption, least-privilege keys, lifecycle, backup, and object/database reconciliation; confirm the API's actual PUT/GET/DELETE verification. |
| Malware scanning | **Not verified** | Run a reachable ClamAV service; enable mandatory scanning; prove clean files are available and infected/unscanned files are blocked. |
| Email delivery | **Not verified** | Configure Resend, a verified sender, SPF/DKIM/DMARC, opt-in, unsubscribe/suppression, bounce handling, and retry behavior; send and receive test messages. |
| Perimeter protection | **Not verified** | Configure trusted ingress/WAF rate limits and ensure proxy headers cannot be spoofed by clients. App-level auth rate limits do not protect every endpoint. |
| Two-account end-to-end acceptance | **Pending** | Test two separate accounts across board access, task moves, live presence/editing, threaded comments, direct-message isolation, invitations, calendar responses, notifications, file access, and member removal. |
| Browser/mobile acceptance | **Pending** | Test current Chrome/Safari/Firefox where supported, mobile breakpoints, keyboard navigation, screen readers, contrast, focus visibility, and empty/loading/error states. |
| Privacy, retention, export, deletion | **Pending** | Approve terms/privacy notice and retention periods; test user/workspace export, deletion, file deletion, chat/meeting deletion, and notification/email preferences. |
| Fine-grained resource authorization | **Known gap** | Complete per-board/channel permission hardening and test that non-members/viewers cannot read or mutate unauthorized board/channel resources. |
| Account recovery | **Known gap** | Password reset and authenticated provider-linking flows are not implemented; document a supported recovery path before broad launch. |
| Multi-workspace navigation | **Needs acceptance** | Verify workspace switching end-to-end; do not advertise multi-workspace switching until the selector and selected-workspace persistence work for multiple memberships. |
| Backup/restore and incident readiness | **Pending** | Restore PostgreSQL and object data into an isolated environment, verify application consistency, and document incident/credential rotation procedures. |
| Production deployment | **Withheld** | No deployment or merge to `main` until all required gates pass and the user explicitly authorizes release. |

## 5. Current preview warning

The README identifies the current Render preview as a test environment backed by a temporary free PostgreSQL database with a documented expiry of **22 December 2026**. It is not a production database or durability guarantee. The only connected Neon project inspected during this review belongs to a different application, so it was left untouched; direct creation of a dedicated CampusForge project was blocked because the Neon organization is managed by Vercel. Provision the intended project through that managed integration before moving the app. Verify the live services and database directly before any release; files in GitHub do not prove that the hosted database has the required migrations or that the running app matches source HEAD.

Because `render.yaml` uses commit-triggered auto-deploy from `main`, keep finalization work on its branch and do not merge it until the release is deliberately authorized. Do not change hosting resources or trigger a deployment as part of this checklist.

## 6. Release decision

**Track A source implementation:** substantial non-AI feature set merged.  
**Track A production acceptance:** **not complete**.  
**Track A release:** **not approved / not deployed**.  
**Track B:** **not started**.

Track A may be called full and final only after the branch hardening, independent review, and every required acceptance gate in Section 4 are evidenced. Track B must remain paused until that sign-off is complete.