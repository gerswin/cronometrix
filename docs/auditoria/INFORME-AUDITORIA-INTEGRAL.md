# Auditoría integral de lógica de negocio

**Proyecto:** Cronometrix

**Versión auditada:** commit `9b36341`

**Fecha de corte:** 2 de agosto de 2026

**Jurisdicción evaluada:** Venezuela

**Ámbito:** control de asistencia biométrico y generación de pre-nómina

## Contenido

1. Informe ejecutivo
2. Informe técnico detallado

# Parte I — Informe ejecutivo

## Dictamen ejecutivo

Cronometrix no debe presentarse ni utilizarse, en su estado actual, como sistema listo para calcular una nómina legalmente correcta. La base técnica contiene controles valiosos, pero existen defectos determinísticos que pueden pagar de más, pagar de menos, omitir trabajadores, duplicar importes y perder marcaciones. También hay fallas críticas en la emisión de licencias y en la exposición de secretos de ingreso de dispositivos.

La auditoría catalogó **40 hallazgos**: **10 críticos, 16 altos, 12 medios y 2 bajos**. La prioridad inmediata no es añadir funcionalidad, sino impedir resultados financieros incorrectos, cerrar las vías de falsificación o pérdida de eventos y retirar afirmaciones comerciales de cumplimiento que hoy no están sustentadas.

El producto sí puede evolucionar hacia una solución sólida de **control de asistencia y pre-nómina**, siempre que se delimite contractualmente ese alcance, se corrija el motor monetario, se implante un calendario laboral venezolano configurable, se preserve el histórico y se incorporen controles verificables de privacidad, auditoría y operación.

## Riesgo agregado

| Área | Nivel | Conclusión |
|---|---:|---|
| Exactitud de pagos | Crítico | Horas extra y retrasos se remuneran/deducen dos veces; algunos salarios quedan en cero; ausentes y anulaciones pueden desaparecer o duplicarse. |
| Integridad de asistencia | Crítico | Una marcación nocturna puede atribuirse al día equivocado y el receptor `push` confirma antes de persistir, con pérdida definitiva posible. |
| Licenciamiento | Crítico | La clave privada correspondiente a la clave pública embebida está en el repositorio y la activación concurrente permite ligar una licencia a más de un equipo. |
| Acceso y fraude | Crítico | El token de ingreso del dispositivo queda en logs; el alta inicial admite una carrera que puede crear dos administradores. |
| Cumplimiento laboral | Alto | Faltan feriados, descansos configurables, control efectivo y autorización de horas extra, vacaciones completas y acceso fiable a la configuración nocturna. |
| Privacidad biométrica | Alto | No existe ciclo de vida demostrable de consentimiento/base jurídica, finalidad, retención, acceso, corrección ni supresión. |
| Trazabilidad y cierre | Alto | No hay nómina cerrada, instantánea histórica, aprobación, artefacto firmado ni auditoría completa del actor y del cálculo. |
| Promesas comerciales | Alto | La propuesta afirma preparación para nómina, cumplimiento y trazabilidad total que la implementación no satisface. |
| Operación y suministro | Medio–Alto | Hay dependencias con avisos de seguridad, pruebas reales pendientes y respaldos incompletos de archivos biométricos/evidencias. |

## Hallazgos críticos

| ID | Problema | Consecuencia principal | Acción inmediata |
|---|---|---|---|
| C-01 | Las horas extra forman parte del pago ordinario y luego se vuelven a sumar con recargo de 150 %. | La hora extra termina pagándose al 250 % del valor base. | Separar minutos ordinarios y extraordinarios y crear pruebas con ejemplos legales. |
| C-02 | El retraso reduce los minutos trabajados y además se descuenta monetariamente otra vez. | Subpago sistemático. | Aplicar una sola política de descuento, explícita y trazable. |
| C-03 | La interfaz promete heredar el salario del departamento, pero el backend guarda cero y el reporte no aplica herencia. | Nómina de valor cero para trabajadores válidos. | Resolver el salario efectivo en escritura o cálculo y bloquear salarios inválidos. |
| C-04 | Pueden coexistir varias anulaciones activas para el mismo registro; el reporte las une todas. | Filas e importes duplicados. | Índice único parcial, revocación transaccional y migración de datos existentes. |
| C-05 | El reporte parte de registros existentes; un empleado totalmente ausente puede no aparecer. | Ausencias y descuentos omitidos. | Generar el universo desde empleados activos y calendario esperado. |
| C-06 | El repositorio contiene la clave privada que firma las licencias aceptadas por la aplicación. | Cualquier poseedor del código puede emitir licencias válidas. | Rotar inmediatamente el par, sacar la firma a KMS/HSM y revocar la confianza anterior. |
| C-07 | La comprobación y vinculación de activación de licencia no es atómica. | Una licencia puede activarse simultáneamente en dos equipos. | Actualización condicional atómica y prueba concurrente. |
| C-08 | El token secreto del receptor `push` aparece en la ruta y se registra en logs. | Un tercero con acceso a logs puede inyectar marcaciones y alterar pagos. | Mover el secreto a cabecera, rotarlo y sanear todos los niveles de logging. |
| C-09 | La creación del primer administrador comprueba y escribe en operaciones separadas. | Dos solicitudes concurrentes pueden crear dos administradores iniciales. | Transacción exclusiva o restricción de unicidad sobre el estado de inicialización. |
| C-10 | El receptor de dispositivos responde `200` antes de garantizar la persistencia y absorbe errores internos. | El dispositivo elimina de su cola una marcación que Cronometrix nunca guardó. | Bandeja de entrada durable antes del acuse, reintentos e indicador de cola muerta. |

## Incumplimientos y brechas legales relevantes

La referencia laboral principal es la **Ley Orgánica del Trabajo, los Trabajadores y las Trabajadoras (LOTTT)**. La propuesta comercial cita artículos incorrectos: el límite de jornada está en el artículo 173, el registro de horas extra en el 183 y el artículo 187 trata feriados regionales, no trazabilidad. El producto tampoco implementa todos los límites de los artículos 173 y 178, la autorización del artículo 182, el registro detallado del artículo 183, los feriados/descansos de los artículos 184 y 188, ni el régimen completo de vacaciones y bono vacacional de los artículos 190, 192 y 203.

En protección de datos no existe una ley general venezolana equivalente al RGPD, pero sí obligaciones directamente relevantes: los artículos 28 y 60 de la Constitución protegen el acceso, propósito, corrección, destrucción, privacidad e imagen; el artículo 56.10 de la LOPCYMAT reconoce el acceso de la persona trabajadora a sus datos; y la Ley Especial contra los Delitos Informáticos sanciona accesos, usos o divulgaciones indebidos. Los rostros, fotos, evidencias médicas y marcaciones requieren, por tanto, un gobierno explícito que hoy no existe.

La aplicación tampoco constituye por sí sola un sistema integral de nómina: no calcula ni gestiona de forma completa IVSS, FAOV, INCES, ISLR, prestaciones y demás conceptos aplicables. La facturación comercial debe evaluarse conforme a las providencias SENIAT vigentes, en particular SNAT/2024/000102 para operaciones electrónicas. La aceptación comercial de USDT/Binance requiere además análisis tributario y contable específico, incluido el IGTF cuando corresponda; no debe generalizarse una tasa o tratamiento sin determinar el sujeto y el canal de pago.

Las referencias ISO, NIST, OWASP y RFC utilizadas en esta auditoría son buenas prácticas voluntarias, salvo incorporación contractual o regulatoria. Sirven como criterio técnico, no se presentan como leyes venezolanas.

## Controles positivos encontrados

La situación es corregible porque ya existen piezas útiles:

- contraseñas con Argon2id y rotación de tokens de refresco con detección de reutilización;
- cifrado AES-GCM de credenciales de dispositivos y exclusión de esas credenciales de respuestas públicas;
- escritor SQLite serializado y validación transaccional de solapamiento de permisos;
- protección en base de datos contra actualización o borrado directo del registro de auditoría;
- protecciones de ruta, enlace simbólico y propiedad para archivos;
- puntos de recuperación de operaciones sobre dispositivos;
- puertos internos no publicados al host y permisos restrictivos para datos y entorno;
- una suite automatizada amplia: backend, frontend y funciones de licenciamiento ejecutaron correctamente en el entorno compatible.

Estos controles no compensan los errores de negocio, pero reducen el esfuerzo de remediación.

## Decisiones recomendadas

