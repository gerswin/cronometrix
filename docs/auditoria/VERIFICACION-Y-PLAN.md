# Verificación de la auditoría y plan de remediación

**Auditoría revisada:** `docs/auditoria/INFORME-AUDITORIA-INTEGRAL.md` (40 hallazgos)
**Auditada contra:** `9b36341` — **verificada contra:** `1ec5c9e` (main actual)
**Fecha:** 2026-08-02

La auditoría la produjo otro agente. Antes de planificar arreglos se comprobó
hallazgo por hallazgo contra el código actual. Este documento registra qué
resultó cierto, qué está impreciso y qué queda sin verificar.

---

## 1. Resultado de la verificación

### Críticos: 10/10 reales

| ID | Veredicto | Evidencia comprobada |
|----|-----------|----------------------|
| C-01 | **Confirmado** | `calc/engine.rs:82` define `overtime_minutes = (work_minutes - ordinary).max(0)` — los extra son **subconjunto** de `work_minutes`. `reports/service.rs:293-302` paga `work_pay(work_minutes)` **y además** `ot_pay(overtime_minutes)` al 150%. Los mismos minutos se pagan 100% + 150% = **250%**. |
| C-02 | **Confirmado** | `calc/engine.rs:70-76`: `work_minutes` es tiempo real entre entrada y salida, así que la tardanza ya reduce el pago. `reports/service.rs:324` resta `late_deduction_cents` otra vez. |
| C-03 | **Confirmado** | `employees/service.rs:105`: `req.base_salary_cents.unwrap_or(0)` — el salario ausente se persiste como `0`, no `NULL`. El reporte lee `e.base_salary_cents` sin herencia departamental. |
| C-04 | **Confirmado** | `009_daily_record_overrides.sql:24` crea `idx_overrides_record` **no único**; no hay índice parcial sobre `status='active'`. `reports/service.rs:196` hace `LEFT JOIN ... AND dro.status='active'` → varias filas activas multiplican el resultado. |
| C-05 | **Confirmado** | `reports/service.rs:193-194`: `FROM daily_records dr JOIN employees e`. El universo nace de los registros; un empleado sin ninguna marca **no aparece**. |
| C-06 | **Confirmado (decisivo)** | SHA-256 de la pública derivada de `do-functions/test-keys/test_priv.pem` = `f0318260deb1672cb624314065c8bd42d394fd799a954580265ca3b19f3106fc`, **idéntico** al de `backend/src/license/pubkey.pem`, que producción carga vía `include_str!` (`license/service.rs:36`). La clave privada del repo firma licencias que el backend acepta. |
| C-07 | **Confirmado** | `activate/index.js:46` hace `SELECT hardware_fingerprint` y `:60` un `UPDATE ... WHERE license_key = $3` sin guarda sobre el fingerprint. TOCTOU. |
| C-08 | **Confirmado** | Ruta `/devices/{device_id}/push/{token}` (`main.rs:259`) y `http_trace.rs:17` registra `request.uri().path()`, que **incluye el token**. |
| C-09 | **Confirmado** | `setup/handlers.rs:68-86` lee `COUNT(*)` por una conexión y `:91-105` inserta por la cola `db_write` — operaciones distintas, sin transacción. Además el hash Argon2 (`:88`) corre **entre** ambas, ensanchando la ventana a cientos de ms. El comentario de `:52` afirma que previene la carrera; no lo hace. |
| C-10 | **Confirmado en sustancia, titular impreciso** | Ver §2. |

### Altos verificados por muestreo

| ID | Veredicto | Evidencia |
|----|-----------|-----------|
| H-03 | **Confirmado** | `departments/models.rs` no contiene `shift_type`, `is_overnight_shift` ni `ordinary_daily_minutes`, pese a existir en migración 012. |
| H-04 | **Confirmado** | `calc/engine.rs:95-98` cablea sábado/domingo como descanso, con el comentario `D-12: v1 hardcodes Sat/Sun`. |
| H-13 | **Confirmado, con matiz** | `license/service.rs:52` fija `validate_exp = false`. `exp` solo se usa en `:253` para decidir si renovar — **nunca** para rechazar. Una licencia vencida funciona indefinidamente. El matiz: está documentado como decisión "D-07 soft expiry", no es un descuido. |
| M-10 | **Confirmado** | `do-functions/package-lock.json` no existe → `npm ci` no puede funcionar. |

