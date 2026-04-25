-- 015_employees_position_hire_date.sql
-- Phase 5 D-30a: add columns Phase 4 employee table already references.
ALTER TABLE employees ADD COLUMN position TEXT NOT NULL DEFAULT '';
ALTER TABLE employees ADD COLUMN hire_date INTEGER;  -- nullable epoch seconds (UTC); '' or 0 are NOT semantically equivalent to unknown.
