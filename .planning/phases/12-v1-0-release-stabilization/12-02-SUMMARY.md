# 12-02 Summary — Functional Contract Remediation

Verdict: PASS — scoped functional checkpoint

Release gate: FAIL — deferred to 12-05

The scoped verdict means that the repaired 12-02 functional contracts, the
full functional command set, and the exact owned-file coverage gate passed. It
is not an unqualified release verdict. Both unchanged project-wide coverage
gates remain below threshold, the out-of-scope coverage ledger below remains
release debt, and Plans 12-03, 12-04, and 12-05 are still required. Plan 12-05
is the sole unqualified release gate.

## Evidence Identity and Boundary

- Branch: `codex/phase12-02-functional-contracts`
- Tested implementation SHA: `0a698b5bee6e7b5bd91f3fa33ca1c3dd0470538f`
- Owned-coverage base SHA: `5f877fe445dfb18e555870d22a37201c1a4ee483`
- Immutable evidence directory: `/tmp/cronometrix-12-02-gate-ZVessY85`
- Platform: macOS 26.3.1 (a), Darwin 25.3.0, arm64
- Rust: `rustc 1.96.0-nightly (48cc71ee8 2026-03-31)` from pinned
  `nightly-2026-04-01`; Cargo 1.96.0-nightly
- Node: 24.15.0; npm: 11.12.1
- Preflight: the tree was clean and ports 3001, 4001, 4400, 4401, and 4402
  were free. The tree was clean again after the gate.

This summary is a documentation commit created after the immutable tested
implementation SHA. The summary commit is therefore not part of the tested
SHA and must not be represented as if the clean gate ran against it.

## Implemented Functional Contracts

1. **Authentication and refresh rotation.** JWTs require a random `jti`, so
   access and refresh tokens differ. Refresh performs one database
   compare-and-swap. A replay or losing concurrent request returns a bare 401
   and does not clear the winning request's cookie. The frontend coalesces
   refreshes into one in-flight request, and `SessionGate` bootstraps a browser
   session from the httpOnly refresh cookie.
2. **Upgrade and session operator contract.** Tokens issued before 12-02 lack
   the now-required `jti`; every user must sign in once after upgrade. The v1
   schema stores one refresh hash per user, so signing in from a new browser
   invalidates that user's previous refresh session. Frontend single-flight is
   scoped to one JavaScript realm; the server compare-and-swap is authoritative
   across tabs and processes.
3. **Login localization.** The live `/login` contract is Spanish and the root
   document uses `lang="es-VE"`. This supersedes Phase 9 decision D-19; the
   Phase 9 English result remains historical evidence rather than the current
   contract.
4. **Devices.** The canonical DTO uses `ip`, `connection_state`, and lifecycle
   `status`, accepts only `entry|exit`, and never exposes a password. The UI
   loads all lifecycle pages and consumes the backend DTO consistently.
5. **Enrollment.** The resumable list is paginated at
   `GET /enrollments?status=in_progress`. Canonical capture and retry routes
   are `POST /enrollments/captures`,
   `GET /enrollments/captures/{capture_id}`, and
   `POST /enrollments/{enrollment_id}/pushes/{device_id}/retry`; obsolete
   routes return 404. `source_device_id` is preserved. Device, webcam, and
   upload captures share one face-quality pipeline, and multipart requests
   include JSON `face_quality_score`. Reloading or reopening resumes the
   server-backed status.
6. **Timesheet identity.** A row is uniquely identified by
   `${employee_id}:${anchor_date}`. Rows include department enrichment, remain
   unambiguous, and use encoded deep links carrying both filters. The frontend
   does not deduplicate backend rows.
7. **Dashboard and SSE.** The `EventSource` lifecycle reacts to token changes.
   Payloads include employee and department data, the UI retains the newest 20
   events, and photos use the real event-photo endpoint with a deterministic
   fallback. HTTP tracing records paths only, so SSE query tokens are not
   logged.
8. **Enrollment E2E evidence boundary.** The enrollment flow ran against
   `mock_hikvision` with a valid stripped 640x480 JPEG, a partial device push,
   canonical retry, and request-log verification. This does not prove digest
   authentication, firmware compatibility, or operation with a physical
   reader.
