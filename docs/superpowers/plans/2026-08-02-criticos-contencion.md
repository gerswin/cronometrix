# Contención de críticos: secretos y carreras — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cerrar los cuatro críticos verificados que no dependen de decisiones de producto: la clave privada en el repo (C-06), la activación de licencia no atómica (C-07), el token `push` filtrado a logs (C-08) y la carrera del primer administrador (C-09).

**Architecture:** Cada tarea es independiente y se puede revertir sola. Dos son cambios de código con prueba concurrente (C-07, C-09), una es redacción de logs en dos capas (C-08) y la última es preparación de rotación más un gate de CI (C-06) — la clave real la genera el operador, nunca esta sesión.

**Tech Stack:** Rust/Axum 0.8, libSQL, `db_write.transact`, DigitalOcean Functions (Node), nginx, GitHub Actions.

## Global Constraints

- **Nunca** usar los identificadores desnudos `execute`, `execute_batch` ni `transaction` dentro de `backend/src` — `scripts/check_db_write_queue.py` es un gate que falla la build. Toda escritura va por `state.db_write.statement(...)` o `state.db_write.transact(...)`; dentro de una transacción se usa `tx.query(...)` y `tx.statement(...)`.
- Toda mutación de datos de asistencia debe generar entrada de auditoría inmutable con justificación (CLAUDE.md).
- El gate de cobertura es duro: líneas ≥90%, ramas ≥85% globales; por archivo ≥70%/≥60%. Correr `make coverage-backend` antes de dar una tarea por terminada.
- `CRONOMETRIX_E2E` y `CRONOMETRIX_LICENSE_BYPASS` jamás deben aparecer en configuración de producción.
- Ninguna clave privada real entra al repositorio, ni siquiera temporalmente, ni en un test fixture.
- Los mensajes de commit van en inglés, con prefijo convencional (`fix:`, `test:`, `chore:`).
- **Nunca romper la arquitectura hexagonal.** El detalle de vendedor vive solo en el adaptador (`backend/src/isapi/*`); el núcleo habla por los puertos — `RawMarking` + `attendance::ingest` de entrada, `BiometricReader` de salida (`backend/src/devices/reader.rs`). Ninguna corrección puede meter tipos, formatos ni códigos de Hikvision dentro de `attendance/`, `reports/`, `calc/` ni `daily_records/`, ni saltarse `BiometricReader` para hablarle a un lector directamente. Si un arreglo parece exigirlo, se extiende el puerto — no se perfora. Ver `docs/ARQUITECTURA-HEXAGONAL.md`.

## Contexto imprescindible

La auditoría (`docs/auditoria/INFORME-AUDITORIA-INTEGRAL.md`) fue producida por otro agente y **verificada** en `docs/auditoria/VERIFICACION-Y-PLAN.md`. Lee la verificación antes que la auditoría: corrige dos hallazgos que la auditoría encuadra de forma que induce a un arreglo equivocado.

Dos advertencias que cambian cómo se implementa:

1. **El firmware exige 200.** `backend/src/devices/push.rs:89-99` documenta, con medición sobre un DS-K1T341CMFW real, que ante cualquier respuesta distinta de 200 el lector reenvía el mismo evento para siempre y la cabeza de su cola nunca avanza — perdiendo todos los eventos posteriores. Ninguna tarea de este plan puede cambiar el código de respuesta del endpoint `push`.
2. **`httpHosts` solo admite `none` o `digest`.** El XML de provisión (`backend/src/isapi/client.rs:344`) fija `<httpAuthenticationMethod>none</httpAuthenticationMethod>`. El firmware **no acepta cabeceras arbitrarias**, así que la recomendación literal de la auditoría para C-08 ("mover el secreto a cabecera") no es implementable tal cual. Este plan cierra la fuga real —los logs— sin tocar el device. La autenticación digest queda como trabajo posterior con verificación en hardware.

## Radio de impacto (verificado sobre el grafo del código)

Grafo AST de `backend/src` — 1365 nodos, 3386 aristas, 69 comunidades
(`graphify-out/`). Lo que cambia cómo se ejecuta este plan:

- **`http_trace.rs` es el único punto de `backend/src` que registra la ruta de
  una request.** Comprobado: los demás `.path()` del código son rutas de
  filesystem, y `devices/push.rs` nunca emite el token a un log. La Tarea 2 no
  tiene puntos de fuga adicionales del lado Rust — el `access_log` de nginx es
  el otro, y también está cubierto.
