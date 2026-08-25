# Bloque 4: alcance de acceso (H-11) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

## Estado de ejecución (2026-08-05)

**Completo.** Todo el bloque está implementado y en verde en CI.

- **Tarea 1** — fundamento (`users.department_id` + `Claims` + JWT): mergeada en `main` vía PR #35.
- **Tareas 2–6** — aplicación del ámbito: PR #36 (rama `claude/bloque-3-plan-review-pos467`).

| Tarea | Contenido | Commit |
|-------|-----------|--------|
| 2 | `auth::scope::ActorScope` (deny-by-default derivado de `Claims`) | `edfea96` |
| 3 | Empleados acotados (list impone ámbito; get/update → 404 fuera de ámbito; create/move rechazado 403) | `349dfce` |
| 4a | Eventos acotados (list/get; unknown-face denegado a scoped) | `ac5a998` |
| 4b | Leaves + daily-records acotados | `1c5c3f9` |
| 4c | Reports acotados (`scoped_department_ids` impone el departamento del actor sobre el request) | `2db7dd1` |
| 4d + 5 | Stream SSE filtrado por suscriptor; foto biométrica + evidencia médica movidas a supervisor+ y acotadas (D2) | `6cee84d` |

**Decisiones de política aplicadas (defaults del plan):** **D1** = `department_id` NULL es org-wide; **D2** = separación estricta (viewer no ve archivos biométricos/de salud); **D3** = un departamento por usuario.

**Contrato de pruebas (Tarea 6):** las pruebas negativas viven junto a cada tarea — `access_scope_test`, `employee_scope_test`, `events_scope_test`, `leaves_daily_scope_test`, `reports_scope_test` — cubriendo lectura/escritura cross-department denegada, admin sin ámbito, y NULL-department org-wide (D1). Los tests RBAC preexistentes usan tokens sin ámbito, así que con D1 su comportamiento no cambia (no requirieron edición, solo se ajustaron las firmas de los callers de `service::list`).

**Goal:** Reemplazar el RBAC puramente por rol global por un **modelo de ámbito** (matriz recurso/acción/ámbito). Hoy cualquier `viewer` autenticado lee toda la biometría, salud y PII de la instalación, y cualquier `supervisor` edita a cualquier empleado, sin ningún alcance departamental. Este bloque acota supervisor y viewer a su departamento, separa la biometría/salud, hace el filtrado **obligatorio en la consulta** (no elegido por el llamador), aplica **autorización denegada por defecto**, y añade **pruebas negativas**.

**Architecture:** El ámbito vive en la **cadena de identidad** — `users.department_id` → `Claims.department_id` → aplicado en cada handler vía un extractor de ámbito y un **predicado de filtrado obligatorio** en la capa de servicio. No es un parche por endpoint: es una dimensión de autorización nueva que atraviesa lectura y escritura.

**Tech Stack:** Rust/Axum 0.8, libSQL/SQLite, `jsonwebtoken`, `cargo nextest`.

> **Aclaración sobre "sin multi-tenant" (PROJECT.md):** acotar por departamento **dentro de una instalación** NO es multi-tenancy. Sigue siendo un cliente, una base de datos, un despliegue. El alcance departamental es mínimo-privilegio interno (ISO 27001 / OWASP ASVS), no aislamiento entre clientes. No contradice esa decisión fundacional.

---

## Decisiones pendientes (política — requieren tu confirmación antes de ejecutar)

> **RESUELTAS (2026-08-05):** el dueño aprobó ejecutar con los defaults recomendados
> — **D1** NULL = org-wide, **D2** separación estricta, **D3** un departamento. La
> tabla siguiente queda como referencia de las alternativas consideradas.

Este bloque construye **mecanismo**, pero tres decisiones son de política y las decides tú. La recomendación de cada una es el default que implementaría si dices "adelante" sin especificar.

