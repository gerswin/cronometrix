# Verificación — Lote 2 (H-09, H-10, H-11, H-12)

Trabajo de solo lectura sobre `/home/gerswin/Proyectos/cronometrix/.claude/worktrees/scratch`
(rama `docs/verificacion-lote2`, HEAD `48af434`). No se modificó ningún archivo.

---

## H-09 — Reportes sin vigencia histórica ni cierre de período

**Veredicto: CONFIRMED**

Evidencia:

- `backend/src/rules/handlers.rs:52-119` (`update_rules`): `global_rules` es una fila
  **singleton** (`WHERE id = 'singleton'`). Cada `PATCH /rules` hace `UPDATE ... SET
  effective_from = unixepoch(), version = version + 1 WHERE id = 'singleton'` — sobrescribe
  la única fila existente. No hay una tabla de versiones por intervalo; `effective_from`
  solo registra cuándo cambió la fila actual, no permite seleccionar la regla vigente en
  una fecha pasada. Un reporte de un período anterior siempre lee la regla *actual*.
- `backend/src/reports/service.rs:286-318`: el `SELECT` que arma cada fila del reporte lee
  `e.name`, `e.position`, `e.base_salary_cents`, `e.salary_kind` directamente de la tabla
  `employees` (estado actual), sin join a ninguna tabla histórica ni columna
  "as-of"/vigencia. Un cambio de sueldo o cargo hoy altera retroactivamente cualquier
  reporte que se regenere para un período pasado.
- Búsqueda exhaustiva de gobernanza de período: `grep -rn "report_run\|calculation_run\|period_lock\|locked_period\|report_snapshot\|payroll_run"` sobre
  `backend/src` y `backend/tests` no arrojó resultados. `backend/src/reports/periods.rs` es
  matemática pura de rangos de fecha (`resolve_period`) — no tiene ningún concepto de
  estado abierto/cerrado.
- Ninguna migración (`backend/src/db/migrations/001..027`) crea una tabla de snapshot,
  aprobación, hash de artefacto o reapertura controlada.

Confirmado: no existe versión histórica de reglas/salario/cargo seleccionable por fecha, ni
cierre/aprobación/snapshot/hash de un cálculo de reporte. El hallazgo es sustancialmente
correcto y su descripción no induce a error.

---

## H-10 — Sin gobierno del ciclo de vida biométrico y médico

**Veredicto: CONFIRMED** (con una advertencia sobre la recomendación, ver abajo)

Evidencia:

- Búsqueda de gobernanza de datos: `grep -rli "consent\|consentimiento\|retention\|retencion\|data_subject\|right_to_erasure\|purge\|scheduled_delete"` sobre
  `backend/src` no encontró registros de aviso, finalidad, base jurídica, vencimiento o
  canal de derechos (ARCO). No existe tabla ni campo de consentimiento en ninguna
  migración.
- `backend/src/workers/purge.rs` (`PurgeWorker`, D-15) es el único mecanismo de borrado
  ligado a la baja de un empleado: al desactivar un empleado, borra **solo** las filas de
  `device_face_mappings` y revoca el rostro en el equipo Hikvision
  (`isapi.revoke(&fid)`, línea 262). No toca `state.paths.events_root` (fotos de eventos),
  `state.paths.enrollments_root` (fotos/XML de enrolamiento) ni `state.paths.leaves_root`
  (evidencias médicas/de permisos). Esto reproduce exactamente la afirmación del hallazgo:
  "limpia mapeos del equipo, pero conserva rostro, fotos, XML y evidencias indefinidamente".
- El único otro worker de limpieza de archivos es `backend/src/workers/capture_cleanup.rs`
  (comentario: "Lifecycle owner for kiosk-capture state and **temporary** JPEGs"), con TTL
  de 120s/5min — limpia solo capturas transitorias de kiosco durante el enrolamiento, no
  las fotos/evidencias persistentes.
- `grep -rln "remove_file\|fs::remove"` sobre `backend/src` solo encuentra
  `capture_cleanup.rs` y `storage/atomic_file.rs` (helper genérico); ningún llamador de
  ese helper borra fotos de eventos, enrolamientos o evidencia de permisos por retención.

