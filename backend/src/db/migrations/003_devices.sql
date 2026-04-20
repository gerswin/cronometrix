-- 003_devices.sql
-- Hikvision device registry. ISAPI credentials stored AES-256-GCM-encrypted per D-01/D-02.
-- encrypted_password is base64(nonce || ciphertext_with_tag); plaintext NEVER in this column.

CREATE TABLE IF NOT EXISTS devices (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    ip TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 443 CHECK(port BETWEEN 1 AND 65535),
    scheme TEXT NOT NULL DEFAULT 'https' CHECK(scheme IN ('http', 'https')),
    username TEXT NOT NULL,
    encrypted_password TEXT NOT NULL,
    direction TEXT NOT NULL CHECK(direction IN ('entry', 'exit')),
    allow_insecure_tls INTEGER NOT NULL DEFAULT 0 CHECK(allow_insecure_tls IN (0,1)),
    connection_state TEXT NOT NULL DEFAULT 'offline' CHECK(connection_state IN ('online','offline','unknown')),
    last_seen_at INTEGER,
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','inactive')),
    deleted_at INTEGER,
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- DEV-01: duplicate (ip, port) among active devices => 409 Conflict.
-- Partial unique index so soft-deleted rows do not block re-registration.
CREATE UNIQUE INDEX IF NOT EXISTS idx_devices_ip_port_active
    ON devices(ip, port) WHERE status = 'active';

CREATE INDEX IF NOT EXISTS idx_devices_status ON devices(status);
CREATE INDEX IF NOT EXISTS idx_devices_connection_state ON devices(connection_state);

-- D-08: face_id -> employee mapping. Phase 7 populates; Phase 2 defines schema + reads.
CREATE TABLE IF NOT EXISTS device_face_mappings (
    id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL REFERENCES devices(id),
    face_id TEXT NOT NULL,
    employee_id TEXT NOT NULL REFERENCES employees(id),
    version INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(device_id, face_id)
);
CREATE INDEX IF NOT EXISTS idx_face_mappings_employee ON device_face_mappings(employee_id);
