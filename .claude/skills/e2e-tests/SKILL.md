---
name: e2e-tests
description: Run, build, and debug the Cronometrix Playwright end-to-end suite against the real Rust backend and Next.js frontend. Use when running `make e2e`, installing Playwright/chromium, working on a spec under frontend/e2e/, checking mock-Hikvision or test ports, or reading a failing "E2E Tests" CI job.
---

# End-to-End tests (Phase 9)

Phase 9 added a Playwright-based end-to-end test suite that runs against the
real Rust/Axum backend (boot via Playwright `webServer`, ephemeral SQLite,
mock Hikvision device) and the real Next.js frontend. The suite is a hard-fail
gate on every PR via the `E2E Tests` job in `.github/workflows/ci.yml`.

Three contracts stay in the root `CLAUDE.md` (always loaded) rather than here,
because they must be known before you touch config, not only when running the
suite: the `CRONOMETRIX_E2E` / `CRONOMETRIX_LICENSE_BYPASS` abort contract, the
`TZ=America/Caracas` three-places freeze, and the Phase 12 Spanish login
language contract.

## Install (one-time per developer)

```bash
cd frontend && npm ci
npx playwright install --with-deps chromium

# Build helper binaries (gated by Cargo features so prod Docker excludes them):
cd backend
cargo build --release --bin cronometrix
cargo build --release --bin mock_hikvision --features mock-hikvision
cargo build --release --bin seed_e2e --features seed-e2e
```

Or use the orchestrated path:
```bash
make e2e-install
make e2e-build
```

## Local commands

```bash
make e2e            # Build backend binaries + run full Playwright suite
make e2e-build      # Build only — no test execution
make e2e-install    # Install npm deps + chromium browser
cd frontend && npx playwright test --grep "<spec name>"  # Run a single spec
```

The same commands run in CI (`.github/workflows/ci.yml::e2e-tests`), so a
green `make e2e` locally implies a green `E2E Tests` job on PRs.

## Default ports

| Process | Port | Notes |
|---------|------|-------|
| Backend (test) | 4001 | webServer probe at `/api/v1/health` |
| Frontend (test) | 3001 | next start (CI) or next dev (local) |
| Mock Hikvision (public) | 4400 | impersonates Hikvision unit; serves `/ISAPI/*` |
| Mock Hikvision (admin) | 4401 | test-only; specs push events into the alertStream queue |

Override via env vars: `SERVER_PORT`, `MOCK_HIKVISION_PORT`, `MOCK_HIKVISION_ADMIN_PORT`.

## CI gate

Workflow file: `.github/workflows/ci.yml`
Job name: **E2E Tests** (the intended required-status-check name; case-sensitive).

Job steps:
1. `actions/checkout@v4`
2. Validate CI file references
3. `actions/setup-node@v4` reads Node `24.15.0` from the root `.nvmrc`
4. Pin npm to `11.12.1`
5. Install the exact `rust-toolchain.toml` nightly + `Swatinem/rust-cache@v2` (target/ + cargo registry)
6. `npm ci`
7. `npm run build`
8. `npx playwright install --with-deps chromium`
9. `cargo build --release` for the 3 binaries (cronometrix, mock_hikvision, seed_e2e)
10. `npx playwright test`
11. `actions/upload-artifact@v4` × 2 — `playwright-html-report` + `playwright-test-results`, both `if: always()`, retention 14 days

Pinned actions: parity with Phase 8 T-08-15 (`actions/checkout@v4`,
`actions/setup-node@v4`, `actions/upload-artifact@v4`, `Swatinem/rust-cache@v2`).
`permissions: contents: read` at workflow scope (least privilege).

## Reading a failing CI run

1. Open the failing job's logs in the Actions tab.
2. Download the `playwright-html-report` artifact (always uploaded).
3. Open `index.html` locally — Playwright's HTML report shows traces, screenshots,
   videos for each failure.
4. Reproduce locally with `cd frontend && npx playwright test <spec>` against
   a fresh DB (`make e2e` rebuilds the binary + reseeds).
5. If the failure is in `setup/`: check that `backend/target/release/seed_e2e`
   and `mock_hikvision` exist (run `make e2e-build`).

## Note on private vs public repo

Playwright HTML reports include screenshots + DOM snapshots that may
contain seeded test names (Ana Pérez, Luis García, etc.). The repo is
currently private, so artifacts are scoped to repo collaborators. If the
repo ever goes public, revisit retention policy and consider scrubbing.
Since seed_e2e uses synthetic test data only (no real PII), the disclosure
risk is low — same disposition as Phase 8 coverage HTML.

## Pending live validation (carried forward to Phase 13)

Plan 12 (CI gate) shipped the workflow file, but the live runtime validation
was deferred per Phase 8 Plan 05 precedent. Three items remain:

1. **Positive verification** — push the branch, confirm `E2E Tests` runs green
   on GitHub Actions, confirm both artifacts are downloadable.
2. **Negative regression PR** — open a deliberate red PR (break a spec assertion);
   confirm `E2E Tests` FAILS and the artifacts include the failing trace.
3. **Branch protection** — Settings → Branches → branch protection rule for `main`
   → Require status checks → add `E2E Tests` to required list.

Phase 13 Plan 13-01 is the current executable owner of items 1-3. Phase 9's
historical code delivery is not live-gate proof. See
`.planning/phases/09-e2e-playwright-test-suite-login-dashboard-marcaciones-emplea/09-12-SUMMARY.md`
for exact commands.