1. **Suspender la afirmación “listo para nómina/cumplimiento”.** Posicionar temporalmente el producto como control de asistencia/pre-nómina sujeto a revisión humana y conciliación externa.
2. **Bloquear releases comerciales hasta resolver C-01 a C-10.** Las correcciones deben incluir migraciones, pruebas de regresión y reconciliación de datos ya calculados.
3. **Rotar hoy la clave de licenciamiento y los tokens `push`.** Considerar comprometidos los secretos presentes en repositorio o logs, aun si no hay evidencia de explotación.
4. **Adoptar un modelo laboral efectivo por fecha.** Calendarios, turnos, salario, reglas, feriados, descansos, autorizaciones y topes deben conservar vigencia histórica.
5. **Crear un cierre de período.** El cálculo aprobado debe quedar congelado, versionado, conciliado y asociado a reglas, eventos, anulaciones y artefactos exportados.
6. **Implantar gobierno de datos biométricos y médicos.** Inventario, finalidad, base jurídica/consentimiento cuando corresponda, acceso, corrección, retención, supresión, responsables y contratos con proveedores.
7. **Validar en entorno real antes de producción.** Hardware Hikvision objetivo, pérdida/reintento de red, turnos nocturnos, concurrencia, instalación limpia, restauración integral, Cloudflare/Turso y activación entre equipos.

## Hoja de ruta priorizada

| Horizonte | Resultado esperado | Impacto | Esfuerzo orientativo |
|---|---|---:|---:|
| 0–72 horas | Retirar promesas no sustentadas; rotar clave y tokens; desactivar o proteger `push`; impedir nuevas activaciones vulnerables. | Muy alto | Bajo–Medio |
| 1–2 semanas | Corregir cálculo monetario, salario efectivo, universo de empleados, anulaciones y acuse durable; añadir pruebas de reconciliación. | Muy alto | Medio–Alto |
| 2–6 semanas | Turnos configurables/nocturnos, tolerancias, calendario venezolano, horas extra legales, vacaciones y descansos. | Alto | Alto |
| 4–8 semanas | Cierre histórico, auditoría completa, control de acceso por ámbito, retención y derechos sobre datos. | Alto | Alto |
| Antes de producción | Prueba legal especializada, UAT de nómina, hardware y recuperación; análisis fiscal/facturación; evaluación de seguridad independiente. | Muy alto | Medio–Alto |

## Criterio de salida a producción

No se recomienda producción con efectos salariales hasta que:

- no quede ningún hallazgo crítico abierto;
- los hallazgos altos laborales, históricos, de privacidad y acceso tengan corrección o control compensatorio aprobado;
- un conjunto dorado de casos de nómina sea reconciliado de forma independiente;
- se demuestre que cada evento aceptado es persistido exactamente una vez o queda visible para reintento;
- exista restauración probada de base de datos, rostros, fotos y evidencias;
- asesoría laboral, tributaria y de privacidad venezolana confirme el modelo operativo y los textos contractuales.

## Alcance y limitaciones

Se inspeccionaron la documentación pública y de planificación, código fuente, migraciones, configuración de despliegue, pruebas y artefactos comerciales disponibles en el repositorio. También se ejecutaron las suites automatizadas y se contrastaron normas primarias o reproducciones oficiales/autoritativas.

No se realizó prueba con hardware físico, penetración externa, restauración completa en infraestructura de producción, validación contractual con proveedores, revisión contable de una empresa concreta ni dictamen jurídico. Este informe es una auditoría técnica y de lógica de negocio; no sustituye asesoramiento legal o tributario profesional. El detalle, la evidencia reproducible y las fuentes se presentan en la segunda parte de este documento.

---

# Parte II — Informe técnico detallado

## 1. Conclusión técnica

La implementación no corresponde íntegramente con la documentación funcional y comercial. Hay reglas documentadas que no se aplican, reglas implementadas con una semántica distinta, funciones que la interfaz promete pero el backend no soporta y afirmaciones regulatorias basadas en artículos legales equivocados.

El defecto no es solamente de cobertura. Algunos cálculos actualmente comprobados por pruebas son incorrectos: el reporte paga las horas extraordinarias al 250 % del valor base, descuenta dos veces el retraso, permite que un salario implícito se convierta en cero y omite al trabajador que no tenga marcaciones. Es decir, parte de la suite consolida una especificación errónea y no puede usarse como única evidencia de corrección.

Se registran **40 hallazgos**: **10 críticos, 16 altos, 12 medios y 2 bajos**. El producto no debe operar como fuente final de pago hasta corregir los críticos, reconciliar un conjunto dorado de casos laborales y completar validación legal, fiscal y operativa independiente.

## 2. Método, evidencia y alcance

### 2.1 Secuencia aplicada

1. Se inventariaron primero los documentos del repositorio, incluidas variantes Markdown, HTML y PDF, guías de QA, arquitectura, manuales comerciales y material de planificación.
2. Se leyó y sintetizó el contenido funcional, técnico y de negocio no redundante antes de juzgar el código. Se compararon también las variantes generadas para detectar divergencias.
3. Se reconstruyeron actores, permisos, estados, entradas, cálculos, salidas, persistencia, sincronización y licenciamiento desde rutas, servicios, migraciones y pruebas.
4. Cada afirmación material se contrastó entre documento, implementación y prueba. Una prueba aprobada no se consideró correcta si su resultado esperado contradecía la regla documentada o legal.
5. Se investigó legislación venezolana aplicable y se separaron obligaciones legales, obligaciones condicionadas por el modelo comercial y estándares técnicos voluntarios.
6. Se ejecutaron pruebas disponibles y análisis de dependencias. No se alteró la lógica del producto.

### 2.2 Universo documental

El inventario arrojó 236 artefactos documentales y de planificación, muchos de ellos copias, resultados generados o material histórico. Las once fuentes públicas sustantivas son:

- `docs/ARQUITECTURA-HEXAGONAL.md`;
- `docs/GUIA-USUARIO.md`;
- `docs/QA-GUIDE.md`;
- `docs/qa/TEST-CASES.md`;
- `docs/qa/REGRESSION-FULL.md`;
- `docs/qa/SMOKE-SUITE.md`;
- `docs/comercial/PROPUESTA-COMERCIAL.md`, `.html` y `.pdf`;
- `docs/comercial/MANUAL-VENDEDORES.md` y `.html`.

Se revisó además la planificación técnica oculta pertinente, que contiene el estado de fases, decisiones de arquitectura, pruebas propuestas y pendientes de liberación. Las variantes HTML/PDF no se trataron como equivalentes automáticamente: contienen datos comerciales distintos.

### 2.3 Fuera de alcance o no demostrable en este entorno

- prueba física con Hikvision DS-K1T341/DS-K1T342;
- penetración externa o explotación sobre una instalación productiva;
- prueba real de Cloudflare, Turso y contratos de subencargado;
- restauración completa en una máquina limpia;
- activación de licencia entre equipos físicos distintos;
- exactitud contable para la situación de una empresa concreta;
- dictamen jurídico vinculante.

Los planes de las fases 12 y 13 confirman que parte de esas validaciones humanas, de hardware y de infraestructura sigue pendiente. La ausencia de prueba no se calificó automáticamente como defecto, pero sí impide afirmar preparación productiva.

## 3. Arquitectura y flujo de negocio reconstruido

### 3.1 Componentes

```text
Navegador
   │ HTTPS / mismo origen
   ▼
nginx ───────────────► Next.js
   │ /api
   ▼
API Rust ── escritor serializado ──► SQLite local ── sync opcional ──► Turso
   │
   ├──► filesystem: rostros, fotos de eventos y evidencias
   ├──► equipos Hikvision: configuración, usuarios y stream de eventos
   └──► servicio de licencias: activación y JWT ligado a hardware
```

Los puertos internos de API y frontend no se publican directamente al host. nginx sirve de límite de origen. La base local es la fuente primaria; Turso es opcional.

### 3.2 Actores y permisos efectivos

| Rol | Capacidades observadas |
|---|---|
| `viewer` | Leer empleados, eventos y fotos, permisos y evidencias, registros diarios, dispositivos y tenant. |
| `supervisor` | Todo lo anterior; editar empleados; consultar anomalías, auditoría y reportes. |
| `admin` | Configuración completa, usuarios, departamentos, reglas, permisos, anulaciones, enrolamiento y dispositivos. |

No hay segmentación por departamento, autoservicio de trabajador ni autorización por propietario del dato. El rol `viewer` tiene acceso a datos biométricos, laborales y médicos de toda la instalación.

### 3.3 Flujo de extremo a extremo

1. **Inicialización:** el sistema exige una licencia y, si no hay usuarios, expone el alta del primer administrador.
2. **Configuración:** el administrador crea usuarios, departamentos, reglas, dispositivos y empleados. Parte de la configuración de turnos existe en la base, pero no en el contrato/API de departamentos.
3. **Enrolamiento facial:** el navegador captura JPEG y métricas de calidad, la API decodifica/normaliza la imagen, persiste rostro/foto y la envía a dispositivos.
4. **Ingreso de marcaciones:** llegan por stream saliente del dispositivo o por receptor público `push`; se analizan XML/JSON/JPEG, se asocia rostro a empleado, se guarda el evento y se solicita recálculo.
5. **Cálculo diario:** se forma una ventana del turno, se elige primera entrada y última salida, se descuenta almuerzo fijo, se calculan trabajo, retraso, salida temprana, horas extra y anomalías.
6. **Excepciones:** permisos y anulaciones administrativas disparan recálculo o sustituyen algunos campos.
7. **Reporte:** se unen registros diarios, empleados, departamentos, anulaciones activas y permisos; se calcula dinero y se exporta JSON/XLSX. El PDF se genera en el frontend.
8. **Auditoría y operación:** triggers registran varias mutaciones; operaciones de dispositivo mantienen checkpoints; la base puede sincronizarse con Turso.