---

## 2. Correcciones a la auditoría

Dos hallazgos están descritos de forma que induciría a un arreglo equivocado.

### C-10 — el titular dice lo contrario de lo que pasa

La auditoría titula: *"Acuse exitoso **antes** de persistencia"*. El código
persiste **antes** del ACK: `devices/push.rs:126-148` recorre las partes y hace
`await ingest_alert(...)` para cada una; el `Ok(ACK)` está después, en `:150`.

El defecto real es otro: los errores se **tragan** (`:141-146`) y aun así
responde 200. Un fallo de parseo o de base pierde el evento en silencio. Eso sí
es real y grave.

**Por qué importa la distinción:** el `swallow` es deliberado y está justificado
con medición sobre hardware (`push.rs:89-99`). El DS-K1T341CMFW trata solo el
200 como confirmación; ante cualquier otra cosa reenvía el mismo evento para
siempre y **la cabeza de la cola nunca avanza**, de modo que todo evento
posterior queda detrás y no se entrega nunca. Está medido: bajo 204 llegó el
mismo `serialNo` en cuatro pushes consecutivos; bajo 200 el contador avanzó.

Un arreglo ingenuo — devolver 500 cuando la ingesta falla — **regresaría** ese
comportamiento y perdería más eventos de los que salva. La corrección correcta
es la que la propia auditoría recomienda: inbox durable (persistir el cuerpo
crudo con hash/idempotency antes de responder, procesar asíncrono, DLQ). Solo
así el 200 deja de ser mentira sin reactivar el bloqueo de cola.

### H-13 — es una decisión de producto, no un descuido

`validate_exp = false` está rotulado "D-07 soft expiry". La expiración blanda
para instalaciones on-premise sin conectividad es una decisión razonable y
probablemente intencional. Lo que sí es defecto es que **no exista ningún
control** de vencimiento: ni gracia acotada, ni aviso, ni bloqueo tras N días.
Tampoco se validan `product`, emisor ni audiencia.

Antes de "arreglarlo" hay que decidir el comportamiento comercial deseado. No
es una corrección técnica con respuesta única.

---

## 3. Lo que NO se verificó

Se comprobaron los **10 críticos** y **4 altos** por muestreo. Quedan **26
hallazgos sin verificar** (H-01, H-02, H-05 a H-12, H-14 a H-16, M-01 a M-09,
M-11, M-12, L-01, L-02).

No asumir que son ciertos. El muestreo salió muy bien (14/14 reales, 2 con
matiz), lo que da confianza en el método del auditor, pero dos de los que sí
miré estaban descritos de forma que llevaba a un arreglo equivocado. Cada
hallazgo debe verificarse antes de tocar código.

Además: la auditoría es contra `9b36341`, anterior al refactor hexagonal
(`f4bcbef`). Las rutas de ingestión se movieron — las referencias de línea de la
zona `isapi`/`devices` pueden haber derivado, aunque la sustancia se sostuvo en
todo lo comprobado.

Fuera de alcance de esta verificación, igual que en la auditoría original: nada
de esto sustituye criterio legal, fiscal ni contable venezolano. Las
afirmaciones sobre LOTTT se tomaron como las presenta el auditor; no se
contrastaron contra la Gaceta.

---

## 4. Plan de remediación

Orden por riesgo, no por esfuerzo. Cada bloque es independiente y entrega algo
verificable.

### Bloque 0 — Contención de secretos (horas, no días)

Nada aquí requiere diseño; es daño ya consumado que sigue abierto.

1. **C-06 — rotar el par de firma.** Tratar la clave del repo como comprometida
   aunque no haya evidencia de uso. Generar par nuevo fuera del repo, sustituir
   `backend/src/license/pubkey.pem`, reemitir licencias vigentes, borrar
   `do-functions/test-keys/` del árbol **y de la historia de git**, añadir
   escaneo de secretos al CI. Mientras la privada esté en la historia, cualquiera
   con el repo emite licencias válidas.
