# Verificación — Lote 4 (H-14, H-15, H-16, M-11, M-12, L-01, L-02)

Auditado contra el árbol de trabajo en
`/home/gerswin/Proyectos/cronometrix/.claude/worktrees/scratch`
(rama `docs/verificacion-lote2`, adelantada respecto de `main` en el mismo
commit visible). Todas las rutas son relativas a esa raíz. Trabajo de solo
lectura; no se modificó nada.

---

## H-14 — Protección, retención y recuperación de archivos incompletas

**Verdict: CONFIRMED**

Evidencia:

- Cifrado de aplicación: solo las credenciales de dispositivo usan AES-256-GCM.
  `backend/src/devices/crypto.rs:18-42` implementa `encrypt_password` /
  `decrypt_password` con `Aes256Gcm`; `backend/src/db/migrations/003_devices.sql:2-3`
  confirma que es la única columna cifrada
  (`encrypted_password TEXT NOT NULL`). No existe ningún módulo de cifrado
  para `cronometrix.db`, fotos, rostros o evidencias — búsqueda de
  `AES-256-GCM|encrypt` en `backend/src/` no arroja nada más.
- Backup/rollback incompleto: `deploy/INSTALL.md:75-77` documenta que el
  instalador, antes de un upgrade, guarda "the current Compose file, Nginx
  file, manifest, runtime environment, container image inventory, **and a
  consistent SQLite backup**" — solo la base de datos. El procedimiento de
  rollback manual (`deploy/INSTALL.md:89-98`) solo restaura
  `cronometrix.db`:
  ```
  sudo cp releases/rollback/TIMESTAMP/cronometrix.db data/cronometrix.db
  ```
  No hay ninguna copia/restauración de directorios de fotos, rostros o
  evidencias (`leaves_root`, `events_root`, `enrollments_root`,
  `overrides_root` per `backend/src/state/paths.rs`). No existe ningún
  script de backup en el repo (`find . -iname "*backup*"` no devuelve nada
  fuera de esas dos menciones en `INSTALL.md`).
- Retención: no hay ningún worker de limpieza por retención de archivos.
  `backend/src/workers/purge.rs` existe pero solo revoca mapeos
  cara↔dispositivo para empleados desactivados (D-15) — no toca ficheros de
  evidencia/fotos ni aplica política de antigüedad.
- Terceros sin gobierno documentado: no hay ningún documento de
  gobierno/DPA para Turso o Cloudflare (`find docs -iname "*governance*" -o
  -iname "*DPA*"` vacío); ambos solo aparecen en material comercial.

Si actuara sobre esto, la corrección real es exactamente la que pide el
informe (cifrado a nivel de volumen/archivo, manifiesto de backup que
incluya los cuatro directorios de evidencia, restauración probada
periódicamente) — no encontré nada que sugiera que el remedio descrito
causaría pérdida de datos; al contrario, hoy el respaldo YA pierde todo lo
que no sea la fila de SQLite.

---

## H-15 — Afirmaciones comerciales de cumplimiento no sustentadas

**Verdict: NUANCED** (sustancia real; un detalle de la descripción es
impreciso)

Lo que sí se sostiene, verificado en `docs/comercial/PROPUESTA-COMERCIAL.md`:

- Línea 9: *"convierte los eventos biométricos... en datos listos para
  nómina, con trazabilidad legal completa y sin cálculos manuales"* — clama
  "trazabilidad legal completa". El propio informe (M-06, no en este lote
  pero corroborante) documenta triggers de auditoría con `actor_id` nulo con
  frecuencia y cobertura de campos incompleta, así que "completa" es
  cuestionable.
- Línea 56: *"✅ Manejo de días feriados nacionales y particulares
  (calendario editable)"* — no existe ningún código de calendario de
  feriados en `backend/src/`. `grep -rln "holiday" backend/` solo encuentra
  `backend/tests/leaves_service_test.rs:87`, donde `"holiday"` es un
  `leave_type` individual (un permiso/vacación de un empleado), no un
  calendario de feriados nacionales/regionales. La funcionalidad anunciada
  no existe.
