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
