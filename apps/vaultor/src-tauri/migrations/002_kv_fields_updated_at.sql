-- Add updated_at to kv_fields for per-field merge timestamps (git sync).
ALTER TABLE kv_fields ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;

-- Backfill: inherit updated_at from the parent secret.
UPDATE kv_fields
SET updated_at = (
    SELECT s.updated_at FROM secrets s WHERE s.id = kv_fields.secret_id
);