Confirmado: no existe gobernanza del ciclo de vida (aviso, base jurídica, vencimiento,
derechos ARCO, borrado programado) para datos biométricos/médicos, y la baja de un
empleado deja intactos rostro, fotos, XML y evidencias.

**Advertencia sobre la recomendación (no cambia el veredicto, pero es una trampa real
para quien implemente):** la recomendación de H-10 pide "plazos y borrado verificable"
para datos biométricos/médicos. Los mismos artefactos que H-10 quiere hacer expirar
(fotos de eventos, XML, evidencias) son exactamente los que H-09 exige preservar como
snapshot inmutable/soporte de auditoría de nómina, y que H-09 cita a la COT 2020 sobre
conservación tributaria. Un implementador que lea H-10 aislado y programe borrado
automático de "todo dato biométrico" tras un plazo corto puede destruir el soporte
probatorio que la propia auditoría (H-09) exige conservar para disputas laborales/fiscales.
La distinción que falta explicitar: la plantilla facial usada para el reconocimiento en
vivo (minimizable/purgable tras la baja — esto es lo que hoy NO ocurre y sí debería) es un
dato distinto de la fotografía/evento que sirve de evidencia de asistencia (con un plazo de
retención probablemente más largo, atado a la legislación laboral). Cualquier fix debe
tratarlos con calendarios de retención distintos, no una única política de "borrar todo".

---

## H-11 — Acceso demasiado amplio a datos sensibles

**Veredicto: CONFIRMED**

Evidencia:

- `backend/src/main.rs:281-317` (`viewer_routes`, ahora en 281-317, no 273-310 — el número
  de línea derivó pero el contenido es el mismo grupo): cualquier rol `Viewer` autenticado
  puede leer `GET /employees`, `/employees/{id}`, `/events`, `/events/{id}`,
  `/events/{id}/photo` (foto facial del evento), `/daily-records`, `/leaves`,
  `/leaves/{id}/evidence` (evidencia médica/justificativa), `/devices`. El único gate es
  `auth::middleware::require_auth` (verifica solo firma/expiración del JWT) +
  `license::middleware::require_license`. No hay ningún filtro de departamento.
- `backend/src/auth/rbac.rs`: `require_supervisor_or_above` (líneas 60-81) y `require_admin`
  (líneas 36-56) solo comparan `claims.role` contra `Role::Admin`/`Role::Supervisor`. No
  existe ningún concepto de alcance/`department_id` en `Claims` (`backend/src/auth/models.rs`)
  ni en la tabla `users` (`grep -n department backend/src/db/migrations/001_initial_schema.sql`
  no encuentra la columna en `users`, solo en `employees`/`departments`).
- `backend/src/employees/handlers.rs:65-78` (`update_employee`, bajo `supervisor_routes`,
  `main.rs:341-354`): el handler ni siquiera extrae `AuthUser`/`Claims` — no hay forma de
  que compare el actor contra el departamento del empleado editado. Cualquier Supervisor
  puede hacer `PATCH /employees/{cualquier-id}` sin importar el departamento.
- `department_id` en `employees/service.rs:202-205` y `289` es únicamente un **filtro
  opcional que el propio llamador elige** (query param en `list`), no una restricción
  derivada de la identidad del usuario autenticado.

Confirmado: el hallazgo es exacto — no existe alcance departamental en ningún punto del
control de acceso; RBAC es puramente por rol global.

---

## H-12 — Autenticación sin defensa suficiente contra abuso y revocación tardía

**Veredicto: CONFIRMED**

Evidencia, punto por punto:

- **Sin rate limit en ningún lado:** `grep -n "governor\|ratelimit\|rate-limit\|limit" backend/Cargo.toml`
  solo encuentra `tower-http`'s `limit` feature, que en este código se usa para
  `RequestBodyLimitLayer` (tamaño de payload) y `TimeoutLayer`, no para throttling de tasa.
  No hay `tower_governor` ni ninguna otra crate de rate limiting en el árbol de
  dependencias. Esto lo confirma además **el propio código**:
  `backend/src/setup/handlers.rs:57-58` dice textualmente: *"`/setup/init` is
  unauthenticated and this backend has no rate limiting anywhere."* — es la propia base de
  código admitiendo la ausencia total.
