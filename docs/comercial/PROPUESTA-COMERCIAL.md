# Propuesta Comercial — Cronometrix

**Sistema biométrico de control de tiempo y asistencia**

---

## 1. Resumen ejecutivo

Cronometrix es un sistema de control de tiempo y asistencia que convierte los eventos biométricos de sus dispositivos Hikvision en datos listos para nómina, con trazabilidad legal completa y sin cálculos manuales.

Se instala **on-premise en su sede** (los datos viven en su servidor), con sincronización cifrada a la nube para acceso remoto y respaldo. Cada instalación es independiente y queda bajo el control directo del cliente.

| Concepto | Detalle |
|---|---|
| **Inversión total** | **USD 1.500** (instalación, configuración, capacitación remota 8h, soporte año 1) |
| **Forma de pago** | 50% al inicio, 50% al go-live, en efectivo (USD) |
| **Renovación año 2** | USD 300–375 anuales (soporte + actualizaciones) |
| **Garantía** | 30 días post go-live para corrección de bugs críticos sin costo |
| **Capacidad** | Hasta 200 empleados, 4 dispositivos Hikvision, 1 sede |

---

## 2. ¿Qué es Cronometrix?

Cronometrix es una plataforma web instalada en su servidor que:

1. **Recibe en tiempo real** los eventos de marcaje de sus lectores faciales Hikvision (entrada, salida, almuerzo).
2. **Calcula automáticamente** la jornada trabajada, horas extras, llegadas tarde y ausencias aplicando las tolerancias y reglas que usted defina.
3. **Genera reportes pre-nómina** exportables a Excel y PDF, listos para entregar a contabilidad.
4. **Mantiene un registro auditable** de cada modificación manual sobre las marcaciones, con justificación obligatoria del supervisor.
5. **Sincroniza con la nube** (cifrada) para acceso remoto vía `{su-empresa}.cronometrix.com` desde cualquier lugar.

### Arquitectura local-first

- Su servidor es la fuente de verdad: si Internet cae, el sistema sigue capturando marcaciones.
- Cuando vuelve la conexión, los datos se sincronizan automáticamente.
- Cada cliente tiene su propia instalación aislada — sus datos no se mezclan con los de nadie más.

---

## 3. Funcionalidades incluidas (versión actual)

### Captura y procesamiento

- ✅ Conexión simultánea con hasta 4 lectores Hikvision en la sede
- ✅ Recepción de eventos en tiempo real vía protocolo ISAPI
- ✅ Captura de foto del rostro en cada marcaje (evidencia)
- ✅ Detección automática de duplicados y marcajes inválidos

### Cálculo de jornadas

- ✅ Configuración de turnos (diurno, nocturno, mixto)
- ✅ Tolerancia configurable de entrada/salida (ej. 5 minutos sin penalidad)
- ✅ Descuento automático de tiempo de almuerzo
- ✅ Cálculo de horas extras diurnas y nocturnas
- ✅ Manejo de días feriados nacionales y particulares (calendario editable)

### Gestión de personal

- ✅ Alta/baja/edición de empleados
- ✅ Asignación de turnos por empleado o grupo
- ✅ Enrolamiento de rostro desde la plataforma (sin tocar el dispositivo)
- ✅ Bloqueo automático de empleados desactivados

### Reportes y exportación

- ✅ Reporte diario, semanal, quincenal, mensual
- ✅ Reporte pre-nómina con totales por empleado
- ✅ Exportación a Excel (.xlsx) y PDF
- ✅ Filtros por departamento, sede, rango de fechas

### Seguridad y auditoría

- ✅ Roles: Administrador, Supervisor, Visualizador
- ✅ Bitácora inmutable de cambios (quién, qué, cuándo, por qué)
- ✅ Justificación obligatoria al modificar una marcación
- ✅ Acceso remoto cifrado (HTTPS vía túnel Cloudflare)
- ✅ Hash de contraseñas con Argon2id (estándar OWASP)

### Acceso remoto

- ✅ URL exclusiva del cliente: `{su-empresa}.cronometrix.com`
- ✅ Compatible con cualquier navegador moderno
- ✅ Diseño responsive (escritorio, tablet, móvil)

---