9. **Helper binaries.** `mock_hikvision` and `seed_e2e` are covered through
   their real process and HTTP interfaces. Fresh owned metrics were 98.56%
   lines / 100% branches for the mock and 94.12% lines / 100% branches for the
   seed helper.
10. **Owned coverage enforcement.** The checker uses base
    `5f877fe445dfb18e555870d22a37201c1a4ee483`, requires exact equality with the
    ownership manifest's 12 backend and 25 frontend files, rejects omissions,
    extras, and malformed artifacts, and passes at the unchanged per-file
    floors.

## Final Review Remediation

- **Coordinated browser sessions.** Login, bootstrap refresh, request-time
  refresh, and logout now share a session generation. Login waits for an older
  refresh before sending credentials, supersedes a bootstrap refresh, and
  prevents an obsolete async result from restoring or clearing a newer
  session. Logout closes refresh admission, waits for the pending refresh, and
  invalidates the cookie that actually won the browser ordering race.
- **Stale logout compare-and-swap.** Backend logout clears the stored refresh
  hash only when both user and caller token hash match. A stale logout still
  expires its caller cookie but cannot invalidate a concurrent refresh winner.
- **Run-id containment.** E2E run IDs accept only portable names whose resolved
  paths are direct children of the OS temporary directory; teardown rejects
  traversal, absolute, separator-bearing, and otherwise uncontained paths.
- **External coverage identity and counters.** The owned checker requires
  externally supplied expected plan and base-SHA identities, proves the base
  is a strict ancestor, and validates frontend `total`, `covered`, `skipped`,
  and percentage relationships rather than trusting percentages alone. Valid
  Vitest zero-denominator encodings remain supported.
- **Enrollment evidence boundary.** `face_quality_score` is now required typed
  JSON with deny-unknown-field and consistency/acceptance validation. The
  server still decodes and normalizes the JPEG separately; it does not claim
  to run a second face detector. Filesystem `photo_path` is internal-only and
  no longer appears in capture API responses or frontend types.
- **Canonical employee and Node contracts.** The obsolete `cedula` employee
  alias was removed in favor of `employee_code`. Standalone Node contract files
  use the `.contract.ts` suffix, are executed explicitly by the
  `node-contracts` gate, and are not collected as Playwright tests.

Two security-review findings were assigned as executable work, not silently
claimed here: capture TTL/orphan cleanup is **not implemented** in 12-02 and is
executable 12-03 work; upstream gateway/container SSE token-log proof is **not
implemented** in 12-02 and is executable 12-04 work. Application path-only
tracing does not substitute for the latter proof.

## Clean-Gate Verification

The exact command exits recorded in `commands.tsv` were:

| Gate command | Exit | Result |
|---|---:|---|
| `node-contracts` | 0 | 4/4 standalone Node contracts passed |
| `directed-vitest` | 0 | 7 files; 72/72 tests passed |
| `cargo-fmt` | 0 | Rust formatting clean |
| `cargo-clippy` | 0 | Strict Clippy clean |
| `cargo-test` | 0 | 56 test binaries; 793 passed, 0 failed, 22 ignored |
| `npm-ci` | 0 | Reproducible install completed; advisory debt retained below |
| `typecheck` | 0 | TypeScript clean |
| `frontend-build` | 0 | Production build generated 20/20 static pages |
| `owned-fixtures` | 0 | 46/46 checker fixtures passed |
| `frontend-coverage` | 2 | Tests passed; unchanged global thresholds failed |
| `backend-coverage` | 2 | Tests passed; unchanged global thresholds failed |
| `owned-coverage` | 0 | Exact owned manifest passed |
| `e2e` | 0 | Playwright 80/80 with one worker |
| `diff-check` | 0 | No whitespace errors |
| `tree-clean` | 0 | Worktree clean after evidence run |

The exact owned-gate output was:

```text
PASS owned-coverage plan=12-02 backend=12 frontend=25
```