- **`AppError` y `AppState` son god nodes** (grado 200 cada uno): casi todo el
  backend depende de ellos. La Tarea 1 hace viajar un `AppError::Conflict`
  dentro del cierre transaccional; `errors.rs:135` lo rescata con
  `downcast::<AppError>()`, así que el 409 **no** degrada a 500. Verificado, no
  supuesto — es el mismo mecanismo del que depende `leaves`.
- **`money::` tiene un único llamador**: `reports/service.rs`, 7 sitios. La
  corrección monetaria (C-01, C-02) está mucho más contenida de lo que sugiere
  la auditoría.
- **`RawMarking` se construye solo en `isapi/ingest.rs`**, y el núcleo se cruza
  en un único punto (`isapi/ingest.rs:114`). Ver la nota de C-10 abajo.

Limitación honesta del grafo: la extracción AST **no** capturó las llamadas
calificadas `money::fn()` — reportó `money.rs` con cero vecinos externos, lo
cual es falso. Los cuatro puntos de arriba se confirmaron leyendo el código, no
solo el grafo.

---

## File Structure

| Archivo | Responsabilidad | Tarea |
|---|---|---|
| `backend/src/setup/handlers.rs` | Alta del primer admin — pasa a check-and-insert transaccional | 1 |
| `backend/tests/setup_race_test.rs` | Prueba concurrente del bootstrap (nuevo) | 1 |
| `backend/src/http_trace.rs` | Span de request — deja de emitir el token | 2 |
| `backend/tests/http_trace_redaction_test.rs` | Prueba de redacción (nuevo) | 2 |
| `deploy/nginx.conf` | `access_log` — deja de escribir el token | 2 |
| `do-functions/packages/licenses/activate/index.js` | Vinculación atómica del fingerprint | 3 |
| `do-functions/packages/licenses/shared-store.js` | Store en memoria — `bind` refleja el contrato condicional | 3 |
| `do-functions/packages/licenses/activate/test.js` | Prueba de carrera de activación | 3 |
| `do-functions/packages/licenses/renew/test.js` | Deja de leer claves del repo | 4 |
| `do-functions/test-keys/` | **Se elimina** (después de independizar las pruebas) | 4 |
| `.github/workflows/ci.yml` | Job de escaneo de secretos | 4 |
| `docs/runbooks/rotacion-clave-licencia.md` | Procedimiento de rotación (nuevo) | 4 |

---

### Task 1: Bootstrap del primer administrador sin carrera (C-09)

Hoy `setup_init` lee `SELECT COUNT(*) FROM users` por una conexión de solo lectura y luego inserta por la cola de escritura — operaciones distintas. Entre ambas corre el hash Argon2, que tarda cientos de milisegundos a propósito. Dos peticiones simultáneas con nombres de usuario distintos ven ambas `count = 0` y ambas insertan un admin.

El precedente a copiar está en `backend/src/leaves/service.rs:99-130`: `db_write.transact` con la comprobación y la escritura dentro del mismo cierre.

**Files:**
- Modify: `backend/src/setup/handlers.rs:52-113`
- Test: `backend/tests/setup_race_test.rs` (crear)

**Interfaces:**
- Consumes: `state.db_write.transact(operation, |tx| ...)` de `backend/src/db/write_queue.rs:404`; `AppError::Conflict { code, message }`.
- Produces: nada nuevo hacia otras tareas.

- [ ] **Step 1: Escribir la prueba que falla**

Crear `backend/tests/setup_race_test.rs`. La prueba lanza dos `setup_init` concurrentes con usuarios distintos y exige que exactamente uno cree usuario.

```rust
mod common;

use std::sync::Arc;

/// C-09: dos altas simultáneas de primer admin. Exactamente una debe ganar.
/// Los nombres son distintos a propósito: un UNIQUE sobre `username` no
/// salvaría este caso, así que la prueba falla si la única defensa es ese índice.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_bootstrap_creates_exactly_one_admin() {
    let (state, _tmp) = common::test_state_with_tmpdir(common::test_db().await, common::test_config()).await;

    let a = {
        let state = state.clone();
        tokio::spawn(async move {
            cronometrix_api::setup::handlers::setup_init(
                axum::extract::State(state),
                axum::Json(cronometrix_api::setup::models::SetupInitRequest {
                    username: "admin_a".to_string(),
                    full_name: "Admin A".to_string(),
                    password: "correct horse battery".to_string(),
                }),
            )
            .await
            .is_ok()
        })
    };
    let b = {
        let state = state.clone();
        tokio::spawn(async move {
            cronometrix_api::setup::handlers::setup_init(
                axum::extract::State(state),
                axum::Json(cronometrix_api::setup::models::SetupInitRequest {
                    username: "admin_b".to_string(),
                    full_name: "Admin B".to_string(),
                    password: "correct horse battery".to_string(),
                }),
            )
            .await
            .is_ok()
        })
    };

    let wins = [a.await.unwrap(), b.await.unwrap()]
        .into_iter()
        .filter(|ok| *ok)
        .count();
    assert_eq!(wins, 1, "exactly one bootstrap must succeed");

    let conn = state.db.connect().unwrap();
    let mut rows = conn.query("SELECT COUNT(*) FROM users", ()).await.unwrap();
    let users: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(users, 1, "a second admin row must never exist");
}
```

