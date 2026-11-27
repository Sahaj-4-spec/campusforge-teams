# CampusForge Teams — Social Sign-in Setup

CampusForge creates new accounts only through server-side OAuth sign-in with Google, GitHub, or Discord. The API requires the provider to confirm that the account email is verified before it creates a CampusForge user. Any valid email domain is accepted when that provider confirms it is verified; Gmail is not required. Existing email/password accounts can still sign in, but password-only registration is disabled. Provider client secrets stay on the Rust API server; they must never be placed in frontend environment variables or committed to Git.

## 1. Configure the local environment

Copy the root .env.example to server/.env (or create a root .env used by the server) and fill in:

- DATABASE_URL
- JWT_SECRET (at least 32 random characters)
- FRONTEND_ORIGIN (for local Vite: http://localhost:5173)
- OAUTH_REDIRECT_BASE_URL (for local Axum: http://localhost:5000)
- GOOGLE_CLIENT_ID and GOOGLE_CLIENT_SECRET
- GITHUB_CLIENT_ID and GITHUB_CLIENT_SECRET
- DISCORD_CLIENT_ID and DISCORD_CLIENT_SECRET

The client starts OAuth at /api/auth/{provider}/start. For local Vite development, leave VITE_API_BASE_URL blank in client/.env; Vite proxies API traffic to localhost:5000. If the frontend and API are hosted on different origins, set VITE_API_BASE_URL to the public HTTPS API origin and set FRONTEND_ORIGIN to the frontend origin. The API callback URLs for local development are:

- Google: http://localhost:5000/api/auth/google/callback
- GitHub: http://localhost:5000/api/auth/github/callback
- Discord: http://localhost:5000/api/auth/discord/callback

Register each exact callback URL in that provider's developer console. For production, use the public HTTPS API host in OAUTH_REDIRECT_BASE_URL and register its exact callback URL.

## 2. Google

Create an OAuth 2.0 Client ID with application type Web application in Google Cloud Console. Add the exact callback URL under Authorized redirect URIs. Configure the OAuth consent screen and authorized domains before opening the app to public users. Google sign-in requests only openid, email, and profile; the API requires Google's email_verified result.

## 3. GitHub

Create an OAuth App in GitHub Developer Settings. Set its Authorization callback URL to the exact API callback. CampusForge requests read:user and user:email; the API selects a primary verified email, or another verified email if no primary verified email is available.

## 4. Discord

Create an application in the Discord Developer Portal and configure the OAuth2 redirect URL to the exact API callback. CampusForge requests only identify and email and requires Discord to return a verified email.

## 5. Account behavior and security

- New self-service accounts can be created only by completing Google, GitHub, or Discord OAuth with a provider-verified email address. `POST /api/auth/register` is retained for compatibility but always returns `403 Forbidden`.
- The API records `users.email_verified_at` when a verified provider identity creates the account. Existing provider accounts are backfilled from their OAuth account creation timestamp.
- Migration 0009 adds database-level guards: inserts with a password hash are rejected, and a new passwordless user cannot commit without an OAuth identity row in the same transaction. This also blocks password registrations from an older API instance during a rolling deployment; older OAuth-capable binaries are covered by a database trigger that fills the verification timestamp.
- Existing email/password accounts can continue to sign in, but they are not automatically linked to a provider, even when the verified email matches. Users must sign in through the existing account method first.
- New provider-only accounts have a NULL password hash; email/password login remains available for existing password accounts.
- OAuth state is a high-entropy random value. Only its SHA-256 hash is stored, and it is deleted when used or allowed to expire after ten minutes.
- Provider access tokens are used only to fetch the user's verified identity and are not saved.
- The API issues the same 24-hour CampusForge session JWT for email/password and OAuth sign-in, but sends it only in an HttpOnly `campusforge_session` cookie. The token is not returned in the JSON body or placed in the redirect URL.
- The Track A finalization branch adds a random session ID in the JWT `jti` claim and stores only that ID, user ID, and expiry in PostgreSQL. Protected requests require an active row; logout revokes it. WebSocket connections re-check the session during their heartbeat. After migration 0014 is released, pre-0014 JWTs without a `jti` must be replaced by signing in again.
- The browser includes cookies on API requests. The API permits credentialed CORS only for the configured `FRONTEND_ORIGIN` and checks the Origin on state-changing cookie-authenticated requests.
- HTTPS deployments set the session cookie as `Secure; SameSite=None` for separately hosted frontend/API origins; local HTTP development uses `SameSite=Lax`.
- WebSocket upgrades authenticate from the HttpOnly session cookie; the frontend never sends the JWT in a WebSocket message.
- The client removes any old `campusforge-auth-token` localStorage entry on startup. Existing users on the new build must sign in once again to establish a cookie session.
- Do not commit provider secrets. Store them in your local server environment and the eventual hosting provider's encrypted environment variables.
- Before making the website public, configure HTTPS, exact callback URLs, the Google consent screen, a privacy policy, and terms of service.

## 6. Test checklist

1. Try `POST /api/auth/register`; confirm it rejects password-only account creation with `403 Forbidden`.
2. Create a new account with each configured provider using a previously unseen email; confirm the account is created only after the provider reports the email as verified.
3. Sign out and sign in again with each provider.
4. Attempt sign-in with a second provider whose verified email belongs to an existing CampusForge user; confirm automatic linking is rejected and the user is instructed to sign in through the existing method.
5. Try a provider account with no verified email; confirm sign-in is rejected.
6. Replay an OAuth callback state value; confirm it is rejected.
7. Start OAuth in one browser and attempt to complete the callback in a different browser; confirm the browser-bound HttpOnly cookie check rejects it.
8. Confirm a successful first sign-in creates a default workspace when the workspace endpoint is opened.
9. Confirm missing provider secrets show a friendly message instead of a raw API error.
10. Apply migration 0009 and attempt a direct insert with a non-NULL password hash; confirm PostgreSQL rejects it.
11. Attempt a passwordless user insert without an `oauth_accounts` row; confirm transaction commit fails.
12. Create a passwordless user and its OAuth identity in the same transaction without supplying `email_verified_at`; confirm the database trigger sets the timestamp and commit succeeds.
13. After migration 0014 is applied, sign in and confirm a corresponding `auth_sessions` row is created; call logout and confirm the row is marked revoked and later protected API requests reject the old cookie/JWT.
14. Keep a WebSocket open, revoke its session from another request, and confirm the socket closes on the next 30-second session heartbeat.
15. Confirm session cleanup removes expired/revoked records only after the seven-day retention buffer.

## 7. Deployment order and rollback

The historical PR #20 then PR #21 release order has already been completed in GitHub: PR #20 merged the SQLx rollback-compatible migration runner, and PR #21 merged verified-provider sign-up plus the remaining non-AI roadmap implementation. PR #19 was not merged separately because its verified sign-up change was included in PR #21.

The source repository contains migrations 0001–0013 on `main`. This does **not** prove that the current Render database has applied those migrations; verify the actual deployed revision and SQLx migration history before planning a rollout. The Track A finalization branch adds migration `0014_revocable_auth_sessions.sql` and the matching application changes. It is not yet merged or deployed.

For a future controlled release:
1. Confirm the currently deployed API version and database migration history.
2. Preserve SQLx rollback compatibility; do not roll back to a pre-PR #20 binary after migration 0009 has been applied.
3. Apply migration 0014 only with the matching application code after its branch CI/review passes.
4. Expect a one-time re-authentication for sessions issued before migration 0014, because old JWTs do not contain a server-recorded `jti`.
5. Verify all three OAuth callbacks, existing password login, direct registration rejection, database account-insert guards, session revocation, WebSocket disconnect, private file scanning/storage, and two-user rollout behavior before opening sign-up to the public.

The repository's Render Blueprint currently auto-deploys commits pushed to `main`. Do not merge to `main` or deploy until the user explicitly authorizes release and the production gates in [Track A Final Acceptance](TRACK_A_FINAL_ACCEPTANCE.md) are met.

See [Database Migration Release Order](DATABASE_MIGRATION_RELEASE_ORDER.md) for the compatibility rationale and expand/contract policy.
 
## Account-linking safety

OAuth state is bound to a short-lived, HttpOnly, SameSite=Lax cookie scoped to the API authentication routes. A callback without the matching browser cookie is rejected. Existing local accounts are not automatically linked to a new provider merely because the provider reports a verified email; sign in through the existing account method first. Provider linking through authenticated account settings is not implemented yet.
