# Exactitud monetaria: C-01 a C-05 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Corregir los cinco críticos verificados que hacen que el reporte pague mal: horas extra al 250% (C-01), doble descuento por tardanza (C-02), salario heredado que queda en cero (C-03), anulaciones activas múltiples que duplican filas (C-04) y el empleado sin marcas que desaparece del reporte (C-05).

**Architecture:** Cuatro tareas independientes. Las dos primeras tocan el motor de minutos y el bloque monetario del reporte; las dos últimas son integridad de datos (índice único con migración) y forma de la consulta. Ninguna cambia el esquema de `daily_records` — `overtime_minutes` conserva su semántica actual (el excedente), porque los acumuladores semanal y anual ya la usan así.

**Tech Stack:** Rust/Axum 0.8, libSQL/SQLite, migraciones SQL numeradas, `cargo nextest`.

## Global Constraints

- **Nunca** usar los identificadores desnudos `execute`, `execute_batch` ni `transaction` dentro de `backend/src` — `scripts/check_db_write_queue.py` es un gate que falla la build. Toda escritura va por `state.db_write.statement(...)` o `state.db_write.transact(...)`; dentro de una transacción, `tx.query(...)` y `tx.statement(...)`.
- **Nunca romper la arquitectura hexagonal.** El detalle de vendedor vive solo en `backend/src/isapi/*`. Ninguna tarea de este plan puede meter tipos, formatos ni códigos de Hikvision en `attendance/`, `reports/`, `calc/` ni `daily_records/`. Ver `docs/ARQUITECTURA-HEXAGONAL.md`.
- Toda mutación de datos de asistencia genera entrada de auditoría inmutable con justificación.
- Gate de cobertura duro: proyecto ≥90% líneas / ≥85% ramas; por archivo ≥70% / ≥60%. Correr `make coverage-backend`.
- `cargo clippy --all-targets --all-features -- -D warnings` debe pasar.
- Los mensajes de commit van en inglés con prefijo convencional.
- Ninguna migración nueva puede reescribir ni borrar evidencia histórica. Las correcciones se registran, no se sobrescriben.

## Contexto imprescindible

Los cinco hallazgos fueron **verificados** contra el código actual — ver `docs/auditoria/VERIFICACION-Y-PLAN.md`. No son sospechas.

Tres cosas que hay que saber antes de escribir una línea:

1. **Las pruebas actuales consolidan la especificación equivocada.** `backend/src/calc/overtime.rs:39-45` afirma la suma diaria errónea, y la prueba QA E3 acepta el pago duplicado. **"La suite pasa" no es evidencia de corrección aquí.** Cada tarea que toque dinero debe empezar por casos calculados a mano, y corregir las pruebas viejas junto con el código — no ajustar el código hasta que la suite vuelva a verde.

2. **`money::` tiene un solo llamador.** `backend/src/reports/service.rs`, 7 sitios. Verificado con el grafo del código y confirmado por grep. El radio de impacto es mucho menor de lo que sugiere la auditoría.

3. **`overtime_minutes` es un subconjunto de `work_minutes`, no algo aparte.** `calc/engine.rs:82` lo define como `(work_minutes - ordinary_daily_minutes).max(0)`. Esa semántica **se conserva** en este plan: `daily_records/service.rs:158` y `:180` la usan para los topes semanal y anual, y cambiarla los rompería en silencio. Lo que se corrige es la **base de pago**, no el significado de la columna.

### Decisiones ya tomadas (no re-litigar)

- **C-02 — tardanza:** se paga el tiempo realmente trabajado y se **elimina** `late_deduction_cents` del total. `late_minutes` se sigue registrando como métrica para disciplina; solo deja de monetizarse. Razón: descontar solo el tiempo no trabajado no es una deducción (el salario no se causa); una deducción punitiva adicional sería doble sanción por el mismo hecho, y la LOTTT ya da el remedio disciplinario (art. 79; Reglamento art. 38: cuatro retardos en un mes como causal). **Pendiente de confirmación por abogado laboral venezolano** — la fuente es doctrina de divulgación, no jurisprudencia.
- **C-03 — salario:** el salario vacío se **rechaza**, no se hereda. Sin herencia departamental.

