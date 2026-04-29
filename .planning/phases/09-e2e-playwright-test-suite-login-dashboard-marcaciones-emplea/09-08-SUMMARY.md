---
phase: 09-e2e-playwright-test-suite-login-dashboard-marcaciones-emplea
plan: 08
subsystem: testing
tags: [playwright, e2e, dashboard, data-testid, sse, ring-buffer, recharts]

requires:
  - phase: 09-01
    provides: Playwright base config, auth fixtures, SEL catalog scaffold
  - phase: 09-06
    provides: SEL catalog with kpiPresentes/kpiRetraso/kpiDispositivos/kpiAlertas/donutDept/ringBuffer/sseBanner entries
  - phase: 04-frontend-ui
    provides: ActivityFeed, DeptChart, KPITile, SSEReconnectBanner components; ring-buffer 20-event cap (D-6); SSE backoff (D-4)

provides:
  - data-testid attributes on all 5 dashboard component surfaces (KPI, donut, ring-buffer, photo, SSE banner)
  - dashboard.spec.ts with 7 tests at D-02 UAT depth
  - SSE banner always-in-DOM pattern (hidden attr) enabling toBeAttached() in Playwright
  - ringRow/photoImg/photoFallback entries added to SEL catalog

affects: [09-09, 09-10, 09-11, 09-12, 09-13]

tech-stack:
  added: []
  patterns:
    - "Always-in-DOM SSE banner: use HTML hidden attr instead of conditional render so Playwright toBeAttached() works regardless of connection state"
    - "KPI testId prop: generic KPITile accepts optional testId and passes to data-testid; page sets slug per tile"
    - "Ring-buffer rows use data-testid=ring-row-{event.id} enabling count assertions with locator('[data-testid^=\"ring-row-\"]')"
    - "mutateDateTime helper in spec creates 25 unique event timestamps for cap-at-20 test"

key-files:
  created:
    - frontend/e2e/dashboard.spec.ts
  modified:
    - frontend/src/components/dashboard/kpi-tile.tsx
    - frontend/src/components/dashboard/dept-chart.tsx
    - frontend/src/components/dashboard/activity-feed.tsx
    - frontend/src/components/dashboard/sse-reconnect-banner.tsx
    - frontend/src/components/dashboard/__tests__/sse-reconnect-banner.test.tsx
    - frontend/src/app/(dashboard)/dashboard/page.tsx
    - frontend/e2e/fixtures/selectors.ts

key-decisions:
  - "SSE banner always rendered in DOM (hidden attr controls visibility) — enables toBeAttached() in tests without triggering disconnect"
  - "KPITile gets optional testId prop; page passes kpi-{slug} per tile rather than deriving from title — decouples test id from display text"
  - "SSE disconnect simulation deferred to future plan — requires backend test-mode endpoint (e.g. ?disconnect_sse=1 query param on /events/stream)"

patterns-established:
  - "Dashboard testId convention: data-testid on outermost container div; child rows use data-testid^= prefix locators"
  - "RESEARCH §Pitfall 5 compliance: SSE test asserts DOM attachment + hidden state only, no backoff-timing assertions"

requirements-completed: [E2E-DASHBOARD, E2E-SELECTORS]

duration: 18min
completed: 2026-04-29
---

# Phase 09 Plan 08: Dashboard E2E Spec Summary

**7-test Playwright suite at D-02 UAT depth — KPIs, donut, ring-buffer 20-cap, photo fallback, SSE banner, empty state; data-testid attributes added to all 5 dashboard component surfaces.**

## Performance

- **Duration:** 18 min
- **Started:** 2026-04-29T03:55:00Z
- **Completed:** 2026-04-29T04:13:00Z
- **Tasks:** 2 completed
- **Files modified:** 7

## Accomplishments

- Added `data-testid` attributes to `kpi-tile.tsx` (via `testId` prop), `dept-chart.tsx` (donut-by-dept wrapper div on both branches), `activity-feed.tsx` (ring-buffer ul, ring-row-{id} li, photo-img/photo-fallback on EventAvatar), `sse-reconnect-banner.tsx` (always-in-DOM with `hidden` attr)
- Wrote `dashboard.spec.ts` with 7 tests (T-01 through T-07) covering all D-02 surfaces: KPI labels, count after events, donut render, ring-buffer 20-cap, photo fallback, SSE banner attachment, empty state
- Extended SEL catalog with `ringRow`, `photoImg`, `photoFallback` entries
- Updated `sse-reconnect-banner.test.tsx` Vitest test to match new always-in-DOM behavior (previously expected `null` when not reconnecting)

