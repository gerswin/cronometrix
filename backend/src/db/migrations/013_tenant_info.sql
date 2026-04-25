-- 013_tenant_info.sql
-- Phase 5 D-30: tenant_info singleton for report branding header.
-- CHECK (id = 1) enforces single row at constraint level (defense in depth alongside CHECK on INSERT).
CREATE TABLE IF NOT EXISTS tenant_info (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    client_name TEXT NOT NULL DEFAULT '',
    client_rif  TEXT NOT NULL DEFAULT '',
    address     TEXT NOT NULL DEFAULT '',
    version     INTEGER NOT NULL DEFAULT 1,
    updated_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Seed the row. INSERT OR IGNORE keeps re-runs idempotent.
INSERT OR IGNORE INTO tenant_info (id, client_name, client_rif, address, version, updated_at)
VALUES (1, '', '', '', 1, unixepoch());
