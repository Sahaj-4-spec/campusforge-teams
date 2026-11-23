-- Record that self-service accounts were created from a provider-verified email.
ALTER TABLE users
    ADD COLUMN email_verified_at TIMESTAMPTZ;

-- Preserve the original verification timestamp for OAuth accounts created before
-- this column existed. Those rows were only written after provider verification.
UPDATE users AS u
   SET email_verified_at = verified.created_at
  FROM (
      SELECT user_id, MIN(created_at) AS created_at
        FROM oauth_accounts
       GROUP BY user_id
  ) AS verified
 WHERE u.id = verified.user_id
   AND u.email_verified_at IS NULL;

-- Enforce the new-account policy in PostgreSQL as well as in the new API.
-- This blocks password-only registration from an older API instance during a
-- rolling deployment, even if that instance is still reachable.
CREATE OR REPLACE FUNCTION campusforge_guard_new_user_insert()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.password_hash IS NOT NULL THEN
        RAISE EXCEPTION 'New CampusForge accounts must use provider-verified sign-up'
            USING ERRCODE = '23514',
                  CONSTRAINT = 'users_require_verified_social_signup';
    END IF;

    -- Older OAuth-capable binaries do not know about email_verified_at.
    -- Their OAuth path has already validated the provider identity before insert.
    NEW.email_verified_at := COALESCE(NEW.email_verified_at, NOW());
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_users_guard_new_user_insert
BEFORE INSERT ON users
FOR EACH ROW
EXECUTE FUNCTION campusforge_guard_new_user_insert();

-- A NULL password alone is not sufficient: every newly inserted user must be
-- accompanied by the OAuth identity in the same transaction. The deferred check
-- allows the existing OAuth flow to insert users first and oauth_accounts second.
CREATE OR REPLACE FUNCTION campusforge_require_oauth_identity()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
          FROM oauth_accounts
         WHERE user_id = NEW.id
    ) THEN
        RAISE EXCEPTION 'New CampusForge accounts require a verified OAuth identity'
            USING ERRCODE = '23514',
                  CONSTRAINT = 'users_require_oauth_identity';
    END IF;

    RETURN NULL;
END;
$$;

CREATE CONSTRAINT TRIGGER trg_users_require_oauth_identity
AFTER INSERT ON users
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW
WHEN (NEW.password_hash IS NULL)
EXECUTE FUNCTION campusforge_require_oauth_identity();