## Task Commits

1. **Task 1: Add data-testid attributes to dashboard components** — `5d4a4ff` (feat)
2. **Task 2: Write dashboard.spec.ts** — `56d7f6f` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `frontend/e2e/dashboard.spec.ts` — 7 Playwright tests at D-02 UAT depth (7 tests, 170 lines)
- `frontend/src/components/dashboard/kpi-tile.tsx` — added optional `testId` prop → `data-testid={testId}` on root div
- `frontend/src/components/dashboard/dept-chart.tsx` — wrapped both empty-state and chart branches in `data-testid="donut-by-dept"` div
- `frontend/src/components/dashboard/activity-feed.tsx` — added `data-testid="ring-buffer"` on ul, `ring-row-{id}` on each li, `photo-img`/`photo-fallback` on EventAvatar branches
- `frontend/src/components/dashboard/sse-reconnect-banner.tsx` — changed from conditional render to always-in-DOM with `hidden` attr + `data-testid="sse-disconnect-banner"` + `role="alert" aria-live="polite"`
- `frontend/src/components/dashboard/__tests__/sse-reconnect-banner.test.tsx` — updated "renders nothing" test to "present in DOM but hidden" to match new behavior
- `frontend/src/app/(dashboard)/dashboard/page.tsx` — added `testId="kpi-{slug}"` to all 4 KPITile instances
- `frontend/e2e/fixtures/selectors.ts` — added `ringRow`, `photoImg`, `photoFallback` to SEL catalog

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Actual component file names differ from plan's best-effort names**
- **Found during:** Task 1
- **Issue:** Plan listed `kpi-card.tsx`, `dept-donut.tsx`, `ring-buffer-list.tsx`, `sse-banner.tsx` as target files — actual names are `kpi-tile.tsx`, `dept-chart.tsx`, `activity-feed.tsx`, `sse-reconnect-banner.tsx`
- **Fix:** Read directory listing first (as instructed by plan's `read_first`), applied changes to actual files
- **Files modified:** All 4 actual files

**2. [Rule 1 - Bug] SSEReconnectBanner returned null when not reconnecting, breaking toBeAttached()**
- **Found during:** Task 1 implementation
- **Issue:** Plan's spec asserts `toBeAttached()` on the SSE banner element; original component returned `null` when `reconnecting=false`, so element was never in DOM
- **Fix:** Changed component to always render with `hidden` attr; updated Vitest test that expected `null` to expect `hidden=true`
- **Files modified:** `sse-reconnect-banner.tsx`, `__tests__/sse-reconnect-banner.test.tsx`
- **Commit:** `5d4a4ff`

**3. [Rule 2 - Missing critical] Added aria-live and role="alert" to SSE banner**
- **Found during:** Task 1
- **Issue:** Banner was a plain div; reconnecting state is an important status change that screen readers should announce
- **Fix:** Added `role="alert"` and `aria-live="polite"` to banner element
- **Files modified:** `sse-reconnect-banner.tsx`

## Known Limitations

### SSE Disconnect Simulation (deferred)

The SSE banner test (T-06) asserts DOM attachment + `toBeHidden()` while connected — it does NOT test the actual disconnect → banner-appears flow. Per RESEARCH §Pitfall 5, asserting exact SSE backoff timings is flake-prone without controlled backend hooks.

To implement a full disconnect test in a future plan:
1. Add `?disconnect_sse=1` query param support to `GET /events/stream` in the backend (returns 200 then closes immediately)
2. Update T-06 to navigate to `/dashboard?disconnect_sse=1`, then assert `toBeVisible()` on the banner with a generous timeout

This is tracked here. No ticket needed — the banner wiring is correct; only the E2E test coverage depth is partial.

## Self-Check: PASSED

- `frontend/e2e/dashboard.spec.ts` — FOUND
- `frontend/src/components/dashboard/kpi-tile.tsx` — FOUND
- `frontend/src/components/dashboard/dept-chart.tsx` — FOUND
- `frontend/src/components/dashboard/activity-feed.tsx` — FOUND
- `frontend/src/components/dashboard/sse-reconnect-banner.tsx` — FOUND
- `frontend/e2e/fixtures/selectors.ts` — FOUND
- Commit `5d4a4ff` — FOUND
- Commit `56d7f6f` — FOUND
- `data-testid="sse-disconnect-banner"` — FOUND in component
- `data-testid="ring-buffer"` — FOUND in component
- `data-testid="donut-by-dept"` — FOUND in component
- Test count: 7 (≥ 6 required)
- Vitest dashboard tests: 14 passed, 0 failed
