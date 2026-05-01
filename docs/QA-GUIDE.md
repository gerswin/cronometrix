# Cronometrix — Guía de QA

Plan de pruebas funcional para validar el demo público (`app-demo.cronometrix.app`) y futuras instalaciones on-premise. Cada sección lista módulo, precondiciones, casos a probar (golden + edge), y resultado esperado.

---

## 0. Entorno y credenciales

| Dato | Valor |
|---|---|
| Web | https://app-demo.cronometrix.app |
| API | https://demo-api.cronometrix.app |
| Health | https://demo-api.cronometrix.app/api/v1/health |
| TZ | America/Caracas (sin DST) |
| Idioma UI | Español (login en inglés — Addendum D-19) |
| Moneda demo | USD |

Usuarios demo (password compartida: `dSQBALuQgXWZp6Oo`):

| Usuario | Rol | Permisos |
|---|---|---|
| `demo_admin` | admin | Todo |
| `demo_super` | supervisor | Consulta operativa + anomalías |
| `demo_viewer` | viewer | Solo lectura |

Empleados sembrados (sueldos $30–$80 USD para validar reportes):

| Código | Nombre | Departamento | Sueldo |
|---|---|---|---|
| EMP001 | Ana Pérez | Producción | $30 |
| EMP002 | Luis García | Producción | $40 |
| EMP003 | María López | Administración | $50 |
| EMP004 | Pedro Ramírez | Administración | $60 |
| EMP005 | Carmen Silva | RRHH | $70 |
| EMP006 | José Hernández | RRHH | $80 |

Dispositivos sembrados: `Entrada Principal` (127.0.0.1:4400, mock), `Salida Principal` (127.0.0.1:4401, mock).

**Nota demo:** DB en `/tmp/cronometrix.db`, ephemera. Reinicio del contenedor → re-seed limpio.

---

## 1. Smoke pre-prueba (60 segundos)

Ejecutar antes de cada sesión de QA:

1. `curl -fsS https://demo-api.cronometrix.app/api/v1/health` → 200 `{"status":"ok"}`
2. Cargar https://app-demo.cronometrix.app → pantalla de login (inglés)
3. Login `demo_admin` → dashboard carga, KPIs no vacíos
4. Logout → vuelve a login
5. DevTools Network: ningún request HTTP 5xx, CORS OK

Si alguno falla → detener QA, abrir issue antes de continuar.

---

## 2. Login y autenticación

**Precondiciones:** logout previo (cookie limpia).

| Caso | Pasos | Esperado |
|---|---|---|
| Login admin OK | user `demo_admin` / pass correcta → Sign In | Redirige a `/dashboard`, sidebar muestra todas las opciones |
| Login supervisor OK | `demo_super` | Sidebar oculta Settings → Usuarios, Reglas; Anomalías visible |
| Login viewer OK | `demo_viewer` | Sidebar solo lectura, sin botones de crear/editar |
| Password incorrecta | `demo_admin` / `xxx` | Toast/banner error, no redirige |
| Usuario inexistente | `nope` / cualquier | Mismo error genérico (no leak) |
| Campos vacíos | submit sin user/pass | Validación cliente bloquea submit |
| Sesión expirada | esperar > JWT exp o borrar cookie → recargar | Redirige a login |
| Logout | menú user → Cerrar sesión | Cookie eliminada, vuelve a login |
| Acceso directo sin login | abrir `/dashboard` en incógnito | Redirige a login |

---

## 3. Dashboard

**Acceso:** todos los roles.

| Caso | Esperado |
|---|---|
| KPIs cargan | Total empleados, presentes hoy, ausentes, tarde — números coherentes con seed |
| Gráfico donut | Render sin errores, leyenda visible |
| Activity feed | Últimas marcaciones del día, foto thumbnail si disponible |
| SSE en vivo | Empujar evento por mock (ver §11) → feed actualiza sin recargar |
| TZ | Todas las horas en `America/Caracas` (sin desfase UTC) |
| Refresh manual | F5 → datos persisten, no flicker |

