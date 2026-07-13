# Diseño de remediación y estabilización v1.0

**Fecha:** 2026-07-13

**Baseline de código:** `1dd6d758fc1ed775189a4fff3f20d6a7c1800e34` (`main` y `origin/main`)

**Estado del diseño:** aprobado por el usuario

**Estrategia:** gate-first

## 1. Propósito

Este diseño define cómo llevar Cronometrix desde una beta funcional avanzada a
una v1.0 instalable, reproducible y verificable. La prioridad no es añadir más
funcionalidad: es restaurar una línea base confiable, corregir contratos rotos,
cerrar invariantes de persistencia y auditoría, y demostrar el release en una
instalación limpia.

El trabajo se divide en dos fases nuevas:

- **Fase 12 — v1.0 Release Stabilization:** remedia código, contratos, pruebas,
  cola de escritura, auditoría, empaquetado e instalador.
- **Fase 13 — v1.0 Live Validation:** demuestra los gates técnicos y la
  instalación real antes del sign-off.

La Fase 11 se conserva como historial parcial y se marca como sustituida para
los ítems activos. Sus dos aceptaciones de riesgo —Hikvision real y LIC-05 en
dos hosts— se trasladan a la Fase 13 sin presentarlas como pruebas superadas.

## 2. Contexto verificado

El baseline remoto incorporó cuatro correcciones que no deben volver a entrar
en el backlog:

- inicialización de `AppState.db_write` y restauración de compilación de tests;
- cancelación de permisos con `status='cancelled'`;
- conflicto optimista correcto en `tenant_info`;
- nombre de empleado visible en marcaciones y novedades.

Persisten, como mínimo, estos bloqueos de release:

- `npm ci` falla porque `package.json` y `package-lock.json` no están
  sincronizados;
- el último CI de `main` falla en Backend Coverage, Frontend Coverage y E2E;
- `test_reset_gating` depende de una variable de entorno global y presenta una
  carrera bajo ejecución paralela;
- `license_bypass_safety` puede cargar `backend/.env`, contaminando el
  subprocess pese a usar `env_clear()`;
- frontend y backend difieren en el DTO de dispositivos;
- las rutas frontend de captura y retry de enrolamiento no coinciden con Axum;
- la sesión no se restaura al cargar páginas que esperan un rol antes de hacer
  requests;
- SSE no se conecta si el token aparece después del montaje inicial;
- el plan de cola existente fue escrito después de implementar gran parte de
  sus Sprints 1–4 y no refleja el estado real;
- el instalador público no es descargable y no existe un flujo completo de
  publicación de imágenes privadas;
- `main` no tiene protección de rama.

## 3. Decisiones aprobadas

### D-01 — Topología documental

Se crearán Fases 12 y 13 dentro de `.planning/phases/`. La Fase 11 se preserva
como evidencia histórica parcial; no se reescribe para aparentar una ejecución
que no ocurrió.

### D-02 — Orden gate-first

Primero se restaura una línea base reproducible. Después se ejecutan en paralelo
los frentes funcional y de persistencia. Empaquetado y validación final sólo se
cierran cuando ambos convergen.

### D-03 — Distribución privada

Las imágenes se publicarán como paquetes privados en GHCR. Cada instalación
recibirá una credencial revocable y de sólo lectura. Según la documentación
oficial vigente, GHCR requiere un personal access token clásico con al menos
`read:packages` para instalar paquetes privados:

- <https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry>

El token se entregará al instalador por entrada protegida o variable de entorno,
se enviará a `docker login ghcr.io` mediante `--password-stdin` y nunca se
imprimirá ni se guardará en el repositorio. Docker almacenará la sesión en un
credential helper cuando esté disponible; el fallback será un `DOCKER_CONFIG`
dedicado, propiedad de `root`, con permisos `0700` para el directorio y `0600`
para `config.json`. La documentación de Docker advierte que, sin helper, el
archivo contiene credenciales sólo codificadas en base64:

- <https://docs.docker.com/reference/cli/docker/login/>

Los tokens se emitirán desde una cuenta técnica que sólo tenga lectura sobre
los paquetes Cronometrix. Habrá un PAT por instalación, con vencimiento y dueño
registrados, para poder revocar un cliente sin afectar a los demás. Si la
organización exige SSO, el token deberá quedar autorizado antes de la entrega.

El instalador no se publicará en un hostname anónimo. CI generará un bundle
privado verificado por checksum que contiene `install.sh`, `docker-compose.yml`
y el manifest de release. Un operador autenticado descargará ese bundle desde
los artefactos privados y lo transferirá al host del cliente. Desde ese punto la
instalación seguirá siendo de un comando: `sudo bash install.sh`. La credencial
del cliente sólo tendrá `read:packages`; no recibirá acceso al repositorio ni a
los artefactos de CI.

### D-04 — Validaciones diferidas

La prueba con un Hikvision físico y la prueba LIC-05 cross-host no bloquearán
v1.0. Permanecerán como riesgos aceptados, con dueño, condición de ejecución y
fecha límite ligada a la primera instalación productiva. Los mocks y pruebas
locales no se describirán como sustitutos de esas validaciones.

### D-05 — Semántica semanal de marcaciones

La vista semanal mantendrá una fila por empleado y día. `anchor_date` será
visible y formará parte de la identidad de la fila, del deep-link y de las
pruebas. No se agregará toda la semana en una única fila.

### D-06 — Evidencia liviana

Git conservará manifests, comandos, resúmenes, hashes y enlaces. HTML de
cobertura, videos, trazas y capturas pesadas se conservarán como artefactos
privados de CI durante 14 días.

### D-07 — Entrada HTTP única

Docker Compose incorporará un gateway HTTP interno con imagen fijada por
versión y digest. El gateway será Nginx con una configuración propia, mínima y
versionada. Enviará `/api/*` y SSE al backend, y el resto al frontend.
Cloudflare Tunnel tendrá un solo origen: el gateway. Esto elimina la dependencia
de `NEXT_PUBLIC_API_URL` embebida en build y mantiene cookies, CORS y
EventSource bajo el mismo origen.

## 4. Arquitectura del trabajo

```text
12-01 Baseline reproducible y CI
          |
          +--------------------+
          |                    |
          v                    v
12-02 Contratos y UX     12-03 Persistencia y auditoría
          |                    |
          +----------+---------+
                     v
          12-04 Distribución privada
                     |
                     v
          12-05 Gate técnico completo
                     |
                     v
          Fase 13 Validación viva
```

`12-02` y `12-03` pueden avanzar en paralelo después de `12-01`. La preparación
del registry y del gateway puede comenzar antes, pero `12-04` no se considera
cerrado hasta construir las imágenes con el código convergido. `12-05` exige
todos los resultados sobre la misma SHA.

## 5. Fase 12 — v1.0 Release Stabilization

### 12-01 — Rebaseline, dependencias y harness de pruebas

Responsabilidades:

1. Registrar `1dd6d758` como baseline de remediación.
2. Corregir `STATE.md`, `ROADMAP.md` y `REQUIREMENTS.md` para que no declaren
   100% mientras existan fases activas.
3. Regenerar `package-lock.json` con la versión de Node usada por CI y demostrar
   `npm ci` desde un checkout limpio.
4. Sustituir `npm install` por `npm ci` en el build del frontend.
5. Hacer determinista `test_reset_gating` sin carreras sobre el entorno global.
6. Ejecutar subprocesses de licencia desde un cwd aislado donde `dotenvy` no
   encuentre `backend/.env`.
7. Ejecutar formato, compilación, tests y cobertura para descubrir la lista real
   de fallos; no tomar los conteos históricos como evidencia.

Criterio de salida:

- `npm ci` es reproducible;
- todas las suites pueden arrancar desde cero;
- los fallos restantes representan comportamiento del producto, no roturas del
  harness;