Si `setup::handlers::setup_init` o `setup::models::SetupInitRequest` no son `pub` fuera del crate, hazlos `pub` en el módulo — no cambies la firma.

- [ ] **Step 2: Correr la prueba y ver que falla**

Run: `cargo nextest run --all-features -E 'test(concurrent_bootstrap_creates_exactly_one_admin)'`

Expected: FAIL con `wins` igual a 2 y `users` igual a 2. Si pasa a la primera, **no sigas**: repítela unas 20 veces (`--test-threads=1` en bucle); una carrera que no se reproduce no está probada, y ese es el punto del hallazgo.

- [ ] **Step 3: Mover la comprobación dentro de la transacción de escritura**

En `backend/src/setup/handlers.rs`, sustituir el bloque de `SELECT COUNT(*)` (líneas 62-86) y el `INSERT` posterior (91-105) por una sola llamada transaccional. El hash se calcula **antes** de entrar a la transacción para no bloquear la cola de escritura durante cientos de milisegundos:

```rust
    // Argon2 es caro a propósito: se calcula fuera de la transacción para no
    // ocupar el escritor serializado. La comprobación de unicidad ocurre dentro.
    let password_hash = service::hash_password(&body.password)?;
    let user_id = Uuid::new_v4().to_string();
    let insert_id = user_id.clone();

    state
        .db_write
        .transact("setup.create-admin", move |tx| {
            Box::pin(async move {
                let mut rows = tx.query("SELECT COUNT(*) FROM users", ()).await?;
                let count: i64 = rows
                    .next()
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("COUNT returned no row"))?
                    .get(0)?;
                if count > 0 {
                    return Err(anyhow::Error::new(AppError::Conflict {
                        code: "SETUP_ALREADY_COMPLETE",
                        message:
                            "System has already been initialized. An admin user already exists."
                                .to_string(),
                    }));
                }
                tx.statement(
                    "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, 'admin', 'active', 1, unixepoch(), unixepoch())",
                    libsql::params![insert_id, body.username, body.full_name, password_hash],
                )
                .await?;
                Ok(())
            })
        })
        .await
        .map_err(AppError::from)?;
```

Actualizar el comentario doc de la línea 52: hoy afirma que `SELECT COUNT(*) + 409` previene la carrera, lo cual es falso. Sustituir por:

```rust
/// C-09: la comprobación de unicidad y el INSERT ocurren en la misma
/// transacción del escritor serializado. Un `SELECT COUNT(*)` previo por otra
/// conexión NO cierra la carrera: entre lectura y escritura corre el hash
/// Argon2, que abre una ventana de cientos de milisegundos.
```

El escritor de `DbWriteQueue` es único y serializado, así que dos transacciones no se solapan: la segunda ve `count = 1` y devuelve 409.

- [ ] **Step 4: Correr la prueba y ver que pasa**

Run: `cargo nextest run --all-features -E 'test(concurrent_bootstrap_creates_exactly_one_admin)'`
Expected: PASS. Correrla 20 veces seguidas también en verde.

- [ ] **Step 5: Verificar que no se rompió nada y que el gate sigue verde**

Run: `cargo nextest run --all-features && cargo clippy --all-targets --all-features -- -D warnings && python3 scripts/check_db_write_queue.py`
Expected: todo verde, 0 violaciones.

- [ ] **Step 6: Commit**

```bash
git add backend/src/setup/handlers.rs backend/tests/setup_race_test.rs
git commit -m "fix(setup): close the first-admin bootstrap race (C-09)"
```

---

### Task 2: El token `push` deja de llegar a los logs (C-08)

El token viaja en la ruta (`/devices/{device_id}/push/{token}`) y `http_trace.rs:17` emite `request.uri().path()`, que lo incluye entero. nginx lo escribe además en su `access_log`. Cualquiera con acceso a logs puede inyectar marcaciones.