- LOPCYMAT: la tabla (línea ~129) sí describe el **Art. 56, num. 4** como
  *"Obligación del patrono de llevar registros de jornada"*. Que el
  contenido real del 56.4 sea otro (notificación de condiciones inseguras,
  según afirma el informe) es un hecho legal que queda fuera de mi alcance
  verificar directamente, pero la cita textual del documento coincide con
  lo que el informe dice que dice.

Lo impreciso — la parte de artículos LOTTT:

- El informe dice: *"La tabla atribuye jornada al artículo 167... corresponden
  173"*. Es cierto que la fila `Art. 167` cubre "Jornada diurna máx. 8h,
  nocturna máx. 7h, mixta máx. 7h 30min" — pero la tabla **también** tiene
  una fila separada `Art. 173` para "Jornada semanal máx. 40h (diurna) / 35h
  (nocturna)" (`PROPUESTA-COMERCIAL.md:110-111`). El documento no ignora el
  173; lo usa para el componente semanal. La propia matriz del informe
  (línea 554) describe el 173 real como cubriendo AMBOS componentes (diario
  y semanal + 5 días/2 descansos), lo que sustenta la crítica de fondo (167
  no es la fuente correcta para el límite diario), pero la frase "la tabla
  atribuye jornada al 167" podría leerse como si el documento nunca citara
  173, y sí lo cita — solo que para la parte semanal, no como fuente única.
  Un implementador que solo lea la descripción del hallazgo podría no notar
  que ya existe una fila 173 que hay que fusionar, no añadir desde cero.
- El informe dice: *"registro extra al 187; corresponde 183"*. El documento
  sí cita `Art. 187 — "Registro de horas extraordinarias autorizadas"`
  (línea 116). Cuál es el artículo correcto (183 vs 187) es una cuestión
  legal fuera de mi alcance; el hecho verificable —qué cita el documento— es
  correcto tal como lo reporta el informe.

Fix real si se actuara: revisar la tabla artículo por artículo con criterio
legal, fusionando 167+173 en una sola fila correcta para jornada (no
simplemente "mover" 167→173, porque 173 ya está usado para otra cosa en la
misma tabla) y corrigiendo 187→183 (o el artículo que el abogado confirme),
más quitar el claim de feriados hasta que exista la funcionalidad.

---

## H-16 — Alcance fiscal, de nómina y pagos no definido

**Verdict: CONFIRMED**, con una precisión importante sobre el Binance/USDT.

- No hay cálculo integral de IVSS/FAOV/INCES/ISLR/prestaciones:
  `grep -rln "IVSS|FAOV|INCES|ISLR" backend/src/` solo encuentra
  `backend/src/reports/excel.rs:148` (`"Días IVSS"`, una etiqueta de
  columna de conteo de días, no un cálculo de aporte) y comentarios en
  `backend/src/reports/service.rs`. No hay ningún motor de cálculo de
  aportes parafiscales.
- Un documento nuevo, **posterior al commit auditado** (`9b36341`),
  confirma exactamente el mismo alcance: `docs/legal/CONSULTA-LABORAL.md`
  (no existía en `9b36341`; añadido en los commits `57e5921`/`dd3c7d0`,
  ambos después del commit auditado) dice explícitamente: *"Cronometrix
  calcula horas trabajadas y produce una **pre-nómina**. No calcula IVSS,
  FAOV, INCES, ISLR ni prestaciones; el importe que emite alimenta un
  sistema de nómina externo."* Esto es una auto-corrección del equipo, no
  una respuesta al hallazgo (el documento es una consulta a abogado/contador
  sobre otras tres decisiones), pero corrobora el diagnóstico H-16 con la
  propia voz del proyecto.
