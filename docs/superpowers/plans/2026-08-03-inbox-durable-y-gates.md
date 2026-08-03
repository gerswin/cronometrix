# Inbox durable, robustez del reporte y gates que sí midan — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cerrar C-10 —el último crítico, y el único que pierde datos de forma irrecuperable— más tres hallazgos de la revisión de rama monetaria (I5, I6, I7) y el piso de cobertura del frontend, que hoy no mide nada.

**Architecture:** Cinco tareas independientes. La primera y la segunda construyen un inbox durable en el **lado adaptador**, para que los bytes crudos de Hikvision nunca entren al núcleo. Las tres restantes son robustez del reporte, cobertura de las rutas monetarias sin asertos, y conectar un enforcer de cobertura que ya existe pero nadie invoca.

**Tech Stack:** Rust/Axum 0.8, libSQL/SQLite, migraciones numeradas, `cargo nextest`, Vitest.

## Global Constraints

- **Nunca** usar los identificadores desnudos `execute`, `execute_batch` ni `transaction` dentro de `backend/src` — `scripts/check_db_write_queue.py` es un gate que falla la build. Toda escritura va por `state.db_write.statement(...)`, `state.db_write.job(...)` o `state.db_write.transact(...)`; dentro de una transacción, `tx.query(...)` y `tx.statement(...)`.
- **Nunca romper la arquitectura hexagonal.** El detalle de vendedor vive solo en `backend/src/isapi/*`. `RawMarking` se construye **únicamente** en `isapi/ingest.rs`, y el núcleo se cruza en un solo punto (`isapi/ingest.rs:114`). Ver `docs/ARQUITECTURA-HEXAGONAL.md`.
- Toda mutación de datos de asistencia genera entrada de auditoría inmutable con justificación.
- Gate de cobertura backend duro: proyecto ≥90% líneas / ≥85% ramas; por archivo ≥70% / ≥60%.
- `cargo clippy --all-targets --all-features -- -D warnings` debe pasar.
- Mensajes de commit en inglés con prefijo convencional.
- **No hay datos productivos.** Las migraciones se hacen limpias, sin resolver filas existentes.
- El proyecto fija **Node 24.15.0** (`.nvmrc`, `engines`). Con Node 22 fallan cuatro tests de `export-buttons*` por motivos de entorno, no de código — si los ves fallar, comprueba tu versión de Node antes de investigar nada.

## Contexto imprescindible

### El contrato del firmware, y el matiz que este plan introduce

`backend/src/devices/push.rs:89-99` documenta, con medición sobre un DS-K1T341CMFW real, que ante cualquier respuesta distinta de 200 el lector **reenvía el mismo evento para siempre** y la cabeza de su cola nunca avanza — perdiendo todos los eventos posteriores. Bajo 204 llegó el mismo `serialNo` en cuatro pushes consecutivos; bajo 200 el contador avanzó.

Hoy el handler traga cualquier error de ingesta y responde 200 igual. Eso convierte un fallo de base de datos en **pérdida silenciosa y permanente** de una marcación.

Este plan introduce una distinción que hoy no existe:

- **Fallo de procesamiento** (XML inválido, evento desconocido, regla de negocio) → **200**, como hoy. El evento ya está a salvo en el inbox; reintentarlo con el device no ayudaría.
- **Fallo de durabilidad** (no se pudo escribir el inbox) → **no-200**, deliberadamente. El device conserva el evento y reintenta. Eso es contrapresión, no un error tragado: cuando la base no puede aceptar el dato, que el lector lo retenga es exactamente lo que queremos.

**No confundas esto con revertir la decisión del 200.** El 200 sigue siendo incondicional respecto al *resultado del procesamiento*. Lo que deja de ser incondicional es mentir cuando el dato no se guardó.

### El inbox NO deduplica

Es tentador poner un índice único sobre el hash del cuerpo. **No lo hagas.** Dos pushes con cuerpo idéntico pueden ser dos eventos legítimos —los heartbeats lo son casi siempre— y un falso positivo de deduplicación **pierde una marcación real**, que es justo el fallo que C-10 existe para impedir.

La deduplicación ya existe aguas abajo: `attendance_events` tiene `bucket_30s` para eso. El inbox guarda todo; el hash se almacena solo para diagnóstico y correlación.