| # | Decisión | Recomendación (default) | Alternativa |
|---|---|---|---|
| **D1** | ¿`department_id` NULL en un supervisor/viewer significa *toda la instalación* o *acceso denegado*? | **NULL = org-wide.** La migración no deja fuera a los operadores existentes (que hoy no tienen departamento); el alcance se **activa** al asignar un departamento. Admin siempre sin ámbito. | NULL = denegar (más estricto, pero bloquea a todo usuario existente hasta reasignarlo — riesgo operativo en el upgrade). |
| **D2** | Separación de biometría/salud: ¿un viewer **dentro** de su departamento sigue viendo la **foto facial** del evento (`/events/{id}/photo`) y la **evidencia médica** (`/leaves/{id}/evidence`)? | **No.** El viewer ve registros/metadatos; los **archivos** biométricos y de salud requieren supervisor+. Es la "separación de salud/biometría" que pide la auditoría. | Mantener esos archivos en lectura para viewer scoped (menos estricto). |
| **D3** | ¿Un usuario pertenece a **un** departamento o a **varios**? | **Uno** (`department_id` singular). Coincide con la simplicidad single-tenant; cubre el caso real. | Multi-departamento (tabla `user_departments`) — mayor complejidad; queda para futuro. |

**Nota (fuera de alcance, pero relacionada — H-12):** el access token es *stateless* y dura hasta 20 min. Cambiar el ámbito de un usuario (o degradar su rol) **no** invalida un token ya emitido: conserva su ámbito original hasta expirar. Cerrar esa ventana es H-12, no este bloque; se documenta pero no se resuelve aquí.

---

## Global Constraints

- **Nunca** usar los identificadores desnudos `execute`, `execute_batch` ni `transaction` dentro de `backend/src` — gate de build (allowlist: `db/mod.rs`, `db/write_queue.rs`, `bin/seed_e2e.rs`, `test_reset/mod.rs`).
- **Nunca romper la arquitectura hexagonal.** El detalle Hikvision vive solo en `backend/src/isapi/*`.
- Toda mutación de datos de asistencia genera entrada de auditoría inmutable.
- Gate de cobertura: proyecto ≥90% líneas / ≥85% ramas; por archivo ≥70% / ≥60%.
- `cargo clippy --all-targets --all-features -- -D warnings` debe pasar.
- **No corras `cargo fmt` a secas** — `main` no está limpio de rustfmt.
- Mensajes de commit en inglés con prefijo convencional.
- **Deny-by-default:** ante cualquier duda de ámbito, denegar. Una consulta sin predicado de ámbito aplicado es un bug, no un caso por defecto.

---

## Contexto imprescindible

### El estado actual (verificado, `docs/auditoria/verificacion/lote-2.md:83-110`)

- **`Claims`** (`auth/models.rs:40-53`) no lleva ningún ámbito; **`users`** (`migrations/001:7-19`) no tiene `department_id` (ninguna de las 29 migraciones lo añade). Los roles son `Admin`/`Supervisor`/`Viewer`.
- **`viewer_routes`** (`main.rs:326-362`, gate `require_auth`) deja a **cualquier** rol autenticado leer: `/employees`, `/employees/{id}`, `/events`, `/events/{id}`, `/events/{id}/photo` (foto facial), `/daily-records`, `/leaves`, `/leaves/{id}/evidence` (evidencia médica), `/devices`, `/departments`, `/rules`, `/tenant-info`. **Sin filtro de departamento.**
- **`update_employee`** (`employees/handlers.rs:65-78`, bajo `supervisor_routes`) **ni siquiera extrae `Claims`** — cualquier supervisor edita a cualquier empleado.
- `department_id` en `employees/service.rs:189-206` es solo un **filtro opcional que elige el llamador**, no una restricción derivada de la identidad.
- `require_admin` / `require_supervisor_or_above` (`auth/rbac.rs:36-81`) solo comparan `claims.role`.

### Lo que NO cambia (fuera de alcance)

