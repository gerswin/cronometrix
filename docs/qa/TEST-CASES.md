# Cronometrix — Casos de Prueba Formales (TC-###)

Catálogo trazable de casos de prueba derivado de `docs/QA-GUIDE.md`. Cada TC tiene ID estable, prioridad, precondiciones, pasos numerados, resultados esperados, y trazabilidad a regla de negocio (§21) o requisito (R-###).

**Convenciones:**
- ID: `TC-MOD-NNN` donde MOD ∈ {AUTH, DASH, EMP, DEP, TS, DEV, RPT, ANO, AUD, EVT, RUL, USR, RBAC, RULE, MNY, CON, SEC}
- Prioridad: P0 (smoke), P1 (regresión), P2 (extendido), P3 (cosmético)
- Tipo: F (Funcional), UI, INT (Integración), REG (Regresión), SEC, PERF
- Estado inicial: `Not Run`. Se actualiza por release.

**Entorno demo:** ver `docs/QA-GUIDE.md` §0. Usuarios `demo_admin`/`demo_super`/`demo_viewer`, password `dSQBALuQgXWZp6Oo`.

---

## Índice por módulo

| Módulo | Rango | TCs | Smoke (P0) |
|---|---|---|---|
| Autenticación | TC-AUTH-001..009 | 9 | 3 |
| Dashboard | TC-DASH-001..006 | 6 | 1 |
| Empleados | TC-EMP-001..012 | 12 | 3 |
| Departamentos | TC-DEP-001..006 | 6 | 1 |
| Timesheet | TC-TS-001..010 | 10 | 2 |
| Dispositivos | TC-DEV-001..007 | 7 | 1 |
| Reportes | TC-RPT-001..009 | 9 | 2 |
| Anomalías | TC-ANO-001..005 | 5 | 1 |
| Auditoría | TC-AUD-001..005 | 5 | 1 |
| Eventos | TC-EVT-001..005 | 5 | 0 |
| Reglas globales | TC-RUL-001..005 | 5 | 1 |
| Usuarios | TC-USR-001..005 | 5 | 1 |
| RBAC matrix | TC-RBAC-001..010 | 10 | 3 |
| Reglas tolerancia | TC-RULE-001..008 | 8 | 0 |
| Money / LOTTT | TC-MNY-001..010 | 10 | 1 |
| Concurrencia | TC-CON-001..003 | 3 | 0 |
| Seguridad | TC-SEC-001..006 | 6 | 1 |
| **Total** | | **111** | **22** |

---

## 1. Autenticación (TC-AUTH-###)

### TC-AUTH-001: Login admin OK
- **Prioridad:** P0
- **Tipo:** F
- **Trazabilidad:** §2
- **Precondiciones:** Logout previo. Cookie limpia.
- **Pasos:**
  1. Navegar a `https://app-demo.cronometrix.app`
     **Esperado:** Pantalla login en inglés (Addendum D-19).
  2. Ingresar `demo_admin` / `dSQBALuQgXWZp6Oo`
     **Esperado:** Campos aceptan input.
  3. Click "Sign In"
     **Esperado:** Redirige a `/dashboard`. Sidebar muestra: Dashboard, Empleados, Marcaciones, Dispositivos, Reportes, Anomalías, Auditoría, Eventos, Settings (Usuarios + Reglas + Departamentos + Tenant).
- **Postcondiciones:** Cookie `httpOnly` con JWT.

### TC-AUTH-002: Login supervisor OK
- **Prioridad:** P0
- **Tipo:** F
- **Trazabilidad:** §2, §16
- **Precondiciones:** Logout previo.
- **Pasos:**
  1. Login `demo_super` / pass demo
     **Esperado:** Redirige `/dashboard`. Sidebar oculta: Settings → Usuarios, Settings → Reglas. Anomalías visible.

### TC-AUTH-003: Login viewer OK
- **Prioridad:** P0
- **Tipo:** F
- **Trazabilidad:** §2, §16
- **Pasos:**
  1. Login `demo_viewer` / pass demo
     **Esperado:** Sidebar solo lectura. Sin botones "Nuevo" / "Editar" / "Eliminar". Anomalías oculta.

### TC-AUTH-004: Password incorrecta
- **Prioridad:** P1
- **Tipo:** SEC
- **Pasos:**
  1. `demo_admin` / `xxx` → Sign In
     **Esperado:** Error genérico ("Invalid credentials"). Sin redirección. Sin leak de existencia del usuario.

### TC-AUTH-005: Usuario inexistente (no-leak)
- **Prioridad:** P1
- **Tipo:** SEC
- **Pasos:**
  1. `nope_user` / cualquier pass → Sign In
     **Esperado:** Mismo error que TC-AUTH-004. Mensaje idéntico (timing y texto).

### TC-AUTH-006: Campos vacíos
- **Prioridad:** P2
- **Tipo:** UI
- **Pasos:**
  1. Submit sin user/pass
     **Esperado:** Validación cliente bloquea submit. Sin request a backend.

### TC-AUTH-007: Sesión expirada
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Login OK
  2. Borrar cookie / esperar `JWT exp`
  3. Recargar página protegida
     **Esperado:** Redirige a login.

### TC-AUTH-008: Logout
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Estando logueado, abrir menú user → Cerrar sesión
     **Esperado:** Cookie eliminada. Redirige a login.

### TC-AUTH-009: Acceso directo sin login
- **Prioridad:** P1
- **Tipo:** SEC
- **Pasos:**
  1. Incógnito → abrir `/dashboard`
     **Esperado:** Redirige a `/login`. Sin flicker de UI protegida.

---

## 2. Dashboard (TC-DASH-###)

### TC-DASH-001: KPIs cargan con datos seed
- **Prioridad:** P0
- **Tipo:** F
- **Trazabilidad:** §3
- **Precondiciones:** Login `demo_admin`. Seed cargada (6 empleados).
- **Pasos:**
  1. Navegar a `/dashboard`
     **Esperado:** KPIs visibles: Total empleados=6, Presentes hoy ≥0, Ausentes ≥0, Tarde ≥0. Sin errores en console.

### TC-DASH-002: Donut renderiza
- **Prioridad:** P1
- **Tipo:** UI
- **Pasos:**
  1. `/dashboard`
     **Esperado:** Gráfico donut con leyenda. Suma de segmentos = 100%.

### TC-DASH-003: Activity feed con últimas marcaciones
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. `/dashboard` → Activity feed sección
     **Esperado:** Muestra últimos N eventos del día. Foto thumbnail si `photo_path != null`.

### TC-DASH-004: SSE en vivo
- **Prioridad:** P1
- **Tipo:** INT
- **Pasos:**
  1. Mantener `/dashboard` abierto
  2. Empujar evento por mock_hikvision admin port (4401)
     **Esperado:** Feed actualiza sin recargar (< 5s).

### TC-DASH-005: Horas en TZ Caracas
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** §21.1, D-20
- **Pasos:**
  1. Verificar timestamps en activity feed
     **Esperado:** Sin desfase UTC. Hora local Caracas (UTC-4 sin DST).

### TC-DASH-006: Refresh manual sin flicker
- **Prioridad:** P2
- **Tipo:** UI
- **Pasos:**
  1. F5
     **Esperado:** Datos persisten. Sin pantalla blanca > 200ms.

---

## 3. Empleados (TC-EMP-###)

### TC-EMP-001: Listado seed completo
- **Prioridad:** P0
- **Tipo:** F
- **Trazabilidad:** §4.1
- **Pasos:**
  1. Login admin → `/employees`
     **Esperado:** 6 filas: EMP001..EMP006 con nombres y depts correctos.

### TC-EMP-002: Crear empleado válido
- **Prioridad:** P0
- **Tipo:** F
- **Trazabilidad:** §4.2
- **Pasos:**
  1. Click "Nuevo" → form
  2. Llenar: code=EMP999, nombre="Test QA", dept=Producción, salary=5000 cents
  3. Submit
     **Esperado:** Toast OK. Empleado en lista. Audit log entry creada.

### TC-EMP-003: Editar empleado
- **Prioridad:** P0
- **Tipo:** F
- **Pasos:**
  1. Editar EMP001 → cambiar nombre
  2. Save
     **Esperado:** Persist OK. Audit entry con before/after.

### TC-EMP-004: Búsqueda por nombre
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Buscar "Ana"
     **Esperado:** Filtra a 1 fila (Ana Pérez).

### TC-EMP-005: Filtro por departamento
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Filtro dept=Producción
     **Esperado:** 2 filas (EMP001, EMP002).

### TC-EMP-006: Conflicto 409 optimistic lock
- **Prioridad:** P1
- **Tipo:** INT
- **Trazabilidad:** §4.2, R14.1
- **Pasos:**
  1. Tab1 + Tab2 abren EMP001 edit
  2. Tab1 save
  3. Tab2 save (con `version` viejo)
     **Esperado:** Tab2 → 409. Toast "El empleado cambió; recarga".

### TC-EMP-007: Desactivar empleado
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. EMP006 → Desactivar → Confirmar
     **Esperado:** Estado=`inactive`. Filtro default no lo muestra.

### TC-EMP-008: Reactivar empleado
- **Prioridad:** P2
- **Tipo:** F
- **Pasos:**
  1. Filtro inactivos → reactivar EMP006
     **Esperado:** Vuelve a activos.

### TC-EMP-009: Validación código duplicado
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Crear con code=EMP001 (ya existe)
     **Esperado:** Error "código duplicado". Sin insert.

### TC-EMP-010: Validación campos vacíos
- **Prioridad:** P2
- **Tipo:** F
- **Pasos:**
  1. Submit con nombre vacío
     **Esperado:** Validación cliente bloquea.

### TC-EMP-011: Supervisor sin botón "Nuevo"
- **Prioridad:** P1
- **Tipo:** RBAC
- **Trazabilidad:** §4.3, §16
- **Pasos:**
  1. Login supervisor → `/employees`
     **Esperado:** Botón "Nuevo" oculto. POST `/api/v1/employees` directo → 403.

### TC-EMP-012: Viewer read-only
- **Prioridad:** P1
- **Tipo:** RBAC
- **Pasos:**
  1. Login viewer → `/employees`
     **Esperado:** Sin botones de mutación. PATCH directo → 403.

---

## 4. Departamentos (TC-DEP-###)

### TC-DEP-001: Listado deptos seed
- **Prioridad:** P0
- **Tipo:** F
- **Pasos:**
  1. Login admin → `/settings/departments`
     **Esperado:** 3 deptos: Producción, Administración, RRHH.

### TC-DEP-002: Crear depto día
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Crear: nombre="QA Dept", shift_type=day, start=08:00, end=17:00, lunch=fixed/60, ord=480
     **Esperado:** Persist. Aparece en lista.

### TC-DEP-003: Crear depto overnight
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** §21.7, R6.1
- **Pasos:**
  1. Crear: start=22:00, end=06:00, `is_overnight_shift=true`
     **Esperado:** Acepta start>end por flag. Persist.

### TC-DEP-004: Crear depto night
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** §21.6
- **Pasos:**
  1. Crear shift_type=night, ord=420
     **Esperado:** Persist. Reportes aplicarán +30% premium.

### TC-DEP-005: Eliminar con empleados (FK)
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Eliminar Producción (tiene EMP001, EMP002)
     **Esperado:** Bloqueado. Mensaje claro de FK.

### TC-DEP-006: Validación overnight sin flag
- **Prioridad:** P2
- **Tipo:** F
- **Pasos:**
  1. Crear start=22:00 end=06:00 con `is_overnight_shift=false`
     **Esperado:** Error de validación (start ≥ end same-day no permitido).

---

## 5. Timesheet / Marcaciones (TC-TS-###)

### TC-TS-001: Listado mes actual
- **Prioridad:** P0
- **Tipo:** F
- **Pasos:**
  1. `/timesheet`
     **Esperado:** Filas (empleado × día) del mes. Datos seed presentes.

### TC-TS-002: Filtro empleado
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Filtro EMP001
     **Esperado:** Solo filas de Ana Pérez.

### TC-TS-003: Columnas calculadas formato HH:MM
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Ver fila con OT
     **Esperado:** work_min, late_min, ot_min en HH:MM.

### TC-TS-004: Edición manual con justificación
- **Prioridad:** P0
- **Tipo:** F
- **Trazabilidad:** §6.2, R9.1
- **Pasos:**
  1. Admin edita entrada de un día
  2. Submit sin justificación
     **Esperado:** Error obligatorio.
  3. Llenar justificación → Submit
     **Esperado:** Persist. Audit entry. Recálculo dispara `RECOMPUTE_AFTER_EDIT`.

### TC-TS-005: Edit dispara recálculo
- **Prioridad:** P1
- **Tipo:** INT
- **Trazabilidad:** R9.2
- **Pasos:**
  1. Editar entrada de 08:00 → 09:00
     **Esperado:** `late_minutes` recalcula. Anomalía aparece en `/anomalies`.

### TC-TS-006: Conflict 409 en edit
- **Prioridad:** P1
- **Tipo:** INT
- **Pasos:**
  1. 2 tabs editan misma fila. Tab1 save. Tab2 save.
     **Esperado:** Tab2 → 409.

### TC-TS-007: Badge On time
- **Prioridad:** P1
- **Tipo:** UI
- **Pasos:**
  1. Fila con late=0
     **Esperado:** Badge "On time" verde.

### TC-TS-008: Badge Late
- **Prioridad:** P1
- **Tipo:** UI
- **Pasos:**
  1. Fila con late > 0
     **Esperado:** Badge "Late" amarillo/rojo según severidad.

### TC-TS-009: Paperclip evidencia leave
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** §6.3, R8.5
- **Pasos:**
  1. Hover fila con `leave_id != null`
  2. Click paperclip
     **Esperado:** Abre PDF/img en nueva tab.

### TC-TS-010: Cancelar leave (admin)
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** §6.3, R8.6
- **Pasos:**
  1. Admin → fila con leave → trash → confirmar
     **Esperado:** DELETE OK. Recálculo del rango. Audit entry.

---

## 6. Dispositivos (TC-DEV-###)

### TC-DEV-001: Listado seed
- **Prioridad:** P0
- **Tipo:** F
- **Pasos:**
  1. `/devices`
     **Esperado:** 2 devices (Entrada 4400, Salida 4401).

### TC-DEV-002: Crear device
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Crear: ip=192.168.1.50, port=80, user/pass
     **Esperado:** Persist. Pass encriptada AES-256-GCM en DB.

### TC-DEV-003: IP+puerto duplicado bloqueado
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Crear con ip=127.0.0.1 port=4400 (existe activo)
     **Esperado:** 409 / unique constraint.

### TC-DEV-004: Health check manual
- **Prioridad:** P1
- **Tipo:** INT
- **Pasos:**
  1. Click "Probar conexión" en dev-entry (mock 4400)
     **Esperado:** Estado actualiza. Mock responde 200 simulado.

### TC-DEV-005: Editar device sin reescribir pass
- **Prioridad:** P2
- **Tipo:** F
- **Pasos:**
  1. Editar nombre, dejar pass vacía → save
     **Esperado:** Pass NO se reescribe. Hash anterior persiste.

### TC-DEV-006: Desactivar device
- **Prioridad:** P2
- **Tipo:** F
- **Pasos:**
  1. Desactivar dev-exit → empujar evento desde mock 4401
     **Esperado:** Evento rechazado o ignorado. No daily_record.

### TC-DEV-007: Supervisor sin acceso
- **Prioridad:** P1
- **Tipo:** RBAC
- **Pasos:**
  1. Login supervisor → `/devices`
     **Esperado:** 403 o redirect. UI oculta entrada en sidebar.

---

## 7. Reportes (TC-RPT-###)

### TC-RPT-001: Reporte mensual con seed
- **Prioridad:** P0
- **Tipo:** F
- **Trazabilidad:** §8
- **Pasos:**
  1. `/reports` → mes actual
     **Esperado:** 6 filas con totales por empleado. Sueldos visibles ($30/$40/$50/$60/$70/$80).

### TC-RPT-002: Filtro mes anterior
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Selector mes → previo
     **Esperado:** Tabla recalcula. Datos del seed previo presentes.

### TC-RPT-003: Filtro depto
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Filtro Producción
     **Esperado:** 2 empleados.

### TC-RPT-004: Drill-down empleado
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Click EMP001
     **Esperado:** Dialog con detalle por día (work_min, late, OT, total).

### TC-RPT-005: Export XLSX
- **Prioridad:** P0
- **Tipo:** INT
- **Trazabilidad:** §8
- **Pasos:**
  1. Click "Export XLSX"
     **Esperado:** Descarga `.xlsx`. Abre en Excel/Numbers. Datos coinciden con UI.

### TC-RPT-006: Export PDF
- **Prioridad:** P1
- **Tipo:** INT
- **Pasos:**
  1. Click "Export PDF"
     **Esperado:** Descarga `.pdf`. Layout legible. Totales coinciden.

### TC-RPT-007: Mes sin datos
- **Prioridad:** P2
- **Tipo:** F
- **Pasos:**
  1. Selector mes futuro
     **Esperado:** Tabla vacía. Sin error.

### TC-RPT-008: Total a pagar = work + ot + night + rest - late
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** §21.12, R12
- **Pasos:**
  1. Verificar columna "Total" para EMP001
     **Esperado:** Coincide con fórmula `work_pay_cents + ot_pay_cents + night_premium_cents + rest_day_surcharge_cents - late_deduction_cents`.

### TC-RPT-009: Sueldos varían por empleado
- **Prioridad:** P0
- **Tipo:** F
- **Pasos:**
  1. Reporte mensual
     **Esperado:** EMP001..006 muestran $30..$80 distintos (no todos iguales).

---

## 8. Anomalías (TC-ANO-###)

### TC-ANO-001: Listado anomalías
- **Prioridad:** P0
- **Tipo:** F
- **Trazabilidad:** §9
- **Pasos:**
  1. Login supervisor → `/anomalies`
     **Esperado:** Lista con anomalías del seed (LATE, OT_CAP, etc.).

### TC-ANO-002: Filtro code=LATE
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Filtro code=LATE
     **Esperado:** Solo tardanzas.

### TC-ANO-003: Click "Ver" abre dialog
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Click "Ver" en una anomalía
     **Esperado:** Dialog con DailyRecord detail (anchor_date, work_min, late_min, etc.).

### TC-ANO-004: Paginación
- **Prioridad:** P2
- **Tipo:** F
- **Pasos:**
  1. Avanzar página
     **Esperado:** Página 2 con anomalías diferentes.

### TC-ANO-005: Viewer 403
- **Prioridad:** P1
- **Tipo:** RBAC
- **Pasos:**
  1. Login viewer → GET `/api/v1/anomalies` directo
     **Esperado:** 403. UI oculta sidebar.

---

## 9. Auditoría (TC-AUD-###)

### TC-AUD-001: Lista cronológica
- **Prioridad:** P0
- **Tipo:** F
- **Trazabilidad:** §10, R15
- **Pasos:**
  1. Login admin → `/audit`
     **Esperado:** Más reciente primero.

### TC-AUD-002: Mutaciones de TC-EMP-002 visibles
- **Prioridad:** P1
- **Tipo:** INT
- **Pasos:**
  1. Filtrar tabla=`employees`, action=`create`
     **Esperado:** Entry de TC-EMP-002 presente con before=null, after=snapshot.

### TC-AUD-003: Justificación visible para edits timesheet
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Buscar entry de TC-TS-004
     **Esperado:** Campo `justification` no vacío.

### TC-AUD-004: Read-only (sin edit/delete)
- **Prioridad:** P1
- **Tipo:** SEC
- **Trazabilidad:** R15.2
- **Pasos:**
  1. UI no muestra botones de mutación
  2. PATCH/DELETE directo a `/api/v1/audit/{id}` → 405 o 404
     **Esperado:** Inmutable.

### TC-AUD-005: Supervisor 403
- **Prioridad:** P1
- **Tipo:** RBAC
- **Pasos:**
  1. supervisor → `/audit` → 403.

---

## 10. Eventos crudos (TC-EVT-###)

### TC-EVT-001: Listado eventos
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Login viewer → `/events`
     **Esperado:** Lista con eventos seed/mock.

### TC-EVT-002: Thumbnail foto
- **Prioridad:** P2
- **Tipo:** UI
- **Pasos:**
  1. Fila con `photo_path`
     **Esperado:** Thumbnail pequeño carga.

### TC-EVT-003: Filtro `include_unknown`
- **Prioridad:** P2
- **Tipo:** F
- **Pasos:**
  1. Toggle include_unknown=true
     **Esperado:** Muestra rostros no reconocidos.

### TC-EVT-004: Click "Ver" foto grande
- **Prioridad:** P2
- **Tipo:** UI
- **Pasos:**
  1. Click "Ver"
     **Esperado:** Dialog con foto grande + metadata.

### TC-EVT-005: Push evento desde mock
- **Prioridad:** P1
- **Tipo:** INT
- **Pasos:**
  1. POST a `http://localhost:4401/admin/push` con XML EventNotificationAlert
     **Esperado:** Evento aparece en `/events` (refresh o SSE).

---

## 11. Reglas globales (TC-RUL-###)

### TC-RUL-001: Carga reglas seed
- **Prioridad:** P0
- **Tipo:** F
- **Trazabilidad:** §12
- **Pasos:**
  1. Admin → `/settings/rules`
     **Esperado:** Form con late_tolerance, early_tolerance, bonus_minutes, version.

### TC-RUL-002: Edit como admin
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. late_tolerance: 10 → 15. Save.
     **Esperado:** Toast OK. Persist. Audit entry.

### TC-RUL-003: Supervisor read-only
- **Prioridad:** P1
- **Tipo:** RBAC
- **Pasos:**
  1. Supervisor → `/settings/rules`
     **Esperado:** Form sin botón Save. Campos disabled.

### TC-RUL-004: Conflicto 409
- **Prioridad:** P1
- **Tipo:** INT
- **Trazabilidad:** R14.3
- **Pasos:**
  1. 2 tabs editan. Tab1 save. Tab2 save.
     **Esperado:** Tab2 → banner "otro admin acaba de cambiar".

### TC-RUL-005: Validación rango 0-60
- **Prioridad:** P2
- **Tipo:** F
- **Pasos:**
  1. bonus=70 → submit
     **Esperado:** Error rango.

---

## 12. Usuarios (TC-USR-###)

### TC-USR-001: Listado users seed
- **Prioridad:** P0
- **Tipo:** F
- **Pasos:**
  1. Admin → `/settings/users`
     **Esperado:** 6 users (3 e2e + 3 demo).

### TC-USR-002: Crear user
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Crear username único, pass, rol viewer
     **Esperado:** Persist. Pass hasheada argon2id.

### TC-USR-003: Cambiar rol
- **Prioridad:** P1
- **Tipo:** F
- **Pasos:**
  1. Cambiar rol de un user a admin
  2. Re-login con ese user
     **Esperado:** Sidebar refleja nuevo rol.

### TC-USR-004: Reset password
- **Prioridad:** P2
- **Tipo:** F
- **Pasos:**
  1. Reset pass de demo_viewer
     **Esperado:** Hash nueva. Login con pass nueva OK.

### TC-USR-005: Desactivar user
- **Prioridad:** P1
- **Tipo:** SEC
- **Pasos:**
  1. Desactivar user → intentar login
     **Esperado:** Login bloqueado.

---

## 13. RBAC matrix (TC-RBAC-###)

### TC-RBAC-001: GET /employees todos los roles
- **Prioridad:** P0
- **Tipo:** RBAC
- **Pasos:**
  1. curl con tokens admin/super/viewer → GET `/api/v1/employees`
     **Esperado:** Todos 200.
  2. Sin token → 401.

### TC-RBAC-002: POST /employees solo admin
- **Prioridad:** P0
- **Tipo:** RBAC
- **Pasos:**
  1. admin → 200/201. super → 403. viewer → 403. anon → 401.

### TC-RBAC-003: PATCH /rules solo admin
- **Prioridad:** P0
- **Tipo:** RBAC
- **Pasos:**
  1. admin → 200. super → 403. viewer → 403.

### TC-RBAC-004: GET /anomalies admin+super
- **Prioridad:** P1
- **Tipo:** RBAC
- **Pasos:**
  1. admin → 200. super → 200. viewer → 403.

### TC-RBAC-005: DELETE /leaves solo admin
- **Prioridad:** P1
- **Tipo:** RBAC
- **Pasos:**
  1. admin → 200. super → 403.

### TC-RBAC-006: GET /audit solo admin
- **Prioridad:** P1
- **Tipo:** RBAC
- **Pasos:**
  1. admin → 200. super → 403. viewer → 403.

### TC-RBAC-007: PATCH /timesheet edits admin+super
- **Prioridad:** P1
- **Tipo:** RBAC
- **Pasos:**
  1. admin → 200. super → 200. viewer → 403.

### TC-RBAC-008: POST /leaves admin+super
- **Prioridad:** P1
- **Tipo:** RBAC
- **Pasos:**
  1. admin → 201. super → 201. viewer → 403.

### TC-RBAC-009: GET /reports todos
- **Prioridad:** P1
- **Tipo:** RBAC
- **Pasos:**
  1. Todos → 200. anon → 401.

### TC-RBAC-010: JWT manipulado
- **Prioridad:** P0
- **Tipo:** SEC
- **Pasos:**
  1. Modificar payload del JWT (rol viewer→admin)
  2. POST /employees
     **Esperado:** 401 (firma inválida).

---

## 14. Reglas de tolerancia (TC-RULE-###)

> Setup base: dept turno 08:00–17:00, lunch fixed 60min, ord=480.

### TC-RULE-001: late=0 dentro tolerancia
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** §21.2 R1.1
- **Setup:** late_tol=10, bonus=0
- **Pasos:**
  1. Inyectar evento entry@08:05
     **Esperado:** `late_minutes=0`. Badge On time.

### TC-RULE-002: late=11 supera tolerancia
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** R1.2
- **Setup:** late_tol=10, bonus=0
- **Pasos:**
  1. entry@08:11
     **Esperado:** `late_minutes=11`. Badge Late.

### TC-RULE-003: bono extiende gracia
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** R1.3, D-17
- **Setup:** late_tol=10, bonus=5
- **Pasos:**
  1. entry@08:14
     **Esperado:** `late_minutes=0` (10+5=15 grace).

### TC-RULE-004: bono respeta umbral final
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** R1.4
- **Setup:** late_tol=10, bonus=5
- **Pasos:**
  1. entry@08:16
     **Esperado:** `late_minutes=16`.

### TC-RULE-005: early dentro tolerancia
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** R1.6
- **Setup:** early_tol=10, bonus=0
- **Pasos:**
  1. exit@16:55
     **Esperado:** `early_departure_minutes=0`.

### TC-RULE-006: early supera tolerancia
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** R1.7
- **Pasos:**
  1. exit@16:45
     **Esperado:** `early_departure_minutes=15`.

### TC-RULE-007: ventana excluye eventos fuera
- **Prioridad:** P2
- **Tipo:** F
- **Trazabilidad:** §21.3 R2.1, D-20
- **Setup:** late_tol=10, bonus=0 → ventana [07:50, 17:10]
- **Pasos:**
  1. Inyectar entry@07:30 + entry@07:55
     **Esperado:** Anchor entry = 07:55. 07:30 ignorado.

### TC-RULE-008: unknown face raise anomalía
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** R2.5
- **Pasos:**
  1. Inyectar entry@08:00 con `is_unknown=1`
     **Esperado:** No anchor. Anomalía `UNKNOWN_FACE_IN_WINDOW`.

---

## 15. Money / LOTTT (TC-MNY-###)

> Verificable por reportes/drilldown o tests `cargo nextest run -p cronometrix_api reports::money`.

### TC-MNY-001: work_pay full day
- **Prioridad:** P0
- **Tipo:** F
- **Trazabilidad:** §21.12 R12.1
- **Inputs:** work=480, base=100 cents, ord=480
- **Esperado:** work_pay = 100.

### TC-MNY-002: work_pay half day
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** R12.2
- **Inputs:** work=240, base=100, ord=480
- **Esperado:** work_pay = 50.

### TC-MNY-003: ot_pay +50% (Art. 118)
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** R12.3
- **Inputs:** ot=60, base=100, ord=480
- **Esperado:** ot_pay = 18 (`60×100×150/(100×480) = 18.75 → 18 trunc`).

### TC-MNY-004: night_premium +30% additive (Art. 117)
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** R12.4
- **Inputs:** work=480, base=100, ord=480
- **Esperado:** night = 30. Additivo sobre work_pay.

### TC-MNY-005: rest_day +50% (Art. 120)
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** R12.5
- **Inputs:** work=480, base=100, ord=480
- **Esperado:** rest = 50.

### TC-MNY-006: late_deduction prorrateado
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** R12.6
- **Inputs:** late=15, base=100, ord=480
- **Esperado:** dedución = 3 (`15×100/480 = 3.125 → 3 trunc`).

### TC-MNY-007: total_a_pagar composición
- **Prioridad:** P1
- **Tipo:** F
- **Inputs:** work=50, ot=18, night=30, rest=0, late=3
- **Esperado:** total = 95.

### TC-MNY-008: misconfig ord=0 sin div/0
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** R12.7
- **Inputs:** ord=0
- **Esperado:** Todas las funciones → 0. Sin panic.

### TC-MNY-009: night premium gates per-day shift
- **Prioridad:** P1
- **Tipo:** F
- **Trazabilidad:** §21.6 R5.4 (W-6)
- **Setup:** dept.shift_type=day, daily_record.shift_type=night (caso edge)
- **Esperado:** Premium night aplica. Gate usa `daily_records.shift_type` no `departments.shift_type`.

### TC-MNY-010: seed reportes muestra $30..$80 distintos
- **Prioridad:** P0
- **Tipo:** REG
- **Pasos:**
  1. Reporte mensual
     **Esperado:** Columna sueldo: 6 valores únicos $30/$40/$50/$60/$70/$80. No todos iguales.

---

## 16. Concurrencia (TC-CON-###)

### TC-CON-001: 409 PATCH version stale
- **Prioridad:** P1
- **Tipo:** INT
- **Trazabilidad:** R14.1
- Ya cubierto en TC-EMP-006, TC-TS-006, TC-RUL-004.

### TC-CON-002: 409 DELETE version stale
- **Prioridad:** P1
- **Tipo:** INT
- **Trazabilidad:** R14.2
- **Pasos:**
  1. GET /leaves/{id} → version=N
  2. Otro user PATCH → version=N+1
  3. DELETE /leaves/{id}?version=N
     **Esperado:** 409.

### TC-CON-003: webhook bursts orden estable
- **Prioridad:** P2
- **Tipo:** PERF
- **Pasos:**
  1. Empujar 50 eventos al mock en < 1s
     **Esperado:** Todos procesados. Sin race. dedup 30s aplica.

---

## 17. Seguridad (TC-SEC-###)

### TC-SEC-001: /__test_reset 404 en demo
- **Prioridad:** P0
- **Tipo:** SEC
- **Trazabilidad:** §17, T-09-15
- **Pasos:**
  1. POST `https://demo-api.cronometrix.app/api/v1/__test_reset`
     **Esperado:** 404 (gated por env `CRONOMETRIX_E2E`).

### TC-SEC-002: license bypass sin E2E aborta
- **Prioridad:** P1
- **Tipo:** SEC
- **Pasos:**
  1. Set `CRONOMETRIX_LICENSE_BYPASS=true` sin `CRONOMETRIX_E2E=true`
  2. Iniciar binario
     **Esperado:** Exit code 2. Locked test `bypass_without_e2e_aborts_with_code_2`.

### TC-SEC-003: SQL injection en filtros
- **Prioridad:** P1
- **Tipo:** SEC
- **Pasos:**
  1. Buscar empleado con `'; DROP TABLE users;--`
     **Esperado:** Tratado como literal. Sin SQL execution. Tabla users intacta.

### TC-SEC-004: XSS en nombres
- **Prioridad:** P1
- **Tipo:** SEC
- **Pasos:**
  1. Crear empleado con nombre `<script>alert(1)</script>`
     **Esperado:** Render escapado. Sin alert.

### TC-SEC-005: CORS origen no permitido
- **Prioridad:** P2
- **Tipo:** SEC
- **Pasos:**
  1. fetch desde `evil.com` a la API
     **Esperado:** Bloqueado por CORS.

### TC-SEC-006: device password encrypt at rest
- **Prioridad:** P1
- **Tipo:** SEC
- **Pasos:**
  1. Crear device con pass conocida
  2. SELECT encrypted_password FROM devices WHERE id=...
     **Esperado:** Valor distinto del plaintext. Decrypt con `DEVICE_CREDS_KEY` recupera plaintext.

---

## Trazabilidad inversa (regla → TC)

| Regla §21 | TCs |
|---|---|
| §21.2 Tolerancia | TC-RULE-001..006, TC-TS-007/008 |
| §21.3 Ventana | TC-RULE-007, TC-RULE-008 |
| §21.4 Lunch | (extender — TC-RULE-009..013 pendiente) |
| §21.5 Overtime caps | TC-MNY-003, TC-ANO-001 |
| §21.6 Night/mixed | TC-MNY-004, TC-MNY-009, TC-DEP-004 |
| §21.7 Overnight | TC-DEP-003 |
| §21.8 Rest day | TC-MNY-005 |
| §21.9 Anomaly codes | TC-ANO-001..005, TC-RULE-008 |
| §21.10 Leaves | TC-TS-009/010 |
| §21.11 Edit + audit | TC-TS-004/005, TC-AUD-002/003 |
| §21.12 Money formulas | TC-MNY-001..010 |
| §21.13 RBAC backend | TC-RBAC-001..010 |
| §21.14 Concurrency | TC-CON-001..003 |
| §21.15 Audit immutable | TC-AUD-004 |

---

## Estado de ejecución (template por release)

| Release | TC totales | Pass | Fail | Blocked | NotRun | Pass% |
|---|---|---|---|---|---|---|
| v0.1.0-demo | 111 | – | – | – | 111 | – |

Tracker per-TC en `docs/qa/runs/<release>.md` (crear al ejecutar).
