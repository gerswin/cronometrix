import type { DailyRecord } from '@/types/api'

export function dailyRecordKey(
  record: Pick<DailyRecord, 'employee_id' | 'anchor_date'>,
): string {
  return `${record.employee_id}:${record.anchor_date}`
}
