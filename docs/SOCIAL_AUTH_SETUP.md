# CampusForge Teams — Social Sign-in Setup

CampusForge keeps email/password sign-in and adds server-side OAuth sign-in with Google, GitHub, and Discord. Provider client secrets stay on the Rust API server; they must never be placed in frontend environment variables or committed to Git.

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

- Existing email/password accounts can use a provider with the same verified email; the OAuth identity is attached to that account.
- New provider-only accounts are created with a NULL password hash; email/password login remains unchanged for password accounts.
- OAuth state is a high-entropy random value. Only its SHA-256 hash is stored, and it is deleted when used or allowed to expire after ten minutes.
- Provider access tokens are used only to fetch the user's verified identity and are not saved.
- The API issues the same 24-hour CampusForge session JWT for email/password and OAuth sign-in, but sends it only in an HttpOnly `campusforge_session` cookie. The token is not returned in the JSON body or placed in the redirect URL.
- The browser includes cookies on API requests. The API permits credentialed CORS only for the configured `FRONTEND_ORIGIN` and checks the Origin on state-changing cookie-authenticated requests.
- HTTPS deployments set the session cookie as `Secure; SameSite=None` for separately hosted frontend/API origins; local HTTP development uses `SameSite=Lax`.
- WebSocket upgrades authenticate from the HttpOnly session cookie; the frontend never sends the JWT in a WebSocket message.
- The client removes any old `campusforge-auth-token` localStorage entry on startup. Existing users on the new build must sign in once again to establish a cookie session.
- Do not commit provider secrets. Store them in your local server environment and the eventual hosting provider's encrypted environment variables.
- Before making the website public, configure HTTPS, exact callback URLs, the Google consent screen, a privacy policy, and terms of service.

## 6. Test checklist

1. Register and sign in using email/password.
2. Sign in with each configured provider using a previously unseen email.
3. Sign in with a second provider whose verified email belongs to an existing CampusForge user; confirm it uses the same CampusForge account.
4. Try a provider account with no verified email; confirm sign-in is rejected.
5. Replay an OAuth callback state value; confirm it is rejected.
6. Start OAuth in one browser and attempt to complete the callback in a different browser; confirm the browser-bound HttpOnly cookie check rejects it.
7. Try signing in with a provider whose verified email already belongs to a local CampusForge account that is not linked to that provider; confirm the server asks the user to authenticate with their existing method instead of silently linking accounts.
6. Confirm a successful first sign-in creates a default workspace when the workspace endpoint is opened.
7. Confirm missing provider secrets show a friendly message instead of a raw API error.


## Account-linking safety

OAuth state is bound to a short-lived, HttpOnly, SameSite=Lax cookie scoped to the API authentication routes. A callback without the matching browser cookie is rejected. Existing local accounts are not automatically linked to a new provider merely because the provider reports a verified email; sign in through the existing account method first. Provider linking through authenticated account settings is not implemented yet.
