---
phase: 08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra
plan: 04C
type: execute
wave: 4
depends_on: [08-04B]
files_modified:
  # New frontend test files for 21 components/hooks/lib gap modules.
  # Tests are co-located in `__tests__/` directories adjacent to source per existing repo convention.
  # 21 source modules in this bucket; expected 18-21 new test files (some can share a __tests__/ file).
  - frontend/src/components/dashboard/__tests__/dept-chart.test.tsx
  - frontend/src/components/dashboard/__tests__/kpi-tile.test.tsx
  - frontend/src/components/dashboard/__tests__/sse-reconnect-banner.test.tsx
  - frontend/src/components/devices/__tests__/command-modal.test.tsx
  - frontend/src/components/devices/__tests__/device-table.test.tsx
  - frontend/src/components/employees/__tests__/employee-table.test.tsx
  - frontend/src/components/enrollment/__tests__/employee-enrollment-picker.test.tsx
  - frontend/src/components/enrollment/__tests__/in-progress-list.test.tsx
  - frontend/src/components/enrollment/__tests__/validation-panel.test.tsx
  - frontend/src/components/layout/__tests__/sidebar.test.tsx
  - frontend/src/components/timesheet/__tests__/week-navigator.test.tsx
  - frontend/src/hooks/__tests__/use-sse.test.ts
  - frontend/src/lib/__tests__/api.test.ts
  - frontend/src/lib/__tests__/face-detection.test.ts
  - frontend/src/__tests__/dashboard-activity-feed-extra.test.tsx          # extends existing src/__tests__/activity-feed.test.ts
  - frontend/src/__tests__/timesheet-novedad-modal-extra.test.tsx          # extends existing src/__tests__/novedad-modal.test.tsx
  - frontend/src/__tests__/timesheet-table-extra.test.tsx                  # extends existing src/__tests__/timesheet-table.test.tsx
  - frontend/src/components/enrollment/__tests__/enrollment-modal-extra.test.tsx  # extends existing test
  - frontend/src/components/enrollment/__tests__/webcam-capture-tab-extra.test.tsx # extends existing test
  - frontend/src/components/reports/__tests__/export-buttons-extra.test.tsx       # extends existing test (branch only)
  - frontend/src/lib/reports/__tests__/pdf-extra.test.ts                          # extends existing pdf.test.ts (branch only)
  # Possibly modified ONLY if exclusions are accepted at the human checkpoint (≤3 frontend total beyond Plan 03's 3):
  - frontend/vitest.config.ts
autonomous: false
requirements: [QUALITY-GATE]
must_haves:
  truths:
    - "Every frontend file in this bucket reaches ≥70% line + ≥60% branch coverage when measured by `make coverage-frontend`"
    - "Existing 105 frontend tests still pass: `cd frontend && npx vitest run` exits 0 after additions"
    - "After 04A + 04B + 04C land, `make coverage` exits 0 — both backend and frontend project-wide gates green AND every counted file ≥70/60"
    - "Tests honor UI-SPEC LOCKED RULE: mount existing components AS-IS, no new copy strings, no new design tokens, no modifications to vendored shadcn"
    - "Human checkpoint approved at Task 3 (the single shared checkpoint for the entire 04A + 04B + 04C gap-fill set)"
  artifacts:
    - path: ".planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-03-COVERAGE-BASELINE.md"
      provides: "Read-only INPUT — authoritative gap list. Every test file written corresponds to a row in the frontend gap table (post the 3 D-09 exclusions already wired)."
      contains: "FAIL:"
    - path: ".planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04A-SUMMARY.md"
      provides: "Read-only INPUT — backend domain bucket result"
      contains: "Files closed"
    - path: ".planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04B-SUMMARY.md"
      provides: "Read-only INPUT — backend infrastructure bucket result; backend gate is green"
      contains: "Files closed"
    - path: "frontend/src/<one file per frontend bucket row, ≤21 total>"
      provides: "New frontend component / hook / lib tests; each closes a row in the baseline frontend gap table"
      contains: "describe("
    - path: ".planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04C-SUMMARY.md"
      provides: "Documents which frontend modules were closed, before% → after% per file, exclusions (if any), confirms `make coverage` is green AND captures the human checkpoint approval"
      contains: "Final coverage"
  key_links:
    - from: "make coverage-frontend"
      to: "frontend/vitest.config.ts thresholds + scripts/enforce-coverage-floor.sh against frontend/coverage/lcov.info"
      via: "Makefile invocation"
      pattern: "make coverage-frontend"
    - from: "frontend/src/<new test files>"
      to: "frontend/src/<source modules>"
      via: "@testing-library/react + msw + vitest mocks"
      pattern: "render\\(|screen\\.|userEvent\\."
---

<objective>
Close the **frontend** subset of the Plan 03 baseline gap (21 files, post the 3 D-09 pure-display exclusions already wired in `vitest.config.ts`): dashboard widgets, devices admin tables, employees table, enrollment workflow components, layout sidebar, reports export buttons, timesheet workflow components, the SSE hook, the API interceptor, the face-detection helper, and the report PDF generator. After this plan, every frontend file in this bucket reaches ≥70% line / ≥60% branch and `make coverage-frontend` exits 0. Combined with 04A + 04B's backend gate, the composite `make coverage` is GREEN.

