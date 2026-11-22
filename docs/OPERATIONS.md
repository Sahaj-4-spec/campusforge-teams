# CampusForge Teams — Storage and Notification Operations

This guide covers the configurable file storage adapter and optional transactional email delivery added to the collaboration-suite branch. These services are server-side integrations. Do not add their secrets to Vite variables, commit them, or expose them to the browser.

## 1. File storage

### Local development

The default is local mode:

- CAMPUSFORGE_STORAGE_MODE=local
- CAMPUSFORGE_UPLOAD_DIR=server/uploads

The directory is ignored by Git. Local mode is for localhost development only; local container disks are not assumed to survive restarts, replacement, or multiple instances.

### Production S3-compatible storage

The backend uses the AWS SDK for Rust to connect to Amazon S3 or an S3-compatible service. AWS credentials are resolved by the SDK's standard credential provider chain. Prefer an IAM role or platform workload identity; otherwise provide AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY as encrypted server environment variables.

Set the following only on the backend service:

| Variable | Required | Purpose |
|---|---|---|
| CAMPUSFORGE_STORAGE_MODE | Yes | Set to s3 to select object storage |
| CAMPUSFORGE_S3_BUCKET | Yes | Private bucket name |
| CAMPUSFORGE_S3_REGION | Yes | Bucket region (defaults to us-east-1) |
| CAMPUSFORGE_S3_ENDPOINT_URL | No | Endpoint URL for compatible services such as R2 or MinIO; path-style addressing is enabled when set |
| AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY | Conditional | Use only when the platform does not provide an IAM-compatible role |

Bucket configuration requirements:

1. Keep the bucket private; disable public access and public ACLs.
2. Enable server-side encryption and versioning where supported.
3. Grant the API only the minimum object permissions needed for the configured bucket/prefix: PutObject, GetObject, and DeleteObject.
4. Do not configure browser CORS access to the bucket. Browser uploads/downloads go through the authenticated CampusForge API, which checks workspace membership.
5. Configure lifecycle, retention, backup, and restore policies appropriate to team documents.
6. Verify the service identity, bucket policy, and region before switching the application to S3 mode.

The application creates opaque object keys and stores file metadata/checksums in PostgreSQL. The download route checks workspace membership and streams the object through the API. It returns attachment disposition and X-Content-Type-Options: nosniff. Uploads are capped at 10 MB and currently limited by filename extension to common documents, images, compressed archives, and source/text files.

**Still required before public release:** malware scanning, per-workspace storage quotas, retention/deletion policy approval, object/database reconciliation, backup/restore exercises, and an S3-compatible integration test against the chosen provider.

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

The API integration uses Resend's POST /emails endpoint with bearer authentication. The outbox uses a notification-specific idempotency key. Failures are retried with exponential backoff and are marked failed after five attempts. The sender uses plain text; no untrusted message body is inserted into HTML.

See the Resend email API documentation: https://resend.com/docs/send-with-nodejs

**Still required before public release:** provider account/domain setup, a successful test email to controlled inboxes, SPF/DKIM/DMARC review, delivery/bounce monitoring, suppression/unsubscribe policy, and privacy/retention review.

## 3. Database and multi-instance considerations

- notification_email_outbox records delivery work independently of WebSocket delivery.
- Notifications are committed to PostgreSQL before their WebSocket update signal is published.
- The email worker claims queue records with FOR UPDATE SKIP LOCKED and uses retries.
- The current app's WebSocket broadcast channel is in-process. Before horizontally scaling the API, use a shared pub/sub transport and run the email/recurrence workers as a single coordinated worker deployment or use database-safe claiming as implemented.
- Back up PostgreSQL and object storage together. A database restore without the corresponding object set can leave unavailable file metadata, and an object restore without metadata can leave orphaned files.

## 4. Local verification

From the repository root, install dependencies with npm install and npm --prefix client install, copy .env.example to .env, keep CAMPUSFORGE_STORAGE_MODE=local, leave RESEND_API_KEY and EMAIL_FROM empty until a provider is configured, and run npm run dev.

The GitHub Actions backend job includes a PostgreSQL service and a migration smoke test. It verifies the collaboration tables are created by the SQLx migration runner. The test runs automatically in CI when DATABASE_URL is supplied.