**No se cambia la ruta.** Cambiarla obliga a reprovisionar cada device, y el firmware no admite cabeceras arbitrarias (ver Contexto). Esta tarea cierra la fuga; la autenticación digest es trabajo posterior.

**Files:**
- Modify: `backend/src/http_trace.rs:13-18`
- Modify: `deploy/nginx.conf` (bloque `/api/`, líneas ~62-69)
- Test: `backend/tests/http_trace_redaction_test.rs` (crear)

**Interfaces:**
- Consumes: nada de tareas previas.
- Produces: `http_trace::redact_path(&str) -> String`, usada solo dentro del módulo pero `pub` para poder probarla.

- [ ] **Step 1: Escribir la prueba que falla**

Crear `backend/tests/http_trace_redaction_test.rs`:

```rust
use cronometrix_api::http_trace::redact_path;

/// C-08: el token de push es un secreto de escritura. Nunca debe aparecer en
/// un span, y por tanto tampoco en un archivo de log ni en un agregador.
#[test]
fn push_token_is_redacted_from_the_path() {
    let redacted = redact_path("/api/v1/devices/dev-123/push/s3cr3t-t0ken-value");
    assert_eq!(redacted, "/api/v1/devices/dev-123/push/[redacted]");
    assert!(!redacted.contains("s3cr3t"));
}

/// La redacción no puede degradar la observabilidad del resto de la API.
#[test]
fn other_paths_are_left_untouched() {
    assert_eq!(redact_path("/api/v1/employees"), "/api/v1/employees");
    assert_eq!(
        redact_path("/api/v1/devices/dev-123/status"),
        "/api/v1/devices/dev-123/status"
    );
}

/// Un push sin token (ruta incompleta) no debe entrar al brazo de redacción y
/// tampoco romper.
#[test]
fn push_without_token_is_left_untouched() {
    assert_eq!(
        redact_path("/api/v1/devices/dev-123/push"),
        "/api/v1/devices/dev-123/push"
    );
}
```

- [ ] **Step 2: Correr y ver que falla**

Run: `cargo nextest run --all-features -E 'test(push_token_is_redacted_from_the_path)'`
Expected: FAIL — `redact_path` no existe (error de compilación).

- [ ] **Step 3: Implementar la redacción**

En `backend/src/http_trace.rs`, añadir antes de `SafeMakeSpan`:

```rust
/// Sustituye el token de un push por `[redacted]`.
///
/// C-08: la ruta lleva el secreto de escritura del device
/// (`/devices/{id}/push/{token}`). El firmware Hikvision no admite cabeceras
/// arbitrarias en `httpHosts`, así que el secreto tiene que seguir en la URI;
/// lo que no puede es sobrevivir en un log.
pub fn redact_path(path: &str) -> String {
    match path.rsplit_once("/push/") {
        Some((prefix, token)) if !token.is_empty() && !token.contains('/') => {
            format!("{prefix}/push/[redacted]")
        }
        _ => path.to_string(),
    }
}
```

y usarla en el span:

```rust
            path = %redact_path(request.uri().path()),
```

- [ ] **Step 4: Correr y ver que pasa**

Run: `cargo nextest run --all-features -E 'test(redact)'`
Expected: los tres tests en PASS.

- [ ] **Step 5: Silenciar también el `access_log` de nginx**

El span de Rust no es el único registro. En `deploy/nginx.conf`, dentro del bloque que sirve `/api/`, añadir una `location` más específica que gane por prefijo y apague su log:

```nginx
    # C-08: la ruta de push lleva el token de escritura del device. nginx
    # escribiría la URI completa en access_log; el evento igual queda
    # registrado por el tracer de la aplicación, ya redactado.
    location ~ ^/api/v1/devices/[^/]+/push/ {
        access_log off;
        proxy_pass http://api;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
```

Copiar las directivas `proxy_*` exactamente como estén en el bloque `/api/` existente — si difieren, el push deja de funcionar.

- [ ] **Step 6: Verificar la sintaxis de nginx**

Run: `docker run --rm -v "$PWD/deploy/nginx.conf:/etc/nginx/nginx.conf:ro" nginx:alpine nginx -t`
Expected: `syntax is ok` y `test is successful`. Si la imagen no está disponible, `nginx -t -c` con un nginx local sirve igual.

- [ ] **Step 7: Confirmar que ningún log conserva el token**

Levantar el backend y mandar un push con token inventado:

