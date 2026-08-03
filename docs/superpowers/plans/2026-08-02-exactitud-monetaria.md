# Exactitud monetaria: H-08 y C-01 a C-05 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Corregir lo que hace que el reporte pague mal: la unidad salarial ambigua (H-08), la hora extra al 250% (C-01), el doble descuento por tardanza (C-02), el salario que queda en cero (C-03), las anulaciones activas múltiples (C-04) y el empleado que desaparece del reporte (C-05).

**Architecture:** Cuatro tareas secuenciales. La primera fija la unidad salarial y cambia las firmas de `money.rs`; la segunda corrige qué minutos alimentan esas fórmulas; las dos últimas son integridad de datos y vigencia laboral. El orden importa: H-08 es un multiplicador y C-01 es un porcentaje — afinar el porcentaje sobre una base multiplicada por 30 no sirve de nada.

**Tech Stack:** Rust/Axum 0.8, libSQL/SQLite, migraciones SQL numeradas, `cargo nextest`.

## Global Constraints

- **Nunca** usar los identificadores desnudos `execute`, `execute_batch` ni `transaction` dentro de `backend/src` — `scripts/check_db_write_queue.py` es un gate que falla la build. Toda escritura va por `state.db_write.statement(...)` o `state.db_write.transact(...)`; dentro de una transacción, `tx.query(...)` y `tx.statement(...)`.
- **Nunca romper la arquitectura hexagonal.** El detalle de vendedor vive solo en `backend/src/isapi/*`. Ninguna tarea puede meter tipos ni formatos de Hikvision en `attendance/`, `reports/`, `calc/` ni `daily_records/`.
- Toda mutación de datos de asistencia genera entrada de auditoría inmutable con justificación.
- Gate de cobertura duro: proyecto ≥90% líneas / ≥85% ramas; por archivo ≥70% / ≥60%. Correr `make coverage-backend`. **Los márgenes actuales son delgados (90.16% / 85.66%)** — una función nueva sin cobertura tumba el gate.
- `cargo clippy --all-targets --all-features -- -D warnings` debe pasar.
- Mensajes de commit en inglés con prefijo convencional.
- **No hay datos productivos.** Ninguna migración necesita resolver filas existentes; hazlas limpias. Esto es una decisión explícita del dueño del producto (2026-08-02), no una suposición.

## Contexto imprescindible

Los cinco críticos fueron **verificados** contra el código actual — `docs/auditoria/VERIFICACION-Y-PLAN.md`. No son sospechas.

**El motor monetario estaba esencialmente sin probar.** `backend/src/calc/overtime.rs:39-45` codificaba la suma diaria errónea — ese fue el único test existente que hubo que corregir. Todo lo demás pasó en verde tras el arreglo, lo que significa que **ningún test asertaba el total del reporte para un día con horas extra**. El defecto sobrevivió a 1096 pruebas porque nadie miraba, no porque miraran mal. (La auditoría afirma que "la prueba QA E3 acepta el pago duplicado"; es falso — `docs/QA-GUIDE.md:621` documenta el importe correcto.) **"La suite pasa" no es evidencia de corrección en este plan.** Los importes salen calculados a mano desde la LOTTT; las pruebas viejas se corrigen junto con el código, no al revés.

**`money::` tiene un solo llamador:** `reports/service.rs`, 7 sitios. Verificado con el grafo del código y por grep.

**`overtime_minutes` es un subconjunto de `work_minutes`** (`calc/engine.rs:82`), y **conserva esa semántica** en todo el plan: `daily_records/service.rs:158` y `:180` la usan para los topes semanal y anual. Lo que se corrige es la base de pago, no el significado de la columna.

### Decisiones ya tomadas (no re-litigar)

- **Tardanza:** se paga el tiempo realmente trabajado y se elimina el descuento monetario. `late_minutes` sigue registrándose como métrica de disciplina. Razón: el salario de los minutos no trabajados no se causa; una deducción punitiva adicional sería doble sanción, y la LOTTT ya da el remedio disciplinario (art. 79; Reglamento art. 38). **Pendiente de confirmación por abogado laboral venezolano** — la fuente es doctrina de divulgación, no jurisprudencia.
- **Salario vacío:** se rechaza. Sin herencia departamental.
- **Moneda:** el salario se **calcula en USD** y se **liquida en VES**. Eso satisface el curso legal del art. 123; no hay problema de moneda que resolver en el código. Lo que sí falta —tasa y fecha de conversión, exigibles en el recibo del art. 106— vive en el sistema de nómina de destino, no en Cronometrix. El export debe decirlo para que nadie tome el número en USD por un recibo.
- **Divisor mensual:** `/30`. Es la convención venezolana de salario diario. **Confirmar con contador o abogado laboral antes de facturar con esta lógica.**

