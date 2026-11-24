# CampusForge Teams — Storage and Notification Operations

This guide covers the configurable file storage adapter and optional transactional email delivery added to the collaboration-suite branch. These services are server-side integrations. Do not add their secrets to Vite variables, commit them, or expose them to the browser.

## 1. File storage

### Local development

The default is local mode:

- CAMPUSFORGE_STORAGE_MODE=local
- CAMPUSFORGE_UPLOAD_DIR=server/uploads

The directory is ignored by Git. Local mode is for localhost development only; local container disks are not assumed to survive restarts, replacement, or multiple instances.

### Production Neon Object Storage

The backend uses the S3-compatible API, so Neon Object Storage can provide durable private file storage alongside the Neon PostgreSQL project. Neon Object Storage is branch-aware: use the production project's main branch for the live Render API, and keep development/test branches separate.

Provision a private bucket named `uploads` on the production branch. Create a Neon storage credential with the `storage:write` scope (which includes the read operations required by this adapter). Neon Object Storage access and bucket visibility are managed through Neon; do not try to manage bucket ACLs or public access through S3 ACL APIs.

Set these values only on the Render API service:

| Variable | Required | Purpose |
|---|---|---|
| CAMPUSFORGE_STORAGE_MODE | Yes | Set to `s3` to select object storage |
| CAMPUSFORGE_REQUIRE_REMOTE_STORAGE | Yes | Set to `true` so the API refuses to start if it falls back to local disk |
| CAMPUSFORGE_S3_BUCKET | Yes | Set to the private bucket name, for example `uploads` |
| AWS_ENDPOINT_URL_S3 | Yes | Neon branch's S3 endpoint from the Neon console or `neon env pull` |
| AWS_REGION | Yes | Region supplied by Neon for the storage endpoint |
| AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY | Yes | Neon storage credential; add as encrypted Render backend secrets |
| AWS_SESSION_TOKEN | No | Session token if the selected provider issues temporary credentials |
| CAMPUSFORGE_S3_URL_STYLE | No | Set to `virtual-host` only when the selected provider requires bucket-prefixed hostnames |

The adapter also supports `CAMPUSFORGE_S3_ENDPOINT_URL` and `CAMPUSFORGE_S3_REGION` as explicit overrides for non-Neon S3-compatible providers. Never put these credentials into Vite variables, browser code, the repository, or logs.

Bucket and access requirements:

1. Keep the `uploads` bucket private. Files must be read through the authenticated CampusForge API, which checks workspace membership.
2. Use a storage credential scoped to the production branch and the minimum scope required. Rotate it if it is exposed.
3. Use the endpoint/region generated for the same production branch as `DATABASE_URL`; do not mix a Neon database branch with another branch's object-store endpoint.
4. Do not configure public bucket access or browser-direct uploads. The app streams uploads and downloads through the API.
5. Neon Object Storage manages its own object version lineage with the database branch; the S3 adapter does not configure S3 lifecycle/versioning policies. For a different S3 provider, configure provider-supported retention and deletion policies explicitly.
6. Configure retention, backup, and restore policies appropriate to team documents.

On API startup, the adapter runs a short-lived private-object PUT/GET/DELETE check and fails startup if the operation cannot complete. In production, `CAMPUSFORGE_REQUIRE_REMOTE_STORAGE=true` prevents an accidental fallback to local ephemeral storage.

The application creates opaque object keys and stores file metadata/checksums in PostgreSQL. The download route checks workspace membership and streams the object through the API. It returns attachment disposition and X-Content-Type-Options: nosniff. Uploads are capped at 10 MB and currently limited by filename extension to common documents, images, compressed archives, and source/text files.

**Still required before public release:** malware scanning, retention/deletion policy approval, object/database reconciliation, backup/restore exercises, and a provider integration test using the production Neon branch.

## 2. Email notifications

### Provider

The backend contains a Resend API adapter. It sends plain-text transactional messages from a persistent PostgreSQL outbox. Email is opt-in per notification category and is disabled until both backend configuration values are present.

Set these on the backend service:

| Variable | Purpose |
|---|---|
| RESEND_API_KEY | Server-only Resend sending API key |
| EMAIL_FROM | Sender address on a verified domain, for example CampusForge <notifications@example.edu> |

Before enabling email:

1. Create/configure the Resend account.
2. Verify a sender domain and configure the DNS records required by the provider.
3. Create a least-privilege sending API key.
4. Add the API key and verified sender to the backend environment.
5. Restart the backend and confirm the notification preferences endpoint reports email delivery as available.
6. In Notification Centre, enable Email for the desired categories. Email is not enabled for a member by default.

The API integration uses Resend's POST /emails endpoint with bearer authentication. The outbox uses a notification-specific idempotency key. Failures are retried with exponential backoff and are marked failed after five attempts. Expired ambiguous sends outside the provider’s 24-hour idempotency retention window move to `reconciliation` and must be checked against provider delivery records before any manual retry. The sender uses plain text; no untrusted message body is inserted into HTML.

See the Resend email API documentation: https://resend.com/docs/send-with-nodejs

**Still required before public release:** provider account/domain setup, a successful test email to controlled inboxes, SPF/DKIM/DMARC review, delivery/bounce monitoring, suppression/unsubscribe policy, and privacy/retention review.

