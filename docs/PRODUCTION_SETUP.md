# CampusForge Teams — Production Setup

This runbook describes the merged Track A/B source and the current deployment boundary as of 28 November 2026. The main-branch Render frontend preview (`campusforge-preview`) and Rust API preview (`campusforge-api-test`) are both deployed `live` on commit `39fc48635d2d063ce016bb99d0d03637cbd2d46f`. The API is connected to the isolated Neon child branch/database `campusforge_release_preview` as `campusforge_release_runtime`; it is not connected to production `campusforge_app`. A separate, verified production Rust API service does not yet exist. Do not point the public site at the preview API or set `CAMPUSFORGE_ENV=production` until every gate in [Track A release acceptance](TRACK_A_FINAL_ACCEPTANCE.md) has evidence.

## 1. Core API settings

Configure these in the API service's secret/environment manager. Never commit credential values.

| Variable | Required value |
|---|---|
| `DATABASE_URL` | Neon PostgreSQL connection URL for `campusforge_app` using `campusforge_app_runtime` and `sslmode=verify-full`. Never use `neondb_owner` for the long-running API. The runtime role grants DML on all 43 application tables. |
| `CAMPUSFORGE_AUTO_MIGRATE` | Set to `false` on the long-running API service. Production defaults to manual migrations. |
| `JWT_SECRET` | Random high-entropy secret, at least 32 bytes. Do not reuse a development value. |
| `FRONTEND_ORIGIN` | Comma-separated list of exact frontend origins allowed by credentialed CORS. Production should list only the canonical HTTPS app origin; isolated previews may add their own exact HTTPS origins. Wildcards and URL paths are rejected. |
| `OAUTH_REDIRECT_BASE_URL` | Exact HTTPS origin of the API that receives OAuth callbacks. |

Set `FRONTEND_REDIRECT_ORIGIN` to the canonical frontend URL used after OAuth callbacks. When omitted, the server redirects to the first entry in `FRONTEND_ORIGIN`. The server appends the provider route to `OAUTH_REDIRECT_BASE_URL`. Register these exact callback URLs with each provider:

- Google: `https://<api-host>/api/auth/google/callback`
- GitHub: `https://<api-host>/api/auth/github/callback`
- Discord: `https://<api-host>/api/auth/discord/callback`

Set `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`, `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET`, `DISCORD_CLIENT_ID`, and `DISCORD_CLIENT_SECRET` in the API secret store. Use the public HTTPS API host in each provider console; do not add wildcard callbacks. Verify that each provider's email claim is verified before allowing sign-up and test both first-time and existing-account flows.

## 2. Durable file storage

Production uses S3-compatible private object storage. Configure:

- `CAMPUSFORGE_STORAGE_MODE=s3`
- `CAMPUSFORGE_REQUIRE_REMOTE_STORAGE=true`
- `CAMPUSFORGE_S3_BUCKET`
- `CAMPUSFORGE_S3_REGION`
- `AWS_ACCESS_KEY_ID`
- `AWS_SECRET_ACCESS_KEY`

Neon Object Storage is enabled on the CampusForge Neon project and the existing `campusforge-private-uploads` bucket is private. The isolated integrated preview has a child-branch-scoped `storage:read`/`storage:write` credential and Render startup has verified PUT/GET/DELETE. For production, create a separate credential on the production `main` branch with `storage:read` and `storage:write` scopes. Map the credential `token_id` to `AWS_ACCESS_KEY_ID` and `s3_secret_access_key` to `AWS_SECRET_ACCESS_KEY`. Use the `main` branch's own endpoint, region, and path-style addressing. Never reuse the preview credential in production. Optional S3-compatible provider settings are `CAMPUSFORGE_S3_ENDPOINT_URL`, `CAMPUSFORGE_S3_URL_STYLE=path`, and `AWS_SESSION_TOKEN` for temporary credentials.

The bucket must be private. Use a branch-scoped Neon credential limited to `storage:read` and `storage:write`; create separate credentials for production and preview. Neon Object Storage is branch-scoped and supports path-style S3 requests. Confirm the provider's supported retention/versioning and backup features rather than assuming standard AWS S3 features are available. Exercise upload, download, deletion, checksum and restore from the deployed API.

