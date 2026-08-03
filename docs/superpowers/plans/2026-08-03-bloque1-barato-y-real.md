# Bloque 1: barato y real — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cerrar cuatro hallazgos verificados cuyo arreglo es mecánico: la evidencia de permisos que confía en el `Content-Type` del cliente (M-07), el trigger de auditoría de empleados al que le faltan columnas (M-06), las restricciones de dominio ausentes (M-12) y la falta total de rate limiting (H-12, L-01).

**Architecture:** Cuatro tareas independientes. Ninguna necesita diseño nuevo: M-07 porta un arreglo que ya existe en este repositorio, M-06 corrige deuda que introdujimos nosotros, M-12 son restricciones SQL y H-12/L-01 comparten un único middleware.

**Tech Stack:** Rust/Axum 0.8, libSQL/SQLite, migraciones numeradas, `cargo nextest`.

## Global Constraints

- **Nunca** usar los identificadores desnudos `execute`, `execute_batch` ni `transaction` dentro de `backend/src` — `scripts/check_db_write_queue.py` es un gate que falla la build.
- **Nunca romper la arquitectura hexagonal.** El detalle de vendedor vive solo en `backend/src/isapi/*`.
- Toda mutación de datos de asistencia genera entrada de auditoría inmutable.
- Gate de cobertura: proyecto ≥90% líneas / ≥85% ramas; por archivo ≥70% / ≥60%.
- `cargo clippy --all-targets --all-features -- -D warnings` debe pasar.
- **No hay datos productivos.** Migraciones limpias.
- Próxima migración libre: **028** (la 027 es `device_push_inbox`). Corre `ls backend/src/db/migrations/` antes de crear.
- Mensajes de commit en inglés con prefijo convencional.
- El proyecto fija **Node 24.15.0**; con Node 22 fallan tests de frontend por entorno.

## Contexto imprescindible

Los cuatro fueron **verificados** contra `48af434` — ver `docs/auditoria/VERIFICACION-LOTE-2.md` y la evidencia en `docs/auditoria/verificacion/`. No son sospechas.

Este bloque existe porque el riesgo por unidad de esfuerzo es el más alto de los 25 hallazgos restantes: dos de los arreglos ya están escritos en algún lugar del repositorio y solo hay que llevarlos a donde faltan.

---

## File Structure

| Archivo | Responsabilidad | Tarea |
|---|---|---|
| `backend/src/leaves/handlers.rs` | Validar magic bytes de la evidencia | 1 |
| `backend/src/db/migrations/028_employee_audit_full_columns.sql` | Trigger con todas las columnas | 2 |
| `backend/src/db/migrations/029_domain_constraints.sql` | `CHECK` de dominio | 3 |
| `backend/src/middleware/rate_limit.rs` | Limitador por IP y por cuenta | 4 |
| `backend/src/main.rs` | Montar el limitador | 4 |

---

### Task 1: La evidencia de permisos deja de creerle al cliente (M-07)

`backend/src/leaves/handlers.rs:104-113` toma `field.content_type()` —una cabecera que envía el cliente— y valida contra una lista de tipos permitidos. Un atacante sube HTML con `Content-Type: image/jpeg` y el archivo queda en el directorio de evidencias médicas.

**El arreglo ya existe en este repositorio.** `backend/src/daily_records/handlers.rs:44-58` tiene `infer_evidence_ext_from_magic`, con el comentario `CR-03: authoritative type check via magic bytes — content-type is advisory`. Nunca se portó a `leaves`.

**Files:**
- Modify: `backend/src/leaves/handlers.rs`
- Test: `backend/tests/leaves_evidence_magic_test.rs` (crear)

**Interfaces:**
- Consumes: el patrón de `daily_records/handlers.rs:44-58`. **Léelo antes de escribir nada** y sigue su forma; si tiene sentido extraerlo a un módulo compartido en vez de duplicarlo, hazlo — pero no cambies el comportamiento del lado que ya funciona.

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
mod common;

/// M-07: un HTML disfrazado de JPEG no puede llegar al directorio de
/// evidencias. La cabecera del cliente es orientativa, no autoritativa.
#[tokio::test]
async fn html_declared_as_jpeg_is_rejected() {
    // subir b"<html>..." con content_type image/jpeg -> 4xx, nada en disco
}

/// Un JPEG real sigue aceptándose — el arreglo no puede romper el caso normal.
#[tokio::test]
async fn a_real_jpeg_is_accepted() {
    // cabecera JPEG valida (0xFF 0xD8 0xFF) -> 2xx y archivo presente
}