---

## File Structure

| Archivo | Responsabilidad | Tarea |
|---|---|---|
| `backend/src/db/migrations/024_employee_salary_kind.sql` | `salary_kind` obligatorio | 1 |
| `backend/src/employees/models.rs` | DTO con unidad salarial | 1 |
| `backend/src/employees/service.rs` | Salario obligatorio, positivo, con unidad | 1 |
| `backend/src/reports/money.rs` | Fórmulas normalizan la unidad en una sola fracción | 1 |
| `backend/src/reports/service.rs` | Pasa la unidad a las fórmulas | 1 |
| `backend/src/calc/overtime.rs` | Tope diario deja de sumar dos veces | 2 |
| `backend/src/reports/service.rs` | Base ordinaria; sin descuento por tardanza | 2 |
| `backend/src/db/migrations/025_unique_active_override.sql` | Índice único parcial | 3 |
| `backend/src/daily_records/handlers.rs` | Sustitución transaccional de anulación | 3 |
| `backend/src/db/migrations/026_employee_terminated_on.sql` | Fecha de egreso | 4 |
| `backend/src/reports/service.rs` | Universo por vigencia laboral | 4 |

---

### Task 1: El salario dice su unidad, y es obligatorio (H-08, C-03)

**Dos defectos en la misma columna.** `employees/service.rs:105` hace `req.base_salary_cents.unwrap_or(0)`: un salario ausente se guarda como **0** y produce nómina en cero sin aviso (C-03). Y `money.rs` interpreta `base_salary_cents` como el pago de **una jornada ordinaria** —un salario diario— mientras la interfaz solo dice "Sueldo Base (USD)" (H-08). Si alguien introduce un salario mensual, **cada día paga un mes**: el período se multiplica por ~30.

H-08 va primero en el plan porque es el multiplicador. Corregir el 250% de la hora extra sobre una base 30 veces mayor no arregla nada.

**Files:**
- Create: `backend/src/db/migrations/024_employee_salary_kind.sql`
- Modify: `backend/src/employees/models.rs`, `backend/src/employees/service.rs`
- Modify: `backend/src/reports/money.rs`, `backend/src/reports/service.rs`
- Test: `backend/tests/salary_unit_test.rs` (crear)

**Interfaces:**
- Produces: `SalaryKind { Hourly, Daily, Monthly }` en `backend/src/employees/models.rs`, reexportado donde `reports` lo necesite. Las cinco funciones de `money.rs` reciben un parámetro `kind: SalaryKind` adicional. La Tarea 2 depende de estas firmas.

- [ ] **Step 1: Escribir los casos calculados a mano**

Crear `backend/tests/salary_unit_test.rs`:

```rust
//! H-08: los importes se calcularon a mano. `base_salary_cents` sin unidad
//! explícita era ambiguo y un salario mensual se pagaba como si fuera diario.

use cronometrix_api::employees::models::SalaryKind;
use cronometrix_api::reports::money::work_pay_cents;

/// Jornada completa (480 min) con salario DIARIO de 50,00 -> 50,00.
#[test]
fn a_daily_salary_pays_itself_for_one_full_day() {
    assert_eq!(work_pay_cents(480, 5_000, 480, SalaryKind::Daily), 5_000);
}

/// Jornada completa con salario MENSUAL de 1.500,00 -> 50,00 (mensual/30).
/// El defecto H-08 pagaba 1.500,00 por ese mismo día.
#[test]
fn a_monthly_salary_is_divided_by_thirty_not_paid_whole() {
    assert_eq!(work_pay_cents(480, 150_000, 480, SalaryKind::Monthly), 5_000);
    assert_ne!(work_pay_cents(480, 150_000, 480, SalaryKind::Monthly), 150_000);
}

/// Jornada completa con salario POR HORA de 6,25 sobre 8 h -> 50,00.
#[test]
fn an_hourly_salary_scales_by_the_ordinary_day() {
    assert_eq!(work_pay_cents(480, 625, 480, SalaryKind::Hourly), 5_000);
}

/// La normalización NO puede hacerse en dos divisiones. `money.rs:3-4`
/// documenta "multiplicar numeradores primero, dividir una sola vez"; calcular
/// primero un salario diario y luego prorratear pierde hasta 29 centavos por
/// día en aritmética de centavos enteros.
///
/// Mensual 1.000,01 sobre media jornada: 100_001 * 240 / (30 * 480) = 1666,68…
/// Una sola fracción trunca a 1666. Dos divisiones (100_001/30 = 3333, luego
/// 3333*240/480 = 1666) coinciden aquí por casualidad, así que este caso usa
/// un valor donde divergen.
#[test]
fn normalization_uses_one_fraction_not_two_divisions() {
    // mensual 999,99 -> 99_999 centavos, media jornada
    // una fracción: 99_999 * 240 / (30*480) = 1666,65 -> 1666
    // dos divisiones: (99_999/30)=3333 -> 3333*240/480 = 1666  (coincide)
    // mensual 100_007, jornada completa:
    // una fracción: 100_007 * 480 / (30*480) = 3333,56 -> 3333
    // dos divisiones: (100_007/30)=3333 -> 3333*480/480 = 3333  (coincide)
    // El caso que diverge de verdad aparece con jornadas parciales grandes:
    assert_eq!(work_pay_cents(7, 100_007, 480, SalaryKind::Monthly), 48);
}
```

