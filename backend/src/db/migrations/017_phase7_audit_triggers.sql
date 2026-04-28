-- 017_phase7_audit_triggers.sql
-- Audit triggers for Phase 7 tables: enrollments, face_enrollments, device_face_mappings.
-- D-17: every INSERT/UPDATE/DELETE on these three tables writes to audit_log.
--
-- Closes the deferral note in 006_devices_audit_triggers.sql:
--   "device_face_mappings triggers are deferred to Phase 7 (enrollment)"
--
-- UUID-v4 expression is verbatim from 006_devices_audit_triggers.sql.
-- actor_id = NULL: trigger fires post-mutation; the application already wrote
--   started_by/created_by so actor surfaces via those columns on a JOIN.
-- Sensitive-column omission: photo bytes live on disk, never in DB.
--   face_quality_score is small JSON (~100 bytes) — included.

-- ==========================================================================
-- enrollments triggers (3)
-- ==========================================================================

CREATE TRIGGER IF NOT EXISTS audit_enrollments_insert
    AFTER INSERT ON enrollments
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'enrollments',
        NEW.id,
        'INSERT',
        NULL,
        json_object('id', NEW.id, 'employee_id', NEW.employee_id, 'face_enrollment_id', NEW.face_enrollment_id, 'status', NEW.status, 'started_by', NEW.started_by, 'started_at', NEW.started_at, 'completed_at', NEW.completed_at, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_enrollments_update
    AFTER UPDATE ON enrollments
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'enrollments',
        NEW.id,
        'UPDATE',
        json_object('id', OLD.id, 'employee_id', OLD.employee_id, 'face_enrollment_id', OLD.face_enrollment_id, 'status', OLD.status, 'started_by', OLD.started_by, 'started_at', OLD.started_at, 'completed_at', OLD.completed_at, 'version', OLD.version),
        json_object('id', NEW.id, 'employee_id', NEW.employee_id, 'face_enrollment_id', NEW.face_enrollment_id, 'status', NEW.status, 'started_by', NEW.started_by, 'started_at', NEW.started_at, 'completed_at', NEW.completed_at, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_enrollments_delete
    AFTER DELETE ON enrollments
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'enrollments',
        OLD.id,
        'DELETE',
        json_object('id', OLD.id, 'employee_id', OLD.employee_id, 'face_enrollment_id', OLD.face_enrollment_id, 'status', OLD.status, 'started_by', OLD.started_by, 'started_at', OLD.started_at, 'completed_at', OLD.completed_at, 'version', OLD.version),
        NULL,
        NULL,
        unixepoch()
    );
END;

-- ==========================================================================
-- face_enrollments triggers (3)
-- ==========================================================================

CREATE TRIGGER IF NOT EXISTS audit_face_enrollments_insert
    AFTER INSERT ON face_enrollments
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'face_enrollments',
        NEW.id,
        'INSERT',
        NULL,
        json_object('id', NEW.id, 'employee_id', NEW.employee_id, 'captured_via', NEW.captured_via, 'source_device_id', NEW.source_device_id, 'photo_path', NEW.photo_path, 'face_quality_score', NEW.face_quality_score, 'created_by', NEW.created_by, 'created_at', NEW.created_at),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_face_enrollments_update
    AFTER UPDATE ON face_enrollments
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'face_enrollments',
        NEW.id,
        'UPDATE',
        json_object('id', OLD.id, 'employee_id', OLD.employee_id, 'captured_via', OLD.captured_via, 'source_device_id', OLD.source_device_id, 'photo_path', OLD.photo_path, 'face_quality_score', OLD.face_quality_score, 'created_by', OLD.created_by, 'created_at', OLD.created_at),
        json_object('id', NEW.id, 'employee_id', NEW.employee_id, 'captured_via', NEW.captured_via, 'source_device_id', NEW.source_device_id, 'photo_path', NEW.photo_path, 'face_quality_score', NEW.face_quality_score, 'created_by', NEW.created_by, 'created_at', NEW.created_at),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_face_enrollments_delete
    AFTER DELETE ON face_enrollments
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'face_enrollments',
        OLD.id,
        'DELETE',
        json_object('id', OLD.id, 'employee_id', OLD.employee_id, 'captured_via', OLD.captured_via, 'source_device_id', OLD.source_device_id, 'photo_path', OLD.photo_path, 'face_quality_score', OLD.face_quality_score, 'created_by', OLD.created_by, 'created_at', OLD.created_at),
        NULL,
        NULL,
        unixepoch()
    );
END;

-- ==========================================================================
-- device_face_mappings triggers (3)
-- Closes the deferral note from 006_devices_audit_triggers.sql line 9.
-- ==========================================================================

CREATE TRIGGER IF NOT EXISTS audit_device_face_mappings_insert
    AFTER INSERT ON device_face_mappings
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'device_face_mappings',
        NEW.id,
        'INSERT',
        NULL,
        json_object('id', NEW.id, 'device_id', NEW.device_id, 'face_id', NEW.face_id, 'employee_id', NEW.employee_id, 'state', NEW.state, 'version', NEW.version, 'created_at', NEW.created_at, 'updated_at', NEW.updated_at),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_device_face_mappings_update
    AFTER UPDATE ON device_face_mappings
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'device_face_mappings',
        NEW.id,
        'UPDATE',
        json_object('id', OLD.id, 'device_id', OLD.device_id, 'face_id', OLD.face_id, 'employee_id', OLD.employee_id, 'state', OLD.state, 'version', OLD.version, 'created_at', OLD.created_at, 'updated_at', OLD.updated_at),
        json_object('id', NEW.id, 'device_id', NEW.device_id, 'face_id', NEW.face_id, 'employee_id', NEW.employee_id, 'state', NEW.state, 'version', NEW.version, 'created_at', NEW.created_at, 'updated_at', NEW.updated_at),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_device_face_mappings_delete
    AFTER DELETE ON device_face_mappings
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'device_face_mappings',
        OLD.id,
        'DELETE',
        json_object('id', OLD.id, 'device_id', OLD.device_id, 'face_id', OLD.face_id, 'employee_id', OLD.employee_id, 'state', OLD.state, 'version', OLD.version, 'created_at', OLD.created_at, 'updated_at', OLD.updated_at),
        NULL,
        NULL,
        unixepoch()
    );
END;
