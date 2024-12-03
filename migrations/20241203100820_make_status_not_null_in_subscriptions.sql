-- Add migration script here
BEGIN;
  -- Backfill the status for historical entries
  UPDATE subscriptions
    SET status = 'confirmed'
    WHERE status IS NULL;

  ALTER TABLE subscriptions ALTER COLUMN status SET NOT NULL;
COMMIT;