# Bloque 2: exactitud de la marcación — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Corregir cuatro defectos verificados que hacen que la jornada registrada no corresponda con la trabajada: la salida de turno nocturno atribuida al día equivocado (H-02), las caras desconocidas contaminando a todos los empleados (M-02), el emparejamiento por extremos con almuerzo fijo incondicional (M-03) y los permisos solo de día completo con filtros incoherentes (M-04).

**Architecture:** Cuatro tareas. H-02 va primera porque puede dejar una jornada entera sin salida; las demás son independientes entre sí.

**Tech Stack:** Rust/Axum 0.8, libSQL/SQLite, `cargo nextest`.

## Global Constraints

- **Nunca** usar los identificadores desnudos `execute`, `execute_batch` ni `transaction` dentro de `backend/src` — `scripts/check_db_write_queue.py` es un gate que falla la build.
- **Nunca romper la arquitectura hexagonal.** Nada de Hikvision en `calc/`, `reports/`, `attendance/`, `daily_records/`.
- Toda mutación de datos de asistencia genera entrada de auditoría inmutable.
- Gate de cobertura: proyecto ≥90% líneas / ≥85% ramas; por archivo ≥70% / ≥60%.
- `cargo clippy --all-targets --all-features -- -D warnings` debe pasar.
- **No hay datos productivos.** Migraciones limpias.
- Próxima migración libre: comprueba con `ls backend/src/db/migrations/`.
- **No corras `cargo fmt` a secas.** `main` no está limpio de rustfmt (`src/bin/mock_hikvision.rs`) y un archivo de test con `mod common;` arrastra el módulo compartido bajo cualquier invocación. Formatea a mano.
- Mensajes de commit en inglés con prefijo convencional.

## Contexto imprescindible

Los cuatro fueron **verificados** contra `48af434` — ver `docs/auditoria/VERIFICACION-LOTE-2.md` y la evidencia en `docs/auditoria/verificacion/lote-{1,3}.md`.

Todo lo de este bloque **llega al importe pagado**. Un defecto de exactitud aquí no produce un número raro en una pantalla: produce una nómina equivocada.

---

## File Structure

| Archivo | Responsabilidad | Tarea |
|---|---|---|
| `backend/src/events/service.rs` | Ancla de recálculo correcta para turnos nocturnos | 1 |
| `backend/src/daily_records/service.rs` | Eventos desconocidos atados al dispositivo | 2 |
| `backend/src/calc/aggregation.rs` | Emparejamiento de marcaciones | 3 |
| `backend/src/calc/lunch.rs` | Almuerzo condicional | 3 |
| `backend/src/calc/engine.rs` | Permisos parciales | 4 |
| `backend/src/reports/service.rs` | Filtro `shift_type` en la consulta secundaria | 4 |

---

### Task 1: La salida de un turno nocturno recalcula el día correcto (H-02)

`backend/src/events/service.rs:165-174` construye la petición de recálculo así:

```rust
anchor_date: captured_at.with_timezone(&state.config.timezone).date_naive()
```

Es **la fecha local del evento**. En un turno 22:00→06:00, la salida de las 06:00 del día D+1 pide recalcular D+1 — un día en el que el trabajador no empezó ninguna jornada. El turno que arrancó en D **nunca recibe su salida** y queda con `MissingExit`.

Peor: el proceso nocturno de las 02:00 consolida ese día incompleto como anomalía, y la salida de las 06:00 no lo repara porque apunta a otro sitio.

**Files:**
- Modify: `backend/src/events/service.rs`
- Test: `backend/tests/overnight_anchor_test.rs` (crear)

**Interfaces:**
- Consumes: `RecomputeRequest::Day { employee_id, anchor_date }` de `backend/src/recompute/`.
- Produces: nada nuevo. Cambia qué ancla se pide, no la forma de la petición.

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
mod common;

/// H-02: en un turno 22:00-06:00, la salida de las 06:00 pertenece a la
/// jornada que empezó AYER. Pedir el recálculo de hoy deja ayer sin salida.
#[tokio::test]
async fn an_overnight_exit_recomputes_the_day_the_shift_started() {
    // departamento con is_overnight_shift, turno 22:00-06:00
    // entrada 22:10 del dia D, salida 06:05 del dia D+1
    // -> el daily_record de D tiene entrada Y salida, sin MissingExit
    // -> D+1 no genera un registro con solo una salida
}

/// Un turno diurno no puede verse afectado por el arreglo.
#[tokio::test]
async fn a_day_shift_still_anchors_on_its_own_date() {
    // entrada 08:00 y salida 17:00 del mismo dia -> ancla ese dia
}