The full frontend coverage execution ran 54 files and passed all 429 tests;
the raw exit remained 2 because project-wide coverage was below its unchanged
gate:

| Frontend metric | Covered / total | Actual | Gate |
|---|---:|---:|---:|
| Statements | 1270 / 1596 | 79.57% | 90% |
| Branches | 784 / 1061 | 73.89% | 85% |
| Functions | 355 / 470 | 75.53% | 90% |
| Lines | 1177 / 1451 | 81.11% | 90% |

The full backend coverage execution passed its 793 tests; the raw exit
remained 2 because project-wide coverage was below its unchanged gate:

| Backend metric | Covered / total | Actual | Gate |
|---|---:|---:|---:|
| Lines | 8839 / 11119 | 79.49% | 90% |
| Branches | 600 / 994 | 60.36% | 85% |

The unchanged per-file floors are backend lines 70% / branches 60%, and
frontend statements 70% / branches 60% / functions 70% / lines 70%. The
owned-file PASS does not waive, reduce, or redefine any project-wide gate or
per-file floor.

## Exact Out-of-Scope Coverage Ledger

Every item in this section is outside the 12-02 ownership manifest and remains
release debt for 12-05. This ledger is not permission to exclude a file or
lower a threshold.

### Frontend files below per-file floors

Values are statements / branches / functions / lines percentages.

- `components/anomalies/anomalies-filters.tsx`: 0 / 0 / 0 / 0
- `components/anomalies/anomalies-table.tsx`: 0 / 0 / 0 / 0
- `components/daily-records/daily-record-dialog.tsx`: 0 / 0 / 0 / 0
- `components/events/event-detail-dialog.tsx`: 0 / 0 / 0 / 0
- `components/events/events-filters.tsx`: 0 / 0 / 0 / 0
- `components/events/events-table.tsx`: 0 / 0 / 0 / 0
- `components/settings/rules-form.tsx`: 0 / 0 / 0 / 0
- `components/settings/tolerance-simulator.tsx`: 0 / 0 / 0 / 0
- `components/timesheet/leave-row-actions.tsx`: 26.66 / 26.31 / 8.33 / 30
- `hooks/use-auth.ts`: 0 / 0 / 0 / 0
- `lib/file-download.ts`: 0 / 0 / 0 / 0
- `lib/format/datetime.ts`: 56.25 / 66.66 / 66.66 / 53.84
- `lib/reports/csv.ts`: 0 / 0 / 0 / 0

### Backend failures from the fresh LCOV postprocessor

- `calc/lunch.rs`: branches 25.00%
- `devices/service.rs`: lines 54.07%, branches 30.00%
- `employees/handlers.rs`: branches 50.00%
- `enrollments/pusher.rs`: branches 55.00%
- `leaves/handlers.rs`: branches 45.83%
- `license/fingerprint.rs`: lines 13.33%, branches 0.00%
- `license/service.rs`: lines 35.92%, branches 15.38%
- `recompute/nightly.rs`: branches 50.00%
- `supervisor/mod.rs`: branches 50.00%
- `supervisor/watchdog.rs`: branches 0.00%
- `tenant_info/service.rs`: lines 53.91%, branches 40.00%
- `test_reset/mod.rs`: branches 50.00%
- `users/handlers.rs`: lines 0.00%
- `users/service.rs`: lines 0.00%, branches 0.00%
- `workers/db_write.rs`: lines 0.00%
- `workers/purge.rs`: branches 55.56%
- Project-wide branches: 60.36% < 85%.

`license/fingerprint.rs` and `license/service.rs` have a documented macOS
pseudo-filesystem limitation because Darwin does not expose the Linux
`/proc/cpuinfo` and `/sys/{class/net,block}` sources. Linux CI is authoritative
for those two files, but that platform note does not make the current global
coverage gate green.

## Security, Deployment, and Operational Debt

- `npm ci` reported 16 vulnerabilities: 1 low, 7 moderate, and 8 high. No
  `npm audit fix` or dependency mutation was authorized. The advisories remain
  security debt requiring triage.
- Real Hikvision validation remains deferred/private. A physical reader,
  digest authentication, firmware variants, and site networking are not
  proven by the mock.