## 4. Matriz de reglas documentación–código

| Regla o promesa | Fuente documental | Implementación observada | Estado |
|---|---|---|---|
| Tolerancia de 10 min + bono de 5 min; 08:14 produce 0 retraso | `docs/QA-GUIDE.md:396-437` | La tolerancia solo amplía la ventana; retraso se calcula desde la hora nominal en `calc/engine.rs:60-78` y `calc/overnight.rs:92-95`. | No cumple |
| Máximo de 10 h efectivas por día | `docs/QA-GUIDE.md:468-481` | `calc/overtime.rs:15-29` suma trabajo total y extraordinario otra vez; activa el límite antes de 10 h. | Incorrecta |
| Turno diurno/nocturno configurable | `docs/QA-GUIDE.md:482-500` | Columnas en migración 012, pero faltan en DTO y CRUD de departamentos. | Inaccesible |
| Sábados y domingos son descanso | Guía QA | Está codificado de forma fija en `calc/engine.rs:93-98`, sin contratos/calendarios alternativos. | Parcial/ rígida |
| Feriados vigentes | `docs/comercial/PROPUESTA-COMERCIAL.md:56` | No hay catálogo, vigencia ni regla de feriados. | No implementada |
| Permiso cubre jornada completa | `docs/QA-GUIDE.md:501-529` | El motor fuerza todos los minutos a cero para ese día. | Implementada, limitada |
| Salario vacío hereda del departamento | UI de empleados | API guarda `0`; reporte usa solo el salario de empleado. | No cumple |
| Hora extra se paga con recargo de 50 % | QA/comercial y LOTTT | Los minutos extra ya están en pago ordinario y se añade 150 % adicional. | Incorrecta |
| Registro inmutable de quién/qué/cuándo/por qué | `PROPUESTA-COMERCIAL.md:75` | Registro no editable por SQL ordinario, pero muchos triggers dejan `actor_id` nulo y faltan cálculos/eventos. | Parcial |
| Hasta 200 empleados y 4 equipos | `PROPUESTA-COMERCIAL.md:146` | La licencia y los servicios de creación no aplican esos límites. | No implementada |
| Exportación de datos personales | `PROPUESTA-COMERCIAL.md:122-134` | Hay reportes operativos, no una solicitud integral de acceso/portabilidad/corrección/supresión. | No cumple |
| Plantilla de consentimiento biométrico | Propuesta comercial | No existe artefacto, estado o prueba de consentimiento/base jurídica. | No cumple |
| Instalación en 7–8 días | Propuesta Markdown | Manual/HTML indican 3–5 días. | Ambigua |
| Modelos Hikvision 671/672 | Propuesta comercial | La planificación y objetivo técnico usan DS-K1T341/342. | Inconsistente |
| Ancho de banda 1 Mbps | Propuesta Markdown | HTML señala 5 Mbps. | Inconsistente |
| Cumplimiento LOTTT/LOPCYMAT | Propuesta/QA | Se citan artículos equivocados y faltan reglas legales materiales. | No sustentada |

## 5. Registro de hallazgos

Las prioridades combinan probabilidad, impacto y detectabilidad. **Impacto** representa el beneficio/riesgo mitigado; **esfuerzo** es una estimación relativa, no un presupuesto.

### 5.1 Críticos

#### C-01 — Pago de horas extra al 250 %

- **Descripción:** el reporte paga todos los minutos efectivos a tarifa ordinaria y luego suma los minutos extra al 150 %. Una hora extra recibe 100 % dentro del primer componente más 150 % en el segundo.
- **Evidencia:** `backend/src/reports/service.rs:293-329`; `backend/src/reports/money.rs:14-40`. Con jornada de 480 min, 60 min extra y salario diario de 50, el código produce 65,625: 56,25 por 540 min más 9,375 de extra. La composición correcta bajo un recargo de 50 % sería 50 + 9,375 = 59,375. La prueba QA E3 acepta el comportamiento duplicado.
- **Riesgo:** sobrepago masivo, pasivo de recuperación, estados salariales inexactos y disputa laboral/contable.
- **Requisito:** LOTTT, artículo 118, recargo mínimo de 50 % sobre el salario convenido para jornada ordinaria.
- **Recomendación:** separar `ordinary_minutes` y `overtime_minutes`; pagar ordinarios una vez y extraordinarios a 1,5×. Mantener casos dorados con desglose por concepto y reconciliación contable.
- **Prioridad:** impacto muy alto; esfuerzo medio.

#### C-02 — Doble descuento por retraso

- **Descripción:** una llegada tarde ya reduce el tiempo efectivo entre entrada y salida; el reporte vuelve a restar `late_minutes` del total monetizable.
- **Evidencia:** cálculo temporal en `backend/src/calc/engine.rs:60-78`; segundo descuento en `backend/src/reports/service.rs:293-329`.
- **Riesgo:** subpago sistemático y reclamos por deducciones no transparentes.
- **Requisito:** LOTTT, artículo 106, exige recibo con conceptos, horas extra, deducciones y asignaciones; cualquier política de descuento debe ser legal, explícita y no duplicada.
- **Recomendación:** escoger una sola representación: tiempo realmente trabajado o tiempo programado menos incidencias. Versionar la política y mostrar la conciliación completa.
- **Prioridad:** impacto muy alto; esfuerzo bajo–medio.

#### C-03 — Salario heredado termina en cero

- **Descripción:** el frontend comunica que un salario vacío hereda el del departamento; la API persiste cero y el reporte nunca consulta el salario departamental.
- **Evidencia:** `frontend/src/app/(dashboard)/employees/page.tsx:41-59`; `backend/src/employees/service.rs:97-120`; `backend/src/reports/service.rs:174-224`.
- **Riesgo:** pago cero, errores silenciosos y falsa confianza en la interfaz.
- **Requisito:** integridad contractual y exactitud del pago; LOTTT, artículo 106, respecto del detalle salarial.
- **Recomendación:** representar ausencia como `NULL`, resolver salario efectivo por vigencia y procedencia, impedir cero/negativos salvo autorización explícita y advertir en previsualización.
- **Prioridad:** impacto muy alto; esfuerzo medio.

#### C-04 — Anulaciones activas múltiples duplican reportes

- **Descripción:** no existe unicidad para una anulación activa por registro. Una nueva anulación no revoca la anterior y el `LEFT JOIN` del reporte multiplica filas.
- **Evidencia:** esquema e índice no único en `backend/src/db/migrations/009_daily_record_overrides.sql:7-25`; inserción en `backend/src/daily_records/handlers.rs:203-247`; unión en `backend/src/reports/service.rs:193-199`.
- **Riesgo:** duplicidad de días, minutos e importes; fraude o error accidental con efecto financiero.
- **Requisito:** integridad, trazabilidad y recibos correctos; LOTTT, artículos 106 y 183 cuando afecte horas extra.
- **Recomendación:** índice único parcial por `daily_record_id WHERE status='active'`, sustitución transaccional, idempotency key, migración que resuelva duplicados y alerta de reconciliación.
- **Prioridad:** impacto muy alto; esfuerzo medio.

#### C-05 — El empleado totalmente ausente desaparece

- **Descripción:** el acumulador del reporte nace de registros diarios o permisos. Si un empleado activo no tiene ninguno, no entra al conjunto sobre el cual se calculan ausencias.
- **Evidencia:** `backend/src/reports/service.rs:207-258`, `408-470` y `487-501`.
- **Riesgo:** omisión de ausencia, deducción y alerta; resultados incompletos y manipulables eliminando/no ingresando eventos.
- **Requisito:** registro fiable de jornada y recibos exactos; controles de integridad operacional.
- **Recomendación:** comenzar por empleados activos durante el período y expandir el calendario esperado según vigencias, descansos, feriados y permisos; luego unir los eventos.
- **Prioridad:** impacto muy alto; esfuerzo medio–alto.

#### C-06 — Clave privada de licenciamiento comprometida

- **Descripción:** el repositorio incluye la clave privada que corresponde a la clave pública compilada en el verificador. No es una clave de prueba aislada mientras la aplicación de producción confíe en ella.
- **Evidencia:** `do-functions/test-keys/test_priv.pem`, `do-functions/test-keys/test_pub.pem`, `backend/src/license/pubkey.pem` y reconocimiento de alineación/rotación manual en `do-functions/README.md:146-163`. La derivación de la clave pública produjo el mismo SHA-256 (`f0318260…`) que la embebida.
- **Riesgo:** emisión ilimitada de licencias auténticas, evasión comercial y pérdida de la raíz de confianza.
- **Requisito:** gestión segura de claves; ISO/IEC 27001:2022 como referencia voluntaria de control criptográfico.
- **Recomendación:** tratar la clave como comprometida; rotar la confianza en una versión urgente, revocar/fechar claves, firmar solo en KMS/HSM, impedir exportación y añadir escaneo de secretos y provenance de build.
- **Prioridad:** impacto muy alto; esfuerzo medio–alto.

