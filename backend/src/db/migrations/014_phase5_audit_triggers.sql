-- 014_phase5_audit_triggers.sql
-- Phase 5: relax audit_log.operation CHECK to allow 'REPORT_EXPORT' (D-21).
--
-- We use the `PRAGMA writable_schema` idiom rather than the standard table-rebuild
-- approach (CREATE _new + INSERT SELECT + DROP + RENAME) because:
--   * Migrations 002/006/011 all defined audit triggers that contain
--     `INSERT INTO audit_log (...)`. Modern SQLite (>= 3.25) recursively validates
--     trigger references during DROP TABLE / RENAME, so the rebuild path fails with
--     "database table is locked" inside libSQL `execute_batch` even with
--     PRAGMA legacy_alter_table = ON (the pragma is not honoured inside the
--     implicit transaction libSQL wraps around the batch).
--   * The CHECK constraint is stored verbatim in sqlite_master.sql; rewriting that
--     text relaxes the constraint without touching trigger references.
--   * Existing rows are preserved exactly. No INSERT SELECT, no DROP, no RENAME.
--
-- After the schema text is rewritten, SQLite parses the new constraint at the next
-- statement, which is why we set the pragma off and immediately register two
-- triggers below — they exercise the new schema state.
PRAGMA writable_schema = 1;

UPDATE sqlite_master
SET sql = replace(
        sql,
        'CHECK(operation IN (''INSERT'', ''UPDATE'', ''DELETE''))',
        'CHECK(operation IN (''INSERT'', ''UPDATE'', ''DELETE'', ''REPORT_EXPORT''))'
    )
WHERE type = 'table' AND name = 'audit_log';

PRAGMA writable_schema = 0;

-- audit_tenant_info_update: AFTER UPDATE trigger captures full row diff.
CREATE TRIGGER IF NOT EXISTS audit_tenant_info_update
    AFTER UPDATE ON tenant_info
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2)
              || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2)
              || '-' || hex(randomblob(6))),
        'tenant_info',
        CAST(NEW.id AS TEXT),
        'UPDATE',
        json_object('id', OLD.id, 'client_name', OLD.client_name, 'client_rif', OLD.client_rif,
                    'address', OLD.address, 'version', OLD.version),
        json_object('id', NEW.id, 'client_name', NEW.client_name, 'client_rif', NEW.client_rif,
                    'address', NEW.address, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

-- DROP + RECREATE audit_employees_update so the new position + hire_date columns are captured.
DROP TRIGGER IF EXISTS audit_employees_update;

CREATE TRIGGER IF NOT EXISTS audit_employees_update
    AFTER UPDATE ON employees
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2)
              || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2)
              || '-' || hex(randomblob(6))),
        'employees',
        NEW.id,
        'UPDATE',
        json_object('id', OLD.id, 'employee_code', OLD.employee_code, 'name', OLD.name,
                    'department_id', OLD.department_id, 'status', OLD.status,
                    'position', OLD.position, 'hire_date', OLD.hire_date, 'version', OLD.version),
        json_object('id', NEW.id, 'employee_code', NEW.employee_code, 'name', NEW.name,
                    'department_id', NEW.department_id, 'status', NEW.status,
                    'position', NEW.position, 'hire_date', NEW.hire_date, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;