/// La extensión guardada sale de los bytes, no de la cabecera ni del nombre.
#[tokio::test]
async fn the_stored_extension_comes_from_the_bytes() {
    // PNG real declarado como pdf -> se guarda como .png
}
```

- [ ] **Step 2: Correr y ver que fallan**

Run: `cargo nextest run --all-features -j 8 -E 'binary(leaves_evidence_magic_test)'`
Expected: el primero falla — hoy se acepta.

- [ ] **Step 3: Portar la validación**

Aplicar en `leaves/handlers.rs` el mismo criterio que `daily_records`: leer los bytes, derivar el tipo real, rechazar si no está en la lista permitida, y **derivar la extensión de los bytes**, no del nombre ni de la cabecera.

Mantener la lista de tipos actual (pdf, jpeg, png) — el hallazgo es sobre en qué se confía, no sobre qué se permite.

- [ ] **Step 4: Verificar y commitear**

Run: `cargo nextest run --all-features -j 8 && cargo clippy --all-targets --all-features -j 8 -- -D warnings && python3 scripts/check_db_write_queue.py && make coverage-backend`

```bash
git add backend/src/leaves/handlers.rs backend/tests/leaves_evidence_magic_test.rs
git commit -m "fix(leaves): verify evidence type from magic bytes, not the client header (M-07)"
```

---

### Task 2: El trigger de auditoría de empleados registra todas las columnas (M-06)

`018_employees_base_salary.sql:12-35` hace `DROP TRIGGER` y recrea `audit_employees_update`. Al recrearlo no incluyó `position`, `hire_date` ni `face_id`, y desde entonces se añadieron `salary_kind` y `terminated_on` —**las dos por nosotros**— sin volver a tocarlo.

Consecuencia: cambiar el salario de un empleado se audita; cambiar su **unidad salarial** no. Y esa unidad multiplica el importe por treinta si se equivoca.

**Files:**
- Create: `backend/src/db/migrations/028_employee_audit_full_columns.sql`
- Test: `backend/tests/employee_audit_columns_test.rs` (crear)

- [ ] **Step 1: Escribir la prueba que falla**

```rust
mod common;

/// M-06: cambiar salary_kind cambia lo que cobra un empleado por treinta.
/// Tiene que quedar en el registro de auditoría.
#[tokio::test]
async fn changing_salary_kind_is_audited() {
    // PATCH salary_kind daily -> monthly
    // asertar que audit_log tiene la fila con old y new de salary_kind
}

/// Y el resto de columnas que el trigger perdió al recrearse.
#[tokio::test]
async fn position_hire_date_and_terminated_on_are_audited() { }
```

- [ ] **Step 2: Correr y ver que falla**

Run: `cargo nextest run --all-features -j 8 -E 'binary(employee_audit_columns_test)'`

- [ ] **Step 3: Recrear el trigger completo**

**Lee `018_employees_base_salary.sql` entero antes de escribir.** La migración nueva repite el mismo `DROP` + `CREATE`, añadiendo a los objetos `json_object` de `old` y `new` las cinco columnas que faltan: `position`, `hire_date`, `face_id`, `salary_kind`, `terminated_on`.

**Comprueba el esquema real** con `.schema employees` o leyendo las migraciones 015, 018, 024 y 026 — no asumas nombres desde este plan.

Deja en el comentario de cabecera la razón por la que esto vuelve a ocurrir: **recrear un trigger para añadir una columna pierde silenciosamente las que se añadieron entre medias**. Cualquier migración futura que toque `employees` tiene que revisar este trigger.

- [ ] **Step 4: Verificar y commitear**

```bash
git add backend/src/db/migrations/028_employee_audit_full_columns.sql backend/tests/employee_audit_columns_test.rs
git commit -m "fix(audit): record every employee column the trigger had silently dropped (M-06)"
```

---

### Task 3: Restricciones de dominio en la base (M-12, parcial)

La base acepta salarios negativos y minutos fuera de rango. La validación vive solo en la capa de servicio, así que cualquier escritura que no pase por ahí —una migración, un script, un futuro endpoint— puede dejar datos imposibles.

**Alcance acotado a propósito:** este plan hace las restricciones de dominio. El `/health` que solo ejecuta `SELECT 1` es el otro medio hallazgo de M-12 y **no entra aquí** — necesita decidir qué se considera sano (cola de recálculo, ingesta, sync, disco, licencia) y eso es diseño, no una restricción.

**Files:**
- Create: `backend/src/db/migrations/029_domain_constraints.sql`
- Test: `backend/tests/domain_constraints_test.rs` (crear)

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
mod common;

/// M-12: la base no puede aceptar un salario negativo aunque el servicio lo
/// valide — la validación de servicio no cubre migraciones ni scripts.
#[tokio::test]
async fn a_negative_salary_is_rejected_by_the_database() { }

/// Minutos trabajados negativos o por encima de un día imposible.
#[tokio::test]
async fn out_of_range_minutes_are_rejected_by_the_database() { }
```

- [ ] **Step 2: Correr y ver que fallan**

- [ ] **Step 3: La migración**

