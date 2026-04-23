-- 008_daily_record_anomalies.sql
-- Append-only per D-18. No version, no update. Cleared in same transaction as upsert.

CREATE TABLE IF NOT EXISTS daily_record_anomalies (
    id TEXT PRIMARY KEY,
    daily_record_id TEXT NOT NULL REFERENCES daily_records(id) ON DELETE CASCADE,
    code TEXT NOT NULL CHECK(code IN (
        'MISSING_ENTRY','MISSING_EXIT','UNKNOWN_FACE_IN_WINDOW','LUNCH_PUNCH_MISSING',
        'OT_CAP_EXCEEDED_DAILY','OT_CAP_EXCEEDED_WEEKLY','OT_CAP_EXCEEDED_ANNUAL',
        'EVENTS_ON_LEAVE_DAY','RECOMPUTE_AFTER_EDIT','OVERNIGHT_INFERENCE_AMBIGUOUS'
    )),
    detail TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_anomalies_record
    ON daily_record_anomalies(daily_record_id);
CREATE INDEX IF NOT EXISTS idx_anomalies_code
    ON daily_record_anomalies(code);
