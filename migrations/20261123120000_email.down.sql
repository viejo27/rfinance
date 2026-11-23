DROP TABLE IF EXISTS email_outbox;
DROP TABLE IF EXISTS email_tokens;

ALTER TABLE users DROP COLUMN IF EXISTS session_epoch;
ALTER TABLE users DROP COLUMN IF EXISTS email_verified_at;