- CI publica resultados aun cuando una prueba falle.

### 12-02 — Contratos funcionales y experiencia autenticada

#### Sesión y refresh

`AuthProvider` usará los estados `initializing`, `authenticated` y `anonymous`.
Durante `initializing` no resolverá RBAC como si el usuario fuese anónimo. Al
montarse, ejecutará un refresh inicial usando la cookie httpOnly.

El cliente HTTP tendrá una sola promesa de refresh compartida. Si varias
requests reciben 401 simultáneamente, una rota el refresh token y las demás
esperan su resultado. Un refresh exitoso reintenta cada request una vez; un
refresh fallido limpia el access token y lleva la sesión a `anonymous` sin
bucles.

#### Dispositivos

El DTO backend es la fuente de verdad:

- `ip: string`;
- `connection_state: online | offline | unknown`;
- `status: active | inactive`;
- `direction: entry | exit`.

El frontend eliminará `ip_address`, dejará de interpretar `status` como
conectividad y retirará la opción `both`. Un fixture producido desde JSON real
del endpoint alimentará las pruebas de consumidor.

#### Enrolamiento

Un cliente tipado centralizará estas rutas:

- crear enrolamiento;
- obtener enrolamiento;
- capturar desde dispositivo;
- consultar captura;
- reintentar un push por dispositivo;
- listar enrolamientos en progreso.

La captura devolverá el dispositivo de origen junto con la imagen. Tanto upload
como captura Hikvision pasarán por la misma validación de calidad antes de
habilitar submit. Iniciar otro empleado limpiará captura, dispositivo,
enrollment ID y polling anteriores. La lista de procesos en progreso se
recuperará desde backend, por lo que sobrevivirá a reload.

#### Marcaciones

La tabla mostrará fecha, empleado, departamento, entrada, salida, novedades y
acciones. La key será `(employee_id, anchor_date)`. La investigación de la issue
#4 comparará el response con esa clave para distinguir duplicados reales de
varios días visualmente indistinguibles.

#### SSE y dashboard

El hook SSE dependerá del access token y abrirá o cerrará la conexión cuando el
estado autenticado cambie. Aplicará backoff acotado y no reintentará después de
logout. El backend enriquecerá el payload con empleado y departamento antes de
publicarlo. El frontend usará el endpoint autenticado de foto cuando
`has_photo=true`, con iniciales como fallback, y renderizará 20 eventos.

Criterio de salida:

- hard reload conserva una sesión válida en páginas RBAC;
- un refresh concurrente rota el token una sola vez;
- dispositivos muestran IP, conectividad y estado correctos;
- enrolamiento llega a captura, submit, fan-out, fallo parcial y retry;
- marcaciones muestran días distintos como filas inequívocas;
- SSE vuelve a conectarse después de restaurar sesión.

### 12-03 — Persistencia serializada y auditoría legal

#### Contrato de `DbWriteQueue`

La cola usará un canal acotado de 1.024 comandos. Cuando alcance capacidad, los
productores esperarán hasta cinco segundos; no se crearán buffers ilimitados.
Una request que no pueda encolar dentro del plazo recibirá HTTP 503 con código
`DB_WRITE_QUEUE_BUSY`; un worker reintentará tres veces con esperas de 100, 250
y 500 ms antes de registrar un fallo terminal y activar su mecanismo de
recuperación de dominio. Los jobs transaccionales podrán devolver resultados
tipados sin abrir una segunda conexión de escritura.

El cierre tendrá tres estados:

1. dejar de aceptar nuevos jobs;
2. procesar todos los jobs aceptados y confirmar un `flush`;
3. terminar y esperar el handle del worker.

La cancelación global no tendrá prioridad sobre el drenaje. Se expondrán logs o
métricas de profundidad, espera, duración, fallo y rechazo por cierre.

#### Invariante queue-only

Toda mutación de producción pasará por la cola. Las únicas excepciones serán:

- migraciones antes de servir tráfico;
- seed E2E en un proceso aislado;
- reset E2E registrado sólo en modo de prueba;
- el propio worker de `DbWriteQueue`.

Un script con allowlist exacta inspeccionará `backend/src` y fallará en CI si
aparece un write directo nuevo.

#### Atomicidad

Estas operaciones serán un único job transaccional:

- overlap-check + insert de permisos;
- enrolamiento + filas de push iniciales;
- recomputación diaria + reemplazo de anomalías;
- cambios de estado de push y mapping cuando formen una sola transición;
- persistencia del audit de exportación después de calcular el reporte.

Los archivos de evidencia conservarán el patrón temporal + rename. Si la DB
falla después de crear un archivo, el flujo eliminará el huérfano; si el archivo
falla, no se encolará la mutación.

#### Inmutabilidad de auditoría

Una migración añadirá triggers `BEFORE UPDATE` y `BEFORE DELETE` sobre
`audit_log` con `RAISE(ABORT)`. Las pruebas E2E no borrarán el audit log: usarán
una base por ejecución, IDs únicos y timestamps/markers para aislar aserciones.
Las mutaciones de asistencia conservarán actor y justificación verificables.

#### Prueba de carga

El script existente `backend/scripts/load_test.sh` se reutilizará. Los perfiles
mínimos serán:

- concurrencia 1, 100% writes;
- concurrencia 32, 100% reads;
- concurrencia 32, 100% writes;
- concurrencia 32, mezcla 70% writes / 30% reads.

Cada perfil durará 60 segundos y guardará JSON/CSV. El gate exige cero HTTP 500,
cero `database is locked`, cero pérdida durante shutdown y comparación de
p50/p95/p99 con el baseline de un hilo.

### 12-04 — Distribución privada e instalador

El workflow de release construirá backend y frontend, ejecutará los gates y
publicará sólo desde un tag de release. CI usará `GITHUB_TOKEN` con permisos
mínimos para publicar; los clientes usarán PAT classic de sólo lectura para
pull.

Cada release generará:

- imágenes `api` y `web` con tag de versión inmutable;
- digest SHA-256 de cada imagen;
- `docker-compose.yml` que referencia los digests aprobados;
- `install.sh` y checksum del script;
- manifest JSON con versión, commit, imágenes y digests.

El instalador:

1. valida SO, arquitectura, Docker y espacio disponible;
2. recibe credencial GHCR sin eco;
3. autentica mediante `--password-stdin`;
4. descarga manifest, compose e imágenes verificando checksums/digests;
5. conserva secretos de aplicación existentes en reejecuciones;
6. no reactiva una licencia ya activada;
7. inicia DB/API, luego web/gateway/túnel;
8. comprueba salud y muestra un resumen sin secretos;
9. revierte a la versión anterior si el smoke no pasa.

El gateway Nginx mantendrá buffering desactivado para SSE y límites de upload
compatibles con enrolamiento/evidencias. Cloudflare Tunnel apuntará al gateway,
no directamente a servicios internos.

### 12-05 — Gate técnico completo

La misma SHA debe superar:

- `cargo fmt --all -- --check`;
- `cargo clippy --all-targets --all-features -- -D warnings`;
- tests backend y cobertura con umbrales del repositorio;
- chequeo queue-only;
- `npm ci`, typecheck, build y Vitest con cobertura;
- Playwright completo;
- build de imágenes;
- smoke Docker single-origin, cookies y SSE;
- análisis de que ningún `todo!()` ignorado se contabilice como evidencia de
  cobertura funcional.

No se protegerá `main` ni se declarará v1.0 mientras estos gates no estén verdes
en la misma ejecución y commit.

## 6. Fase 13 — v1.0 Live Validation

### 13-01 — Baseline local y CI verde

Ejecutar `make coverage` y `make e2e` localmente. Después, ejecutar los tres
jobs actuales y los nuevos gates de release en GitHub sobre la misma SHA.

