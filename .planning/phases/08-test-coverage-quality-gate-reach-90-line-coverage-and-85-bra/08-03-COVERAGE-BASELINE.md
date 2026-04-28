# 08-03 Coverage Baseline (working artifact)

Generated: 2026-04-28T17:32:00Z
Backend lcov.info: backend/lcov.info (479 KB, 319 tests, 22 skipped)
Frontend lcov.info: frontend/coverage/lcov.info (29.6 KB, 105 tests in 20 files)

## Run-time provenance

- **Frontend command:** `cd frontend && npx vitest run --coverage` (Vitest 4.1.5 + @vitest/coverage-v8 4.1.5)
- **Backend command:** `cd backend && cargo llvm-cov nextest --all-features --ignore-filename-regex '(main\.rs|tests/common/.*)' --lcov --output-path lcov.info`
- **Backend caveat — branch coverage absent:** This baseline run used **stable rustc 1.93.0** (Homebrew-installed; no rustup on local box). cargo-llvm-cov's `--branch` flag is nightly-only (RESEARCH § Branch coverage path decision). Stable lcov output therefore reports `BRF:0` for every record and the script reports project-wide branch as 100% (no data). Per-file branch% column below shows 100% for every backend row for the same reason. **Plan 04 + Plan 05 will run with nightly toolchain installed via rustup; backend branch coverage will be measured then.** Line coverage numbers below are accurate and measured.
- **Project gate target:** ≥90% line / ≥85% branch / ≥90% functions / ≥90% statements
- **Per-file floor:** ≥70% line / ≥60% branch / ≥70% functions / ≥70% statements (Vitest enforces all four; backend script enforces line + branch only)

## Backend gaps (sourced from scripts/enforce-coverage-floor.sh FAIL lines)

Project-wide backend: line=63.09% (LF=8414 LH=5308), branch=N/A (stable run, no --branch)

### Raw FAIL output from `bash scripts/enforce-coverage-floor.sh backend/lcov.info 85 70 60`

```
FAIL: backend/src/anomalies/handlers.rs line coverage 0.00% < floor 70%
FAIL: backend/src/auth/handlers.rs line coverage 67.39% < floor 70%
FAIL: backend/src/auth/models.rs line coverage 30.77% < floor 70%
FAIL: backend/src/calc/anomalies.rs line coverage 61.54% < floor 70%
FAIL: backend/src/config.rs line coverage 0.00% < floor 70%
FAIL: backend/src/daily_records/handlers.rs line coverage 0.00% < floor 70%
FAIL: backend/src/daily_records/service.rs line coverage 53.10% < floor 70%
FAIL: backend/src/db/mod.rs line coverage 46.67% < floor 70%
FAIL: backend/src/departments/service.rs line coverage 66.95% < floor 70%
FAIL: backend/src/devices/models.rs line coverage 50.00% < floor 70%
FAIL: backend/src/employees/service.rs line coverage 61.29% < floor 70%
FAIL: backend/src/enrollments/handlers.rs line coverage 0.94% < floor 70%
FAIL: backend/src/enrollments/models.rs line coverage 0.00% < floor 70%
FAIL: backend/src/enrollments/pusher.rs line coverage 56.57% < floor 70%
FAIL: backend/src/enrollments/service.rs line coverage 23.17% < floor 70%
FAIL: backend/src/events/handlers.rs line coverage 55.68% < floor 70%
FAIL: backend/src/isapi/client.rs line coverage 57.23% < floor 70%
FAIL: backend/src/leaves/handlers.rs line coverage 46.56% < floor 70%
FAIL: backend/src/leaves/service.rs line coverage 69.87% < floor 70%
FAIL: backend/src/license/fingerprint.rs line coverage 13.33% < floor 70%
FAIL: backend/src/license/service.rs line coverage 18.95% < floor 70%
FAIL: backend/src/recompute/nightly.rs line coverage 0.00% < floor 70%
FAIL: backend/src/recompute/worker.rs line coverage 0.00% < floor 70%
FAIL: backend/src/state/paths.rs line coverage 33.33% < floor 70%
FAIL: backend/src/supervisor/watchdog.rs line coverage 53.57% < floor 70%
FAIL: backend/src/workers/backfill.rs line coverage 0.00% < floor 70%
FAIL: backend/src/workers/purge.rs line coverage 0.00% < floor 70%
```

Exit code: 1 (27 file-level fails; project-wide branch check skipped because BRF=0 on stable rustc — re-run on nightly will surface the project-wide branch FAIL line).

Files below the 70% line floor (backend script flagged 27):