---

## File Structure

| Archivo | Responsabilidad | Tarea |
|---|---|---|
| `backend/src/calc/overtime.rs` | Tope diario deja de sumar dos veces | 1 |
| `backend/src/reports/service.rs` | Base de pago ordinaria; sin descuento por tardanza | 1 |
| `backend/src/reports/money.rs` | Doc de `late_deduction_cents` (queda, sin usar en el total) | 1 |
| `backend/src/employees/service.rs` | Salario obligatorio y positivo | 2 |
| `backend/src/db/migrations/024_employee_salary_not_null.sql` | Marca los salarios en cero existentes | 2 |
| `backend/src/db/migrations/025_unique_active_override.sql` | Índice único parcial + resolución de duplicados | 3 |
| `backend/src/daily_records/handlers.rs` | Sustitución transaccional de anulación | 3 |
| `backend/src/reports/service.rs` | Universo desde empleados activos | 4 |

---

### Task 1: La hora extra deja de pagarse al 250% y la tardanza deja de descontarse dos veces (C-01, C-02)

Hoy el reporte paga `work_minutes` completos a tarifa ordinaria y **además** suma `overtime_minutes` al 150%. Como los extra están dentro de `work_minutes`, cada minuto extra cobra 100% + 150% = **250%**. Aparte, `late_deduction_cents` resta otra vez unos minutos que el trabajador ya no cobró, porque `work_minutes` es tiempo real entre entrada y salida.

Con jornada de 480 min, 60 min extra y salario diario de 50: el código produce **65,625** (56,25 por 540 min + 9,375 de extra). Lo correcto bajo recargo de 50% es **59,375** (50 de jornada ordinaria + 9,375 de extra).

**Files:**
- Modify: `backend/src/calc/overtime.rs:15-29` y sus tests en el mismo archivo
- Modify: `backend/src/reports/service.rs:293-341`
- Modify: `backend/src/reports/money.rs` (solo documentación)
- Test: `backend/tests/reports_money_correctness_test.rs` (crear)

**Interfaces:**
- Consumes: `money::work_pay_cents`, `money::ot_pay_cents`, `money::total_a_pagar_cents` de `backend/src/reports/money.rs`.
- Produces: nada nuevo hacia otras tareas. `overtime_minutes` conserva su semántica.

- [ ] **Step 1: Escribir los casos calculados a mano**

Crear `backend/tests/reports_money_correctness_test.rs`. Estos números se derivan de la LOTTT, no del código actual — si el código no coincide, el código está mal:

```rust
//! C-01/C-02: los importes de este archivo se calcularon a mano desde la LOTTT,
//! NO se derivaron del comportamiento existente. La suite anterior codificaba la
//! especificación equivocada, así que no sirve como referencia.

use cronometrix_api::reports::money::{ot_pay_cents, total_a_pagar_cents, work_pay_cents};

/// Jornada 480 min, 60 min extra, salario diario 50,00 (5000 centavos).
/// LOTTT 118: la hora extra lleva recargo mínimo de 50%, o sea 1,5x.
///   ordinarios: 480 min -> 5000
///   extra:       60 min a 1,5x -> 60*5000*150/(100*480) = 937 (9,375 truncado)
///   total esperado: 5937
/// El comportamiento defectuoso producía 6562 (56,25 + 9,375) — la hora extra
/// cobrada al 250%.
#[test]
fn overtime_is_paid_once_at_150_percent_not_250() {
    let ordinary_minutes = 480_i64;
    let overtime_minutes = 60_i64;
    let base = 5_000_i64;
    let ord_day = 480_i64;

    let work = work_pay_cents(ordinary_minutes, base, ord_day);
    let ot = ot_pay_cents(overtime_minutes, base, ord_day);
    let total = total_a_pagar_cents(work, ot, 0, 0, 0);

    assert_eq!(work, 5_000, "los minutos ordinarios se pagan una sola vez");
    assert_eq!(ot, 937, "60 min extra a 1,5x sobre jornada de 480");
    assert_eq!(total, 5_937);
    assert_ne!(total, 6_562, "250% — el defecto C-01");
}

/// C-02: llegar 30 min tarde y salir a la hora nominal produce 450 min
/// trabajados. Se pagan 450. No se resta nada más: el salario de esos 30 min
/// simplemente no se causó.
#[test]
fn lateness_costs_the_unworked_minutes_and_nothing_more() {
    let worked = 450_i64;
    let base = 5_000_i64;
    let ord_day = 480_i64;

    let work = work_pay_cents(worked, base, ord_day);
    let total = total_a_pagar_cents(work, 0, 0, 0, 0);

    assert_eq!(work, 4_687);
    assert_eq!(total, 4_687, "sin deducción punitiva adicional");
    assert_ne!(total, 4_375, "doble descuento — el defecto C-02");
}
```