/// El caso limite: salida justo despues de medianoche.
#[tokio::test]
async fn an_exit_at_00_30_belongs_to_the_previous_day() { }
```

- [ ] **Step 2: Correr y ver que fallan**

Run: `cargo nextest run --all-features -j 8 -E 'binary(overnight_anchor_test)'`
Expected: la primera y la tercera fallan.

- [ ] **Step 3: Resolver el ancla por ventana de turno, no por fecha del evento**

El ancla correcta **no se puede deducir del evento solo** — depende del turno del departamento del empleado. Un evento a las 06:05 pertenece al día anterior si el departamento tiene turno nocturno, y al mismo día si no.

`backend/src/calc/aggregation.rs` ya tiene `shift_window(anchor_date, &dept, &rules, tz)`, que calcula la ventana de una jornada. **Léelo antes de escribir**: la función inversa —dado un instante, ¿a qué ancla pertenece?— probablemente deba vivir junto a ella, no en `events/service.rs`.

Criterio: si el departamento es de turno nocturno y el evento cae antes del final de la ventana que empezó el día anterior, el ancla es el día anterior. Si no, es el propio día.

**Dos cosas que hay que resolver y no adivinar:**

1. **`publish_recompute_if_employee` no tiene el departamento a mano.** Averigua de dónde sacarlo sin añadir una consulta por evento en la ruta de ingesta — esa ruta procesa ráfagas de un lector y no puede pagar una consulta extra por marcación. Si la única salida limpia es consultar, dilo y mide el coste antes de aceptarlo.
2. **Un evento ambiguo puede pertenecer a dos anclas.** Con turno nocturno, las 06:05 podrían ser la salida de ayer o una entrada muy temprana de hoy. Decide una regla, escríbela en un comentario, y **prueba ambos lados**. Si eliges recalcular las dos anclas, asegúrate de que el recálculo es idempotente.

- [ ] **Step 4: Verificar y commitear**

Run: `cargo nextest run --all-features -j 8 && cargo clippy --all-targets --all-features -j 8 -- -D warnings && python3 scripts/check_db_write_queue.py && make coverage-backend`

```bash
git add backend/src/events/service.rs backend/src/calc/ backend/tests/overnight_anchor_test.rs
git commit -m "fix(events): anchor an overnight exit to the day the shift started (H-02)"
```

---

### Task 2: Una cara desconocida deja de contaminar a todos (M-02)

`backend/src/daily_records/service.rs:123-129` selecciona los eventos de la ventana así:

```sql
WHERE (employee_id = ?1 OR (employee_id IS NULL AND is_unknown = 1))
  AND captured_at BETWEEN ?2 AND ?3
```

Cada evento de cara desconocida entra en el recálculo de **todos** los empleados cuya ventana lo solape, sin atarse siquiera al dispositivo. Una sola cara no asociada marca `UNKNOWN_FACE_IN_WINDOW` en decenas de trabajadores, y el resultado es que las anomalías dejan de significar nada.

**Files:**
- Modify: `backend/src/daily_records/service.rs`
- Test: `backend/tests/unknown_face_scope_test.rs` (crear)

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
mod common;

/// M-02: una cara desconocida no puede marcar a un empleado que fichó en otro
/// dispositivo. La anomalía debe existir, pero acotada.
#[tokio::test]
async fn an_unknown_face_does_not_flag_employees_from_other_devices() { }

/// Sigue marcando a quien SÍ comparte dispositivo y ventana — el arreglo no
/// puede silenciar la señal, solo acotarla.
#[tokio::test]
async fn an_unknown_face_still_flags_the_same_device_window() { }
```

- [ ] **Step 2: Correr y ver que fallan**

- [ ] **Step 3: Acotar por dispositivo**

Atar el evento desconocido al `device_id` de los eventos del propio empleado en esa ventana.

**Piensa el caso de que el empleado no tenga ningún evento en la ventana:** entonces no hay dispositivo con el que comparar. Decide si en ese caso la anomalía se emite o no, escríbelo en un comentario, y prueba ese caso — es exactamente donde un arreglo así se queda a medias.

- [ ] **Step 4: Verificar y commitear**

```bash
git add backend/src/daily_records/service.rs backend/tests/unknown_face_scope_test.rs
git commit -m "fix(daily-records): scope unknown-face anomalies to the device that saw them (M-02)"
```

---

### Task 3: El emparejamiento de marcaciones deja de deformar la jornada (M-03)

Dos defectos distintos en el mismo camino:

- `backend/src/calc/aggregation.rs` toma **el primer y el último** evento de la ventana como entrada y salida. Una pausa larga en medio cuenta como trabajo, y varios bloques de trabajo se colapsan en uno.
- El almuerzo nominal se descuenta **incondicionalmente**. Una jornada de tres horas pierde una hora de almuerzo que nadie tomó.

