# Bloque 3: retención contra preservación — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolver el choque entre H-10 (borrar datos biométricos y médicos) y H-09 (preservar evidencia de jornada), purgar la plantilla facial al terminar la relación laboral —que hoy no ocurre—, hacer que el respaldo cubra los archivos (H-14) y dar al reporte una lectura consistente (M-05).

**Architecture:** Cuatro tareas. La primera separa las dos clases de dato que hoy se tratan como una; las demás dependen de esa separación o son independientes. **Construye mecanismo, no política**: los plazos de retención se configuran, no se cablean, porque dependen de una consulta laboral aún sin responder.

**Tech Stack:** Rust/Axum 0.8, libSQL/SQLite, filesystem, `cargo nextest`.

## Global Constraints

- **Nunca** usar los identificadores desnudos `execute`, `execute_batch` ni `transaction` dentro de `backend/src` — gate de build.
- **Nunca romper la arquitectura hexagonal.** El detalle Hikvision vive solo en `backend/src/isapi/*`.
- Toda mutación de datos de asistencia genera entrada de auditoría inmutable.
- Todo acceso a un archivo debe pasar por las validaciones de ruta, propiedad y enlace simbólico que ya existen en `backend/src/storage/` — son un control positivo reconocido por la auditoría y no se rodean.
- Gate de cobertura: proyecto ≥90% líneas / ≥85% ramas; por archivo ≥70% / ≥60%.
- `cargo clippy --all-targets --all-features -- -D warnings` debe pasar.
- **No corras `cargo fmt` a secas** — `main` no está limpio de rustfmt.
- Mensajes de commit en inglés con prefijo convencional.

## Contexto imprescindible

### El choque, y por qué no es un conflicto de implementación

**H-10** pide plazos y borrado verificable de datos biométricos y médicos.
**H-09** exige preservar la evidencia de jornada como rastro inmutable de nómina, citando conservación tributaria.

Apuntan a **los mismos archivos**: fotos de eventos, XML crudo, evidencias de permisos.

No se contradicen por error. Se contradicen porque **ambos tratan como un solo dato lo que son dos**:

| Clase | Qué es | Qué debe pasarle |
|---|---|---|
| **Plantilla facial viva** | El rostro enrolado, usado para reconocer a la persona **de aquí en adelante** | Se purga al terminar la relación laboral. Ya no hay finalidad que la justifique. **Hoy no se purga nada del disco.** |
| **Evidencia de una marcación** | La foto y el XML de un fichaje concreto — **prueba de que esa jornada ocurrió** | Reloj de retención propio, mucho más largo, fijado por obligación fiscal y laboral |

Confundirlas es lo que vuelve irreconciliables a H-09 y H-10. Separadas, cada una tiene una respuesta clara.

### Lo que hoy hace `purge.rs`

`backend/src/workers/purge.rs` al desactivar un empleado **solo** revoca el mapeo facial en el lector y borra la fila de `device_face_mappings`. **No toca ni un archivo.** El rostro enrolado, las fotos de sus fichajes, el XML y las evidencias médicas siguen en disco indefinidamente.

O sea: H-10 no es "falta afinar la retención". Es que **no existe retención de ninguna clase**.

### Mecanismo, no política

Los plazos concretos —cuántos años se conserva una evidencia de jornada— dependen de `docs/legal/CONSULTA-LABORAL.md`, aún sin responder.

**No los cablees.** Construye el mecanismo con la retención configurable y **el valor por defecto más seguro: no borrar nada**. Cuando llegue la respuesta legal, fijar el plazo debe ser configuración, no un desarrollo nuevo.

La única excepción es la plantilla facial al terminar la relación: ahí la finalidad desaparece con la relación misma, y esa sí se purga sin esperar a nadie.

---

## File Structure

| Archivo | Responsabilidad | Tarea |
|---|---|---|
| `backend/src/workers/purge.rs` | Purga de la plantilla facial | 1 |
| `backend/src/db/migrations/0NN_retention_policy.sql` | Política configurable | 2 |
| `backend/src/workers/retention.rs` | Barrido por retención | 2 |
| `deploy/` (script de respaldo) | Respaldo que incluye archivos | 3 |
| `backend/src/reports/service.rs` | Lectura consistente | 4 |