---

## 4. Empleados (`/employees`)

**Roles:** admin (CRUD), supervisor (lectura), viewer (lectura).

### 4.1 Listado
| Caso | Esperado |
|---|---|
| Tabla carga 6 empleados | Nombres + códigos + departamentos correctos |
| Búsqueda por nombre | Filtra en vivo |
| Filtro por departamento | Reduce a 2 empleados por dept |
| Filtro por estado (activo/inactivo) | Default = activos |
| Paginación | Funciona con datasets > página (sembrar más si hace falta) |
| Ordenar por columna | Asc/desc |

### 4.2 CRUD (admin)
| Caso | Esperado |
|---|---|
| Crear empleado | Form valida, código único, persist, audit log entry |
| Editar empleado | Cambios persisten, version optimistic lock |
| Conflicto 409 | Editar mismo empleado en 2 tabs → segundo save 409 con toast |
| Desactivar | Confirmación dialog → estado `inactive`, no en filtro default |
| Reactivar | Filtro inactivos → reactivar → vuelve a activos |
| Validaciones | Código vacío, nombre vacío, sueldo negativo → bloqueado |

### 4.3 RBAC
| Caso | Esperado |
|---|---|
| Supervisor ve botón "Nuevo" | NO visible |
| Viewer abre `/employees/new` directo | 403 o redirige |

---

## 5. Departamentos (`/settings/departments`)

**Roles:** admin.

| Caso | Esperado |
|---|---|
| Lista 3 deptos seed | Producción, Administración, RRHH |
| Crear depto | Nombre, turno (start/end), tipo (day/night/overnight), tolerancia lunch |
| Validar turno overnight | start > end permitido si `is_overnight_shift = true` |
| Editar depto | Cambios reflejan en empleados asignados |
| Eliminar con empleados | Bloqueado (FK), mensaje claro |
| Eliminar sin empleados | OK |

---

## 6. Marcaciones / Timesheet (`/timesheet`)

**Roles:** todos (lectura); admin/supervisor (edición).

### 6.1 Listado
| Caso | Esperado |
|---|---|
| Tabla del mes actual | Filas por (empleado × día) con datos seed |
| Filtro empleado | Reduce a sus filas |
| Filtro rango fechas | Solo días dentro del rango |
| Columnas calculadas | Entrada, salida, work_min, late_min, OT_min en HH:MM |
| Badge estado | On time / Late / Early / Half day / Absent / OT |
| TZ | Horas en Caracas |

### 6.2 Edición manual (admin)
| Caso | Esperado |
|---|---|
| Editar entrada/salida | Genera audit log con justificación obligatoria |
| Sin justificación | Submit bloqueado |
| Conflicto version | 409 toast |
| Recálculo | Tras editar, late/OT recalculan automáticamente |

### 6.3 Novedades / Leaves (Group B del plan)
| Caso | Esperado |
|---|---|
| Fila con `leave_id` | Iconos paperclip + trash (admin) |
| Click paperclip | Abre evidencia en nueva tab (PDF/img) |
| Click trash (admin) | Dialog confirmar → DELETE → fila vuelve a estado normal |
| Trash en supervisor | NO visible |
| 409 al cancelar | Toast "Esta novedad cambió; recarga" |

---

## 7. Dispositivos (`/devices`)

**Roles:** admin.

| Caso | Esperado |
|---|---|
| Lista 2 devices seed | Entrada (4400), Salida (4401) |
| Estado conexión | offline (mock no responde ping real) |
| Crear device | IP, puerto, user, pass — pass encriptada AES-256-GCM en DB |
| IP+puerto duplicado | Bloqueado por unique index parcial (status=active) |
| Health check manual | Botón → tira request, muestra estado |
| Editar device | Pass solo se reescribe si campo no vacío |
| Desactivar device | No recibe webhooks |

---

## 8. Reportes (`/reports`)

