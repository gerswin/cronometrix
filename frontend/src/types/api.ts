// All shapes derived from backend handler response types

export interface DailyRecord {
  id: string
  employee_id: string
  employee_name?: string   // joined by frontend enrichment
  department_id: string
  anchor_date: string      // YYYY-MM-DD
  shift_type: string
  work_minutes: number
  overtime_minutes: number
  late_minutes: number
  early_departure_minutes: number
  is_rest_day_worked: boolean
  entry_at: string | null  // ISO 8601
  exit_at: string | null   // ISO 8601
  leave_id: string | null
  computed_at: string
  created_at: string
  updated_at: string
  anomalies: string[]
}

/**
 * WR-02: Field name `data` matches the backend `crate::common::PaginatedResponse`
 * struct (see backend/src/common.rs). Do not rename to `items` — the wire
 * format is owned by the backend and several handlers depend on it.
 */
export interface PaginatedResponse<T> {
  data: T[]
  total: number
  limit: number
  offset: number
}

export interface Employee {
  id: string
  cedula: string
  name: string
  department_id: string
  department_name?: string
  position: string
  hire_date: string
  status: 'active' | 'inactive' | 'pending'
  created_at: string
  updated_at: string
}

export interface Department {
  id: string
  name: string
  base_salary: number
  shift_start: string   // HH:MM
  shift_end: string     // HH:MM
  lunch_mode: 'fixed' | 'punch'
  lunch_minutes: number
  overtime_threshold_minutes: number
  is_overnight: boolean
}

export interface Device {
  id: string
  name: string
  ip_address: string
  direction: 'entry' | 'exit' | 'both'
  status: 'online' | 'offline' | 'unknown'
  last_seen_at: string | null
  created_at: string
  updated_at: string
}

export interface AttendanceEvent {
  id: string
  employee_id: string | null
  device_id: string
  captured_at: string   // ISO 8601
  direction: 'entry' | 'exit'
  photo_path: string | null
  created_at: string
}

// SSE payload from GET /api/v1/events/stream
export interface AttendanceEventSSEPayload {
  id: string
  employee_id: string | null
  employee_name: string | null
  department: string | null
  captured_at: string
  direction: 'entry' | 'exit'
  has_photo: boolean
}

export interface Leave {
  id: string
  employee_id: string
  from_date: string
  to_date: string
  leave_type: 'medical' | 'vacation' | 'unpaid' | 'manual'
  justification: string
  evidence_path: string | null
  created_by: string
  status: 'active' | 'cancelled'
  version: number
  created_at: string
  updated_at: string
}

export interface JWTClaims {
  sub: string        // user_id
  role: 'admin' | 'supervisor' | 'viewer'
  exp: number
  iat: number
}

// ──────────────────────────────────────────────────────────────────────────
// Phase 5 — Reports & Tenant Info
// Mirrors backend/src/reports/models.rs::ReportPayload (Plan 05-02)
// and backend/src/tenant_info/models.rs::TenantInfo (Plan 05-01).
// ──────────────────────────────────────────────────────────────────────────

export interface BrandingHeader {
  client_name: string
  client_rif: string
  from_date: string
  to_date: string
  generated_at_iso: string
}

export interface Aggregates {
  work_min: number
  ot_min: number
  late_min: number
  days_worked: number
  days_absent: number
  work_pay_cents: number
  ot_pay_cents: number
  night_premium_cents: number
  rest_day_surcharge_cents: number
  late_deduction_cents: number
  total_a_pagar_cents: number
  days_ivss: number
  days_vacation: number
  days_permission: number
  days_unpaid: number
}

export interface EmployeeReportRow extends Aggregates {
  employee_id: string
  dept_id: string
  cedula: string
  nombre: string
  departamento: string
  cargo: string
  shift_type: string
  anomaly_codes: string[]
  anomaly_count: number
}

export interface DeptSummary {
  id: string
  name: string
}

export interface DeptSubtotal {
  dept_id: string
  dept_name: string
  aggregates: Aggregates
}

export interface ReportPayload {
  header: BrandingHeader
  rows: EmployeeReportRow[]
  dept_subtotals: DeptSubtotal[]
  grand_total: Aggregates
  departments_in_order: DeptSummary[]
}

export type PeriodType =
  | 'weekly'
  | 'biweekly_first'
  | 'biweekly_second'
  | 'monthly'
  | 'custom'

export interface ReportFilters {
  period_type: PeriodType
  from_date: string
  to_date: string
  department_ids?: string[]
  include_inactive?: boolean
  employee_id?: string
  shift_type?: 'day' | 'night' | 'mixed'
}

export interface TenantInfo {
  client_name: string
  client_rif: string
  address: string
  version: number
  updated_at: string
}
