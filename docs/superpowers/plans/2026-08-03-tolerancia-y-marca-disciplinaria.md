# Tolerancia real y marca disciplinaria — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que la tolerancia configurada afecte realmente al retraso (H-01), y sobre esa base construir una marca de retardos reiterados que sirva como evidencia de causal de despido justificado — sin monetizar la tardanza.

**Architecture:** Dos tareas, y el orden no es negociable. La primera corrige el cálculo del retraso; la segunda cuenta retardos para efectos disciplinarios. Construir la segunda sobre el cálculo actual produciría causales de despido falsas.

**Tech Stack:** Rust/Axum 0.8, libSQL/SQLite, `cargo nextest`.

## Global Constraints

- **Nunca** usar los identificadores desnudos `execute`, `execute_batch` ni `transaction` dentro de `backend/src` — `scripts/check_db_write_queue.py` es un gate que falla la build.
- **Nunca romper la arquitectura hexagonal.** Nada de Hikvision en `calc/`, `reports/`, `attendance/`, `daily_records/`.
- Toda mutación de datos de asistencia genera entrada de auditoría inmutable con justificación.
- Gate de cobertura backend: proyecto ≥90% líneas / ≥85% ramas; por archivo ≥70% / ≥60%.
- `cargo clippy --all-targets --all-features -- -D warnings` debe pasar.
- Mensajes de commit en inglés con prefijo convencional.
- **No hay datos productivos.** Migraciones limpias.
- **Número de migración:** el plan del inbox durable (`2026-08-03-inbox-durable-y-gates.md`) reserva la **027**. Corre `ls backend/src/db/migrations/` antes de crear la tuya y usa la siguiente libre.
- El proyecto fija **Node 24.15.0**. Con Node 22 fallan cuatro tests de `export-buttons*` por entorno, no por código.

## Contexto imprescindible

### Por qué el orden importa más de lo habitual

Esta es la única función del producto que puede **terminar una relación laboral**. El listón de exactitud es más alto que en un reporte: un importe equivocado se devuelve, una causal de despido falsa no.

Y hoy el cálculo del retraso está mal de una forma que envenena exactamente este uso.

### H-01, verificado

`backend/src/calc/engine.rs:76` calcula:

```rust
let late = (((ent - nominal_start).max(0)) / 60).max(0);
```

Mide desde la hora nominal y **no consulta la tolerancia**. La tolerancia solo ensancha la ventana de captura de eventos (`calc/overnight.rs:92-95`), que es otra cosa.

Consecuencia: con `late_arrival_tolerance_min = 10`, alguien que entra a las 08:01 obtiene `late_minutes = 1`. Contarlo como retardo, cuatro veces en un mes, produciría una causal de despido **en contra del criterio que la propia empresa configuró**.

### La regla correcta es un acantilado, no una resta

`docs/QA-GUIDE.md:425-436` la especifica con ocho casos y es contraintuitiva — dentro de la gracia el retraso es cero, y **pasada la gracia el retraso es completo desde la hora nominal**, no el exceso sobre la gracia:

| # | Setup | Marcación | `late_min` esperado |
|---|---|---|---|
| R1.1 | turno 08:00, tol=10, bono=0 | 08:05 | **0** |
| R1.2 | turno 08:00, tol=10, bono=0 | 08:11 | **11** ← no 1 |
| R1.3 | turno 08:00, tol=10, bono=5 | 08:14 | **0** |
| R1.4 | turno 08:00, tol=10, bono=5 | 08:16 | **16** ← no 1 |
| R1.5 | turno 08:00, tol=10 | 07:55 | **0** |
| R1.6 | salida 17:00, tol=10, bono=0 | 16:55 | early **0** |
| R1.7 | salida 17:00, tol=10, bono=0 | 16:45 | early **15** |
| R1.8 | salida 17:00, tol=10, bono=5 | 16:46 | early **0** |

El bono es gracia **adicional** a la tolerancia, no la reemplaza. Umbral = `shift_start + late_tolerance + bonus`.