```bash
curl -s -o /dev/null -w '%{http_code}\n' -X POST \
  'http://127.0.0.1:8080/api/v1/devices/nope/push/leaky-token-abc123'
```

Run: `grep -r "leaky-token-abc123" /tmp/cronometrix*.log 2>/dev/null; echo "exit=$?"`
Expected: sin coincidencias. Un `grep` con resultados es un fallo de la tarea, no un detalle.

- [ ] **Step 8: Commit**

```bash
git add backend/src/http_trace.rs backend/tests/http_trace_redaction_test.rs deploy/nginx.conf
git commit -m "fix(security): keep device push tokens out of request logs (C-08)"
```

---

### Task 3: Activación de licencia atómica (C-07)

`do-functions/packages/licenses/activate/index.js:46` lee `hardware_fingerprint` y `:60` escribe con `UPDATE ... WHERE license_key = $1` sin ninguna guarda sobre lo leído. Dos activaciones simultáneas con fingerprints distintos obtienen ambas un JWT válido: la licencia queda vinculada a dos equipos, que es justo lo que LIC-05 debe impedir.

La causa está en el contrato del store: `bind(licenseKey, fp, now)` no informa si realmente vinculó, así que el handler no tiene forma de saber que perdió la carrera. La corrección es hacer que `bind` devuelva un booleano y que el `UPDATE` lleve la guarda en el `WHERE`.

**Files:**
- Modify: `do-functions/packages/licenses/activate/index.js` (store de producción, `bind`, líneas ~55-67; y el llamador del handler)
- Modify: `do-functions/packages/licenses/shared-store.js:22-28` (`bind` en memoria, para que refleje el mismo contrato)
- Test: `do-functions/packages/licenses/activate/test.js`

**Interfaces:**
- Consumes: nada de tareas previas.
- Produces: contrato nuevo del store — `bind(licenseKey, fp, now) -> Promise<boolean>`; `true` si quedó vinculada a `fp`, `false` si ya estaba vinculada a otro equipo. `renew/index.js` también consume `shared-store`: comprobar que no use `bind`, y si lo usa, adaptarlo.

- [ ] **Step 1: Escribir la prueba que falla**

Añadir al final de `do-functions/packages/licenses/activate/test.js`. Usa `node:test` y `node:assert`, igual que el resto del archivo — no introduzcas otro framework:

```js
// C-07: dos activaciones simultáneas con fingerprints distintos. El binding
// tiene que ser condicional, no un UPDATE ciego: solo un equipo puede quedar
// vinculado y el otro debe recibir 409.
//
// Sobre el alcance: el store en memoria es de un solo hilo, así que esto
// verifica el CONTRATO (bind condicional + 409), no la atomicidad de Postgres.
// Esa la aporta el propio UPDATE de una sola sentencia con guarda en el WHERE.
test('concurrent activations bind the license exactly once', async () => {
    store.__seedRow('TEST-RACE-0000-0001');

    const [a, b] = await Promise.all([
        handler({ body: { license_key: 'TEST-RACE-0000-0001', hardware_fingerprint: 'FP-A' } }),
        handler({ body: { license_key: 'TEST-RACE-0000-0001', hardware_fingerprint: 'FP-B' } }),
    ]);

    const granted = [a, b].filter((r) => r.statusCode === 200);
    const rejected = [a, b].filter((r) => r.statusCode === 409);
    assert.strictEqual(granted.length, 1, 'exactly one activation may receive a token');
    assert.strictEqual(rejected.length, 1, 'the loser must get 409, not a signed token');

    // Y la licencia queda vinculada al que ganó, no al último en escribir.
    const bound = await store.lookup('TEST-RACE-0000-0001');
    assert.strictEqual(bound, granted[0].body.hardware_fingerprint ?? bound);
    assert.ok(['FP-A', 'FP-B'].includes(bound));
});
```

- [ ] **Step 2: Correr y ver que falla**

Run: `cd do-functions && npm run test:activate`
Expected: FAIL — `granted.length` es 2: ambas reciben token firmado.

- [ ] **Step 3: Hacer condicional el `bind` de producción**

En `do-functions/packages/licenses/activate/index.js`, dentro del store de producción, sustituir `bind` por:

