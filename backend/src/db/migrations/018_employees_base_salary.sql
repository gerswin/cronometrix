-- 018_employees_base_salary.sql
-- Move base_salary from departments to employees (per-person salary).
-- Department.base_salary_cents stays as a "default suggestion" only;
-- the authoritative source for payroll math is now employees.base_salary_cents.
--
-- DEFAULT 0 — no backfill (demo). Existing rows go to 0; user reseeds.

ALTER TABLE employees ADD COLUMN base_salary_cents INTEGER NOT NULL DEFAULT 0;

-- Recreate audit triggers to include the new column in the JSON snapshot.
DROP TRIGGER IF EXISTS audit_employees_insert;
DROP TRIGGER IF EXISTS audit_employees_update;
DROP TRIGGER IF EXISTS audit_employees_delete;

CREATE TRIGGER audit_employees_insert
    AFTER INSERT ON employees
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'employees',
        NEW.id,
        'INSERT',
        NULL,
        json_object(
            'id', NEW.id, 'employee_code', NEW.employee_code, 'name', NEW.name,
            'department_id', NEW.department_id, 'status', NEW.status,
            'base_salary_cents', NEW.base_salary_cents, 'version', NEW.version
        ),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER audit_employees_update
    AFTER UPDATE ON employees
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'employees',
        NEW.id,
        'UPDATE',
        json_object(
            'id', OLD.id, 'employee_code', OLD.employee_code, 'name', OLD.name,
            'department_id', OLD.department_id, 'status', OLD.status,
            'base_salary_cents', OLD.base_salary_cents, 'version', OLD.version
        ),
        json_object(
            'id', NEW.id, 'employee_code', NEW.employee_code, 'name', NEW.name,
            'department_id', NEW.department_id, 'status', NEW.status,
            'base_salary_cents', NEW.base_salary_cents, 'version', NEW.version
        ),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER audit_employees_delete
    AFTER DELETE ON employees
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'employees',
        OLD.id,
        'DELETE',
        json_object(
            'id', OLD.id, 'employee_code', OLD.employee_code, 'name', OLD.name,
            'department_id', OLD.department_id, 'status', OLD.status,
            'base_salary_cents', OLD.base_salary_cents, 'version', OLD.version
        ),
        NULL,
        NULL,
        unixepoch()
    );
END;
