-- 022_device_ingest_mode.sql
-- Let a device deliver events by push instead of the alertStream we pull.
--
-- The alertStream admits a single subscriber per device (`deployExceedMax`), so
-- anyone opening iVMS or the reader's own web page steals the slot and ingestion
-- stops until they close it. A device that pushes to us instead has no such
-- contention.
--
-- The two transports are mutually exclusive per device, never both: the dedup
-- index on attendance_events keys on employee_id, and SQLite treats NULLs as
-- distinct, so an unknown-face marking arriving over both paths would be stored
-- twice with nothing to catch it.
--
-- Defaults to 'stream' so existing installations keep their current behaviour
-- until an operator opts a reader in.

ALTER TABLE devices ADD COLUMN ingest_mode TEXT NOT NULL DEFAULT 'stream'
    CHECK(ingest_mode IN ('stream', 'push'));

-- Shared secret embedded in the webhook path. The reader cannot present a JWT
-- and its firmware leaves `httpAuthenticationMethod` empty, so the URL itself is
-- the only credential available: without it, anything that can reach the port
-- could inject attendance records straight into payroll.
--
-- Nullable because it is minted when a device is switched to push, not for the
-- rows that already exist.
ALTER TABLE devices ADD COLUMN push_token TEXT;

-- Recreate the audit triggers so a switch between transports is recorded. The
-- original field list from 006 is preserved verbatim and `ingest_mode` appended:
-- dropping `connection_state` or `version` here would quietly shrink the audit
-- trail for every future device change.
--
-- `push_token` is DELIBERATELY absent, for the same reason 006 omits the
-- credential ciphertext: the audit trail records who changed what, never the
-- secret itself.
DROP TRIGGER IF EXISTS audit_devices_insert;
DROP TRIGGER IF EXISTS audit_devices_update;
DROP TRIGGER IF EXISTS audit_devices_delete;

CREATE TRIGGER audit_devices_insert
    AFTER INSERT ON devices
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'devices',
        NEW.id,
        'INSERT',
        NULL,
        json_object('id', NEW.id, 'name', NEW.name, 'ip', NEW.ip, 'port', NEW.port, 'scheme', NEW.scheme, 'username', NEW.username, 'direction', NEW.direction, 'allow_insecure_tls', NEW.allow_insecure_tls, 'connection_state', NEW.connection_state, 'status', NEW.status, 'version', NEW.version, 'ingest_mode', NEW.ingest_mode),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER audit_devices_update
    AFTER UPDATE ON devices
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'devices',
        NEW.id,
        'UPDATE',
        json_object('id', OLD.id, 'name', OLD.name, 'ip', OLD.ip, 'port', OLD.port, 'scheme', OLD.scheme, 'username', OLD.username, 'direction', OLD.direction, 'allow_insecure_tls', OLD.allow_insecure_tls, 'connection_state', OLD.connection_state, 'status', OLD.status, 'version', OLD.version, 'ingest_mode', OLD.ingest_mode),
        json_object('id', NEW.id, 'name', NEW.name, 'ip', NEW.ip, 'port', NEW.port, 'scheme', NEW.scheme, 'username', NEW.username, 'direction', NEW.direction, 'allow_insecure_tls', NEW.allow_insecure_tls, 'connection_state', NEW.connection_state, 'status', NEW.status, 'version', NEW.version, 'ingest_mode', NEW.ingest_mode),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER audit_devices_delete
    AFTER DELETE ON devices
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'devices',
        OLD.id,
        'DELETE',
        json_object('id', OLD.id, 'name', OLD.name, 'ip', OLD.ip, 'port', OLD.port, 'scheme', OLD.scheme, 'username', OLD.username, 'direction', OLD.direction, 'allow_insecure_tls', OLD.allow_insecure_tls, 'connection_state', OLD.connection_state, 'status', OLD.status, 'version', OLD.version, 'ingest_mode', OLD.ingest_mode),
        NULL,
        NULL,
        unixepoch()
    );
END;
