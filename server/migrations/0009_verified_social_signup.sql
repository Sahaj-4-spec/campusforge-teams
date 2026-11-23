-- Record that self-service accounts were created from a provider-verified email.
ALTER TABLE users
    ADD COLUMN email_verified_at TIMESTAMPTZ;

-- Accounts that already have an OAuth identity were only created after the provider
-- returned a verified email, so retain their original verification time.
UPDATE users AS u
   SET email_verified_at = verified.created_at
  FROM (
      SELECT user_id, MIN(created_at) AS created_at
        FROM oauth_accounts
       GROUP BY user_id
  ) AS verified
 WHERE u.id = verified.user_id
   AND u.email_verified_at IS NULL;