| File | Line% | Branch% | Func% | Floor miss |
|------|-------|---------|-------|------------|
| backend/src/anomalies/handlers.rs | 0.00 | N/A | 0.00 | line + func |
| backend/src/auth/handlers.rs | 67.39 | N/A | 16.67 | line + func |
| backend/src/auth/models.rs | 30.77 | N/A | 50.00 | line + func |
| backend/src/calc/anomalies.rs | 61.54 | N/A | 100.00 | line |
| backend/src/config.rs | 0.00 | N/A | 0.00 | line + func |
| backend/src/daily_records/handlers.rs | 0.00 | N/A | 0.00 | line + func |
| backend/src/daily_records/service.rs | 53.10 | N/A | 5.81 | line + func |
| backend/src/db/mod.rs | 46.67 | N/A | 33.33 | line + func |
| backend/src/departments/service.rs | 66.95 | N/A | 31.43 | line + func |
| backend/src/devices/models.rs | 50.00 | N/A | 71.43 | line |
| backend/src/employees/service.rs | 61.29 | N/A | 29.79 | line + func |
| backend/src/enrollments/handlers.rs | 0.94 | N/A | 2.70 | line + func |
| backend/src/enrollments/models.rs | 0.00 | N/A | 0.00 | line + func |
| backend/src/enrollments/pusher.rs | 56.57 | N/A | 53.85 | line + func |
| backend/src/enrollments/service.rs | 23.17 | N/A | 10.68 | line + func |
| backend/src/events/handlers.rs | 55.68 | N/A | 40.00 | line + func |
| backend/src/isapi/client.rs | 57.23 | N/A | 52.38 | line + func |
| backend/src/leaves/handlers.rs | 46.56 | N/A | 28.12 | line + func |
| backend/src/leaves/service.rs | 69.87 | N/A | 21.82 | line + func |
| backend/src/license/fingerprint.rs | 13.33 | N/A | 30.00 | line + func |
| backend/src/license/service.rs | 18.95 | N/A | 30.77 | line + func |
| backend/src/recompute/nightly.rs | 0.00 | N/A | 0.00 | line + func |
| backend/src/recompute/worker.rs | 0.00 | N/A | 0.00 | line + func |
| backend/src/state/paths.rs | 33.33 | N/A | 25.00 | line + func |
| backend/src/supervisor/watchdog.rs | 53.57 | N/A | 50.00 | line + func |
| backend/src/workers/backfill.rs | 0.00 | N/A | 0.00 | line + func |
| backend/src/workers/purge.rs | 0.00 | N/A | 0.00 | line + func |

**Backend file count below floor: 27**

### Backend files at or above floor (informational)

| File | Line% | Func% |
|------|-------|-------|
| backend/src/auth/middleware.rs | 100.00 | 100.00 |
| backend/src/auth/rbac.rs | 100.00 | 100.00 |
| backend/src/auth/service.rs | 93.85 | 77.78 |
| backend/src/calc/aggregation.rs | 98.84 | 100.00 |
| backend/src/calc/engine.rs | 92.11 | 100.00 |
| backend/src/calc/lunch.rs | 82.93 | 85.71 |
| backend/src/calc/overnight.rs | 95.31 | 100.00 |
| backend/src/calc/overtime.rs | 100.00 | 100.00 |
| backend/src/common.rs | 100.00 | 100.00 |
| backend/src/departments/handlers.rs | 79.59 | 57.14 |
| backend/src/devices/crypto.rs | 82.76 | 61.54 |
| backend/src/devices/handlers.rs | 83.57 | 44.74 |
| backend/src/devices/service.rs | 77.30 | 25.64 |
| backend/src/employees/handlers.rs | 78.12 | 58.82 |
| backend/src/enrollments/image_pipeline.rs | 87.67 | 70.00 |
| backend/src/enrollments/isapi_face.rs | 97.87 | 100.00 |
| backend/src/errors.rs | 95.74 | 100.00 |
| backend/src/events/service.rs | 87.39 | 60.53 |
| backend/src/isapi/events.rs | 98.08 | 100.00 |
| backend/src/isapi/parser.rs | 97.89 | 100.00 |
| backend/src/isapi/stream.rs | 84.13 | 66.67 |
| backend/src/license/middleware.rs | 100.00 | 100.00 |
| backend/src/reports/excel.rs | 94.01 | 36.84 |
| backend/src/reports/handlers.rs | 80.49 | 55.56 |
| backend/src/reports/money.rs | 98.55 | 100.00 |
| backend/src/reports/periods.rs | 93.23 | 94.44 |
| backend/src/reports/service.rs | 86.59 | 30.51 |
| backend/src/rules/handlers.rs | 77.45 | 23.81 |
| backend/src/setup/handlers.rs | 82.40 | 57.89 |
| backend/src/supervisor/mod.rs | 77.32 | 71.43 |
| backend/src/supervisor/status.rs | 100.00 | 100.00 |
| backend/src/supervisor/task.rs | 95.16 | 100.00 |
| backend/src/tenant_info/handlers.rs | 78.26 | 57.14 |
| backend/src/tenant_info/service.rs | 82.89 | 35.71 |

