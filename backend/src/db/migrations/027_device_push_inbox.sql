-- C-10: el receptor push respondia 200 aunque la ingesta fallara, y el lector
-- avanzaba su cola dando el evento por entregado. Un fallo de base perdia la
-- marcacion para siempre.
--
-- El inbox guarda el cuerpo crudo antes de confirmar. Es deliberadamente
-- tonto: no interpreta, no valida, no deduplica.
--
-- SIN indice unico sobre body_sha256: dos cuerpos identicos pueden ser dos
-- eventos legitimos (los heartbeats lo son casi siempre) y un falso positivo
-- de deduplicacion perderia una marcacion real — exactamente el fallo que esta
-- tabla existe para impedir. La deduplicacion ya vive en bucket_30s de
-- attendance_events.
CREATE TABLE IF NOT EXISTS device_push_inbox (
    id            TEXT PRIMARY KEY,
    device_id     TEXT NOT NULL REFERENCES devices(id),
    content_type  TEXT NOT NULL,
    body          BLOB NOT NULL,
    body_sha256   TEXT NOT NULL,          -- diagnostico y correlacion, NO clave de dedup
    received_at   INTEGER NOT NULL,       -- epoch seconds UTC
    status        TEXT NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending', 'done', 'failed')),
    attempts      INTEGER NOT NULL DEFAULT 0,
    last_error    TEXT,
    processed_at  INTEGER
);

-- El drenador busca pendientes por orden de llegada.
CREATE INDEX IF NOT EXISTS idx_push_inbox_pending
    ON device_push_inbox(received_at)
    WHERE status = 'pending';

-- La cola muerta se consulta por separado y debe ser barata.
CREATE INDEX IF NOT EXISTS idx_push_inbox_failed
    ON device_push_inbox(received_at)
    WHERE status = 'failed';