### Decisión de producto ya tomada (no re-litigar)

La tardanza **no se monetiza**. Se paga el tiempo realmente trabajado y no hay descuento adicional; eso ya está implementado. Esta marca es la alternativa disciplinaria, no un paso hacia el descuento.

---

## File Structure

| Archivo | Responsabilidad | Tarea |
|---|---|---|
| `backend/src/calc/engine.rs` | Tolerancia aplicada al retraso y a la salida temprana | 1 |
| `backend/tests/tolerance_cliff_test.rs` | Los ocho casos de la QA-GUIDE | 1 |
| `backend/src/db/migrations/0NN_late_streak_threshold.sql` | Umbral configurable | 2 |
| `backend/src/rules/models.rs` | Umbral en el DTO de reglas | 2 |
| `backend/src/daily_records/service.rs` | Conteo mensual de retardos computables | 2 |
| `backend/src/calc/anomalies.rs` | Código de anomalía nuevo | 2 |
| `backend/tests/late_streak_test.rs` | Conteo, umbral y reconstrucción | 2 |

---

### Task 1: La tolerancia configurada afecta al retraso (H-01)

**Files:**
- Modify: `backend/src/calc/engine.rs` (~línea 76)
- Test: `backend/tests/tolerance_cliff_test.rs` (crear)

**Interfaces:**
- Consumes: `input.rules.late_arrival_tolerance_min`, `input.rules.bonus_minutes`, `input.rules.early_departure_tolerance_min` — ya presentes en `EngineInput` (`calc/models.rs:34-35`), hoy sin usar para esto.
- Produces: `late_minutes` y `early_departure_minutes` con semántica de acantilado. La Tarea 2 depende de ello.

- [ ] **Step 1: Escribir los ocho casos de la QA-GUIDE**

Crear `backend/tests/tolerance_cliff_test.rs` con los ocho casos de la tabla de arriba, tomados **verbatim** de `docs/QA-GUIDE.md:425-436`. No los reinterpretes: R1.2 espera **11** y R1.4 espera **16**, no 1. Si te parecen raros, es porque la regla es un acantilado y esa es exactamente la parte que hay que fijar con pruebas.

Incluye explícitamente **los dos lados del límite** en un caso propio: con tol=10 bono=0, la entrada a 08:10 debe dar 0 y la de 08:11 debe dar 11. El minuto límite es donde este tipo de regla se implementa mal.

- [ ] **Step 2: Correr y ver cuáles fallan**

Run: `cargo nextest run --all-features -E 'binary(tolerance_cliff_test)'`
Expected: fallan R1.1, R1.3, R1.6 y R1.8 (los que esperan 0 y hoy devuelven el retraso crudo). R1.2, R1.4, R1.5 y R1.7 probablemente ya pasan. Reporta cuáles fallaron de verdad — si el reparto no es ese, el diagnóstico de este plan está incompleto y quiero saberlo antes de que sigas.

- [ ] **Step 3: Aplicar el acantilado**

En `backend/src/calc/engine.rs`, sustituir el cálculo de `late` y `early`:

```rust
            // H-01: la tolerancia configurada no afectaba al retraso — solo
            // ensanchaba la ventana de captura (calc/overnight.rs:92-95). Con
            // tolerancia de 10 min, entrar a las 08:01 producía late = 1.
            //
            // La regla es un ACANTILADO, no una resta (QA-GUIDE §21.2, casos
            // R1.1-R1.8): dentro de la gracia el retraso es cero; pasada la
            // gracia es el retraso COMPLETO desde la hora nominal, no el
            // exceso sobre la gracia. Con tol=10 y bono=5, entrar a las 08:16
            // son 16 minutos de retraso, no 1.
            //
            // El bono es gracia adicional a la tolerancia, no la reemplaza.
            let late_grace_s = (input.rules.late_arrival_tolerance_min
                + input.rules.bonus_minutes)
                * 60;
            let raw_late_s = (ent - nominal_start).max(0);
            let late = if raw_late_s > late_grace_s {
                (raw_late_s / 60).max(0)
            } else {
                0
            };

            let early_grace_s = (input.rules.early_departure_tolerance_min
                + input.rules.bonus_minutes)
                * 60;
            let raw_early_s = (nominal_end - exi).max(0);
            let early = if raw_early_s > early_grace_s {
                (raw_early_s / 60).max(0)
            } else {
                0
            };
```

