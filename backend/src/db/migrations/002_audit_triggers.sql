-- 002_audit_triggers.sql
-- AFTER INSERT/UPDATE/DELETE triggers on all mutable tables.
-- Writes immutable audit_log entries for every mutation.
-- actor_id is NULL in triggers (Phase 1 acceptable per Pitfall 4);
-- service layer may write a secondary entry with actor context.
--
-- UUID v4 generation uses hex(randomblob()) pattern — libSQL has no built-in uuid().

-- ============================================================
-- employees triggers
-- ============================================================

CREATE TRIGGER IF NOT EXISTS audit_employees_insert
    AFTER INSERT ON employees
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'employees',
        NEW.id,
        'INSERT',
        NULL,
        json_object('id', NEW.id, 'employee_code', NEW.employee_code, 'name', NEW.name, 'department_id', NEW.department_id, 'status', NEW.status, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_employees_update
    AFTER UPDATE ON employees
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'employees',
        NEW.id,
        'UPDATE',
        json_object('id', OLD.id, 'employee_code', OLD.employee_code, 'name', OLD.name, 'department_id', OLD.department_id, 'status', OLD.status, 'version', OLD.version),
        json_object('id', NEW.id, 'employee_code', NEW.employee_code, 'name', NEW.name, 'department_id', NEW.department_id, 'status', NEW.status, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_employees_delete
    AFTER DELETE ON employees
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'employees',
        OLD.id,
        'DELETE',
        json_object('id', OLD.id, 'employee_code', OLD.employee_code, 'name', OLD.name, 'department_id', OLD.department_id, 'status', OLD.status, 'version', OLD.version),
        NULL,
        NULL,
        unixepoch()
    );
END;

-- ============================================================
-- departments triggers
-- ============================================================

CREATE TRIGGER IF NOT EXISTS audit_departments_insert
    AFTER INSERT ON departments
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'departments',
        NEW.id,
        'INSERT',
        NULL,
        json_object('id', NEW.id, 'name', NEW.name, 'base_salary_cents', NEW.base_salary_cents, 'shift_start_time', NEW.shift_start_time, 'shift_end_time', NEW.shift_end_time, 'lunch_mode', NEW.lunch_mode, 'status', NEW.status, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_departments_update
    AFTER UPDATE ON departments
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'departments',
        NEW.id,
        'UPDATE',
        json_object('id', OLD.id, 'name', OLD.name, 'base_salary_cents', OLD.base_salary_cents, 'shift_start_time', OLD.shift_start_time, 'shift_end_time', OLD.shift_end_time, 'lunch_mode', OLD.lunch_mode, 'status', OLD.status, 'version', OLD.version),
        json_object('id', NEW.id, 'name', NEW.name, 'base_salary_cents', NEW.base_salary_cents, 'shift_start_time', NEW.shift_start_time, 'shift_end_time', NEW.shift_end_time, 'lunch_mode', NEW.lunch_mode, 'status', NEW.status, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_departments_delete
    AFTER DELETE ON departments
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'departments',
        OLD.id,
        'DELETE',
        json_object('id', OLD.id, 'name', OLD.name, 'base_salary_cents', OLD.base_salary_cents, 'shift_start_time', OLD.shift_start_time, 'shift_end_time', OLD.shift_end_time, 'lunch_mode', OLD.lunch_mode, 'status', OLD.status, 'version', OLD.version),
        NULL,
        NULL,
        unixepoch()
    );
END;

-- ============================================================
-- users triggers
-- ============================================================

CREATE TRIGGER IF NOT EXISTS audit_users_insert
    AFTER INSERT ON users
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'users',
        NEW.id,
        'INSERT',
        NULL,
        json_object('id', NEW.id, 'username', NEW.username, 'full_name', NEW.full_name, 'role', NEW.role, 'status', NEW.status, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_users_update
    AFTER UPDATE ON users
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'users',
        NEW.id,
        'UPDATE',
        json_object('id', OLD.id, 'username', OLD.username, 'full_name', OLD.full_name, 'role', OLD.role, 'status', OLD.status, 'version', OLD.version),
        json_object('id', NEW.id, 'username', NEW.username, 'full_name', NEW.full_name, 'role', NEW.role, 'status', NEW.status, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_users_delete
    AFTER DELETE ON users
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'users',
        OLD.id,
        'DELETE',
        json_object('id', OLD.id, 'username', OLD.username, 'full_name', OLD.full_name, 'role', OLD.role, 'status', OLD.status, 'version', OLD.version),
        NULL,
        NULL,
        unixepoch()
    );
END;

-- ============================================================
-- global_rules triggers
-- ============================================================

CREATE TRIGGER IF NOT EXISTS audit_global_rules_update
    AFTER UPDATE ON global_rules
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'global_rules',
        NEW.id,
        'UPDATE',
        json_object('id', OLD.id, 'late_arrival_tolerance_min', OLD.late_arrival_tolerance_min, 'early_departure_tolerance_min', OLD.early_departure_tolerance_min, 'bonus_minutes', OLD.bonus_minutes, 'effective_from', OLD.effective_from, 'version', OLD.version),
        json_object('id', NEW.id, 'late_arrival_tolerance_min', NEW.late_arrival_tolerance_min, 'early_departure_tolerance_min', NEW.early_departure_tolerance_min, 'bonus_minutes', NEW.bonus_minutes, 'effective_from', NEW.effective_from, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_global_rules_insert
    AFTER INSERT ON global_rules
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'global_rules',
        NEW.id,
        'INSERT',
        NULL,
        json_object('id', NEW.id, 'late_arrival_tolerance_min', NEW.late_arrival_tolerance_min, 'early_departure_tolerance_min', NEW.early_departure_tolerance_min, 'bonus_minutes', NEW.bonus_minutes, 'effective_from', NEW.effective_from, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;