---

## File Structure

| Archivo | Responsabilidad | Tarea |
|---|---|---|
| `backend/src/db/migrations/027_device_push_inbox.sql` | Tabla del inbox | 1 |
| `backend/src/devices/push.rs` | Persistir antes del ACK | 1 |
| `backend/src/workers/push_drain.rs` | Procesador asíncrono + reintentos | 2 |
| `backend/src/main.rs` | Lanzar el worker | 2 |
| `backend/src/devices/handlers.rs` | Endpoint de cola muerta | 2 |
| `backend/src/reports/service.rs` | Universo acotado (I5); error por fila (I7) | 3 |
| `backend/tests/reports_money_paths_test.rs` | Asertos de las rutas sin cubrir (I6) | 4 |
| `scripts/enforce-frontend-file-floor.mjs` | Piso por archivo real | 5 |
| `Makefile`, `.github/workflows/ci.yml` | Conectar el enforcer | 5 |

---

### Task 1: El push se persiste antes de confirmar (C-10, parte 1)

Hoy `receive_push` parsea y llama a `ingest_alert` para cada parte, traga cualquier error y responde 200. Si la base está ocupada —por ejemplo porque la cola de escritura agotó su `DEFAULT_ENQUEUE_TIMEOUT` de 5 s— el evento se pierde y el lector avanza su cola creyendo que se entregó.

Esta tarea guarda el cuerpo crudo **antes** de responder. Todavía no cambia el procesamiento: eso es la Tarea 2.

**Files:**
- Create: `backend/src/db/migrations/027_device_push_inbox.sql`
- Modify: `backend/src/devices/push.rs`
- Test: `backend/tests/push_inbox_test.rs` (crear)

**Interfaces:**
- Produces: tabla `device_push_inbox` con `status IN ('pending','done','failed')`. La Tarea 2 consume `pending` y escribe `done`/`failed`.

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
mod common;

/// C-10: el cuerpo crudo tiene que estar en disco ANTES de responder. Hoy el
/// handler procesa, traga errores y responde 200 igual.
#[tokio::test]
async fn a_push_is_persisted_before_it_is_acknowledged() {
    // POST con cuerpo valido -> 200, y existe una fila en device_push_inbox
    // con status='pending' y el cuerpo byte a byte
}

/// Dos pushes con cuerpo IDENTICO producen DOS filas. El inbox no deduplica:
/// un falso positivo perderia una marcacion real. La deduplicacion vive aguas
/// abajo, en bucket_30s de attendance_events.
#[tokio::test]
async fn identical_bodies_are_both_stored() {
    // mismo cuerpo dos veces -> COUNT(*) == 2
}

/// Token invalido no escribe nada.
#[tokio::test]
async fn an_unauthorized_push_stores_nothing() {
    // token malo -> 401 y COUNT(*) == 0
}
```

- [ ] **Step 2: Correr y ver que fallan**

Run: `cargo nextest run --all-features -E 'binary(push_inbox_test)'`
Expected: fallan — la tabla no existe.

- [ ] **Step 3: La migración**

Crear `backend/src/db/migrations/027_device_push_inbox.sql`:

```sql
-- C-10: el receptor push respondia 200 aunque la ingesta fallara, y el lector
-- avanzaba su cola dando el evento por entregado. Un fallo de base perdia la
-- marcacion para siempre.
--
-- El inbox guarda el cuerpo crudo antes de confirmar. Es deliberadamente
-- tonto: no interpreta, no valida, no deduplica.
--
-- SIN indice unico sobre body_sha256: dos cuerpos identicos pueden ser dos
-- eventos legitimos (los heartbeats lo son casi siempre) y un falso positivo
-- de deduplicacion perderia una marcacion real — exactamente el fallo que esta
-- tabla existe para impedir. La deduplicacion ya vive en bucket_30s de
-- attendance_events.
CREATE TABLE IF NOT EXISTS device_push_inbox (
    id            TEXT PRIMARY KEY,
    device_id     TEXT NOT NULL REFERENCES devices(id),
    content_type  TEXT NOT NULL,
    body          BLOB NOT NULL,
    body_sha256   TEXT NOT NULL,          -- diagnostico y correlacion, NO clave de dedup
    received_at   INTEGER NOT NULL,       -- epoch seconds UTC
    status        TEXT NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending', 'done', 'failed')),
    attempts      INTEGER NOT NULL DEFAULT 0,
    last_error    TEXT,
    processed_at  INTEGER
);

