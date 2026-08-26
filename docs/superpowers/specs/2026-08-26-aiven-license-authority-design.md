# Autoridad de licencias con Aiven PostgreSQL

Fecha: 2026-08-26
Estado: aprobado en conversación, pendiente de revisión de la especificación

## Problema

Cronometrix ya contiene el dominio de licencias, las Functions de activación y
renovación y el instalador, pero la autoridad central nunca se desplegó. No
existe un namespace de Cronometrix en DigitalOcean, una base con licencias
emitidas ni una clave privada de producción disponible. Por eso el instalador
de la NanoPi no puede completar el paso `License key` de forma legítima.

La instalación no puede resolverse con `CRONOMETRIX_LICENSE_BYPASS`: ese flag
es exclusivo de E2E y el binario aborta si aparece fuera de ese contexto. La
autoridad tampoco puede vivir en la NanoPi, porque colocar la clave privada y
el registro de licencias junto al cliente anularía el límite anti-clonación.

## Objetivo

Desplegar la autoridad central existente en DigitalOcean Functions usando el
servicio Aiven PostgreSQL proporcionado por el operador, emitir una licencia
para `pi@192.168.1.239`, publicar una API ARM64 que confíe en la nueva clave
pública y finalizar la instalación sin revelar credenciales al agente.

## Fuera de alcance

- Panel comercial para crear, revocar o reasignar licencias.
- Migrar las Functions a Aiven Runtime.
- Añadir planes, cobros o límites por número de empleados/dispositivos.
- Rotación automática de la clave RSA. Esta primera entrega documenta y prueba
  la rotación, pero la operación sigue siendo deliberada y manual.
- Cambiar la vigencia anual y la renovación diaria ya decididas en Fase 6.

## Decisión arquitectónica

Se conserva DigitalOcean Functions como adaptador HTTP de la autoridad y se
reemplaza únicamente su almacenamiento por Aiven PostgreSQL. El dominio de
licencias no cambia: activación vincula una clave sin usar al fingerprint del
primer equipo; renovación sólo firma si el fingerprint coincide.

Se descartaron dos alternativas:

1. Aiven Runtime para Functions y base: consolida proveedor, pero obliga a
   reemplazar un adaptador de despliegue que ya existe y amplía el riesgo del
   primer lanzamiento.
2. Autoridad en la NanoPi: compromete la clave de firma, permite alterar el
   registro local y contradice LIC-05.

La dirección de dependencias permanece hexagonal:

```text
NanoPi/API Rust -> puerto HTTP de licencias -> DO Functions
                                              |
                                              v
                                  adaptador pg/TLS -> Aiven
```

La API Rust sólo conoce URLs de activación/renovación y una clave pública. Las
Functions conocen el puerto de persistencia mediante el adaptador `pg`; Aiven
no entra en el dominio ni en el instalador del cliente.

## Aislamiento en Aiven

El servicio Aiven puede ser compartido, pero Cronometrix no usará `defaultdb`
ni `avnadmin` en runtime.

- Base: `cronometrix_licenses`.
- Esquema: `license_authority`.
- Propietario sin login: `cronometrix_license_owner`.
- Rol runtime con login: `cronometrix_license_runtime`.
- Tabla: `license_authority.licenses`.

`cronometrix_license_runtime` recibe sólo `CONNECT`, `USAGE`, `SELECT` y
`UPDATE` sobre la tabla. No puede insertar claves, eliminarlas, alterar el
esquema ni crear objetos. La emisión usa la conexión administrativa inyectada
al comando operacional, nunca la credencial runtime de las Functions.

El aprovisionamiento es idempotente: comprueba y crea base, roles, esquema,
tabla y grants. Una segunda ejecución converge al mismo estado sin cambiar el
password runtime ni perder fingerprints ya vinculados.

## TLS y adaptador PostgreSQL

Aiven obliga TLS. `sslmode=require` cifra el tránsito, pero no prueba que el
certificado pertenezca a la CA de Aiven; producción usará la CA descargada del
servicio con `rejectUnauthorized: true`.

Se extraerá la configuración duplicada de `activate/index.js` y
`renew/index.js` a un adaptador compartido. Éste:

1. exige `DATABASE_URL` y `DATABASE_CA_CERT_BASE64`;
2. decodifica la CA en memoria;
3. elimina de la URL `sslmode`, `sslcert`, `sslkey` y `sslrootcert` antes de
   crear el cliente;
