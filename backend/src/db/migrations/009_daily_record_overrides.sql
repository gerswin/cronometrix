-- 009_daily_record_overrides.sql
-- Operator edits live here, NOT in daily_records (D-04). Engine recomputes never touch this table.
-- Phase 4 timesheet editor writes here. Phase 5 reports JOIN at read time.
-- Version column + optimistic concurrency per Phase 1 convention.
-- Soft-delete via status+deleted_at (never hard-delete — LOTTT retention).

CREATE TABLE IF NOT EXISTS daily_record_overrides (
    id TEXT PRIMARY KEY,
    daily_record_id TEXT NOT NULL REFERENCES daily_records(id),
    override_work_minutes INTEGER,
    override_entry_at INTEGER,
    override_exit_at INTEGER,
    justification TEXT NOT NULL,
    evidence_path TEXT,
    overridden_by TEXT NOT NULL REFERENCES users(id),
    overridden_at INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'revoked')),
    deleted_at INTEGER,
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_overrides_record
    ON daily_record_overrides(daily_record_id)
    WHERE deleted_at IS NULL;