## 3. Mandatory malware scanning

Before production startup, configure a reachable ClamAV service and set:

- `CAMPUSFORGE_REQUIRE_MALWARE_SCAN=true`
- `CAMPUSFORGE_CLAMAV_ADDR` to the private scanner host and port reachable from the API service.

Do not expose the scanner to the public internet. Test clean, infected, oversized, timeout, unavailable-scanner and unscanned-file paths. Production must fail closed: files that have not passed scanning cannot be downloaded or included in AI context.

## 4. Email delivery

Set `RESEND_API_KEY` and `EMAIL_FROM`. Verify the sender/domain in Resend, configure SPF, DKIM and DMARC, and test invitation/notification delivery, bounced/suppressed addresses, retry behavior and outbox reconciliation. Keep provider credentials on the server only.

## 5. Project Pulse and optional AI assistant — disabled by default

Project Pulse is a workspace-scoped metrics feature and is independently gated. Keep both `CAMPUSFORGE_PROJECT_PULSE_ENABLED=false` on the API and `VITE_CAMPUSFORGE_PROJECT_PULSE_ENABLED=false` on the frontend for initial launch. Enable both only after the board-scoped analytics and two-account authorization tests pass. The server returns a forbidden response when disabled, so hiding the frontend control is not the only enforcement layer.

The AI assistant requires both `CAMPUSFORGE_AI_ENABLED` on the API and `VITE_CAMPUSFORGE_AI_ENABLED` at frontend build time. Both stay false by default, so the entry point is hidden and the API also rejects requests until provider and data-processing approvals.

Keep `CAMPUSFORGE_AI_ENABLED=false` for initial production release. The assistant must remain disabled until the organization approves its data processing, provider retention/training terms, subprocessor list, data residency, spending budget, request limits and user-facing disclosure.

Only after approval, configure:

- `CAMPUSFORGE_AI_ENABLED=true`
- `VITE_CAMPUSFORGE_AI_ENABLED=true`
- `CAMPUSFORGE_AI_API_BASE_URL` (HTTPS provider endpoint)
- `CAMPUSFORGE_AI_API_KEY` (server-only secret)
- `CAMPUSFORGE_AI_MODEL`

Provider requests require explicit data-sharing acknowledgement. Do not enable local HTTP providers with bearer credentials. Task plans are proposals: task creation is only possible after a separate authorized human approval and a fresh state/permission check.

## 6. Production startup gate

Set `CAMPUSFORGE_ENV=production` only after all of these are ready:

```text
CAMPUSFORGE_ENV=production
CAMPUSFORGE_STORAGE_MODE=s3
CAMPUSFORGE_REQUIRE_REMOTE_STORAGE=true
CAMPUSFORGE_REQUIRE_MALWARE_SCAN=true
CAMPUSFORGE_CLAMAV_ADDR=<private-clamav-host>:<port>
```

The API intentionally refuses production startup when remote storage or mandatory malware scanning is not configured. When malware scanning is mandatory, production startup performs a private ClamAV `zPING`/`zPONG` TCP probe with a five-second timeout; an unreachable or non-ClamAV endpoint prevents startup. A configured scanner address must be private and reachable from the API; do not expose clamd publicly. The live `campusforge-api-test` preview has `CAMPUSFORGE_STORAGE_MODE=s3`, `CAMPUSFORGE_REQUIRE_REMOTE_STORAGE=true`, a child-branch-only Neon storage credential, and successful PUT/GET/DELETE startup verification. The PR #45 Blueprint update makes these settings reproducible on future syncs. The preview still does not have ClamAV and keeps `CAMPUSFORGE_REQUIRE_MALWARE_SCAN=false`; production remains blocked until ClamAV is privately reachable and clean/infected/timeout/unavailable scanner paths pass.

## 7. Database release order

The Neon `campusforge_app` and isolated `campusforge_release_preview` databases both have SQLx migrations 0001–0017 recorded successful. Migration 0014 is the AI proposal/audit schema and must not be edited or renumbered. Migrations 0015–0017 are already applied; do not repeat them. The production database has a DML-only `campusforge_app_runtime` role with default privileges configured for future tables/sequences. The integrated preview API uses a separate `campusforge_release_runtime` role and isolated branch database.

