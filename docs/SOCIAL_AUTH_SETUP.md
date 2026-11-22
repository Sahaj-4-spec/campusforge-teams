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

The client starts OAuth at /api/auth/oauth/{provider}/start. The API callback URLs for local development are:

- Google: http://localhost:5000/api/auth/oauth/google/callback
- GitHub: http://localhost:5000/api/auth/oauth/github/callback
- Discord: http://localhost:5000/api/auth/oauth/discord/callback

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
- The API issues the same 24-hour CampusForge bearer session token used by email/password login.
- Do not commit provider secrets. Store them in your local server environment and the eventual hosting provider's encrypted environment variables.
- Before making the website public, configure HTTPS, exact callback URLs, the Google consent screen, a privacy policy, and terms of service.

## 6. Test checklist

1. Register and sign in using email/password.
2. Sign in with each configured provider using a previously unseen email.
3. Sign in with a second provider whose verified email belongs to an existing CampusForge user; confirm it uses the same CampusForge account.
4. Try a provider account with no verified email; confirm sign-in is rejected.
5. Replay an OAuth callback state value; confirm it is rejected.
6. Confirm a successful first sign-in creates a default workspace when the workspace endpoint is opened.
7. Confirm missing provider secrets show a friendly message instead of a raw API error.
