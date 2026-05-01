# Cronometrix — Suite de Regresión Completa

**Duración objetivo:** 2-4 h
**Frecuencia:** pre-release / weekly
**Política:** todos los P0 deben pasar. ≥90% de P1. P2/P3 documentados pero no bloqueantes.

Cubre los 111 TCs de `TEST-CASES.md` agrupados por dependencia y rol para minimizar logout/login.

**Precondición obligatoria:** suite SMOKE pasada (`SMOKE-SUITE.md`). Si smoke falla, NO ejecutar regresión.

---

## Resumen por prioridad

| Prioridad | TCs | % suite | Tiempo est. |
|---|---|---|---|
| P0 | 22 | 20% | 30 min (= smoke) |
| P1 | 65 | 58% | 90 min |
| P2 | 20 | 18% | 45 min |
| P3 | 4 | 4% | 15 min |
| **Total** | **111** | 100% | **3 h** |

---

## Fase 1 — Smoke (30 min)

Ejecutar `SMOKE-SUITE.md` completa. Registrar resultados.

**Si smoke FAIL:** STOP regresión. Reportar y resolver smoke primero.

---

## Fase 2 — Bloque admin extendido (60 min)

**Login:** `demo_admin`

### 2.1 Empleados (15 min)
- [ ] TC-EMP-004 — Búsqueda por nombre (P1)
- [ ] TC-EMP-005 — Filtro depto (P1)
- [ ] TC-EMP-006 — Conflicto 409 optimistic lock (P1)
- [ ] TC-EMP-007 — Desactivar empleado (P1)
- [ ] TC-EMP-008 — Reactivar (P2)
- [ ] TC-EMP-009 — Código duplicado bloqueado (P1)
- [ ] TC-EMP-010 — Validación campos vacíos (P2)

### 2.2 Departamentos (10 min)
- [ ] TC-DEP-002 — Crear depto día (P1)
- [ ] TC-DEP-003 — Crear depto overnight (P1)
- [ ] TC-DEP-004 — Crear depto night (P1)
- [ ] TC-DEP-005 — Eliminar con FK bloqueado (P1)
- [ ] TC-DEP-006 — Validación overnight sin flag (P2)

### 2.3 Timesheet (15 min)
- [ ] TC-TS-002 — Filtro empleado (P1)
- [ ] TC-TS-003 — Columnas HH:MM (P1)
- [ ] TC-TS-005 — Edit dispara recálculo (P1)
- [ ] TC-TS-006 — Conflict 409 edit (P1)
- [ ] TC-TS-007 — Badge On time (P1)
- [ ] TC-TS-008 — Badge Late (P1)
- [ ] TC-TS-009 — Paperclip evidencia (P1)
- [ ] TC-TS-010 — Cancelar leave (P1)

### 2.4 Dispositivos (10 min)
- [ ] TC-DEV-002 — Crear device (P1)
- [ ] TC-DEV-003 — IP+puerto duplicado (P1)
- [ ] TC-DEV-004 — Health check (P1)
- [ ] TC-DEV-005 — Edit sin reescribir pass (P2)
- [ ] TC-DEV-006 — Desactivar device (P2)

### 2.5 Reportes (10 min)
- [ ] TC-RPT-002 — Filtro mes anterior (P1)
- [ ] TC-RPT-003 — Filtro depto (P1)
- [ ] TC-RPT-004 — Drill-down (P1)
- [ ] TC-RPT-006 — Export PDF (P1)
- [ ] TC-RPT-007 — Mes sin datos (P2)
- [ ] TC-RPT-008 — Total fórmula coincide (P1)

---

## Fase 3 — Reglas + Usuarios (20 min)

**Login:** `demo_admin`

- [ ] TC-RUL-002 — Edit reglas (P1)
- [ ] TC-RUL-004 — Conflicto 409 reglas (P1)
- [ ] TC-RUL-005 — Validación rango 0-60 (P2)
- [ ] TC-USR-002 — Crear user (P1)
- [ ] TC-USR-003 — Cambiar rol (P1)
- [ ] TC-USR-004 — Reset password (P2)
- [ ] TC-USR-005 — Desactivar user (P1)

---

## Fase 4 — Auditoría completa (15 min)

- [ ] TC-AUD-003 — Justificación visible (P1)
- [ ] TC-AUD-004 — Read-only (no edit/delete) (P1)
- [ ] Verificar: cada mutación de Fases 2-3 generó audit entry con before/after.

---

## Fase 5 — Bloque supervisor (15 min)

**Login:** `demo_super`