#### C-07 — Activación concurrente de una licencia en dos equipos

- **Descripción:** la función consulta el estado y luego vincula el fingerprint mediante otra operación, sin comparación y actualización atómicas.
- **Evidencia:** `do-functions/packages/licenses/activate/index.js:41-66` y `116-149`.
- **Riesgo:** dos solicitudes simultáneas reciben tokens firmados válidos para huellas diferentes.
- **Requisito:** unicidad e integridad de licencia; obligación contractual de límites comerciales.
- **Recomendación:** `UPDATE ... WHERE bound_fingerprint IS NULL OR bound_fingerprint=?` y exigir una fila afectada dentro de transacción; nonce/idempotencia y prueba de carrera real.
- **Prioridad:** impacto muy alto; esfuerzo bajo–medio.

#### C-08 — Token de escritura de dispositivo expuesto en logs

- **Descripción:** el secreto se incluye en `/devices/{device_id}/push/{token}`. El tracer registra la URI completa y el acceso genérico de nginx registra la ruta.
- **Evidencia:** ruta en `backend/src/main.rs:258-260`; logging en `backend/src/http_trace.rs:13-18`; bloque `/api/` en `deploy/nginx.conf:62-69`.
- **Riesgo:** cualquier operador, agregador o proveedor con acceso al log puede inyectar eventos falsos y alterar asistencia/pagos.
- **Requisito:** confidencialidad de credenciales; Ley Especial contra los Delitos Informáticos, artículos 6, 20 y 22; RFC 6750/RFC 9700 como buenas prácticas para bearer tokens.
- **Recomendación:** aceptar el secreto en cabecera autenticada, rotar todos los tokens, censurar rutas/cabeceras, reducir acceso/retención de logs y añadir firma/replay protection del mensaje.
- **Prioridad:** impacto muy alto; esfuerzo medio.

#### C-09 — Carrera en creación del primer administrador

- **Descripción:** se lee `COUNT(*)` fuera de la transacción escritora y después se encola el alta. Dos nombres distintos pueden observar cero y ambos insertarse como administradores.
- **Evidencia:** `backend/src/setup/handlers.rs:62-105`; ruta pública en `backend/src/main.rs:245-260`. El comentario que afirma impedir la carrera no coincide con el límite transaccional real.
- **Riesgo:** toma de control durante instalación o reinicialización y presencia de administrador no autorizado.
- **Requisito:** control de acceso y alta segura; OWASP ASVS 5.0 como referencia voluntaria.
- **Recomendación:** transacción `BEGIN IMMEDIATE` que compruebe e inserte, o fila singleton/índice que permita un único bootstrap; token de instalación de un solo uso y cierre explícito del endpoint.
- **Prioridad:** impacto muy alto; esfuerzo bajo.

#### C-10 — Acuse exitoso antes de persistencia de la marcación

- **Descripción:** el receptor `push` responde éxito y el procesamiento posterior absorbe fallas de parseo o base. El equipo puede avanzar su cola aunque el evento se pierda.
- **Evidencia:** `backend/src/devices/push.rs:87-100` y `125-150`.
- **Riesgo:** pérdida irrecuperable y silenciosa de entrada/salida con efecto directo en pago, ausencia y evidencia de jornada.
- **Requisito:** integridad del registro de jornada; LOTTT, artículos 106 y 183; buenas prácticas de auditoría.
- **Recomendación:** guardar primero el cuerpo crudo en inbox durable con hash/idempotency key, confirmar solo tras commit, procesar asíncronamente, reintentar y exponer DLQ/alerta.
- **Prioridad:** impacto muy alto; esfuerzo alto.

### 5.2 Altos

#### H-01 — Tolerancia y bono no afectan el retraso

- **Descripción/evidencia:** `calc/overnight.rs:92-95` solo amplía la ventana de captura; `calc/engine.rs:60-78` mide desde la hora nominal. Contradice el ejemplo 08:14 → 0 de `docs/QA-GUIDE.md:430`.
- **Riesgo:** incidencias y descuentos indebidos.
- **Requisito:** regla contractual documentada y recibo transparente (LOTTT 106).
- **Recomendación:** modelar umbral y gracia explícitos, definir qué ocurre en el minuto límite y probar ambos lados. **Impacto alto; esfuerzo bajo.**

#### H-02 — Salida de turno nocturno se atribuye al día equivocado

- **Descripción/evidencia:** `events/service.rs:165-173` solicita recálculo usando la fecha local del evento. Una salida posterior a medianoche recalcula el nuevo día, no el día de inicio. `daily_records/service.rs:110-129` solo cruza medianoche si recibe el ancla correcta.
- **Riesgo:** turno previo queda con salida faltante; el proceso de las 02:00 puede consolidar una anomalía que la salida de las 06:00 no repara.
- **Requisito:** LOTTT 117, 173 y registros exactos.
- **Recomendación:** resolver candidatos de turno por ventana y recalcular ancla actual/anterior de forma idempotente; pruebas 22:00–06:00, DST no aplicable localmente pero sí cambios de zona. **Impacto muy alto; esfuerzo medio.**

#### H-03 — Configuración de turno almacenada pero no administrable

- **Descripción/evidencia:** migración `012_shift_type_to_departments.sql:5-9` agrega `shift_type`, `is_overnight_shift` y `ordinary_daily_minutes`; faltan en `departments/models.rs:4-45` y sus consultas de servicio.
- **Riesgo:** todos operan con valores por defecto; imposibilidad de representar jornadas legales distintas.
- **Requisito:** LOTTT 117 y 173.
- **Recomendación:** API/UI completa, validación legal, vigencia temporal y migración de departamentos existentes. **Impacto alto; esfuerzo medio.**

#### H-04 — No existe calendario venezolano configurable

- **Descripción/evidencia:** descanso sábado/domingo codificado en `calc/engine.rs:93-98`; no se encontró dominio de feriados, descansos alternativos o convenio.
- **Riesgo:** pago y ausencia incorrectos en feriados, trabajo dominical, esquemas 5×2 distintos y actividades continuas.
- **Requisito:** LOTTT 119, 120, 173, 184, 188 y feriados regionales del 187.
- **Recomendación:** calendario versionado nacional/regional/empresa, patrón de descanso por trabajador, compensatorio y precedencia de reglas. **Impacto alto; esfuerzo alto.**

#### H-05 — Horas extra legales incompletas y tope diario mal calculado

- **Descripción/evidencia:** `calc/overtime.rs:15-29` evalúa `work_minutes + overtime_minutes > 600`, aunque el primero ya contiene los extra. El código sí suma y alerta al superar 10 h extra/semana o 100 h/año, con acumulados obtenidos en `daily_records/service.rs:151-196`, pero no impide ni autoriza el exceso y carece de excepción urgente y libro detallado. La prueba `overtime.rs:39-45` codifica la suma diaria errónea.
- **Riesgo:** falsos excesos, excesos reales no detectados y falta de evidencia ante inspección.
- **Requisito:** LOTTT 178, 182 y 183.
- **Recomendación:** corregir el contador diario, preservar/verificar los acumuladores semanales y anuales por vigencia, exigir autorización y causal, producir registro exportable e inmutable, y aplicar la regla de remuneración correspondiente cuando no hubo autorización. **Impacto alto; esfuerzo alto.**

#### H-06 — Vacaciones y bono vacacional sin remuneración completa

- **Descripción/evidencia:** `reports/service.rs:359-375` cuenta días de vacaciones que solo existen como permisos, pero no crea pago; no hay antigüedad, incremento anual ni registro vacacional.
- **Riesgo:** subpago y registro legal incompleto.
- **Requisito:** LOTTT 190, 192 y 203.
- **Recomendación:** motor de derecho por aniversario y antigüedad, días hábiles según calendario, bono, disfrute/fraccionamiento y registro; o declarar explícitamente que el cálculo se realiza externamente. **Impacto alto; esfuerzo alto.**

#### H-07 — Anulaciones aceptan estados inválidos y no recalculan coherentemente

- **Descripción/evidencia:** `daily_records/handlers.rs:103-151` convierte entradas mal formadas a `None`, acepta minutos negativos/excesivos y permite una anulación sin cambio. Solo valida orden cuando ambos timestamps parsean (`:177-187`). El reporte usa `override_work_minutes`, pero ignora entrada/salida y conserva horas extra originales.
- **Riesgo:** manipulación, combinación imposible de campos y pago desacoplado de la evidencia.
- **Requisito:** integridad, trazabilidad y LOTTT 106/183.
- **Recomendación:** DTO tipado y límites, exigir motivo/cambio, recalcular un snapshot completo bajo una política versionada, four-eyes para impacto monetario. **Impacto alto; esfuerzo medio.**

#### H-08 — Unidad salarial ambigua y pago de descanso/feriado incompleto