For future schema releases:
1. Verify the intended Neon project/branch/database and a recoverable backup.
2. Pass the relevant integrated CI and security workflows.
3. Inject `MIGRATION_DATABASE_URL` from the protected one-off release secret store (never type it into shell history), with `sslmode=verify-full`, then run `npm run db:migrate`.
4. Verify `_sqlx_migrations` and the migration's expected data effects.
5. Deploy the matching API with `CAMPUSFORGE_AUTO_MIGRATE=false` and the DML-only runtime role.
6. Complete an isolated restore and rollback rehearsal before production launch.

Do not roll back to an API build that does not understand `auth_sessions` after migration 0015 is applied, or `notifications.board_id` after migration 0016 is applied. Follow [Database migration release order](DATABASE_MIGRATION_RELEASE_ORDER.md).

## 8. Final operational gates

Before public launch, additionally verify a production custom domain/TLS, trusted proxy/edge rate limiting, monitoring and alerting, backups and restore, incident response, privacy/export/deletion, account recovery/provider linking, accessibility, mobile/browser compatibility and two-account negative authorization tests. The canonical preview is `campusforge-preview` plus `campusforge-api-test`; both have a `live` deployment on main commit `39fc48635d2d063ce016bb99d0d03637cbd2d46f`. Render startup logs confirm the API uses the isolated `campusforge_release_preview` database, the DML-only `campusforge_release_runtime` role, and successful private object-storage PUT/GET/DELETE verification. The older `campusforge-teams` Node service also completed a main-branch deploy on that commit, correcting its previous `vite: not found` build failure, but it has not been accepted as the production topology or independently verified as the canonical frontend/API pair. A separate production Rust API service, production frontend/API connection, Google/GitHub/Discord credentials, Resend sender configuration, private ClamAV service, and production backups/restore acceptance remain required before calling the system production-ready.


Before applying migration 0016, confirm the matching API build writes `notifications.board_id` and checks it during inbox reads and email dispatch. Do not deploy the migration separately from that code.

## Database migration and runtime privilege separation

The API runtime must not connect as `neondb_owner` or another database/schema owner. Use a dedicated Neon role scoped to `campusforge_app` with only the table DML and sequence privileges the application requires. It must not have superuser, role-creation, database-creation, or RLS-bypass privileges.

Keep `CAMPUSFORGE_AUTO_MIGRATE=false` on the Render API service. This prevents ordinary service restarts and rollouts from attempting DDL with the application role. Run migrations as a separate release operation after injecting `MIGRATION_DATABASE_URL` from a protected one-off release secret store. Never type its value into a shell command or shell history. The URL must use `sslmode=verify-full`; do not configure this secret on the long-running API service:

```bash
npm run db:migrate
```

The migration command applies the repository's embedded SQLx migrations and checks their checksums. Use a controlled migration credential with schema ownership/DDL authority, keep its secret in a protected operator or release environment, and never copy it into application logs or frontend variables. Confirm the expected branch, database and migration journal before running it. The runtime API then starts with the separate read/write role and does not perform schema changes.


## 9. Backup and restore evidence (28 November 2026)

A manual snapshot of the production Neon `main` branch was created before further production changes:

- Snapshot: `campusforge-prelaunch-2026-11-28` (`snap-orange-grass-bbu0vwvb`).
- Restore drill: `campusforge-restore-drill-2026-11-28` (`br-bold-math-bbmdqptf`), restored with `finalize=false`; the original production branch was not replaced.
- The restored `campusforge_app` had 43 public tables and SQLx migrations 0001–0017 successful, with 48-byte checksums.
- The selected core data tables contained zero rows in the restored database. The legacy `neondb` core tables were also empty in the source branch during this check. This is schema/journal restore evidence, **not** proof of recovering populated user records.
- Neon snapshot schedule was empty before and after the manual snapshot; no recurring retention policy is configured. The restored drill branch could not be marked protected because the current Neon plan had reached its protected-branch limit.

Before launch, set and approve an automatic snapshot/PITR retention policy appropriate for the chosen Neon plan and verify restore against a populated test dataset. Do not claim customer-data recovery until a representative populated restore has been exercised.