- [ ] **Step 2: Correr y ver que fallan**

Run: `cargo nextest run --all-features -E 'binary(reports_money_correctness_test)'`
Expected: compilan y pasan — estas funciones puras ya son correctas por separado. **El defecto no está en `money.rs`, está en qué minutos le pasa `reports/service.rs`.** Si alguna falla, para y repórtalo: significa que la aritmética base también está mal y este plan asume que no.

- [ ] **Step 3: Corregir la base de pago en el reporte**

En `backend/src/reports/service.rs`, dentro del brazo `_ =>` (día laboral estándar), la llamada a `work_pay_cents` recibe hoy `effective_work_min`, que **incluye** los minutos extra. Separar:

```rust
                // C-01: `overtime_minutes` es un SUBCONJUNTO de los minutos
                // trabajados (calc/engine.rs:82). Pagar el total a tarifa
                // ordinaria y sumar además el extra al 150% cobra cada minuto
                // extraordinario al 250%. La base ordinaria excluye el extra;
                // el recargo lo aporta `ot_pay_cents`.
                let ordinary_min = (effective_work_min - overtime_minutes).max(0);
                let work_pay = money::work_pay_cents(
                    ordinary_min,
                    base_salary_cents,
                    ordinary_daily_minutes,
                );
```

**Ojo con la anulación:** `effective_work_min` puede venir de `override_work_minutes`, mientras `overtime_minutes` es el valor original del cálculo. Si una anulación reduce los minutos por debajo del extra original, `ordinary_min` se satura en 0 — de ahí el `.max(0)`. Que la anulación no recalcule el extra es el hallazgo H-07, **fuera del alcance de este plan**; no lo arregles aquí, pero deja este comentario para que el siguiente lector sepa que es conocido.

Los tres usos restantes de `effective_work_min` (prima nocturna, recargo dominical y el agregado `entry.agg.work_min`) **no cambian**: la prima nocturna del art. 117 se causa sobre toda la jornada nocturna, y el agregado informa minutos trabajados, no base de pago.

- [ ] **Step 4: Quitar la tardanza del total**

En el mismo bloque, `late` deja de entrar en `total_a_pagar_cents`:

```rust
                // C-02: la tardanza YA se reflejó en los minutos trabajados
                // (calc/engine.rs:70-76 mide entre entrada y salida reales), así
                // que restar late_minutes otra vez descuenta dos veces el mismo
                // hecho. `late_minutes` se conserva como métrica de disciplina;
                // deja de monetizarse. Decisión registrada en el plan; pendiente
                // de confirmación laboral.
                let total = money::total_a_pagar_cents(work_pay, ot_pay, night, rest, 0);
```

Mantener `entry.agg.late_deduction_cents` alimentándose de `money::late_deduction_cents(...)` **solo si el reporte lo expone como columna informativa**; si al leer el `struct` ves que únicamente alimentaba el total, ponlo en 0 y borra el cálculo. Decide leyendo la definición de la struct de agregado — no adivines.

- [ ] **Step 5: Corregir el tope diario que suma dos veces**

`backend/src/calc/overtime.rs:22` evalúa `work_minutes + overtime_minutes > 600`, pero `work_minutes` ya contiene el extra: con 480 ordinarios y 121 extra el total real es 601, y el código evalúa 601+121=722, disparando la anomalía antes de tiempo. Corregir:

