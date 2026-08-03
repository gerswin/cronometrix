-- C-05: `status` ('active'/'inactive') no lleva fecha, así que el reporte no
-- podía saber CUÁNDO dejó de trabajar alguien. Filtrando por status='active'
-- un empleado que egresó a mitad del período desaparecía junto con los días
-- que trabajó y se le deben; sin filtrar, acumulaba ausencias después de irse.
--
-- `status` sigue siendo útil para la interfaz (a quién ofrecer en un
-- selector). Deja de gobernar el reporte: la pregunta "¿quién cobra este
-- período?" es de vigencia, no de estado actual.
ALTER TABLE employees ADD COLUMN terminated_on INTEGER;  -- epoch seconds UTC, nullable = sigue activo

-- Backfill: `deactivate_queued` already stamps `deleted_at = unixepoch()` at
-- the exact moment status flips to 'inactive' — that IS the termination
-- event. Employees deactivated before this migration existed would
-- otherwise read as `terminated_on IS NULL`, which the report treats as
-- "still employed" (nullable = sigue activo, see column comment above) and
-- would resume accruing absences for them in every future period — the
-- exact defect this migration exists to close, just for older rows. No
-- production installs exist yet (see migration 025), so this is a
-- correctness guard rather than a live-data fix.
UPDATE employees
   SET terminated_on = deleted_at
 WHERE status = 'inactive'
   AND terminated_on IS NULL
   AND deleted_at IS NOT NULL;