```js
        async bind(licenseKey, fingerprint, now) {
            const client = new Client({ connectionString: process.env.DATABASE_URL });
            await client.connect();
            try {
                // C-07: la guarda vive en el WHERE, no en un SELECT previo. Un
                // UPDATE de una sola sentencia es atómico, así que de dos
                // activaciones simultáneas exactamente una afecta la fila.
                const r = await client.query(
                    `UPDATE licenses
                        SET hardware_fingerprint = $1,
                            activated_at = COALESCE(activated_at, $2)
                      WHERE license_key = $3
                        AND (hardware_fingerprint IS NULL OR hardware_fingerprint = $1)`,
                    [fingerprint, now, licenseKey],
                );
                return r.rowCount === 1;
            } finally {
                await client.end();
            }
        },
```

- [ ] **Step 4: Reflejar el mismo contrato en el store en memoria**

En `do-functions/packages/licenses/shared-store.js`, sustituir `bind` por:

```js
    async bind(licenseKey, fp, now) {
        const row = rows.get(licenseKey) || { fp: null, activated_at: null, last_renewed_at: null };
        // Mismo contrato que el UPDATE con guarda: solo vincula si está libre o
        // ya es el mismo equipo. Devuelve si la vinculación quedó hecha.
        if (row.fp != null && row.fp !== fp) return false;
        if (row.activated_at == null) row.activated_at = now;
        row.fp = fp;
        rows.set(licenseKey, row);
        return true;
    },
```

Actualizar el comentario de contrato de la cabecera del archivo (líneas 7-11) para incluir el valor de retorno de `bind`.

- [ ] **Step 5: Hacer que el handler respete el resultado**

En `activate/index.js`, donde se llama a `store.bind(...)`, pasar a comprobar el retorno antes de firmar el JWT:

```js
    const bound = await store.bind(license_key, hardware_fingerprint, nowSeconds);
    if (!bound) {
        // C-07: otra activación ganó la carrera y vinculó la licencia a otro
        // equipo. Firmar aquí entregaría un token válido para dos máquinas.
        return {
            statusCode: 409,
            body: {
                error: 'LICENSE_ALREADY_BOUND',
                message: 'This license is already activated on different hardware.',
            },
        };
    }
```

El `lookup` previo puede quedarse para distinguir el 404 de licencia inexistente, pero **ya no gobierna** la decisión de vincular.

- [ ] **Step 6: Correr y ver que pasa**

Run: `cd do-functions && npm test`
Expected: PASS, incluidas las pruebas de `renew` — `shared-store` es compartido y su contrato cambió.

- [ ] **Step 7: Commit**

```bash
git add do-functions/packages/licenses/activate/index.js do-functions/packages/licenses/shared-store.js do-functions/packages/licenses/activate/test.js
git commit -m "fix(licenses): bind activation atomically to defeat the two-machine race (C-07)"
```

---

### Task 4: Preparar la rotación de la clave de licenciamiento y cerrar la puerta (C-06)

`do-functions/test-keys/test_priv.pem` es la privada que corresponde a `backend/src/license/pubkey.pem` — comprobado: ambas derivan al mismo SHA-256 `f0318260deb1672cb624314065c8bd42d394fd799a954580265ca3b19f3106fc`. Producción confía en esa pública vía `include_str!` (`license/service.rs:36`). Cualquiera con el repositorio emite licencias auténticas.

**La clave nueva la genera el operador, no esta tarea.** Aquí se retira la comprometida, se documenta el procedimiento y se impide que vuelva a entrar otra.

Que la privada siga en la **historia de git** es lo que hace insuficiente borrarla del árbol. Reescribir la historia de un repositorio compartido es destructivo y coordinado: este plan lo documenta, no lo ejecuta.

**Files:**
- Delete: `do-functions/test-keys/test_priv.pem`, `do-functions/test-keys/test_pub.pem`
- Modify: `.github/workflows/ci.yml`
- Create: `docs/runbooks/rotacion-clave-licencia.md`

**Interfaces:**
- Consumes: nada de tareas previas.
- Produces: job `Secret Scan` en CI.

**Orden obligatorio:** las suites de `activate` y `renew` **leen** las claves del repo al cargar el módulo (`test.js:17-24`). Borrar el directorio antes de sustituir esa lectura deja la suite en rojo y sin forma de distinguir una regresión real. Primero se independizan las pruebas (pasos 1-3), después se borra (paso 4).

- [ ] **Step 1: Confirmar el alcance de la dependencia**

Run: `grep -rn "test-keys\|test_priv\|test_pub" --include='*.js' --include='*.rs' --include='*.yml' --include='*.json' --include='*.md' . | grep -v node_modules`

Expected: al menos `packages/licenses/activate/test.js` y `packages/licenses/renew/test.js`. Anota cualquier otra referencia — si aparece una en `backend/`, trátala en este mismo paso con el mismo criterio: la clave se genera, no se lee del repo.