```rust
    // LOTTT 178: el tope es de 10 h EFECTIVAS al día. `work_minutes` ya incluye
    // los extraordinarios (calc/engine.rs:82), así que sumarlos otra vez evalúa
    // una jornada que nadie trabajó.
    if work_minutes > 600 {
        out.push(AnomalyCode::OtCapExceededDaily);
    }
```

Y corregir la prueba del mismo archivo, que hoy afirma la suma errónea:

```rust
    #[test]
    fn daily_cap_triggers_only_when_the_real_workday_exceeds_600() {
        // 600 exactos, de los cuales 120 extraordinarios — no excede.
        assert!(check_overtime_caps(600, 120, 0, 0).is_empty());
        // 601 — un minuto por encima del tope legal.
        let out = check_overtime_caps(601, 121, 0, 0);
        assert!(out.contains(&AnomalyCode::OtCapExceededDaily));
    }
```

Los topes semanal y anual **no se tocan**: suman `overtime_minutes`, que es el excedente, y esa lectura es correcta.

- [ ] **Step 6: Encontrar y corregir el resto de pruebas que afirman lo viejo**

Run: `cargo nextest run --all-features 2>&1 | tail -40`

Habrá fallos en pruebas que codificaban el comportamiento defectuoso. **Cada una hay que juzgarla, no ajustarla:** si su valor esperado corresponde a la hora extra al 250% o al doble descuento, el valor esperado estaba mal y se corrige con el número calculado a mano. Si una prueba falla por otra razón, es una regresión tuya y hay que arreglar el código. Documenta en el reporte cada prueba tocada y en qué categoría cayó.

- [ ] **Step 7: Verificar todo**

Run: `cargo nextest run --all-features && cargo clippy --all-targets --all-features -- -D warnings && python3 scripts/check_db_write_queue.py && make coverage-backend`
Expected: suite verde, clippy limpio, 0 violaciones, gate de cobertura pasando.

- [ ] **Step 8: Commit**

```bash
git add backend/src/calc/overtime.rs backend/src/reports/service.rs backend/src/reports/money.rs backend/tests/reports_money_correctness_test.rs
git commit -m "fix(reports): pay overtime once at 150% and stop double-charging lateness (C-01, C-02)"
```

---

### Task 2: El salario vacío se rechaza en vez de guardarse como cero (C-03)

`backend/src/employees/service.rs:105` hace `req.base_salary_cents.unwrap_or(0)`. La interfaz promete que un salario vacío hereda el del departamento; el backend guarda **0** y el reporte lee solo el salario del empleado. Resultado: nómina en cero para trabajadores válidos, sin ningún aviso.

Decisión tomada: **no hay herencia**. El salario es obligatorio y debe ser positivo.

**Files:**
- Modify: `backend/src/employees/service.rs` (creación ~línea 105 y actualización ~línea 306)
- Modify: el DTO de creación en `backend/src/employees/models.rs`
- Create: `backend/src/db/migrations/024_employee_salary_not_null.sql`
- Test: `backend/tests/employees_salary_required_test.rs` (crear)

**Interfaces:**
- Consumes: `AppError::Validation { code, message }`.
- Produces: `base_salary_cents` deja de poder ser 0 o negativo para empleados nuevos.

- [ ] **Step 1: Escribir la prueba que falla**

Crear `backend/tests/employees_salary_required_test.rs`. Adapta los helpers a los que existan en `backend/tests/common/mod.rs` — el compilador manda:

```rust
mod common;

/// C-03: un salario ausente se guardaba como 0 y producía nómina en cero sin
/// aviso. Ahora es un error de validación, no un valor por defecto.
#[tokio::test]
async fn creating_an_employee_without_salary_is_rejected() {
    // construir la request SIN base_salary_cents y esperar 422/400 de validación,
    // con code == "SALARY_REQUIRED"
}

#[tokio::test]
async fn zero_and_negative_salaries_are_rejected() {
    // base_salary_cents = 0 y = -1 deben fallar ambos
}

#[tokio::test]
async fn a_positive_salary_is_accepted_and_persisted_exactly() {
    // 150_000 centavos entra y sale como 150_000, sin redondeo ni conversión
}
```

- [ ] **Step 2: Correr y ver que falla**

Run: `cargo nextest run --all-features -E 'binary(employees_salary_required_test)'`
Expected: los dos primeros fallan — hoy se aceptan y persisten como 0.