**Roles:** todos (lectura).

| Caso | Esperado |
|---|---|
| Reporte mensual | Tabla con totales por empleado: días, horas, tarde, OT, sueldo prorrateado |
| Sueldos visibles | $30/$40/$50/$60/$70/$80 según seed |
| Filtro mes | Selector mes → recalcula |
| Filtro depto | Reduce a empleados del depto |
| Drill-down | Click empleado → detalle por día |
| Exportar XLSX | Descarga `.xlsx`, abre en Excel/Numbers, datos correctos |
| Exportar PDF | Descarga `.pdf`, layout legible, totales coinciden |
| Mes sin datos | Tabla vacía, no error |

---

## 9. Anomalías (`/anomalies`) — Group C plan

**Roles:** admin, supervisor.

| Caso | Esperado |
|---|---|
| Listado con filtros | code, employee, fechas |
| Filtro `code=LATE` | Solo tardanzas |
| Click "Ver" | Dialog con detalle del daily_record |
| Paginación | Avanza/retrocede |
| Viewer accede directo | Sidebar oculta entrada; `/anomalies` directo → 403 |

---

## 10. Auditoría (`/audit`)

**Roles:** admin.

| Caso | Esperado |
|---|---|
| Lista cronológica | Más reciente primero |
| Filtros | Acción (create/update/delete), tabla, usuario, rango |
| Cada mutación generó entrada | Crear empleado en §4 → row aquí |
| Justificación visible | Para edits de timesheet |
| Inmutabilidad | Sin botón de editar/borrar (read-only) |

---

## 11. Eventos crudos (`/events`) — Group D plan

**Roles:** todos.

| Caso | Esperado |
|---|---|
| Listado eventos | Marcaciones del mock + reales |
| Thumbnail foto | Carga si `photo_path != null` |
| Filtro `include_unknown` | Muestra rostros no reconocidos |
| Click "Ver" | Dialog con foto grande + metadata |
| Empuje desde mock | `POST` al admin port 4401 con XML EventNotificationAlert → fila aparece (refresh o SSE) |

**Comando para empujar evento de prueba** (SSH al server):
```sh
sudo docker exec dokku-cronometrix-api curl -X POST \
  http://localhost:4401/admin/push \
  -H "Content-Type: application/xml" \
  --data-binary @/path/to/event.xml
```

---

## 12. Reglas globales (`/settings/rules`) — Group A plan

**Roles:** admin (CRUD), otros (read-only).

| Caso | Esperado |
|---|---|
| Carga reglas seed | Tolerance late/early, bonus minutes |
| Edit como admin | Save → toast OK → valor persiste |
| Edit como supervisor | Form read-only, sin botón Save |
| Conflicto 409 | 2 tabs editando → segundo recibe banner |
| Validación rango | Valores 0–60, fuera bloquea |

---

## 13. Usuarios (`/settings/users`)

**Roles:** admin.

| Caso | Esperado |
|---|---|
| Lista 6 users seed | 3 e2e + 3 demo |
| Crear user | username único, password hasheada argon2id, rol válido |
| Cambiar rol | Refleja en sidebar tras re-login |
| Reset password | Genera nueva hash, sesiones previas no invalidadas (nota) |
| Desactivar | No puede login |

---

## 14. Tenant info (`/settings/tenant-info`)

**Roles:** admin.

| Caso | Esperado |
|---|---|
| Edit nombre empresa, RIF, dirección | Persiste, aparece en reportes PDF |
| Logo upload | Si soportado, render en PDF/header |

---

## 15. Enrolamiento facial (`/enrollment`)

**Roles:** admin.

**Nota demo:** mock_hikvision NO procesa enrolment real; flow validable hasta el dispatch.

| Caso | Esperado |
|---|---|
| Lista empleados sin foto | EMP001..006 |
| Capturar foto (webcam) | Permiso navegador → preview |
| Subir a device | Request a mock → 200 OK simulado |
| Estado sync | Per-device columna |

