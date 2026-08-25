-- 021_face_id_isapi_length.sql
-- Shorten face_id values that exceed the ISAPI employeeNo limit.
--
-- face_id is pushed to Hikvision devices as `UserInfo.employeeNo`, whose
-- capabilities declare `@max: 32`. It used to be generated as a 36-char
-- hyphenated UUID, so the device rejected every push with
-- `statusCode 6 / badJsonContent / errorMsg "employeeNo"` (verified on
-- DS-K1T341CMFW firmware V3.3.8). Generation now uses the hyphen-less form;
-- this migration brings already-stored rows in line.
--
-- Rewriting in place rather than nulling: `replace(uuid, '-', '')` yields the
-- same 128 bits in exactly 32 characters, so no identity is lost and no row
-- passes through a NULL face_id state (which startup recovery rejects when a
-- pending enrollment checkpoint still references the employee).
--
-- Mappings are updated FIRST so the two tables never disagree mid-migration —
-- `events::service::lookup_employee_for_event` resolves inbound attendance
-- events against `device_face_mappings.face_id`, and a mismatch there would
-- silently turn known employees into unknown-face events.
--
-- Idempotent: after one pass every value is <= 32 chars, so the WHERE clauses
-- match nothing on a re-run.

UPDATE device_face_mappings
SET face_id = replace(face_id, '-', '')
WHERE length(face_id) > 32;

UPDATE employees
SET face_id = replace(face_id, '-', '')
WHERE face_id IS NOT NULL
  AND length(face_id) > 32;