**Comprueba el operador de comparación contra R1.1.** Con tol=10 y entrada a 08:05, `raw_late_s` son 300 s y la gracia 600 s: `300 > 600` es falso, luego 0. Correcto. Verifica también el límite exacto — 08:10 son 600 s, y `600 > 600` es falso, luego 0. Si usaras `>=`, el minuto 10 daría retraso y contradiría R1.1.

Arregla la salida temprana en el mismo paso: es el mismo defecto y dejar una mitad rota sería raro.

- [ ] **Step 4: Correr y ver que pasan los ocho**

Run: `cargo nextest run --all-features -E 'binary(tolerance_cliff_test)'`

- [ ] **Step 5: Juzgar el resto de la suite**

Run: `cargo nextest run --all-features 2>&1 | tail -40`

`late_minutes` alimenta la columna `Min Retraso` del reporte y posiblemente anomalías. Habrá tests que asumían el valor crudo. **Cada fallo se juzga:** si su valor esperado ignoraba una tolerancia configurada, estaba mal y se corrige contra la QA-GUIDE. Si falla por otra razón, es una regresión tuya. Documenta cada test tocado y en qué categoría cayó.

- [ ] **Step 6: Verificar y commitear**

Run: `cargo nextest run --all-features && cargo clippy --all-targets --all-features -- -D warnings && python3 scripts/check_db_write_queue.py && make coverage-backend`

```bash
git add backend/src/calc/engine.rs backend/tests/tolerance_cliff_test.rs
git commit -m "fix(calc): apply the configured tolerance to lateness and early departure (H-01)"
```

---

### Task 2: Marca de retardos reiterados

Con el retraso ya correcto, contar retardos **computables** —los que superan la gracia— por mes calendario, y marcar cuando alcanzan el umbral.

Base legal invocada: Reglamento de la LOTTT, artículo 38, que tipifica cuatro retardos en un mes como incumplimiento reiterado del horario de trabajo. **Pendiente de confirmación profesional**, igual que las otras decisiones laborales del producto.

**Files:**
- Create: `backend/src/db/migrations/0NN_late_streak_threshold.sql` (siguiente número libre)
- Modify: `backend/src/rules/models.rs`, `backend/src/rules/handlers.rs`
- Modify: `backend/src/daily_records/service.rs`, `backend/src/calc/anomalies.rs`
- Test: `backend/tests/late_streak_test.rs` (crear)

**Interfaces:**
- Consumes: `late_minutes` con semántica de acantilado, de la Tarea 1.
- Produces: `AnomalyCode::LateStreakThreshold`; campo `late_streak_threshold_per_month` en `global_rules` (default 4).

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
mod common;

/// Un retardo dentro de la tolerancia NO cuenta. Este es el test que impide
/// fabricar una causal de despido contra el criterio del propio patrono.
#[tokio::test]
async fn arrivals_within_tolerance_never_count_toward_the_streak() {
    // tol=10; cuatro entradas a 08:05 en un mes -> conteo 0, sin marca
}

/// Cuatro retardos computables en un mes calendario alcanzan el umbral.
#[tokio::test]
async fn four_countable_late_arrivals_in_one_month_raise_the_mark() {
    // tol=10; cuatro entradas a 08:20 -> conteo 4, marca presente
}

/// El mes es calendario, no ventana movil: dos en enero y dos en febrero no
/// alcanzan el umbral en ninguno de los dos.
#[tokio::test]
async fn the_window_is_a_calendar_month_not_a_rolling_one() { }

/// El umbral es configurable, no cableado: un convenio colectivo puede fijar
/// otro numero.
#[tokio::test]
async fn the_threshold_comes_from_global_rules() {
    // umbral=3 -> tres retardos ya marcan
}