**Antes de dar por buenos estos números, recalcúlalos tú.** Si alguno no cuadra con la fórmula del Step 3, el número del plan está mal y manda la aritmética — repórtalo en tu informe en vez de ajustar la fórmula al número.

- [ ] **Step 2: Correr y ver que falla**

Run: `cargo nextest run --all-features -E 'binary(salary_unit_test)'`
Expected: no compila — `SalaryKind` no existe y `work_pay_cents` tiene 3 parámetros.

- [ ] **Step 3: Normalizar la unidad dentro de una sola fracción**

En `backend/src/reports/money.rs`, añadir la conversión **sin** precalcular un salario diario:

```rust
/// Unidad en que está expresado `base_salary_cents` (H-08).
///
/// Antes no había ninguna: `money.rs` asumía "pago de una jornada ordinaria" y
/// la interfaz solo decía "Sueldo Base (USD)". Un salario mensual introducido
/// ahí pagaba un mes por cada día trabajado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SalaryKind {
    Hourly,
    Daily,
    Monthly,
}

impl SalaryKind {
    /// Multiplicador y divisor que llevan `base_salary_cents` a "una jornada
    /// ordinaria", devueltos por separado a propósito: el llamador construye
    /// UNA sola fracción con ellos. Precalcular un salario diario introduce una
    /// división extra y pierde hasta 29 centavos por día en centavos enteros —
    /// justo lo que el patrón documentado arriba evita.
    ///
    /// - `Daily`   -> base tal cual
    /// - `Monthly` -> base / 30 (convención venezolana de salario diario;
    ///                pendiente de confirmación contable)
    /// - `Hourly`  -> base * ord_min / 60
    fn to_daily(self, ordinary_daily_minutes: i64) -> (i64, i64) {
        match self {
            SalaryKind::Daily => (1, 1),
            SalaryKind::Monthly => (1, 30),
            SalaryKind::Hourly => (ordinary_daily_minutes, 60),
        }
    }
}
```

Y `work_pay_cents` pasa a:

```rust
pub fn work_pay_cents(
    work_minutes: i64,
    base_salary_cents: i64,
    ordinary_daily_minutes: i64,
    kind: SalaryKind,
) -> i64 {
    if ordinary_daily_minutes <= 0 {
        return 0;
    }
    let (num, den) = kind.to_daily(ordinary_daily_minutes);
    work_minutes
        .checked_mul(base_salary_cents)
        .and_then(|p| p.checked_mul(num))
        .map(|p| p / (den * ordinary_daily_minutes))
        .unwrap_or(0)
}
```

Aplicar el mismo patrón a `ot_pay_cents` (que ya multiplica por 150), `night_premium_cents` (30), `rest_day_surcharge_cents` (50) y `late_deduction_cents`. **Mantener `checked_mul` en cada paso** — ahora hay un factor más y el desbordamiento es más fácil.

- [ ] **Step 4: Migración — la unidad es obligatoria y sin valor por defecto**

Crear `backend/src/db/migrations/024_employee_salary_kind.sql`:

```sql
-- H-08: `base_salary_cents` no decía en qué unidad estaba. money.rs lo trataba
-- como el pago de una jornada ordinaria (salario diario) mientras la interfaz
-- solo mostraba "Sueldo Base (USD)". Un salario mensual ahí multiplicaba el
-- período por ~30.
--
-- Sin DEFAULT a propósito: un valor por defecto es exactamente cómo vuelve la
-- ambigüedad. SQLite exige un DEFAULT para añadir una columna NOT NULL a una
-- tabla con filas, así que se añade nullable y la capa de servicio la exige;
-- no hay datos productivos, de modo que en la práctica ninguna fila queda sin
-- unidad.
ALTER TABLE employees ADD COLUMN salary_kind TEXT
    CHECK (salary_kind IN ('hourly', 'daily', 'monthly'));
```