- **H-12** (rate limiting, revocación stateless, política de contraseña) — su propio trabajo.
- **Rol de auto-servicio del empleado**: hoy no existe un rol "employee". No se añade en este bloque; el alcance es admin/supervisor/viewer.
- **Multi-departamento** (salvo que D3 diga lo contrario).
- Los datos de enrolamiento (`/enrollments/*`) ya están tras `require_admin`; se mantienen admin-only (los handlers extraen `AuthUser(_claims)` y lo descartan — se puede aprovechar para auditar, pero el gate no cambia).

---

## Matriz recurso / acción / ámbito (objetivo)

`own-dept` = solo empleados cuyo `department_id` coincide con el del actor (o cualquiera si el actor no tiene ámbito, según D1). Deny-by-default fuera de la matriz.

| Recurso / acción | Admin | Supervisor | Viewer |
|---|---|---|---|
| `GET /employees`, `/employees/{id}` | todos | own-dept | own-dept |
| `POST /employees`, `PATCH /employees/{id}` | todos | own-dept | ✗ |
| `DELETE /employees/{id}` | todos | ✗ (hoy admin-only) | ✗ |
| `GET /events`, `/daily-records`, `/leaves` (metadatos) | todos | own-dept | own-dept |
| `GET /events/{id}/photo` (foto facial) | ✓ | own-dept | **✗ (D2)** |
| `GET /leaves/{id}/evidence` (salud) | ✓ | own-dept | **✗ (D2)** |
| `POST /reports/*` | todos | own-dept | ✗ (hoy supervisor+) |
| `/users`, `/devices` (escritura), `/tenant-info` (escritura), `/rules` (escritura) | ✓ | ✗ | ✗ |

---

## File Structure

| Archivo | Responsabilidad | Tarea |
|---|---|---|
| `migrations/030_users_department_id.sql` | Ámbito en la tabla `users` | 1 |
| `auth/models.rs`, `auth/service.rs` | Ámbito en `Claims` + emisión/decodificación del JWT | 1 |
| `users/models.rs`, `users/service.rs` | Asignar departamento al crear/editar usuario | 1 |
| `auth/rbac.rs` (o `auth/scope.rs` nuevo) | Extractor de ámbito + predicado obligatorio, deny-by-default | 2 |
| `employees/handlers.rs`, `employees/service.rs` | Lectura/escritura de empleado acotada | 3 |
| `events/`, `leaves/`, `daily_records/`, `reports/` handlers+service | Lecturas/reportes acotados por el departamento del empleado (incl. stream SSE) | 4 |
| `main.rs` | Mover foto/evidencia fuera de `viewer_routes` (D2) | 5 |
| `tests/` + `tests/common/mod.rs` | Dimensión de ámbito en helpers de token; pruebas negativas | todas |

---

### Task 1: El ámbito entra en la cadena de identidad

**Files:** migración `030`; `auth/models.rs`; `auth/service.rs`; `users/models.rs`; `users/service.rs`; `tests/` (crear `access_scope_identity_test.rs`).

- [ ] **Step 1: Pruebas que fallan** — un usuario creado con `department_id` lo persiste; su access token lleva `department_id` en los claims; decodificar lo recupera; un usuario sin departamento lleva `None`.
- [ ] **Step 2: Correr y ver fallar.**
- [ ] **Step 3: Migración** — `ALTER TABLE users ADD COLUMN department_id TEXT REFERENCES departments(id)` (nullable). Registrar la tupla en `db/mod.rs`. **NULL por defecto** (D1: NULL = org-wide).
- [ ] **Step 4: `Claims` + JWT** — `Claims.department_id: Option<String>`; `issue_access_token` lo incluye; `verify_access_token` lo decodifica. Cuidado: cambiar la forma de `Claims` toca todos los `test_access_token(...)` (los helpers ganan un parámetro opcional de departamento — mantén una variante compatible).
- [ ] **Step 5: Crear/editar usuario con departamento** — `CreateUserRequest` gana `department_id: Option<String>`; `users/service.rs` lo persiste y valida que exista (FK). Solo admin gestiona usuarios (sin cambio de gate).

  **PATCH tri-estado (obligatorio):** `UpdateUserRequest.department_id` debe ser `Option<Option<String>>`, no `Option<String>`. El patrón de update actual solo aplica campos `Some`, así que `Option<String>` no puede distinguir "campo omitido" de "`null` explícito" — un admin podría asignar un departamento pero **nunca volver a dejarlo en NULL (org-wide, D1)**. `None` = no tocar; `Some(None)` = limpiar a NULL; `Some(Some(id))` = asignar. Probar **ambos**: omisión y limpieza explícita. (Requiere `#[serde(default, deserialize_with = ...)]` o el patrón de doble-Option que use el repo.)