---

## 16. RBAC matrix (cross-cut)

Repetir en cada módulo:

| Acción | admin | supervisor | viewer |
|---|---|---|---|
| Ver listas | ✅ | ✅ | ✅ |
| Crear/editar empleados | ✅ | ❌ | ❌ |
| Editar timesheet | ✅ | ✅ | ❌ |
| Cancelar leave | ✅ | ❌ | ❌ |
| Ver auditoría | ✅ | ❌ | ❌ |
| Editar reglas | ✅ | ❌ | ❌ |
| Ver anomalías | ✅ | ✅ | ❌ |
| CRUD usuarios | ✅ | ❌ | ❌ |
| CRUD dispositivos | ✅ | ❌ | ❌ |
| Exportar reportes | ✅ | ✅ | ✅ |

Validar tanto en UI (botones ocultos) como en API (curl directo → 401/403).

---

## 17. Seguridad y bordes

| Caso | Esperado |
|---|---|
| `/api/v1/__test_reset` | 404 (bloqueado en demo, gated por env) |
| Bypass licencia sin E2E | Backend abort exit 2 (locked test) |
| CORS desde origin no permitido | Bloqueado |
| Inyección SQL en filtros | Bind params → safe |
| XSS en nombres empleados | Render escapado |
| JWT manipulado | 401 |
| Rate limiting login | (Verificar si activo en demo) |

---

## 18. Performance / UX

| Caso | Esperado |
|---|---|
| Listado 1000+ empleados | Virtualización TanStack — scroll fluido |
| Reporte mes con 6×30 = 180 filas | < 1s render |
| Export XLSX 1k filas | < 5s |
| Carga inicial dashboard | < 2s en red local, < 4s vía Cloudflare |
| Mobile / responsive | Sidebar colapsa, tablas con scroll horizontal |

---

## 19. Datos de prueba (escenarios cubiertos por seed-reports-data.py)

El seed siembra el mes actual + mes previo con:

- **on_time** (30%): entrada/salida dentro de tolerancia
- **late_within_tol** (10%): tarde pero < tolerance, no penaliza
- **late_moderate** (8%): tarde > tolerance, < 30 min
- **late_severe** (4%): tarde > 30 min
- **early_dep_within_tol**: salida temprana < tolerance
- **early_dep_severe**: salida temprana fuerte
- **overtime_short**: OT 30–60 min
- **overtime_long**: OT > 60 min
- **half_day**: solo entrada o solo salida
- **daily_cap_breach**: > 12h trabajadas (flag anomalía)
- **absent**: sin marcación
- **absent_with_leave**: ausente con novedad justificada

Validar cada escenario aparece en Reportes con badge correcto.

---

## 20. Bug reporting

Al reportar:

1. **URL exacta** y **rol** logueado
2. **Pasos reproducir** (numerados)
3. **Esperado vs Actual**
4. **Screenshot/video**
5. **DevTools Network** (request fallido + response body)
6. **DevTools Console** (errores JS)
7. **Hora aproximada** (Caracas TZ) — para correlacionar con logs server

Logs server (admin):
```sh
ssh gerswin@192.168.0.44 'sudo docker exec dokku dokku logs cronometrix-api --tail 200'
```

---

## 21. Reglas de negocio (motor de cálculo + reportes)

Marco legal: **Venezuela / LOTTT** (Ley Orgánica del Trabajo). TZ `America/Caracas` sin DST. Toda regla siguiente es validable en el módulo Reportes (§8) o Anomalías (§9) tras inyectar marcaciones.

### 21.1 Constantes y configuración (origen de las reglas)