---

### Task 1: La plantilla facial se purga al terminar la relación (H-10, primera mitad)

**Files:**
- Modify: `backend/src/workers/purge.rs`
- Test: `backend/tests/face_template_purge_test.rs` (crear)

**Interfaces:**
- Consumes: las utilidades de `backend/src/storage/` para borrar con validación de ruta y propiedad. **No borres con `std::fs` directo** — ese módulo existe porque hubo un hallazgo sobre enlaces simbólicos.

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
mod common;

/// H-10: al desactivar un empleado, su plantilla facial deja de existir en
/// disco. Ya no hay finalidad que la justifique.
#[tokio::test]
async fn deactivating_an_employee_deletes_the_enrolled_face() { }

/// Y la evidencia de sus fichajes NO se toca — es prueba de jornada y
/// H-09 exige conservarla.
#[tokio::test]
async fn deactivating_an_employee_keeps_the_attendance_evidence() {
    // este test es el que impide "arreglar" H-10 rompiendo H-09
}

/// La purga queda auditada: quién, cuándo, qué.
#[tokio::test]
async fn the_purge_is_recorded_in_the_audit_log() { }
```

**El segundo test es el más importante del bloque.** Es lo que impide que alguien cierre H-10 borrando de más.

- [ ] **Step 2: Correr y ver que fallan**

- [ ] **Step 3: Purgar solo la plantilla**

Extender `purge.rs` para borrar el rostro enrolado del directorio de enrolamientos, además de revocarlo en el lector.

**Lee primero qué hay en cada directorio.** `enrollments_root`, `events_root` y `leaves_root` guardan cosas distintas y solo uno de ellos es plantilla viva. Si al leerlos descubres que un directorio mezcla ambas clases, **para y repórtalo**: significa que la separación que este plan asume no existe en disco, y eso cambia el trabajo.

Registrar la purga en auditoría con el empleado, el momento y qué se borró.

- [ ] **Step 4: Verificar y commitear**

```bash
git add backend/src/workers/purge.rs backend/tests/face_template_purge_test.rs
git commit -m "fix(privacy): purge the enrolled face template on termination (H-10)"
```

---

### Task 2: Retención configurable de la evidencia (H-10, segunda mitad)

**Files:**
- Create: migración de política de retención (siguiente número libre)
- Create: `backend/src/workers/retention.rs`
- Modify: `backend/src/main.rs`
- Test: `backend/tests/retention_worker_test.rs` (crear)

- [ ] **Step 1: Escribir las pruebas que fallan**

```rust
mod common;

/// Con la política por defecto —no borrar— el barrido no toca nada, por
/// antiguo que sea el archivo.
#[tokio::test]
async fn the_default_policy_deletes_nothing() { }

/// Con un plazo configurado, borra lo que lo supera y conserva lo demás.
#[tokio::test]
async fn a_configured_period_deletes_only_what_exceeds_it() { }