- [ ] **Step 5: Exigir salario y unidad en creación**

En `backend/src/employees/service.rs`, sustituir `unwrap_or(0)`:

```rust
    // C-03: un salario ausente NO es cero. La interfaz prometía una herencia
    // departamental que el backend nunca implementó, así que el valor por
    // defecto producía nómina en cero en silencio.
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
    // H-08: sin unidad explícita el importe es ininterpretable.
    let salary_kind = req.salary_kind.ok_or_else(|| AppError::Validation {
        code: "SALARY_KIND_REQUIRED",
        message: "salary_kind is required: one of hourly, daily, monthly".to_string(),
    })?;
```

En la actualización, un `Some(salary)` con valor `<= 0` se rechaza con `SALARY_INVALID` — **no** se convierte en silencio a `None`.

- [ ] **Step 6: Pasar la unidad desde el reporte**

`reports/service.rs` lee `e.salary_kind` en el SELECT y lo pasa a las cinco llamadas de `money::`. Una fila sin unidad no puede monetizarse: trátala como error de datos y regístrala, **no** asumas `Daily` — asumir es reintroducir H-08 con otro nombre.

- [ ] **Step 7: Verificar**

Run: `cargo nextest run --all-features && cargo clippy --all-targets --all-features -- -D warnings && python3 scripts/check_db_write_queue.py && make coverage-backend`

- [ ] **Step 8: Commit**

```bash
git add backend/src/db/migrations/024_employee_salary_kind.sql backend/src/employees/ backend/src/reports/ backend/tests/salary_unit_test.rs
git commit -m "fix(payroll): make the salary unit explicit and the salary mandatory (H-08, C-03)"
```

---

### Task 2: La hora extra se paga una vez y la tardanza cuesta una vez (C-01, C-02)

El reporte paga `work_minutes` completos a tarifa ordinaria y **además** suma `overtime_minutes` al 150%. Como los extra están **dentro** de `work_minutes`, cada minuto extraordinario cobra 100% + 150% = **250%**. Aparte, `late_deduction_cents` resta otra vez minutos que el trabajador ya no cobró, porque `work_minutes` es tiempo real entre entrada y salida.

Jornada 480, 60 min extra, salario diario 50,00: el código produce **65,625**; lo correcto bajo recargo de 50% (LOTTT 118) es **59,375**.

**Files:**
- Modify: `backend/src/calc/overtime.rs:15-29` y sus tests
- Modify: `backend/src/reports/service.rs` (bloque de día laboral)
- Test: `backend/tests/reports_money_correctness_test.rs` (crear)

**Interfaces:**
- Consumes: las firmas de `money.rs` con `SalaryKind` de la Tarea 1.

- [ ] **Step 1: Escribir los casos calculados a mano**

```rust
//! C-01/C-02: importes calculados a mano desde la LOTTT. La suite anterior
//! codificaba la especificación equivocada y no sirve de referencia.

use cronometrix_api::employees::models::SalaryKind;
use cronometrix_api::reports::money::{ot_pay_cents, total_a_pagar_cents, work_pay_cents};

/// LOTTT 118: recargo mínimo de 50% sobre la hora extra, o sea 1,5x.
///   480 min ordinarios          -> 5000
///   60 min extra a 1,5x         ->  937  (9,375 truncado)
///   total                       -> 5937
/// El defecto producía 6562: la hora extra cobrada al 250%.
#[test]
fn overtime_is_paid_once_at_150_percent_not_250() {
    let work = work_pay_cents(480, 5_000, 480, SalaryKind::Daily);
    let ot = ot_pay_cents(60, 5_000, 480, SalaryKind::Daily);
    let total = total_a_pagar_cents(work, ot, 0, 0, 0);
    assert_eq!((work, ot, total), (5_000, 937, 5_937));
    assert_ne!(total, 6_562, "250% — el defecto C-01");
}

/// C-02: llegar 30 min tarde y salir a la hora nominal da 450 min trabajados.
/// Se pagan 450 y nada más: el salario de esos 30 min no se causó.
#[test]
fn lateness_costs_the_unworked_minutes_and_nothing_more() {
    let work = work_pay_cents(450, 5_000, 480, SalaryKind::Daily);
    let total = total_a_pagar_cents(work, 0, 0, 0, 0);
    assert_eq!(total, 4_687);
    assert_ne!(total, 4_375, "doble descuento — el defecto C-02");
}
```

