# Auditoría de arquitectura hexagonal

**Fecha:** 2026-08-02
**Alcance:** backend Rust, con foco en qué haría falta para soportar lectores
biométricos de marcas distintas de Hikvision.
**Método:** inspección del código y del esquema, con las cifras medidas sobre el
árbol en el commit `ec45894`.

---

## Veredicto

El dominio está limpio; la periferia no tiene puertos. Añadir una segunda marca
hoy obligaría a tocar **siete módulos de aplicación**, no solo a escribir un
adaptador nuevo.

La buena noticia es que el trabajo caro ya está hecho: el núcleo de negocio no
sabe que Hikvision existe.

---

## Lo que ya está bien

### El dominio es agnóstico de marca

```
calc/  rules/  recompute/  daily_records/   →  0 referencias a isapi
events/service.rs                           →  0 referencias a isapi
```

El motor de cálculo de horas, el de reglas y la persistencia de eventos no
conocen el protocolo del lector. Eso es precisamente lo que la arquitectura
hexagonal existe para proteger, y está protegido.

### El esquema no lleva marca

`attendance_events` almacena `direction`, `captured_at`, `is_unknown`,
`employee_no_string` — conceptos de negocio, no de Hikvision. La única fuga es
el nombre de una columna (ver deuda, más abajo).

---

## Los tres problemas reales

### 1. Hay adaptadores, pero no hay puertos

`DeviceConnection` es un struct concreto con **17 métodos públicos**, construido
en **siete sitios fuera de `isapi/`**:

| Módulo | Qué usa |
|---|---|
| `devices/handlers.rs` | `door_open`, `reboot`, `enrollment_mode` |
| `enrollments/pusher.rs` | `upsert_user`, `upload_face` |
| `enrollments/service.rs` | construcción de conexiones |
| `enrollments/handlers.rs` | `capture_face_image` |
| `workers/purge.rs` | `delete_user` |
| `supervisor/task.rs` | `connect_and_stream`, `provision_device` |
| `devices/push.rs` | `ingest_alert` |

No existe **ni un solo `trait`** en el código de aplicación (los dos que hay son
internos del write queue). La aplicación llama al adaptador directamente. Para
integrar ZKTeco habría que editar los siete módulos.

### 2. La ingesta compartida vive dentro del vendor y habla su idioma

`isapi::ingest::ingest_alert` mezcla hoy dos responsabilidades:

```
parsear EventNotificationAlert   ←  específico de Hikvision
        ↓
resolver empleado + persistir    ←  dominio puro
```

Extraerlo del stream consumer fue un paso correcto, pero se quedó a medio
camino: sigue viviendo en `isapi/` y su tipo de entrada es
`EventNotificationAlert`. Un lector de otra marca no puede reutilizarlo sin
fabricar un struct Hikvision falso.

### 3. `devices` no sabe de qué marca es cada lector

```
devices: id, name, ip, port, scheme, username, encrypted_password,
         direction, allow_insecure_tls, connection_state, last_seen_at,
         status, deleted_at, version, created_at, updated_at,
         ingest_mode, push_token
```

No hay columna `vendor`. Además el modelo de conexión asume HTTP
(`base_url = scheme://ip:port`), lo que excluye lectores con SDK propietario
sobre TCP — ZKTeco es el caso típico.

---

## Los puertos que faltan

### Puerto de salida: el lector

```rust
#[async_trait]
pub trait BiometricReader: Send + Sync {
    async fn provision(&self, intent: &ProvisioningIntent) -> Result<ProvisionReport>;
    async fn enroll(&self, person: &PersonRef, face: &[u8]) -> Result<()>;
    async fn revoke(&self, person: &PersonRef) -> Result<()>;
    async fn capture_face(&self) -> Result<Vec<u8>>;
    async fn send_command(&self, command: DeviceCommand) -> Result<String>;
}
```

**`provision` debe recibir una intención, no órdenes.** El provisioning actual
dice *"pon `manualAndAuto`, escribe el weekPlan 1, mapea 6 teclas de función"* —
vocabulario Hikvision puro. Debería decir *"necesito que cada marcación traiga
dirección; el corte del día son las 13:00"*, y que cada adaptador decida cómo
lograrlo con su firmware.

