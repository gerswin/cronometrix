// MSW request handlers for vitest. Centralised so individual test files can
// pick the subset they need or rely on a shared `setupServer(...handlers)`.
//
// All paths use the absolute base `http://localhost:3001/api/v1` matching
// `frontend/src/lib/api.ts` axios baseURL. MSW intercepts at the network
// layer so the axios instance does not need any special configuration.

import { http, HttpResponse } from 'msw'

const API_BASE = 'http://localhost:3001/api/v1'

const EMPTY_AGG = {
  work_min: 0,
  ot_min: 0,
  late_min: 0,
  days_worked: 0,
  days_absent: 0,
  work_pay_cents: 0,
  ot_pay_cents: 0,
  night_premium_cents: 0,
  rest_day_surcharge_cents: 0,
  late_deduction_cents: 0,
  total_a_pagar_cents: 0,
  days_ivss: 0,
  days_vacation: 0,
  days_permission: 0,
  days_unpaid: 0,
}

// ──────────────────────────────────────────────────────────────────────
// /api/v1/reports/json (Plan 05-02)
// ──────────────────────────────────────────────────────────────────────

export const reportsJsonHandler = http.post(
  `${API_BASE}/reports/json`,
  async ({ request }) => {
    const body = (await request.json()) as {
      from_date: string
      to_date: string
    }
    return HttpResponse.json({
      header: {
        client_name: 'Test SA',
        client_rif: 'J-1-9',
        from_date: body.from_date,
        to_date: body.to_date,
        generated_at_iso: '2026-04-25T18:00:00Z',
      },
      rows: [],
      dept_subtotals: [],
      grand_total: { ...EMPTY_AGG },
      departments_in_order: [],
    })
  },
)

// ──────────────────────────────────────────────────────────────────────
// /api/v1/reports/excel (Plan 05-03)
// ──────────────────────────────────────────────────────────────────────

export const reportsExcelHandler = http.post(
  `${API_BASE}/reports/excel`,
  () =>
    new HttpResponse(
      new Blob(['xlsx-mock'], {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      }),
      {
        status: 200,
        headers: {
          'content-type':
            'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
          'content-disposition':
            'attachment; filename="prenomina_2026-04-01_2026-04-30.xlsx"',
        },
      },
    ),
)

// ──────────────────────────────────────────────────────────────────────
// /api/v1/tenant-info GET + PATCH (Plan 05-01)
// ──────────────────────────────────────────────────────────────────────

export const tenantInfoGetHandler = http.get(`${API_BASE}/tenant-info`, () =>
  HttpResponse.json({
    client_name: '',
    client_rif: '',
    address: '',
    version: 1,
    updated_at: '2026-04-25T00:00:00Z',
  }),
)

export const tenantInfoPatchHandler = http.patch(
  `${API_BASE}/tenant-info`,
  async ({ request }) => {
    const body = (await request.json()) as {
      client_name?: string
      client_rif?: string
      address?: string
      version: number
    }
    if (body.version !== 1) {
      return HttpResponse.json(
        {
          error: {
            code: 'VERSION_CONFLICT',
            message:
              'Esta información fue modificada por otro usuario. Recargue la página.',
          },
        },
        { status: 409 },
      )
    }
    return HttpResponse.json({
      client_name: body.client_name ?? '',
      client_rif: body.client_rif ?? '',
      address: body.address ?? '',
      version: 2,
      updated_at: '2026-04-25T01:00:00Z',
    })
  },
)

// ──────────────────────────────────────────────────────────────────────
// /api/v1/daily-records (used by drill-down dialog)
// ──────────────────────────────────────────────────────────────────────

export const dailyRecordsHandler = http.get(
  `${API_BASE}/daily-records`,
  () =>
    HttpResponse.json({
      data: [],
      total: 0,
      limit: 100,
      offset: 0,
    }),
)

// ──────────────────────────────────────────────────────────────────────
// /api/v1/departments list
// ──────────────────────────────────────────────────────────────────────

export const departmentsHandler = http.get(`${API_BASE}/departments`, () =>
  HttpResponse.json({
    data: [
      {
        id: 'd1',
        name: 'Operaciones',
        base_salary: 100_000,
        shift_start: '08:00',
        shift_end: '17:00',
        lunch_mode: 'fixed',
        lunch_minutes: 60,
        overtime_threshold_minutes: 480,
        is_overnight: false,
      },
    ],
    total: 1,
    limit: 100,
    offset: 0,
  }),
)

export const handlers = [
  reportsJsonHandler,
  reportsExcelHandler,
  tenantInfoGetHandler,
  tenantInfoPatchHandler,
  dailyRecordsHandler,
  departmentsHandler,
]