### 13-02 — Regresión deliberadamente roja

Abrir un PR temporal que rompa de forma controlada backend, frontend y E2E.
Capturar que cada gate bloquea el PR, retirar el cambio y cerrar el PR sin merge.

### 13-03 — Protección de `main`

Exigir los checks por nombre, review antes de merge y rama actualizada. Confirmar
mediante API que la protección está activa.

### 13-04 — VM limpia y túnel

En una VM Linux soportada:

1. ejecutar el instalador con credencial GHCR de sólo lectura;
2. verificar descarga por digest;
3. activar licencia mediante DO Functions;
4. comprobar reinicio offline con licencia cacheada;
5. validar web, API, cookies y SSE desde el hostname Cloudflare;
6. repetir el instalador y demostrar idempotencia;
7. revocar la credencial de prueba al cerrar la evidencia.

### 13-05 — Sign-off y riesgos diferidos

Actualizar `STATE.md`, `ROADMAP.md`, `REQUIREMENTS.md` y el milestone audit sólo
después de completar 13-01 a 13-04. Mantener dos registros de riesgo:

- Hikvision físico: ejecutar antes o durante la primera instalación productiva;
- LIC-05 cross-host: ejecutar con dos hosts reales durante esa misma ventana.

Cada registro incluirá propietario, pasos, evidencia esperada, impacto si falla
y criterio de rollback.

## 7. Manejo de errores y rollback

- Un fallo de refresh termina en sesión anónima; nunca en un bucle de requests.
- Saturación de la cola produce backpressure o error controlado, no crecimiento
  ilimitado de memoria.
- Un job transaccional falla completo y conserva el error de dominio esperado.
- Una imagen que no coincide con el digest no se ejecuta.
- Si el smoke del release falla, Compose vuelve al manifest anterior.
- Revocar una credencial GHCR impide upgrades futuros, pero no detiene los
  contenedores ya descargados.
- Las migraciones de auditoría se respaldan y prueban sobre una copia antes de
  aplicarlas a una instalación existente.

## 8. Estrategia de pruebas

Cada cambio seguirá red-green-refactor y tendrá una unidad de commit revisable.
Las pruebas se distribuyen así:

- **Unitarias:** state machines, mapeos DTO, refresh single-flight, políticas de
  retry, lógica de cola y validación de manifests.
- **Integración backend:** transacciones, conflictos optimistas, inmutabilidad
  de auditoría, shutdown y ausencia de writes directos.
- **Contrato:** respuesta Rust real consumida por tipos y adaptadores frontend.
- **Frontend:** estados de sesión, tablas, modales, SSE y errores visibles.
- **E2E:** reload autenticado, dispositivos, enrolamiento completo, marcaciones,
  reportes/auditoría y RBAC.
- **Carga:** perfiles definidos en 12-03.
- **Distribución:** checkout limpio, build, pull privado, VM limpia, reejecución y
  rollback.

## 9. Criterios de éxito de v1.0

Cronometrix estará listo para sign-off cuando:

1. todos los gates técnicos pasen sobre una SHA única;
2. `main` esté protegida;
3. una VM limpia instale imágenes privadas por digest;
4. web, API, cookies y SSE funcionen bajo el hostname final;
5. el instalador sea idempotente y tenga rollback demostrado;
6. audit log sea inmutable en la base de datos;
7. no existan rutas productivas de escritura fuera de `DbWriteQueue`;
8. la documentación refleje el estado real;
9. Hikvision y LIC-05 permanezcan explícitamente como riesgos diferidos, no
   como validaciones aprobadas.

## 10. Fuera de alcance

- nuevas funciones de negocio ajenas a los hallazgos;
- soporte para más de cuatro dispositivos;
- aplicación Tauri;
- sustitución de Axum, Next.js, SQLite/libSQL o Cloudflare Tunnel;
- ejecución inmediata de las dos pruebas físicas formalmente diferidas.