**SQLite no permite añadir un `CHECK` a una tabla existente con `ALTER TABLE`.** Recrear la tabla es el camino soportado, y es delicado: hay que preservar índices, triggers y claves foráneas.

**Antes de escribir, lee `backend/src/db/migrations/014_phase5_audit_triggers.sql`** — usa `PRAGMA writable_schema`, que es justamente lo que M-12 señala como frágil. No copies ese enfoque.

Si recrear las tablas resulta más arriesgado que el beneficio, **dilo en el informe y propón el alcance reducido** (por ejemplo, solo las columnas de dinero) en vez de forzar una migración que pueda perder un índice. Un `CHECK` menos es mejor que un índice perdido.

Cubrir al menos: `employees.base_salary_cents > 0`, y los minutos de `daily_records` dentro de un rango sensato.

- [ ] **Step 4: Verificar el esquema tras migrar**

Además de los tests: comprobar que índices, triggers y claves foráneas siguen ahí tras la recreación. Un `.schema` antes y después, comparado, va en el informe.

- [ ] **Step 5: Commit**

```bash
git add backend/src/db/migrations/029_domain_constraints.sql backend/tests/domain_constraints_test.rs
git commit -m "fix(db): reject impossible salaries and minutes at the database level (M-12)"
```

---

### Task 4: Rate limiting (H-12, L-01)

**No hay rate limiting en ninguna parte del backend.** Lo admite el propio código en `backend/src/setup/handlers.rs:57-58`, en un comentario que escribimos al cerrar el DoS de Argon2.

Sin él: `/auth/login` permite credential stuffing sin coste, `/setup/status` permite enumerar instalaciones, y el hash Argon2 de `/setup/init` solo está protegido por un chequeo previo que un atacante puede seguir provocando.

**Files:**
- Create: `backend/src/middleware/rate_limit.rs`
- Modify: `backend/src/main.rs`
- Test: `backend/tests/rate_limit_test.rs` (crear)

**Interfaces:**
- Produces: capa de middleware aplicable por ruta.

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
mod common;

/// H-12: intentos repetidos de login desde la misma IP se frenan.
#[tokio::test]
async fn repeated_failed_logins_from_one_ip_are_throttled() {
    // N intentos fallidos -> el siguiente devuelve 429
}

/// Un usuario legítimo no queda bloqueado por el ruido de otro.
#[tokio::test]
async fn one_noisy_ip_does_not_lock_out_a_different_one() { }

/// L-01: el endpoint publico de estado tambien se limita.
#[tokio::test]
async fn the_public_status_endpoint_is_throttled() { }
```

- [ ] **Step 2: Correr y ver que fallan**

- [ ] **Step 3: Elegir e implementar**

`tower_governor` es la opción idiomática con Axum y Tower. **Antes de añadir la dependencia**, comprueba si algo equivalente ya está en el árbol — este proyecto ya ha tenido una dependencia añadida que resultó estar disponible transitivamente.

Aplicar al menos a `/auth/login`, `/setup/init` y `/setup/status`.

**Cuidado con dos cosas:**

1. **La clave del límite.** Por IP es lo obvio y es insuficiente detrás de un proxy: todo el tráfico llega con la IP de nginx. Comprueba si `X-Forwarded-For` se propaga y, si no, dilo en el informe en vez de implementar un límite que en producción cuenta a todos los clientes como uno.
2. **No romper los E2E.** La suite de Playwright hace muchas peticiones seguidas contra los mismos endpoints. Si el límite es agresivo, la rompes. Corre `make e2e` y ajusta el umbral con ese dato, no a ojo.

- [ ] **Step 4: Verificar**

Run: `cargo nextest run --all-features -j 8 && cargo clippy --all-targets --all-features -j 8 -- -D warnings && make coverage-backend`
Y además: `make e2e` — esta tarea es la que más probablemente rompa E2E.

- [ ] **Step 5: Commit**

```bash
git add backend/src/middleware/ backend/src/main.rs backend/tests/rate_limit_test.rs
git commit -m "feat(security): add rate limiting to the public authentication surface (H-12, L-01)"
```

---

## Fuera de alcance

- **La otra mitad de M-12** — `/health` como `SELECT 1`. Necesita decidir qué se considera sano; es diseño.
- **Revocación inmediata de tokens** (la otra mitad de H-12) — los access tokens son decodificación JWT pura y desactivar a alguien no revoca su token durante hasta 20 minutos. Arreglarlo exige versión de sesión o consulta por petición, con su coste. Bloque aparte.
- **Todo el resto de la hoja de ruta** — ver `docs/auditoria/HOJA-DE-RUTA.md`.

## Pendiente que no es código

**La rotación de la clave de licencia.** C-06 está contenido, no cerrado.
`git show 6edc39f:do-functions/test-keys/test_priv.pem` sigue funcionando para
cualquiera con un clon. Vence antes de emitir la primera licencia real.
