/**
 * Centralized data-testid catalog — single source of truth for all Playwright specs.
 * Plans 07-12 MUST add entries here rather than hardcoding strings in spec files.
 */
export const SEL = {
  // Layout
  topBarTitle: 'topbar-title',

  // Login (Spanish copy per the 2026-07-13 Phase 12 supersession of D-19)
  loginHeading: { role: 'heading', name: 'Iniciar Sesión' } as const,
  loginUsername: { role: 'textbox', name: 'Usuario' } as const,
  loginPassword: { role: 'textbox', name: 'Contraseña' } as const,
  loginSubmit: { role: 'button', name: 'Iniciar Sesión' } as const,
  loginShowPassword: { role: 'button', name: 'Mostrar contraseña' } as const,
  loginHidePassword: { role: 'button', name: 'Ocultar contraseña' } as const,

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

  // Device table per-row testids (Plan 09-10)
  devRow: (id: string) => `dev-row-${id}`,
  devActions: (id: string) => `dev-actions-${id}`,
  devStatus: (id: string) => `dev-status-${id}`,

  // Command modal testids (Plan 09-10)
  commandModal: 'command-modal',
  commandModalSelect: 'command-modal-select',
  commandModalSubmit: 'command-modal-submit',

  // Timesheet / Marcaciones page
  timesheetPage: 'timesheet-page',
  timesheetRow: (id: string) => `timesheet-row-${id}`,
  editTimesheetBtn: 'edit-timesheet-btn',
  timesheetPeriodPicker: 'timesheet-period-picker',

  // Novedad modal (Plan 09-09)
  openNovedadModal: 'open-novedad-modal',
  novedadModal: 'novedad-modal',
  novedadJustification: 'novedad-justification',
  novedadEvidence: 'novedad-evidence',
  novedadSubmit: 'novedad-submit',

  // Employee CRUD (Plan 09-09)
  newEmpButton: 'new-employee-button',
  newEmpForm: 'new-employee-form',
  newEmpSubmit: 'new-employee-submit',
  empActions: (id: string) => `emp-actions-${id}`,
  empActionEdit: (id: string) => `emp-action-edit-${id}`,
  empActionDeactivate: (id: string) => `emp-action-deactivate-${id}`,

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
