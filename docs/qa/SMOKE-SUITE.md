# Cronometrix — Suite de Smoke Tests

**Duración objetivo:** 15-30 min
**Frecuencia:** cada deploy + diaria en demo
**Política:** falla cualquier P0 → bloquea release, no continuar con regresión.

Cubre solo **caminos críticos P0**. Si todos pasan, sistema está vivo y funcional para uso básico. Para regresión completa ver `REGRESSION-FULL.md`.

---

## Precondiciones (5 min)

1. **Health endpoint:**
   ```sh
   curl -fsS https://demo-api.cronometrix.app/api/v1/health
   ```
   **Esperado:** `200 OK` con `{"status":"ok"}`. Si falla → STOP. Investigar logs.

2. **Web responde:**
   ```sh
   curl -fsS -o /dev/null -w "%{http_code}" https://app-demo.cronometrix.app/
   ```
   **Esperado:** `200` o `307` (redirect a login). Si 5xx → STOP.

3. **`/__test_reset` bloqueado:**
   ```sh
   curl -s -o /dev/null -w "%{http_code}" -X POST https://demo-api.cronometrix.app/api/v1/__test_reset
   ```
   **Esperado:** `404`. Si retorna otro código → BLOCKER de seguridad. STOP.

4. **DB seed presente:**
   - Login admin (paso 1) → si dashboard muestra `0 empleados` → seed no corrió, reiniciar contenedor.

5. **TZ configurado:**
   - Hora del activity feed debe coincidir con hora local Caracas (UTC-4 sin DST).

---

## Suite (22 TCs · ~20 min)

| # | TC ID | Módulo | Tiempo | Crítico si falla |
|---|---|---|---|---|
| 1 | TC-AUTH-001 | Login admin | 1 min | SI — bloquea todo |
| 2 | TC-AUTH-002 | Login supervisor | 30s | SI — RBAC roto |
| 3 | TC-AUTH-003 | Login viewer | 30s | SI — RBAC roto |
| 4 | TC-DASH-001 | KPIs cargan | 30s | NO — degradación visible |
| 5 | TC-EMP-001 | Listado seed (6 empleados) | 30s | SI — sin datos no se prueba nada |
| 6 | TC-EMP-002 | Crear empleado | 1 min | SI — CRUD core |
| 7 | TC-EMP-003 | Editar empleado | 1 min | SI — CRUD core |
| 8 | TC-DEP-001 | Listado deptos seed | 30s | NO |
| 9 | TC-TS-001 | Listado timesheet | 30s | SI |
| 10 | TC-TS-004 | Edit con justificación | 2 min | SI — audit core |
| 11 | TC-DEV-001 | Listado devices | 30s | NO |
| 12 | TC-RPT-001 | Reporte mensual | 1 min | SI |
| 13 | TC-RPT-005 | Export XLSX | 1 min | SI — entregable cliente |
| 14 | TC-RPT-009 | Sueldos $30..$80 | 30s | SI — datos coherentes |
| 15 | TC-ANO-001 | Listado anomalías | 30s | NO |
| 16 | TC-AUD-001 | Audit log carga | 30s | SI |
| 17 | TC-AUD-002 | Mutación de TC-EMP-002 visible | 30s | SI — audit funciona |
| 18 | TC-RUL-001 | Reglas globales cargan | 30s | NO |
| 19 | TC-USR-001 | Listado users | 30s | NO |
| 20 | TC-RBAC-001 | GET /employees per rol | 1 min | SI — RBAC |
| 21 | TC-RBAC-002 | POST /employees solo admin | 1 min | SI — RBAC |
| 22 | TC-MNY-010 | Sueldos seed varían | 30s | SI — money math |

**Tiempo total estimado:** ~20 min ejecutando manual.

---

## Flujo recomendado (sin saltar entre roles)

### Bloque A — admin (10 min)
1. TC-AUTH-001 (login admin)
2. TC-DASH-001 (dashboard)
3. TC-EMP-001 (listado)
4. TC-EMP-002 (crear empleado QA-Smoke-{timestamp})
5. TC-EMP-003 (editar el creado)
6. TC-DEP-001 (deptos)
7. TC-DEV-001 (devices)
8. TC-TS-001 (timesheet)
9. TC-TS-004 (editar con justificación + verificar audit)
10. TC-RPT-001, TC-RPT-005, TC-RPT-009 (reportes + export + sueldos)
11. TC-AUD-001, TC-AUD-002 (audit del create + edit recientes)
12. TC-RUL-001, TC-USR-001
13. Logout

### Bloque B — supervisor (3 min)
14. TC-AUTH-002 (login)
15. TC-ANO-001 (anomalías)
16. Verificar sidebar oculta Settings → Usuarios + Reglas
17. Logout

### Bloque C — viewer (2 min)
18. TC-AUTH-003 (login)
19. Verificar `/employees` sin botón "Nuevo"
20. Verificar `/anomalies` redirige o oculta
21. Logout

### Bloque D — RBAC backend (5 min)
22. TC-RBAC-001 (curl con 3 tokens)
23. TC-RBAC-002 (POST employees con 3 tokens + sin token)

---

## Criterio Pass/Fail

**PASS (release autorizada para regresión completa):**
- Todos los 22 TCs pasan
- Sin errores 5xx en DevTools Network
- Sin excepciones en console.error

**FAIL (BLOCKER — no continuar):**
- Cualquier TC marcado "Crítico si falla = SI" falla
- Health endpoint no responde
- `/__test_reset` ≠ 404
- Login admin no funciona

**CONDITIONAL (proceder con regresión, abrir bug):**
- TC con "Crítico = NO" falla
- Degradación UI visible pero funcional

---

## Comando rápido (script futuro)

```sh
# TODO: scripts/qa-smoke.sh — automatiza Bloque D + curls de health
# Por ahora: ejecutar manualmente y registrar abajo
```

---

## Plantilla de registro

```markdown
## Smoke Run — YYYY-MM-DD HH:MM (Caracas)

**Release:** v0.X.Y
**Tester:** {nombre}
**Build commit:** {hash}

| TC | Estado | Notas |
|---|---|---|
| TC-AUTH-001 | PASS | |
| TC-AUTH-002 | PASS | |
| ... | | |

**Resultado:** PASS / FAIL / CONDITIONAL
**Bugs abiertos:** BUG-### links
**Próximo paso:** ejecutar REGRESSION-FULL / detener
```

Guardar en `docs/qa/runs/smoke-YYYY-MM-DD.md`.
