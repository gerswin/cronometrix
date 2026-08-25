-- Bloque 3 (H-10): configurable retention policy for attendance evidence.
--
-- MECHANISM, NOT POLICY. The concrete retention periods depend on an unanswered
-- labour consultation (docs/legal/CONSULTA-LABORAL.md). The safe default is NULL
-- for every class, meaning "keep forever" — an unattended deployment must never
-- silently lose proof-of-work (H-09). Setting a period later is configuration,
-- not a code change.
CREATE TABLE IF NOT EXISTS retention_policy (
    id                    INTEGER PRIMARY KEY CHECK (id = 1),
    events_retention_days INTEGER,   -- NULL = keep forever
    leaves_retention_days INTEGER,   -- NULL = keep forever
    updated_at            INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Seed the singleton row with the safe default (keep everything). INSERT OR
-- IGNORE keeps migration re-runs idempotent.
INSERT OR IGNORE INTO retention_policy (id, events_retention_days, leaves_retention_days, updated_at)
VALUES (1, NULL, NULL, unixepoch());