- **Sin bloqueo de cuenta, sin MFA:** `grep -rli "totp|mfa|two_factor|2fa|otp"` sobre
  `backend/src` (excluyendo el binario mock de pruebas) no da resultados.
- **Mínimo de 8 caracteres, sin política adicional:**
  `backend/src/users/models.rs:26,36` y el propio `setup/handlers.rs:45` validan
  `length(min = 8)` y nada más (sin complejidad, sin lista de contraseñas comprometidas).
- **Diferenciación temporal en login:** `backend/src/auth/handlers.rs:19-54` (línea actual,
  el rango citado por la auditoría `33-54` sigue siendo válido para el cuerpo de la
  función). El flujo es: `SELECT ... WHERE username = ?1` (líneas 33-39) →
  `rows.next()...ok_or(AppError::Unauthorized)?` (líneas 41-45) — **retorna inmediatamente**
  si el usuario no existe — y solo si la fila existe se llega a
  `service::verify_password(...)` (línea 54), que ejecuta Argon2id. Un usuario inexistente
  responde en microsegundos; uno existente tarda lo que cuesta Argon2 (decenas/cientos de
  ms). El comentario en línea 18 ("generic 'Invalid credentials' — no username
  enumeration") solo cubre el contenido del mensaje, no el canal de tiempo — la
  vulnerabilidad de temporización persiste tal como la describe el hallazgo.
- **Revocación tardía (hasta 20 min):** `backend/src/auth/service.rs:21-38`
  (`issue_access_token`) fija `exp: now + 20 * 60`. `backend/src/auth/middleware.rs:11-28`
  (`require_auth`) y `backend/src/auth/rbac.rs:36-81` (`require_admin`,
  `require_supervisor_or_above`) llaman únicamente a
  `service::verify_access_token`, que en `auth/service.rs:63-76` es un `decode::<Claims>`
  **puramente stateless** (sin consulta a la DB, sin verificación de `status` del usuario
  ni de versión de sesión). Desactivar un usuario o bajarle el rol no invalida un access
  token ya emitido — sigue siendo válido con sus claims originales hasta que expira,
  hasta 20 minutos después.

Confirmado en su totalidad: ausencia de rate limit/bloqueo/MFA, política de contraseña
mínima, canal de temporización en login, y revocación no inmediata de privilegios/tokens.
Nota adicional relevante: el commit hint sobre "`/setup/init` ahora rechaza un sistema ya
inicializado con un chequeo barato antes de hashear" (visto en
`setup/handlers.rs:93-117`, el `precheck_count` fuera de la transacción) es un fix real
pero **acotado**: cierra el vector de "grinder de Argon2 contra un sistema ya
inicializado" en esa ruta específica, no añade rate limiting general — el propio
comentario del archivo lo dice explícitamente. No cambia el veredicto de H-12: el login
normal (`/auth/login`) sigue sin ningún throttling.

---

## Nota sobre las dos pistas de contexto

- **Bootstrap race / `/setup/init`:** confirmado arriba, mencionado dentro de H-12 como
  contexto (el propio código documenta la ausencia de rate limiting), pero el fix de
  `/setup/init` no resuelve H-12 en general — solo un caso de esa ruta puntual.
- **Token de push de dispositivo fuera de los logs:** confirmado en
  `backend/src/http_trace.rs:5-46` (`redact_path`, referencia a C-08) — el path
  `/devices/{id}/push/{token}` se redacta incondicionalmente en los spans de tracing.
  Esto **no aparece mencionado en el texto de H-09 a H-12** tal como está redactado hoy en
  `docs/auditoria/INFORME-AUDITORIA-INTEGRAL.md` (ninguno de los cuatro habla de logging de
  tokens de push) — no afecta ninguno de los cuatro veredictos de este lote. Puede ser
  relevante para un hallazgo Crítico fuera de mi lote (p.ej. uno relacionado con exposición
  de credenciales de dispositivo), pero dentro de H-09–H-12 es irrelevante.

---

## Resumen de veredictos

| Hallazgo | Veredicto |
|---|---|
| H-09 | CONFIRMED |
| H-10 | CONFIRMED (recomendación de borrado necesita distinguir plantilla facial vs. evidencia de asistencia, ver advertencia) |
| H-11 | CONFIRMED |
| H-12 | CONFIRMED |