| Parámetro | Default | Origen | Editable en |
|---|---|---|---|
| `ordinary_daily_minutes` (jornada diurna) | 480 (8h) | LOTTT Art. 173 | Departments |
| `ordinary_daily_minutes` (jornada nocturna) | 420 (7h) | LOTTT Art. 117 | Departments |
| `ordinary_daily_minutes` (jornada mixta) | 450 (7.5h) | LOTTT Art. 173 | Departments |
| `late_arrival_tolerance_min` | 0–60 | RULE-01 | Settings → Reglas |
| `early_departure_tolerance_min` | 0–60 | RULE-01 | Settings → Reglas |
| `bonus_minutes` (gracia adicional) | 0–60 | RULE-02 / D-17 | Settings → Reglas |
| `lunch_mode` | `fixed` \| `punch` | D-09 | Departments |
| `lunch_duration_min` | numérico | D-09 | Departments |
| `is_overnight_shift` | bool | D-06 | Departments |
| `shift_type` | `day` \| `night` \| `mixed` | D-11 | Departments |
| Día de descanso | Sáb + Dom | D-12 v1 | (configurable futuro) |
| OT cap diario (anomalía) | 120 min | LOTTT Art. 178 | Hardcoded |
| OT cap semanal (anomalía) | 600 min | LOTTT Art. 178 | Hardcoded |
| OT cap anual (anomalía) | 6000 min | LOTTT Art. 178 | Hardcoded |

### 21.2 Reglas de tolerancia (RULE-01 + RULE-02)

**Fórmula efectiva** (D-17):
- Umbral de tarde = `shift_start + late_tolerance + bonus`
- Umbral de salida temprana = `shift_end - early_tolerance - bonus`
- Bono = gracia *adicional* a la tolerancia (no la reemplaza)

**Casos:**

| # | Setup | Marcación | `late_min` esperado | Badge |
|---|---|---|---|---|
| R1.1 | turno 08:00, tol=10, bono=0 | entrada 08:05 | 0 (dentro tol) | On time |
| R1.2 | turno 08:00, tol=10, bono=0 | entrada 08:11 | 11 | Late |
| R1.3 | turno 08:00, tol=10, bono=5 | entrada 08:14 | 0 (10+5 grace) | On time |
| R1.4 | turno 08:00, tol=10, bono=5 | entrada 08:16 | 16 | Late |
| R1.5 | turno 08:00, tol=10 | entrada 07:55 | 0 | On time |
| R1.6 (early) | salida 17:00, tol=10, bono=0 | salida 16:55 | early=0 | On time |
| R1.7 (early) | salida 17:00, tol=10, bono=0 | salida 16:45 | early=15 | Early |
| R1.8 (early) | salida 17:00, tol=10, bono=5 | salida 16:46 | early=0 | On time |

**Esperado backend:** `late_minutes` = `max(0, entry - nominal_start) / 60` cuando supera tolerancia+bono.

### 21.3 Ventana de agregación (D-20)

**Regla:** `window = [shift_start - late_tol - bonus, shift_end + early_tol + bonus]`. Eventos fuera de esta ventana se ignoran para anchor.

| # | Setup | Eventos | Resultado |
|---|---|---|---|
| R2.1 | turno 08:00–17:00, tol=10, bono=0 | entrada 07:30 + entrada 07:55 | Anchor = 07:55 (07:30 fuera de ventana 07:50) |
| R2.2 | mismo | entrada 07:55 + entrada 08:10 (multi-device) | Anchor = 07:55 (first entry) |
| R2.3 | mismo | salida 16:55 + salida 17:30 | Anchor exit = 17:30 (last exit, dentro de 17:10? **NO** — fuera ventana → 16:55 vence) |
| R2.4 | dedup 30s | dos entradas en 0–29s | una sola contada (Phase 2) |
| R2.5 | unknown face en ventana | entrada 08:00 con `is_unknown=1` | Excluido de anchor + emite `UNKNOWN_FACE_IN_WINDOW` |

### 21.4 Almuerzo / Lunch (D-09)

**Modo `fixed`:** descuenta `lunch_duration_min` siempre (sin importar marcaciones).