**Files:**
- Modify: `backend/src/calc/aggregation.rs`, `backend/src/calc/lunch.rs`
- Test: `backend/tests/punch_pairing_test.rs` (crear)

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
mod common;

/// M-03: entrada, salida a mediodia, vuelta, salida final. Las dos horas de
/// ausencia intermedia NO son trabajo.
#[tokio::test]
async fn a_long_midday_absence_is_not_counted_as_worked_time() { }

/// M-03: una jornada de 3 horas no pierde un almuerzo que nadie tomo.
#[tokio::test]
async fn a_short_shift_does_not_lose_a_lunch_that_never_happened() { }

/// Marcaciones impares: entrada, salida, entrada, sin salida final.
#[tokio::test]
async fn an_odd_number_of_punches_has_a_defined_outcome() {
    // decide la politica y asertala; lo que no vale es que sea accidental
}
```

- [ ] **Step 2: Correr y ver que fallan**

- [ ] **Step 3: Emparejar por pares, no por extremos**

Sustituir "primero y último" por un emparejamiento determinista entrada/salida que sume los intervalos realmente trabajados.

**Y decidir la política de marcaciones impares.** Hoy el comportamiento existe por accidente del algoritmo. Elige una regla —descartar la última huérfana, marcar anomalía, o ambas—, escríbela y pruébala.

Para el almuerzo: `lunch_mode` ya distingue `fixed` de `punch`. En modo `fixed`, **no descontar más minutos de los trabajados**, y considerar si tiene sentido descontar cuando la jornada es más corta que el propio almuerzo. Ese umbral es una decisión: documéntala.

**Este cambio moverá importes.** Los tests que asuman "primero y último" van a fallar. Cada fallo se juzga: si su valor asumía el comportamiento defectuoso, se corrige con el número calculado a mano; si falla por otra razón, es una regresión tuya. Reporta cada test tocado y en qué categoría cayó.

- [ ] **Step 4: Verificar y commitear**

```bash
git add backend/src/calc/ backend/tests/punch_pairing_test.rs
git commit -m "fix(calc): pair punches instead of taking extremes, and stop deducting phantom lunches (M-03)"
```

---

### Task 4: Permisos parciales y filtros coherentes (M-04)

Tres defectos relacionados:

- `backend/src/calc/engine.rs:15-37` pone **todos** los minutos a cero ante cualquier permiso. No existen permisos de horas.
- El filtro `shift_type` se aplica a la consulta principal del reporte pero **no** a la secundaria de permisos. Es el mismo tipo de defecto que ya corregimos en I5 con el filtro de período.
- Los contadores mezclan días calendario y días hábiles.

**Files:**
- Modify: `backend/src/calc/engine.rs`, `backend/src/reports/service.rs`
- Test: `backend/tests/partial_leave_test.rs` (crear)

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
mod common;

/// M-04: el filtro de turno debe aplicarse igual en las dos consultas del
/// reporte. Hoy la secundaria de permisos lo ignora.
#[tokio::test]
async fn the_shift_filter_applies_to_the_leave_query_too() { }

/// M-04: un permiso de medio día deja trabajada la otra mitad.
#[tokio::test]
async fn a_half_day_leave_leaves_the_other_half_worked() { }
```

- [ ] **Step 2: Correr y ver que fallan**

- [ ] **Step 3: Empezar por el filtro, que es lo barato**

El filtro `shift_type` en la consulta secundaria es una línea y cierra una incoherencia real. Hazlo primero y commitéalo aparte si quieres.

- [ ] **Step 4: Permisos parciales**

Requiere que `leaves` exprese un intervalo, no solo un día. **Mira el esquema antes de decidir**: si la tabla solo tiene `from_date`/`to_date`, esto necesita una migración y es la parte cara de la tarea.

**Si el alcance resulta mayor de lo que este plan supone, dilo y entrega solo el filtro.** Media tarea bien hecha y reportada vale más que una migración apresurada sobre datos de permisos.

- [ ] **Step 5: Verificar y commitear**

```bash
git add backend/src/calc/engine.rs backend/src/reports/service.rs backend/tests/partial_leave_test.rs
git commit -m "fix(leaves): apply the shift filter consistently and support partial-day leave (M-04)"
```

---

## Fuera de alcance

- **M-01** — la deduplicación entre dispositivos necesita una decisión de despliegue antes que un plan. Ver `docs/auditoria/HOJA-DE-RUTA.md`.
- **Bloque 3** — retención contra preservación (H-09, H-10, H-14, M-05).
- **La rotación de la clave de licencia** — acción del operador, vence antes de la primera licencia real.
