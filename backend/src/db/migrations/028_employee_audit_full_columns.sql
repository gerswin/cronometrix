-- M-06: `audit_employees_update` no auditaba `salary_kind`. Cambiar el
-- salario de un empleado quedaba en el registro; cambiar la UNIDAD de ese
-- salario (diario vs. mensual) no — y esa unidad multiplica el importe por
-- treinta si se equivoca (ver H-08, migración 024).
--
-- Cómo llegamos aquí, para que no vuelva a pasar:
--   1. 002_audit_triggers.sql definió los TRES triggers (insert/update/delete)
--      con las columnas que `employees` tenía entonces: id, employee_code,
--      name, department_id, status, version.
--   2. 014_phase5_audit_triggers.sql hizo DROP + CREATE de
--      `audit_employees_update` (y SOLO de ese trigger, no de insert/delete)
--      para sumar `position` y `hire_date`.
--   3. 016_enrollments.sql agregó `face_id` y `current_face_enrollment_id` a
--      la tabla sin tocar ningún trigger — nadie lo mencionó, así que nadie
--      lo arregló.
--   4. 018_employees_base_salary.sql volvió a hacer DROP + CREATE de los TRES
--      triggers (insert/update/delete) para sumar `base_salary_cents`, pero
--      los recreó copiando la versión de 002, no la de 014: perdió
--      `position` y `hire_date` del trigger de update en el camino, y ningún
--      trigger tuvo nunca `face_id` ni `current_face_enrollment_id`.
--   5. 024_employee_salary_kind.sql y 026_employee_terminated_on.sql
--      agregaron `salary_kind` y `terminated_on` — otra vez sin tocar ningún
--      trigger, porque nada en esas migraciones obligaba a revisarlos.
--
-- La causa raíz es estructural, no un descuido puntual: un trigger de
-- auditoría no se puede ALTER, solo DROP + CREATE. Recrearlo para sumar UNA
-- columna nueva significa reescribir el `json_object` entero a mano, y
-- reescribirlo a mano pierde en silencio cualquier columna que se haya
-- agregado desde la última vez que alguien lo escribió — no falla, no
-- avisa, simplemente dejan de auditarse esas columnas. Ya pasó dos veces
-- (paso 2 se perdió en el paso 4; los pasos 3, 5 nunca se incorporaron a
-- ningún trigger, ni update ni insert ni delete).
--
-- Esta migración corrige los TRES triggers de `employees`
-- (`audit_employees_insert`, `audit_employees_update`,
-- `audit_employees_delete`), no solo el de update: los tres arrastraban el
-- mismo defecto desde el paso 4, porque 018 los recreó juntos con el mismo
-- json_object recortado. Dejar solo `update` corregido habría sido peor que
-- no corregir nada: un registro de auditoría que luce completo al editar un
-- empleado pero sigue incompleto al crearlo o borrarlo es más engañoso que
-- uno incompleto en todas partes, porque invita a confiar en él.
--
-- Por eso este archivo vuelve a poner las columnas completas de `employees`
-- tal como existen HOY (verificado con PRAGMA table_info, no copiado de
-- ninguna migración anterior) en los tres triggers: id, employee_code, name,
-- department_id, status, deleted_at, version, created_at, updated_at,
-- position, hire_date, face_id, current_face_enrollment_id,
-- base_salary_cents, salary_kind, terminated_on.
--
-- CUALQUIER migración futura que agregue una columna a `employees` (ALTER
-- TABLE employees ADD COLUMN ...) TIENE que agregar esa misma columna a los
-- tres triggers de abajo — a los `json_object` de `old` y `new` en
-- `audit_employees_update`, al `json_object` de `new` en
-- `audit_employees_insert`, y al `json_object` de `old` en
-- `audit_employees_delete`. Son TRES lugares que revisar, no uno. Si no lo
-- hace, la columna nueva queda fuera de la auditoría desde el día uno, en
-- silencio, hasta que alguien la note — como pasó con `salary_kind`.
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
            'deleted_at', NEW.deleted_at, 'version', NEW.version,
            'created_at', NEW.created_at, 'updated_at', NEW.updated_at,
            'position', NEW.position, 'hire_date', NEW.hire_date,
            'face_id', NEW.face_id, 'current_face_enrollment_id', NEW.current_face_enrollment_id,
            'base_salary_cents', NEW.base_salary_cents, 'salary_kind', NEW.salary_kind,
            'terminated_on', NEW.terminated_on
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
            'deleted_at', OLD.deleted_at, 'version', OLD.version,
            'created_at', OLD.created_at, 'updated_at', OLD.updated_at,
            'position', OLD.position, 'hire_date', OLD.hire_date,
            'face_id', OLD.face_id, 'current_face_enrollment_id', OLD.current_face_enrollment_id,
            'base_salary_cents', OLD.base_salary_cents, 'salary_kind', OLD.salary_kind,
            'terminated_on', OLD.terminated_on
        ),
        json_object(
            'id', NEW.id, 'employee_code', NEW.employee_code, 'name', NEW.name,
            'department_id', NEW.department_id, 'status', NEW.status,
            'deleted_at', NEW.deleted_at, 'version', NEW.version,
            'created_at', NEW.created_at, 'updated_at', NEW.updated_at,
            'position', NEW.position, 'hire_date', NEW.hire_date,
            'face_id', NEW.face_id, 'current_face_enrollment_id', NEW.current_face_enrollment_id,
            'base_salary_cents', NEW.base_salary_cents, 'salary_kind', NEW.salary_kind,
            'terminated_on', NEW.terminated_on
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
            'deleted_at', OLD.deleted_at, 'version', OLD.version,
            'created_at', OLD.created_at, 'updated_at', OLD.updated_at,
            'position', OLD.position, 'hire_date', OLD.hire_date,
            'face_id', OLD.face_id, 'current_face_enrollment_id', OLD.current_face_enrollment_id,
            'base_salary_cents', OLD.base_salary_cents, 'salary_kind', OLD.salary_kind,
            'terminated_on', OLD.terminated_on
        ),
        NULL,
        NULL,
        unixepoch()
    );
END;
