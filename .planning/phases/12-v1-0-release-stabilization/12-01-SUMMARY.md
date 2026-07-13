# 12-01 Summary — Rebaseline and Reproducible Test Harness

Verdict: PASS

Scope note: this is a PASS for the 12-01 harness/rebaseline scope only. The
frontend functional coverage suite still exits non-zero with deterministic
product/test-contract failures assigned to 12-02. No 12-01 harness crash,
dependency drift, Rust lint failure, Playwright startup failure, or DB/run-id
contamination remains.

The CI configuration is now also guarded locally. This does not claim a live
GitHub Actions run: that external proof remains in the Phase 12/13 release
validation sequence.

## Branch and SHAs

- Worktree: `/Users/gerswin/Proyectos/cronometrix-12-01`
- Branch: `codex/phase12-01-harness`
- Baseline / `origin/main`: `1dd6d758fc1ed775189a4fff3f20d6a7c1800e34`
- Harness-tested implementation SHA: `be24f17b67916e8659a0a1a0daca5d5428dc3060`
- CI-follow-up tested SHA: `4866babf982b1fb4fefc54b3a35039f06b64bf72`
- CI-association/toolchain follow-up tested SHA: `b5316327fa6f17b7427c8fb9942821f4259af39c`
- CI path-containment follow-up tested SHA: `eb7f8d6dfb8093eb3b7bc2478bef0abb2ed5b7e1`

## Commits

- `731a7c7` docs(12-01): rebaseline v1 remediation status
- `98ff196` style(12-01): normalize Rust formatting
- `dddaa7c` refactor(12-01): clear strict Clippy baseline
- `5c7e01d` build(12-01): restore reproducible frontend install
- `c39991b` test(12-01): inject deterministic E2E mode
- `70da6ae` test(12-01): isolate E2E run context
- `5d2c0b1` test(12-01): stabilize E2E process run id
- `37d5913` test(12-01): disable stale E2E server reuse
- `be24f17` test(12-01): make Playwright harness reproducible
- `7ce3b60` docs(12-01): record reproducible harness baseline
- `4866bab` test(12-01): validate CI Node version files
- `b531632` test(12-01): validate CI toolchain associations
- `eb7f8d6` test(12-01): reject CI version-file escapes

## Completed Tasks

1. Planning truth reconciled: Phase 11 marked superseded partial, Phase 12 set
   in progress, Phase 13 planned, and remediation blockers/deferrals recorded.
2. Rust formatting normalized in a dedicated mechanical commit.
3. Strict Clippy baseline restored without blanket suppressions.
4. Frontend install made reproducible with Node `24.15.0` from the root
   `.nvmrc`, npm `11.12.1`, regenerated lockfile, and Docker/web workflow
   alignment. `make test-ci-config` prevents missing `setup-node` file paths.
5. E2E test mode made startup-captured and fail-closed: license bypass and
   reset route require explicit E2E capabilities.
6. Playwright run context centralized: one run id, one DB path, no Turso dotenv
   contamination, no stale server reuse.
7. Static storage state removed. Authenticated specs now create fresh role
   contexts and Bearer-backed API request contexts per test.
8. Setup project now proves the backend reads the seeded DB by asserting the six
   seeded employee IDs through `GET /employees?limit=100`.
9. CI Node-version references have a red/green regression guard and both Node
   jobs validate it before `actions/setup-node`.
10. A fixture-driven follow-up proves each individual `setup-node` step owns
    exactly one literal, repository-contained version file; both Rust jobs now
    install the exact `rust-toolchain.toml` channel rather than a shadowed
    floating default.

## Post-summary CI correction

Root cause: commit `5c7e01d` correctly created the repository-root `.nvmrc`
but changed both `actions/setup-node` steps to `frontend/.nvmrc`. GitHub resolves
`node-version-file` from the repository root, and that duplicate path did not
exist.

TDD evidence:

- RED: `make test-ci-config` exited 2 and reported the two missing
  `frontend/.nvmrc` references.
- GREEN: after changing only those values to `.nvmrc`, the same command exited
  0; `bash -n scripts/test-ci-node-version-files.sh` and `git diff --check`
  also exited 0.

The second follow-up closed a count-only false positive: a workflow fixture
with two setup actions, one associated version file, and one decoy field on a
different step passed the original global-count guard. The new fixture suite
failed first, then passed after the guard began validating association within
each YAML step. It also covers duplicate entries, missing files, no setup
steps, and named setup steps.
Commit `eb7f8d6` added the repository-escape fixture and corrected stale CI
comments; the same fixture suite and real-workflow guard remained green.

## Original harness verification (`be24f17`)