- [ ] **Step 2: Generar el par en tiempo de ejecución en las pruebas**

En `do-functions/packages/licenses/activate/test.js`, sustituir el bloque que lee los PEM (líneas 17-24) por generación efímera:

```js
// Par RSA efímero, generado por corrida. C-06: ninguna clave privada vive en
// el repositorio, ni siquiera como fixture de prueba — la que estaba aquí
// resultó ser la misma que el backend confía en producción.
const { generateKeyPairSync } = require('node:crypto');
const { privateKey, publicKey } = generateKeyPairSync('rsa', {
    modulusLength: 2048,
    publicKeyEncoding: { type: 'spki', format: 'pem' },
    privateKeyEncoding: { type: 'pkcs8', format: 'pem' },
});
process.env.LICENSE_PRIVATE_KEY = privateKey;
const TEST_PUBKEY = publicKey;
```

`fs` y `path` quedan sin uso en ese archivo: quitar ambos `require` si no los usa nada más.

Ajustar el comentario de cabecera (líneas 4-7): hoy afirma que el par se copia byte a byte de `backend/tests/fixtures/` para dar determinismo end-to-end. Eso deja de ser cierto y era justamente el origen del problema. Sustituir por:

```js
// Uses the in-memory shared-store via process.env.TEST_STORE so no Postgres
// is required for unit tests. The RSA keypair is generated per run: these
// tests verify that what this function signs, this function can verify —
// they do not and must not depend on the production signing key.
```

- [ ] **Step 3: Repetir en `renew/test.js`**

Aplicar exactamente el mismo cambio en `do-functions/packages/licenses/renew/test.js`. Repetido a propósito: son dos archivos independientes y cada uno carga su propia clave.

Run: `cd do-functions && npm test`
Expected: todas las pruebas en verde **todavía con `test-keys/` presente**. Esto demuestra que las pruebas ya no dependen del directorio, antes de borrarlo.

- [ ] **Step 4: Eliminar el directorio**

```bash
git rm -r do-functions/test-keys
```

Run: `cd do-functions && npm test`
Expected: verde. Si algo se rompe aquí, quedó una referencia sin migrar en el paso 1.

- [ ] **Step 5: Añadir el gate de secretos a CI**

En `.github/workflows/ci.yml`, añadir un job nuevo. Va con `permissions: contents: read`, igual que los demás (T-08-15):

```yaml
  secret-scan:
    name: Secret Scan
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - name: Fail on committed private keys
        run: |
          if git grep -nE -- '-----BEGIN (RSA |EC |OPENSSH |PGP )?PRIVATE KEY-----' -- . ':!*.md'; then
            echo "::error::A private key is committed in the working tree."
            exit 1
          fi
          echo "No private keys in the working tree."
```

La exclusión de `*.md` permite que el runbook muestre el encabezado de un PEM sin volver rojo su propio gate.

- [ ] **Step 6: Verificar que el gate detecta lo que debe**

```bash
printf -- '-----BEGIN RSA PRIVATE KEY-----\nnot-a-real-key\n-----END RSA PRIVATE KEY-----\n' > /tmp/canary.pem
cp /tmp/canary.pem ./canary.pem && git add canary.pem
git grep -nE -- '-----BEGIN (RSA |EC |OPENSSH |PGP )?PRIVATE KEY-----' -- . ':!*.md'; echo "exit=$?"
git rm -f --cached canary.pem && rm -f canary.pem
```

Expected: la primera invocación imprime `canary.pem` y sale con `exit=0` (git grep encontró algo → el job hará `exit 1`). Tras el `rm`, no debe encontrar nada. Si no detecta el canario, el gate es decorativo — arréglalo antes de seguir.

- [ ] **Step 7: Escribir el runbook de rotación**

Crear `docs/runbooks/rotacion-clave-licencia.md`:

````markdown
# Rotación de la clave de firma de licencias (C-06)

La clave privada `do-functions/test-keys/test_priv.pem` estuvo versionada y
corresponde a la pública que el backend compila. **Debe tratarse como
comprometida**, aunque no haya evidencia de uso indebido: sigue estando en la
historia de git y sigue siendo válida hasta que se rote.

## 1. Generar el par nuevo (fuera del repositorio)

```bash
umask 077
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:4096 -out ~/cronometrix-license-priv.pem
openssl pkey -in ~/cronometrix-license-priv.pem -pubout -out ~/cronometrix-license-pub.pem
```

