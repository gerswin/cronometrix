# Consulta laboral — decisiones de cálculo en Cronometrix

**Para:** abogado laboral / contador venezolano
**De:** equipo Cronometrix
**Fecha:** 2026-08-03
**Jurisdicción:** Venezuela (LOTTT)

Cronometrix calcula horas trabajadas y produce una **pre-nómina**. No calcula
IVSS, FAOV, INCES, ISLR ni prestaciones; el importe que emite alimenta un
sistema de nómina externo.

Tres decisiones de cálculo se tomaron con criterio propio. Este documento las
expone para que confirme o corrija. Dos tienen base legal localizada y se
piden confirmar; la tercera está genuinamente abierta y es la que más nos
interesa.

El salario se **calcula en USD** y se **liquida en VES**. La tasa y fecha de
conversión, exigibles en el recibo del artículo 106, las aporta el sistema de
nómina de destino, no Cronometrix.

---

## Pregunta 1 — Descuento por llegada tarde *(abierta, es la que más nos importa)*

### Qué hacía el sistema

Descontaba **dos veces** la misma tardanza. Los minutos trabajados se miden
entre la entrada y la salida reales, de modo que llegar tarde ya reducía el
pago; encima se restaba un descuento monetario adicional por esos mismos
minutos.

### Qué hace ahora

Paga el tiempo **realmente trabajado** y nada más. Los minutos de tardanza se
siguen registrando como métrica —quedan visibles en el reporte como
`Min Retraso`— pero ya no se traducen en dinero.

Ejemplo, salario diario 50,00 y jornada de 8 h. Quien llega 30 minutos tarde y
sale a la hora nominal trabaja 450 minutos y cobra **46,87**. Antes cobraba
**43,75**.

### Nuestro razonamiento

Descontar el tiempo no trabajado no nos parece una *deducción*: el salario de
esos minutos sencillamente no se causa. Una deducción punitiva **adicional**
sería una segunda sanción por el mismo hecho, y entendemos que la LOTTT ya
prevé el remedio disciplinario por la vía correspondiente —artículo 79, y el
artículo 38 del Reglamento, que tipifica cuatro retardos en un mes como
incumplimiento reiterado del horario.

### Qué necesitamos que nos confirme

1. ¿Es correcto que **no** se descuente monetariamente la tardanza más allá del
   tiempo efectivamente no trabajado?
2. Si el patrono quisiera aplicar un descuento adicional, ¿sería lícito? ¿Bajo
   qué condiciones —reglamento interno, notificación previa, tope?
3. ¿Cambia la respuesta si el trabajador **compensa** el tiempo quedándose
   después de la hora de salida?

**Advertencia sobre nuestra fuente.** La base de esta decisión es doctrina de
divulgación, no jurisprudencia ni norma expresa. Es la más débilmente
fundamentada de las tres y por eso encabeza la consulta.

---

## Pregunta 2 — Hora extra en jornada nocturna o en día de descanso *(base localizada, confirmar)*

### La duda

Cuando una hora extraordinaria cae en jornada nocturna, concurren dos
recargos: el 30 % del artículo 117 y el 50 % del artículo 118. Lo mismo con el
50 % del artículo 120 por trabajo en día de descanso.

¿Se **suman** sobre el salario base (1,5 + 0,3 = 1,8) o se **componen** sobre
la hora ya recargada (1,3 × 1,5 = 1,95)?

### Qué hace ahora

Compone: el recargo por hora extra se calcula sobre el valor de la hora ya
recargada.

| Caso | Multiplicador | Importe (50,00/día, 8 h + 1 h extra) |
|---|---|---|
| Nocturno + extra | 1,3 × 1,5 = **1,95×** | **77,18** |
| Descanso + extra | 1,5 × 1,5 = **2,25×** | **89,06** |

Antes sumaba (1,8× y 2,0×), pagando **76,24** y **87,49** — es decir,
**subpagaba** en ambos casos.

Las horas ordinarias no cambiaron: un domingo normal sigue al 1,5× y una
jornada nocturna normal al 1,3×.

### Nuestro razonamiento

El artículo 118 ordena calcular el recargo tomando como base *"el salario
normal devengado durante la jornada respectiva"*. Si la jornada respectiva es
nocturna, ese salario normal ya incorpora el 30 %. La doctrina consultada
coincide: el 50 % se calcula sobre el valor de la hora nocturna, que ya
incluye el recargo.

### Qué necesitamos que nos confirme

1. ¿Es correcta la composición multiplicativa para nocturno + extra?
2. ¿Aplica el mismo criterio al día de descanso trabajado (artículo 120) más
   hora extra?
3. ¿Y en jornada mixta?

---

## Pregunta 3 — Conversión de salario mensual a diario *(base localizada, confirmar)*

### Qué hace ahora

El sistema exige que cada empleado declare la **unidad** de su salario: por
hora, diario o mensual. Antes el campo no tenía unidad y decía solo "Sueldo
Base", de modo que un salario mensual introducido ahí se pagaba **completo
cada día** — el período se multiplicaba por unos 30.

Para normalizar:

- **mensual → diario:** se divide entre **30**
- **por hora → diario:** se multiplica por las horas de la jornada ordinaria

### Nuestro razonamiento

El artículo 121 de la LOTTT establece que, estipulado el salario por mes, se
entiende por salario diario la **treintava parte** de la remuneración mensual,
y que el salario por hora resulta de dividir el diario entre las horas de la
jornada.

### Qué necesitamos que nos confirme

1. ¿Es el divisor 30 el correcto para este uso —remunerar días efectivamente
   trabajados—, o el artículo 121 aplica solo a prestaciones e indemnizaciones?
2. ¿Debe usarse otro divisor cuando la jornada semanal no es de lunes a
   viernes?

---

## Anexo — Cómo verificar los importes

Todos los cálculos son aritmética entera en céntimos, con una sola división al
final para no perder precisión. Con salario diario de 50,00 (5 000 céntimos) y
jornada ordinaria de 480 minutos:

| Concepto | Fórmula | Resultado |
|---|---|---|
| Jornada completa | 480 × 5000 / 480 | 5 000 |
| 60 min extra | 60 × 5000 × 150 / (100 × 480) | 937 |
| 60 min extra, nocturno | 60 × 5000 × 150 × 130 / (100 × 100 × 480) | 1 218 |
| 60 min extra, descanso | 60 × 5000 × 150 × 150 / (100 × 100 × 480) | 1 406 |
| Mensual 1 500,00, jornada completa | 480 × 150000 / (30 × 480) | 5 000 |

El truncamiento es siempre hacia abajo, nunca redondeo. Con hasta cuatro
componentes por día, eso favorece sistemáticamente al patrono en unos pocos
céntimos. **Si prefiere redondeo simétrico, díganoslo** — es un cambio de una
línea y también nos interesa su criterio sobre esto.

---

## Lo que NO estamos preguntando

Para acotar el alcance de la consulta: no calculamos ni gestionamos IVSS,
FAOV, INCES, ISLR, prestaciones, vacaciones ni bono vacacional. Tampoco
emitimos recibos de pago. Si considera que alguno de esos elementos es
inseparable de lo anterior, agradeceríamos que lo señale.