- Precisión sobre Binance/USDT: el informe dice *"El manual acepta
  Binance/USDT sin proceso de factura..."* sin distinguir cuál manual. Verifiqué
  ambos documentos comerciales:
  - `docs/comercial/PROPUESTA-COMERCIAL.md:16` — forma de pago del **cliente**
    es *"50% al inicio, 50% al go-live, en efectivo (USD)"*. Ningún Binance/USDT
    en ningún punto de pago del cliente.
  - `docs/comercial/MANUAL-VENDEDORES.md:11,104` — este es el manual para
    **representantes de venta independientes** ("Aplica a representantes
    comerciales independientes (no empleados)"), y el Binance/USDT
    (línea 104: `"Binance (USDT u otra stablecoin acordada)"`) es la forma en
    que la empresa **paga comisiones a sus vendedores**, no la forma en que
    el cliente paga la licencia.

  Esto no invalida el hallazgo — pagar comisiones en USDT a contratistas
  independientes sigue teniendo implicaciones de IGTF/retención/factura que
  hoy no están documentadas — pero la descripción del informe, leída sin
  contexto, sugiere que es el **cliente** quien paga en cripto, lo cual es
  falso y llevaría a un implementador a revisar el documento equivocado
  (la propuesta comercial) en vez del correcto (el manual de vendedores).

---

## M-11 — Configuración de equipos es best-effort y se admite transporte inseguro

**Verdict: CONFIRMED**

- Best-effort, sin estado persistente de conformidad:
  `backend/src/isapi/stream.rs:266-395` (`provision_device`) registra cada
  fallo de aprovisionamiento con `tracing::warn!` y **sigue ejecutando**
  (nunca aborta ni bloquea la ingesta): construcción de cliente fallida
  (línea 282-285), reloj (341-346), webhook (306-310), paso genérico
  (347-349) — todos son `warn!` y la función continúa o retorna
  silenciosamente sin marcar nada persistente. La única columna `status` en
  `devices` (`backend/src/db/migrations/003_devices.sql:17`) es
  `CHECK(status IN ('active','inactive'))` — administrativa, no de
  conformidad de aprovisionamiento. No hay tabla ni columna que registre
  "este dispositivo quedó mal configurado".
- Transporte inseguro permitido: `003_devices.sql:10` —
  `scheme TEXT NOT NULL DEFAULT 'https' CHECK(scheme IN ('http', 'https'))`
  — HTTP explícito es un valor válido. `backend/src/isapi/client.rs:139-147`
  tiene un flag `allow_insecure_tls` que, si es `true`, llama
  `.danger_accept_invalid_certs(allow_insecure_tls)` en el cliente HTTP —
  desactiva la verificación de certificado TLS por dispositivo, configurable
  desde `backend/src/devices/handlers.rs` / `service.rs`.

---

## M-12 — Restricciones de dominio y observabilidad insuficientes

**Verdict: CONFIRMED**

- Salarios negativos permitidos: `backend/src/db/migrations/018_employees_base_salary.sql:8`
  añade `base_salary_cents INTEGER NOT NULL DEFAULT 0` sin ningún `CHECK`.
  `024_employee_salary_kind.sql` añade `salary_kind` con `CHECK` de enum,
  pero ningún migration posterior agrega `CHECK(base_salary_cents >= 0)`.
- Minutos sin rango: `backend/src/db/migrations/007_daily_records.sql:13-16`
  declara `work_minutes`, `overtime_minutes`, `late_minutes`,
  `early_departure_minutes` como `INTEGER NOT NULL DEFAULT 0` sin ningún
  `CHECK` de rango (ej. `>= 0` o `<= 1440`).
- `writable_schema`: `backend/src/db/migrations/014_phase5_audit_triggers.sql:19-29`
  usa literalmente `PRAGMA writable_schema = 1; UPDATE sqlite_master ...
  PRAGMA writable_schema = 0;` para reescribir el SQL de un `CHECK`
  almacenado, en vez de reconstruir la tabla de forma soportada.
- `/health` solo hace `SELECT 1`: `backend/src/main.rs:687-705` — la función
  `health()` conecta, ejecuta `"SELECT 1"`, y devuelve
  `{"status": "ok", "database": "connected"}`. No consulta cola de
  recálculo, estado de ingesta, sync, disco, licencia ni dispositivos —
  coincide exactamente con la descripción del informe.

---

## L-01 — Estado público e información operativa minimizable

**Verdict: CONFIRMED**

- `backend/src/main.rs:251-258` agrupa explícitamente
  `/health`, `/auth/login`, `/setup/status`, `/setup/init`, `/setup/activate`
  bajo el comentario `// Public routes — no auth required`.
- `backend/src/setup/handlers.rs:12-36` (`setup_status`) es alcanzable sin
  autenticación y devuelve `{"initialized": count > 0, "licensed": <bool>}`
  — permite a cualquiera en la red distinguir una instancia recién
  desplegada (sin admin, sin licencia activada) de una operativa, sin
  ningún límite de tasa.
- No hay rate limiting en ningún punto del backend:
  `grep -rln "rate_limit|RateLimit|tower_governor" backend/src/` no
  devuelve nada.
- `/health` (misma sección) tampoco diferencia degradación — devuelve solo
  `ok`/error binario, consistente con lo que ya se confirmó en M-12.

---

## L-02 — Deriva entre artefactos documentales

**Verdict: CONFIRMED** para instalación/ancho de banda/modelos; **matizado**
para el límite de evidencia (probablemente diseño en capas, no deriva
accidental) — pero el hecho numérico reportado es correcto.

- Días de instalación:
  - `docs/comercial/PROPUESTA-COMERCIAL.md:268` — *"Instalación y
    configuración inicial | 7–8 días hábiles"*.
  - `docs/comercial/PROPUESTA-COMERCIAL.html:692` — *"Instalación y
    configuración inicial | 3–5 días hábiles"*.
  - Corrobora el HTML: `docs/comercial/MANUAL-VENDEDORES.md:385` —
    *"Los tiempos de instalación (3–5 días) y go-live (10–15 días
    hábiles) son los oficiales."* — es decir, dos de los tres documentos
    dicen 3–5 y uno (el Markdown de la propuesta) dice 7–8.