- [ ] **Step 3: Exigir el salario en creación**

En `backend/src/employees/service.rs`, sustituir el `unwrap_or(0)`:

```rust
    // C-03: un salario ausente NO es cero. La interfaz prometía herencia
    // departamental que el backend nunca implementó, así que el valor por
    // defecto producía nómina en cero en silencio. Ahora es obligatorio.
    let salary = req.base_salary_cents.ok_or_else(|| AppError::Validation {
        code: "SALARY_REQUIRED",
        message: "base_salary_cents is required and must be greater than zero".to_string(),
    })?;
    if salary <= 0 {
        return Err(AppError::Validation {
            code: "SALARY_INVALID",
            message: "base_salary_cents must be greater than zero".to_string(),
        });
    }
```

- [ ] **Step 4: Aplicar el mismo límite en actualización**

En el bloque de actualización (~línea 306), un `Some(salary)` con valor `<= 0` debe rechazarse con el mismo `SALARY_INVALID`. No conviertas silenciosamente a `None`.

- [ ] **Step 5: Migración que marca los datos ya corruptos**

Crear `backend/src/db/migrations/024_employee_salary_not_null.sql`. **No inventa un salario** — no hay forma de saber cuál era. Marca las filas para que un humano las corrija:

```sql
-- C-03: los empleados creados antes de esta corrección pudieron quedar con
-- base_salary_cents = 0 porque la API convertía un salario ausente en cero.
-- No se puede inferir el valor correcto, así que NO se inventa: se registra la
-- anomalía para revisión humana y se impide que vuelva a ocurrir.
--
-- No se añade CHECK(base_salary_cents > 0) a la tabla: SQLite exige recrearla
-- para eso, y las filas existentes en cero lo harían fallar. La validación vive
-- en la capa de servicio; esta migración solo deja el rastro.

INSERT INTO audit_log (id, table_name, record_id, action, actor_id, changes, created_at)
SELECT
    lower(hex(randomblob(16))),
    'employees',
    id,
    'DATA_ANOMALY',
    NULL,
    json_object(
        'finding', 'C-03',
        'detail', 'base_salary_cents is zero — created before salary became mandatory; requires human correction'
    ),
    unixepoch()
FROM employees
WHERE base_salary_cents <= 0 AND deleted_at IS NULL;
```

**Antes de escribirla, lee `backend/src/db/migrations/001_initial_schema.sql` y confirma los nombres reales de las columnas de `audit_log`.** Si no coinciden con los de arriba, usa los reales — no fuerces el esquema al SQL de este plan.

- [ ] **Step 6: Verificar**

Run: `cargo nextest run --all-features && cargo clippy --all-targets --all-features -- -D warnings && make coverage-backend`

- [ ] **Step 7: Commit**

```bash
git add backend/src/employees/ backend/src/db/migrations/024_employee_salary_not_null.sql backend/tests/employees_salary_required_test.rs
git commit -m "fix(employees): require a positive salary instead of defaulting to zero (C-03)"
```

---

### Task 3: Una sola anulación activa por registro (C-04)

`009_daily_record_overrides.sql:24` crea `idx_overrides_record` sobre `daily_record_id` **sin `UNIQUE`**, y filtra por `deleted_at IS NULL`, no por `status`. Nada impide varias anulaciones `active` sobre el mismo registro, y `reports/service.rs:196` hace `LEFT JOIN ... AND dro.status = 'active'` — cada anulación activa extra **multiplica la fila** del reporte, duplicando días, minutos e importes.

**Files:**
- Create: `backend/src/db/migrations/025_unique_active_override.sql`
- Modify: `backend/src/daily_records/handlers.rs:203-247` (inserción)
- Test: `backend/tests/override_uniqueness_test.rs` (crear)

**Interfaces:**
- Consumes: `state.db_write.transact(...)`.
- Produces: invariante — como máximo una fila con `status='active'` por `daily_record_id`.

- [ ] **Step 1: Escribir la prueba que falla**