La privada nunca entra al repositorio, ni a un `.env` versionado, ni a un
issue, ni a un chat.

## 2. Sustituir la pública que confía el backend

```bash
cp ~/cronometrix-license-pub.pem backend/src/license/pubkey.pem
```

Se compila con `include_str!`, así que **rotar exige recompilar y redesplegar**.

## 3. Cargar la privada en la función de firma

Colocarla como secreto de DigitalOcean Functions (nunca como archivo del
paquete) y verificar que `do-functions` la lee de entorno.

## 4. Reemitir las licencias vigentes

Toda licencia firmada con la clave vieja deja de verificar en cuanto se
despliegue el binario nuevo. Reemitir **antes** de desplegar, o los clientes
quedan fuera de servicio.

## 5. Verificar

```bash
openssl pkey -in ~/cronometrix-license-priv.pem -pubout | openssl sha256
openssl pkey -pubin -in backend/src/license/pubkey.pem -pubout | openssl sha256
```

Los dos SHA-256 deben coincidir entre sí y **ser distintos** de
`f0318260deb1672cb624314065c8bd42d394fd799a954580265ca3b19f3106fc`, que es el
de la clave comprometida.

## 6. Purgar la historia (coordinado, destructivo)

Borrarla del árbol no la saca de la historia. Requiere `git filter-repo` o
BFG y un push forzado, coordinando con todos los clones — incluidos los
worktrees activos. Mientras esto no se haga, la rotación de los pasos 1-5 es
lo que realmente protege: la clave vieja queda en la historia pero ya no
firma nada que el producto acepte.
````

- [ ] **Step 8: Confirmar que la suite sigue verde sin las claves**

Run: `cd do-functions && npm test`
Expected: las 17 pruebas en verde. Si alguna dependía de `test-keys`, arréglala para que genere su par en tiempo de ejecución.

- [ ] **Step 9: Commit**

```bash
git add -A do-functions/test-keys do-functions/packages/licenses/activate/test.js do-functions/packages/licenses/renew/test.js .github/workflows/ci.yml docs/runbooks/rotacion-clave-licencia.md
git commit -m "chore(security): remove the committed signing key and gate against new ones (C-06)"
```

---

## Fuera de este plan

Deliberadamente **no** se incluyen aquí, y cada uno necesita su propio plan:

- **C-01 a C-05** (motor monetario) — decisiones ya tomadas: tardanza se paga por tiempo realmente trabajado eliminando `late_deduction_cents`; salario vacío se rechaza en vez de heredarse. Requiere corregir además las pruebas que hoy consolidan la especificación equivocada (`calc/overtime.rs:39-45`).
- **C-10** (inbox durable de ingesta) — el arreglo es persistir el cuerpo crudo antes de responder y procesar asíncrono, **manteniendo el 200 incondicional**.

  El grafo fija dónde puede vivir ese inbox. `RawMarking` se construye en un
  solo sitio —`isapi/ingest.rs`— y el núcleo se cruza en una sola línea
  (`isapi/ingest.rs:114`). El cuerpo crudo que hay que persistir es **de forma
  Hikvision**: multipart, XML o JSON según firmware, con JPEG embebido. Meter
  esos bytes en `attendance/` pondría formato de vendedor dentro del núcleo y
  rompería la arquitectura hexagonal.

  Por tanto el inbox durable pertenece al **lado adaptador** (`devices/push.rs`
  + `isapi/`): guarda bytes crudos con hash/idempotency, responde 200, y el
  reproceso asíncrono los pasa por el mismo traductor `isapi::ingest`, que
  sigue siendo el único que fabrica `RawMarking`. El núcleo no se entera de que
  hubo un reintento.
- **Autenticación digest del webhook** — continuación natural de la Tarea 2, pero exige verificación contra hardware real: el firmware ya demostró mentir sobre escrituras de `httpHosts`.
- **Los 26 hallazgos sin verificar** — verificar antes de planificar. El muestreo dio 14/14 reales, pero dos estaban encuadrados de forma que inducía a un arreglo equivocado.

## Punto pendiente de confirmación legal

La decisión sobre C-02 (eliminar el descuento monetario por tardanza) se apoya en que una deducción punitiva **adicional** al tiempo no trabajado constituiría doble sanción por el mismo hecho, y en que la LOTTT ya ofrece el remedio disciplinario (art. 79; Reglamento art. 38: cuatro retardos en un mes como causal). La fuente consultada es doctrina de divulgación, no jurisprudencia. **Confirmar con abogado laboral venezolano antes de facturar con esta lógica.**