/// La marca es reconstruible: se puede recuperar QUE dias y a que hora la
/// originaron. Si se usa para despedir, alguien va a preguntarlo.
#[tokio::test]
async fn the_mark_can_be_traced_back_to_the_exact_days() { }
```

- [ ] **Step 2: Correr y ver que fallan**

Run: `cargo nextest run --all-features -E 'binary(late_streak_test)'`

- [ ] **Step 3: El umbral configurable**

Migración (usa el siguiente número libre; **no** asumas 027, que está reservado):

```sql
-- Marca de retardos reiterados. El umbral sale del Reglamento de la LOTTT
-- art. 38 (cuatro retardos en un mes como incumplimiento reiterado del
-- horario), pero es configurable a proposito: un convenio colectivo o un
-- reglamento interno puede fijar otro numero, y cablearlo impondria nuestra
-- lectura a todos los clientes.
ALTER TABLE global_rules
    ADD COLUMN late_streak_threshold_per_month INTEGER NOT NULL DEFAULT 4;
```

Exponerlo en el DTO de reglas y en el endpoint, como los demás parámetros.

- [ ] **Step 4: Contar y marcar**

Un retardo **computable** es un día con `late_minutes > 0` **después** de la Tarea 1 — es decir, que superó tolerancia más bono. Contar por empleado y mes calendario.

Al alcanzar el umbral, añadir `AnomalyCode::LateStreakThreshold` a las anomalías del día que lo alcanza. Sigue el patrón de los topes de horas extra en `calc/overtime.rs`, que ya hacen algo equivalente con acumulados semanales y anuales.

Tres restricciones de diseño, y son la razón de ser de la tarea:

1. **Nunca actúa, solo expone.** La marca es evidencia para una decisión humana. No dispara nada, no bloquea nada, no notifica a nadie automáticamente.
2. **Reconstruible.** Debe poder responderse "¿qué días y a qué hora?" a partir de los datos, no solo mostrar un 4. Los `daily_records` ya guardan `late_minutes` por día — asegúrate de que la consulta que sustenta la marca sea reproducible y esté documentada.
3. **No retroactiva por cambio de reglas.** Bajar el umbral de 4 a 3 no debe fabricar marcas sobre meses ya cerrados. Si no puedes garantizarlo con la infraestructura actual —`global_rules.effective_from` existe pero H-09 señala que no selecciona versión histórica— **dilo en el informe en vez de fingir que sí**, y deja el límite documentado.

- [ ] **Step 5: Verificar y commitear**

Run: `cargo nextest run --all-features && cargo clippy --all-targets --all-features -- -D warnings && python3 scripts/check_db_write_queue.py && make coverage-backend`

```bash
git add backend/src/db/migrations/ backend/src/rules/ backend/src/daily_records/ backend/src/calc/anomalies.rs backend/tests/late_streak_test.rs
git commit -m "feat(attendance): flag repeated countable late arrivals per calendar month"
```

---

## Añadir a la consulta laboral

`docs/legal/CONSULTA-LABORAL.md` gana dos preguntas, en la sección de tardanza:

1. ¿Sigue vigente el umbral de cuatro retardos en un mes del artículo 38 del Reglamento como incumplimiento reiterado del horario?
2. ¿Puede un sistema computar retardos con ese fin sin un procedimiento de descargo previo, o el conteo solo es válido tras notificar cada retardo al trabajador?

La segunda importa más de lo que parece: si la respuesta exige notificación previa por cada retardo, la marca tal como está diseñada no sirve como evidencia y habría que añadir el flujo de notificación.

## Fuera de alcance

- **Monetizar la tardanza.** Decisión tomada: no se descuenta. Esta marca es la alternativa disciplinaria.
- **H-09** — vigencia histórica de reglas. Limita la garantía de no-retroactividad del umbral; documentado, no resuelto aquí.
- **Notificación al trabajador** — depende de la respuesta a la pregunta 2 de la consulta.
