-- 016_enrollments.sql
-- Phase 7: Facial Enrollment schema additions.
-- New tables: face_enrollments, enrollments, enrollment_device_pushes.
-- Column additions: employees.face_id, employees.current_face_enrollment_id,
--                   device_face_mappings.state.
-- D-10: face_id = Cronometrix-generated UUID v4, stable per employee.
-- D-11: canonical photo stored on disk; photo_path in face_enrollments.
-- D-13: device_face_mappings.state tracks pending_delete for purge worker (D-15).

-- --------------------------------------------------------------------------
-- Extend existing tables
-- --------------------------------------------------------------------------

-- D-10: face_id — server-generated UUID, UNIQUE, assigned on first enrollment.
-- SQLite does not support ALTER TABLE ... ADD COLUMN ... UNIQUE; enforce via index.
ALTER TABLE employees ADD COLUMN face_id TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_employees_face_id ON employees(face_id) WHERE face_id IS NOT NULL;

-- D-11: pointer to the active face_enrollment for this employee.
ALTER TABLE employees ADD COLUMN current_face_enrollment_id TEXT REFERENCES face_enrollments(id);

-- D-15: state column for purge worker. 'active' = synced; 'pending_delete' = purge failed, will retry.
ALTER TABLE device_face_mappings ADD COLUMN state TEXT NOT NULL DEFAULT 'active' CHECK(state IN ('active','pending_delete'));

-- --------------------------------------------------------------------------
-- face_enrollments — one row per enrollment attempt (photo source + metadata)
-- --------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS face_enrollments (
    id              TEXT PRIMARY KEY,
    employee_id     TEXT NOT NULL REFERENCES employees(id),
    captured_via    TEXT NOT NULL CHECK(captured_via IN ('device','webcam','upload')),
    source_device_id TEXT REFERENCES devices(id),  -- NULL unless captured_via='device'
    photo_path      TEXT NOT NULL,                  -- relative: {employee_id}/{enrollment_id}.jpg
    face_quality_score TEXT,                        -- JSON: {face_detected, luminance, width, height}
    created_by      TEXT NOT NULL REFERENCES users(id),
    created_at      INTEGER NOT NULL                -- unixepoch()
);

CREATE INDEX IF NOT EXISTS idx_face_enrollments_employee
    ON face_enrollments(employee_id);

-- --------------------------------------------------------------------------
-- enrollments — one row per admin-initiated enrollment session
-- --------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS enrollments (
    id                 TEXT PRIMARY KEY,
    employee_id        TEXT NOT NULL REFERENCES employees(id),
    face_enrollment_id TEXT NOT NULL REFERENCES face_enrollments(id),
    status             TEXT NOT NULL DEFAULT 'in_progress'
                           CHECK(status IN ('in_progress','success','partial','failed')),
    started_by         TEXT NOT NULL REFERENCES users(id),
    started_at         INTEGER NOT NULL,            -- unixepoch()
    completed_at       INTEGER,                     -- NULL until all push tasks settle
    version            INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_enrollments_employee ON enrollments(employee_id);
CREATE INDEX IF NOT EXISTS idx_enrollments_status   ON enrollments(status);

-- --------------------------------------------------------------------------
-- enrollment_device_pushes — one row per (enrollment, device) pair
-- --------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS enrollment_device_pushes (
    id            TEXT PRIMARY KEY,
    enrollment_id TEXT NOT NULL REFERENCES enrollments(id),
    device_id     TEXT NOT NULL REFERENCES devices(id),
    status        TEXT NOT NULL DEFAULT 'pending'
                      CHECK(status IN ('pending','in_progress','success','failed')),
    error_message TEXT,                             -- scrubbed (no device password substring)
    started_at    INTEGER,                          -- NULL until task picks it up
    completed_at  INTEGER,                          -- NULL until terminal
    UNIQUE(enrollment_id, device_id)                -- supports INSERT OR REPLACE on retry
);

CREATE INDEX IF NOT EXISTS idx_edp_enrollment ON enrollment_device_pushes(enrollment_id);
CREATE INDEX IF NOT EXISTS idx_edp_status     ON enrollment_device_pushes(status);
