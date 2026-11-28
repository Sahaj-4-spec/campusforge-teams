# Database migration and rollback release order

Hosted CampusForge API processes use a least-privilege runtime database role and do not apply DDL at startup. Local/non-production development can opt into automatic migrations. Production schema changes are run separately with the protected `MIGRATION_DATABASE_URL` using `npm run db:migrate`. Keep database changes compatible with the application versions that may be live during a rolling deploy and with the last rollback target.

> **Current status — 28 November 2026:** PR #41 (Track A/B integration) and PR #43 (runtime database/migration separation) are merged to `main`. Neon `campusforge_app` and the isolated `campusforge_release_preview` database both have successful SQLx migrations 0001–0017; all recorded checksums are 48-byte SQLx SHA-384 values. Migrations 0015–0017 are already applied; do not run or edit them again. The hosted integrated preview API uses `campusforge_release_runtime` on the child branch and has `CAMPUSFORGE_AUTO_MIGRATE=false`. The production database has a separate `campusforge_app_runtime` DML-only role, and default privileges for future tables/sequences are configured, but no production Rust API service has been attached to it yet. The legacy `campusforge-teams` Render Node service still has a failed deploy (`vite: not found`). Production launch remains blocked by the missing production API rollout, OAuth/email provider configuration, mandatory private ClamAV service and full acceptance/restore checks.

## SQLx rollback compatibility

The API migration runner sets `ignore_missing` so a binary that contains an older, still-compatible set of migrations can start when the database has additional forward migrations. SQLx continues to check checksums for migration versions that are present in the binary.

This is not permission to make arbitrary schema changes. Use expand/contract migrations:

1. Add nullable columns, tables, indexes, or database guards without removing or renaming objects used by currently running code.
2. Deploy application code that understands the expanded schema.
3. Switch all traffic to the new application version and verify it.
4. Remove obsolete schema only in a later release, after the rollback window has closed.

Never edit an applied migration file; add a new migration instead.

## Required order for provider-verified sign-up

The provider-only sign-up change adds migration `0009_verified_social_signup.sql`. Use this order:

1. Merge and deploy the SQLx rollback-compatibility bridge first. This release changes only the migration runner and documentation; it does not apply migration 0009.
2. Confirm all API instances are running the bridge release.
3. Then merge and deploy the provider-verified sign-up change. Migration 0009 adds the verification timestamp and database-level insert guards.
4. Verify Google, GitHub, and Discord sign-up, existing password login, rejected password-only registration, and old/new API overlap behavior.
5. Keep the bridge release as the rollback target. Do not roll back to a binary older than the bridge after migration 0009 has been applied.

The database guard in migration 0009 blocks password-hash account inserts even if an older API instance temporarily remains active during the rollout. It also requires a new passwordless user to have a corresponding OAuth identity before the transaction can commit.

## Board presence and threaded task comments

Migration `0011_board_presence_and_threaded_comments.sql` is an additive expansion. It adds the current board to each connection-scoped presence lease, an index for active viewers per board, and an optional parent-comment link for one-level task discussion threads. Apply it by deploying the API version that contains migration 0011; do not edit or rewrite already-applied migrations. The preceding workspace-level presence rows remain valid with a null board until each active client selects a board.

The application broadcasts ephemeral board-view and card-editing signals over the authenticated WebSocket. Durable card/list/comment/activity changes remain in PostgreSQL and are delivered to other clients through the existing workspace event refresh path.

## Revocable sessions (migration 0015)

The merged release adds `0015_revocable_auth_sessions.sql` after the already-applied Track B migration 0014:

- `auth_sessions.id` stores a random UUID referenced by the JWT `jti` claim.
- The table records the owner, expiry, and optional `revoked_at` timestamp; it never stores raw JWTs.
- Protected API calls require an active session row. Logout marks the row revoked.
- WebSocket handshakes validate the session and connected sockets re-check it every 30 seconds.
- Expired/revoked rows are cleaned after a seven-day retention buffer.

Migrations 0015, 0016, and 0017 are applied on both the live `campusforge_app` database and the isolated preview database. Do not reapply them. All existing JWTs issued before migration 0015 lack `jti`; users must authenticate again after the first session-aware rollout. For future schema changes, verify a database backup/restore point, then run the protected one-off migration command against the intended Neon branch and database before deploying the matching API binary. Do not infer that a migration has run merely because its file exists in GitHub.

