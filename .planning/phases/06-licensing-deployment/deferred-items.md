## Pre-existing build/type errors (fixed inline because plan 06-02 verify gates demand `tsc --noEmit` and `next build` exit 0)

### From plan 06-02 execution (Task 2) — Rule 3 inline fixes

The plan's `<verification>` block explicitly requires `cd frontend && npx tsc --noEmit` and `cd frontend && npm run build` to exit 0. Pre-existing TS errors and Suspense boundary issues blocked these gates from passing. Fixes were minimal and behavior-preserving.

- `frontend/src/__tests__/device-banner.test.tsx`: replaced `Partial<Device>` with explicit `Device[]` typing on the test arrays. No runtime behavior change.
- `frontend/src/components/dashboard/dept-chart.tsx`: Recharts v3 Tooltip formatter type changed to expect `ValueType | undefined`; widened the `val` parameter and guarded with `typeof val === 'number'` fallback. Same rendered output for the present-count case.
- `frontend/src/app/login/page.tsx`: extracted body into `LoginPageInner` and wrapped the default export in `<Suspense>` so Next.js 16 can statically prerender the route. Required because `useSearchParams()` must live under a Suspense boundary in App Router 16.

All three fixes were committed alongside the plan 06-02 Task 2 commit because reverting them would re-break the `next build` verify gate the plan author explicitly enumerated.