## 3. Database and multi-instance considerations

- notification_email_outbox records delivery work independently of WebSocket delivery.
- Notifications are committed to PostgreSQL before their WebSocket update signal is published.
- The email worker claims queue records with FOR UPDATE SKIP LOCKED and uses retries.
- Workspace update events are relayed between API instances through PostgreSQL LISTEN/NOTIFY and delivered locally through each instance's broadcast channel. Keep the database connection pool sized for an additional listener connection per API instance.
- Reminder and recurrence workers use database row locking/skip-locked claims, and recurrence generation writes a unique run ledger. Email and file-deletion workers also use durable database outboxes. Monitor background-job errors and queue age across instances.
- Back up PostgreSQL and object storage together. A database restore without the corresponding object set can leave unavailable file metadata, and an object restore without metadata can leave orphaned files.

## 4. Local verification

From the repository root, install dependencies with npm install and npm --prefix client install, copy .env.example to .env, keep CAMPUSFORGE_STORAGE_MODE=local, leave RESEND_API_KEY and EMAIL_FROM empty until a provider is configured, and run npm run dev.

The GitHub Actions backend job includes a PostgreSQL service and a migration smoke test. It verifies the collaboration tables are created by the SQLx migration runner. The test runs automatically in CI when DATABASE_URL is supplied.


## 5. Retention, notification opt-out, and object deletion

- Turning off an in-app notification category immediately removes that category from the inbox and unread count; saved notification rows remain so they can be shown again if the member re-enables the category.
- Turning off an email category suppresses queued and processing outbox rows for that category. The worker rechecks the current preference and claimed outbox attempt before sending. A provider request already in flight when the member changes the setting cannot be recalled.
- Every workspace file metadata deletion that has not already confirmed object removal writes its opaque storage key to object_deletion_outbox in the same database transaction. The trigger also runs for task, board, and workspace cascades, so the cleanup worker retains a durable reference to orphaned object bytes.
- The cleanup worker retries failed object deletion with backoff. The outbox row is removed only after the storage adapter confirms deletion (including an already-missing object). Uploads also create a delayed cleanup intent before writing object bytes; metadata creation and intent removal commit together, so interrupted or partially failed uploads remain eligible for cleanup.

## 6. Malware scanning and file governance

### Production startup gate

Set `CAMPUSFORGE_ENV=production` only after all production dependencies below are ready. In this mode the API now refuses to start unless all of these are true:

- `CAMPUSFORGE_STORAGE_MODE=s3`
- `CAMPUSFORGE_REQUIRE_REMOTE_STORAGE=true`
- `CAMPUSFORGE_REQUIRE_MALWARE_SCAN=true`
- `CAMPUSFORGE_CLAMAV_ADDR` points to a reachable private ClamAV TCP service

This is intentionally fail-closed: do not enable production mode on the current preview service until the S3 bucket/credentials and scanner are provisioned and tested. Configure the production API and a staging API separately.

Before public traffic, also configure an ingress/WAF rate limit for login, OAuth start/callback, and invitation endpoints. The application currently does not provide a shared distributed per-IP rate limiter; do not rely on a process-local limiter when running multiple instances. Ensure the ingress overwrites client-IP forwarding headers and never trusts user-supplied values.

### ClamAV scanner configuration

The upload API performs file signature checks before storage. It can stream uploaded bytes to a ClamAV daemon using the INSTREAM protocol. Configure the following only on the Rust API service:

| Variable | Purpose |
|---|---|
| `CAMPUSFORGE_CLAMAV_ADDR` | Private TCP address of the ClamAV daemon, for example `clamav.internal:3310` |
| `CAMPUSFORGE_REQUIRE_MALWARE_SCAN` | Set to `true` to reject uploads when scanning is not configured or the scan result is not clean |

The scanner endpoint must only be reachable from the API's private network. Do not expose the ClamAV daemon directly to the public internet. Set reasonable CPU/memory limits and keep virus signatures updated. Configure the scanner to allow the API's 10 MB maximum upload size.

When the require-scan flag is false and no scanner is configured, uploaded files are marked `unscanned`. This mode is intended for local development and testing only. With the flag enabled, the API rejects uploads unless ClamAV returns a clean result; downloads and previews of files without a clean scan status are also blocked.

### Workspace quota and audit

- New workspaces receive a default 100 MiB quota.
- Owners/admins can raise or lower the quota between 10 MiB and 100 GiB; a quota cannot be reduced below current file usage plus pending upload reservations.
- Upload reservations are transactionally accounted under a workspace lock to prevent concurrent uploads from exceeding the quota.
- File uploads, previews, downloads, queued deletions and completed deletions are recorded in the workspace file audit history.
- Owners/admins can inspect the audit history. The file uploader can delete their own file; workspace owners/admins can also delete files within the workspace.
- Only PDFs and raster images are exposed through the inline preview route. Other supported file types must be downloaded as attachments. Responses include no-store, nosniff and a restrictive content security policy.

Before public launch, test clean and infected samples, scanner unavailability, timeouts, quotas during concurrent uploads, interrupted uploads, deletion retries, S3 object lifecycle and restore behavior.