- [ ] TC-ANO-002 — Filtro code=LATE (P1)
- [ ] TC-ANO-003 — Click "Ver" abre dialog (P1)
- [ ] TC-ANO-004 — Paginación (P2)
- [ ] TC-RBAC-007 — PATCH timesheet OK (P1)
- [ ] TC-RBAC-008 — POST leaves OK (P1)
- [ ] TC-RUL-003 — Form read-only en supervisor (P1)
- [ ] TC-EMP-011 — Sin botón Nuevo (P1)
- [ ] TC-DEV-007 — 403 en /devices (P1)
- [ ] TC-AUD-005 — 403 en /audit (P1)

---

## Fase 6 — Bloque viewer (10 min)

**Login:** `demo_viewer`

- [ ] TC-EMP-012 — Read-only (P1)
- [ ] TC-EVT-001 — Listado eventos (P1)
- [ ] TC-EVT-002 — Thumbnail foto (P2)
- [ ] TC-EVT-003 — include_unknown (P2)
- [ ] TC-EVT-004 — Click "Ver" foto grande (P2)
- [ ] TC-ANO-005 — 403 /anomalies (P1)
- [ ] TC-RBAC-009 — GET /reports OK (P1)

---

## Fase 7 — RBAC matrix backend (15 min)

**Tool:** curl + 3 tokens (admin/super/viewer) + sin token.

```sh
ADMIN_TOK=$(curl -s -X POST https://demo-api.cronometrix.app/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"demo_admin","password":"dSQBALuQgXWZp6Oo"}' | jq -r .token)
# repetir para SUPER_TOK, VIEWER_TOK
```

- [ ] TC-RBAC-003 — PATCH /rules solo admin (P0)
- [ ] TC-RBAC-004 — GET /anomalies admin+super (P1)
- [ ] TC-RBAC-005 — DELETE /leaves solo admin (P1)
- [ ] TC-RBAC-006 — GET /audit solo admin (P1)
- [ ] TC-RBAC-010 — JWT manipulado → 401 (P0)

---

## Fase 8 — Reglas de negocio (motor cálculo) (45 min)

**Setup:** crear depto QA-Test con tolerancias controladas, asignar 1 empleado, inyectar eventos por mock_hikvision admin port.

**Pre-trabajo:**
1. Crear depto: shift 08:00–17:00, lunch fixed=60, ord=480
2. Crear empleado QA-Test asignado al depto
3. Configurar reglas globales: late_tol=10, early_tol=10, bonus=0
4. Push eventos vía:
   ```sh
   sudo docker exec dokku-cronometrix-api curl -X POST \
     http://localhost:4401/admin/push -H "Content-Type: application/xml" \
     --data-binary @event-{N}.xml
   ```

### 8.1 Tolerancias (15 min)
- [ ] TC-RULE-001 — late=0 dentro tol (P1)
- [ ] TC-RULE-002 — late=11 supera tol (P1)
- [ ] TC-RULE-003 — bono extiende grace (P1)
- [ ] TC-RULE-004 — bono respeta umbral (P1)
- [ ] TC-RULE-005 — early dentro tol (P1)
- [ ] TC-RULE-006 — early supera tol (P1)
- [ ] TC-RULE-007 — ventana excluye fuera (P2)
- [ ] TC-RULE-008 — unknown face anomalía (P1)

### 8.2 Money formulas (10 min)
- [ ] TC-MNY-001 — work_pay full (P0)
- [ ] TC-MNY-002 — work_pay half (P1)
- [ ] TC-MNY-003 — ot_pay +50% (P1)
- [ ] TC-MNY-004 — night +30% additive (P1)
- [ ] TC-MNY-005 — rest_day +50% (P1)
- [ ] TC-MNY-006 — late_deduction (P1)
- [ ] TC-MNY-007 — total composición (P1)
- [ ] TC-MNY-008 — misconfig ord=0 (P1)
- [ ] TC-MNY-009 — night gates per-day shift (P1)

**Atajo:** verificable también con `cargo nextest run -p cronometrix_api reports::money` localmente — validar 13 tests pasen.

### 8.3 Anomaly codes end-to-end (15 min)
Para cada código de §21.9, inyectar setup que lo dispare y verificar aparece en `/anomalies`:

- [ ] `MISSING_ENTRY` — solo evento exit sin entry
- [ ] `MISSING_EXIT` — solo evento entry sin exit
- [ ] `UNKNOWN_FACE_IN_WINDOW` — TC-RULE-008
- [ ] `LUNCH_PUNCH_MISSING` — modo punch sin par completo
- [ ] `OT_CAP_EXCEEDED_DAILY` — work=605 min
- [ ] `OT_CAP_EXCEEDED_WEEKLY` — acumular 610 OT en semana
- [ ] `OT_CAP_EXCEEDED_ANNUAL` — acumular 6010 OT en año
- [ ] `EVENTS_ON_LEAVE_DAY` — leave full-day + eventos ese día
- [ ] `RECOMPUTE_AFTER_EDIT` — TC-TS-005
- [ ] `OVERNIGHT_INFERENCE_AMBIGUOUS` — N/A en VE (skip)

