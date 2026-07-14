-- 019_device_operation_checkpoints.sql
-- Durable, non-sensitive checkpoints for external Hikvision side effects.
-- A prepared checkpoint makes an ambiguous attempt manual/DB-only; it is
-- never permission to replay the device call. device_applied means only the
-- local mapping/delete transition remains.

CREATE TABLE IF NOT EXISTS device_operation_checkpoints (
    operation_key TEXT PRIMARY KEY,
    operation     TEXT NOT NULL CHECK(operation IN ('enrollment_push','backfill_push','purge_delete')),
    state         TEXT NOT NULL CHECK(state IN ('prepared','device_applied','manual')),
    updated_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_device_operation_checkpoints_state
    ON device_operation_checkpoints(operation, state);