## 4. Roadmap (incluido sin costo en año 1 si liberamos)

Funcionalidades en desarrollo activo que se entregarán como actualizaciones durante los primeros 6 meses:

- 🚧 Notificaciones automáticas por correo/WhatsApp (ausencias, retardos)
- 🚧 Aplicación móvil para supervisores (consulta y aprobación de novedades)
- 🚧 Dashboard ejecutivo con KPIs (ausentismo, puntualidad, horas extras)
- 🚧 Solicitudes de permisos y vacaciones desde la plataforma
- 🚧 Exportación directa a sistemas de nómina locales (formato configurable)

> Las actualizaciones publicadas durante los primeros 6 meses se incluyen automáticamente. A partir del mes 7, las nuevas versiones requieren contrato de soporte vigente.

---

## 5. Cumplimiento legal — Venezuela

Cronometrix está diseñado para cumplir con la normativa laboral venezolana vigente. A continuación el marco legal que el sistema soporta:

### 5.1 Ley Orgánica del Trabajo, los Trabajadores y las Trabajadoras (LOTTT, 2012)

| Artículo | Materia | Cómo lo cumple Cronometrix |
|---|---|---|
| **Art. 167** | Jornada diurna máx. 8h, nocturna máx. 7h, mixta máx. 7h 30min | Configuración de turnos por tipo, alertas al exceder el tope legal |
| **Art. 173** | Jornada semanal máx. 40h (diurna) / 35h (nocturna) | Reporte semanal con totales y advertencia al superar el límite |
| **Art. 175** | Descanso intrajornada (1 hora cuando la jornada exceda 5h) | Descuento automático de tiempo de almuerzo configurable |
| **Art. 178** | Horas extraordinarias: máx. 2h diarias, 10h semanales, 100h anuales | Cálculo separado de horas extras con acumulado anual por empleado |
| **Art. 184** | Día de descanso semanal obligatorio | Marcado automático en calendario; alerta si se trabaja en domingo |
| **Art. 119** | Obligación del patrono de llevar registro de jornadas | Bitácora completa de marcaciones por empleado, exportable a auditoría |
| **Art. 187** | Registro de horas extraordinarias autorizadas | Reporte segregado de horas extras con totales mensuales y anuales |

### 5.2 Constitución de la República Bolivariana de Venezuela

| Artículo | Materia | Cómo lo cumple Cronometrix |
|---|---|---|
| **Art. 28** | Derecho de habeas data (acceso del trabajador a sus datos) | El empleado puede solicitar y recibir su histórico de marcaciones |
| **Art. 60** | Derecho a la privacidad y protección de datos personales | Datos almacenados localmente, cifrados en tránsito (HTTPS), acceso por roles |

### 5.3 LOPCYMAT (Ley Orgánica de Prevención, Condiciones y Medio Ambiente de Trabajo)

| Artículo | Materia | Cómo lo cumple Cronometrix |
|---|---|---|
| **Art. 53, num. 9** | Derecho del trabajador a información sobre su jornada | Acceso del empleado a su registro personal |
| **Art. 56, num. 4** | Obligación del patrono de llevar registros de jornada | Bitácora inmutable con sello de tiempo |

### 5.4 Cláusula de consentimiento biométrico (recomendada)

El sistema incluye plantilla de **consentimiento informado** para que cada empleado autorice expresamente la captura y procesamiento de su dato biométrico facial. Esto protege legalmente al patrono en caso de reclamación.

> **Importante:** Cronometrix proporciona las herramientas técnicas para cumplir con la ley. La aplicación correcta de los reglamentos internos de trabajo, contratos y consentimientos es responsabilidad del cliente y de su asesor jurídico.

---

## 6. ¿Qué incluye la inversión de USD 1.500?

### ✅ Incluido