## Rollback

For migrations 0009–0013, roll back only to an application version that includes the SQLx compatibility bridge, not to a pre-bridge binary. The bridge runner tolerates later additive schema while preserving already-applied database guards. Do not manually delete migration-history rows or drop triggers/columns as part of an application rollback.

**After migration 0015, do not roll back to an authentication binary that does not enforce `auth_sessions`.** Such a binary can verify the JWT signature but will ignore the `jti` session record and therefore cannot enforce logout revocation. Keep a known-good session-aware build as the rollback target. In an emergency where a pre-0015 binary must be used, rotate `JWT_SECRET` before sending traffic to it (which invalidates all existing JWTs), require every user to sign in again, and treat revocation enforcement as unavailable until the session-aware release is restored. Do not run a mixed fleet where only some instances enforce revocation.

If a later change needs a destructive schema contraction, ship it only after the rollback window and older binary support have deliberately ended.


The connected Neon `campusforge_app` database and isolated `campusforge_release_preview` database both record successful SQLx migration versions 0001–0017. Version 14 is `ai_action_proposals` and remains unchanged. Version 15 adds revocable sessions; version 16 adds `notifications.board_id`; version 17 preserves proposal history after target-list deletion. The legacy `neondb` database is separate and was preserved. A DML-only `campusforge_app_runtime` role has `SELECT/INSERT/UPDATE/DELETE` on all 43 application tables, cannot create roles/databases/schemas or bypass RLS, and has default table/sequence privileges configured for objects created by `neondb_owner`. The preview service uses the correspondingly restricted `campusforge_release_runtime` role.


SQLx calculates migration checksums with SHA-384 (48 bytes). The Neon `campusforge_app` journal was initially populated with 32-byte SHA-256 digests, which caused the API's `MigrateError::VersionMismatch(1)` at startup. The journal entries 0001–0014 were corrected using the exact SQL source and SQLx descriptions; migration files and application data were not modified. Verify the 48-byte SQLx SHA-384 checksum format for any new migration. Do not rewrite already-applied journal rows.


## Board-scoped notification security (migration 0016)

Migration `0016_notification_board_scope.sql` adds `notifications.board_id`, backfills it for extant task notifications whose tasks still exist, and indexes recipient/board/created time. The application writes board scope for every board activity notification and restricts recipients, inbox list/unread/read actions, and email dispatch to explicit board members or workspace owners/admins. Workspace invitations remain a separate invitation-validated path. Legacy task notifications that cannot be mapped to a surviving board are hidden and suppressed rather than exposed without an authorization scope.


## Proposal-history retention (migration 0017)

Migration `0017_preserve_ai_proposal_history.sql` makes `ai_action_proposals.target_list_id` nullable and changes its composite foreign key to `ON DELETE SET NULL (target_list_id)`. This preserves proposal task details, applied task IDs, and audit linkage if an otherwise deletable column is removed. The application blocks deletion only while a proposal is still pending and unexpired; a deleted target cannot be used for later approval. Migration 0014 remains unchanged.

Apply migration 0017 only after migrations 0015 and 0016, with the application version that models `target_list_id` as nullable and rejects applying proposals whose target column no longer exists. Verify both cases in the migration/integration suite: deleting a column with completed proposal history preserves the proposal; deleting a column with a pending unexpired proposal is rejected by the application guard.


## Hosted migration command

The long-running API uses `DATABASE_URL` with the DML-only runtime role and keeps `CAMPUSFORGE_AUTO_MIGRATE=false`. A release operator runs schema migrations separately with a protected migration owner credential:

```bash
# Inject MIGRATION_DATABASE_URL from the protected release secret store.
# The secret URL must use sslmode=verify-full and the canonical Neon hostname.
npm run db:migrate
```

Configure `MIGRATION_DATABASE_URL` only in the protected one-off release environment/secret store; never type its value into a shell command or place it in shell history. Do not configure it on the public API service, expose it to the frontend, or log it. The PostgreSQL URL must use `sslmode=verify-full`, which SQLx 0.8.6 supports, so the certificate chain is checked against trusted roots and the Neon hostname is verified. SQLx's Rustls build includes WebPKI roots unless an explicit `PGSSLROOTCERT` is supplied. Confirm the Neon project, branch, database name and a restorable backup before running the command. After the migration completes, deploy the matching API code with the restricted runtime role.