- **Descripción/evidencia:** la UI solo dice “Sueldo Base (USD)”. `reports/money.rs:120-134` trata 480 min como un salario base completo, equivalente a salario diario, sin explicarlo. `money.rs:62-77` suma 50 % por trabajo en descanso pero no modela separadamente el derecho al día pagado; tampoco existen feriados.
- **Riesgo:** si se ingresa salario mensual, el resultado puede multiplicarse aproximadamente por los días; descanso/feriado puede quedar corto según la composición salarial.
- **Requisito:** LOTTT 104–106, 119 y 120.
- **Recomendación:** tipo de salario (`hourly/daily/monthly`), moneda, divisor, vigencia y conceptos separados: derecho ordinario y trabajo con recargo. Validación con asesor laboral. **Impacto muy alto; esfuerzo alto.**

#### H-09 — Reportes sin vigencia histórica ni cierre de período

- **Descripción/evidencia:** salario, nombre, cargo y reglas se leen en su estado actual; `effective_from` de reglas no selecciona una versión histórica. No hay estado abierto/cerrado, snapshot, aprobación, hash del artefacto ni reapertura controlada.
- **Riesgo:** un cambio presente modifica retrospectivamente un período, y el mismo reporte puede producir otro resultado sin dejar explicación suficiente.
- **Requisito:** LOTTT 106/183; COT 2020 sobre conservación tributaria cuando el artefacto se use como soporte; NIST SP 800-92 como práctica de gestión de logs.
- **Recomendación:** entidades versionadas por intervalo, ejecución de cálculo identificada, cierre/aprobación, snapshot de entradas/reglas/resultados, hash y correcciones por asiento, no reescritura. **Impacto alto; esfuerzo alto.**

#### H-10 — Sin gobierno del ciclo de vida biométrico y médico

- **Descripción/evidencia:** no existen registros de aviso, finalidad, base jurídica/consentimiento, vencimiento, solicitud de acceso/corrección/supresión ni borrado programado. Desactivar un empleado limpia mapeos del equipo, pero conserva rostro, fotos, XML y evidencias indefinidamente.
- **Riesgo:** uso desproporcionado, acceso indebido, incumplimiento de derechos constitucionales y mayor impacto de una filtración.
- **Requisito:** Constitución, artículos 28 y 60; LOPCYMAT 56.10–12; Ley Especial contra los Delitos Informáticos 20–22.
- **Recomendación:** evaluación de impacto, inventario y flujo de datos, finalidad/base jurídica por dato, aviso comprensible, minimización, plazos y borrado verificable, canal de derechos y contratos con proveedores. **Impacto alto; esfuerzo alto.**

#### H-11 — Acceso demasiado amplio a datos sensibles

- **Descripción/evidencia:** rutas agrupadas en `backend/src/main.rs:273-310` permiten a cualquier `viewer` leer todos los empleados, eventos, fotos faciales, permisos y evidencias; no existe alcance departamental. Supervisor puede editar todo empleado.
- **Riesgo:** exposición interna masiva de biometría, salud y conducta laboral.
- **Requisito:** Constitución 28/60; LOPCYMAT 56.10; delitos informáticos 20–22; mínimo privilegio de ISO 27001/OWASP ASVS como referencia.
- **Recomendación:** matriz recurso/acción/ámbito, filtrado obligatorio en consulta, separación de salud/biometría, autoservicio limitado, autorización denegada por defecto y pruebas negativas. **Impacto alto; esfuerzo medio–alto.**

#### H-12 — Autenticación sin defensa suficiente contra abuso y revocación tardía

- **Descripción/evidencia:** no hay rate limit, bloqueo o MFA; mínimo de ocho caracteres. `auth/handlers.rs:33-54` retorna antes de Argon2 para usuario inexistente, permitiendo diferenciación temporal. `auth/middleware.rs:9-27` confía solo en claims: desactivar o bajar rol no revoca el access token durante hasta 20 min.
- **Riesgo:** credential stuffing, enumeración, persistencia temporal de privilegio y compromiso de administrador.
- **Requisito:** control de acceso razonable; OWASP ASVS 5.0 y NIST SP 800-63B como prácticas voluntarias.
- **Recomendación:** throttling por cuenta/IP/dispositivo, hash señuelo, MFA para privilegios, política moderna de contraseña, versión de sesión/usuario y revocación inmediata para acciones sensibles. **Impacto alto; esfuerzo medio.**

#### H-13 — Licencia no hace cumplir expiración, producto ni límites comerciales

- **Descripción/evidencia:** `license/service.rs:47-55` valida con expiración desactivada. No se comprueban `product`, emisor, audiencia, `kid`/revocación. Claims y servicios no limitan 200 empleados/4 equipos ni addons.
- **Riesgo:** uso indefinido o cruzado, límites contractuales inoperantes y revocación imposible sin release.
- **Requisito:** exactitud de oferta y contrato; gestión criptográfica segura.
- **Recomendación:** esquema de claims versionado, validaciones estrictas, claves rotables, gracia offline explícita, revocación/renovación y enforcement transaccional en creación. **Impacto alto; esfuerzo medio–alto.**

#### H-14 — Protección, retención y recuperación de archivos incompletas

- **Descripción/evidencia:** SQLite y archivos biométricos/médicos no están cifrados por la aplicación; solo credenciales de dispositivos usan AES-GCM. El flujo de respaldo/rollback no demuestra incluir y restaurar todos los directorios de fotos, rostros y evidencias. No hay limpieza por retención. Turso/Cloudflare amplían el mapa de terceros sin documentación de gobierno.
- **Riesgo:** exposición por copia de disco, recuperación inconsistente, pérdida probatoria, crecimiento sin límite y transferencias no evaluadas.
- **Requisito:** Constitución 28/60, LOPCYMAT 56.10 y deberes contractuales; ISO 27001/27701 como referencia.
- **Recomendación:** cifrado de volumen/archivo con claves separadas, manifiesto de backup consistente, restauración periódica, retención por clase, borrado criptográfico e inventario/contratos/localización de proveedores. **Impacto alto; esfuerzo alto.**

#### H-15 — Afirmaciones comerciales de cumplimiento no sustentadas

- **Descripción/evidencia:** `PROPUESTA-COMERCIAL.md:9,56,75,104-123` afirma preparación para nómina, feriados actuales, trazabilidad completa y diseño de cumplimiento. La tabla atribuye jornada al artículo 167 y registro extra al 187; corresponden 173 y 183. LOPCYMAT 56.4 se describe como registro de jornada, pero trata notificación de condiciones inseguras.
- **Riesgo:** responsabilidad contractual, expectativa errónea, decisiones de compra engañadas y mayor exposición ante disputa.
- **Requisito:** deber de información y protección al usuario/consumidor cuando aplique; Constitución 117 y Ley Orgánica de Precios Justos; buena fe contractual.
- **Recomendación:** revisión legal de todo material, matriz verificable de claims, disclaimer de pre-nómina y publicación de límites/pendientes. No usar “cumple” hasta certificar operación concreta. **Impacto alto; esfuerzo bajo.**

#### H-16 — Alcance fiscal, de nómina y pagos no definido

- **Descripción/evidencia:** no hay cálculo integral de IVSS, FAOV, INCES, ISLR, prestaciones u otros conceptos, aunque el material sugiere preparación para nómina. El manual acepta Binance/USDT sin proceso de factura, conciliación, tasa, comprobante o tratamiento IGTF documentado.
- **Riesgo:** clientes interpretan una pre-nómina como nómina final; facturación o tratamiento tributario incorrectos; conciliación difícil.
- **Requisito:** Providencia SENIAT SNAT/2024/000102 para facturación digital en los supuestos aplicables; COT 2020; Ley IGTF y obligaciones parafiscales según el empleador. LOTTT 123 exige salario en moneda de curso legal y no debe confundirse con el medio de pago de una licencia B2B.
- **Recomendación:** definir producto como pre-nómina o implementar un motor regulatorio mantenido; integrar/exportar hacia nómina certificada. Para la venta, procedimiento fiscal por tipo de cliente/canal y criterio escrito de contador venezolano sobre divisas/cripto/IGTF. **Impacto alto; esfuerzo alto.**

### 5.3 Medios

#### M-01 — Deduplicación insuficiente entre dispositivos y para desconocidos

- **Descripción/evidencia:** la clave de `004_attendance_events.sql:2-23` incluye dispositivo y empleado. La misma marca en dos equipos persiste dos veces; eventos desconocidos con `employee_id NULL` tampoco se agrupan útilmente. Una prueba acepta expresamente el caso entre dispositivos.
- **Riesgo:** duplicación de evidencia, agregación errónea y fraude por reenvío.
- **Requisito:** integridad de datos.
- **Recomendación:** identificador nativo del evento o hash canónico, ventana configurable y ledger de duplicados sin borrar evidencia. **Impacto medio–alto; esfuerzo medio.**

#### M-02 — Evento desconocido contamina a todos los empleados