| Ítem | Detalle |
|---|---|
| **Licencia de software** | 1 sede, hasta 200 empleados activos, hasta 4 dispositivos Hikvision |
| **Instalación** | Despliegue del sistema en el servidor del cliente vía conexión remota |
| **Configuración inicial** | Conexión con sus dispositivos Hikvision, configuración de turnos, tolerancias, calendario laboral |
| **Capacitación remota** | 8 horas vía videollamada (administrador + supervisores), agendables en bloques de 2h |
| **Acceso remoto** | Túnel Cloudflare configurado con subdominio del cliente: `{su-empresa}.cronometrix.com` |
| **Soporte año 1** | 12 meses, horario de oficina, vía sistema de tickets |
| **Actualizaciones** | Todas las versiones publicadas durante los primeros 6 meses |
| **Garantía** | 30 días post go-live para corrección de bugs críticos sin costo adicional |

### ❌ No incluido

- Hardware (lectores Hikvision, servidor, red interna) — el cliente provee
- Conexión a Internet del servidor
- Capacitación adicional fuera de las 8 horas pactadas
- Integraciones a la medida con sistemas de nómina propietarios
- Migración de datos históricos desde otros sistemas
- Soporte presencial (toda atención es remota)
- Personalizaciones del sistema, módulos a la medida, cambios de diseño
- Visitas técnicas físicas

### 6.5 Add-ons opcionales

Si su operación crece, puede ampliar la licencia base sin reinstalar el sistema:

#### Empleados adicionales (pago único, expansión perpetua)

| Pack | Precio | Costo por empleado |
|---|---|---|
| +10 empleados | USD 80 | USD 8.00 |
| +20 empleados | USD 150 | USD 7.50 |
| +50 empleados | USD 325 | USD 6.50 |
| +100 empleados | USD 550 | USD 5.50 |

> Los packs son acumulables: ej. base 200 + pack 50 + pack 20 = 270 empleados activos.

#### Dispositivos Hikvision adicionales (pago único)

| Concepto | Precio | Detalle |
|---|---|---|
| 5to o 6to dispositivo | USD 200 c/u | Integración + configuración + prueba remota |
| 7mo u 8vo dispositivo | USD 250 c/u | Mayor carga concurrente |
| 9 o más dispositivos | A cotizar | Requiere revisión de arquitectura |

#### Sede adicional

| Concepto | Precio | Detalle |
|---|---|---|
| Instalación de sede adicional | **USD 750** | 50% de descuento sobre la base. Sede independiente con servidor propio y URL `{cliente-sede2}.cronometrix.com` (activable o desactivable a voluntad del cliente) |

> **Nota sobre acceso remoto:** la URL pública `{cliente}.cronometrix.com` es **opcional**. Puede activarse o desactivarse en cualquier momento desde el panel de administración. Con el acceso remoto desactivado, el sistema sigue operando en red local y conserva la sincronización cifrada con la nube para respaldo, sin exposición pública.

#### Servicios adicionales

| Concepto | Precio | Detalle |
|---|---|---|
| Capacitación remota adicional | USD 50 / hora | Más allá de las 8 horas incluidas |
| Capacitación presencial (Táchira) | USD 150 / hora + viáticos | Visita técnica al sitio |
| Migración de datos históricos | USD 200 – 500 | Según volumen y formato fuente |
| Integración con sistema de nómina | USD 1.000 – 2.000 | Galac, Profit u otros — alcance a definir |
| Onboarding express (go-live ≤5 días) | +USD 200 | Sobrecosto sobre la base |

#### Escalado de soporte año 2 (según tamaño de operación)

| Empleados activos | Estándar / año | Plus / año |
|---|---|---|
| Hasta 200 (base) | USD 300 | USD 375 |
| 201 – 500 | USD 400 | USD 500 |
| 501 – 1000 | USD 550 | USD 700 |
| Más de 1000 | A cotizar | A cotizar |

---

## 7. Soporte técnico — SLA año 1

| Concepto | Detalle |
|---|---|
| **Horario** | Lunes a viernes, 8:00 a.m. – 5:00 p.m. (hora Venezuela), días hábiles |
| **Canal** | Sistema de tickets (acceso web 24/7 para registro; atención en horario hábil) |
| **Tiempo de respuesta** | 4 horas hábiles desde apertura del ticket |
| **Tiempo de resolución** | Best-effort según severidad |
| **Severidad alta** (sistema caído) | Atención prioritaria mismo día hábil |
| **Severidad media** (función afectada con workaround) | 2 días hábiles |
| **Severidad baja** (consulta, mejora) | 5 días hábiles |
| **Excluido** | Soporte fuera de horario, soporte presencial, problemas de hardware, problemas de red del cliente, capacitación nueva |

