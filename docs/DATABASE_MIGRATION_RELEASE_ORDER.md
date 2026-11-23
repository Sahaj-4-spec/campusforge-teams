# Database migration and rollback release order

CampusForge applies SQLx migrations when the API starts. Keep database changes compatible with the application versions that may be live during a rolling deploy and with the last rollback target.

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

## Rollback

Roll back the application to the bridge release, not to a pre-bridge binary. The bridge runner ignores the later migration version while still allowing the database to retain its additive schema and database guard. Do not manually delete migration-history rows or drop the trigger/column as part of an application rollback.

If a later change needs a destructive schema contraction, ship it only after the rollback window and older binary support have deliberately ended.
