-- 001_initial_schema.sql
-- Core tables for Cronometrix.
-- All timestamps: UTC epoch integers. All IDs: UUID v4 strings.
-- All mutable tables have a version column for optimistic concurrency.

-- users table: per AUTH-01 through AUTH-05
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    full_name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK(role IN ('admin', 'supervisor', 'viewer')),
    refresh_token_hash TEXT,
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'inactive')),
    deleted_at INTEGER,
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- departments table: per DEPT-01 through DEPT-03
CREATE TABLE IF NOT EXISTS departments (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    base_salary_cents INTEGER NOT NULL DEFAULT 0,
    shift_start_time TEXT NOT NULL,
    shift_end_time TEXT NOT NULL,
    lunch_mode TEXT NOT NULL CHECK(lunch_mode IN ('fixed', 'punch')),
    lunch_duration_min INTEGER,
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'inactive')),
    deleted_at INTEGER,
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- employees table: per EMP-01 through EMP-04
CREATE TABLE IF NOT EXISTS employees (
    id TEXT PRIMARY KEY,
    employee_code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    department_id TEXT NOT NULL REFERENCES departments(id),
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'inactive')),
    deleted_at INTEGER,
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_employees_department ON employees(department_id);
CREATE INDEX IF NOT EXISTS idx_employees_status ON employees(status);
CREATE INDEX IF NOT EXISTS idx_employees_name ON employees(name);

-- global_rules table: per RULE-01 through RULE-03
-- Singleton table — always exactly one row with id = 'singleton'.
CREATE TABLE IF NOT EXISTS global_rules (
    id TEXT PRIMARY KEY DEFAULT 'singleton',
    late_arrival_tolerance_min INTEGER NOT NULL DEFAULT 10,
    early_departure_tolerance_min INTEGER NOT NULL DEFAULT 10,
    bonus_minutes INTEGER NOT NULL DEFAULT 0,
    effective_from INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    updated_at INTEGER NOT NULL
);

-- Seed the singleton row on first migration (INSERT OR IGNORE is idempotent)
INSERT OR IGNORE INTO global_rules (id, late_arrival_tolerance_min, early_departure_tolerance_min, bonus_minutes, effective_from, version, updated_at)
VALUES ('singleton', 10, 10, 0, unixepoch(), 1, unixepoch());

-- audit_log table: per DATA-04, D-01
-- Append-only — no UPDATE or DELETE triggers; enforced by application convention
-- and the absence of any triggers that modify this table.
CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY,
    table_name TEXT NOT NULL,
    record_id TEXT NOT NULL,
    operation TEXT NOT NULL CHECK(operation IN ('INSERT', 'UPDATE', 'DELETE')),
    old_data TEXT,
    new_data TEXT,
    actor_id TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_log_table ON audit_log(table_name);
CREATE INDEX IF NOT EXISTS idx_audit_log_record ON audit_log(record_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_created ON audit_log(created_at);