- **Descripción/evidencia:** `daily_records/service.rs:125-129` incluye todos los eventos desconocidos dentro de la ventana de cada empleado recalculado.
- **Riesgo:** una sola cara no asociada marca anomalía en numerosos trabajadores.
- **Requisito:** exactitud y minimización.
- **Recomendación:** cola separada de eventos no asociados, resolución manual auditada y vínculo posterior único; nunca propagarlos por ventana. **Impacto medio; esfuerzo bajo.**

#### M-03 — Primera entrada/última salida y almuerzo fijo inflan o reducen jornada

- **Descripción/evidencia:** `calc/aggregation.rs:61-99` elige extremos y el primer par de almuerzo; el motor descuenta almuerzo nominal incluso si no existe o la jornada fue corta.
- **Riesgo:** pausas largas se cuentan como trabajo, múltiples bloques se pierden y jornadas cortas pueden quedar infravaloradas.
- **Requisito:** LOTTT 173 y registros exactos.
- **Recomendación:** máquina de estados entrada/salida/pausa, emparejamiento determinista, política para marcas impares y evidencia de cada intervalo. **Impacto medio–alto; esfuerzo alto.**

#### M-04 — Permisos solo de día completo y filtros incoherentes

- **Descripción/evidencia:** `calc/engine.rs:15-37` pone todos los minutos en cero ante permiso; no hay horas parciales. El filtro de `shift_type` se aplica a registros (`reports/service.rs:159-165`) pero no a la consulta secundaria de permisos (`:376-406`). Los contadores mezclan días calendario y laborales.
- **Riesgo:** cobertura excesiva, reportes por turno contaminados y saldo de vacaciones incorrecto.
- **Requisito:** LOTTT 190/203 y política interna.
- **Recomendación:** intervalos parciales, unidad explícita, calendario hábil y aplicación uniforme de filtros. **Impacto medio; esfuerzo medio–alto.**

#### M-05 — Lecturas de reporte sin snapshot consistente

- **Descripción/evidencia:** el servicio ejecuta consultas separadas de registros, permisos y otros datos sin una transacción de lectura que fije versión.
- **Riesgo:** una modificación concurrente produce un reporte internamente contradictorio aun antes del cierre.
- **Requisito:** integridad y reproducibilidad.
- **Recomendación:** transacción de lectura/snapshot, identificador de corrida y verificación de versión al publicar. **Impacto medio; esfuerzo medio.**

#### M-06 — Auditoría inmutable pero incompleta y sin evidencia antimanipulación fuerte

- **Descripción/evidencia:** `backend/src/db/migrations/020_audit_immutability.sql:154-164` impide `UPDATE/DELETE`, control positivo. Sin embargo, los triggers de `017_phase7_audit_triggers.sql` y `018_employees_base_salary.sql` escriben con frecuencia el actor como nulo; al recrear los de empleados no se preservan todos los campos añadidos después. Cambios derivados en `daily_records`, eventos y comandos no tienen cobertura completa. No hay encadenamiento/hash/anchor externo; un administrador de DB puede retirar triggers.
- **Riesgo:** no se puede responder de forma fiable quién cambió qué ni demostrar ausencia de alteración privilegiada.
- **Requisito:** LOTTT 106/183 cuando soporte pago/extra; NIST SP 800-92 como guía voluntaria.
- **Recomendación:** contexto de actor obligatorio, auditoría semántica en servicio y DB, cobertura de campos, hash encadenado con checkpoint externo, monitoreo de esquema y exportación WORM. **Impacto medio–alto; esfuerzo alto.**

#### M-07 — Evidencia de permisos confía en `Content-Type`

- **Descripción/evidencia:** `leaves/handlers.rs:103-129` acepta el tipo declarado; la carga de anulaciones sí valida magic bytes en `daily_records/handlers.rs:44-58,120-151`.
- **Riesgo:** almacenamiento y posterior entrega de HTML/script u otro contenido disfrazado; malware y fuga mediante render inseguro.
- **Requisito:** seguridad razonable de datos personales/médicos.
- **Recomendación:** detectar tipo real, decodificar/re-encode de imágenes, servir como adjunto desde origen aislado con `nosniff`, antivirus y límites consistentes. **Impacto medio; esfuerzo bajo–medio.**

#### M-08 — Calidad facial se confía al navegador y no hay PAD/liveness

- **Descripción/evidencia:** `enrollments/models.rs:99-159` acepta `face_detected`, luminancia y dimensiones del cliente; el backend no ejecuta segundo detector. Solo normaliza JPEG.
- **Riesgo:** cliente modificado enrola imagen deficiente o fabricada; mayor suplantación y falsos positivos.
- **Requisito:** seguridad proporcional de biometría; ISO/IEC 30107-3:2023 y NIST SP 800-63B son referencias voluntarias, no mandato general venezolano.
- **Recomendación:** validación server-side o dispositivo confiable, detección de presentación acorde al riesgo, revisión humana y métricas de desempeño por modelo. **Impacto medio–alto; esfuerzo alto.**

#### M-09 — Bearer en query SSE y cabeceras web defensivas ausentes

- **Descripción/evidencia:** SSE acepta bearer en query (`main.rs:252-254`). Aunque el trace y el bloque exacto reducen algunos logs, historial/proxies pueden conservarlo. `deploy/nginx.conf:30-80` no define HSTS, CSP, `nosniff`, frame/referrer/permissions policy ni `Cache-Control: no-store` para PII.
- **Riesgo:** fuga de sesión, clickjacking, MIME confusion y caché de información sensible.
- **Requisito:** RFC 6750/9700 y OWASP ASVS como referencias.
- **Recomendación:** cookie HttpOnly same-site o ticket SSE de un uso; paquete de cabeceras probado, no-store para auth/PII/exportaciones y CSP compatible con frontend. **Impacto medio; esfuerzo bajo–medio.**

#### M-10 — Riesgo de cadena de suministro y build no reproducible

- **Descripción/evidencia:** `npm audit` del frontend reportó 20 avisos (12 altos, 6 medios y 2 bajos), incluidos `axios`, `next` y `xlsx`; este último sin corrección automática disponible. `do-functions` carece de lockfile, por lo que `npm ci` falla. CI no demuestra SCA, SBOM, firma de imagen o provenance.
- **Riesgo:** vulnerabilidades transitivas/directas y builds diferentes entre ambientes. Un aviso de paquete requiere triage; no prueba por sí mismo explotación en esta aplicación.
- **Requisito:** gestión de vulnerabilidades; ISO 27001/OWASP como referencias.
- **Recomendación:** triage por rutas alcanzables, actualizar/reemplazar `xlsx`, lockfile obligatorio, Dependabot/Renovate, SCA bloqueante por política, SBOM y firma. **Impacto medio–alto; esfuerzo medio.**

#### M-11 — Configuración de equipos es best-effort y se admite transporte inseguro

- **Descripción/evidencia:** fallas al configurar dirección/eventos se registran como advertencia y la ingesta puede continuar; no queda un control persistente de conformidad. La configuración permite HTTP/TLS no verificado según entorno.
- **Riesgo:** entradas/salidas con sentido incorrecto, pérdida silenciosa o interceptación de credenciales/eventos; contradice “cifrado en tránsito” sin calificación.
- **Requisito:** exactitud de jornada, confidencialidad y promesa contractual.
- **Recomendación:** estado deseado/observado, health y bloqueo o alerta de no conformidad, HTTPS validado, pinning/CA administrada cuando el equipo lo permita y red aislada. **Impacto medio–alto; esfuerzo medio.**

#### M-12 — Restricciones de dominio y observabilidad insuficientes

- **Descripción/evidencia:** la base permite salarios negativos, minutos ordinarios fuera de rango y otros estados inválidos; migración 014 modifica `sqlite_master` mediante `writable_schema`. `/health` solo ejecuta `SELECT 1` y no informa cola de recálculo, ingesta, sync, disco, licencia o equipos.
- **Riesgo:** corrupción lógica, migraciones frágiles y “salud verde” con procesamiento detenido.
- **Requisito:** integridad y continuidad operativa.
- **Recomendación:** `CHECK`, claves e índices de dominio; migraciones recreando tablas de modo soportado; endpoints separados de liveness/readiness y métricas/alertas de negocio. **Impacto medio; esfuerzo medio.**

### 5.4 Bajos

#### L-01 — Estado público e información operativa minimizable

- **Descripción/evidencia:** los endpoints públicos de estado/setup/licencia exponen información necesaria para onboarding, pero facilitan reconocer una instancia no inicializada. La salud no diferencia degradación.
- **Riesgo:** enumeración y selección de ventana de ataque.
- **Requisito:** reducción de superficie, OWASP como referencia.
- **Recomendación:** respuesta mínima, rate limit, token de instalación y cierre irreversible del bootstrap. **Impacto bajo–medio; esfuerzo bajo.**

#### L-02 — Deriva entre artefactos documentales