```rust
mod common;

/// C-04: dos anulaciones sobre el mismo registro. La segunda debe revocar la
/// primera, no coexistir con ella — coexistiendo, el LEFT JOIN del reporte
/// multiplica la fila y duplica los importes.
#[tokio::test]
async fn a_second_override_revokes_the_first() {
    // crear daily_record, aplicar anulación A, aplicar anulación B,
    // y asertar: COUNT(*) WHERE status='active' == 1, y que la activa es B
}

/// La evidencia no se borra: la anulación revocada sigue existiendo.
#[tokio::test]
async fn the_revoked_override_is_kept_for_audit() {
    // tras revocar A, sigue existiendo una fila para A con status='revoked'
}
```

- [ ] **Step 2: Correr y ver que falla**

Run: `cargo nextest run --all-features -E 'binary(override_uniqueness_test)'`
Expected: la primera falla con 2 anulaciones activas.

- [ ] **Step 3: Migración — resolver duplicados y luego imponer el índice**

Crear `backend/src/db/migrations/025_unique_active_override.sql`. **El orden importa:** crear el índice único primero fallaría en cualquier base que ya tenga duplicados.

```sql
-- C-04: podían coexistir varias anulaciones activas por registro diario, y el
-- LEFT JOIN del reporte multiplicaba la fila, duplicando minutos e importes.
--
-- Paso 1: conservar como activa solo la más reciente de cada registro y revocar
-- las anteriores. No se borra ninguna: la anulación es evidencia y LOTTT exige
-- conservarla.
UPDATE daily_record_overrides
   SET status = 'revoked',
       updated_at = unixepoch()
 WHERE status = 'active'
   AND deleted_at IS NULL
   AND id NOT IN (
       SELECT id FROM (
           SELECT id,
                  ROW_NUMBER() OVER (
                      PARTITION BY daily_record_id
                      ORDER BY created_at DESC, id DESC
                  ) AS rn
             FROM daily_record_overrides
            WHERE status = 'active' AND deleted_at IS NULL
       ) WHERE rn = 1
   );

-- Paso 2: impedir que vuelva a ocurrir. Índice parcial: solo restringe las
-- activas, así que el histórico revocado puede acumularse sin límite.
CREATE UNIQUE INDEX IF NOT EXISTS idx_overrides_one_active_per_record
    ON daily_record_overrides(daily_record_id)
    WHERE status = 'active' AND deleted_at IS NULL;
```

- [ ] **Step 4: Sustitución transaccional en el handler**

En `backend/src/daily_records/handlers.rs`, la inserción de una anulación debe revocar la activa anterior **y** insertar la nueva en la misma transacción — si no, el índice único hace fallar la inserción legítima:

```rust
    state
        .db_write
        .transact("daily_records.replace-override", move |tx| {
            Box::pin(async move {
                // C-04: revocar antes de insertar, en la misma transacción. El
                // índice único parcial rechazaría la inserción si quedara otra
                // activa, y hacerlo en dos operaciones deja una ventana donde
                // no hay ninguna activa.
                tx.statement(
                    "UPDATE daily_record_overrides \
                        SET status = 'revoked', updated_at = unixepoch() \
                      WHERE daily_record_id = ?1 AND status = 'active' AND deleted_at IS NULL",
                    libsql::params![daily_record_id.clone()],
                )
                .await?;
                tx.statement(
                    /* el INSERT existente, sin cambios */,
                    /* los mismos parámetros */,
                )
                .await?;
                Ok(())
            })
        })
        .await
        .map_err(AppError::from)?;
```

Copia el INSERT existente **verbatim** desde el código actual; no lo reescribas de memoria.

- [ ] **Step 5: Verificar**

Run: `cargo nextest run --all-features && cargo clippy --all-targets --all-features -- -D warnings && python3 scripts/check_db_write_queue.py && make coverage-backend`

- [ ] **Step 6: Commit**

```bash
git add backend/src/db/migrations/025_unique_active_override.sql backend/src/daily_records/handlers.rs backend/tests/override_uniqueness_test.rs
git commit -m "fix(overrides): allow only one active override per record (C-04)"
```

---

### Task 4: El empleado sin marcas aparece como ausente (C-05)

