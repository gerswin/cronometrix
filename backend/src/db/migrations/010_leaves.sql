-- 010_leaves.sql
-- Leave records per D-13. Full-day only (D-14). Immediate approval (D-15). Overlay precedence (D-16).
-- Soft-delete via status+deleted_at; never hard-delete (Venezuelan LOTTT legal retention).
-- leave_type enum enforced via CHECK — engine also validates but DB is source of truth.

CREATE TABLE IF NOT EXISTS leaves (
    id TEXT PRIMARY KEY,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    from_date TEXT NOT NULL,             -- 'YYYY-MM-DD' inclusive
    to_date TEXT NOT NULL,               -- 'YYYY-MM-DD' inclusive
    leave_type TEXT NOT NULL CHECK(leave_type IN ('medical', 'vacation', 'unpaid', 'manual')),
    justification TEXT NOT NULL,
    evidence_path TEXT,
    created_by TEXT NOT NULL REFERENCES users(id),
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'cancelled')),
    deleted_at INTEGER,
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_leaves_employee ON leaves(employee_id);
CREATE INDEX IF NOT EXISTS idx_leaves_dates ON leaves(from_date, to_date);
CREATE INDEX IF NOT EXISTS idx_leaves_active ON leaves(employee_id, from_date, to_date)
    WHERE status = 'active' AND deleted_at IS NULL;