| # | Setup | Resultado |
|---|---|---|
| R3.1 | fixed, lunch=60, work raw=8h | work_min = 480 - 60 = 420 |
| R3.2 | fixed, lunch=0 | sin descuento |

**Modo `punch`:** busca par de eventos exit→entry entre entrada y salida del turno; descuenta delta. Si falta uno → fallback a `lunch_duration_min` + emite `LUNCH_PUNCH_MISSING`.

| # | Setup | Eventos almuerzo | Resultado |
|---|---|---|---|
| R3.3 | punch, lunch_min=60 | exit 12:00 + entry 12:45 | descuenta 45 min |
| R3.4 | punch | exit 12:00 sin entry | descuenta 60 (fallback) + anomalía `LUNCH_PUNCH_MISSING` |
| R3.5 | punch | sin pares | descuenta 60 (fallback) + anomalía |

### 21.5 Overtime / Horas extra (CALC + LOTTT Art. 118)

**Definición:** `overtime_minutes = max(0, work_minutes - ordinary_daily_minutes)`.

| # | Setup | work_min | OT esperado | Anomalía |
|---|---|---|---|---|
| R4.1 | dept ord=480 | 480 | 0 | — |
| R4.2 | dept ord=480 | 540 | 60 | — |
| R4.3 | dept ord=480 | 605 | 125 | `OT_CAP_EXCEEDED_DAILY` (>120 OT/día) |
| R4.4 | semana acumula 550 OT, hoy +60 | — | 60 | `OT_CAP_EXCEEDED_WEEKLY` (550+60=610 > 600) |
| R4.5 | anual acumula 5990, hoy +60 | — | 60 | `OT_CAP_EXCEEDED_ANNUAL` (>6000) |

**Importante:** caps son anomalías *informativas* — los minutos se atribuyen igual, no se rechazan. Supervisor revisa.

### 21.6 Turno nocturno y mixto (D-11, LOTTT Art. 117)

| # | Setup | Esperado |
|---|---|---|
| R5.1 | dept `shift_type=night`, ord=420 | 7h = jornada ordinaria |
| R5.2 | dept `shift_type=mixed`, ord=450 | 7.5h = ordinaria |
| R5.3 | reportes en turno night | aplica `+30%` premium ADITIVO sobre work_pay (Art. 117) |
| R5.4 | gating | premium nocturno usa `daily_records.shift_type` (per-day actual), NO `departments.shift_type` (W-6) |

### 21.7 Turnos overnight (D-05/D-06)

**Regla:** `is_overnight_shift=true` → ventana cruza medianoche. **Anchor date = start date** (no end date).

| # | Setup | Marcación | Anchor date |
|---|---|---|---|
| R6.1 | turno 22:00–06:00, overnight | entrada Lun 22:00, salida Mar 06:00 | DailyRecord en **Lun** |
| R6.2 | sin overnight flag, turno 22:00–06:00 | mismas | shift_window degenera (start>end same day) |
| R6.3 | DST gap (no aplica VE) | — | `OVERNIGHT_INFERENCE_AMBIGUOUS` (dead code en VE) |

### 21.8 Día de descanso / Domingo (D-12, LOTTT Art. 120)

**v1 default:** rest days = Sábado + Domingo.

| # | Setup | Esperado |
|---|---|---|
| R7.1 | empleado trabaja Domingo | `is_rest_day_worked=1`, reporte aplica +50% surcharge sobre work_pay |
| R7.2 | empleado trabaja Sábado | mismo |
| R7.3 | empleado trabaja Lunes | sin surcharge |

### 21.9 Anomalías — códigos completos (D-18)