4. construye `new Client({ connectionString, ssl: { ca,
   rejectUnauthorized: true }, connectionTimeoutMillis: 5000,
   query_timeout: 5000 })`;
5. nunca registra URL, password, CA ni consultas con parámetros.

Eliminar esos parámetros es obligatorio: `node-postgres` reemplaza el objeto
`ssl` programático cuando la URL trae opciones SSL, lo que descartaría la CA
verificada aunque el código pareciera configurarla.

Las consultas se califican como `license_authority.licenses`. La persistencia
continúa parametrizada; no se interpolan claves ni fingerprints en SQL.

## Cadena criptográfica

Se generará un par RSA-2048 nuevo porque no existe una autoridad de producción
desplegada y no se reutilizarán claves de prueba o históricamente expuestas.

- Privada: `cronometrix-license-private-key-pem` en `secretctl`, inyectada a
  `LICENSE_PRIVATE_KEY` sólo durante el despliegue y almacenada como variable
  cifrada de DigitalOcean Functions.
- Pública: `backend/src/license/pubkey.pem`, versionada y embebida al compilar
  la API.

Un helper ejecutado por el operador genera el par en un directorio temporal
`0700`, introduce la privada directamente en `secretctl`, copia sólo la pública
al repositorio y elimina el temporal. El agente nunca recibe la privada. Antes
de publicar, una prueba firma un JWT efímero mediante `secretctl run` y exige
que la clave pública versionada lo valide; no imprime token ni material PEM.

Cambiar la pública obliga a publicar una imagen API nueva. No se despliegan las
Functions antes de que esa imagen haya pasado CI, para evitar una ventana donde
el servidor emita JWT que la API disponible no puede verificar.

## Flujo de secretos

Entradas de `secretctl`:

| Clave | Uso |
|---|---|
| `cronometrix-aiven-admin-url` | Aprovisionar base, roles, esquema y emitir licencias |
| `cronometrix-aiven-license-password` | Password estable del rol runtime |
| `cronometrix-aiven-ca-base64` | Verificación TLS de Aiven |
| `cronometrix-license-private-key-pem` | Firma RS256 en Functions |
| `cronometrix-license-nanopi-192-168-1-239` | Primera licencia emitida |

`secretctl run` transforma esos nombres en variables mayúsculas con guiones
convertidos a `_`. Los scripts consumen variables, no llaman `secretctl get` y
no usan `set -x`. La sanitización de salida queda habilitada. Ningún secreto se
escribe en argumentos, archivos del repo, logs, GitHub Actions ni el chat.

El password administrativo de Aiven se usa sólo en el comando de
aprovisionamiento. Las Functions reciben una URL derivada para el rol runtime.
La URL derivada no se imprime y se conserva en DigitalOcean sólo como variable
de entorno del namespace.

## Componentes operacionales

### Helper de preparación de secretos

Un script guiado prepara los tres valores que aún no existan: password runtime,
CA codificada y par RSA. Cada escritura al vault ocurre mediante `secretctl`
en la terminal del operador. Re-ejecutar no rota valores existentes sin una
opción explícita.

### Aprovisionador Aiven

Un módulo Node usa `pg` y el mismo constructor TLS probado que las Functions.
Recibe la URL administrativa y el password runtime por entorno, crea los
objetos idempotentes y verifica al final los grants efectivos. La salida sólo
contiene nombres de objetos y estados, nunca cadenas de conexión.

### Despliegue DigitalOcean

Se crea o selecciona un namespace dedicado `cronometrix` en `nyc1`; nunca se
despliega sobre los namespaces existentes `tiquemax`, `somonexa` o
`tiquemax-pdf-fn`. `project.yml` pasa las tres variables requeridas:
`DATABASE_URL`, `DATABASE_CA_CERT_BASE64` y `LICENSE_PRIVATE_KEY`.
También registra `SOURCE_SHA`, calculado desde un `origin/main` limpio, para
conservar la cadena de custodia entre código, Functions y release.

Después del despliegue se obtienen las URLs públicas de
`licenses/activate` y `licenses/renew`, se ejecutan probes de entradas
inválidas que deben responder 400 sin filtrar detalles y se conservan las URLs
como datos no secretos para el instalador.

