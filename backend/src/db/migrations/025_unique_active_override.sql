-- C-04: podían coexistir varias anulaciones activas por registro, y el LEFT
-- JOIN del reporte multiplicaba la fila, duplicando minutos e importes.
--
-- Índice parcial: solo restringe las activas, así que el histórico revocado se
-- acumula sin límite. La anulación es evidencia y no se borra nunca.
--
-- Sin paso de resolución de duplicados: no hay instalaciones productivas
-- (decisión del dueño del producto, 2026-08-02). Si alguna vez se aplica sobre
-- una base con datos, este CREATE fallará — y fallar es lo correcto: obliga a
-- decidir conscientemente qué anulación gana.
CREATE UNIQUE INDEX IF NOT EXISTS idx_overrides_one_active_per_record
    ON daily_record_overrides(daily_record_id)
    WHERE status = 'active' AND deleted_at IS NULL;