-- El drenador busca pendientes por orden de llegada.
CREATE INDEX IF NOT EXISTS idx_push_inbox_pending
    ON device_push_inbox(received_at)
    WHERE status = 'pending';

-- La cola muerta se consulta por separado y debe ser barata.
CREATE INDEX IF NOT EXISTS idx_push_inbox_failed
    ON device_push_inbox(received_at)
    WHERE status = 'failed';
```

- [ ] **Step 4: Persistir antes del ACK**

En `backend/src/devices/push.rs`, tras `authorize` y antes de cualquier parseo:

```rust
    // C-10: guardar primero, interpretar despues. Todo lo que ocurra a partir
    // de aqui puede fallar sin perder el evento.
    let body_sha256 = hex::encode(Sha256::digest(&body));
    let inbox_id = Uuid::new_v4().to_string();
    let stored = state
        .db_write
        .statement(
            "push.inbox-store",
            "INSERT INTO device_push_inbox \
               (id, device_id, content_type, body, body_sha256, received_at, status, attempts) \
             VALUES (?1, ?2, ?3, ?4, ?5, unixepoch(), 'pending', 0)",
            vec![
                libsql::Value::Text(inbox_id.clone()),
                libsql::Value::Text(device_id.clone()),
                libsql::Value::Text(content_type.clone()),
                libsql::Value::Blob(body.to_vec()),
                libsql::Value::Text(body_sha256),
            ],
        )
        .await;

    if let Err(error) = stored {
        // Fallo de DURABILIDAD, no de procesamiento. Aqui SI respondemos
        // no-200 a proposito: el device conserva el evento y reintenta, que es
        // contrapresion correcta cuando la base no puede aceptarlo. Responder
        // 200 sin haber guardado es la mentira que causo C-10.
        tracing::error!(device_id = %device_id, err = %error, "push inbox store failed");
        return Ok(StatusCode::SERVICE_UNAVAILABLE);
    }
```

El procesamiento existente **se mantiene tal cual** por ahora: sigue tragando errores y respondiendo `ACK`. La Tarea 2 lo saca de aquí.

Añade `sha2` y `hex` a `Cargo.toml` si no están; `md-5` ya está, así que revisa primero qué hay disponible antes de añadir nada.

- [ ] **Step 5: Verificar**

Run: `cargo nextest run --all-features && cargo clippy --all-targets --all-features -- -D warnings && python3 scripts/check_db_write_queue.py && make coverage-backend`

- [ ] **Step 6: Commit**

```bash
git add backend/src/db/migrations/027_device_push_inbox.sql backend/src/devices/push.rs backend/tests/push_inbox_test.rs
git commit -m "feat(devices): persist raw pushes before acknowledging them (C-10)"
```

---

### Task 2: El procesamiento sale del handler y gana reintentos (C-10, parte 2)

Con el cuerpo a salvo, el parseo y la ingesta dejan de ocurrir en la ruta de respuesta. Un worker drena el inbox, reintenta lo que falla de forma transitoria y deja visible lo que no.

**Files:**
- Create: `backend/src/workers/push_drain.rs`
- Modify: `backend/src/workers/mod.rs`, `backend/src/main.rs`, `backend/src/devices/push.rs`, `backend/src/devices/handlers.rs`
- Test: `backend/tests/push_drain_test.rs` (crear)

**Interfaces:**
- Consumes: `device_push_inbox` de la Tarea 1; `isapi::ingest::ingest_alert` y `split_payload`.
- Produces: `workers::push_drain::run(state, shutdown)`; endpoint `GET /api/v1/devices/push-inbox/failed`.

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
mod common;

/// El worker convierte un pendiente en un evento de asistencia y marca la fila
/// como procesada.
#[tokio::test]
async fn the_drainer_turns_a_pending_row_into_an_attendance_event() { }

/// Un cuerpo que no se puede parsear NUNCA se reintenta en bucle: se marca
/// 'failed' y queda visible. Reintentar un XML invalido no lo arregla.
#[tokio::test]
async fn an_unparseable_body_lands_in_the_dead_letter_queue() { }

/// Un fallo transitorio SI se reintenta, y el contador de intentos sube.
#[tokio::test]
async fn a_transient_failure_is_retried_and_counted() { }

/// Ningun evento aceptado se pierde: tras drenar, todo pendiente quedo en
/// 'done' o en 'failed'. Ninguno se queda en 'pending' en silencio.
#[tokio::test]
async fn every_accepted_push_ends_up_done_or_failed_never_lost() { }
```

