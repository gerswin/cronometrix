-- 004_attendance_events.sql
-- Dedup key is a composite UNIQUE index on (employee_id, device_id, direction, bucket_30s)
-- where bucket = floor(captured_at / 30). Per D-05 / D-06 this makes dedup a DB invariant.
-- NOTE (Pitfall 6): SQLite treats NULL != NULL in UNIQUE, so unknown-face rows (employee_id IS NULL)
-- intentionally all persist. This matches D-07's forensic intent.

CREATE TABLE IF NOT EXISTS attendance_events (
    id TEXT PRIMARY KEY,
    employee_id TEXT REFERENCES employees(id),   -- NULL when unknown face (D-07)
    device_id TEXT NOT NULL REFERENCES devices(id),
    direction TEXT NOT NULL CHECK(direction IN ('entry','exit')),
    captured_at INTEGER NOT NULL,                 -- UTC epoch seconds (EVT-04)
    bucket_30s INTEGER NOT NULL,                  -- floor(captured_at / 30)
    is_unknown INTEGER NOT NULL DEFAULT 0 CHECK(is_unknown IN (0,1)),
    face_id TEXT,                                 -- device-emitted face identifier (A2)
    employee_no_string TEXT,                      -- device-emitted employeeNoString (fallback lookup)
    raw_xml TEXT NOT NULL,                        -- full EventNotificationAlert block (D-12)
    photo_path TEXT,                              -- relative path under ./data/events/ (D-13) or NULL
    created_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_attendance_dedup
    ON attendance_events(employee_id, device_id, direction, bucket_30s);

CREATE INDEX IF NOT EXISTS idx_attendance_captured ON attendance_events(captured_at);
CREATE INDEX IF NOT EXISTS idx_attendance_employee ON attendance_events(employee_id);
CREATE INDEX IF NOT EXISTS idx_attendance_device ON attendance_events(device_id);