| Código | Disparo | Sección demo |
|---|---|---|
| `MISSING_ENTRY` | hay salida sin entrada en ventana | R10.1 |
| `MISSING_EXIT` | hay entrada sin salida en ventana | R10.2 |
| `UNKNOWN_FACE_IN_WINDOW` | evento `is_unknown=1` dentro ventana | R2.5 |
| `LUNCH_PUNCH_MISSING` | modo punch sin par completo | R3.4 |
| `OT_CAP_EXCEEDED_DAILY` | OT día > 120 min | R4.3 |
| `OT_CAP_EXCEEDED_WEEKLY` | OT semana > 600 min | R4.4 |
| `OT_CAP_EXCEEDED_ANNUAL` | OT año > 6000 min | R4.5 |
| `EVENTS_ON_LEAVE_DAY` | hay eventos en día con leave full-day | R8.3 |
| `RECOMPUTE_AFTER_EDIT` | edit manual disparó recálculo | R11.x |
| `OVERNIGHT_INFERENCE_AMBIGUOUS` | resolución DST falló (dead code VE) | n/a |

Validar que cada anomalía aparece en `/anomalies` con su `code` correcto.

### 21.10 Novedades / Leaves (LEAVE-01..04, D-14)

**Tipos:** `medical | vacation | unpaid | manual`. **v1 = full-day only** (medio día se hace vía edición Phase 4).

| # | Caso | Esperado |
|---|---|---|
| R8.1 | leave `medical` cubre día | `work_min=0`, no penaliza, badge "Justificada" |
| R8.2 | leave `vacation` rango Lun–Vie | 5 daily_records con leave overlay |
| R8.3 | leave `vacation` + hay eventos ese día | anomalía `EVENTS_ON_LEAVE_DAY` |
| R8.4 | leave `unpaid` | excluido del pago en reporte |
| R8.5 | leave con evidencia (PDF) | botón paperclip abre archivo |
| R8.6 | cancelar leave (admin) | recálculo automático del rango |
| R8.7 | leave fechas inválidas (`from > to`) | API 400 |
| R8.8 | versión obsoleta DELETE | 409 |

### 21.11 Edición manual + Auditoría (Phase 4)

| # | Caso | Esperado |
|---|---|---|
| R9.1 | admin edita entrada de timesheet | requiere justificación obligatoria, audit_log entry creada |
| R9.2 | edit dispara recálculo | anomalía `RECOMPUTE_AFTER_EDIT`, late/OT/lunch recalculan |
| R9.3 | edit con `version` viejo | 409 |
| R9.4 | viewer intenta PATCH directo | 403 |

### 21.12 Money / Reportes — fórmulas exactas (`backend/src/reports/money.rs`)

Todo en céntimos enteros (`i64`). `ord` = `ordinary_daily_minutes`, `base` = `base_salary_cents`.

| Componente | Fórmula | Referencia LOTTT |
|---|---|---|
| `work_pay` | `work_min × base / ord` | — |
| `ot_pay` | `ot_min × base × 150 / (100 × ord)` | Art. 118 (+50%) |
| `night_premium` | `work_min × base × 30 / (100 × ord)` ADITIVO | Art. 117 (+30%) |
| `rest_day_surcharge` | `work_min × base × 50 / (100 × ord)` | Art. 120 (+50%) |
| `late_deduction` | `late_min × base / ord` | descuento prorrateado |
| `total_a_pagar` | `work_pay + ot_pay + night + rest - late` | saturating |

**Casos numéricos exactos** (ord=480, base=$1.00 = 100 cents):

| # | Inputs | Esperado (cents) |
|---|---|---|
| R12.1 | work=480 | work_pay = 100 ($1.00) |
| R12.2 | work=240 | work_pay = 50 ($0.50) |
| R12.3 | ot=60 | ot_pay = 18 (~$0.18, +50%) |
| R12.4 | night, work=480 | night = 30 (+30% sobre $1.00) |
| R12.5 | rest_day, work=480 | rest = 50 (+50%) |
| R12.6 | late=15 | deducción = 3 (~$0.03) |
| R12.7 | misconfig ord=0 | todas las funciones → 0 (no panic, no div/0) |