- [ ] **Step 6: Verificar y commitear.**

---

### Task 2: Extractor de ámbito y predicado obligatorio (deny-by-default)

**Files:** `auth/scope.rs` (nuevo) o extender `auth/rbac.rs`; `tests/`.

- [ ] **Step 1: Pruebas que fallan** — un `ActorScope` derivado de `Claims`: `Unscoped` (admin, o `department_id` NULL según D1) vs `Department(id)`. Un helper que produce el predicado SQL/So filtro para "empleados en el ámbito del actor". Un admin no filtra; un supervisor scoped filtra por su departamento.
- [ ] **Step 2: Correr y ver fallar.**
- [ ] **Step 3: El mecanismo** — `ActorScope::from_claims(&Claims)`, y un método que, dado un `department_id` de recurso, responde `permitido/denegado`. **La ausencia de ámbito aplicado es un error, no un pase.** Documentar que admin es el único `Unscoped` legítimo.
- [ ] **Step 4: Verificar y commitear.**

---

### Task 3: Empleados acotados (lectura y escritura, incluida la creación)

**Files:** `employees/handlers.rs`, `employees/service.rs`, `tests/employee_scope_test.rs` (crear).

- [ ] **Step 1: Pruebas que fallan** — un supervisor de Dept-A: `GET /employees` solo lista los de A; `GET/PATCH /employees/{id-de-B}` responde **404** (no 403 — no filtrar existencia); un supervisor de A sí edita a los de A. **`POST /employees` con `department_id = B` es rechazado** (403/422) o forzado a A — un supervisor no crea fuera de su ámbito. Un admin ve/edita/crea en cualquiera.
- [ ] **Step 2: Correr y ver fallar.**
- [ ] **Step 3: Aplicar** — `create_employee`/`list_employees`/`get_employee`/`update_employee`/`deactivate_employee` extraen `AuthUser(claims)` → `ActorScope`. `list` añade el predicado de ámbito **además** de los filtros del llamador. `get`/`update`/`deactivate` verifican el departamento del empleado contra el ámbito y devuelven `404` si no coincide. **`update_employee` y `create_employee` por fin extraen claims.**

  **Creación acotada (Codex P1):** hoy `create_employee` (`handlers.rs:18-29`) no extrae claims y pasa el `department_id` del request directo al servicio. Un supervisor acotado **no** puede crear en otro departamento: si el actor está acotado, el `department_id` del empleado se **impone** desde el ámbito del actor (o se rechaza si el request pide otro), nunca se confía en el request.

  **404, no 403** (para recursos existentes fuera de ámbito): responder 404 evita filtrar su existencia. Documentarlo.
- [ ] **Step 4: Verificar y commitear.**

---

### Task 4: Eventos, permisos, registros diarios, stream SSE y reportes acotados

**Files:** `events/`, `leaves/`, `daily_records/`, `reports/` (handlers+service); `main.rs` (stream); `tests/`.

