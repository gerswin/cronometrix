-- 005_command_audit_log.sql
-- Append-only audit of every ISAPI command dispatch (D-11).
-- Separate from audit_log because this logs DEVICE interactions, not DB mutations.

CREATE TABLE IF NOT EXISTS command_audit_log (
    id TEXT PRIMARY KEY,
    actor_id TEXT NOT NULL REFERENCES users(id),
    device_id TEXT NOT NULL REFERENCES devices(id),
    command TEXT NOT NULL CHECK(command IN ('door_open','reboot','enrollment_mode')),
    outcome TEXT NOT NULL CHECK(outcome IN ('ok','error','timeout')),
    result TEXT,
    error_code TEXT,
    error_message TEXT,
    dispatched_at INTEGER NOT NULL,
    completed_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_cmd_audit_device ON command_audit_log(device_id);
CREATE INDEX IF NOT EXISTS idx_cmd_audit_actor ON command_audit_log(actor_id);
CREATE INDEX IF NOT EXISTS idx_cmd_audit_dispatched ON command_audit_log(dispatched_at);
