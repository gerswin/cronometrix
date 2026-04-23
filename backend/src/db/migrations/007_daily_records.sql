-- 007_daily_records.sql
-- Materialized daily records per (employee_id, anchor_date). Engine-owned.
-- No version column (D-04 — engine writes via ON CONFLICT DO UPDATE, not optimistic concurrency).
-- No audit triggers (engine recomputes too frequently; manual edit audits live on
-- daily_record_overrides in Plan 03-03).

CREATE TABLE IF NOT EXISTS daily_records (
    id TEXT PRIMARY KEY,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    department_id TEXT NOT NULL REFERENCES departments(id),
    anchor_date TEXT NOT NULL,
    shift_type TEXT NOT NULL CHECK(shift_type IN ('day', 'night', 'mixed')),
    work_minutes INTEGER NOT NULL DEFAULT 0,
    overtime_minutes INTEGER NOT NULL DEFAULT 0,
    late_minutes INTEGER NOT NULL DEFAULT 0,
    early_departure_minutes INTEGER NOT NULL DEFAULT 0,
    is_rest_day_worked INTEGER NOT NULL DEFAULT 0 CHECK(is_rest_day_worked IN (0,1)),
    entry_at INTEGER,
    exit_at INTEGER,
    leave_id TEXT,
    computed_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_daily_records_employee_date
    ON daily_records(employee_id, anchor_date);
CREATE INDEX IF NOT EXISTS idx_daily_records_anchor ON daily_records(anchor_date);
CREATE INDEX IF NOT EXISTS idx_daily_records_employee ON daily_records(employee_id);