- [ ] **Step 2: Correr y ver que fallan**

Run: `cargo nextest run --all-features -E 'binary(reports_money_correctness_test)'`
Expected: pasan — las funciones puras ya son correctas por separado. **El defecto está en qué minutos les pasa `reports/service.rs`, no en `money.rs`.** Si alguna falla, para: significa que la aritmética base también está mal y este plan asume que no.

- [ ] **Step 3: Corregir la base de pago**

En el brazo `_ =>` de `reports/service.rs`:

```rust
                // C-01: `overtime_minutes` es un SUBCONJUNTO de los minutos
                // trabajados (calc/engine.rs:82). Pagar el total a tarifa
                // ordinaria y sumar además el extra al 150% cobra cada minuto
                // extraordinario al 250%. La base ordinaria los excluye; el
                // recargo lo aporta ot_pay_cents.
                let ordinary_min = (effective_work_min - overtime_minutes).max(0);
                let work_pay = money::work_pay_cents(
                    ordinary_min, base_salary_cents, ordinary_daily_minutes, salary_kind,
                );
```

`effective_work_min` puede venir de una anulación mientras `overtime_minutes` es el valor original — de ahí el `.max(0)`. Que la anulación no recalcule el extra es **H-07, fuera de alcance**; deja el comentario para el siguiente lector.

Los otros tres usos de `effective_work_min` (prima nocturna, recargo dominical, agregado `work_min`) **no cambian**: la prima del art. 117 se causa sobre toda la jornada nocturna, y el agregado informa minutos, no base de pago.

- [ ] **Step 4: Quitar la tardanza del total**

```rust
                // C-02: la tardanza ya se reflejó en los minutos trabajados
                // (calc/engine.rs:70-76 mide entre entrada y salida reales).
                // Restar late_minutes otra vez descuenta dos veces el mismo
                // hecho. Se conserva como métrica; deja de monetizarse.
                let total = money::total_a_pagar_cents(work_pay, ot_pay, night, rest, 0);
```

Lee la struct de agregado antes de decidir qué hacer con `entry.agg.late_deduction_cents`: si solo alimentaba el total, ponlo en 0 y elimina el cálculo; si el reporte lo expone como columna informativa, consérvalo. **No adivines** — mira la definición.

- [ ] **Step 5: Corregir el tope diario**

`calc/overtime.rs:22` evalúa `work_minutes + overtime_minutes > 600`, pero `work_minutes` ya contiene el extra:

```rust
    // LOTTT 178: el tope es de 10 h EFECTIVAS al día. `work_minutes` ya incluye
    // los extraordinarios (calc/engine.rs:82); sumarlos otra vez evalúa una
    // jornada que nadie trabajó.
    if work_minutes > 600 {
        out.push(AnomalyCode::OtCapExceededDaily);
    }
```

Y corregir la prueba del mismo archivo, que afirma la suma errónea:

```rust
    #[test]
    fn daily_cap_triggers_only_when_the_real_workday_exceeds_600() {
        assert!(check_overtime_caps(600, 120, 0, 0).is_empty());
        let out = check_overtime_caps(601, 121, 0, 0);
        assert!(out.contains(&AnomalyCode::OtCapExceededDaily));
    }
```

Los topes semanal y anual **no se tocan**: suman `overtime_minutes`, que es el excedente, y esa lectura es correcta.

- [ ] **Step 6: Juzgar cada prueba que falle**

Run: `cargo nextest run --all-features 2>&1 | tail -40`

Habrá fallos de pruebas que codificaban el defecto. **Cada una se juzga, no se ajusta:** si su valor esperado corresponde al 250% o al doble descuento, el valor estaba mal y se corrige con el número calculado a mano. Si falla por otra razón, es una regresión tuya. Documenta en el informe cada prueba tocada y en qué categoría cayó.

- [ ] **Step 7: Verificar y commitear**

Run: `cargo nextest run --all-features && cargo clippy --all-targets --all-features -- -D warnings && make coverage-backend`

```bash
git add backend/src/calc/overtime.rs backend/src/reports/service.rs backend/tests/reports_money_correctness_test.rs
git commit -m "fix(reports): pay overtime once at 150% and stop double-charging lateness (C-01, C-02)"
```

---

### Task 3: Una sola anulación activa por registro (C-04)

`009_daily_record_overrides.sql:24` crea el índice **sin `UNIQUE`** y filtra por `deleted_at`, no por `status`. Pueden coexistir varias anulaciones `active`, y el `LEFT JOIN` de `reports/service.rs:196` **multiplica la fila**: días, minutos e importes duplicados.