- [ ] **Step 1: Pruebas que fallan** — un supervisor/viewer de A solo ve eventos/permisos/registros de empleados de A; los de B no aparecen (y `GET .../{id}` de B → 404).
- [ ] **Step 2: Correr y ver fallar.**
- [ ] **Step 3: Aplicar (list/detail)** — cada consulta de lista/detalle une con `employees` y filtra por el ámbito del actor. Reusar el predicado de la Tarea 2. Cuidado con las consultas que hoy no unen `employees` — puede requerir un JOIN nuevo (verificar que no rompa agregaciones existentes).
- [ ] **Step 4: El stream SSE (Codex P1)** — `GET /events/stream` (`events/handlers.rs:35-54`) hoy verifica el token pero **descarta los claims** y reenvía el broadcast compartido —cuyo payload incluye id/nombre/departamento del empleado— a **todos** los suscriptores. Un viewer/supervisor acotado seguiría recibiendo PII de asistencia en vivo de otros departamentos. Extraer los claims → `ActorScope` y **filtrar por suscriptor**: no emitir eventos de empleados fuera del ámbito. El payload ya trae `department` (o se resuelve al enriquecer). Prueba de stream cross-department: un suscriptor de A no recibe el evento de un empleado de B.
- [ ] **Step 5: Reportes (Codex P1)** — `POST /reports/json|excel` (`reports/service.rs::compute_report`) hoy confía en el `department_ids` **opcional que elige el llamador**; un supervisor acotado puede **omitirlo** y descargar la nómina de todos los departamentos. Imponer el ámbito del actor **independiente** del filtro del request: si el actor está acotado, su departamento se **intersecta/impone** sobre cualquier `department_ids` pedido (o lo reemplaza), nunca se confía en el request. Reusar el predicado de la Tarea 2 en las consultas del reporte. Prueba negativa: un supervisor de A que omite `department_ids` obtiene solo A; si pide B, no obtiene B.
- [ ] **Step 6: Verificar y commitear.**

---

### Task 5: Separación de biometría/salud (D2)

**Files:** `main.rs`; `tests/`.

- [ ] **Step 1: Pruebas que fallan** — un viewer (aún dentro de su departamento) recibe **403** en `GET /events/{id}/photo` y `GET /leaves/{id}/evidence`; un supervisor de ese departamento recibe 200; de otro departamento, 404.
- [ ] **Step 2: Correr y ver fallar.**
- [ ] **Step 3: Aplicar** — mover `/events/{id}/photo` y `/leaves/{id}/evidence` de `viewer_routes` a un grupo `require_supervisor_or_above` (+ el filtro de ámbito de la Tarea 4). **Solo si D2 = "no viewer".**
- [ ] **Step 4: Verificar y commitear.**

---

### Task 6: Contrato de pruebas y helpers

**Files:** `tests/common/mod.rs`; los ~30 tests que fijan RBAC.

- [ ] **Step 1** — `test_access_token` gana una variante con `department_id`. Los tests existentes que asumen "viewer lee todo" se actualizan al nuevo contrato (viewer scoped). Los tests de grupo→rol (`employee_tests.rs`, `audit_handlers_test.rs`, `enrollments_handlers_test.rs`) se revisan.
- [ ] **Step 2** — batería de **pruebas negativas**: cross-department read/write denegado; admin sin ámbito; NULL-department según D1.
- [ ] **Step 3: Verificar, correr el gate de cobertura, commitear.**

---

## Fuera de alcance, y por qué

- **H-12** (rate limit, revocación stateless de token/rol, política de contraseña) — su propio bloque. Nota: mientras H-12 no se resuelva, un cambio de ámbito tarda hasta 20 min en surtir efecto sobre tokens ya emitidos.
- **Rol de empleado / auto-servicio** — no existe hoy; añadirlo es un rediseño de roles aparte.
- **Multi-departamento por usuario** — salvo que D3 lo pida.

## Pendiente que no es código

- Asignar departamentos a los usuarios existentes tras el upgrade (operativo). Con D1 = NULL-org-wide, es opt-in y no bloquea; con NULL-deny, es obligatorio antes de que los no-admin puedan trabajar.