| Command | Exit | Evidence |
|---|---:|---|
| `cargo fmt --all -- --check` | 0 | Rust format clean |
| `npm ci` | 0 | Clean install from lockfile; npm audit reports 16 dependency advisories |
| `cargo clippy --all-targets --all-features -- -D warnings` | 0 | Strict Clippy clean |
| `npm run typecheck` | 0 | TypeScript clean |
| `cargo test --all-targets --all-features --no-fail-fast` | 0 | Backend tests passed |
| `npm run build` | 0 | Next.js production build passed |
| `npm run test:coverage` | 1 | 323 passed, 16 failed; deterministic functional failures listed below |
| `make e2e-build` | 0 | Release backend binaries and Next build passed |
| `CRONOMETRIX_E2E_RUN_ID=phase12-01-harness npx playwright test --project=setup` | 0 | 4 setup tests passed |
| `CRONOMETRIX_E2E_RUN_ID=phase12-01-rbac npx playwright test --project=chromium e2e/rbac.spec.ts` | 0 | 15 tests passed, including setup dependency |
| `CRONOMETRIX_E2E_RUN_ID=phase12-01-release CRONOMETRIX_E2E_RELEASE=true npx playwright test --project=setup` | 0 | 4 setup tests passed against release binaries + `next start` |
| `npx playwright test --list` | 0 | 76 tests discovered in 9 files |
| `git diff --check` | 0 | No whitespace/check errors |

## CI follow-up verification (`4866bab`)

| Command | Exit | Evidence |
|---|---:|---|
| `bash -n scripts/test-ci-node-version-files.sh` | 0 | Regression script syntax valid |
| `make test-ci-config` | 0 | The original count/path guard finds two root `.nvmrc` references and both files exist |
| `git diff --check` | 0 | Follow-up patch has no whitespace errors |

## CI association/toolchain verification (`b531632`, extended at `eb7f8d6`)

| Command | Exit | Evidence |
|---|---:|---|
| `bash scripts/tests/test-ci-node-version-files.sh` | 0 | Valid, misassociated, duplicate, missing-file, repository-escape, and no-setup fixtures behave fail-closed |
| `make test-ci-config` | 0 | Fixture suite and real workflow both pass |
| `bash -n scripts/test-ci-node-version-files.sh` | 0 | Production guard syntax valid |
| `bash -n scripts/tests/test-ci-node-version-files.sh` | 0 | Fixture suite syntax valid |
| CI YAML assertion | 0 | Two Node steps use root `.nvmrc`; two Rust steps read `rust-toolchain.toml` |
| `git diff --check` | 0 | Association/toolchain patch has no whitespace errors |

## Remaining Functional Failures Assigned to 12-02

`npm run test:coverage` is intentionally not masked. The current deterministic
failures are product/test-contract mismatches, not harness failures:

- `src/components/enrollment/__tests__/enrollment-modal.test.tsx`: close button
  selection finds multiple `Cerrar` buttons.
- `src/components/enrollment/__tests__/enrollment-modal-extra.test.tsx`: close
  behavior/toast expectations are out of sync with current modal behavior.
- `src/__tests__/dashboard-activity-feed-extra.test.tsx`: activity feed link,
  em-dash query, and photo-fetch expectations are stale versus current UI.
- `src/__tests__/timesheet-novedad-modal-extra.test.tsx`: labels/submission
  expectations are stale versus current novedad modal behavior.
- `src/__tests__/timesheet-table-extra.test.tsx`: leave badge expectations are
  stale versus current table rendering.
- `src/components/reports/__tests__/drill-down-dialog.test.tsx`: expects raw
  minute text `480`; UI now renders formatted time with title metadata.
- `src/components/dashboard/__tests__/kpi-tile.test.tsx`: warning/danger class
  expectations are stale versus current tile styling.

The last direct Playwright product probe also shows the login spec still expects
English copy while the current UI renders Spanish. The decision is now closed:
Spanish is authoritative, and 12-02 Task 3 updates the live code/tests and adds
an explicit supersession note to Phase 9's historical D-19 evidence.

## Risks and Handoff

- `next start` emits a warning with `output: standalone`: Next recommends
  `node .next/standalone/server.js`. It did not block the 12-01 release setup,
  but 12-04 should reconcile the production/private distribution command.
- Live Hikvision validation and LIC-05 cross-host validation remain deferred to
  Phase 13 with real infrastructure.
- Next step: fast-forward the integration branch through `eb7f8d6` and the
  reconciled plan/summary commit created after it, then start 12-02 from that
  resulting integration HEAD in a fresh isolated worktree. Do not branch
  12-02 directly from `be24f17` or `4866bab`.