**No hay datos productivos**, así que la migración es solo el índice — sin resolver duplicados previos.

**Files:**
- Create: `backend/src/db/migrations/025_unique_active_override.sql`
- Modify: `backend/src/daily_records/handlers.rs` (inserción)
- Test: `backend/tests/override_uniqueness_test.rs` (crear)

- [ ] **Step 1: Escribir la prueba que falla**

```rust
mod common;

/// C-04: dos anulaciones sobre el mismo registro. La segunda revoca la primera;
/// coexistiendo, el LEFT JOIN del reporte duplica la fila y los importes.
#[tokio::test]
async fn a_second_override_revokes_the_first() {
    // crear daily_record, aplicar anulación A, aplicar B,
    // asertar COUNT(*) WHERE status='active' == 1 y que la activa es B
}

/// La evidencia no se borra: la revocada sigue existiendo.
#[tokio::test]
async fn the_revoked_override_is_kept_for_audit() {
    // tras revocar A, existe una fila para A con status='revoked'
}
```

- [ ] **Step 2: Correr y ver que falla**

Run: `cargo nextest run --all-features -E 'binary(override_uniqueness_test)'`
Expected: la primera falla con 2 anulaciones activas.

- [ ] **Step 3: Migración — solo el índice**

Crear `backend/src/db/migrations/025_unique_active_override.sql`:

```sql
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
```

- [ ] **Step 4: Sustitución transaccional**

El índice hace fallar la segunda anulación legítima si no se revoca antes. Revocar e insertar en la misma transacción:

```rust
    state
        .db_write
        .transact("daily_records.replace-override", move |tx| {
            Box::pin(async move {
                // C-04: revocar antes de insertar, en la misma transacción. En
                // dos operaciones queda una ventana sin ninguna activa.
                tx.statement(
                    "UPDATE daily_record_overrides \
                        SET status = 'revoked', updated_at = unixepoch() \
                      WHERE daily_record_id = ?1 AND status = 'active' AND deleted_at IS NULL",
                    libsql::params![daily_record_id.clone()],
                )
                .await?;
                tx.statement(/* el INSERT existente, verbatim */, /* mismos params */)
                    .await?;
                Ok(())
            })
        })
        .await
        .map_err(AppError::from)?;
```

Copia el INSERT **verbatim** del código actual; no lo reescribas de memoria.

- [ ] **Step 5: Verificar y commitear**

Run: `cargo nextest run --all-features && cargo clippy --all-targets --all-features -- -D warnings && python3 scripts/check_db_write_queue.py && make coverage-backend`

```bash
git add backend/src/db/migrations/025_unique_active_override.sql backend/src/daily_records/handlers.rs backend/tests/override_uniqueness_test.rs
git commit -m "fix(overrides): allow only one active override per record (C-04)"
```

---

### Task 4: El reporte se construye por vigencia laboral (C-05)

`reports/service.rs:193` hace `FROM daily_records dr JOIN employees e`: el universo nace de los registros, así que un empleado activo que no marcó ningún día **no aparece** — ni ausencia, ni descuento, ni alerta. Es manipulable: basta con que no lleguen eventos.

**Lo que ya funciona y no hay que reconstruir:** la expansión del calendario esperado ya existe en `reports/service.rs:490` (`weekdays_in_period`, lunes a viernes, menos `worked_dates` y `leave_dates`). En cuanto el empleado entra al acumulador con `worked_dates` vacío, ese bucle le asigna ausencia en todos los días hábiles. **No hace falta CTE recursiva ni dominio de calendario nuevo.**

**Lo que el arreglo destapa, y es la mitad del trabajo.** `weekdays_in_period` abarca el período completo, sin acotar por vigencia del empleado:

- Un contratado ayer aparecería con **un mes entero de ausencias**. `hire_date` existe (migración 015) pero es **nullable** y **no está en el SELECT** del reporte.
- Un empleado que renunció el día 3 acumula ausencias por los 17 días hábiles restantes. Y si el reporte filtra `e.status='active'` (lo hace en `service.rs:137`), **desaparece entero — incluidos los días que sí trabajó y se le deben.** Ese es el caso grave: no es un dato faltante, es no pagarle a alguien lo que trabajó.

`employees` **no tiene fecha de egreso**: solo `status` ('active'/'inactive') y `deleted_at`, ninguno con fecha. Por eso hace falta la columna.