Devolver un `ProvisionReport` en vez de `()` permite que un lector declare *"no
soporto división horaria"* en lugar de mentir con un `Ok`. Esto no es
hipotético: el DS-K1T341CMFW ya responde `statusCode 1` a escrituras que no
aplica (ver `set_event_http_host`).

### Puerto de entrada: la ingesta

```rust
pub struct RawMarking {          // sin marca
    pub external_person_id: String,
    pub occurred_at: DateTime<Utc>,
    pub direction: Option<Direction>,
    pub photo: Option<Vec<u8>>,
    pub raw_payload: String,
}

pub async fn ingest(
    state: &AppState,
    device_id: &str,
    fallback: Direction,
    marking: RawMarking,
) -> Result<IngestOutcome>;
```

El adaptador traduce su formato propio a `RawMarking`. La resolución de
empleado, el filtrado de eventos sin identidad y la persistencia pasan a ser
dominio puro. **Este es el movimiento que más desacopla** de todo lo propuesto.

---

## Camino de migración

Ordenado por dependencia, no por importancia.

1. **[HECHO] Renombrar `attendance_events.raw_xml` → `raw_payload`.**
   Ya guarda JSON desde el soporte de firmware V3.3.8; el nombre mentía.
   Migración de una línea. Commit `f78a28f`.

2. **[HECHO] Extraer `RawMarking` y mover resolución + persistencia a
   `attendance::ingest`.**
   `isapi::ingest` se quedó solo con la traducción. Sin cambio de
   comportamiento observable, cubierto por la suite existente. Commit
   `1fd36d3`.