- Linux `linux/arm64` container runtime validation of the enrollment stack is
  deferred. The Docker image pull exceeded the six-minute cap; the macOS
  build, unit, and E2E results do not substitute for that runtime proof.
- Cross-host LIC-05 hardware-binding validation remains deferred/private.
  Anti-cloning has not been live-validated across hosts.
- Phase 8 live CI follow-ups remain external: a positive GitHub Actions run
  with downloadable artifacts, a deliberate negative coverage PR, and
  required branch-protection status checks. No completion is claimed without
  that evidence.
- The E2E log retains Next's warning that `next start` is incompatible with
  `output: standalone` and recommends the standalone server entrypoint. This
  did not fail the functional gate, but production/private distribution must
  reconcile it in 12-04.
- Plans 12-03, 12-04, and 12-05 remain mandatory. Plan 12-05 owns the only
  unqualified release verdict.

## Ordered Implementation Commits

The following is the exact reverse log from the owned-coverage base through
the tested implementation SHA:

- `62347f7` fix(auth): rotate refresh tokens with atomic compare-and-swap
- `9e6038b` fix(auth): bootstrap and gate browser sessions
- `1633388` fix(auth): close browser session races
- `b6de007` fix(login): lock Spanish copy and supersede English D-19
- `c7b4a9a` fix(e2e): wire browser API origin into release harness
- `6499542` fix(e2e): harden harness configuration guard
- `f104423` fix(e2e): close harness guard bypasses
- `4295103` fix(devices): align UI with canonical device DTO
- `bbe799e` fix(e2e): align Hikvision mock command routes
- `7404aa7` fix(devices): load all lifecycle pages
- `eff926a` feat(enrollment): expose resumable enrollment status API
- `bbddebe` fix(enrollment): unify capture validation and resumable UI
- `df3b518` fix(enrollment): classify zero-device terminal failure
- `46de533` fix(timesheet): make employee-day rows unambiguous
- `f4f55b8` fix(dashboard): publish enriched events and restore SSE lifecycle
- `fbe5e67` fix(dashboard): clear stale event photos deterministically
- `433ff90` test(e2e): cover enrollment and repaired functional contracts
- `2dd7ee4` fix(e2e): harden enrollment evidence portability
- `7195593` test(e2e): align legacy suites with current contracts
- `36ef9dc` test(e2e): use default waits for legacy contracts
- `989e126` docs(12): separate scoped checkpoints from release gate
- `bbe76d4` test(devices): cover canonical modal and card contracts
- `6416d3f` test(devices): assert empty fields after reopen
- `881d06a` docs(12): make scoped coverage gates reproducible
- `4910749` test(e2e): cover helper binaries through real interfaces
- `dc81552` test(e2e): harden helper process coverage
- `ac8a24d` test(coverage): enforce plan-owned floors
- `baeb38f` fix(coverage): harden owned artifact parsing
- `e655763` docs(12-02): record functional contract remediation
- `60a4491` fix(auth): serialize login with pending refresh
- `5d3d9f5` fix(auth): preserve refresh winner on stale logout
- `27eb89a` fix(e2e): contain per-run teardown paths
- `2a18e65` fix(coverage): fail closed on ownership identity
- `cae2a8a` fix(enrollment): validate typed face quality evidence
- `aea6f39` fix(enrollment): keep capture paths internal
- `c15e63a` fix(frontend): remove obsolete employee aliases
- `885fc4c` docs(12): assign deferred security cleanup
- `8ade624` fix(auth): supersede bootstrap refresh on login
- `0436a82` fix(auth): serialize logout with pending refresh
- `25e7456` fix(coverage): require external plan identity
- `87224af` fix(coverage): accept Vitest empty metrics
- `0a698b5` fix(e2e): keep Node contracts out of Playwright

The historical `e655763` summary commit is part of the now-tested 42-commit
history. The later `docs(12-02): refresh final immutable gate evidence` commit
only refreshes this checkpoint document; it remains outside the tested
implementation SHA and does not promote the scoped PASS to a release PASS.
