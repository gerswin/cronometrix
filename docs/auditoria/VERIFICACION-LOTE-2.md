# Verificación de los 25 hallazgos restantes

**Verificado contra:** `48af434` (main, 2026-08-03)
**Auditoría original:** `docs/auditoria/INFORME-AUDITORIA-INTEGRAL.md`, contra `9b36341`
**Evidencia detallada:** `docs/auditoria/verificacion/lote-{1,2,3,4}.md`

Completa la verificación empezada en `VERIFICACION-Y-PLAN.md`, que cubrió los 10
críticos y 4 altos por muestreo. Aquí van los 25 restantes.

Nota de conteo: el documento anterior decía 26. Son 25 — **H-01 se verificó y se
arregló** en el trabajo del acantilado de tolerancia.

---

## Resultado

**Ninguno resultó completamente falso.** De 25: **20 confirmados**, **5 con
matiz**. El método del auditor se sostiene: 39 de 40 hallazgos verificados a lo
largo del proyecto resultaron reales en sustancia.

Pero la sustancia no es lo mismo que la descripción, y ahí hay un patrón que
importa más que el recuento.

## Lo que hay que saber antes de actuar sobre esta auditoría

### 1. Tres titulares inducen al arreglo equivocado

| Hallazgo | El titular dice | Lo que pasa realmente |
|---|---|---|
| **C-10** | "Acuse exitoso **antes** de persistencia" | Es al revés: persiste y luego confirma. El defecto real es tragar errores y responder 200 igual. Devolver 500 —el arreglo obvio— perdería **más** eventos, porque el firmware bloquea su cola ante cualquier no-200. |
| **H-13** | Expiración de licencia "desactivada" | Es una decisión de producto documentada (D-07, expiración blanda para instalaciones sin conectividad). El defecto es que no exista **ningún** control, no que exista este. |
| **H-06** | Vacaciones "sin remuneración completa" | El pago de vacaciones **ya existía** en el commit auditado. Las líneas que la propia auditoría cita apuntan a un comentario sobre una limitación acotada, no a una ausencia. Lo real es que falta motor de derecho por antigüedad. |

### 2. Dos afirmaciones probatorias son falsas

- **"La prueba QA E3 acepta el pago duplicado"** — falso. `docs/QA-GUIDE.md:621`
  documenta `$50 + $18.75 = $68.75`, el importe correcto. Corregido en `bf1231e`.
- **"Una prueba acepta expresamente el caso entre dispositivos"** (M-01) —
  falso. Los dos tests de deduplicación existentes reproducen contra el **mismo**
  dispositivo.

Ambas son del mismo tipo: el hallazgo señala un problema real y respalda su
evidencia con un test que no dice lo que se afirma.

### 3. Dos recomendaciones harían daño si se siguen literalmente

- **M-01** empuja hacia deduplicación por contenido entre dispositivos. Es
  exactamente lo que se rechazó a propósito en el inbox durable
  (`027_device_push_inbox.sql`): dos cuerpos idénticos pueden ser dos eventos
  legítimos, y un falso positivo **pierde una marcación real**.
- **H-10** pide plazos y borrado verificable de datos biométricos y médicos.
  **H-09 exige preservar esos mismos archivos** como rastro inmutable de nómina,
  citando el COT 2020 sobre conservación tributaria. Son las mismas fotos, el
  mismo XML, las mismas evidencias.

Ese segundo choque es el hallazgo más importante de esta verificación, y ninguna
revisión hallazgo-por-hallazgo lo habría visto.

**La separación correcta:** la *plantilla facial viva* se purga al terminar la
relación laboral —hoy no se purga, y debería—, mientras que la *foto de evidencia
de una marcación* tiene su propio reloj de retención, probablemente mucho más
largo. Tratarlas como el mismo dato es lo que hace irreconciliables a H-09 y H-10.

### 4. Dos hallazgos cambiaron por trabajo posterior a la auditoría

- **H-05** — el tope diario que contaba dos veces las horas extra lo arregló
  `e8b04c5`, el mismo commit de C-01 y C-02. Sobrevive la falta de autorización
  y registro legal del artículo 183.
- **M-06** — el trigger de auditoría de empleados omite `position`, `hire_date`,
  `face_id`, y ahora también `salary_kind` y `terminated_on`. **Esas dos últimas
  las añadimos nosotros** sin actualizar el trigger. El hallazgo empeoró por
  nuestra causa.