Con seed actual ($30..$80 USD = 3000..8000 cents):
- EMP001 ($30, 480 min, 0 OT) → work_pay = 3000 cents = $30
- EMP006 ($80, 540 min, 60 OT) → work_pay = 8000, ot_pay = (60×8000×150)/(100×480) = 1500 → total $95
- Validar la columna "Total a pagar" en Reportes XLSX coincida con esta fórmula.

### 21.13 RBAC backend (autoridad)

| Endpoint | admin | supervisor | viewer | anon |
|---|---|---|---|---|
| `GET /api/v1/employees` | ✅ | ✅ | ✅ | 401 |
| `POST /api/v1/employees` | ✅ | 403 | 403 | 401 |
| `GET /api/v1/anomalies` | ✅ | ✅ | 403 | 401 |
| `PATCH /api/v1/rules` | ✅ | 403 | 403 | 401 |
| `POST /api/v1/leaves` | ✅ | ✅ | 403 | 401 |
| `DELETE /api/v1/leaves/{id}` | ✅ | 403 | 403 | 401 |
| `GET /api/v1/audit` | ✅ | 403 | 403 | 401 |
| `POST /api/v1/__test_reset` | 404 (gated `CRONOMETRIX_E2E`) | 404 | 404 | 404 |

Validar con `curl -H "Authorization: Bearer <token>"`.

### 21.14 Concurrencia / Optimistic locking

| # | Caso | Esperado |
|---|---|---|
| R14.1 | dos PATCHes con mismo `version` | el segundo recibe 409 |
| R14.2 | DELETE con `version` viejo | 409 |
| R14.3 | UI en 2 tabs editando reglas | banner "otro admin acaba de cambiar" |

### 21.15 Audit log inmutable (D-13)

| # | Caso | Esperado |
|---|---|---|
| R15.1 | cualquier mutación | crea entry con `actor_id`, `action`, `before/after`, `justification`, `created_at` |
| R15.2 | cero endpoints UPDATE/DELETE de audit | confirmar en API spec |
| R15.3 | borrar empleado | audit guarda snapshot completo |

### 21.16 Checks integradores (escenarios end-to-end)

Combinar reglas en un solo día y verificar resultados.

| # | Escenario | Validación |
|---|---|---|
| E1 | Lun, turno 08–17 day, tol=10, bono=0, ord=480, base=$50, lunch fixed 60 | entrada 08:00, salida 17:00 → work=480, late=0, OT=0, total=$50.00 |
| E2 | igual + entrada 08:25 | late=25, deducción = 25×5000/480 ≈ 260 cents → total = $50 - $2.60 = $47.40 |
| E3 | igual + salida 19:00 | OT=120, ot_pay = 120×5000×150/(100×480) = 1875 cents → total = $50+$18.75=$68.75 |
| E4 | igual + salida 19:30 | OT=150, anomalía `OT_CAP_EXCEEDED_DAILY` (>120) |
| E5 | Domingo trabajado (rest day), full shift | total = work_pay + rest_surcharge = $50 + $25 = $75 |
| E6 | turno night, full shift | total = work_pay + night_premium = $50 + $15 = $65 |
| E7 | leave medical full-day | work_min=0, total=$0, sin penalización; reporte muestra "Justificada" |
| E8 | overnight 22:00 Lun → 06:00 Mar | DailyRecord anchor=Lunes; reporte agrupa por Lunes |

---

## 22. Checklist final pre-aceptación demo

- [ ] §1 smoke pasa
- [ ] §2 los 3 roles login + logout OK
- [ ] §3 dashboard KPIs coherentes
- [ ] §4 CRUD empleados completo
- [ ] §6 timesheet edit + audit log
- [ ] §8 reportes XLSX + PDF descargan con datos
- [ ] §9 anomalías filtran (si Group C deployed)
- [ ] §10 audit muestra mutaciones de §4 y §6
- [ ] §16 RBAC matrix verificada
- [ ] §17 `/__test_reset` retorna 404
- [ ] Mobile responsive básico OK

Sign-off → captura del checklist completo + nombre QA + fecha (Caracas TZ).
