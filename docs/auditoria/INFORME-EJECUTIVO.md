# Auditoría integral de lógica de negocio — Informe ejecutivo

**Proyecto:** Cronometrix

**Versión auditada:** commit `9b36341`

**Fecha de corte:** 2 de agosto de 2026

**Ámbito principal:** control de asistencia biométrico y generación de pre-nómina para Venezuela

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

No se realizó prueba con hardware físico, penetración externa, restauración completa en infraestructura de producción, validación contractual con proveedores, revisión contable de una empresa concreta ni dictamen jurídico. Este informe es una auditoría técnica y de lógica de negocio; no sustituye asesoramiento legal o tributario profesional. El detalle, la evidencia reproducible y las fuentes están en el [informe técnico](INFORME-TECNICO.md).