`reports/service.rs:193-194` hace `FROM daily_records dr JOIN employees e`. El universo del reporte nace de los registros diarios, así que un empleado activo que no marcó **ningún** día simplemente no aparece: ni ausencia, ni descuento, ni alerta. Es además manipulable — basta con que no lleguen eventos.

**Files:**
- Modify: `backend/src/reports/service.rs` (la consulta principal y el acumulador)
- Test: `backend/tests/reports_absent_employee_test.rs` (crear)

**Interfaces:**
- Consumes: la forma de fila existente del acumulador.
- Produces: filas de reporte para empleados sin ningún `daily_record` en el período.

- [ ] **Step 1: Escribir la prueba que falla**

```rust
mod common;

/// C-05: un empleado activo que no marcó ningún día del período debe aparecer
/// como ausente. Hoy desaparece del reporte, y con él su descuento y su alerta.
#[tokio::test]
async fn an_employee_with_no_events_at_all_still_appears_as_absent() {
    // sembrar dos empleados activos; generar eventos SOLO para el primero;
    // pedir el reporte del período y asertar que ambos aparecen,
    // el segundo con minutos en cero y contado como ausencia
}
```

- [ ] **Step 2: Correr y ver que falla**

Run: `cargo nextest run --all-features -E 'binary(reports_absent_employee_test)'`
Expected: falla — el reporte devuelve un solo empleado.

- [ ] **Step 3: Invertir el origen de la consulta**

Cambiar el `FROM` para que parta de empleados activos y los registros sean opcionales:

```sql
         FROM employees e
         JOIN departments d ON d.id = e.department_id
         LEFT JOIN daily_records dr
                ON dr.employee_id = e.id
               AND dr.anchor_date BETWEEN ?1 AND ?2
         LEFT JOIN daily_record_overrides dro
                ON dro.daily_record_id = dr.id AND dro.status = 'active'
         LEFT JOIN leaves l ON l.id = dr.leave_id AND l.status = 'active'
```

Consecuencias que hay que manejar en el acumulador, **no ignorar**:

- Todas las columnas `dr.*` pasan a ser nulables. Los `row.get::<i64>(...)` de `reports/service.rs:227-228` y alrededores empiezan a fallar en cuanto haya un empleado sin registros. Léelos con `Option<i64>` y trátalos como cero.
- `dr.anchor_date` nulo significa "sin registro ese día"; ese caso no puede entrar al brazo de día laboral con salario, porque produciría pago por un día no trabajado.
- El filtro por período pasa del `WHERE` al `ON` del `LEFT JOIN`. Si se queda en el `WHERE`, degrada el `LEFT JOIN` a `INNER JOIN` y el defecto vuelve **sin que ninguna prueba lo note**. Verifica el `where_clause` construido y muévelo si hace falta.

- [ ] **Step 4: Verificar que las cuentas de ausencia siguen bien**

Run: `cargo nextest run --all-features -E 'binary(reports_test)'`
Expected: verde. Este es el archivo de pruebas más grande del reporte; si algo se rompe aquí, la inversión del `FROM` cambió una semántica que alguien dependía.

- [ ] **Step 5: Verificar todo**

Run: `cargo nextest run --all-features && cargo clippy --all-targets --all-features -- -D warnings && make coverage-backend`

- [ ] **Step 6: Commit**

```bash
git add backend/src/reports/service.rs backend/tests/reports_absent_employee_test.rs
git commit -m "fix(reports): build the report universe from active employees (C-05)"
```

---

## Después de este plan

**Recalcular los períodos ya emitidos** y producir un informe de diferencias, sin sobrescribir la evidencia original. Si algún cliente ya recibió números con estos defectos, la corrección mueve importes en ambos sentidos — eso es una conversación comercial, no un deploy.

Queda **fuera de alcance** y necesita su propio plan:
- **H-07** — las anulaciones no recalculan las horas extra ni validan combinaciones imposibles. La Tarea 1 deja un comentario donde se nota.
- **H-08** — la unidad salarial es ambigua: la interfaz dice "Sueldo Base (USD)" y `money.rs` trata 480 min como un salario diario completo. Si alguien introduce un salario mensual, el resultado se multiplica por los días del período. **Este es probablemente el defecto monetario más grande que queda abierto** tras este plan.
- **C-10** — inbox durable de ingesta.