3. **[PARCIAL] Definir `BiometricReader` e implementarlo sobre `DeviceConnection`;
   migrar los siete llamadores.**
   Definición del trait y del `ProvisionReport`/`ProvisioningIntent`:
   commits `12cdcf5`, `91246e3`. De los siete llamadores originales, CINCO
   quedaron migrados, uno por sitio (todos validados con la suite completa —
   1057 tests, 0 fallos — entre cada uno):
   - `workers/purge.rs` (`delete_user` → `revoke`): `b947cff`
   - `devices/handlers.rs` (`door_open`/`reboot`/`enrollment_mode` →
     `execute(DeviceCommand)`, later renamed `send_command` to avoid
     colliding with the DB-write-queue gate's forbidden-identifier check):
     `a6afb41`
   - `enrollments/handlers.rs` (`capture_face_image` → `capture_face`):
     `69c58f5`
   - `enrollments/pusher.rs` (`upsert_user`+`upload_face` → `enroll`):
     `f10e6da`. Este era el llamador señalado como posible punto de parada
     (podía cambiar qué queda dentro del timeout de 30s o cuándo se escribe
     el checkpoint); tras leer el checkpoint y el propio `enroll` del
     adaptador se confirmó que hacen la misma secuencia en el mismo orden,
     así que se migró en vez de dejarlo en `DeviceConnection`.
   - `enrollments/service.rs` (validación de conectividad antes de escribir
     filas): `2e740f2`

   `isapi/stream.rs::provision_device` también fue migrado (delega en
   `BiometricReader::provision`) — pero ese sitio nunca estuvo en la lista
   original de siete; su migración tomó, sin querer, el lugar de dos
   llamadores que SÍ estaban en la lista y NO se migraron: `supervisor/task.rs`
   (sigue con `use crate::isapi::stream::{connect_and_stream, DeviceConfig}` y
   llama `provision_device` directamente) y `devices/push.rs` (sigue con
   `use crate::isapi::ingest::{ingest_alert, IngestOutcome}` y
   `use crate::isapi::parser`). Ver "Todavía abierto" para lo que esto
   significa.

## Todavía abierto

4. **`BiometricReader` cubre control, no datos — la mitad más difícil sigue
   sin puerto.** El trait tiene `provision`, `enroll`, `revoke`,
   `capture_face`, `send_command`: todo comandos que el backend envía AL
   lector.
   No tiene ningún método para que el lector ENTREGUE marcaciones — ni
   `stream`, ni `connect`. Por eso:
   - `supervisor/task.rs` sigue construyendo `DeviceConfig` y llamando
     `isapi::stream::connect_and_stream` directamente: el bucle de
     reconexión del alertStream es 100% Hikvision.
   - `devices/push.rs` sigue llamando `isapi::ingest::ingest_alert` e
     `isapi::parser` directamente: el webhook de push es 100% Hikvision.

   `attendance::ingest` (el puerto de ENTRADA, `RawMarking` → `ingest()`) sí
   es agnóstico de marca y ya está listo para recibirlas. Lo que falta es el
   transporte: quien escriba un adaptador ZKTeco satisfaría `BiometricReader`
   por completo — compilaría, pasaría cualquier test de contrato — y seguiría
   sin recibir una sola marcación, porque no hay ningún puerto que le pida
   "entrégame lo que el lector capture". Lo descubriría a mitad de la
   integración, no al leer el trait. Migrar estos dos módulos a un puerto de
   transporte (probablemente algo como `BiometricReader::stream(&self) ->
   impl Stream<Item = RawMarking>`, o un callback de entrega) es el trabajo
   real que queda antes de que "agregar una marca" sea cierto.

5. **`enrollments/pusher.rs:602` decide `terminal` con un downcast a un tipo
   de error Hikvision.** `e.downcast_ref::<crate::isapi::client::DeviceResponseError>()`
   distingue un rechazo limpio del dispositivo (se archiva como fallo
   terminal) de un resultado ambiguo (va a la cola de reconciliación manual
   del operador). Un segundo adaptador devuelve su propio tipo de error, así
   que `terminal` daría `false` para CUALQUIER fallo de ese adaptador — un
   401 llano, un 404, una cara rechazada — y cada enrolamiento fallido
   terminaría en la cola de reconciliación manual. Corregirlo requiere un
   tipo de error a nivel de puerto que distinga un RECHAZO del dispositivo de
   un resultado AMBIGUO — cambio de diseño, no de esta pasada de limpieza; el
   sitio queda marcado con un comentario explicando esto exactamente.

6. **Columna `devices.vendor`** con default `hikvision`, más un `match` en
   `reader_for` que despache por marca. Hoy `reader_for` ya tiene la forma
   correcta (`base_url, username, password, allow_insecure_tls) -> Box<dyn
   BiometricReader>`) pero solo construye `DeviceConnection` — es una
   migración de una línea, mejor hacerla cuando exista una segunda marca real
   con la que probar el `match`, no antes.

7. **Generalizar el modelo de conexión** cuando llegue el primer lector no-HTTP.
   No antes: hacerlo ahora sería especular sobre un protocolo desconocido.

---

## Deuda encontrada de paso

**[RESUELTO] `isapi/parser.rs` ya no es código muerto.** Esto era cierto cuando
se escribió la auditoría: ningún módulo lo importaba y `stream.rs` usaba
`multer` directamente. El commit `9b36341` lo recableó como el único parser
multipart tolerante del código, consolidando tres implementaciones
independientes (`stream.rs`, `client.rs::extract_face_jpeg`,
`devices/push.rs`) en una. Hoy lo importan `isapi/client.rs`
(`extract_face_jpeg`) y `devices/push.rs` (`split_payload`); `stream.rs` sigue
usando `multer` directamente porque consume una conexión en vivo en vez de un
buffer, no porque `parser.rs` esté huérfano.

**`posix_time_zone` es `pub` en `client.rs`** y lo consume `stream.rs`. Es un
detalle del formato que espera Hikvision expuesto como utilidad general.

---

## Qué se rompería hoy con una segunda marca

Lista de verificación para quien haga la integración:

- [ ] `attendance_events.raw_xml` — nombre atado a XML
- [ ] `device_face_mappings.face_id` — semánticamente es *"el identificador con
      el que ese lector conoce a esta persona"*; el nombre filtra el concepto
      Hikvision
- [ ] `DeviceWithPlaintext.base_url` — asume HTTP
- [ ] `Command` (`door_open` / `reboot` / `enrollment_mode`) — genérico, se
      salva
- [ ] `devices.ingest_mode` (`stream` / `push`) — genérico, se salva
- [ ] Constantes de provisioning en `isapi/stream.rs`
      (`ATTENDANCE_MODE`, `ATTENDANCE_KEYS`, `ATTENDANCE_DAY_SPLIT`) — describen
      capacidades de Hikvision, deberían ser una intención de dominio
