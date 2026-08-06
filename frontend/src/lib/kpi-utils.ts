interface KPIRecord { late_minutes: number }

// Solo `late` tiene consumidor de producción (dashboard/page.tsx). `present`,
// `absent` y `total` quedaron muertos cuando el tablero pasó a
// /presence/today; `present` además era la definición vieja de "presente"
// (work_minutes > 0), lista para divergir de la nueva (entrada registrada
// sin salida).
export function aggregateKPIs(records: KPIRecord[]) {
  return {
    late: records.filter(r => r.late_minutes > 0).length,
  }
}