- [ ] **Step 2: Correr y ver que fallan**

Run: `cargo nextest run --all-features -E 'binary(push_drain_test)'`

- [ ] **Step 3: El worker**

Crear `backend/src/workers/push_drain.rs`, imitando la forma de `backend/src/workers/capture_cleanup.rs` (léelo antes: cadencia, señal de apagado, manejo de errores).

El bucle, en pseudocódigo — la implementación real va en Rust siguiendo el patrón del archivo vecino:

```
cada N segundos, o al recibir una señal de "hay trabajo":
  seleccionar hasta K filas 'pending' por received_at ascendente
  para cada fila:
    split_payload(content_type, body)     // el parser tolerante existente
    para cada parte:
      ingest_alert(...)                    // el traductor de isapi, SIN cambios
    si todo fue bien           -> status='done', processed_at=unixepoch()
    si el fallo NO es transitorio (parseo, evento desconocido)
                               -> status='failed', last_error=<motivo>
    si el fallo es transitorio (base ocupada)
                               -> attempts += 1, sigue 'pending'
                                  y si attempts >= MAX -> 'failed'
```

**La distinción entre transitorio y permanente es el corazón de esta tarea.** Reintentar un XML inválido para siempre es un bucle infinito que además tapa la cola; marcar como definitivo un fallo por base ocupada pierde el evento igual que antes. Si no puedes clasificar un error con certeza, trátalo como **transitorio** hasta agotar `MAX_ATTEMPTS` y solo entonces como definitivo: equivocarse hacia el reintento cuesta trabajo, equivocarse hacia el descarte cuesta una marcación.

**Frontera hexagonal:** el worker vive en `workers/`, pero `split_payload` e `ingest_alert` son del adaptador `isapi`. El worker los invoca; **no** replica su lógica ni interpreta el cuerpo por su cuenta. `RawMarking` se sigue construyendo solo dentro de `isapi/ingest.rs`.

- [ ] **Step 4: Sacar el procesamiento del handler**

En `push.rs`, eliminar el bucle de `split_payload`/`ingest_alert` de `receive_push`. Tras guardar en el inbox, responder `ACK` y ya. Actualizar el comentario de `ACK` para reflejar el nuevo reparto: 200 significa "recibido y a salvo", no "procesado".

- [ ] **Step 5: Lanzar el worker**

En `main.rs`, junto a los demás (`capture_cleanup_handle`, línea ~206), con la misma señal de apagado.

- [ ] **Step 6: Hacer visible la cola muerta**

Un `GET /api/v1/devices/push-inbox/failed` que liste las filas `failed` con `device_id`, `received_at`, `attempts` y `last_error`. Rol mínimo: `supervisor`.

**No devuelvas el `body` crudo en la lista** — puede contener un JPEG de un rostro, y esta es una ruta de diagnóstico, no de biometría. Si hace falta inspeccionarlo, que sea otro endpoint con rol `admin` y su propia entrada de auditoría.

- [ ] **Step 7: Verificar y commitear**

Run: `cargo nextest run --all-features && cargo clippy --all-targets --all-features -- -D warnings && python3 scripts/check_db_write_queue.py && make coverage-backend`

```bash
git add backend/src/workers/ backend/src/main.rs backend/src/devices/ backend/tests/push_drain_test.rs
git commit -m "feat(devices): drain the push inbox asynchronously with retries and a DLQ (C-10)"
```

---

### Task 3: El reporte acota su universo y no muere por una fila (I5, I7)

Dos hallazgos de la revisión de rama, en el mismo archivo.

**I5 — el universo incluye a todo empleado desactivado alguna vez.** Quitar `e.status = 'active'` fue correcto para el caso grave (un empleado que egresó a mitad del período desaparecía con sus días trabajados), pero ese filtro estaba acotando implícitamente el universo. Ahora alguien desactivado en 2024 produce una fila de ceros en todos los reportes de 2026.