- **Descripción/evidencia:** propuesta Markdown indica instalación 7–8 días y 1 Mbps; manual/HTML indican 3–5 días y 5 Mbps. Modelos 671/672 no coinciden con 341/342 planificados. Límites de evidencia aparecen como 5 MB en una guía y 10 MB en backend.
- **Riesgo:** cotizaciones, soporte y QA aplican expectativas diferentes.
- **Requisito:** exactitud de información comercial/contractual.
- **Recomendación:** fuente única versionada, generación automática de HTML/PDF, pruebas de claims y propietario de cada requisito. **Impacto medio; esfuerzo bajo.**

## 6. Matriz legal y regulatoria

Esta matriz distingue reglas directamente relacionadas con el producto de obligaciones que dependen de que Cronometrix sea operador, empleador, proveedor facturante o simple licenciante. La aplicabilidad final requiere abogado y contador venezolanos con el contrato y flujo real.

| Fuente | Requisito relevante | Evaluación |
|---|---|---|
| LOTTT art. 106 | Recibo detallado con remuneraciones, horas extra, trabajo nocturno, deducciones y asignaciones. | Parcial/no fiable: el cálculo monetario es incorrecto y no hay cierre histórico. |
| LOTTT art. 117 | Recargo mínimo de 30 % por jornada nocturna. | La fórmula existe, pero depende de un tipo de turno que el CRUD no permite administrar y requiere validación de punta a punta. |
| LOTTT art. 118 | Hora extra con recargo mínimo de 50 %. | Implementada con doble base, resulta 250 %. |
| LOTTT arts. 119–120 | Descanso/feriado pagado; si se trabaja, derecho al día más trabajo con 50 % de recargo. | Incompleto; sin feriados y composición salarial ambigua. |
| LOTTT art. 123 | Salario en moneda de curso legal; cheque/banco por acuerdo; no mercancías/vales. | Debe gobernar el pago laboral; el USDT comercial B2B es un asunto distinto. |
| LOTTT art. 173 | Cinco días y dos descansos; límites 8/40 diurno, 7/35 nocturno, 7,5/37,5 mixto. | Solo configuración parcial e inaccesible; descanso fijo. |
| LOTTT arts. 178, 182–183 | 10 h efectivas/día, 10 extra/semana, 100/año; autorización/excepción y registro detallado. | El diario es defectuoso; semana/año generan alertas pero no control efectivo; sin autorización/registro legal completo. |
| LOTTT arts. 184, 187–188 | Feriados nacionales/regionales y descanso compensatorio. | No implementado. |
| LOTTT arts. 190, 192, 203 | Vacaciones por antigüedad, bono y registro. | Conteo parcial, sin pago/derecho completo. |
| Constitución arts. 28 y 60 | Acceso, finalidad/uso, corrección/destrucción; privacidad, imagen, confidencialidad. | Sin proceso de derechos, finalidad o retención demostrable. |
| LOPCYMAT art. 56.10–12 | Privacidad/acceso a datos personales y registros de enfermedad/accidente/seguridad. | Acceso interno excesivo y gobierno insuficiente. El art. 56.4 fue citado erróneamente por la documentación. |
| Ley Especial contra los Delitos Informáticos arts. 6, 20–22 | Acceso excedido y uso/divulgación indebidos de datos/comunicaciones. | Controles útiles, pero tokens en logs y RBAC amplio aumentan exposición. |
| SENIAT SNAT/2024/000102 | Factura digital y requisitos para supuestos, en especial comercio electrónico. | No es función del producto; falta procedimiento comercial demostrado para la venta. |
| Código Orgánico Tributario 2020 | Conservación/prescripción y deberes/documentos tributarios. | Debe definirse si exportes o facturas forman soporte; no hay archivo histórico cerrado. |
| Ley IGTF | Gravamen según sujeto, moneda/cripto y canal de pago. | La aceptación de USDT requiere análisis caso por caso; no hay proceso documentado. |
| Ley de Mensajes de Datos y Firmas Electrónicas | Efecto jurídico de mensajes y firmas bajo sus condiciones. | Un log interno no equivale automáticamente a firma electrónica cualificada. |
| Constitución art. 117 / Ley Orgánica de Precios Justos | Información, calidad y protección del usuario/consumidor cuando corresponda. | Claims de cumplimiento y alcance pueden inducir a error; aplicabilidad B2B depende de la relación concreta. |

### 6.1 Fuentes legales consultadas