Purpose: This is the THIRD and final sub-plan splitting Plan 04 by subsystem. 04A closed the backend domain bucket. 04B closed the backend infrastructure bucket — backend gate is green going into 04C. 04C closes the frontend bucket AND provides the single human verification checkpoint for the entire gap-fill set. Splitting the human checkpoint across three sub-plans would have produced three context switches; consolidating it here lets the reviewer evaluate the full delta in one pass.

Output: New tests under `frontend/src/`, mostly co-located in `__tests__/` directories adjacent to the source per existing repo convention. The human checkpoint at Task 3 reviews the complete `make coverage` run (both backend halves from 04A + 04B AND the frontend half from 04C), the cumulative exclusions (≤3 backend + ≤3 frontend beyond Plan 03's 3 layout exclusions), and signals approval to proceed to Plan 05 (CI gate).
</objective>

<bucket_definition>
**Source files in this bucket (21 modules — drawn verbatim from `08-03-COVERAGE-BASELINE.md` frontend FAIL list, MINUS the 3 D-09 pure-display exclusions already wired in `vitest.config.ts`):**

| # | Source file | Baseline line% | Baseline branch% | Test target |
|---|---|---|---|---|
| 1 | `src/components/dashboard/activity-feed.tsx` | 0% | 0% | extend `src/__tests__/activity-feed.test.ts` (existing 495B; needs branch coverage) |
| 2 | `src/components/dashboard/dept-chart.tsx` | 0% | 0% | new `dashboard/__tests__/dept-chart.test.tsx` |
| 3 | `src/components/dashboard/kpi-tile.tsx` | 0% | 0% | new `dashboard/__tests__/kpi-tile.test.tsx` |
| 4 | `src/components/dashboard/sse-reconnect-banner.tsx` | 0% | 0% | new `dashboard/__tests__/sse-reconnect-banner.test.tsx` |
| 5 | `src/components/devices/command-modal.tsx` | 0% | 0% | new `devices/__tests__/command-modal.test.tsx` |
| 6 | `src/components/devices/device-table.tsx` | 0% | 0% | new `devices/__tests__/device-table.test.tsx` |
| 7 | `src/components/employees/employee-table.tsx` | 0% | 0% | new `employees/__tests__/employee-table.test.tsx` |
| 8 | `src/components/enrollment/employee-enrollment-picker.tsx` | 0% | 0% | new `enrollment/__tests__/employee-enrollment-picker.test.tsx` |
| 9 | `src/components/enrollment/enrollment-modal.tsx` | 63.33% | 54.41% | extend existing `enrollment/__tests__/enrollment-modal.test.tsx` |
| 10 | `src/components/enrollment/in-progress-list.tsx` | 0% | 0% | new `enrollment/__tests__/in-progress-list.test.tsx` |
| 11 | `src/components/enrollment/validation-panel.tsx` | 72.22% | 40% | new `enrollment/__tests__/validation-panel.test.tsx` (branch only — line is already at 72.22%) |
| 12 | `src/components/enrollment/webcam-capture-tab.tsx` | 47.06% | 40.74% | extend existing `enrollment/__tests__/webcam-capture-tab.test.tsx` |
| 13 | `src/components/layout/sidebar.tsx` | 0% | 0% | new `layout/__tests__/sidebar.test.tsx` |
| 14 | `src/components/reports/export-buttons.tsx` | 100% | 50% | extend existing `reports/__tests__/export-buttons.test.tsx` (branch only) |
| 15 | `src/components/timesheet/novedad-modal.tsx` | 0% | 0% | extend existing `src/__tests__/novedad-modal.test.tsx` |
| 16 | `src/components/timesheet/timesheet-table.tsx` | 0% | 0% | extend existing `src/__tests__/timesheet-table.test.tsx` |
| 17 | `src/components/timesheet/week-navigator.tsx` | 0% | 100% | new `timesheet/__tests__/week-navigator.test.tsx` (line only — branch is 100%) |
| 18 | `src/hooks/use-sse.ts` | 0% | 0% | new `hooks/__tests__/use-sse.test.ts` (msw EventSource fixture) |
| 19 | `src/lib/api.ts` | 38.71% | 50% | new `lib/__tests__/api.test.ts` |
| 20 | `src/lib/face-detection.ts` | 0% | 0% | new `lib/__tests__/face-detection.test.ts` (CANDIDATE for exclusion if WebAssembly init cannot be jsdom-mocked — surfaced at human checkpoint with written rationale) |
| 21 | `src/lib/reports/pdf.ts` | 70% (already at floor) | 36.36% | extend existing `lib/reports/__tests__/pdf.test.ts` (branch only) |

**File count: 21 source modules → expected 18-21 new test files** (a few extension files combine with existing tests; some shells with no logic may be added to vitest exclude with sign-off at the checkpoint).

This exceeds the original Plan 04's 15-file scope cap by 6, justified by:
1. The cap existed for the *combined* Plan 04 (backend + frontend, ~50% context budget). Splitting into 04A/04B/04C distributes work; 04C's ~50% budget can absorb 18-21 test files because most are co-located component tests that follow a uniform mount-and-assert pattern (low context cost per test).
2. 7 of the 21 modules are EXTENSIONS to existing test files (rows 1, 9, 11, 12, 14, 15, 16, 17, 21) — those add a few `it()` blocks rather than full new test scaffolds; they consume less context than full new files.
3. The user approved the 3-way split with the explicit understanding that 04C would carry close to 21 frontend files.
4. `lib/face-detection.ts` is a known exclusion candidate (WebAssembly face-api init cannot be mocked cleanly in jsdom). If accepted at the human checkpoint, the file count drops to 20.

**Files NOT in this bucket** (handled by 04A + 04B): all 27 backend FAIL files.

**Files already excluded** (per Plan 03's `vitest.config.ts` extension under D-09):
- `src/components/providers.tsx` (pure QueryClientProvider wrapper, no logic)
- `src/components/layout/top-bar.tsx` (pure display)
- `src/components/common/access-restricted.tsx` (pure display)
</bucket_definition>

<scope_cap>
**Hard cap on Plan 04C scope:** at most **21 new test files** AND at most **6 hours** of estimated work.

The 21 modules are mostly component tests (uniform pattern: render with props, assert against existing copy/role/aria, simulate one user interaction, assert state change). The setup-heaviest are `use-sse.test.ts` (msw EventSource fixture), `api.test.ts` (mock fetch + interceptor + retry logic), and `face-detection.test.ts` (potential exclusion).

If estimated work exceeds 6 hours OR file count exceeds 21, escalate before continuing.
</scope_cap>

<exclusions_policy>
**No exclusions are pre-approved at planning time** beyond the 3 D-09 layout shells already in `vitest.config.ts`. The 3-exclusion budget for the frontend side is for files surfaced DURING Plan 04C execution (in addition to the 3 baseline exclusions).

**Specifically for files in this bucket:**
- `frontend/src/lib/face-detection.ts` — Known candidate. Per CONTEXT.md and original Plan 04 hedge, this MAY require `face-api.js` WebAssembly init that jsdom cannot simulate. If the executor cannot construct a deterministic test (mocking the WASM module returns), surface at the human checkpoint with the specific reason. If accepted, add to `frontend/vitest.config.ts` `coverage.exclude` and mirror in `CLAUDE.md` (Plan 06).

If the executor genuinely cannot lift any other file above floor (extremely unlikely for the rest of the bucket), surface at the human checkpoint with written rationale per the original Plan 04 policy.
</exclusions_policy>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/STATE.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-CONTEXT.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-RESEARCH.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-PATTERNS.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-UI-SPEC.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-03-COVERAGE-BASELINE.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04A-PLAN.md
@.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04B-PLAN.md

@frontend/vitest.config.ts
@frontend/src/__tests__/setup.ts
@frontend/package.json

<interfaces>
<!-- UI-SPEC LOCKED RULE applies to every test file in this plan: -->
<!--   - Import existing exported component/hook/util AS-IS — do not modify it -->
<!--   - Mount with props the component already accepts; do not add new props for testability -->
<!--   - Assert against EXISTING rendered output (text, role, label, structure) — do not change copy strings, classNames, or styling -->
<!--   - Use existing test infra: @testing-library/react, @testing-library/jest-dom, msw -->
<!--   - Do NOT add new design tokens, copy strings, layouts, or shadcn registry entries -->
<!--   - Do NOT modify components in `src/components/ui/` (vendored shadcn — excluded from coverage anyway) -->
<!--   - If a component is missing an a11y label or a copy string is wrong, FILE A SEPARATE DEFECT -->

<!-- Test file location convention (from existing repo layout): -->
<!--   - Co-located: src/components/<area>/__tests__/<name>.test.tsx -->
<!--     Examples: src/components/enrollment/__tests__/sync-panel.test.tsx (existing) -->
<!--   - Top-level: src/__tests__/<name>.test.tsx (some early dashboard/timesheet tests use this) -->
<!--     Examples: src/__tests__/activity-feed.test.ts, src/__tests__/novedad-modal.test.tsx -->
<!--   New tests SHOULD use co-located __tests__/ directories; extensions to existing top-level tests stay top-level. -->

<!-- Existing test infra is wired in src/__tests__/setup.ts (35B — minimal, likely just imports @testing-library/jest-dom). -->
<!-- Vitest config (frontend/vitest.config.ts) is already correct per Plan 03 — do NOT change unless adding an approved exclusion. -->

<!-- Existing tests provide patterns: -->
<!--   - src/components/enrollment/__tests__/sync-panel.test.tsx (currently 100% coverage — gold standard mount-and-assert) -->
<!--   - src/components/enrollment/__tests__/upload-capture-tab.test.tsx (96.77% — file upload + error branch coverage) -->
<!--   - src/components/reports/__tests__/period-picker.test.tsx (86% — date picker interaction) -->
<!--   - src/components/reports/__tests__/summary-table.test.tsx (100% — data table render) -->
<!-- New tests SHOULD mirror these patterns. -->

<!-- Per security threat model: tests for hooks/use-sse.ts and lib/api.ts MUST cover the auth-error path
     (401 → token refresh → retry) — negative-path coverage is itself a security control (T-08-12). -->
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Frontend components (dashboard + devices + employees + enrollment + layout + timesheet + reports) — gap-fill</name>
  <files>
    frontend/src/__tests__/dashboard-activity-feed-extra.test.tsx (extend existing activity-feed.test.ts — branch coverage),
    frontend/src/components/dashboard/__tests__/dept-chart.test.tsx (new — 0% → ≥70/60),
    frontend/src/components/dashboard/__tests__/kpi-tile.test.tsx (new — 0% → ≥70/60),
    frontend/src/components/dashboard/__tests__/sse-reconnect-banner.test.tsx (new — 0% → ≥70/60),
    frontend/src/components/devices/__tests__/command-modal.test.tsx (new — 0% → ≥70/60),
    frontend/src/components/devices/__tests__/device-table.test.tsx (new — 0% → ≥70/60),
    frontend/src/components/employees/__tests__/employee-table.test.tsx (new — 0% → ≥70/60),
    frontend/src/components/enrollment/__tests__/employee-enrollment-picker.test.tsx (new — 0% → ≥70/60),
    frontend/src/components/enrollment/__tests__/enrollment-modal-extra.test.tsx (extend — 63.33%/54.41% → ≥70/60),
    frontend/src/components/enrollment/__tests__/in-progress-list.test.tsx (new — 0% → ≥70/60),
    frontend/src/components/enrollment/__tests__/validation-panel.test.tsx (new — branch 40% → ≥60),
    frontend/src/components/enrollment/__tests__/webcam-capture-tab-extra.test.tsx (extend — 47.06%/40.74% → ≥70/60),
    frontend/src/components/layout/__tests__/sidebar.test.tsx (new — 0% → ≥70/60),
    frontend/src/components/reports/__tests__/export-buttons-extra.test.tsx (extend — branch 50% → ≥60),
    frontend/src/__tests__/timesheet-novedad-modal-extra.test.tsx (extend — 0% → ≥70/60),
    frontend/src/__tests__/timesheet-table-extra.test.tsx (extend — 0% → ≥70/60),
    frontend/src/components/timesheet/__tests__/week-navigator.test.tsx (new — 0% → ≥70 line; branch already 100%)
  </files>
  <read_first>
    - .planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-03-COVERAGE-BASELINE.md (verify 17 component module rows still appear)
    - .planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-UI-SPEC.md (LOCKED RULE — re-read before every test file)
    - .planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04A-SUMMARY.md (backend domain delta context)
    - .planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04B-SUMMARY.md (backend gate green confirmation)
    - frontend/src/__tests__/setup.ts (existing setup — extend if needed, do not recreate)
    - frontend/src/components/enrollment/__tests__/sync-panel.test.tsx (gold-standard mount-and-assert)
    - frontend/src/components/enrollment/__tests__/upload-capture-tab.test.tsx (file upload + error branch pattern)
    - frontend/src/components/reports/__tests__/period-picker.test.tsx (date-picker interaction pattern)
    - frontend/src/components/reports/__tests__/summary-table.test.tsx (data-table render pattern)
    - For each gap target, READ THE SOURCE FILE FIRST to identify props, hooks, rendered strings, and branches. UI-SPEC requires assertions against EXISTING rendered output — never fabricate copy.
  </read_first>
  <coverage_discipline>
    UI-SPEC LOCKED RULE applies. Co-locate tests in `__tests__/` directories. Do NOT modify `src/components/ui/` (vendored shadcn) — already excluded.

    **Per-source-file test design** (CONFIRM against baseline before writing each):

    1. `dashboard/activity-feed.tsx` (extend existing test). Branch coverage gap. Cover:
       - Empty list: `data: []` → renders empty-state (existing copy).
       - Loading state: `isLoading: true` → renders skeleton.
       - Error state: `error: ...` → renders error message.
       - Populated list: `data: [event1, event2]` → renders rows in correct order.

    2. `dashboard/dept-chart.tsx` (new). Renders a Recharts pie/bar with department breakdown. Cover:
       - Renders chart with seeded data.
       - Empty state.
       - Tooltip / legend interaction (if exposed).

    3. `dashboard/kpi-tile.tsx` (new). Tile component receiving `label`, `value`, `delta` props. Cover:
       - Renders label + value.
       - Renders positive delta with up-arrow / negative with down-arrow (existing copy + icon name).
       - Loading state: shows skeleton.

    4. `dashboard/sse-reconnect-banner.tsx` (new). Reads `useSse` connection state; renders banner only when disconnected. Cover:
       - Connected state → banner is null/hidden.
       - Disconnected state → banner renders with retry button.
       - Retry button click → invokes the reconnect callback (mock the hook).

    5. `devices/command-modal.tsx` (new). Modal that issues device commands (door-open, restart, sync). Cover:
       - Modal opens when `isOpen=true`.
       - Each command button click → invokes the command-mutation hook with correct payload (mock `useMutation`).
       - Submitting in flight: button disabled.
       - Error state: error message renders.

    6. `devices/device-table.tsx` (new). TanStack Table render. Cover:
       - Renders header columns.
       - Renders rows from fixture data.
       - Sort interaction (click column header).
       - Row action (open command modal).
       - Empty state.

    7. `employees/employee-table.tsx` (new). Mirror of device-table pattern. Cover:
       - Renders header.
       - Filter input updates rendered rows.
       - Pagination (if exposed in props).
       - Row click → navigates / opens detail (mock router).

    8. `enrollment/employee-enrollment-picker.tsx` (new). Searchable employee picker (Combobox style). Cover:
       - Renders list of employees from props.
       - Filter input narrows the list.
       - Selecting an employee invokes the onSelect callback.

    9. `enrollment/enrollment-modal.tsx` (extend). Existing test covers happy path; gap is in error/cancel branches. Cover:
       - Cancel button click → invokes onClose without submitting.
       - Tab switching (kiosk → upload → webcam).
       - Validation-fail state: existing error message renders.

    10. `enrollment/in-progress-list.tsx` (new). List of in-flight enrollments with retry/cancel actions. Cover:
        - Empty state.
        - Renders rows with employee name + status.
        - Retry button click → invokes retry mutation.
        - Cancel button click → invokes cancel mutation.

    11. `enrollment/validation-panel.tsx` (new — branch only; line is at 72.22%). Cover the validation result branches:
        - Pass result → renders success icon + green text.
        - Fail result with reason `BlurDetected` / `MultipleFaces` / `NoFace` → each renders the matching message (existing copy from the source file).
        - Loading: renders spinner.

    12. `enrollment/webcam-capture-tab.tsx` (extend). Gap is in camera-permission-denied + capture-error branches. Cover:
        - getUserMedia rejected → renders permission-denied state.
        - Capture click after permission granted → image preview renders.
        - Submit click → invokes the capture-submit mutation.

    13. `layout/sidebar.tsx` (new). Nav component with role-based menu items. Cover:
        - Admin role: full menu list renders.
        - Supervisor role: subset of menu (per existing role rules in the source).
        - Viewer role: minimal menu.
        - Active route: corresponding nav item has the active style class (existing className from source).

    14. `reports/export-buttons.tsx` (extend — branch only; line is 100%). Gap is the disabled-state and error-toast branches. Cover:
        - Empty data → button disabled.
        - Click XLSX → invokes xlsx export (mock `xlsx.writeFile`).
        - Click PDF → invokes pdf generator.
        - Export failure → error toast renders (existing copy).

    15. `timesheet/novedad-modal.tsx` (extend existing top-level test). Cover the form-state branches:
        - Submit valid data → invokes mutation.
        - Submit invalid (missing required field) → renders validation error (existing Zod error message).
        - Cancel → invokes onClose.

    16. `timesheet/timesheet-table.tsx` (extend existing top-level test). Cover:
        - Renders week's daily rows.
        - Override cell click → opens novedad-modal.
        - Empty state.

    17. `timesheet/week-navigator.tsx` (new — line only; branch is 100%). Cover:
        - Initial render shows current week range (existing date format).
        - Click "next week" → range advances 7 days.
        - Click "previous week" → range moves back 7 days.

    **After every ~4 component tests, re-run `make coverage-frontend`** and inspect both the threshold table and the lcov post-processor output. Stop adding tests once each row is at floor.

    **Self-review for UI-SPEC compliance** before declaring each test done:
    - `grep "<asserted-string>" <source-file>` returns at least 1 match for each asserted string in the new test (verifies UI-SPEC LOCKED RULE: assertions on existing copy).
    - Test file does NOT modify `src/components/ui/` (verify with `git diff --name-only frontend/src/components/ui/` empty).
    - `git diff frontend/src/app/globals.css` empty (no new design tokens).
    - `git diff frontend/components.json` empty (no new shadcn registry entries).
  </coverage_discipline>
  <action>
    Execute the per-source-file test design listed in <coverage_discipline> for the 17 component modules in Task 1.

    **Step 1 — re-read the baseline.** Confirm 17 rows still in the frontend gap table.

    **Step 2 — for each component, READ THE SOURCE FIRST.** Identify props, hooks consumed, and the exact rendered strings/roles. UI-SPEC LOCKED RULE applies — assert against existing output only.

    **Step 3 — write the test, then run.**
    ```
    cd frontend && npx vitest run --coverage <test-file>
    ```
    Verify the source file moves above floor in the per-file table.

    **Step 4 — exclusion handling.** No component in Task 1 is a planning-time pre-approved exclusion. If a component renders only via `next/font` + a third-party shim that jsdom cannot simulate, surface at Task 3 (the human checkpoint) with the specific reason.

    Per UI-SPEC: every test file MUST satisfy the LOCKED RULE checklist. Self-verify per the bullets in <coverage_discipline>.

    Per security threat model: components that surface auth/role-gated UI (sidebar.tsx is the primary one in Task 1) MUST have tests for each role's exposed menu — negative-role coverage matters.
  </action>
  <verify>
    <automated>cd frontend && npx vitest run > /tmp/cov-04c-task1.log 2>&1; ec=$?; tail -10 /tmp/cov-04c-task1.log; if [ $ec -ne 0 ]; then exit $ec; fi; cd /Users/gerswin/Proyectos/cronometrix && (make coverage-frontend > /tmp/cov-04c-task1-gate.log 2>&1 || true); echo "--- last 30 ---"; tail -30 /tmp/cov-04c-task1-gate.log; awk '/^FAIL:.*src\/components\/(dashboard|devices|employees|enrollment|layout|reports|timesheet)/' /tmp/cov-04c-task1-gate.log; echo "--- end ---"</automated>
  </verify>
  <acceptance_criteria>
    - `cd frontend && npx vitest run` exits 0 (no regression from new tests).
    - Every Task 1 source file (17 listed in <files>) is at or above ≥70% line AND ≥60% branch in the per-file output of `make coverage-frontend`.
    - HTML report renders: `frontend/coverage/index.html` exists.
    - Every new test file uses ONLY existing copy strings — verify by sampling 3 random new test files: each asserted text/role/aria string MUST appear in the source component being tested.
    - No file under `frontend/src/components/ui/` was modified (`git diff --name-only frontend/src/components/ui/` empty).
    - No new design tokens added (`git diff frontend/src/app/globals.css` empty).
    - No new shadcn registry entries (`git diff frontend/components.json` empty).
    - `setup.ts` not modified beyond minimal extensions if required (e.g., adding msw setup); document any setup change in 04C-SUMMARY.
    - No new flaky tests: `cd frontend && npx vitest run` exits 0 across 3 successive runs.
  </acceptance_criteria>
  <done>
    All 17 Task 1 component modules ≥70% line + ≥60% branch. Tests honor UI-SPEC LOCKED RULE. No new copy/tokens/registry entries.
  </done>
</task>

<task type="auto">
  <name>Task 2: Frontend hooks + lib (use-sse, api, face-detection, reports/pdf) — gap-fill</name>
  <files>
    frontend/src/hooks/__tests__/use-sse.test.ts (new — 0% → ≥70/60; msw EventSource fixture),
    frontend/src/lib/__tests__/api.test.ts (new — 38.71%/50% → ≥70/60),
    frontend/src/lib/__tests__/face-detection.test.ts (new — 0% → ≥70/60 OR exclusion candidate at Task 3 checkpoint),
    frontend/src/lib/reports/__tests__/pdf-extra.test.ts (extend existing pdf.test.ts — branch 36.36% → ≥60)
  </files>
  <read_first>
    - .planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-03-COVERAGE-BASELINE.md (verify 4 module rows)
    - frontend/src/__tests__/setup.ts
    - frontend/src/lib/reports/__tests__/pdf.test.ts (existing — extend pattern)
    - frontend/src/lib/format/__tests__/currency.test.ts (existing pure-fn pattern reference — 100% coverage)
    - frontend/package.json (verify msw is in deps; if not, file an issue and ask reviewer)
    - frontend/src/hooks/use-sse.ts (read whole — 0%; identify EventSource subscription pattern, reconnect logic, onMessage handler)
    - frontend/src/lib/api.ts (read whole — 38.71%; identify the fetch wrapper, the 401 → refresh-token → retry flow, 5xx pass-through)
    - frontend/src/lib/face-detection.ts (read whole — 0%; identify whether it imports `face-api.js` or another WASM module, and whether the import is at module level or behind a lazy loader)
    - frontend/src/lib/reports/pdf.ts (find the 63.64% uncovered branches — likely the empty-data branch + the multi-page pagination branch in jspdf-autotable)
  </read_first>
  <coverage_discipline>
    Same UI-SPEC + co-location rules. Specific patterns:

    1. `hooks/use-sse.ts` (new test). Use msw (Mock Service Worker) — verify it's in package.json dev-deps; if not, file a defect and surface at Task 3 checkpoint. Cover:
       - Initial connection: EventSource opens to the configured endpoint; assert `onopen` callback fires.
       - Message receipt: msw publishes a message → hook's `onMessage` callback receives it; assert state update.
       - Reconnect on disconnect: simulate close, advance timers (`vi.useFakeTimers()`), assert reconnect attempt fires after the configured backoff.
       - Cleanup on unmount: render with `renderHook`, unmount, assert EventSource.close() was called.
       - Error path: server returns 4xx/5xx on connect → onError fires.

    2. `lib/api.ts` (new test). Mock fetch globally via `vi.spyOn(global, 'fetch')`. Cover:
       - 200 happy path: returns parsed JSON.
       - 401 path: first call returns 401, refresh-token call returns 200 with new token, original call retried with new Bearer header → assertion: fetch called 3 times in correct order.
       - 401 then refresh fails → propagates the original 401.
       - 5xx pass-through: NO refresh attempted, error bubbles.
       - Network error: rejected promise propagates.
       - Bearer header attached on every request.

    3. `lib/face-detection.ts` (new test OR exclusion candidate). FIRST attempt to test:
       - If the module exports pure helpers (e.g., `cropImageToFace(blob, bbox)`, `isFacePositionValid(landmarks)`) that don't require WASM init, write tests for those.
       - If face-api.js init is behind a lazy loader (`async function loadModels()`), mock the loader and test the post-init API.
       - If the module is a thin wrapper around WASM functions with no testable pure logic AND mocking the WASM at module-level fails in jsdom, surface at Task 3 with the specific reason: "frontend/src/lib/face-detection.ts requires face-api.js WebAssembly init that jsdom cannot simulate; module-level mock attempts produced [specific error]; recommend exclusion."

    4. `lib/reports/pdf.ts` (extend existing test). Branch coverage gap (36.36% → ≥60). Find the uncovered branches:
       - Empty-data branch: `generateReportPdf({ rows: [] })` → returns a PDF with empty-state copy (existing string).
       - Multi-page: > N rows → autoTable splits into multiple pages; assert `pdf.getNumberOfPages()` > 1.
       - Date formatting boundary: localized currency / date formatting per memory's Venezuela `America/Caracas` rule.
  </coverage_discipline>
  <action>
    Execute the per-source-file test design for the 4 hooks/lib modules.

    **Step 1 — re-read the baseline.**

    **Step 2 — verify msw in dev-deps.** If not present, surface at Task 3.

    **Step 3 — for each module, READ THE SOURCE FIRST.**

    **Step 4 — run the suite per module.**

    **Step 5 — `face-detection.ts` decision point:** First attempt to test. If unsuccessful, surface at Task 3 with concrete reason. Do NOT silently exclude.

    Per security threat model: `lib/api.ts` 401 → refresh → retry test is itself a security control coverage (T-08-12). MUST cover both successful refresh AND failed refresh.
  </action>
  <verify>
    <automated>cd frontend && npx vitest run > /tmp/cov-04c-task2.log 2>&1; ec=$?; tail -10 /tmp/cov-04c-task2.log; if [ $ec -ne 0 ]; then exit $ec; fi; cd /Users/gerswin/Proyectos/cronometrix && (make coverage-frontend > /tmp/cov-04c-task2-gate.log 2>&1 || true); echo "--- last 30 ---"; tail -30 /tmp/cov-04c-task2-gate.log; awk '/^FAIL:.*(hooks\/use-sse|lib\/(api|face-detection)|lib\/reports\/pdf)/' /tmp/cov-04c-task2-gate.log; echo "--- end ---"</automated>
  </verify>
  <acceptance_criteria>
    - `cd frontend && npx vitest run` exits 0.
    - Every Task 2 source file is at or above ≥70% line + ≥60% branch IN OR is added to `vitest.config.ts` `coverage.exclude` after Task 3 sign-off (`face-detection.ts` is the only candidate).
    - msw used per RESEARCH pattern for `use-sse.test.ts`; no manual EventSource mocking.
    - `lib/api.ts` test covers BOTH successful 401 → refresh → retry AND failed-refresh path.
    - **Cumulative milestone:** after this task, `make coverage-frontend` should exit 0 (modulo any Task-3-approved exclusions). Frontend project-wide gate ≥90/85/90/90; per-file floor met across all counted frontend code.
    - No new flaky tests: 3× successive runs all green.
  </acceptance_criteria>
  <done>
    4 frontend hooks/lib modules ≥70/60 OR `face-detection.ts` surfaced for exclusion at Task 3. After this task, `make coverage` (composite) is ready for the human checkpoint.
  </done>
</task>

<task type="checkpoint:human-verify" gate="blocking">
  <name>Task 3: Phase 8 review checkpoint — confirm `make coverage` is GREEN and exclusions across 04A + 04B + 04C are reasonable</name>
  <action>
    Pause execution and present the verification steps below to the user. Do not proceed to Plan 05 (CI gate) until the user types "approved". The action for the executor is: (1) ensure `make coverage` was run end-to-end immediately before reaching this checkpoint and the exit code captured; (2) ensure 04A-SUMMARY, 04B-SUMMARY, and 04C-SUMMARY all exist and list the closed files plus any exclusions with justifications; (3) if any exclusion candidate was surfaced (most likely `face-detection.ts` or `license/fingerprint.rs`), include it explicitly in the what-built / how-to-verify sections; (4) display the what-built / how-to-verify content to the user verbatim and wait for the resume-signal.
  </action>
  <what-built>
    Plans 01-04C collectively delivered the Phase 8 quality gate:
    - **Plan 01:** AppState `Paths` injection (5 source-side call-site updates).
    - **Plan 02:** 16 backend test files migrated off `*RootGuard` to `test_state_with_tmpdir` + 3-arg `common::test_state` signature.
    - **Plan 03:** Vitest coverage thresholds + Makefile + `scripts/enforce-coverage-floor.sh` + `rust-toolchain.toml` + the COVERAGE-BASELINE.md gap map.
    - **Plan 04A:** 16 backend domain modules ≥70% line (auth, calc, config, daily_records, departments, employees, devices/models, events, isapi/client, leaves, state/paths).
    - **Plan 04B:** 11 backend infrastructure modules ≥70% line (enrollments, license, recompute, supervisor/watchdog, workers/{backfill, purge}). Backend gate green.
    - **Plan 04C:** 21 frontend modules ≥70% line + ≥60% branch (dashboard widgets, devices admin tables, employees table, enrollment workflow, layout sidebar, timesheet workflow, reports export, use-sse hook, lib/api interceptor, lib/face-detection [or exclusion], lib/reports/pdf).
  </what-built>
  <how-to-verify>
    1. From repo root, run: `make coverage`
    2. Confirm both jobs exit 0 (final line of output reads "All coverage gates passed.").
    3. Open `backend/target/llvm-cov/html/index.html` — confirm project numbers ≥90% line. (Branch% will show ≥85% only under nightly toolchain; if local is on stable, document that branch verification is deferred to Plan 05 CI per Plan 03's known caveat.)
    4. Open `frontend/coverage/index.html` — confirm project numbers ≥90% line + ≥85% branch + ≥90% functions + ≥90% statements.
    5. Read the three SUMMARYs (`08-04A-SUMMARY.md`, `08-04B-SUMMARY.md`, `08-04C-SUMMARY.md`). Review the **Exclusions** section in each. For each exclusion (≤3 backend total + ≤3 frontend total beyond Plan 03's 3 layout exclusions), decide:
       - Is the justification specific and reasonable? (NOT "hard to test"; must cite a concrete reason like "requires face-api.js WASM init that jsdom cannot simulate" or "requires real ISAPI device traffic that wiremock cannot replicate accurately".)
       - Is the file genuinely uncoverable in this phase, or could the test be deferred to a quick-task instead of excluded?
    6. Read each SUMMARY's "Files closed" list. Confirm the count matches the bucket size:
       - 04A: 16 backend domain modules listed.
       - 04B: 11 backend infrastructure modules listed.
       - 04C: 21 frontend modules listed (or 20 if face-detection excluded).
    7. Run `cd backend && cargo nextest run` and `cd frontend && npx vitest run` — both exit 0 with no flaky output across 2 runs.
    8. Verify UI-SPEC compliance for 04C (sample 3 random new frontend test files):
       - `grep "<asserted-string>" <source-component>` returns ≥1 match for each asserted text/role.
       - `git diff frontend/src/components/ui/` empty.
       - `git diff frontend/src/app/globals.css` empty.
       - `git diff frontend/components.json` empty.
  </how-to-verify>
  <resume-signal>
    Type "approved" if all eight checks pass and exclusions are reasonable. If exclusions look excessive (>3 backend total, >3 frontend total beyond Plan 03's 3, or any look like "we got bored"), respond with "rework: <which exclusions to convert into tests>". If an exclusion was added without surfacing it here (silent exclusion), respond "rework: undo silent exclusion of <file>".
  </resume-signal>
</task>

</tasks>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| test → coverage report | Tests are local code; coverage report is local artifact; no untrusted data. |
| msw → frontend test | MSW intercepts fetches in-process; no real network. Used in `use-sse.test.ts` and may be used in `api.test.ts` for the refresh-token flow. |
| jsdom → DOM-rendered components | jsdom is the canonical jsdom — known limitations (no WebAssembly bindings, no `getUserMedia` without polyfill); tests respect these limits and mock at the module boundary. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-08-11C | Tampering | Coverage exclusion abuse (frontend bucket) | mitigate | No silent exclusions. Each exclusion candidate surfaces at Task 3 checkpoint with specific written rationale. ≤3 frontend total beyond Plan 03's 3 layout exclusions. Reviewer rejects "we got bored" rationales explicitly per Task 3 resume-signal. |
| T-08-12C | Repudiation | Negative-path coverage of API auth flow | mitigate | `lib/api.ts` test MUST cover both 401 → refresh → retry success AND 401 → refresh-fail → propagate. `hooks/use-sse.ts` test covers reconnect on disconnect. Sidebar role-based gating tests cover negative-role paths. Each is a security control via test coverage. |
| T-08-13C | Information Disclosure | Test fixtures containing PII | accept | All new fixtures use synthetic data (UUIDs, fake employee names like "Test User One", no real PII). Existing repo policy applies. |
| T-08-14C | Tampering | UI-SPEC LOCKED RULE compliance | mitigate | Self-review checklist in Task 1's <coverage_discipline> + Task 3 sample-verification of 3 random new test files. `git diff` checks on `components/ui/`, `globals.css`, `components.json` confirm no shadcn / tokens / registry pollution. |
| T-08-15C | Spoofing | msw-mocked auth flow vs real auth | accept | msw mocks are explicitly test-only; bundled into the `__tests__/` setup. No production code path reads msw. Negative-test: ensure msw setup is gated by `process.env.NODE_ENV === 'test'` if exposed at module level (verify in Task 1 if any setup change is required). |
</threat_model>

<verification>
1. `cd frontend && npx vitest run` exits 0 (no regression from 04C's new tests).
2. `make coverage-frontend` exits 0 — frontend gate green: project ≥90/85/90/90 + every counted file ≥70/60/70/70.
3. `make coverage` (composite, requires 04A + 04B already landed) exits 0.
4. HTML report renders: `frontend/coverage/index.html` exists.
5. UI-SPEC compliance verified per Task 1 self-review + Task 3 sample verification.
6. No new flaky tests: `cd frontend && npx vitest run` exits 0 across 3 successive runs.
7. Any exclusion across 04A + 04B + 04C is reflected in the appropriate config file (Makefile `--ignore-filename-regex` for backend; `frontend/vitest.config.ts` `coverage.exclude` for frontend) AND in the corresponding SUMMARY.
8. Human checkpoint approved (Task 3) — the single signal that all three sub-plans landed correctly.

This is the green-light signal for Plan 05 (CI gate) to add `.github/workflows/ci.yml`.
</verification>

<success_criteria>
- All 21 frontend modules in this bucket ≥70% line + ≥60% branch (or one accepted exclusion: `face-detection.ts`).
- Cumulative effect across 04A + 04B + 04C: `make coverage` exits 0 — both backend and frontend gates green.
- Test additions follow existing patterns (no new test framework; existing msw + @testing-library/react + @testing-library/jest-dom).
- UI-SPEC LOCKED RULE honored on every test file (assertions against existing copy/role/aria; no `components/ui/` modifications; no new design tokens; no new shadcn registry entries).
- Exclusions ≤3 frontend (beyond Plan 03's 3 layout exclusions) + ≤3 backend (across 04A+04B); each justified in its plan's SUMMARY.
- Human checkpoint approved.
</success_criteria>

<output>
After completion, create `.planning/phases/08-test-coverage-quality-gate-reach-90-line-coverage-and-85-bra/08-04C-SUMMARY.md` with:
- Files closed (path + before% → after%)
- Test files added (path list — actual files, including which extended existing tests vs which are new)
- Patterns established (msw EventSource fixture, jsdom getUserMedia handling, Recharts test pattern)
- Any files surfaced for exclusion at the Task 3 checkpoint (with the specific written rationale and the reviewer's decision)
- Final per-file frontend numbers for the 21 (or 20) bucket files
- Final project-wide numbers for both backend (cumulative 04A+04B+04C effect) AND frontend
- Confirmation: `make coverage` exits 0
- Note any local-vs-CI toolchain caveats (e.g., backend branch% deferred to Plan 05 nightly per Plan 03 caveat)
- The Task 3 reviewer's signal (approved / rework) preserved verbatim
</output>
