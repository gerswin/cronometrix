interface KPIRecord { work_minutes: number; late_minutes: number; leave_id: string | null }

export function aggregateKPIs(records: KPIRecord[]) {
  return {
    present: records.filter(r => r.work_minutes > 0).length,
    late: records.filter(r => r.late_minutes > 0).length,
    absent: records.filter(r => r.work_minutes === 0 && !r.leave_id).length,
    total: records.length,
  }
}