2. **C-08 — sacar el token de la URI.** Aceptarlo por cabecera, rotar todos los
   tokens `push` existentes, y verificar que ni `http_trace` ni el `access_log`
   de nginx conserven rastro. Requiere reconfigurar los `httpHosts` de cada
   device — hay que coordinarlo con el reprovisioning, no es solo backend.
3. **C-09 — cerrar la carrera de bootstrap.** `BEGIN IMMEDIATE` que compruebe e
   inserte en la misma transacción, o índice singleton sobre el estado de
   inicialización. Es el arreglo más barato de los diez. Prueba concurrente
   obligatoria: dos requests simultáneos, uno gana, el otro recibe 409.
4. **C-07 — activación atómica.** `UPDATE ... WHERE license_key = ? AND
   (hardware_fingerprint IS NULL OR hardware_fingerprint = ?)` exigiendo una fila
   afectada. Prueba de carrera real.

### Bloque 1 — Exactitud monetaria (lo que hoy paga mal)

Estos cinco se tocan juntos porque comparten el modelo de minutos y el mismo
conjunto de pruebas.

5. **C-01 + C-02 — rehacer el modelo de minutos.** Separar `ordinary_minutes` de
   `overtime_minutes` de forma que no se solapen, y decidir **una sola** política
   de tardanza. Ojo: las pruebas actuales codifican el comportamiento equivocado
   (`calc/overtime.rs` suma `work + overtime` para el tope diario, y la prueba lo
   afirma). Hay que corregir las pruebas junto con el código; no sirven como red.
6. **C-03 — salario efectivo.** Representar ausencia como `NULL`, resolver la
   herencia departamental en escritura o en cálculo, y rechazar cero/negativo
   salvo autorización explícita. Migración para los que ya quedaron en 0.
7. **C-04 — una sola anulación activa.** Índice único parcial
   `WHERE status='active'`, sustitución transaccional, y migración que resuelva
   los duplicados que ya existan antes de poder crear el índice.
8. **C-05 — universo desde empleados.** Invertir el `FROM`: partir de empleados
   activos × calendario esperado y unir los registros, no al revés.

**Antes de cerrar el bloque:** recalcular los períodos ya emitidos y producir un
informe de diferencias. Si algún cliente ya pagó con estos números, la
corrección cambia importes hacia arriba y hacia abajo — eso es una conversación
comercial, no solo un deploy.

### Bloque 2 — Integridad de ingesta

9. **C-10 — inbox durable.** Persistir el cuerpo crudo con hash/idempotency key
   **antes** de responder; procesar asíncrono con reintentos y DLQ visible.
   Mantener el 200 incondicional: es requisito del firmware, no un descuido
   (§2). El invariante a demostrar: *todo evento aceptado queda persistido
   exactamente una vez o visible para reintento*.

### Bloque 3 — En adelante, verificar primero

Los 26 hallazgos restantes entran solo después de comprobarse uno por uno. El
orden sugerido por la auditoría (modelo laboral → histórico → privacidad →
operación) es razonable, pero varios de esos bloques son decisiones de producto
antes que trabajo técnico: qué es pre-nómina y qué no, qué expiración de
licencia se quiere, qué se promete comercialmente.

---

## 5. Advertencia sobre las pruebas

La auditoría lo señala y se confirmó: **parte de la suite consolida la
especificación equivocada**. `calc/overtime.rs:39-45` afirmaba la suma diaria errónea.

**Corrección (2026-08-03):** la auditoría añade que "la prueba QA E3 acepta el
comportamiento duplicado". **Es falso** — `docs/QA-GUIDE.md:621` documenta
`total = $50 + $18.75 = $68.75`, que es la composición correcta; el defecto
habría dado $81.25. Al ejecutar la corrección resultó además que **solo un**
test existente afirmaba lo erróneo. El problema real era lo contrario de lo
descrito: ningún test asertaba el total del reporte para un día con horas
extra, y por eso el defecto sobrevivió a 1096 pruebas.

Consecuencia práctica: "las pruebas pasan" no es evidencia de corrección en el
motor monetario. Cualquier arreglo del Bloque 1 debe empezar por un corpus de
casos con resultados calculados a mano y revisados por alguien de nómina, no por
ajustar el código hasta que la suite existente vuelva a verde.