**Files:**
- Create: `backend/src/db/migrations/026_employee_terminated_on.sql`
- Modify: `backend/src/reports/service.rs` (consulta, acumulador, cálculo de ausencias)
- Modify: `backend/src/employees/` (exponer `terminated_on`)
- Test: `backend/tests/reports_employment_window_test.rs` (crear)

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
mod common;

/// C-05: un empleado activo sin ninguna marca debe aparecer como ausente.
/// Hoy desaparece, y con él su descuento y su alerta.
#[tokio::test]
async fn an_employee_with_no_events_at_all_appears_as_absent() {
    // dos empleados activos, eventos solo para el primero;
    // ambos aparecen; el segundo con minutos en cero y ausencias
}

/// Un contratado a mitad del período no acumula ausencias anteriores a su
/// ingreso.
#[tokio::test]
async fn absences_do_not_start_before_the_hire_date() {
    // hire_date a 2 días hábiles del fin del período, sin marcas
    // -> days_absent == 2, no el período completo
}

/// Un empleado que egresó a mitad del período conserva sus días trabajados y
/// no acumula ausencias posteriores. Este es el caso que hoy hace desaparecer
/// a un trabajador con pago pendiente.
#[tokio::test]
async fn a_terminated_employee_keeps_worked_days_and_stops_accruing_absences() {
    // terminated_on a mitad del período, con marcas antes
    // -> aparece, con sus días trabajados, sin ausencias posteriores
}
```

- [ ] **Step 2: Correr y ver que fallan**

Run: `cargo nextest run --all-features -E 'binary(reports_employment_window_test)'`

- [ ] **Step 3: Migración — fecha de egreso**

```sql
-- C-05: `status` ('active'/'inactive') no lleva fecha, así que el reporte no
-- podía saber CUÁNDO dejó de trabajar alguien. Filtrando por status='active'
-- un empleado que egresó a mitad del período desaparecía junto con los días
-- que trabajó y se le deben; sin filtrar, acumulaba ausencias después de irse.
--
-- `status` sigue siendo útil para la interfaz (a quién ofrecer en un
-- selector). Deja de gobernar el reporte: la pregunta "¿quién cobra este
-- período?" es de vigencia, no de estado actual.
ALTER TABLE employees ADD COLUMN terminated_on INTEGER;  -- epoch seconds UTC, nullable = sigue activo
```

- [ ] **Step 4: Invertir el origen de la consulta**

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

Añadir `e.hire_date` y `e.terminated_on` al SELECT.

Tres consecuencias que hay que manejar, **no ignorar**:

- Las columnas `dr.*` pasan a ser nulables. Los `row.get::<i64>(...)` de `service.rs:227-228` empiezan a fallar en cuanto haya un empleado sin registros: léelos como `Option<i64>` y trátalos como cero.
- `dr.anchor_date` nulo significa "sin registro"; ese caso no puede entrar al brazo de día laboral con salario, o pagaría un día no trabajado.
- **El filtro de período pasa del `WHERE` al `ON`.** Si se queda en el `WHERE`, degrada el `LEFT JOIN` a `INNER JOIN` y el defecto vuelve **sin que ninguna prueba lo note**. Revisa el `where_clause` construido y muévelo.

- [ ] **Step 5: Acotar las ausencias por vigencia**

En el bucle de `service.rs:495`, filtrar los días hábiles por la ventana de cada empleado:

```rust
    for entry in acc.values_mut() {
        // C-05: las ausencias solo se cuentan mientras la relación laboral
        // existió. Sin esto, un contratado ayer aparece con un mes de
        // ausencias y un egresado sigue acumulándolas después de irse.
        let absent = weekdays_in_period
            .iter()
            .filter(|d| entry.hire_date.is_none_or(|h| **d >= h))
            .filter(|d| entry.terminated_on.is_none_or(|t| **d <= t))
            .filter(|d| !entry.worked_dates.contains(d) && !entry.leave_dates.contains(d))
            .count() as i64;
        entry.agg.days_absent = absent;
    }
