-- 011_phase3_audit_triggers.sql
-- AFTER INSERT/UPDATE/DELETE audit triggers for Phase 3 user-editable tables.
-- Mirrors 006_devices_audit_triggers.sql exactly — UUID v4 via hex(randomblob),
-- json_object payload capturing the business-visible columns, actor_id NULL
-- in trigger (app-code writes a secondary audit row when actor context is needed).
--
-- daily_records is intentionally NOT audited here: engine recomputes too frequently,
-- and operator-visible mutations live on daily_record_overrides (D-04).

-- ============================================================
-- leaves triggers
-- ============================================================

CREATE TRIGGER IF NOT EXISTS audit_leaves_insert
    AFTER INSERT ON leaves
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'leaves',
        NEW.id,
        'INSERT',
        NULL,
        json_object('id', NEW.id, 'employee_id', NEW.employee_id, 'from_date', NEW.from_date, 'to_date', NEW.to_date, 'leave_type', NEW.leave_type, 'justification', NEW.justification, 'evidence_path', NEW.evidence_path, 'status', NEW.status, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_leaves_update
    AFTER UPDATE ON leaves
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'leaves',
        NEW.id,
        'UPDATE',
        json_object('id', OLD.id, 'employee_id', OLD.employee_id, 'from_date', OLD.from_date, 'to_date', OLD.to_date, 'leave_type', OLD.leave_type, 'justification', OLD.justification, 'evidence_path', OLD.evidence_path, 'status', OLD.status, 'version', OLD.version),
        json_object('id', NEW.id, 'employee_id', NEW.employee_id, 'from_date', NEW.from_date, 'to_date', NEW.to_date, 'leave_type', NEW.leave_type, 'justification', NEW.justification, 'evidence_path', NEW.evidence_path, 'status', NEW.status, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_leaves_delete
    AFTER DELETE ON leaves
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'leaves',
        OLD.id,
        'DELETE',
        json_object('id', OLD.id, 'employee_id', OLD.employee_id, 'from_date', OLD.from_date, 'to_date', OLD.to_date, 'leave_type', OLD.leave_type, 'justification', OLD.justification, 'evidence_path', OLD.evidence_path, 'status', OLD.status, 'version', OLD.version),
        NULL,
        NULL,
        unixepoch()
    );
END;

-- ============================================================
-- daily_record_overrides triggers
-- ============================================================

CREATE TRIGGER IF NOT EXISTS audit_daily_record_overrides_insert
    AFTER INSERT ON daily_record_overrides
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'daily_record_overrides',
        NEW.id,
        'INSERT',
        NULL,
        json_object('id', NEW.id, 'daily_record_id', NEW.daily_record_id, 'override_work_minutes', NEW.override_work_minutes, 'override_entry_at', NEW.override_entry_at, 'override_exit_at', NEW.override_exit_at, 'justification', NEW.justification, 'evidence_path', NEW.evidence_path, 'overridden_by', NEW.overridden_by, 'status', NEW.status, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_daily_record_overrides_update
    AFTER UPDATE ON daily_record_overrides
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'daily_record_overrides',
        NEW.id,
        'UPDATE',
        json_object('id', OLD.id, 'daily_record_id', OLD.daily_record_id, 'override_work_minutes', OLD.override_work_minutes, 'override_entry_at', OLD.override_entry_at, 'override_exit_at', OLD.override_exit_at, 'justification', OLD.justification, 'evidence_path', OLD.evidence_path, 'overridden_by', OLD.overridden_by, 'status', OLD.status, 'version', OLD.version),
        json_object('id', NEW.id, 'daily_record_id', NEW.daily_record_id, 'override_work_minutes', NEW.override_work_minutes, 'override_entry_at', NEW.override_entry_at, 'override_exit_at', NEW.override_exit_at, 'justification', NEW.justification, 'evidence_path', NEW.evidence_path, 'overridden_by', NEW.overridden_by, 'status', NEW.status, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_daily_record_overrides_delete
    AFTER DELETE ON daily_record_overrides
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'daily_record_overrides',
        OLD.id,
        'DELETE',
        json_object('id', OLD.id, 'daily_record_id', OLD.daily_record_id, 'override_work_minutes', OLD.override_work_minutes, 'override_entry_at', OLD.override_entry_at, 'override_exit_at', OLD.override_exit_at, 'justification', OLD.justification, 'evidence_path', OLD.evidence_path, 'overridden_by', OLD.overridden_by, 'status', OLD.status, 'version', OLD.version),
        NULL,
        NULL,
        unixepoch()
    );
END;
