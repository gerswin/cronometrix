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
    async fn execute(&self, command: DeviceCommand) -> Result<String>;
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

1. **Renombrar `attendance_events.raw_xml` → `raw_payload`.**
   Ya guarda JSON desde el soporte de firmware V3.3.8; el nombre miente.
   Migración de una línea.

2. **Extraer `RawMarking` y mover resolución + persistencia a
   `attendance::ingest`.**
   `isapi::ingest` se queda solo con la traducción. Sin cambio de comportamiento
   observable, cubierto por la suite actual.

3. **Definir `BiometricReader` e implementarlo sobre `DeviceConnection`.**
   Los siete llamadores pasan a depender del trait. Sigue habiendo un único
   adaptador, pero ya nadie lo nombra directamente.

4. **Columna `devices.vendor`** con default `hikvision`, más una fábrica
   `reader_for(&device) -> Box<dyn BiometricReader>`.

5. **Generalizar el modelo de conexión** cuando llegue el primer lector no-HTTP.
   No antes: hacerlo ahora sería especular sobre un protocolo desconocido.

Los pasos 1-3 son refactor puro y los valida la suite existente. El 4 habilita
la segunda marca. El 5 solo si hace falta.

---

## Deuda encontrada de paso

**`isapi/parser.rs` son 240 líneas de código muerto.** Ningún módulo lo importa;
`stream.rs` usa `multer` directamente. Se escribió como fallback para firmware
no estándar (RESEARCH § Pitfall 2) y quedó huérfano. Borrarlo o volver a
cablearlo, pero no dejarlo: da falsa impresión de cobertura sobre un camino que
nunca se ejecuta.

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
