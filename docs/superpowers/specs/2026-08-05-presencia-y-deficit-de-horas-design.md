# Presencia en vivo y déficit de horas

Fecha: 2026-08-05
Estado: aprobado, pendiente de plan de implementación

## Problema

El dashboard responde *cuántos* empleados están presentes, nunca *quiénes*. El
KPI "Empleados Presentes" cuenta `work_minutes > 0` sobre los `daily_records`
de hoy (`frontend/src/lib/kpi-utils.ts`) y ahí se acaba: no hay lista, y no se
distingue entre quien está en el sitio ahora mismo y quien ya se fue.

Tampoco existe el concepto de jornada esperada. `/reports` entrega horas
trabajadas, extras, tardanza y días ausentes, pero nada contra qué compararlas,
así que no se puede responder quién no cumplió su jornada ni por cuánto. El
supervisor lo calcula a mano.

## Alcance

1. Saber quién está dentro ahora y quién asistió hoy, con nombre y hora.
2. Saber, por empleado, cuánto tiempo de jornada quedó sin cumplir — tanto en
   el día (dashboard) como en el periodo de nómina (`/reports`, Excel incluido).

Fuera de alcance: calendario laboral configurable, congelar la jornada esperada
en el histórico, y notificaciones o alertas por déficit.

## Definiciones

Las tres reglas que gobiernan todo lo demás. Fueron decisiones explícitas, no
supuestos.

**Jornada esperada de un día** = `shift_end_time − shift_start_time − almuerzo`,
del departamento del empleado. Sábado y domingo esperan 0. Un día con permiso
aprobado espera 0 — el déficit mide incumplimiento real, no vacaciones.

**Déficit** = `max(0, esperada − trabajada)`. Nunca negativo: trabajar de más no
compensa un día corto. Las horas extra ya tienen su propia columna y su propia
regla de pago; mezclarlas aquí escondería el incumplimiento.

**Presencia**, dos métricas distintas:
- *Dentro ahora*: tiene `entry_at` de hoy y no tiene `exit_at`.
- *Asistieron hoy*: tiene `entry_at` de hoy, se haya ido o no.

### Casos límite decididos

| Caso | Resuelto como |
|---|---|
| Turno nocturno (`shift_end < shift_start`) | Cruza medianoche: se suman 24 h antes de restar. Un 22:00→06:00 espera 480 min, no −960. |
| `lunch_mode = 'fixed'` | Se resta `lunch_duration_min`. |
| `lunch_mode = 'punch'` | Se resta `lunch_duration_min` si está definido; si es NULL, no se resta nada. El almuerzo punchado ya queda fuera de `work_minutes` por el pareo de intervalos (`calc/aggregation.rs`), así que restarlo otra vez lo contaría dos veces. |
| Almuerzo ≥ duración del turno | La esperada es 0, nunca negativa. Configuración inválida, pero no debe envenenar los subtotales. |
| Empleado sin departamento | Esperada 0 y déficit 0. No hay horario del que derivarla; inventarlo produciría un déficit falso. |
| Contratado a mitad de periodo | Los días fuera de `hire_date`..`terminated_on` esperan 0, igual que ya hace `/reports` para decidir qué filas existen. |

## Arquitectura

El cálculo vive en el backend; el frontend solo pinta. Una única definición de
"esperada" alimenta las dos superficies, que así no pueden divergir.

### `backend/src/calc/expected.rs` — nuevo

Función pura, sin acceso a base de datos:

```rust
pub fn expected_minutes(
    shift_start: &str,     // "HH:MM"
    shift_end: &str,       // "HH:MM"
    lunch_mode: &str,      // "fixed" | "punch"
    lunch_duration_min: Option<i64>,
    date: NaiveDate,
    has_leave: bool,
) -> i64
```

Es el único sitio donde vive la regla. Se prueba sola, sin servidor ni fixtures.

### `GET /api/v1/presence/today` — nuevo

Va en `viewer_routes` (cualquier rol autenticado lee, D-09) y aplica
`ActorScope` como el resto de lecturas tras H-11: un supervisor con
`department_id` ve solo su departamento; admin y usuarios org-wide ven todo.
Filtrar es obligatorio, no opcional — una consulta sin scope es un bug.

```json
{
  "date": "2026-08-05",
  "inside_now": 12,
  "attended_today": 27,
  "data": [
    {
      "employee_id": "emp-ana",
      "employee_name": "Ana Pérez",
      "department_name": "Producción",
      "status": "inside",
      "entry_at": "2026-08-05T12:02:00+00:00",
      "exit_at": null,
      "expected_min": 480,
      "worked_min": 210,
      "deficit_min": 270
    }
  ]
}
```

Una sola consulta con join a `departments` y `leaves`; nada de N+1.

### `/reports` — campos nuevos

`expected_min` y `deficit_min` se suman a `Aggregates`
(`backend/src/reports/models.rs:73`), de modo que aparecen a la vez en las filas
por empleado, en los subtotales por departamento y en el gran total. La UI y el
Excel (`reports/excel.rs`) ganan las columnas **Esperadas**, **Trabajadas** y
**Déficit**.

`Aggregates` es `Default`, así que agregar campos no rompe a quien lo construye
por partes; sí hay que actualizar los sitios que lo suman.

### Frontend

- `components/dashboard/presence-table.tsx` — nuevo: tabla fija bajo los KPI,
  con pestañas *Dentro ahora* / *Asistieron hoy*, columnas empleado, entrada,
  departamento. Consume `/presence/today` con TanStack Query.
- `components/dashboard/deficit-panel.tsx` — nuevo: quién no cumplió hoy y
  cuántos minutos, ordenado por déficit descendente.
- `kpi-utils.ts` — `aggregateKPIs` deja de derivar `present` de `work_minutes`;
  los dos contadores vienen del endpoint.
- La tabla de `/reports` y su exportación reciben las tres columnas nuevas.

## Errores

- Departamento sin horario o con horario inválido → esperada 0, sin romper la
  respuesta. Un dato mal cargado no debe tumbar el dashboard entero, igual que
  `require_salary_kind` decidió no fallar el reporte completo por una fila sin
  unidad (`reports/service.rs:65`).
- Sin registros hoy → `data: []`, contadores en 0. La tabla muestra su estado
  vacío.
- El endpoint hereda el timeout y el middleware de licencia del grupo de rutas
  donde se monta.

## Pruebas

**Unitarias (`calc/expected.rs`)**: turno diurno normal; nocturno cruzando
medianoche; sábado y domingo; día con permiso; `fixed` vs `punch`; almuerzo más
largo que el turno; horario malformado.

**Integración backend**: `/presence/today` distingue dentro/salió; un supervisor
con departamento no ve empleados de otro (el caso de scope es obligatorio tras
H-11); `/reports` cuadra `expected_min` y `deficit_min` en fila, subtotal y gran
total; el Excel trae las columnas nuevas.

**Frontend**: las pestañas cambian de lista; el panel de déficit ordena y
formatea minutos como `Xh Ym`; estados vacíos.

**E2E**: login como `demo_admin` → el dashboard muestra la tabla de presencia
con datos del seed; login como supervisor → solo su departamento.

## Criterio de éxito

Un supervisor abre el dashboard y responde sin calcular nada: quién está en el
sitio, quién vino hoy, y quién debe tiempo y cuánto. La misma cifra de déficit
aparece en el reporte del periodo y en su Excel.