**I7 — una fila mala tumba el reporte entero.** `require_salary_kind` (`reports/service.rs:47`) devuelve `AppError::CalcError` y se propaga con `?` en la línea 395, así que **un** empleado con `salary_kind` NULL da 500 para *todos*. Un `hire_date` faltante, en cambio, solo levanta una anomalía por fila. La inconsistencia sugiere que no fue una decisión.

**Files:**
- Modify: `backend/src/reports/service.rs`
- Test: `backend/tests/reports_universe_test.rs` (crear)

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
mod common;

/// I5: quien egreso antes del periodo no debe aparecer.
#[tokio::test]
async fn an_employee_terminated_before_the_period_does_not_appear() { }

/// I5: quien ingreso despues del periodo tampoco.
#[tokio::test]
async fn an_employee_hired_after_the_period_does_not_appear() { }

/// I5: quien estuvo vigente parte del periodo SI aparece — este es el caso
/// que no puede romperse al acotar.
#[tokio::test]
async fn an_employee_whose_employment_overlaps_the_period_appears() { }

/// I7: un empleado con salary_kind NULL no puede tumbar el reporte de los
/// demas. Debe salir como anomalia en SU fila, igual que hire_date faltante.
#[tokio::test]
async fn one_employee_without_salary_kind_does_not_break_the_whole_report() { }
```

- [ ] **Step 2: Correr y ver que fallan**

Run: `cargo nextest run --all-features -E 'binary(reports_universe_test)'`

- [ ] **Step 3: Acotar el universo (I5)**

Añadir a los predicados del reporte la condición de que la relación laboral **intersecte** el período:

```sql
  AND (e.terminated_on IS NULL OR e.terminated_on >= ?from)
  AND (e.hire_date     IS NULL OR e.hire_date     <= ?to)