- Ancho de banda:
  - MD línea 285: *"mínimo 1 Mbps simétricos recomendado"*.
  - HTML línea 706: *"mínimo 5 Mbps simétricos recomendado"*.
  Contradicción directa confirmada, 1 vs 5 Mbps.
- Modelos de dispositivo:
  - Material comercial (MD y HTML, ambos coinciden entre sí):
    *"modelos faciales como DS-K1T671/672 y similares"*
    (`PROPUESTA-COMERCIAL.md:284`, `.html:705`).
  - Material técnico/de pruebas usa un modelo distinto:
    `docs/auditoria/INFORME-TECNICO.md:49` — *"prueba física con Hikvision
    DS-K1T341/DS-K1T342"*; el firmware real contra el que está construido
    el parser (`backend/src/isapi/parser.rs:5`, migraciones 021/023) es
    `DS-K1T341CMFW`. El código está construido y probado contra la serie
    341/342; lo que se vende es la serie 671/672 — no hay evidencia en el
    repo de que 671/672 se haya probado.
- Límite de evidencia 5 MB vs 10 MB:
  - `frontend/src/lib/validations.ts:26` — `f.size <= 5 * 1024 * 1024,
    'Máximo 5MB'`; el modal de UI dice literalmente *"Máx. 5MB"*
    (`frontend/src/components/timesheet/novedad-modal.tsx:380`).
  - `backend/src/daily_records/handlers.rs:77` — `MAX_EVIDENCE_BYTES: usize
    = 10 * 1024 * 1024; // 10MB backend cap; frontend enforces 5MB` — el
    propio comentario del backend documenta la asimetría a propósito.
  - Matiz: esto lee más como **validación en dos capas intencional** (UI
    más estricta por UX, backend con margen de seguridad) que como "deriva
    documental accidental" — pero el efecto práctico que señala el informe
    es real y explotable: quien llame a la API directamente (o bordee el
    frontend) puede subir hasta 10 MB pese a que la interfaz promete un
    tope de 5 MB, así que el número comunicado al usuario no es el que
    realmente se aplica.

Fix real: fuente única de verdad para instalación/ancho de banda/modelo
(regenerar HTML/PDF desde el Markdown en vez de mantenerlos a mano), y para
el límite de evidencia, documentar explícitamente que 5 MB es un límite de
UX y 10 MB el límite real de seguridad — o alinear ambos si la intención es
que sean el mismo número.
