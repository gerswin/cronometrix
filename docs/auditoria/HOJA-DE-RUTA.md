# Hoja de ruta de los 25 hallazgos verificados

**Base:** `docs/auditoria/VERIFICACION-LOTE-2.md` (veredictos) y
`docs/auditoria/verificacion/lote-{1,2,3,4}.md` (evidencia).

Ordenados por **riesgo por unidad de esfuerzo**, no por severidad nominal de la
auditoría. Un medio barato que cierra un agujero real va antes que un alto caro
que necesita diseño.

Dos hallazgos **no entran** en ningún bloque y se explican al final.

---

## Bloque 1 — Barato y real *(el arreglo ya existe, o casi)*

Cuatro cosas que cierran riesgo verificado en horas, no días. Es el único bloque
donde el trabajo es mecánico.

| ID | Qué | Por qué es barato |
|----|-----|-------------------|
| **M-07** | `leaves/handlers.rs` confía en el `Content-Type` del cliente | `daily_records/handlers.rs` **ya valida magic bytes**. El arreglo existe en el repo, a cincuenta líneas, y nunca se portó |
| **M-06** | El trigger de auditoría de empleados omite columnas | Añadir `position`, `hire_date`, `face_id`, `salary_kind`, `terminated_on`. **Las dos últimas las rompimos nosotros** al añadirlas sin tocar el trigger |
| **M-12** | Sin `CHECK` sobre salario ni minutos | Restricciones de dominio en migración. `/health` como `SELECT 1` es aparte y algo mayor |
| **L-01 + H-12** | Cero rate limiting en todo el backend | Un middleware cubre ambos. El código ya lo admite en un comentario propio |

**Por qué primero:** M-07 es una vía de subida de HTML disfrazado de imagen a un
directorio de evidencias médicas. M-06 es deuda que introdujimos hoy. Y el rate
limiting compone con el DoS de Argon2 que ya cerramos: sin él, `/setup/status`
y `/auth/login` siguen siendo enumerables sin coste.

---

## Bloque 2 — Exactitud de la marcación *(alimenta la nómina)*

| ID | Qué | Nota |
|----|-----|------|
| **H-02** | Salida de turno nocturno atribuida al día equivocado | El más grave del bloque: una salida tras medianoche recalcula el día nuevo, no el de inicio. El proceso de las 02:00 puede consolidar una anomalía que la salida de las 06:00 no repara |
| **M-02** | Cada cara desconocida se propaga a **todos** los empleados con ventana solapada | Convierte las anomalías en ruido: una sola cara no asociada marca a decenas de trabajadores |
| **M-03** | Primer-entra/último-sale y almuerzo fijo incondicional | Pausas largas cuentan como trabajo; jornadas cortas pierden un almuerzo que no ocurrió |
| **M-04** | Permiso solo de día completo; filtro `shift_type` ausente en la consulta secundaria | Ese filtro ausente es el mismo tipo de defecto que ya corregimos en I5 |

**Por qué segundo:** todo esto llega al importe pagado. H-02 en particular puede
dejar una jornada nocturna entera sin salida registrada.

---

## Bloque 3 — Retención contra preservación *(necesita diseño, no código)*

**H-09 y H-10 se contradicen y hay que resolverlos juntos.** H-10 pide plazos y
borrado verificable de datos biométricos y médicos; H-09 exige preservar **esos
mismos archivos** como rastro inmutable de nómina, citando conservación
tributaria.

No es un conflicto de implementación: es que ambos tratan como un solo dato lo
que son dos.

- **La plantilla facial viva** — se purga al terminar la relación laboral. Hoy
  `purge.rs` solo revoca el mapeo en el lector y **no borra nada del disco**.
- **La foto de evidencia de una marcación** — es prueba de jornada. Reloj de
  retención propio, mucho más largo, probablemente fijado por obligación fiscal.

Ese bloque incluye además:

| ID | Qué |
|----|-----|
| **H-09** | Sin cierre de período, snapshot, aprobación ni hash. Un cambio de salario hoy altera un período ya emitido |
| **H-14** | Solo las credenciales de dispositivo están cifradas; el respaldo cubre `cronometrix.db` y **ningún** directorio de fotos, rostros o evidencias |
| **M-05** | El reporte lee en consultas secuenciales sin transacción — puede ser internamente contradictorio |

**H-14 merece subrayado:** un `restore` hoy recupera la base y pierde todas las
imágenes. Eso no es degradación, es pérdida.

---

## Bloque 4 — Alcance de acceso

| ID | Qué |
|----|-----|
| **H-11** | `department_id` no existe en `users`; `update_employee` ni siquiera extrae claims. Cualquier `viewer` lee toda la biometría de la instalación; cualquier supervisor edita a cualquier empleado |

Requiere modelo de ámbito, no un parche. Va después del Bloque 3 porque el
diseño de retención condiciona qué se puede exponer y a quién.

---

## Bloque 5 — Operación de dispositivos y biometría

| ID | Qué |
|----|-----|
| **M-11** | Provisión best-effort sin estado de conformidad; `allow_insecure_tls` desactiva verificación |
| **M-08** | Calidad facial confiada al navegador, sin segundo detector ni PAD |
| **M-09** | Bearer en query de SSE; cero cabeceras defensivas en nginx |

---

## Bloque 6 — Documentos, no código

| ID | Qué |
|----|-----|
| **H-15** | Feriados prometidos y no implementados. **Ojo:** la tabla ya cita el art. 173 para el componente semanal — hay que fusionar, no añadir |
| **H-16** | Alcance fiscal. **Corrección:** el USDT es para comisiones de vendedores, no para el cobro al cliente |
| **L-02** | Deriva entre variantes: 7-8 contra 3-5 días, 1 contra 5 Mbps, modelos 671/672 contra 341/342 reales |

Barato y con riesgo contractual. Puede ir en paralelo a cualquier bloque técnico
porque no toca código.

---

## Los dos que no entran en ningún bloque

### M-01 — necesita una decisión, no un plan

La sustancia es real: la clave de deduplicación incluye `device_id`, así que la
misma marca en dos lectores persiste dos veces.

Pero **su recomendación haría daño**. Empuja hacia deduplicación por contenido
entre dispositivos, que es exactamente lo que el inbox durable rechaza a
propósito: dos cuerpos idénticos pueden ser dos eventos legítimos y un falso
positivo **pierde una marcación real**.

Antes de planificar hay que decidir qué significa "el mismo fichaje" cuando dos
lectores ven a la misma persona en la misma ventana. ¿Es duplicado, o es que
pasó por dos puertas? La respuesta depende del despliegue del cliente, no del
código.

### H-05 y H-06 — parcialmente muertos

De **H-05** solo sobrevive la falta de autorización y registro del artículo 183;
el tope diario ya se arregló. De **H-06**, solo la falta de motor de derecho
vacacional por antigüedad. Ambos restos son cumplimiento legal, no defectos de
cálculo, y dependen de la consulta laboral pendiente.

---

## Lo que no está aquí y sigue siendo lo más urgente

**La rotación de la clave de licencia.** C-06 está *contenido*, no cerrado:
`git show 6edc39f:do-functions/test-keys/test_priv.pem` sigue funcionando para
cualquiera con un clon del repositorio. No es un hallazgo pendiente de
planificar — es una acción del operador, diferida conscientemente, que vence
antes de emitir la primera licencia real.

Procedimiento: `docs/runbooks/rotacion-clave-licencia.md`.