```

Cuidado con dos cosas: estos predicados son sobre `e.*`, no sobre `dr.*`, así que van en el `WHERE` sin degradar el `LEFT JOIN` — a diferencia de `shift_type`, que tuvo que moverse al `ON` por esa razón exacta. Y el `NULL` significa "sin límite por ese lado", no "excluir".

- [ ] **Step 4: Degradar el error de unidad salarial a anomalía (I7)**

`require_salary_kind` deja de devolver `Result` y pasa a devolver `Option<SalaryKind>`. Cuando falta:

- la fila no monetiza (importes en cero), igual que hoy hace un día sin registro;
- se añade `SALARY_KIND_MISSING` a los `anomaly_codes` de esa fila, del mismo modo que `HIRE_DATE_MISSING`;
- el resto del reporte se calcula normalmente.

**No inventes una unidad por defecto.** Asumir `Daily` para seguir adelante reintroduce H-08 exactamente como estaba. Cero e anomalía visible es lo correcto: es evidente que falta un dato, y nadie cobra un importe inventado.

- [ ] **Step 5: Verificar y commitear**

Run: `cargo nextest run --all-features && cargo clippy --all-targets --all-features -- -D warnings && make coverage-backend`

```bash
git add backend/src/reports/service.rs backend/tests/reports_universe_test.rs
git commit -m "fix(reports): bound the universe to employment overlapping the period and stop one bad row failing all (I5, I7)"
```

---

### Task 4: Asertar las rutas monetarias que nada mira (I6)

Este es el hallazgo más importante de los tres, aunque no cambie una línea de producción.

Corregir C-01 —la hora extra al 250%— exigió tocar **un solo** test. Todo lo demás siguió verde. Eso significa que **ningún test asertaba el total del reporte para un día con horas extra**: el defecto no sobrevivió a 1096 pruebas porque miraran mal, sino porque no miraban. La misma ausencia mantuvo viva la composición aditiva hasta que una revisión de rama la calculó a mano.

Huecos concretos, verificados:

- ningún aserto de total de reporte con **prima nocturna**
- ninguno con **recargo dominical**
- ninguno con **hora extra en día con recargo**
- de las cinco funciones de `money.rs`, solo `work_pay_cents` tiene test de valor para `Monthly` y `Hourly`; las otras cuatro pueden regresar a dos divisiones y pasar en verde
- `reports_employment_window_test.rs` asierta `total_a_pagar_cents > 0` donde conoce el valor exacto

**Files:**
- Test: `backend/tests/reports_money_paths_test.rs` (crear)
- Modify: `backend/tests/reports_employment_window_test.rs` (endurecer el aserto flojo)

- [ ] **Step 1: Escribir la matriz de casos**

Todos los importes **calculados a mano** desde la LOTTT, nunca leídos de la implementación. Base: salario diario 5 000 céntimos, jornada 480 min. Recalcula cada uno antes de escribirlo:

| Caso | Composición | Esperado |
|---|---|---|
| Jornada completa | 480 ord | 5 000 |
| + 60 min extra | 5 000 + 60×1,5 | 5 938 |
| Nocturno completo | 480 × 1,3 | 6 500 |
| Nocturno + 60 extra | 5 000 + 1 500 + 60 extra a 1,95× | 7 719 |
| Domingo completo | 480 × 1,5 | 7 500 |
| Domingo + 60 extra | 5 000 + 2 500 + 60 extra a 2,25× | 8 906 |
| Mensual 150 000, jornada completa | 150 000/30 | 5 000 |
| Por hora 625, jornada completa | 625 × 8 | 5 000 |

Los tres últimos importes deben coincidir con el caso diario equivalente: la unidad no puede cambiar lo que se paga por el mismo trabajo. **Ese es el aserto más valioso de la tabla** — es el que habría cazado H-08 solo.

Cubrir además, para cada una de las cinco funciones de `money.rs`, un valor con `Monthly` y otro con `Hourly`, para que una regresión a dos divisiones no pase en verde.

- [ ] **Step 2: Correr y ver cuáles fallan**

Run: `cargo nextest run --all-features -E 'binary(reports_money_paths_test)'`

Expected: **deberían pasar todos.** El código es correcto tras el plan monetario; estas pruebas fijan ese comportamiento. Si alguna falla, has encontrado un defecto real que nadie había visto — para y repórtalo antes de tocar nada, porque es más valioso que la tarea.

- [ ] **Step 3: Endurecer el aserto flojo**

En `reports_employment_window_test.rs`, sustituir `assert!(total_a_pagar_cents > 0)` por el valor exacto que ese test ya conoce (`200_000`). Un `> 0` sobre un valor conocido no prueba nada.

- [ ] **Step 4: Commit**

```bash
git add backend/tests/reports_money_paths_test.rs backend/tests/reports_employment_window_test.rs
git commit -m "test(reports): assert the money paths nothing was asserting (I6)"
```

---

### Task 5: Que el piso de cobertura del frontend mida algo

`frontend/vitest.config.ts` declara un piso por archivo así:

```ts
thresholds: {
  lines: 90, branches: 85, functions: 90, statements: 90,
  '**/*.{ts,tsx}': { lines: 70, branches: 60, functions: 70, statements: 70 },
}
```

Parece correcto y **no lo es**: Vitest evalúa un threshold con patrón glob sobre el **agregado** de los archivos que coinciden, no archivo por archivo. Como el glob abarca todo, ese "piso por archivo" mide prácticamente lo mismo que el gate de proyecto, y con umbrales más bajos, de modo que nunca ata. Archivos por debajo del piso salen con exit 0 — comprobado.

`perFile: true` no lo arregla: aplicaría los umbrales de proyecto (90/85) a cada archivo, mucho más estricto que la política que se quiso. La combinación "proyecto 90, archivo 70" no es expresable en la configuración de Vitest.

Esto no es teórico: es la razón por la que `frontend/src/app/**` albergó dos defectos —el formulario de edición sin la unidad salarial, y la tarjeta que decía "(−) resta al neto" sobre un número que no restaba— sin que ningún gate los notara.

El backend ya resuelve esto con un post-procesador (`scripts/enforce-coverage-floor.sh` sobre `lcov.info`). El frontend ya emite el insumo necesario: `reporter: [..., 'json-summary']`.

**Nota:** existe `scripts/enforce-owned-coverage.mjs` con umbrales por archivo idénticos a los que buscamos, pero está acotado a un plan concreto (`--manifest`, `--expected-plan`) y **nada lo invoca**. Léelo antes de escribir el nuevo: puede que te sirva de base, y en todo caso conviene saber por qué existe.

**Files:**
- Create: `scripts/enforce-frontend-file-floor.mjs`
- Modify: `Makefile`, `.github/workflows/ci.yml`, `frontend/vitest.config.ts`

- [ ] **Step 1: Escribir el enforcer**

`scripts/enforce-frontend-file-floor.mjs` lee `frontend/coverage/coverage-summary.json` y falla con exit 1 si algún archivo bajo el `include` del config queda por debajo de 70/60/70/70, imprimiendo una línea `FAIL:` por archivo — el mismo formato que usa el enforcer del backend, para que un log rojo se lea igual en ambos lados.

Debe respetar las mismas exclusiones que `vitest.config.ts` (`src/components/ui/**`, `providers.tsx`, `top-bar.tsx`, `access-restricted.tsx`, tests y `.d.ts`). Duplicar esa lista en dos sitios es cómo se desincronizan: léela del propio config si puedes, y si no, deja un comentario en **ambos** archivos advirtiéndolo.

- [ ] **Step 2: Probar que el enforcer detecta lo que debe**

Corre el enforcer contra el `coverage-summary.json` actual. Debería **fallar**, señalando los archivos que la revisión de rama identificó por debajo del piso (`timesheet/row-actions.tsx`, `lib/format/datetime.ts`).

Si pasa a la primera, el enforcer no está midiendo nada y hay que arreglarlo antes de seguir. Un gate que nunca has visto fallar no es un gate.

- [ ] **Step 3: Decidir qué hacer con los archivos que ya están por debajo**

Ahora el enforcer está rojo por deuda preexistente. Hay dos salidas legítimas y una ilegítima:

- **subir la cobertura** de esos archivos hasta el piso — preferible;
- **excluirlos explícitamente** con justificación escrita en `CLAUDE.md`, como exige la política de exclusiones del proyecto;
- ~~bajar el piso hasta que pasen~~ — eso es volver a tener un gate decorativo con otra forma.

Elige por archivo y documenta cuál elegiste y por qué.

- [ ] **Step 4: Conectar el enforcer**

Añadirlo al target de cobertura del frontend en el `Makefile` y al job `Frontend Coverage` de `.github/workflows/ci.yml`, después de `vitest run --coverage`.

Y en `frontend/vitest.config.ts`, sustituir el bloque glob por un comentario que diga por qué ya no está ahí y a dónde se movió — si lo dejas, el siguiente lector creerá que el piso se aplica dos veces.

- [ ] **Step 5: Verificar el ciclo completo**

Baja a propósito la cobertura de un archivo (comenta un test), corre el target y comprueba que **falla nombrando ese archivo**. Restaura y comprueba que pasa. Reporta ambas salidas.

- [ ] **Step 6: Commit**

```bash
git add scripts/enforce-frontend-file-floor.mjs Makefile .github/workflows/ci.yml frontend/vitest.config.ts
git commit -m "ci(frontend): enforce the per-file coverage floor that the glob never applied"
```

---

## Fuera de alcance

- **H-07** — las anulaciones no recalculan las horas extra ni validan combinaciones imposibles.
- **H-09** — sin vigencia histórica ni cierre de período: un cambio de salario hoy altera retroactivamente un período ya emitido.
- **H-04** — descanso sábado/domingo cableado, sin feriados.
- **Los 26 hallazgos de auditoría sin verificar.** Ninguno entra a un plan sin comprobarse primero: el muestreo dio 14/14 reales, pero dos estaban descritos de forma que inducía a un arreglo equivocado.
- **Rotación de la clave de licencia** — diferida por decisión del dueño del producto; vence antes de emitir la primera licencia real. Ver `docs/runbooks/rotacion-clave-licencia.md`.

## Gates que existen y no corren

Encontrados durante este trabajo, todos con la misma forma: protección escrita, documentada, y nunca invocada. La Tarea 5 cierra el tercero.

1. `deploy/tests/gateway-config-test.sh` — nada lo invoca. Es el test que habría cazado la fuga de `error_log` de nginx.
2. Ningún job de CI corre los tests de `do-functions` — la prueba de activación concurrente de C-07 solo corre a mano.
3. El piso por archivo del frontend (Tarea 5).
4. `scripts/enforce-owned-coverage.mjs` — escrito, con los umbrales correctos, sin invocar.
