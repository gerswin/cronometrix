# Task 5 report: frontend per-file coverage floor

## What was built

- `scripts/enforce-frontend-file-floor.mjs` (new): reads
  `frontend/coverage/coverage-summary.json` (emitted by Vitest's
  `json-summary` reporter) after the Vitest run finishes, and fails per file
  below 70/60/70/70 (lines/branches/functions/statements), printing one
  `FAIL:` line per offending metric — same shape as
  `scripts/enforce-coverage-floor.sh` on the backend side. Exit 1 on any
  violation or structural error (missing artifact, unreadable config, zero
  governed files); exit 0 with a `PASS ...` line otherwise.
- Governed files = `vitest.config.ts`'s `coverage.include` minus
  `coverage.exclude`. These are **not duplicated** in the script — it loads
  them at runtime via `vite.loadConfigFromFile()` against `vitest.config.ts`
  (`vite` is a direct dependency of the installed `vitest` package, and
  `picomatch`, used for the glob matching, is a direct dependency of `vite`
  itself — both guaranteed present after `npm ci`, resolved via
  `createRequire` against `frontend/package.json` so they work regardless of
  the script's own location or invocation cwd). If that loader ever breaks,
  the script fails loudly rather than silently reusing a stale copy of the
  globs.
- `frontend/vitest.config.ts`: removed the glob-keyed `'**/*.{ts,tsx}': {...}`
  threshold block and replaced it with a comment explaining why it never
  worked and where the real per-file floor now lives. Added a short note
  above `exclude` pointing at the enforcer so a future editor of these globs
  knows a second consumer reads them.
- `Makefile`: `coverage-frontend` now runs
  `node scripts/enforce-frontend-file-floor.mjs` right after
  `vitest run --coverage`.
- `.github/workflows/ci.yml`: the `Frontend Coverage` job's `working-directory:
  frontend` default means the equivalent invocation is
  `node ../scripts/enforce-frontend-file-floor.mjs`, added as its own step
  right after the Vitest step, so a failure surfaces with its own red step
  name in the Actions UI.

## Step 2 — proving the enforcer measures something (first run, before any new tests)

```
FAIL: src/components/timesheet/leave-row-actions.tsx lines coverage 30.00% < floor 70%
FAIL: src/components/timesheet/leave-row-actions.tsx branches coverage 26.31% < floor 60%
FAIL: src/components/timesheet/leave-row-actions.tsx functions coverage 8.33% < floor 70%
FAIL: src/components/timesheet/leave-row-actions.tsx statements coverage 26.66% < floor 70%
FAIL: src/lib/format/datetime.ts lines coverage 53.84% < floor 70%
FAIL: src/lib/format/datetime.ts functions coverage 66.66% < floor 70%
FAIL: src/lib/format/datetime.ts statements coverage 56.25% < floor 70%
FAIL: 7 per-file coverage floor violation(s) across 61 governed file(s) (floor: lines=70 branches=60 functions=70 statements=70)
```
Exit code 1. This names exactly the two files the branch review flagged
(`timesheet/leave-row-actions.tsx` — the brief's "row-actions.tsx" — and
`lib/format/datetime.ts`), confirming the old glob-keyed threshold really
was silently passing files this far under the intended floor. Meanwhile
`vitest run --coverage` on its own exited 0 the whole time — that's the
defect, reproduced live, not asserted from the brief's description.

One false positive surfaced on this first run and was fixed before treating
the enforcer as correct: `src/hooks/use-auth.ts` (a one-line re-export
barrel, `export { useAuth } from '@/contexts/auth-context'`) has zero
executable statements, so Vitest's V8 `json-summary` reporter emits
`{ total: 0, pct: 0 }` for every metric — "nothing to cover", not "0%
covered". The enforcer now treats a zero-total metric as vacuously passing
(there is no test that could ever raise a 0/0 metric above a percentage
floor, so failing it would be a permanent, unfixable red).

## Step 3 — pre-existing offenders: raised, not excluded

Both files were fixed by **raising coverage**, not by excluding them:

- **`frontend/src/components/timesheet/leave-row-actions.tsx`** — had zero
  dedicated tests; its only coverage came incidentally from
  `TimesheetTable`'s tests, which never hover/click into a row's leave
  actions. Added
  `frontend/src/components/timesheet/__tests__/leave-row-actions.test.tsx`
  (18 tests) covering the lazy-fetch-on-hover gate, the evidence-download
  success/404/other-error/in-flight-disabled branches, the admin-only
  cancel-button visibility (including the cancelled-leave and non-admin
  cases), and the cancel-mutation success/409-conflict/other-error toasts.
  Result: 91.11/73.68/91.66/97.5 (stmts/branch/func/lines) — clear of floor
  on all four metrics.
- **`frontend/src/lib/format/datetime.ts`** — had zero dedicated tests
  either; coverage came incidentally from components that render formatted
  timestamps. Added
  `frontend/src/lib/format/__tests__/datetime.test.ts` (11 tests) covering
  all three exported formatters (`fmtTime`, `fmtDateTime`, `fmtDate`)
  against a fixed UTC instant plus the shared null/undefined short-circuit.
  Result: 81.25/100/100/76.92 — clear of floor. (The `catch` blocks in all
  three functions stay uncovered: `new Date(x).toLocaleTimeString(...)`
  does not throw on an Invalid Date in this runtime — it returns the string
  `"Invalid Date"` — so those `catch` arms appear to be defensive dead code
  reachable only by pathological non-string inputs the TS signature already
  excludes. Confirmed this in a scratch `node -e` before deciding not to
  chase it further.)

**Why raise instead of exclude:** the project's exclusion policy
(`CLAUDE.md` → Test Coverage → Exclusion policy) caps frontend exclusions at
3 without an explicit re-discussion, and the frontend side already has
exactly 3 (`providers.tsx`, `top-bar.tsx`, `access-restricted.tsx`, all
D-09) plus the categorical `src/components/ui/**` vendored-code exclusion.
Adding either of these two files would have both required a fresh policy
discussion AND been the wrong call substantively — neither file is a
pure-display wrapper or vendored code; both have real, previously-untested
branching logic (an evidence-download error-status switch, a cancel-mutation
conflict-status switch, three date formatters with a shared short-circuit).
`CLAUDE.md`'s exclusion table was left unchanged — no new rows were needed.

## Step 5 — full regression cycle

Renamed `frontend/src/lib/format/__tests__/datetime.test.ts` out of the
tree (moved to a scratch path, not deleted) to simulate "comment out a
file's tests," reran `vitest run --coverage` + the enforcer:

```
lib/format        |      75 |    89.47 |   83.33 |   72.72 |
  datetime.ts      |   56.25 |    66.66 |   66.66 |   53.84 | 15,32-46
...
=== ENFORCER ===
FAIL: src/lib/format/datetime.ts lines coverage 53.84% < floor 70%
FAIL: src/lib/format/datetime.ts functions coverage 66.66% < floor 70%
FAIL: src/lib/format/datetime.ts statements coverage 56.25% < floor 70%
FAIL: 3 per-file coverage floor violation(s) across 61 governed file(s) (floor: lines=70 branches=60 functions=70 statements=70)
EXIT: 1
```
(Note `vitest run --coverage` itself still exited 0 here — the project-wide
gate alone cannot see this regression; only the new enforcer catches it.)

Restored the file, reran both:

```
lib/format        |   89.28 |      100 |     100 |   86.36 |
  datetime.ts      |   81.25 |      100 |     100 |   76.92 | 15,32,46
...
=== ENFORCER ===
PASS frontend-file-floor governed=61 floor=lines70/branches60/functions70/statements70
EXIT: 0
```

The gate fails naming the regressed file and only that file, and passes
clean once restored.

## Full-suite confirmation (final state)

```
Test Files  66 passed (66)
     Tests  516 passed (516)
Statements   : 94.16% ( 1549/1645 )
Branches     : 87.17% ( 979/1123 )
Functions    : 95.24% ( 461/484 )
Lines        : 96.05% ( 1435/1494 )
PASS frontend-file-floor governed=61 floor=lines70/branches60/functions70/statements70
```
(64→66 test files, 490→516 tests: +2 files / +26 tests from this task, no
existing test weakened or deleted.)

`make coverage-frontend` from the repo root and the CI-equivalent
`cd frontend && node ../scripts/enforce-frontend-file-floor.mjs` both
produce the identical `PASS` line — confirmed by running both invocation
styles directly.

## Makefile / CI agreement

- `Makefile` `coverage-frontend`: `cd frontend && npx vitest run --coverage`
  then `node scripts/enforce-frontend-file-floor.mjs` (repo-root cwd).
- `.github/workflows/ci.yml` `Frontend Coverage` job (working-directory:
  `frontend`): `npx vitest run --coverage` then
  `node ../scripts/enforce-frontend-file-floor.mjs`.

Both invoke the same script with the same effective arguments; the script
resolves its own repo root from `import.meta.url` (not `process.cwd()`), so
both invocation styles are equivalent by construction, not by convention —
verified by running both from their respective working directories above.
`make test-ci-config` (which the CI job also runs as its first step) still
passes unchanged.

## Side effects / things noticed, not fixed (out of scope for this task)

- `npx tsc --noEmit` in `frontend/` fails on a pre-existing, unrelated error
  in `src/__tests__/setup.ts` (a `ReadableStream<Uint8Array>` generic
  mismatch between installed `@types/node` and `lib.dom`). Confirmed via
  `git status`/`git diff` that this file was not touched by this task and
  the error is not connected to anything in this change. Not fixed — out of
  scope for Task 5.
- `scripts/enforce-owned-coverage.mjs` remains uninvoked, as documented in
  the brief ("Gates that exist and don't run", item 4). This task did not
  wire it in — that would be a different task with different scope (it's
  keyed to a specific plan manifest, not a general-purpose gate).
