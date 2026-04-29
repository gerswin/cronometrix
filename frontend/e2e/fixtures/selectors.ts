/**
 * Centralized data-testid catalog — single source of truth for all Playwright specs.
 * Plans 07-12 MUST add entries here rather than hardcoding strings in spec files.
 */
export const SEL = {
  // Layout
  topBarTitle: 'topbar-title',

  // Login (English copy per D-19 Addendum — login page is English)
  loginUsername: { role: 'textbox', name: 'Username' } as const,
  loginPassword: { role: 'textbox', name: 'Password' } as const,
  loginSubmit: { role: 'button', name: 'Log in' } as const,

  // Dashboard KPIs (Spanish UI per D-19)
  kpiPresentes: 'kpi-empleados-presentes',
  kpiRetraso: 'kpi-retraso-hoy',
  kpiDispositivos: 'kpi-dispositivos-activos',
  kpiAlertas: 'kpi-alertas-diurnas',
  donutDept: 'donut-by-dept',
  ringBuffer: 'ring-buffer',
  ringRow: (id: string) => `ring-row-${id}`,
  photoImg: 'photo-img',
  photoFallback: 'photo-fallback',
  sseBanner: 'sse-disconnect-banner',

  // Audit page (added by Plan 05)
  auditPage: 'audit-page',
  auditRow: (id: string) => `audit-row-${id}`,
  auditFilterActor: 'audit-filter-actor',
  auditFilterFrom: 'audit-filter-from',
  auditFilterTo: 'audit-filter-to',
  auditFilterTable: 'audit-filter-table',

  // Employees page
  employeesPage: 'employees-page',
  employeeRow: (id: string) => `employee-row-${id}`,
  employeeSearch: 'employee-search',
  newEmployeeBtn: 'new-employee-btn',

  // Devices page
  devicesPage: 'devices-page',
  deviceRow: (id: string) => `device-row-${id}`,
  deviceStatus: (id: string) => `device-status-${id}`,

  // Timesheet / Marcaciones page
  timesheetPage: 'timesheet-page',
  timesheetRow: (id: string) => `timesheet-row-${id}`,
  editTimesheetBtn: 'edit-timesheet-btn',
  timesheetPeriodPicker: 'timesheet-period-picker',

  // Reports page
  reportsPage: 'reports-page',
  exportExcelBtn: 'export-excel-btn',
  exportPdfBtn: 'export-pdf-btn',

  // RBAC / Access denied
  accessRestricted: 'access-restricted',

  // Navigation sidebar
  navDashboard: 'nav-dashboard',
  navEmployees: 'nav-employees',
  navTimesheet: 'nav-timesheet',
  navDevices: 'nav-devices',
  navReports: 'nav-reports',
  navAudit: 'nav-audit',
} as const
