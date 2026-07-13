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
  employee_code: string
  /** @deprecated backend now returns `employee_code`; kept for legacy callers. */
  cedula?: string
  name: string
  department_id: string
  department_name?: string
  position: string
  hire_date: string | null
  /** Per-employee base salary in cents (migration 018). Authoritative for payroll. */
  base_salary_cents: number
  status: 'active' | 'inactive' | 'pending'
  version: number
  created_at: string
  updated_at: string
}

export interface Department {
  id: string
  name: string
  base_salary_cents: number
  shift_start_time: string   // HH:MM
  shift_end_time: string     // HH:MM
  lunch_mode: 'fixed' | 'punch'
  lunch_duration_min: number | null
  status: string
  deleted_at: string | null
  version: number
  created_at: string
  updated_at: string
}

export type DeviceConnectionState = 'online' | 'offline' | 'unknown'
export type DeviceStatus = 'active' | 'inactive'

export interface Device {
  id: string
  name: string
  ip: string
  port: number
  scheme: 'http' | 'https'
  username: string
  direction: 'entry' | 'exit'
  allow_insecure_tls: boolean
  connection_state: DeviceConnectionState
  last_seen_at: string | null
  status: DeviceStatus
  deleted_at: string | null
  version: number
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
  jti: string
  token_type: 'access' | 'refresh'
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

export interface GlobalRules {
  late_arrival_tolerance_min: number
  early_departure_tolerance_min: number
  bonus_minutes: number
  effective_from: string
  version: number
  updated_at: string
}

export interface UpdateRulesRequest {
  late_arrival_tolerance_min?: number
  early_departure_tolerance_min?: number
  bonus_minutes?: number
  version: number
}

export interface Anomaly {
  id: string
  daily_record_id: string
  employee_id: string
  anchor_date: string
  code: string
  detail: string | null
  created_at: string
}

export interface DailyRecordDetail {
  id: string
  employee_id: string
  department_id: string
  anchor_date: string
  shift_type: string
  work_minutes: number
  overtime_minutes: number
  late_minutes: number
  early_departure_minutes: number
  is_rest_day_worked: boolean
  entry_at: string | null
  exit_at: string | null
  leave_id: string | null
  computed_at: string
  created_at: string
  updated_at: string
  anomalies: string[]
}

export interface RawAttendanceEvent {
  id: string
  employee_id: string | null
  device_id: string
  direction: 'entry' | 'exit'
  captured_at: string
  is_unknown: boolean
  face_id: string | null
  employee_no_string: string | null
  photo_path: string | null
  created_at: string
}

// ──────────────────────────────────────────────────────────────────────────
// Phase 7 — Facial Enrollment & Sync (07-02)
// Mirrors backend/src/enrollments/models.rs response types
// ──────────────────────────────────────────────────────────────────────────

export interface EnrollmentDevicePush {
  device_id: string
  device_name: string
  status: 'pending' | 'in_progress' | 'success' | 'failed'
  error_message: string | null
  started_at: string | null
  completed_at: string | null
}

export interface Enrollment {
  id: string
  employee_id: string
  status: 'in_progress' | 'success' | 'partial' | 'failed'
  started_at: string
  completed_at: string | null
  device_pushes: EnrollmentDevicePush[]
}

export interface CaptureFromDeviceState {
  capture_id: string
  status: 'capturing' | 'captured' | 'timeout' | 'error'
  photo_path: string | null
  photo_b64: string | null      // base64 JPEG iff status=='captured'
  error_message: string | null
}

export type UserRole = 'admin' | 'supervisor' | 'viewer'

export interface User {
  id: string
  username: string
  full_name: string
  role: UserRole
  status: 'active' | 'inactive'
  deleted_at: string | null
  version: number
  created_at: string
  updated_at: string
}

export interface CreateUserRequest {
  username: string
  full_name: string
  role: UserRole
  password: string
}

export interface UpdateUserRequest {
  full_name?: string
  role?: UserRole
  password?: string
  status?: 'active' | 'inactive'
  version: number
}
