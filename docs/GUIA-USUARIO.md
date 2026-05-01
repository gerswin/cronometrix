# Cronometrix - Guía de Usuario

## 1. Qué es Cronometrix

Cronometrix es una plataforma de control de asistencia y gestión de tiempo para instalaciones locales que usan dispositivos biométricos Hikvision. La aplicación combina:

- Un backend en Rust que recibe marcaciones, calcula jornadas y mantiene auditoría.
- Un frontend en Next.js que permite operar empleados, dispositivos, reportes, auditoría y configuración.
- Una base SQLite local con cola de escrituras para reducir contención.
- Integración con licencias, eventos en tiempo real y sincronización con dispositivos.

Esta guía describe el uso funcional del sistema desde la perspectiva del usuario.

## 2. Roles disponibles

Cronometrix maneja tres roles principales:

- `admin`: acceso completo a configuración, usuarios, dispositivos, empleados, reglas, reportes, auditoría y mantenimiento.
- `supervisor`: acceso a consulta operativa y seguimiento de anomalías.
- `viewer`: acceso de solo lectura a los módulos principales.

La interfaz oculta algunas opciones según el rol, pero el backend siempre valida permisos.

## 3. Primer acceso

### 3.1 Activar la licencia

Antes de usar el sistema, la instalación debe estar licenciada.

Flujo:

1. Abrir `/setup/license`.
2. Ingresar la clave de licencia.
3. Esperar la confirmación.
4. Continuar con la configuración inicial.

Si la licencia no está activa, el sistema bloquea el acceso al resto de la aplicación.

### 3.2 Crear el primer administrador

Si la instalación es nueva:

1. Abrir `/setup`.
2. Completar nombre, usuario y contraseña del administrador.
3. Confirmar la creación.
4. Iniciar sesión en `/login`.

Si el sistema ya está configurado, la pantalla redirige al inicio de sesión.

## 4. Inicio de sesión

### 4.1 Acceso

La pantalla de login solicita:

- usuario
- contraseña

Al iniciar sesión correctamente:

- se guarda el token de acceso en memoria
- se mantiene una cookie de refresco
- el sistema redirige al panel principal

Si la sesión expira, el frontend intenta renovarla automáticamente. Si no puede hacerlo, redirige al login.

### 4.2 Cierre de sesión

Desde la barra superior, el usuario puede salir manualmente. Esto invalida la sesión y limpia el token local.

## 5. Navegación principal

La aplicación organiza el trabajo por módulos en una barra lateral:

- Dashboard
- Marcaciones
- Empleados
- Dispositivos
- Enrolamiento
- Anomalías
- Eventos
- Reportes
- Auditoría
- Usuarios
- Departamentos
- Reglas
- Configuración

## 6. Dashboard

El dashboard resume la operación diaria:

- KPIs operativos
- gráfico por departamento
- actividad reciente
- estado de dispositivos
- alertas de reconexión SSE

Este panel sirve para monitoreo rápido de la instalación.

## 7. Marcaciones

En este módulo se consultan las marcaciones procesadas por el backend.

Qué permite:

- navegar por semanas
- ver entradas y salidas
- revisar incidencias y novedades
- abrir el detalle de una marcación
- aplicar acciones sobre permisos o ausencias según el rol

Es la vista de trabajo para revisar el tiempo calculado por el sistema.

## 8. Empleados

Permite administrar el padrón de personal.

Acciones típicas:

- crear empleado
- editar datos personales
- cambiar departamento
- definir cargo, fecha de ingreso y salario base
- desactivar empleados

El sistema usa estos datos para:

- asociar marcaciones
- calcular reportes
- enrolar rostros
- aplicar reglas de asistencia

## 9. Dispositivos

En este módulo se administran los lectores biométricos.

Qué se puede hacer:

- ver la lista de dispositivos
- revisar estado de conexión
- consultar información básica
- enviar comandos, como abrir puerta
- desactivar o actualizar equipos

También se visualizan estados como conexión, último contacto y otras señales operativas.

## 10. Enrolamiento

El enrolamiento vincula el rostro de un empleado con el sistema y con los dispositivos.

Flujos disponibles:

- capturar rostro desde webcam
- subir una foto existente
- capturar desde un dispositivo
- seguir el estado de sincronización por equipo

Qué ocurre detrás:

1. Se crea la sesión de enrolamiento.
2. Se almacena la foto normalizada.
3. Se generan tareas de sincronización por dispositivo.
4. El sistema actualiza el estado hasta completar o fallar.

Si una sincronización falla, se puede reintentar.

## 11. Eventos

Los eventos son las marcaciones crudas recibidas desde dispositivos.

En esta vista puedes:

- revisar el historial de eventos
- filtrar por dispositivo, empleado o tipo
- abrir el detalle
- ver la foto asociada cuando existe

También sirve para entender por qué una marcación terminó en un cálculo específico.

## 12. Anomalías

Disponible para `admin` y `supervisor`.

Muestra situaciones que requieren revisión, por ejemplo:

- faltas
- inconsistencias
- marcaciones faltantes
- problemas detectados por las reglas de cálculo

Es la vista para seguimiento operativo y corrección manual.

## 13. Reportes

Permite consultar resultados consolidados y exportarlos.

Qué ofrece:

- filtros de período
- resúmenes operativos
- desglose por empleado o departamento
- exportación a Excel
- exportación a JSON

Se usa para cierre de nómina, control interno y análisis histórico.

## 14. Auditoría

La auditoría conserva un registro de cambios importantes.

Desde aquí se puede:

- revisar eventos de auditoría
- filtrar por actor, entidad o acción
- inspeccionar diferencias entre estados

Es la sección para trazabilidad y control interno.

## 15. Usuarios

Solo disponible para administradores.

Permite:

- crear usuarios del sistema
- asignar rol
- desactivar accesos
- revisar cuentas existentes

## 16. Departamentos

Solo administradores.

Sirve para:

- crear departamentos
- editar nombre y parámetros laborales
- definir hora de entrada y salida
- configurar pausa de almuerzo

Estos valores afectan el cálculo de jornadas y reportes.

## 17. Reglas

Solo administradores.

Aquí se configuran parámetros globales como:

- tolerancia de entrada
- tolerancia de salida
- minutos bonificados
- otros ajustes de cálculo

Estas reglas afectan cómo el backend interpreta las marcaciones.

## 18. Configuración

Incluye información general de la instalación, como:

- nombre del tenant
- datos de presentación de la empresa
- ajustes administrativos

Si un campo cambia en paralelo desde otra sesión, el sistema avisa y recarga la información.

## 19. Flujo operativo recomendado

Un flujo normal de uso es:

1. Activar licencia.
2. Crear el primer administrador.
3. Configurar tenant y reglas.
4. Crear departamentos.
5. Registrar empleados.
6. Registrar dispositivos.
7. Enrolar rostros.
8. Verificar sincronización con equipos.
9. Revisar eventos y marcaciones.
10. Corregir anomalías si aparecen.
11. Exportar reportes para nómina o revisión interna.

## 20. Integraciones del sistema

### 20.1 Dispositivos Hikvision

El backend recibe:

- eventos de asistencia
- capturas de rostro
- estados de conexión

Y envía:

- comandos de dispositivo
- enrolamiento
- sincronización de perfiles

### 20.2 Tiempo real

La interfaz usa eventos en vivo para actualizar el feed de actividad y el estado de dispositivos.

### 20.3 Exportaciones

Los reportes pueden descargarse en formatos útiles para análisis y nómina.

## 21. Consejos de uso

- Mantén la configuración de reglas y departamentos alineada con la política real de la empresa.
- Verifica la hora y zona horaria de la instalación.
- Enrola empleados antes de esperar sincronización con dispositivos.
- Revisa auditoría cuando una modificación parezca inesperada.
- Usa reportes para validar que las reglas de cálculo reflejen lo esperado.

## 22. Qué hacer si algo falla

- Si no puedes entrar, revisa usuario, contraseña y estado de la licencia.
- Si un dispositivo aparece fuera de línea, valida red, energía y credenciales.
- Si un enrolamiento no sincroniza, revisa el estado de cada push por dispositivo.
- Si una marcación no cuadra, abre el evento y la auditoría asociada.
- Si el sistema rechaza un cambio, puede existir una colisión de versión o una validación de negocio.

## 23. Soporte

Para soporte conviene registrar:

- usuario afectado
- hora exacta
- pantalla o módulo
- mensaje de error
- dispositivo involucrado
- acción que se estaba intentando realizar

Eso acelera el diagnóstico del problema.
