-- 023_raw_payload_rename.sql
-- The column has not held XML since firmware V3.3.8 support landed: DS-K1T341CMFW
-- pushes JSON, and `isapi::parser` now accepts either. A column called `raw_xml`
-- full of JSON is a trap for whoever debugs ingestion next.
--
-- Rename only. The contents are untouched, and the read-side mappers still omit
-- the column from `EVENT_SELECT_COLS` (T-2-14) — raw device payloads are kept for
-- forensics and never exposed on the API.
--
-- `ALTER TABLE ... RENAME COLUMN` needs SQLite 3.25+; libSQL is well past that.
-- No audit trigger references attendance_events, so nothing else has to move.

ALTER TABLE attendance_events RENAME COLUMN raw_xml TO raw_payload;
