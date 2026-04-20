-- 006_devices_audit_triggers.sql
-- Audit triggers for the `devices` table. Mirrors the shape of 002_audit_triggers.sql.
--
-- CRITICAL: the AES-GCM credential ciphertext column is DELIBERATELY omitted from
-- the json_object payload, per RESEARCH § Security Domain → Credential Handling
-- Rules rule #4. The audit trail records who/when/what fields changed; it does
-- NOT record the ciphertext.
--
-- device_face_mappings triggers are deferred to Phase 7 (enrollment).
-- command_audit_log is append-only by convention — no meta-audit triggers.

CREATE TRIGGER IF NOT EXISTS audit_devices_insert
    AFTER INSERT ON devices
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'devices',
        NEW.id,
        'INSERT',
        NULL,
        json_object('id', NEW.id, 'name', NEW.name, 'ip', NEW.ip, 'port', NEW.port, 'scheme', NEW.scheme, 'username', NEW.username, 'direction', NEW.direction, 'allow_insecure_tls', NEW.allow_insecure_tls, 'connection_state', NEW.connection_state, 'status', NEW.status, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_devices_update
    AFTER UPDATE ON devices
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'devices',
        NEW.id,
        'UPDATE',
        json_object('id', OLD.id, 'name', OLD.name, 'ip', OLD.ip, 'port', OLD.port, 'scheme', OLD.scheme, 'username', OLD.username, 'direction', OLD.direction, 'allow_insecure_tls', OLD.allow_insecure_tls, 'connection_state', OLD.connection_state, 'status', OLD.status, 'version', OLD.version),
        json_object('id', NEW.id, 'name', NEW.name, 'ip', NEW.ip, 'port', NEW.port, 'scheme', NEW.scheme, 'username', NEW.username, 'direction', NEW.direction, 'allow_insecure_tls', NEW.allow_insecure_tls, 'connection_state', NEW.connection_state, 'status', NEW.status, 'version', NEW.version),
        NULL,
        unixepoch()
    );
END;

CREATE TRIGGER IF NOT EXISTS audit_devices_delete
    AFTER DELETE ON devices
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'devices',
        OLD.id,
        'DELETE',
        json_object('id', OLD.id, 'name', OLD.name, 'ip', OLD.ip, 'port', OLD.port, 'scheme', OLD.scheme, 'username', OLD.username, 'direction', OLD.direction, 'allow_insecure_tls', OLD.allow_insecure_tls, 'connection_state', OLD.connection_state, 'status', OLD.status, 'version', OLD.version),
        NULL,
        NULL,
        unixepoch()
    );
END;