### Emisión

La clave tiene cuatro grupos de cuatro caracteres alfanuméricos mayúsculos y
entropía criptográfica. Se guarda primero en `secretctl` y luego un comando
inyectado hace `INSERT ... ON CONFLICT DO NOTHING` usando la conexión admin. Si
la fila ya existe, sólo acepta que siga sin vincular o que pertenezca al mismo
fingerprint; nunca reinicia un vínculo existente.

## Secuencia de entrega

1. Añadir pruebas y soporte de CA verificada al adaptador `pg`.
2. Añadir y probar los helpers de aprovisionamiento, despliegue y emisión.
3. Generar la clave pública de producción sin exponer la privada.
4. Ejecutar tests Node, Rust, release y secret scanning.
5. Crear PR, esperar CI y fusionar a `main`.
6. Publicar y verificar imágenes `linux/amd64` + `linux/arm64` por digest.
7. Añadir `License Functions` a la protección de `main` sin retirar checks
   existentes.
8. Aprovisionar Aiven y desplegar las Functions desde el commit fusionado.
9. Emitir la licencia y reanudar el instalador con URLs y secretos inyectados.
10. Verificar licencia, contenedores, health, persistencia y acceso por túnel.

El orden evita firmar con una clave privada cuya pública todavía no esté en una
API aprobada y evita instalar una imagen no reproducible desde la rama local.

## Errores, recuperación y rollback

- CA ausente o inválida: configuración falla antes de abrir conexión.
- Certificado no verificable: conexión fail-closed; nunca se degrada a
  `rejectUnauthorized: false`.
- Aiven inaccesible durante activación: la API devuelve error de activación y
  no crea `license.jwt`.
- Aiven inaccesible después de activar: el JWT cacheado mantiene operación
  offline hasta su expiración, conforme a DEPL-04.
- Despliegue DO fallido: la instalación permanece cancelada y no se emite
  licencia; los objetos Aiven idempotentes pueden reutilizarse al reintentar.
- API nueva fallida: se conserva el release anterior; la autoridad no se
  despliega hasta que el release nuevo esté verificado.
- Licencia vinculada a otro fingerprint: no se modifica la fila; se investiga
  antes de cualquier reasignación manual.

La eliminación de base, roles, namespace o claves queda fuera de los scripts
normales. Todo rollback de datos es explícito para evitar perder trazabilidad.

## Pruebas

### Node unitarias

- URL con `sslmode=require` se normaliza sin perder host, puerto, usuario,
  password ni base.
- Configuración siempre incluye CA y `rejectUnauthorized: true`.
- CA/URL ausentes o base64 inválido fallan sin incluir valores en el error.
- Activate y renew consultan la tabla calificada y preservan respuestas
  200/400/403/404/409/500 existentes.
- Tests de concurrencia de vinculación siguen verdes.

### Aprovisionamiento

- Dry-run valida entradas sin mutar Aiven.
- Dos ejecuciones producen el mismo esquema y grants.
- Rol runtime puede `SELECT`/`UPDATE` y no puede `INSERT`, `DELETE`, `CREATE` ni
  acceder a otras bases del servicio.
- El probe comprueba TLS verificado, no sólo conexión exitosa.

### Rust y release

- La pública versionada parsea como RSA-2048.
- JWT firmado con la privada inyectada valida con la pública versionada.
- Suite de licencia, cobertura, E2E, container smoke y release gate pasan.
- Los tres manifiestos publicados incluyen `linux/arm64`.

### Validación live

- Functions inválidas responden 400 y no filtran configuración.
- Licencia nueva activa exactamente una vez contra el fingerprint de la
  NanoPi y crea `license.jwt` con modo `0600`.
- `/api/v1/setup/status` devuelve `licensed: true`.
- API, web, gateway y cloudflared quedan healthy.
- Reinicio de Docker conserva licencia, base local y acceso.

## Criterio de éxito

`main` contiene el adaptador Aiven/TLS y la clave pública aprobados; CI y el
release multi-arquitectura están verdes; un namespace dedicado ejecuta las dos
Functions contra un rol Aiven restringido; la licencia emitida queda vinculada
a la NanoPi; y Cronometrix responde sano localmente y mediante su túnel sin
ningún bypass de pruebas ni secreto expuesto al agente.