## Frontend gaps (sourced from Vitest --coverage threshold table + scripts/enforce-coverage-floor.sh against frontend/coverage/lcov.info)

Project-wide frontend: line=51.81% (LF=774 LH=401), branch=44.79% (BRF=585 BRH=262), functions=50.53% (FNF=281 FNH=142), statements=50.87% (435/855)

Targets: line=90%, branch=85%, functions=90%, statements=90%

### Raw FAIL output from `bash scripts/enforce-coverage-floor.sh frontend/coverage/lcov.info 85 70 60`

```
FAIL: src/components/providers.tsx line coverage 0.00% < floor 70%
FAIL: src/components/common/access-restricted.tsx line coverage 0.00% < floor 70%
FAIL: src/components/dashboard/activity-feed.tsx line coverage 0.00% < floor 70%
FAIL: src/components/dashboard/activity-feed.tsx branch coverage 0.00% < floor 60%
FAIL: src/components/dashboard/dept-chart.tsx line coverage 0.00% < floor 70%
FAIL: src/components/dashboard/dept-chart.tsx branch coverage 0.00% < floor 60%
FAIL: src/components/dashboard/kpi-tile.tsx line coverage 0.00% < floor 70%
FAIL: src/components/dashboard/kpi-tile.tsx branch coverage 0.00% < floor 60%
FAIL: src/components/dashboard/sse-reconnect-banner.tsx line coverage 0.00% < floor 70%
FAIL: src/components/dashboard/sse-reconnect-banner.tsx branch coverage 0.00% < floor 60%
FAIL: src/components/devices/command-modal.tsx line coverage 0.00% < floor 70%
FAIL: src/components/devices/command-modal.tsx branch coverage 0.00% < floor 60%
FAIL: src/components/devices/device-table.tsx line coverage 0.00% < floor 70%
FAIL: src/components/devices/device-table.tsx branch coverage 0.00% < floor 60%
FAIL: src/components/employees/employee-table.tsx line coverage 0.00% < floor 70%
FAIL: src/components/employees/employee-table.tsx branch coverage 0.00% < floor 60%
FAIL: src/components/enrollment/employee-enrollment-picker.tsx line coverage 0.00% < floor 70%
FAIL: src/components/enrollment/employee-enrollment-picker.tsx branch coverage 0.00% < floor 60%
FAIL: src/components/enrollment/enrollment-modal.tsx line coverage 63.33% < floor 70%
FAIL: src/components/enrollment/enrollment-modal.tsx branch coverage 54.41% < floor 60%
FAIL: src/components/enrollment/in-progress-list.tsx line coverage 0.00% < floor 70%
FAIL: src/components/enrollment/in-progress-list.tsx branch coverage 0.00% < floor 60%
FAIL: src/components/enrollment/validation-panel.tsx branch coverage 40.00% < floor 60%
FAIL: src/components/enrollment/webcam-capture-tab.tsx line coverage 47.06% < floor 70%
FAIL: src/components/enrollment/webcam-capture-tab.tsx branch coverage 40.74% < floor 60%
FAIL: src/components/layout/sidebar.tsx line coverage 0.00% < floor 70%
FAIL: src/components/layout/sidebar.tsx branch coverage 0.00% < floor 60%
FAIL: src/components/layout/top-bar.tsx line coverage 0.00% < floor 70%
FAIL: src/components/reports/export-buttons.tsx branch coverage 50.00% < floor 60%
FAIL: src/components/timesheet/novedad-modal.tsx line coverage 0.00% < floor 70%
FAIL: src/components/timesheet/novedad-modal.tsx branch coverage 0.00% < floor 60%
FAIL: src/components/timesheet/timesheet-table.tsx line coverage 0.00% < floor 70%
FAIL: src/components/timesheet/timesheet-table.tsx branch coverage 0.00% < floor 60%
FAIL: src/components/timesheet/week-navigator.tsx line coverage 0.00% < floor 70%
FAIL: src/hooks/use-sse.ts line coverage 0.00% < floor 70%
FAIL: src/hooks/use-sse.ts branch coverage 0.00% < floor 60%
FAIL: src/lib/api.ts line coverage 38.71% < floor 70%
FAIL: src/lib/api.ts branch coverage 50.00% < floor 60%
FAIL: src/lib/face-detection.ts line coverage 0.00% < floor 70%
FAIL: src/lib/face-detection.ts branch coverage 0.00% < floor 60%
FAIL: src/lib/reports/pdf.ts branch coverage 36.36% < floor 60%
FAIL: project-wide branch coverage 44.79% < gate 85%
```