/// Cada borrado queda auditado.
#[tokio::test]
async fn every_deletion_is_audited() { }
```

- [ ] **Step 2: Correr y ver que fallan**

- [ ] **Step 3: La política**

Una fila de configuración con el plazo por clase de archivo, **nula por defecto**, y nulo significa *conservar indefinidamente*.

**El valor por defecto es la decisión de seguridad de esta tarea.** Un default que borra convierte un despliegue desatendido en pérdida de prueba de jornada. Un default que conserva solo cuesta disco.

- [ ] **Step 4: El barrido**

Un worker con la forma de `capture_cleanup.rs` — **léelo antes**. Borra solo lo que supera su plazo, usando las utilidades de `storage/`, y audita cada borrado.

**No borres nada cuyo período pueda seguir abierto.** Sin cierre de período (H-09, fuera de este plan), no hay forma fiable de saberlo, así que sé conservador y dilo en el informe.

- [ ] **Step 5: Verificar y commitear**

```bash
git add backend/src/db/migrations/ backend/src/workers/ backend/src/main.rs backend/tests/retention_worker_test.rs
git commit -m "feat(privacy): add a configurable retention sweep, defaulting to keep everything (H-10)"
```

---

### Task 3: El respaldo cubre los archivos (H-14)

`deploy/INSTALL.md:75-98` documenta un respaldo que cubre **solo** `cronometrix.db`. Los directorios de rostros, fotos de eventos y evidencias no entran.

Consecuencia: un `restore` recupera la base y **pierde todas las imágenes**. Las filas siguen apuntando a archivos que ya no existen. Eso no es degradación, es pérdida — y es evidencia de jornada, justo lo que H-09 exige conservar.

**Files:**
- Modify: el script y la documentación de respaldo en `deploy/`
- Test: `deploy/tests/` o equivalente

- [ ] **Step 1: Leer qué hace hoy el respaldo**

Antes de tocar nada, documenta en tu informe qué cubre exactamente y qué no. **Ojo:** `deploy/tests/gateway-config-test.sh` existe y **nada lo invoca** — si añades pruebas ahí, comprueba que alguien las corra, o habrás escrito un séptimo gate decorativo.

- [ ] **Step 2: Incluir los directorios**

Extender el respaldo a `enrollments_root`, `events_root`, `leaves_root` y `overrides_root` — confirma los nombres reales en `backend/src/state/paths.rs`, no los tomes de aquí.

- [ ] **Step 3: Un manifiesto que detecte la inconsistencia**

Una base restaurada sin sus archivos es peor que un fallo de restauración: parece funcionar. El respaldo debe llevar un manifiesto que permita comprobar, al restaurar, que base y archivos vienen del mismo momento.

- [ ] **Step 4: Probar la restauración de verdad**

Respaldar, borrar, restaurar, y comprobar que las imágenes vuelven. **Un respaldo que nunca se ha restaurado es una suposición.**

- [ ] **Step 5: Commit**

```bash
git add deploy/
git commit -m "fix(deploy): back up and restore the evidence directories, not just the database (H-14)"
```

---

### Task 4: El reporte lee un estado consistente (M-05)

`backend/src/reports/service.rs` ejecuta varias consultas secuenciales en autocommit. Una escritura concurrente entre ellas produce un reporte internamente contradictorio.

**Files:**
- Modify: `backend/src/reports/service.rs`
- Test: `backend/tests/report_snapshot_test.rs` (crear)

- [ ] **Step 1: Escribir la prueba que falla**

```rust
mod common;

/// M-05: una escritura concurrente a mitad del reporte no puede producir
/// un resultado que mezcle dos estados.
#[tokio::test]
async fn a_concurrent_write_cannot_split_a_report() {
    // arrancar el reporte, escribir en medio, comprobar coherencia interna
}
```

Este es el test difícil del bloque: reproducir la carrera exige control del entrelazado. **Si no consigues una reproducción fiable, dilo** en vez de escribir una prueba que pasa siempre — una prueba de carrera que no falla contra el código roto no prueba nada.

- [ ] **Step 2: Correr y ver que falla**

- [ ] **Step 3: Lectura consistente**

Envolver las consultas del reporte en una lectura consistente.

**Ojo con el gate:** `transaction` es un identificador prohibido en `backend/src`. Mira cómo lo resuelve `db_write.transact` y si hay un camino equivalente para lectura; si no lo hay, dilo antes de inventar uno que el gate rechace.

- [ ] **Step 4: Verificar y commitear**

```bash
git add backend/src/reports/service.rs backend/tests/report_snapshot_test.rs
git commit -m "fix(reports): read a consistent snapshot instead of sequential queries (M-05)"
```

---

## Fuera de alcance, y por qué

**H-09 completo —cierre de período— es su propio bloque.** Snapshot de entradas y reglas, aprobación, hash del artefacto, reapertura controlada y corrección por asiento en vez de reescritura. Es la pieza más grande que queda de toda la auditoría y no cabe aquí.

Este bloque hace lo que H-09 necesita como cimiento —que la evidencia siga existiendo— sin construir el cierre en sí.

**Cifrado de archivos en reposo** (la otra mitad de H-14). Necesita decidir gestión de claves, y ese debate se parece al de C-06. Aparte.

## Pendiente que no es código

Los **plazos de retención** dependen de `docs/legal/CONSULTA-LABORAL.md`. Este plan construye el mecanismo con el default seguro para que fijarlos sea configuración.

Y la **rotación de la clave de licencia** sigue vencida: C-06 está contenido, no cerrado.