### 8.4 Escenarios E2E integradores (5 min)
Validar §21.16:
- [ ] E1 — full shift on-time → total = $50 base
- [ ] E2 — late=25 → total ≈ $47.40
- [ ] E3 — OT=120 → total = $68.75
- [ ] E5 — domingo trabajado → total = $75
- [ ] E6 — turno night → total = $65
- [ ] E7 — leave medical → total = $0 sin penalización
- [ ] E8 — overnight 22:00→06:00 anchor=Lun

---

## Fase 9 — Concurrencia (10 min)

- [ ] TC-CON-002 — DELETE leave version stale → 409 (P1)
- [ ] TC-CON-003 — webhook bursts orden estable (P2)

---

## Fase 10 — Seguridad (15 min)

- [ ] TC-SEC-002 — license bypass sin E2E aborta (P1)
  - **Atajo:** `cargo nextest run -p cronometrix_api license_bypass_safety`
- [ ] TC-SEC-003 — SQL injection (P1)
- [ ] TC-SEC-004 — XSS escape (P1)
- [ ] TC-SEC-005 — CORS bloqueado (P2)
- [ ] TC-SEC-006 — device password encrypt at rest (P1)
  - **Atajo:** `dokku enter cronometrix-api && sqlite3 /tmp/cronometrix.db 'SELECT encrypted_password FROM devices LIMIT 1;'`

---

## Fase 11 — UI/UX edge (15 min)

- [ ] TC-DASH-002 — Donut renderiza (P1)
- [ ] TC-DASH-003 — Activity feed (P1)
- [ ] TC-DASH-004 — SSE en vivo (P1)
- [ ] TC-DASH-005 — TZ Caracas (P1)
- [ ] TC-DASH-006 — Refresh sin flicker (P2)
- [ ] TC-AUTH-004 — Password incorrecta (P1)
- [ ] TC-AUTH-005 — User inexistente (P1)
- [ ] TC-AUTH-006 — Campos vacíos (P2)
- [ ] TC-AUTH-007 — Sesión expirada (P1)
- [ ] TC-AUTH-008 — Logout (P1)
- [ ] TC-AUTH-009 — Acceso directo sin login (P1)

---

## Fase 12 — Eventos / Mock integration (10 min)

- [ ] TC-EVT-005 — Push evento mock → aparece en UI (P1)

---

## Criterio Pass/Fail global

**PASS (release aprobada):**
- 100% de P0 PASS (22/22)
- ≥90% de P1 PASS (≥59/65)
- Total bugs P0/P1 abiertos = 0
- P2/P3 documentados con tickets pero no bloqueantes

**FAIL (no aprobar release):**
- Cualquier P0 FAIL
- <90% de P1 PASS
- Bug crítico de seguridad descubierto
- Pérdida de datos en alguna prueba

**CONDITIONAL (release con caveats):**
- 100% P0, 85-89% P1
- P1 fail con workaround documentado
- Plan de fix definido para próximo release

---

## Plantilla de reporte final

```markdown
## Regression Run — YYYY-MM-DD

**Release:** v0.X.Y
**Tester:** {nombre}
**Build commit:** {hash}
**Inicio:** HH:MM   **Fin:** HH:MM   **Duración:** Xh Ym

### Resumen

| Prioridad | Total | Pass | Fail | Blocked | NotRun | Pass% |
|---|---|---|---|---|---|---|
| P0 | 22 | – | – | – | – | – |
| P1 | 65 | – | – | – | – | – |
| P2 | 20 | – | – | – | – | – |
| P3 | 4 | – | – | – | – | – |

### Bugs encontrados

- BUG-### — [título] — severidad — TC origen
- ...

### Bugs bloqueantes
- ...

### Recomendación
- [ ] APROBAR release
- [ ] APROBAR con caveats: ...
- [ ] BLOQUEAR — corregir BUG-### antes de re-test

### Firma QA
{nombre} — {fecha Caracas}
```

Guardar en `docs/qa/runs/regression-YYYY-MM-DD.md`.

---

## Atajos útiles

```sh
# Backend tests automatizan TC-MNY-* y TC-SEC-002
cd backend && cargo nextest run -p cronometrix_api reports::money license_bypass_safety

# Frontend tests
cd frontend && npm run test

# E2E suite (Phase 9)
make e2e

# Coverage gates (debe pasar antes de regresión manual)
make coverage

# Logs producción para correlacionar bugs
ssh gerswin@192.168.0.44 'sudo docker exec dokku dokku logs cronometrix-api --tail 500'
```

---

## Mantenimiento de la suite

- Cada feature nuevo añade TCs nuevos a `TEST-CASES.md` y referencia en la fase apropiada de este doc.
- Cada bug de regresión real en producción → añadir TC nuevo (sección "Regresión histórica").
- Revisar prioridades trimestralmente — un P2 que se rompe muy seguido sube a P1.