---

## 8. Garantía

Durante los **30 días posteriores al go-live**, cualquier bug crítico (que impida el uso normal del sistema) será corregido sin costo adicional dentro del marco del SLA, independientemente de su origen.

No cubre: cambios de alcance, nuevas funcionalidades solicitadas, errores derivados de configuración modificada por el cliente sin asesoría, daños por hardware o red.

---

## 9. Renovación año 2 en adelante

A partir del mes 13, el cliente puede contratar el plan de soporte y actualizaciones anual:

| Plan | Costo anual | Incluye |
|---|---|---|
| **Soporte estándar** | USD 300 | Tickets en horario de oficina, parches de seguridad, correcciones de bugs |
| **Soporte plus** | USD 375 | Estándar + todas las actualizaciones de funcionalidad publicadas en el período |

> Sin contrato de soporte vigente, el sistema sigue funcionando, pero no se garantiza atención de incidentes ni se entregan nuevas versiones.

---

## 10. Forma de pago

- **50%** a la firma de la propuesta — se inicia instalación, configuración y agendamiento de capacitación
- **50%** al go-live — entrega oficial, validación del cliente, inicio del año de soporte
- **Moneda:** Dólares estadounidenses (USD), efectivo
- **Sin anticipo no se inicia trabajo**

---

## 11. Tiempos de entrega

| Hito | Plazo |
|---|---|
| Firma + 50% inicial | Día 0 |
| Instalación y configuración inicial | 7–8 días hábiles |
| Capacitación remota (8h) | A acordar dentro de las 2 semanas siguientes |
| Go-live (entrega oficial) | Día 10–15 hábil aprox. |
| Pago final 50% | Al go-live |

Sujeto a disponibilidad del cliente para sesiones de configuración y capacitación.

---

## 12. Requisitos técnicos del cliente

Para una instalación exitosa, el cliente debe proveer:

### Hardware

- **Servidor local** (puede ser PC dedicada): Linux Ubuntu 22.04+ o Debian 12+, 4 GB RAM mínimo, 50 GB disco, conexión Ethernet a la red interna.
- **Lectores Hikvision** compatibles con protocolo ISAPI (modelos faciales como DS-K1T671/672 y similares).
- **Conexión a Internet** estable en el servidor (mínimo 1 Mbps simétricos recomendado para sincronización con la nube).

### Acceso

- Acceso remoto SSH al servidor durante la instalación.
- Credenciales de administrador de los dispositivos Hikvision.
- Datos básicos de la empresa (nombre, RIF, sede, departamentos, listado de empleados, turnos vigentes).

---

## 13. Términos y condiciones

1. **Propiedad intelectual:** El software es licenciado, no vendido. El cliente no adquiere propiedad sobre el código fuente.
2. **Confidencialidad:** Ambas partes se comprometen a confidencialidad sobre información sensible compartida durante el proyecto.
3. **Datos del cliente:** Los datos de marcaciones son propiedad exclusiva del cliente. En caso de terminación, se entrega respaldo completo en formato estándar (SQLite + exportación CSV).
4. **Limitación de responsabilidad:** Cronometrix no asume responsabilidad por errores de cálculo derivados de configuración incorrecta proporcionada por el cliente, decisiones de nómina, ni reclamaciones laborales basadas en los reportes generados.
5. **Vigencia de la propuesta:** 30 días desde la fecha de emisión.
6. **Jurisdicción:** Cualquier controversia se resolverá en la jurisdicción de la República Bolivariana de Venezuela.

---

## 14. Próximos pasos

1. Confirmación por escrito de aceptación de la propuesta.
2. Pago del 50% inicial.
3. Agendamiento de sesión de levantamiento técnico (1 hora).
4. Cronograma confirmado de instalación y capacitación.

---

**Contacto comercial**

| | |
|---|---|
| **Nombre** | Gerswin Pineda |
| **Correo** | me@gerswin.com |
| **Teléfono** | 0412 594 5195 |

---

_Propuesta válida por 30 días desde su emisión. Precios expresados en dólares estadounidenses (USD)._