- [LOTTT, Gaceta Oficial Extraordinaria 6.076, reproducción ILO/NATLEX](https://natlex.ilo.org/dyn/natlex2/natlex2/files/download/90040/VEN90040.pdf).
- [LOPCYMAT, Gaceta Oficial 38.236, reproducción ILO/NATLEX](https://natlex.ilo.org/dyn/natlex2/natlex2/files/download/106161/Ley%20Org%C3%A1nica%20de%20Prevenci%C3%B3n%2C%20Condiciones%20y%20Medio%20.pdf).
- [Constitución de la República Bolivariana de Venezuela, reproducción OEA](https://www.oas.org/ext/Portals/33/Files/MLA/Ven_extra_leg_esp_3.pdf).
- [Ley Especial contra los Delitos Informáticos, ficha WIPO](https://www.wipo.int/wipolex/es/legislation/details/10223) y [texto reproducido por OEA](https://iin.oea.org/badaj/wp-content/uploads/2014/07/Ley_Especial_contra_Delitos_Inform%C3%A1ticos.pdf).
- [Providencia SNAT/2024/000102, reproducción de Gaceta Oficial 43.032](https://www.adaptaproerp.com/wp-content/descargas/pdf/PA102.pdf).
- [Providencia SNAT/2011/00071, reproducción de Gaceta Oficial 39.795](https://medisoftware.com.ve/Download/Normas/Providencia0071.pdf).
- [Código Orgánico Tributario 2020, Gaceta Oficial Extraordinaria 6.507](https://accesoalajusticia.org/wp-content/uploads/2020/01/Gaceta-Oficial-n.%C2%BA-6.507-Extraordinario-del-29-de-enero-de-2020.pdf).
- [Reforma de la Ley IGTF, Gaceta Oficial Extraordinaria 6.687, reproducción KPMG](https://assets.kpmg.com/content/dam/kpmg/ve/pdf/2022/03/gaceta-oficial-extraordinario-6687.pdf).
- [Ley sobre Mensajes de Datos y Firmas Electrónicas, Gaceta Oficial 37.148](https://tugacetaoficial.com/leyes/ley-de-mensajes-de-datos-y-firmas-electronicas-gaceta-37148-2001-texto/).
- [Ley Orgánica de Precios Justos, Gaceta Oficial 40.787](https://tugacetaoficial.com/leyes/ley-organica-de-precios-justos-gaceta-40787-2015-texto/).
- [Ley del Seguro Social, reproducción institucional](https://www.incret.gob.ve/public/documentos/LEY_DEL_SEGURO_SOCIAL.pdf).
- [Reforma FAOV/BANAVIH, Gaceta Oficial Extraordinaria 6.805](https://camaradecomerciobolivar.org/wp-content/uploads/2024/05/Reforma-parcial-del-BANAVIH-01-05-2024.pdf).

### 6.2 Protección de datos: precisión importante

Venezuela no dispone actualmente de una ley general y única de protección de datos equivalente al RGPD; esto no significa ausencia de obligaciones. La Constitución, LOPCYMAT, leyes sectoriales, delitos informáticos, contrato y jurisprudencia siguen siendo relevantes. Esta caracterización se contrastó con la [guía vigente de DLA Piper para Venezuela](https://www.dlapiperdataprotection.com/?c=VE&t=security), usada como fuente secundaria de contexto. Cualquier uso de consentimiento debe analizar su validez en la relación laboral y no emplearse como sustituto automático de necesidad, proporcionalidad, seguridad y derechos.

### 6.3 Estándares y buenas prácticas no obligatorios por sí solos

- [ISO/IEC 27001:2022](https://www.iso.org/standard/27001), seguridad de la información.
- [ISO/IEC 27701:2025](https://www.iso.org/standard/27701), gestión de privacidad.
- [ISO/IEC 30107-3:2023](https://www.iso.org/standard/79520.html), pruebas de detección de ataques de presentación biométrica.
- [NIST SP 800-63B](https://pages.nist.gov/800-63-4/sp800-63b.html), autenticación y PAD en el alcance de identidad digital NIST.
- [OWASP ASVS 5.0](https://owasp.org/www-project-application-security-verification-standard/), verificación de seguridad de aplicaciones.
- [NIST SP 800-92](https://csrc.nist.gov/pubs/sp/800/92/final), gestión de logs.
- [RFC 6750](https://www.rfc-editor.org/info/rfc6750/) y [RFC 9700](https://www.rfc-editor.org/info/rfc9700/), uso seguro de bearer tokens/OAuth.

No se atribuye a estas referencias fuerza de ley venezolana. Pueden volverse exigibles por contrato, política corporativa, certificación o regulación sectorial.

## 7. Casos límite, carreras y fraude que deben entrar en aceptación

| Escenario | Resultado seguro esperado |
|---|---|
| Dos solicitudes simultáneas de primer admin | Solo una transacción gana; la otra recibe estado cerrado sin crear usuario. |
| Dos activaciones con fingerprints distintos | Solo una vinculación atómica obtiene licencia. |
| Reenvío idéntico por uno o varios dispositivos | Un evento canónico; duplicados conservados aparte para auditoría. |
| `push` con DB ocupada/caída o XML inválido | No confirmar hasta persistir, o confirmar inbox durable; DLQ visible. |
| Turno 22:00–06:00 y salida tras ejecución nocturna | El día de inicio se recalcula y cierra exactamente una vez. |
| 08:14 con tolerancia 10 + bono 5 | Cero retraso; 08:16 aplica la política documentada. |
| 10 h exactas y un minuto adicional | Exactamente 10 h no excede; el minuto 601 sí. |
| 10 h extra semanales / 100 anuales | Umbral y período se aplican por vigencia y autorización. |
| Dos anulaciones concurrentes | Una activa; conflicto/idempotencia, sin duplicar reporte. |
| Empleado sin ninguna marca | Aparece como ausencia salvo descanso, feriado, permiso o vigencia. |
| Salario omitido/cero/negativo | Herencia inequívoca o error; nunca silencio. |
| Cambio de salario/regla tras cierre | Período cerrado conserva resultado; ajuste es nuevo asiento. |
| Permiso parcial cruza almuerzo o medianoche | Solo cubre intersección válida y calendario correspondiente. |
| Foto con `Content-Type: image/jpeg` pero HTML | Rechazo antes de almacenar/servir. |
| Usuario desactivado con access token vigente | Revocación inmediata en operaciones protegidas. |
| Disco lleno por fotos/XML | Backpressure, alerta, no acuse falso y política de retención. |
| Restore SQLite sin archivos o viceversa | Manifiesto detecta inconsistencia y aborta/recupera de forma controlada. |

## 8. Verificación ejecutada

| Verificación | Resultado | Observación |
|---|---|---|
| `cargo test --all-features` | Aprobada | 117 pruebas unitarias de librería más integraciones; existen stubs/escenarios ignorados, incluido Turso live. |
| Vitest con Node 24.15.0 fijado por el proyecto | Aprobada | 63 archivos, 472 pruebas. Con Node 22 del sistema hubo cuatro fallas ambientales; no se imputaron como defecto de negocio. |
| Pruebas de `do-functions` | Aprobada | 17 pruebas tras instalar dependencias sin lockfile. |
| `npm ci` en `do-functions` | Fallida | No existe `package-lock.json`; el build no es reproducible mediante `npm ci`. |
| `npm audit` frontend | 20 avisos | 12 altos, 6 medios, 2 bajos; requieren triage de alcanzabilidad. |
| Correspondencia de clave de licencia | Confirmada | La pública derivada de `test_priv.pem` coincide con la pública confiada por backend. |
| Inspección estática documental/código/migraciones | Completada | Originó la matriz y los 40 hallazgos. |

Que las suites aprueben no valida por sí mismo las reglas. Dos ejemplos concretos —tope diario de horas extra y pago extraordinario— tienen expectativas de prueba alineadas con la implementación equivocada.

## 9. Controles correctos que deben preservarse

- Argon2id para contraseña y refresh tokens con rotación/detección de replay.
- Credenciales Hikvision cifradas con AES-GCM, `Debug` censurado y DTO público sin token `push`.
- Escritor de base serializado y comprobación transaccional de solapamiento de permisos.
- Triggers que bloquean `UPDATE` y `DELETE` ordinarios sobre `audit_log`.
- Escritura de archivos con validaciones de path, ownership y symlink.
- Checkpoints de operaciones de equipo y recuperación parcial.
- Comparación constante del token de dispositivo.
- Versiones Node/Rust fijadas y cobertura automatizada amplia.
- API/frontend internos no expuestos al host; permisos restrictivos de datos/env.

Cada remediación debe incluir una prueba para no degradar estos controles.

## 10. Plan de remediación

### Fase 0 — Contención (0–72 h)

1. Rotar el par de firma, retirar la clave confiada actual y suspender activaciones hasta desplegar vinculación atómica.
2. Rotar tokens `push`, retirar secretos de URI/logs y restringir el endpoint mientras se implementa inbox durable.
3. Deshabilitar bootstrap después de una transición atómica y verificar instalaciones existentes con más de un admin inicial.
4. Retirar de propuestas/web la afirmación de nómina/cumplimiento/feriados/trazabilidad completa y aclarar pre-nómina.
5. Impedir exportación monetaria automática o marcarla “no conciliada” hasta corregir C-01–C-05.

### Fase 1 — Exactitud financiera e integridad (1–2 semanas)

1. Rehacer el modelo de minutos ordinarios/extra/retraso y unidad salarial.
2. Generar reportes desde empleados × calendario, con salario efectivo y una anulación activa.
3. Validar anulaciones y recalcular todos sus campos de forma coherente.
4. Persistir el ingreso de eventos antes del ACK; implementar idempotencia, DLQ y métricas.
5. Crear corpus dorado revisado por nómina/abogado y pruebas metamórficas: añadir una hora ordinaria no puede reducir pago; duplicar entrada no puede duplicar jornada, etc.
6. Recalcular períodos históricos afectados y producir un informe de diferencias, sin sobrescribir evidencia original.

### Fase 2 — Modelo laboral venezolano (2–6 semanas)

1. Configuración efectiva por fecha de turno diurno/nocturno/mixto, patrón semanal y descansos.
2. Calendario nacional, regional y empresarial; trabajo en descanso y descanso compensatorio.
3. Activación y validación del recargo nocturno; corrección/control de topes diario/semanal/anual de horas extra, autorización y registro del artículo 183.
4. Vacaciones, bono, antigüedad y registro; permisos parciales.
5. Política explícita de pausas y máquina de estados de marcaciones.

### Fase 3 — Cierre, privacidad y control (4–8 semanas)

1. Cierre de período con snapshot, reglas/versiones, aprobación doble, hash, reapertura y ajuste.
2. Auditoría con actor obligatorio, cobertura de resultados/eventos y anclaje externo.
3. RBAC por ámbito, MFA administrativa y revocación inmediata.
4. Programa de privacidad: aviso, base jurídica, minimización, retención, derechos, borrado y terceros.
5. Backup cifrado y consistente de DB + archivos; restauración automatizada y probada.

### Fase 4 — Gate productivo

1. UAT legal/contable independiente con corpus dorado.
2. Pruebas Hikvision reales: red intermitente, reenvío, cola, reloj, cambio de dirección y volumen.
3. Instalación/restauración en máquina limpia y prueba Cloudflare/Turso/licencia entre equipos.
4. Pentest, threat model de fraude interno y biometría, triage completo de dependencias.
5. Aprobación formal de negocio, legal, privacidad, seguridad y operación.

## 11. Priorización impacto–esfuerzo

| Grupo | IDs | Impacto | Esfuerzo típico | Orden |
|---|---|---:|---:|---:|
| Contención de secretos/carreras simples | C-06, C-07, C-08, C-09, H-15 | Muy alto | Bajo–Medio | 1 |
| Corrección monetaria | C-01–C-05, H-01, H-08 | Muy alto | Medio–Alto | 2 |
| Ingesta y turno nocturno | C-10, H-02, M-01–M-03 | Muy alto | Medio–Alto | 3 |
| Modelo laboral | H-03–H-07, H-16, M-04 | Alto | Alto | 4 |
| Histórico/auditoría | H-09, M-05, M-06 | Alto | Alto | 5 |
| Privacidad/acceso/recuperación | H-10–H-14, M-07–M-09 | Alto | Medio–Alto | 6 |
| Operación y mantenimiento | M-10–M-12, L-01–L-02 | Medio | Bajo–Medio | 7 |

## 12. Criterios verificables de cierre

Un hallazgo solo debe cerrarse si existen simultáneamente:

1. requisito de negocio corregido y aprobado por su propietario;
2. implementación y migración seguras para datos existentes;
3. prueba positiva, negativa, de límite y, donde aplique, concurrente;
4. evidencia de reconciliación sobre períodos de muestra;
5. documentación/comercial actualizada desde fuente única;
6. revisión independiente para cambios críticos;
7. observabilidad que detecte una regresión en producción.

La salida productiva con efecto salarial exige cero críticos abiertos y aceptación explícita, con control compensatorio documentado, de cualquier alto restante. La exactitud debe demostrarse sobre resultados, no inferirse de cobertura de código.

## 13. Nota de responsabilidad

Este documento expresa una evaluación técnica y de lógica de negocio basada en el repositorio y las fuentes citadas a la fecha de corte. No sustituye una opinión legal, fiscal, laboral, contable, de protección de datos ni una certificación de seguridad. Las leyes pueden cambiar y la aplicabilidad depende del contrato, sector, tamaño del empleador, ubicación, convenios colectivos, condición tributaria y operación efectiva. Antes de comercializar o usar el sistema para pagar salarios se requiere validación profesional venezolana y prueba del despliegue real.