Exit code: 1 (41 file-level fails + 1 project-wide branch fail).

Vitest's own threshold table (from `npx vitest run --coverage`) reported, in addition to the per-file fails above:

```
ERROR: Coverage for lines (51.8%) does not meet global threshold (90%)
ERROR: Coverage for functions (50.53%) does not meet global threshold (90%)
ERROR: Coverage for statements (50.87%) does not meet global threshold (90%)
ERROR: Coverage for branches (44.78%) does not meet global threshold (85%)
ERROR: Coverage for lines (51.8%) does not meet "**/*.{ts,tsx}" threshold (70%)
ERROR: Coverage for functions (50.53%) does not meet "**/*.{ts,tsx}" threshold (70%)
ERROR: Coverage for statements (50.87%) does not meet "**/*.{ts,tsx}" threshold (70%)
ERROR: Coverage for branches (44.78%) does not meet "**/*.{ts,tsx}" threshold (60%)
```

(Note: Vitest's per-file `**/*.{ts,tsx}` threshold check, like the lcov post-processor, only fails the merged total here — Vitest sums LF/LH across the matched files. Both project-wide and per-file gates are wired correctly.)


| File | Line% | Branch% | Func% | Floor miss |
|------|-------|---------|-------|------------|
| src/components/common/access-restricted.tsx | 0.00 | 100.00 | 0.00 | line + func |
| src/components/dashboard/activity-feed.tsx | 0.00 | 0.00 | 0.00 | line + branch + func |
| src/components/dashboard/dept-chart.tsx | 0.00 | 0.00 | 0.00 | line + branch + func |
| src/components/dashboard/kpi-tile.tsx | 0.00 | 0.00 | 0.00 | line + branch + func |
| src/components/dashboard/sse-reconnect-banner.tsx | 0.00 | 0.00 | 0.00 | line + branch + func |
| src/components/devices/command-modal.tsx | 0.00 | 0.00 | 0.00 | line + branch + func |
| src/components/devices/device-table.tsx | 0.00 | 0.00 | 0.00 | line + branch + func |
| src/components/employees/employee-table.tsx | 0.00 | 0.00 | 0.00 | line + branch + func |
| src/components/enrollment/employee-enrollment-picker.tsx | 0.00 | 0.00 | 0.00 | line + branch + func |
| src/components/enrollment/enrollment-modal.tsx | 63.33 | 54.41 | 63.64 | line + branch + func |
| src/components/enrollment/in-progress-list.tsx | 0.00 | 0.00 | 0.00 | line + branch + func |
| src/components/enrollment/validation-panel.tsx | 72.22 | 40.00 | 80.00 | branch |
| src/components/enrollment/webcam-capture-tab.tsx | 47.06 | 40.74 | 37.50 | line + branch + func |
| src/components/layout/sidebar.tsx | 0.00 | 0.00 | 0.00 | line + branch + func |
| src/components/layout/top-bar.tsx | 0.00 | 100.00 | 0.00 | line + func |
| src/components/providers.tsx | 0.00 | 100.00 | 0.00 | line + func |
| src/components/reports/export-buttons.tsx | 100.00 | 50.00 | 100.00 | branch |
| src/components/timesheet/novedad-modal.tsx | 0.00 | 0.00 | 0.00 | line + branch + func |
| src/components/timesheet/timesheet-table.tsx | 0.00 | 0.00 | 0.00 | line + branch + func |
| src/components/timesheet/week-navigator.tsx | 0.00 | 100.00 | 0.00 | line + func |
| src/hooks/use-sse.ts | 0.00 | 0.00 | 0.00 | line + branch + func |
| src/lib/api.ts | 38.71 | 50.00 | 37.50 | line + branch + func |
| src/lib/face-detection.ts | 0.00 | 0.00 | 0.00 | line + branch + func |
| src/lib/reports/pdf.ts | 70.00 | 36.36 | 80.00 | branch |

**Frontend file count below floor: 24**

### Frontend files at or above floor (informational)

| File | Line% | Branch% | Func% |
|------|-------|---------|-------|
| src/components/dashboard/device-banner.tsx | 100.00 | 83.33 | 100.00 |
| src/components/enrollment/kiosk-capture-tab.tsx | 77.78 | 71.15 | 78.26 |
| src/components/enrollment/sync-panel.tsx | 100.00 | 100.00 | 100.00 |
| src/components/enrollment/sync-row.tsx | 81.82 | 70.00 | 83.33 |
| src/components/enrollment/upload-capture-tab.tsx | 96.77 | 77.27 | 83.33 |
| src/components/reports/drill-down-dialog.tsx | 83.33 | 70.00 | 83.33 |
| src/components/reports/filters-bar.tsx | 92.31 | 76.19 | 88.89 |
| src/components/reports/period-picker.tsx | 86.05 | 86.21 | 87.50 |
| src/components/reports/summary-table.tsx | 100.00 | 76.74 | 100.00 |
| src/components/settings/tenant-info-form.tsx | 94.12 | 71.43 | 100.00 |
| src/hooks/use-auth.ts | 100.00 | 100.00 | 100.00 |
| src/lib/format/currency.ts | 100.00 | 100.00 | 100.00 |
| src/lib/kpi-utils.ts | 100.00 | 100.00 | 100.00 |
| src/lib/ring-buffer.ts | 100.00 | 100.00 | 100.00 |
| src/lib/utils.ts | 100.00 | 100.00 | 100.00 |
| src/lib/validations.ts | 84.62 | 71.43 | 66.67 |

## File count and budget

- Backend files below floor: **27**
- Frontend files below floor: **24**
- Total: **51**

> Plan 04 scope cap: if N+M > 15 OR estimated work > 10 hours, Plan 04 STOPS and escalates
> to the user before adding tests. See Plan 04 <scope_cap> block.

**Status: ESCALATION REQUIRED for Plan 04.** With 51 files below floor, the plan-04 work envelope clearly exceeds the 15-file/10-hour cap. The planner / orchestrator must triage:

1. Tightening the include/exclude globs (e.g., excluding `lib/face-detection.ts` if it's a thin wrapper, excluding pure-display layout components like `top-bar.tsx`/`sidebar.tsx`/`providers.tsx` if they have no logic — D-09 allows shells with no logic).
2. Splitting Plan 04 into 04a/04b/04c by subsystem (e.g., 04a = backend handlers + services; 04b = frontend dashboard + tables; 04c = workers + license).
3. Or accepting a phased threshold ramp (e.g., Plan 04 lands per-file ≥70%, project ≥75%; a follow-up phase pushes to ≥90/85).

Plan 04 frontmatter `files_modified` must be revised against this baseline before execution.

## Verification artifacts produced by this run

- `backend/lcov.info` — 479 KB, 61 source files instrumented
- `backend/target/llvm-cov-target/...` — instrumented target dir (gitignored)
- `frontend/coverage/lcov.info` — 29.6 KB, 40 source files instrumented
- `frontend/coverage/index.html` — HTML report (gitignored)
- `frontend/coverage/lcov-report/` — HTML report tree (gitignored)

## Tooling environment notes (for Plan 05 / CI executors)

- **Local executor's machine:** macOS arm64, Homebrew-installed stable rustc 1.93.0, no rustup. cargo-llvm-cov 0.8.5 installed via `cargo install`. LLVM tools (`llvm-cov`, `llvm-profdata`) available via Homebrew `llvm@21` package — set `LLVM_COV` and `LLVM_PROFDATA` to `/opt/homebrew/opt/llvm/bin/llvm-{cov,profdata}` for local runs.
- **`make coverage-backend` failure on local box:** `--branch` flag rejects on stable rustc with "the option `Z` is only accepted on the nightly compiler". The `rust-toolchain.toml` pin to `nightly-2026-04-01` requires rustup to act on it. CI (Plan 05) will install rustup + nightly + llvm-tools-preview and the recipe will succeed there. Local devs without rustup must `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh` before `make coverage-backend`.
- **`make coverage-frontend` ran end-to-end** locally with the v8 coverage provider enforcing both project-wide and per-file thresholds; the failure is *measurement* not *config*.
- **Plan 02's "1 leaky test under nextest" note:** Did not interfere with this run — all 319 tests passed under cargo-llvm-cov nextest. The leaky-warning may have been a transient resource (file descriptor) leak in a background task that didn't reproduce here.
