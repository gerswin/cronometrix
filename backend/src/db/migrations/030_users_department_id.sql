-- Bloque 4 (H-11): department scope on users.
--
-- Adds the identity half of the access-scope model: a user may be bound to a
-- department. NULL means "no scope" (org-wide) — the D1 default, so existing
-- users are not locked out by the upgrade; the scope activates once a
-- department is assigned. Admins are unscoped regardless. The FK to departments
-- keeps the reference honest; foreign_keys must be ON (it is, per db init).
ALTER TABLE users ADD COLUMN department_id TEXT REFERENCES departments(id);

CREATE INDEX IF NOT EXISTS idx_users_department ON users(department_id);
