# CampusForge Teams — Production Setup

This runbook is for the integrated release candidate. Do not set `CAMPUSFORGE_ENV=production` until every gate in [Track A release acceptance](TRACK_A_FINAL_ACCEPTANCE.md) has evidence.

## 1. Core API settings

Configure these in the API service's secret/environment manager. Never commit credential values.

| Variable | Required value |
|---|---|
| `DATABASE_URL` | Neon PostgreSQL connection URL for the dedicated `campusforge_app` database. Use TLS and a least-privilege application role. |
| `JWT_SECRET` | Random high-entropy secret, at least 32 bytes. Do not reuse a development value. |
| `FRONTEND_ORIGIN` | Exact HTTPS origin of the deployed frontend, without a trailing path. |
| `OAUTH_REDIRECT_BASE_URL` | Exact HTTPS origin of the API that receives OAuth callbacks. |

The server appends the provider route to `OAUTH_REDIRECT_BASE_URL`. Register these exact callback URLs with each provider:

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

Optional S3-compatible provider settings are `CAMPUSFORGE_S3_ENDPOINT_URL`, `CAMPUSFORGE_S3_URL_STYLE` (set to `virtual-host` only when required), and `AWS_SESSION_TOKEN` for temporary credentials.

The bucket must be private. Use a least-privilege identity restricted to the application bucket and required object operations. Enforce encryption at rest and TLS in transit, enable provider-side versioning/retention where appropriate, configure lifecycle rules, and document backup/restore procedures. Verify denied public access and exercise upload, download, deletion, checksum, and restore from the deployed API.

## 3. Mandatory malware scanning

Before production startup, configure a reachable ClamAV service and set:

- `CAMPUSFORGE_REQUIRE_MALWARE_SCAN=true`
- `CAMPUSFORGE_CLAMAV_ADDR` to the private scanner host and port reachable from the API service.

Do not expose the scanner to the public internet. Test clean, infected, oversized, timeout, unavailable-scanner and unscanned-file paths. Production must fail closed: files that have not passed scanning cannot be downloaded or included in AI context.

## 4. Email delivery

Set `RESEND_API_KEY` and `EMAIL_FROM`. Verify the sender/domain in Resend, configure SPF, DKIM and DMARC, and test invitation/notification delivery, bounced/suppressed addresses, retry behavior and outbox reconciliation. Keep provider credentials on the server only.

## 5. Project Pulse and optional AI assistant — disabled by default

Project Pulse is a workspace-scoped metrics feature and is independently gated. Keep both `CAMPUSFORGE_PROJECT_PULSE_ENABLED=false` on the API and `VITE_CAMPUSFORGE_PROJECT_PULSE_ENABLED=false` on the frontend for initial launch. Enable both only after the board-scoped analytics and two-account authorization tests pass. The server returns a forbidden response when disabled, so hiding the frontend control is not the only enforcement layer.

The AI assistant is independently controlled by `CAMPUSFORGE_AI_ENABLED` and remains disabled until provider and data-processing approvals.

Keep `CAMPUSFORGE_AI_ENABLED=false` for initial production release. The assistant must remain disabled until the organization approves its data processing, provider retention/training terms, subprocessor list, data residency, spending budget, request limits and user-facing disclosure.

Only after approval, configure:

- `CAMPUSFORGE_AI_ENABLED=true`
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

The API intentionally refuses production startup when remote storage or mandatory malware scanning is not configured.

## 7. Database release order

The Neon `campusforge_app` database currently has SQLx migrations 0001–0014 recorded successfully. Migration 0014 is the AI proposal/audit schema and must not be edited or renumbered. Migration 0015 adds revocable authentication sessions.

1. Verify SQLx SHA-384 migration journal checksums and a database backup.
2. Pass the integrated CI and security workflows.
3. Apply migration 0015 with the session-aware API release.
4. Verify password login, Google/GitHub/Discord sign-in, cookie and bearer logout, session revocation, WebSocket termination and old-token reauthentication.
5. Complete isolated database restore and two-account authorization tests.

Do not roll back to an API build that does not understand `auth_sessions` after migration 0015 is applied. Follow [Database migration release order](DATABASE_MIGRATION_RELEASE_ORDER.md).

## 8. Final operational gates

Before public launch, additionally verify a production custom domain/TLS, trusted proxy/edge rate limiting, monitoring and alerting, backups and restore, incident response, privacy/export/deletion, account recovery/provider linking, accessibility, mobile/browser compatibility and two-account negative authorization tests. The preview/test Render services are not a production environment.