---

## Veredictos

### Altos

| ID | Veredicto | Nota |
|----|-----------|------|
| H-02 | Confirmado | Salida de turno nocturno atribuida al día equivocado |
| H-05 | Con matiz | Tope diario ya arreglado; falta autorización y registro (art. 183) |
| H-06 | Con matiz | El pago de vacaciones existe; falta motor por antigüedad |
| H-07 | Confirmado | El código ya nombra "H-07" como defecto conocido (`reports/service.rs:481-485`) |
| H-09 | Confirmado | `global_rules` es fila única sobrescrita; sin snapshot, aprobación ni hash |
| H-10 | Confirmado | `purge.rs` solo revoca el mapeo facial del lector; fotos, XML y evidencias nunca se tocan |
| H-11 | Confirmado | `department_id` no existe en `users`; `update_employee` no extrae claims |
| H-12 | Confirmado | Sin rate limiting en ninguna parte; token JWT sin consulta a base — revocación tarda hasta 20 min |
| H-14 | Confirmado | Solo credenciales de dispositivo cifradas; el respaldo cubre `cronometrix.db` y ningún directorio de archivos |
| H-15 | Con matiz | Feriados sin implementar (real); pero la tabla ya cita el art. 173 para el componente semanal — hay que fusionar, no añadir |
| H-16 | Confirmado, con corrección | El USDT es para comisiones de vendedores en `MANUAL-VENDEDORES.md`, no para el cobro al cliente |

### Medios

| ID | Veredicto | Nota |
|----|-----------|------|
| M-01 | Con matiz | Sustancia real; evidencia falsa; recomendación peligrosa (ver arriba) |
| M-02 | Confirmado | Cada cara desconocida se propaga a todos los empleados con ventana solapada |
| M-03 | Confirmado | Extremos únicamente, almuerzo fijo incondicional, solo el primer par |
| M-04 | Confirmado | Permiso pone el día entero a cero; filtro `shift_type` ausente en la consulta secundaria |
| M-05 | Confirmado | Consultas secuenciales en autocommit, sin transacción de lectura |
| M-06 | Confirmado | Ver §4 — empeorado por nuestro propio trabajo |
| M-07 | Confirmado | `leaves/handlers.rs` confía en `Content-Type`; `daily_records/handlers.rs` **sí** valida magic bytes. El arreglo existe en el repo y nunca se portó |
| M-08 | Confirmado | Calidad facial del navegador, sin segundo detector ni PAD; autodocumentado como frontera de confianza |
| M-09 | Confirmado | Bearer en query de SSE; nginx ya no filtra por log, pero historial y proxies sí; cero cabeceras defensivas |
| M-11 | Confirmado | Provisión best-effort sin estado de conformidad; `allow_insecure_tls` desactiva verificación |
| M-12 | Confirmado | Sin `CHECK` sobre salario ni minutos; migración 014 usa `PRAGMA writable_schema`; `/health` es `SELECT 1` |

### Bajos

| ID | Veredicto | Nota |
|----|-----------|------|
| L-01 | Confirmado | `/setup/status` sin autenticar expone `initialized`/`licensed` |
| L-02 | Confirmado (parcial) | Instalación 7-8 vs 3-5 días, 1 vs 5 Mbps, modelos 671/672 vs 341/342 reales. El límite 5MB/10MB es más plausiblemente validación en dos capas que deriva accidental |

---

## Cómo usar esto

**No implementes desde los titulares.** El registro es: 39 de 40 hallazgos
reales en sustancia, 3 titulares que inducen al arreglo equivocado, 2
afirmaciones probatorias falsas y 2 recomendaciones que causarían daño si se
siguen literalmente.

La auditoría es un buen mapa de dónde mirar y un mal manual de qué hacer.

Antes de tocar código por cualquiera de estos: leer la evidencia en
`docs/auditoria/verificacion/`, comprobar contra el código actual —varios
cambiaron desde la auditoría— y verificar que la recomendación no choque con otro
hallazgo. H-09 contra H-10 no será el último caso.

## Fuera de alcance

Nada de esto adjudica derecho venezolano. Las afirmaciones sobre LOTTT, COT o
IGTF se tomaron como las presenta el auditor. Las tres decisiones laborales
pendientes están en `docs/legal/CONSULTA-LABORAL.md`.