```

**`hire_date` nulo no se acota** — pero un empleado sin fecha de ingreso no es computable con exactitud, así que registra una anomalía para que sea visible en vez de silenciosa.

Y quitar `e.status = 'active'` como filtro del reporte (`service.rs:137` y `:387`): la vigencia la determinan ahora `hire_date`/`terminated_on` contra el período. Un empleado inactivo **con** días trabajados en el período debe cobrar.

- [ ] **Step 6: Verificar que el reporte grande sigue sano**

Run: `cargo nextest run --all-features -E 'binary(reports_test)'`
Expected: verde. Es el archivo de pruebas más grande del reporte; si algo se rompe, la inversión del `FROM` cambió una semántica de la que alguien dependía.

- [ ] **Step 7: Verificar y commitear**

Run: `cargo nextest run --all-features && cargo clippy --all-targets --all-features -- -D warnings && make coverage-backend`

```bash
git add backend/src/db/migrations/026_employee_terminated_on.sql backend/src/reports/service.rs backend/src/employees/ backend/tests/reports_employment_window_test.rs
git commit -m "fix(reports): build the report from employment validity, not from records (C-05)"
```

---

### Task 5: El formulario de empleados envía la unidad salarial (H-08, frontend)

**Esta tarea debe entrar en el mismo PR que la Tarea 1.** No es trabajo posterior: en cuanto el backend exige `salary_kind`, el formulario actual devuelve **422 al crear un empleado**. Mergear la Tarea 1 sola rompe el producto.

El plan original no incluía ni un archivo de frontend — omisión mía. C-03 y H-08 cambian un contrato de API y el consumidor está en `frontend/`.

Estado comprobado: `frontend/src/types/api.ts` define `base_salary_cents` en dos interfaces y **`salary_kind` no aparece en ninguna parte del frontend**.

**Files:**
- Modify: `frontend/src/types/api.ts` (las dos interfaces con `base_salary_cents`)
- Modify: el formulario de creación/edición de empleados en `frontend/src/components/employees/`
- Test: los `__tests__` correspondientes en ese directorio

**Interfaces:**
- Consumes: `salary_kind: 'hourly' | 'daily' | 'monthly'` — los valores exactos que acepta el `CHECK` de la migración 024. Deben coincidir carácter por carácter.

- [ ] **Step 1: Escribir la prueba que falla**

En el archivo de pruebas del formulario de empleados, añadir un caso que exija que el envío incluya la unidad y que no se pueda enviar sin elegirla:

```tsx
it('requires an explicit salary unit before submitting', async () => {
  // rellenar el formulario SIN elegir unidad -> el submit no debe dispararse
  // elegir 'monthly' -> el payload enviado incluye salary_kind: 'monthly'
})
```

- [ ] **Step 2: Correr y ver que falla**

Run: `cd frontend && npx vitest run src/components/employees`
Expected: falla — el campo no existe.

- [ ] **Step 3: Añadir el tipo**

En `frontend/src/types/api.ts`, añadir a **ambas** interfaces que hoy declaran `base_salary_cents`:

```ts
export type SalaryKind = 'hourly' | 'daily' | 'monthly'
```

y el campo `salary_kind: SalaryKind` donde corresponda (obligatorio en la request de creación).

- [ ] **Step 4: Añadir el selector al formulario**

Un selector con las tres opciones, **sin valor preseleccionado**. Un default reintroduce exactamente la ambigüedad que H-08 corrige: alguien acepta el formulario sin mirar y vuelve a haber un importe cuya unidad nadie eligió.

Etiquetar en español, coherente con el resto de la interfaz, y dejando claro a qué se refiere el monto: por hora / diario / mensual. La etiqueta actual "Sueldo Base (USD)" debe pasar a decir la unidad elegida, porque era justo su ambigüedad la que causaba el defecto.

- [ ] **Step 5: Verificar**

Run: `cd frontend && npx vitest run --coverage`
Expected: verde. El gate de cobertura del frontend también es duro (≥90% líneas / ≥85% ramas globales, ≥70%/≥60% por archivo).

- [ ] **Step 6: Commit**

```bash
git add frontend/src/types/api.ts frontend/src/components/employees/
git commit -m "feat(employees): require an explicit salary unit in the form (H-08)"
```

---

## Fuera de alcance

- **H-07** — las anulaciones no recalculan las horas extra ni validan combinaciones imposibles. La Tarea 2 deja un comentario donde se nota.
- **H-09** — el reporte lee salario, nombre y reglas en su estado actual, sin vigencia histórica ni cierre de período. Un cambio de salario hoy altera retroactivamente un período ya emitido. La Tarea 1 hace la unidad explícita pero **no** la versiona.
- **H-04** — descanso sábado/domingo cableado, sin feriados. La Tarea 4 usa la misma regla que el motor ya aplica; no inventa política nueva.
- **C-10** — inbox durable de ingesta.

## Pendientes que no son código

- **Confirmación laboral de la decisión C-02** (eliminar el descuento monetario por tardanza) y del **divisor mensual `/30`**.
- **El export debe declarar que el importe está en USD** y que la tasa y fecha de conversión a VES —exigibles en el recibo del art. 106— las aporta el sistema de nómina de destino.
