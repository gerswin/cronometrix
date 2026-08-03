# Observaciones: tests inestables

Hallazgos que quedan **observados** — investigados pero sin causa raíz probada,
por lo que no se cambió código de producción. Se documentan para que la próxima
aparición no empiece desde cero.

---

## `db_write_queue_test::concurrent_close_persists_exactly_all_accepted_jobs`

**Estado:** observado. Causa raíz **no** encontrada. No bloquea merge.

**Fecha:** 2026-08-02

### Qué se vio

Dos fallos consecutivos en `main` bajo `cargo llvm-cov nextest --branch`
(suite completa). El mensaje de fallo no se conservó — las corridas solo
pasaron por `grep` en terminal y ningún log quedó en disco. Sin ese mensaje
no se sabe qué rama del test reventó.

Se describió inicialmente como "determinista". **Es incorrecto**: dos fallos
seguidos motivaron esa etiqueta, pero `main` pasó después 9/9 bajo la misma
suite completa instrumentada.

### Qué se descartó (con experimento, no con opinión)

| Hipótesis | Experimento | Resultado |
|-----------|-------------|-----------|
| Pérdida de datos por TOCTOU en el cierre | 200 iteraciones del escenario | 0 pérdidas |
| Ídem, bajo instrumentación `llvm-cov` | 200 iteraciones instrumentadas | 0 pérdidas |
| Contención de CPU disparando el timeout | 5 corridas con 64 procesos quemando 32 cores | 5/5 pasa |
| Cierre concurrente con llegada de productores | 200 iteraciones, 16 worker threads (~20.000 interleavings) | 0 pérdidas, 0 fantasmas |

El invariante `persisted == accepted` aguantó en todos los casos.

### Fragilidades reales encontradas al leer el código

No son la causa probada. Son deuda observada:

1. **`backend/tests/db_write_queue_test.rs`** trata `DbWriteError::Busy` como
   `panic!("unexpected producer result")`. Con `capacity: 4`, 100 productores y
   `enqueue_timeout` de 1s, `Busy` es un resultado *legítimo* de la API bajo
   carga, no un fallo. Los runners de CI tienen 2-4 cores; la máquina donde se
   investigó tiene 32 — el margen en CI es mucho más estrecho. Si el flake
   reaparece en CI, esta es la primera sospechosa.

2. **`backend/src/db/write_queue.rs`** (`run_write_worker`): al recibir
   `WriteCommand::Shutdown` hace `return Ok(())`, abandonando lo que quede en
   el canal. Hoy lo protege el orden FIFO de `reserve_owned` — el shutdown
   encola su permiso detrás de los productores ya estacionados. Es una garantía
   de *ordenamiento*, no *estructural*: entre el chequeo de `closed` y el
   `reserve_owned` del productor no hay `await`, así que solo un desalojo del SO
   abre la ventana. Reproducirla no se logró.

### Si reaparece

- Capturar la salida completa a archivo (`> log.txt 2>&1`), no por `grep` —
  el mensaje de la aserción es lo que faltó para cerrar el diagnóstico.
- Distinguir `unexpected producer result: Busy` (fragilidad 1) de un
  `assert_eq!` de `persisted` vs `accepted` (fragilidad 2). Son bugs distintos
  y el arreglo es distinto.

---

## Hueco de cobertura aceptado: `POST /setup/activate` éxito HTTP (C-06)

**Estado:** aceptado conscientemente. No bloquea merge. No se puede cerrar
sin reintroducir el problema que C-06 eliminó.

**Fecha:** 2026-08-02

### Qué se perdió

Antes de C-06, `backend/tests/license_tests.rs::test_setup_activate_succeeds_via_wiremock`
ejercía el camino completo: wiremock devuelve un JWT firmado con la clave de
prueba (`do-functions/test-keys/` / `backend/tests/fixtures/`, ambas
byte-idénticas a `backend/src/license/pubkey.pem`), el router real
(`POST /api/v1/setup/activate` → `setup::handlers::setup_activate` →
`license::service::activate_license`) lo verificaba con éxito, y el test
comprobaba `StatusCode::OK`, el cuerpo `{"activated": true}`, y el flip de
`license_valid`.

C-06 retiró esa clave de prueba (era la misma que el backend confía en
producción — ver `docs/runbooks/rotacion-clave-licencia.md`). Las pruebas
ahora firman con un par RSA efímero por binario de prueba. Ese par nunca
verificará contra `backend/src/license/pubkey.pem`, que sigue siendo la
única clave que `activate_license` — y por tanto el router real — acepta
(embebida en tiempo de compilación vía `include_str!`, sin punto de
inyección en el camino de producción).

Se instrumentó el test en los dos commits para confirmarlo, no se asumió:

| Commit | `resp.status()` |
|--------|------------------|
| `dec0fd3` (antes del fix round 1) | `200 OK` |
| `a26df90` (después) | `403 Forbidden`, siempre |

El test se renombró a
`test_setup_activate_fails_closed_via_wiremock_since_c06` y se reescribió
para afirmar directamente lo único que sigue siendo alcanzable — la rama
`StatusCode::OK` quedó muerta y ya no se tolera como posibilidad.

### Qué queda sin cubrir

La respuesta HTTP de un `POST /setup/activate` **exitoso** a través del
router real: el status `200`, el cuerpo `{"activated": true}`, y el
cableado router → `activate_license` → flip de `license_valid` en ese caso.
`test_activation_calls_do_functions_with_fingerprint` (mismo archivo) sigue
cubriendo la ruta de éxito, pero a nivel de servicio — llama a
`activate_license_with_key` directamente con la clave efímera del test, sin
pasar por el router. La serialización de la respuesta HTTP y el mapeo de
status en el camino feliz no los ejerce nada hoy.

### Por qué no se cierra con una inyección de clave en la capa HTTP

Se consideró y se descartó deliberadamente. Darle a `setup_activate` (o a
`AppState`/`Config`) una forma de aceptar una clave de verificación distinta
en pruebas es, en la práctica, un punto de sustitución de clave: si existe
el mecanismo para que el router confíe en una clave que no es
`pubkey.pem`, existe la superficie para que production apunte, por error de
configuración o por un flag mal puesto, a una clave que no es la
comprometida-y-ya-rotada. Es exactamente el problema que C-06 cerró.
Preferible un hueco de cobertura documentado a reabrir esa superficie.

### Qué lo cerraría correctamente

Una identidad de firma **separada para pruebas**, no derivada de ni
sustituible por la de producción:

- Generar un par RSA de pruebas de punta a punta (do-functions test suite +
  backend test suite) que sea **distinto** al de producción desde el día
  uno — nunca copiado de `pubkey.pem`, nunca usado para firmar nada real.
- `backend/src/license/pubkey.pem` seguiría confiando **solo** en la clave
  de producción — sin cambios en `verify_license_jwt` ni en el router.
- El test E2E de la ruta HTTP feliz correría contra un **binario de test
  dedicado** compilado con la clave pública de pruebas embebida en lugar de
  la de producción (dos binarios, dos `pubkey.pem`, seleccionados en tiempo
  de compilación vía feature flag o perfil de build) — no en tiempo de
  ejecución, para no reabrir la superficie de sustitución.
- Alternativa más simple si el equipo la prefiere: aceptar el hueco de forma
  permanente y dejar que el E2E real (Playwright, Fase 9) contra un backend
  compilado con una clave de test-only dedicada sea la prueba de humo del
  camino feliz HTTP — evaluar si ese E2E ya existe o si haría falta
  agregarlo.

Cualquiera de las dos rutas es trabajo nuevo con su propio análisis de
riesgo — no se implementa aquí.
