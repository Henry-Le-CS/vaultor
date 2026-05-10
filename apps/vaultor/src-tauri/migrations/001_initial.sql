-- Vaultor — initial schema
-- Phase 3 will wire the DB connection; this file is created in Phase 1.

CREATE TABLE IF NOT EXISTS namespaces (
    id          TEXT    PRIMARY KEY,     -- UUIDv7
    name        TEXT    NOT NULL,
    created_at  INTEGER NOT NULL,        -- Unix milliseconds
    updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS secrets (
    id            TEXT    PRIMARY KEY,   -- UUIDv7
    namespace_id  TEXT    NOT NULL REFERENCES namespaces(id) ON DELETE CASCADE,
    name          TEXT    NOT NULL,      -- plaintext label — not considered sensitive
    kind          TEXT    NOT NULL,      -- 'kv' | 'file'
    is_draft      INTEGER NOT NULL DEFAULT 0,
    created_at    INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL
);

-- Key-value secrets: one row per field
CREATE TABLE IF NOT EXISTS kv_fields (
    id          TEXT    PRIMARY KEY,
    secret_id   TEXT    NOT NULL REFERENCES secrets(id) ON DELETE CASCADE,
    title       TEXT    NOT NULL,        -- plaintext field label
    value_enc   BLOB    NOT NULL,        -- AES-256-GCM ciphertext
    value_nonce BLOB    NOT NULL,        -- 12-byte GCM nonce
    hidden      INTEGER NOT NULL DEFAULT 1,
    sort_order  INTEGER NOT NULL DEFAULT 0
);

-- File secrets: one row per file secret
CREATE TABLE IF NOT EXISTS file_secrets (
    id            TEXT    PRIMARY KEY,
    secret_id     TEXT    NOT NULL REFERENCES secrets(id) ON DELETE CASCADE,
    filename      TEXT    NOT NULL,      -- original filename
    content_enc   BLOB    NOT NULL,      -- AES-256-GCM ciphertext
    content_nonce BLOB    NOT NULL,      -- 12-byte GCM nonce
    size_bytes    INTEGER NOT NULL       -- original size in bytes (max 1_048_576)
);

CREATE INDEX IF NOT EXISTS idx_secrets_namespace ON secrets(namespace_id);
CREATE INDEX IF NOT EXISTS idx_kv_fields_secret  ON kv_fields(secret_id, sort_order);
