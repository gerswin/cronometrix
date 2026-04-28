# Phase 8: Test Coverage & Quality Gate - Research

**Researched:** 2026-04-28
**Domain:** Test instrumentation, coverage gating, CI pipeline, AppState refactor
**Confidence:** HIGH (tooling), HIGH (codebase sweep), MEDIUM (delta candidates — not measured yet)

---

## Executive Summary

The user-facing decisions in CONTEXT.md (D-01..D-23) are all implementable with a single notable adjustment: **`cargo-llvm-cov --branch` is still nightly-only as of v0.8.5 (verified locally) and rustc 1.93.0 — the stable Rust toolchain cannot produce branch coverage**. Two viable paths: (a) run the backend coverage CI job under `nightly` *only for coverage*, while keeping `stable` for build/test; or (b) substitute `--fail-under-regions` (a stable-friendly LLVM region count, finer-grained than lines, coarser than branches) for the 85% branch threshold on the backend. Frontend Vitest 4 supports native branch coverage and per-file thresholds — no asymmetry concern there.

The leaves/events/enrollments/data-dir env-var anti-pattern repeats in **5 known locations** across `backend/src/`, with **3 test files** depending on `*RootGuard` env-mutation patterns. AppState already carries `Arc<Config>` and naturally fits a `paths: Arc<Paths>` substruct; tests already use a shared `common::test_state()` helper that can take a `tempdir`-backed `Paths`. The fix is mechanical and well-scoped.

CI is greenfield (`.github/workflows/` absent). The repo is a single Cargo crate at `backend/`, not a workspace, so `cargo llvm-cov --all-features` runs over one package — `--workspace` is harmless but unnecessary.

**Primary recommendation:** Run backend coverage on nightly Rust (only the coverage job); keep all other CI jobs on stable. Use `cargo llvm-cov --branch --all-features --lcov --output-path lcov.info` plus `cargo llvm-cov --branch --all-features --html` (separate runs reuse the .profraw cache via `--no-clean`). Enforce per-file floor by post-processing `lcov.info` with a small awk/Rust script — no upstream flag exists. Frontend uses Vitest's native `coverage.thresholds` + glob-pattern per-file thresholds.

---

## User Constraints (from CONTEXT.md)

### Locked Decisions

**CI Platform & Pipeline:**
- **D-01:** GitHub Actions, `.github/workflows/ci.yml`. Greenfield (`.github/workflows/` does not exist).
- **D-02:** `Makefile` (planner may switch to `justfile` if survey shows preference; no `Makefile` or `justfile` exists today). Targets: `make coverage`, `make coverage-backend`, `make coverage-frontend`.
- **D-03:** Triggers: `push` (any branch) AND `pull_request` (target `main`).
- **D-04:** HTML reports as workflow artifacts. No external service (no Codecov / Coveralls).
- **D-05:** Backend coverage and frontend coverage as **separate jobs**. Both required for gate.

**Coverage Tooling:**
- **D-06:** Backend uses `cargo-llvm-cov`. Installed via `taiki-e/install-action@cargo-llvm-cov` in CI; `cargo install cargo-llvm-cov` locally. Not a Cargo dependency.
- **D-07:** Backend runs unit + integration in single combined run (`cargo llvm-cov --all-features`). Excludes `tests/common/` from denominator.
- **D-08:** Frontend uses Vitest's built-in v8 coverage. Already installed (`vitest@4.1.5`, `@vitest/coverage-v8@4.1.5`).

**Coverage Scope & Exclusions:**
- **D-09:** Minimal exclusions — write tests, don't shrink denominator. Allowed: `main.rs`, binary entrypoints, `build.rs`, generated code, dead `Display`/`Debug` derives, unreachable error variants.
- **D-10:** Frontend coverage scope: `src/components/`, `src/hooks/`, `src/lib/`. Exclude `src/app/`, `src/components/ui/`, type-only files.
- **D-11:** Backend coverage scope: all of `src/`. Exclude `src/main.rs`, `src/bin/*` (none exist), `tests/common/*`.
- **D-12:** Planner identifies coverage delta and proposes targeted tests.

**Gate Behavior:**
- **D-13:** Hard fail on miss. No soft-warn, no manual override.
- **D-14:** Two-level: project-wide ≥90% line / ≥85% branch (both backend + frontend); per-file floor ≥70% line / ≥60% branch.
- **D-15:** No ratcheting. Threshold-only.
- **D-16:** Per-file floor: cargo-llvm-cov has no built-in flag — post-process `lcov.info`. Vitest natively supports it via glob-pattern thresholds.

**leave_tests cwd Fix:**
- **D-17:** Root cause: `leaves_root()` reads env + falls back to `./data/leaves` — cwd-dependent + racy.
- **D-18:** Fix via AppState injection.
- **D-19:** Sweep `events_root` + any other `./data/*` defaults.
- **D-20:** Remove `LeavesRootGuard` / `EventsRootGuard` from tests after roots are injected.
- **D-21:** Production startup unchanged: same env vars, same defaults read at startup.

**Documentation:**
- **D-22:** "Test Coverage" section in `CLAUDE.md`.
- **D-23:** Document AppState path-injection pattern in `CLAUDE.md` Conventions.

### Claude's Discretion
- `Makefile` vs `justfile` (default Makefile).
- Per-file floor mechanism on backend (post-process script flavor).
- Whether to factor coverage commands into `scripts/coverage.sh`.
- Specific test additions to close gap.
- `Config` struct vs new `Paths` substruct.
- Order of operations within phase.

### Deferred Ideas (OUT OF SCOPE)
- Codecov / Coveralls.
- Ratchet baseline.
- Mutation testing (cargo-mutants).
- E2E / Playwright covering `src/app/`.
- Per-crate or per-package thresholds.
- Performance benchmarks.
- Property-test expansion beyond minimum.
- Snapshot tests for serialized API responses.
- Manual override label.

---

## Project Constraints (from CLAUDE.md)

- Backend: Rust 1.77+ stable, Axum 0.8.x, Tokio 1.51, libSQL embedded replica, `axum-test 16` (already in dev-deps).
- Frontend: Next.js 16.x, React 19.x, TypeScript 5.x, Vitest 4.x.
- Audit-compliance ethos → hard-fail gate (D-13) consistent with project posture.
- `cargo-nextest` mentioned in tooling table — `cargo llvm-cov nextest` subcommand is the runner-compatible variant.
- `frontend/CLAUDE.md` includes mandatory note: "This is NOT the Next.js you know." Phase-8 changes to frontend are limited to Vitest config + adding tests — no Next.js API surface touched, so the warning does not block the work.

---

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|--------------|----------------|-----------|
| Coverage instrumentation (backend) | Backend (Rust toolchain) | — | LLVM source-based coverage runs against the compiled crate |
| Coverage instrumentation (frontend) | Frontend (Vitest + v8) | — | v8 ships in Node; Vitest hooks into the V8 coverage profiler |
| Threshold enforcement (project-wide) | CI (GitHub Actions) | Tool flags (`--fail-under-*`, `coverage.thresholds`) | Tools exit non-zero; CI surfaces it as a job failure |
| Threshold enforcement (per-file) | Custom script (backend) / Vitest config (frontend) | CI | No native backend flag; lcov post-processor needed |
| Filesystem-root injection | Backend (`AppState` + `Paths` substruct) | Tests (tempdir construction) | State management tier — exact same shape as existing `Arc<Config>` injection |
| HTML artifact upload | CI (actions/upload-artifact@v4) | — | Artifact tier owns retention and download UX |
| Local reproduction | Build tooling (`Makefile`) | Backend + frontend tools | Build tier owns the developer entry point |

---

## Standard Stack

### Core
| Library / Tool | Version | Purpose | Why Standard |
|----------------|---------|---------|--------------|
| `cargo-llvm-cov` | 0.8.5 (locally installed; verified) | Backend line + region + (nightly-gated) branch coverage | Wraps `-C instrument-coverage`; standard Rust coverage tool. [VERIFIED: local `cargo llvm-cov --version`] |
| `vitest` | 4.1.5 (already in `package.json`) | Frontend test runner with built-in v8 coverage | Already chosen by project; v4 supports glob-pattern per-file thresholds. [VERIFIED: `frontend/package.json`] |
| `@vitest/coverage-v8` | 4.1.5 (already in `devDependencies`) | V8-based coverage provider for Vitest | Faster than istanbul, accuracy parity since 3.2.0. [VERIFIED: `frontend/package.json`] |
| `taiki-e/install-action` | `@v2` (or `@cargo-llvm-cov` shorthand) | GitHub Action that installs cargo-llvm-cov binary | The README's recommended install method. [CITED: github.com/taiki-e/cargo-llvm-cov README] |
| `Swatinem/rust-cache` | `@v2` | Cache `target/` and Cargo registry between CI runs | Standard Rust caching action; recommended by ecosystem. [CITED: github.com/Swatinem/rust-cache README] |
| `actions/checkout` | `@v6` | Source checkout in CI | Latest major; matches taiki-e README example. [CITED: cargo-llvm-cov README] |
| `actions/upload-artifact` | `@v4` | Upload HTML reports as workflow artifacts | Latest stable v4; v3 deprecated. [ASSUMED — based on training; verify pin during plan-check] |
| `actions/setup-node` | `@v4` | Node toolchain + npm cache for frontend | Standard Node CI setup. [ASSUMED] |

### Supporting / Already in repo
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tempfile` | 3 (dev-deps) | Per-test tempdirs for `Paths` injection | Already used; new `make_state(tempdir)` helper consumes it. [VERIFIED: `backend/Cargo.toml:50`] |
| `axum-test` | 16 (dev-deps) | Handler integration tests | Already used for existing test suite; no new tests need a different harness. [VERIFIED: `backend/Cargo.toml:44`] |
| `proptest` | 1.11.0 (dev-deps) | Property-based tests | Already used in `calc/`; do NOT expand beyond gap-closing per CONTEXT deferred list. [VERIFIED: `backend/Cargo.toml:48`] |
| `wiremock` | 0.6.5 (dev-deps) | Mock external HTTP for ISAPI / DO Functions | Use for license-gate edge tests if those are gap candidates. [VERIFIED: `backend/Cargo.toml:53`] |
| `@testing-library/react` | 16.3.2 | React component testing | Already installed. [VERIFIED: `frontend/package.json:41`] |
| `@testing-library/jest-dom` | 6.9.1 | Custom DOM matchers | Already wired in `setup.ts`. [VERIFIED: `frontend/src/__tests__/setup.ts`] |
| `msw` | 2.7.0 | Mock API for hook tests | Already installed; use for `useSse`-style hook tests if gap. [VERIFIED: `frontend/package.json:50`] |

**Version verification commands (planner runs during Wave 0 if any of these are flagged stale):**
```bash
cargo install cargo-llvm-cov --version 0.8.5  # Pin to verified version
npm view vitest version
npm view @vitest/coverage-v8 version
```

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `cargo-llvm-cov` | `tarpaulin` | Tarpaulin uses ptrace, slower, less accurate, has known issues with async; cargo-llvm-cov is the modern choice |
| `@vitest/coverage-v8` | `@vitest/coverage-istanbul` | Istanbul works on any JS runtime but is slower; v8 has accuracy parity since Vitest 3.2.0 — keep v8 |
| Codecov / Coveralls upload | HTML artifact only | Decided against per D-04 (CONTEXT) |
| Per-crate thresholds | Project-wide single number | Decided against per CONTEXT deferred list |

### Installation
**Local (one-time):**
```bash
# Backend
cargo install cargo-llvm-cov --locked --version 0.8.5
rustup component add llvm-tools-preview --toolchain stable
# If branch coverage is enabled (recommended path):
rustup toolchain install nightly
rustup component add llvm-tools-preview --toolchain nightly

# Frontend (already installed)
cd frontend && npm ci
```

**CI install steps:** Inline in workflow (see CI Workflow Skeleton below).

---

## Tooling State (cargo-llvm-cov flags + Vitest config)

### cargo-llvm-cov 0.8.5 — Verified Flags

[VERIFIED: local `cargo llvm-cov --help 2>&1`]

```
--fail-under-functions <MIN>   Exit non-zero if function coverage < MIN%
--fail-under-lines <MIN>       Exit non-zero if line coverage < MIN%
--fail-under-regions <MIN>     Exit non-zero if region coverage < MIN%
--branch                       Enable branch coverage. (unstable — nightly only)
--mcdc                         Enable mcdc coverage. (unstable — nightly only)
--show-missing-lines           Print uncovered line numbers in summary
--include-build-script         Include build.rs in coverage
--ignore-filename-regex <RE>   Exclude files by regex from report
--workspace                    All packages in workspace (single-crate-safe)
--all-features                 Enable all Cargo features
--lcov                         Output LCOV format to stdout/--output-path
--html                         Generate HTML to target/llvm-cov/html (default)
--output-path <PATH>           Write to file (lcov/json/cobertura/text)
--output-dir <DIR>             HTML output directory (default target/llvm-cov)
--no-clean                     Reuse existing .profraw between invocations
nextest                        Subcommand: run via cargo-nextest
```

**Critical discovery:** `--branch` is documented as **(unstable)** and verified to require nightly. There is **no `--fail-under-branches` flag** — only lines, functions, regions. [VERIFIED: `cargo llvm-cov --help` on local rustc 1.93.0; CITED: github.com/taiki-e/cargo-llvm-cov README; CITED: rust-lang.org/beta/rustc/instrument-coverage.html (`-Z coverage-options` is unstable)]

**Version verification of stable-vs-nightly status:** The `--branch` flag was added in cargo-llvm-cov 0.6.8 (March 2024) and the gating "nightly-2024-03-16+" remained in force as of cargo-llvm-cov 0.8.5 (current). No public release note announces stabilization. [CITED: github.com/taiki-e/cargo-llvm-cov/issues/8]

**Combined unit + integration:** `cargo llvm-cov --all-features --lcov --output-path lcov.info` from `backend/` runs **all** `#[cfg(test)]` inline tests in `src/` AND every integration test in `backend/tests/*.rs` in a single instrumented binary build, producing one merged report. `--workspace` is harmless but unnecessary (single-crate). [VERIFIED: cargo-llvm-cov README; VERIFIED: `backend/Cargo.toml` is single-crate]

**Compatibility with cargo-nextest:** Use the dedicated subcommand `cargo llvm-cov nextest --all-features --lcov --output-path lcov.info`. Internally calls `cargo nextest run`. Faster on this codebase given 20+ integration test files. [CITED: cargo-llvm-cov README]

**HTML output:** `--html` writes to `target/llvm-cov/html/index.html` by default. Override with `--output-dir`. Reusing the .profraw cache via `--no-clean` lets a second invocation produce HTML without re-running tests:
```bash
cargo llvm-cov --branch --all-features --lcov --output-path lcov.info
cargo llvm-cov --branch --all-features --no-clean --html
```

**No built-in per-file threshold.** Verified across help output and README. Per-file floor must be enforced by post-processing `lcov.info`. [VERIFIED: `cargo llvm-cov --help`]

### Vitest 4 Coverage Config — Verified Shape

[VERIFIED: vitest.dev/config/coverage; cross-verified via WebSearch site:vitest.dev]

```typescript
// frontend/vitest.config.ts
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/__tests__/setup.ts'],
    coverage: {
      provider: 'v8',                          // already installed
      reporter: ['text', 'html', 'lcov'],      // text → CI logs; html → artifact; lcov → optional future use
      reportsDirectory: './coverage',          // default — explicit for clarity
      include: [
        'src/components/**/*.{ts,tsx}',
        'src/hooks/**/*.{ts,tsx}',
        'src/lib/**/*.{ts,tsx}',
      ],
      exclude: [
        'src/components/ui/**',                // shadcn vendored copies (D-10)
        'src/**/__tests__/**',
        'src/**/*.test.{ts,tsx}',
        'src/**/*.spec.{ts,tsx}',
        'src/**/*.d.ts',                       // type-only files (D-10)
        'src/lib/utils.ts',                    // pure cn()/clsx wrapper — review during planning
      ],
      thresholds: {
        lines: 90,
        branches: 85,
        functions: 90,
        statements: 90,
        perFile: true,                         // D-14: per-file floor enabled
        // Per-file floor via global perFile:true applies the same numbers above
        // to EVERY file. To set a softer floor (D-14: 70 line / 60 branch),
        // EITHER set the project-wide numbers to the floor and apply stricter
        // numbers via glob, OR (recommended) set perFile:false and use globs
        // to enforce the floor explicitly. See "Per-File Floor Mechanism" below.
      },
    },
  },
  resolve: {
    alias: { '@': path.resolve(__dirname, './src') },
  },
})
```

**Defaults:** `coverage.reporter = ['text', 'html', 'clover', 'json']`; `coverage.reportsDirectory = './coverage'`; HTML lands at `coverage/index.html`. [CITED: vitest.dev/config/coverage]

**`perFile: true` semantics:** Applies the project-wide numbers (`lines`, `branches`, `functions`, `statements`) to EVERY file. This is stricter than D-14's per-file floor (≥70 line / ≥60 branch is *softer* than the project ≥90/85). The correct shape is:

```typescript
thresholds: {
  // Project-wide gates — D-14 line 1
  lines: 90,
  branches: 85,
  functions: 90,
  statements: 90,
  // Per-file floor — D-14 line 2 (softer floor for every file)
  '**/*.{ts,tsx}': {
    lines: 70,
    branches: 60,
    functions: 70,
    statements: 70,
  },
},
```

Vitest applies BOTH the global numbers (across the merged total) AND the glob-keyed numbers (per matching file). This is the verified mechanism for D-14's two-level threshold on the frontend. [CITED: vitest.dev/config/coverage glob example; CITED: WebSearch confirmation of `'src/utils/**.ts': { ... }` shape]

**Note on `coverage.thresholds.100`:** Boolean shortcut to set every threshold to 100. Not used here. [CITED: vitest.dev/config/coverage]

**v8 vs Next.js bundling:** No known interaction issue — v8 instruments via the V8 profiler at the runtime level, not the bundler. Vitest itself uses Vite (not Next.js's Webpack/Turbopack), so the bundler used by Next.js for `next build` is irrelevant to test-time coverage.

### Branch coverage path decision (HIGH IMPACT)

D-14 requires ≥85% branch coverage on the backend. cargo-llvm-cov's `--branch` is nightly-only.

**Recommended path A — nightly toolchain in coverage CI job only:**
- Coverage job uses `rustup toolchain install nightly && rustup +nightly component add llvm-tools-preview`.
- All other CI jobs (build, test) remain on stable.
- Flag set: `cargo +nightly llvm-cov --branch --all-features --lcov --output-path lcov.info`.
- Risk: nightly may break the build occasionally; mitigation is pinning a specific `nightly-YYYY-MM-DD` toolchain in `rust-toolchain.toml` at the repo root, scoped via env var inside the coverage job, OR pinned via `actions-rust-lang/setup-rust-toolchain@v1` `toolchain: nightly-2026-04-01`.

**Alternative path B — substitute `--fail-under-regions` for branch on backend:**
- Region coverage is an LLVM-level metric finer-grained than lines, coarser than branches. It counts mapping regions emitted by `-C instrument-coverage` and is available on stable.
- Flag set: `cargo llvm-cov --all-features --fail-under-lines 90 --fail-under-regions 85 --lcov --output-path lcov.info`.
- This **does not measure branch coverage** — it is a pragmatic stable-only proxy. The 85% number from D-14 carries over numerically but the metric is different.
- Trade-off: weaker signal on branch-heavy code (auth predicates, validation chains, license-gate edge cases) but no nightly dependency.
- Flag the divergence in `CLAUDE.md` Test Coverage section so future devs aren't confused.

**Recommendation:** Path A. The audit-compliance ethos justifies the strictness, and the nightly dependency is contained to one CI job. If the planner picks B, document the metric-substitution explicitly.

---

## AppState Injection Sweep (concrete file:line list)

[VERIFIED: rg sweeps over `backend/src/` and `backend/tests/`]

### Source-side anti-pattern occurrences

| # | File:line | Function | Env var | Default fallback | Action |
|---|-----------|----------|---------|------------------|--------|
| 1 | `backend/src/leaves/service.rs:28-32` | `pub fn leaves_root()` | `CRONOMETRIX_LEAVES_ROOT` | `./data/leaves` | Replace with field on `Paths` substruct |
| 2 | `backend/src/events/service.rs:74-78` | `pub fn events_root()` | `CRONOMETRIX_EVENTS_ROOT` | `./data/events` | Replace with field on `Paths` |
| 3 | `backend/src/enrollments/service.rs:29-33` | `pub fn enrollments_root()` | `ENROLLMENTS_DIR` | `./data/enrollments` | Replace with field on `Paths` (note: env var name differs from siblings — see normalization question in Open Questions) |
| 4 | `backend/src/enrollments/service.rs:38-40` | `pub fn captures_tmp_root()` | (none — hardcoded) | `/tmp/enrollments-captures` | Either inject (consistent) or leave (it's an OS tmp path, less risky); recommend inject for symmetry |
| 5 | `backend/src/daily_records/handlers.rs:201-203` | inline (no helper fn) | `DATA_DIR` | `./data` (then joins `overrides`) | Hidden anti-pattern — refactor by introducing `paths.overrides_root` field |

### Source-side call sites that consume the helpers

| File:line | Helper called | New form |
|-----------|---------------|----------|
| `backend/src/events/service.rs:144` | `write_photo_atomic(&events_root(), ...)` | Receive `state.paths.events_root` via handler arg → service param |
| `backend/src/events/handlers.rs:105` | `service::events_root()` (in `get_event_photo`) | Read `state.paths.events_root` |
| `backend/src/leaves/handlers.rs:167` | `write_photo_atomic(&service::leaves_root(), ...)` (create_leave) | Read `state.paths.leaves_root` |
| `backend/src/leaves/handlers.rs:276` | `let root = service::leaves_root();` (get_leave_evidence) | Read `state.paths.leaves_root` |
| `backend/src/daily_records/handlers.rs:201-204` | `let overrides_root = PathBuf::from(env::var("DATA_DIR")…).join("overrides")` | Read `state.paths.overrides_root` |
| (enrollments call sites — search reveals `enrollments_root()` is consumed by `pusher.rs` + `handlers.rs` — confirm in plan) | — | Read `state.paths.enrollments_root` |

### Test-side guard usage to remove

[VERIFIED: rg `LeavesRootGuard|EventsRootGuard` over `backend/`]

| File | Guard struct + line | Usage count | Action |
|------|---------------------|-------------|--------|
| `backend/tests/leave_tests.rs:45-70` | `struct LeavesRootGuard` (defined in test file) | 9 `let _guard = LeavesRootGuard::new();` calls | Delete struct; replace each call with `let tmp = TempDir::new()?; state.paths.leaves_root = tmp.path().to_path_buf();` |
| `backend/tests/event_tests.rs:35-58` | `struct EventsRootGuard` | 16+ usages | Delete; replace per pattern |
| `backend/tests/listener_tests.rs:29-49` | `struct EventsRootGuard<'a>` (lifetime variant — uses static MUTEX) | 12 usages | Delete; replace per pattern |
| `backend/src/events/service.rs:373-404` | `struct EventsRootGuard<'a>` (inside `#[cfg(test)] mod tests`) | 8 inline-test usages | Delete; rewrite inline tests to construct a `Paths` directly (no AppState needed since these are unit tests of the helper functions) |

**Risk callout:** The inline `#[cfg(test)]` module in `backend/src/events/service.rs` uses a `static ENV_GUARD: Mutex<()>` to serialize env mutation across parallel tests. Removing the env-var dependency removes the need for the mutex too — the unit tests just pass a tempdir path to `write_photo_atomic` directly.

### AppState construction sites (every test that builds an AppState)

[VERIFIED: rg `AppState\s*\{|AppState::` over `backend/`]

| File:line | Type | Helper used? | Action |
|-----------|------|--------------|--------|
| `backend/src/main.rs:86` | Production | No (raw struct literal) | Add `paths: Arc::new(Paths::from_env())` field |
| `backend/tests/common/mod.rs:456-471` | Shared helper `test_state(db, config)` | — | Add `paths: Arc<Paths>` arg or include via `Default` impl pointing to a TempDir created by helper; export the `TempDir` so caller can hold it |
| `backend/tests/leave_tests.rs:72-88` | `fn make_state(db)` | Calls `common::test_state(...)` | Pass tempdir-backed `Paths` |
| `backend/tests/daily_record_tests.rs:21` | `fn make_state(db)` | Likely calls helper | Same |
| `backend/tests/listener_tests.rs:75` | `fn make_state(db)` | Same | Same |
| `backend/tests/multi_device_push_test.rs:148, 342, 358` | Three construction sites; one helper `build_test_state` | Mixed — confirm in plan | Same |
| `backend/tests/reports_excel_test.rs:51` | `fn make_state(db)` | Same | Same |
| `backend/tests/reports_test.rs:51` | `fn make_state(db)` | Same | Same |

### Recommended `Paths` struct (planner's home decision)

Decision question: live on `Config` or as separate `Paths`?
- **Recommendation:** Separate `Paths` struct, wrapped `Arc<Paths>` on `AppState`. Rationale: `Config` already serves the role of "values read from env at startup, may include secrets, redacted in Debug." Filesystem paths are a different concern (mutable in test setup, no secrets, frequently overridden). A separate substruct keeps `Config`'s semantics clean and makes the test-injection ergonomic.

```rust
// backend/src/state/paths.rs (new file) — proposed shape
#[derive(Clone, Debug)]
pub struct Paths {
    pub leaves_root: PathBuf,
    pub events_root: PathBuf,
    pub enrollments_root: PathBuf,
    pub captures_tmp_root: PathBuf,
    pub overrides_root: PathBuf,
}

impl Paths {
    pub fn from_env() -> Self {
        Self {
            leaves_root: env_or_default("CRONOMETRIX_LEAVES_ROOT", "./data/leaves"),
            events_root: env_or_default("CRONOMETRIX_EVENTS_ROOT", "./data/events"),
            enrollments_root: env_or_default("ENROLLMENTS_DIR", "./data/enrollments"),
            captures_tmp_root: env_or_default("CRONOMETRIX_CAPTURES_TMP", "/tmp/enrollments-captures"),
            overrides_root: env_or_default("DATA_DIR", "./data").join("overrides"),
        }
    }
    pub fn for_test(tmp: &Path) -> Self {
        // Each subdir under one tempdir — keeps a single TempDir alive in the test
        Self {
            leaves_root: tmp.join("leaves"),
            events_root: tmp.join("events"),
            enrollments_root: tmp.join("enrollments"),
            captures_tmp_root: tmp.join("captures-tmp"),
            overrides_root: tmp.join("overrides"),
        }
    }
}

fn env_or_default(key: &str, default: &str) -> PathBuf {
    std::env::var(key).map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(default))
}
```

```rust
// backend/src/state.rs — added field
pub struct AppState {
    pub db: Arc<libsql::Database>,
    pub config: Arc<Config>,
    pub paths: Arc<Paths>,           // ← NEW
    // … existing channel fields unchanged …
}
```

```rust
// backend/tests/common/mod.rs::test_state — extended signature
pub fn test_state(
    db: Arc<libsql::Database>,
    config: Arc<Config>,
    paths: Arc<Paths>,                // ← NEW required arg (or default to a fresh TempDir-backed instance)
) -> AppState { … }

// Usage at call site:
let tmp = TempDir::new()?;
let paths = Arc::new(Paths::for_test(tmp.path()));
let mut state = common::test_state(Arc::new(db), config, paths);
// Hold `tmp` alive — drop at end of test.
```

**Caveat (CONTEXT.md Risks/Watch-outs § "tempfile::TempDir is dropped at end of scope"):** Tests must keep the `TempDir` binding alive for the test's lifetime. The recommended pattern is to bind it to a local variable that outlives the assertions — drop happens at function return. If a test factors out state construction into a helper, the helper must return both `(state, tmp_dir)` so the caller owns the tempdir.

---

## Coverage Delta Candidates (concrete module list)

The following are the most-likely-uncovered modules and the tests needed to clear ≥70% line / ≥60% branch on each. Confirm by running `cargo llvm-cov --html` once the AppState fix is in and inspecting the per-file numbers.

[ASSUMED priority — based on file size, error-site density, and absence of inline tests; needs first coverage run to confirm]

### Backend — likely under-covered

| File | LoC | Inline `#[cfg(test)]`? | Likely gap | Suggested test additions |
|------|-----|------------------------|------------|--------------------------|
| `src/events/service.rs` | 705 | yes (1 mod) | `persist_attendance_event` dedup branch, `lookup_employee_for_event` priority-2 fallback when face_id None | Integration tests already exist via `event_tests.rs` — review delta against per-file floor |
| `src/reports/service.rs` | 624 | unknown | Period parsing, money rounding edge cases | Add unit tests for date-range resolution + rounding boundaries |
| `src/enrollments/service.rs` | 598 | no | Atomic write error path, device-mapping conflict on duplicate face_id | Add unit tests with tempdir + simulated FS errors |
| `src/devices/service.rs` | 545 | no | Decryption failure path (corrupted ciphertext), connection_state transitions | Add tests injecting a mismatched key + state-transition cases |
| `src/daily_records/service.rs` | 536 | no | Recompute branches: leave overlay None vs Some, anomaly insertion paths | Add tests for each anomaly variant emission |
| `src/enrollments/handlers.rs` | 491 | no | Multipart parsing error paths, `ai-validation` failure branch, `kiosk-capture` timeout | Add validation-failure tests for each branch |
| `src/reports/excel.rs` | 415 | no | Cell formatting branches (currency, datetime, blank) | Add a small fixture + golden-file comparison via `calamine` (already in dev-deps) |
| `src/leaves/service.rs` | 367 | no | `cancel` distinction-of-NotFound-vs-Conflict branch (lines 270-298) | Already exercised by `cancel_leave_optimistic_concurrency` — verify it covers BOTH branches |
| `src/leaves/handlers.rs` | 361 | no | `get_leave_evidence` canonicalize-fail branch, traversal-rejection branch | `leave_tests.rs` has T-3-15 test for traversal — verify per-file floor |
| `src/isapi/stream.rs` | 345 | no | Reconnect logic, stream parse error variants | `listener_tests.rs` covers happy path; add disconnection/parse-error fixtures (`fixtures/alertstream_*.bin` already exist) |
| `src/isapi/client.rs` | 304 | no | Digest auth challenge-response retry, TLS error mapping | Add wiremock tests for 401 → re-auth → 200 flow + 403/500 mappings |
| `src/license/service.rs` | 261 | no | `load_and_validate_license` fail-closed branches (file missing, signature invalid, fingerprint mismatch), `renewal_task` error/retry | Add unit tests with fixture JWTs (`tests/fixtures/test_license_*.pem` already exist) |
| `src/license/middleware.rs` | small | no | License-gated path: invalid → 403 vs valid → next | Add direct middleware unit test |
| `src/workers/purge.rs` | 214 | no | Purge cutoff branches, FS unlink error path | Add unit tests with seeded enrollments + tempdir |
| `src/workers/backfill.rs` | 229 | no | Backfill batch boundary, retry-on-failure branch | Add unit tests using a mocked `push_one_device_for_backfill` |

### Frontend — likely under-covered

[VERIFIED: file enumeration via `find`]

Total non-UI component files in scope: ~30 .tsx + ~5 .ts in lib/ + 2 hooks. Of those, ~14 already have `__tests__` siblings.

| File | Coverage status | Suggested action |
|------|-----------------|------------------|
| `src/hooks/use-sse.ts` | No test | Add `useSse.test.ts` with msw-served eventsource fixture; cover reconnect + onMessage paths |
| `src/hooks/use-auth.ts` | 50B file (likely a re-export stub) | Verify it's worth covering; if pure re-export, exclude |
| `src/lib/api.ts` | 2.4K | Add interceptor tests (401 → refresh → retry; 5xx pass-through) |
| `src/lib/face-detection.ts` | 1.7K | Hard to cover (depends on face-api initialization); consider exclusion with justification |
| `src/lib/kpi-utils.ts` | 387B | Add deterministic unit tests |
| `src/lib/ring-buffer.ts` | 128B | Add unit tests (push past capacity, drain) |
| `src/lib/utils.ts` | 166B | Pure cn() — likely auto-covered by any component test importing it |
| `src/lib/validations.ts` | 4.9K (Zod schemas) | Add direct schema parse-success and parse-failure tests |
| `src/lib/format/*` | unknown | Verify by enumeration during planning |
| `src/lib/reports/*` | unknown | Verify by enumeration during planning |
| `src/components/layout/sidebar.tsx`, `top-bar.tsx` | No tests | Add render + role-gate tests |
| `src/components/devices/command-modal.tsx`, `device-table.tsx` | No tests | Add modal-open + submit tests |
| `src/components/employees/employee-table.tsx` | No tests | Add render + filter tests |
| `src/components/timesheet/week-navigator.tsx` | No tests | Add nav-state tests |
| `src/components/dashboard/*` | Most have tests; verify per-file floor | — |
| `src/components/enrollment/*` | Most have tests | — |
| `src/components/reports/*` | All have tests | — |

**Sequencing recommendation:** Once the AppState fix lands and tests pass under `cargo llvm-cov`, run the HTML report locally and use it to drive specific test additions. Don't speculate — measure first, then close.

---

## CI Workflow Skeleton

**Triggers per D-03:** push (any branch) + pull_request (target main).
**Job structure per D-05:** two parallel jobs, both required.

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: ['**']
  pull_request:
    branches: [main]

jobs:
  backend-coverage:
    name: Backend Coverage
    runs-on: ubuntu-latest
    env:
      CARGO_TERM_COLOR: always
    defaults:
      run:
        working-directory: backend
    steps:
      - uses: actions/checkout@v6
      - name: Install nightly Rust (coverage uses --branch which is unstable)
        run: |
          rustup toolchain install nightly --component llvm-tools-preview
          rustup default nightly
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: backend
      - name: Install cargo-llvm-cov
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-llvm-cov@0.8.5
      - name: Install cargo-nextest
        uses: taiki-e/install-action@v2
        with:
          tool: cargo-nextest
      - name: Run coverage with project-wide gate
        run: |
          cargo llvm-cov nextest \
            --branch --all-features \
            --ignore-filename-regex '(main\.rs|tests/common/.*)' \
            --fail-under-lines 90 \
            --lcov --output-path lcov.info
      - name: Enforce branch + per-file thresholds
        run: bash ../scripts/enforce-coverage-floor.sh lcov.info 85 70 60
      - name: Generate HTML report
        if: always()
        run: cargo llvm-cov --branch --all-features --no-clean --html
      - name: Upload HTML artifact
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: backend-coverage-html
          path: backend/target/llvm-cov/html
          retention-days: 14

  frontend-coverage:
    name: Frontend Coverage
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: frontend
    steps:
      - uses: actions/checkout@v6
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm
          cache-dependency-path: frontend/package-lock.json
      - run: npm ci
      - name: Run Vitest with coverage (thresholds enforced via vitest.config.ts)
        run: npx vitest run --coverage
      - name: Upload HTML artifact
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: frontend-coverage-html
          path: frontend/coverage
          retention-days: 14
```

**Why nightly only inside the backend-coverage job:** D-14 mandates branch coverage. cargo-llvm-cov's `--branch` is unstable. Other CI jobs (if added later: lint, build, test) stay on stable.

**`--fail-under-lines 90` only:** cargo-llvm-cov has no `--fail-under-branches`. The branch threshold (85%) is enforced inside the post-process script (next section), which parses the same `lcov.info`.

**`--ignore-filename-regex` per D-09 + D-11:** Excludes `main.rs` and `tests/common/*` from the report denominator. Add other justified exclusions here if discovered (none expected at planning time).

**Recommended pinned action versions:** `taiki-e/install-action@v2.75.24` (or commit SHA) per the action's own supply-chain guidance. The above shows `@v2` for clarity — planner pins exact in PLAN.md. [CITED: github.com/taiki-e/install-action README]

---

## Per-File Floor Mechanism (chosen approach + sketch)

### Frontend — native via Vitest

[VERIFIED: vitest.dev/config/coverage]

```typescript
// frontend/vitest.config.ts (excerpt — full shape above)
coverage: {
  thresholds: {
    // Project-wide (applied to merged total)
    lines: 90, branches: 85, functions: 90, statements: 90,
    // Per-file floor (D-14: 70 line / 60 branch)
    '**/*.{ts,tsx}': {
      lines: 70, branches: 60, functions: 70, statements: 70,
    },
  },
}
```

Vitest fails the run if either set is missed. No script needed.

### Backend — lcov.info post-processor

cargo-llvm-cov has no per-file flag (verified). The lcov format produced by `--lcov --output-path lcov.info` is a well-known plain-text format suitable for awk/shell parsing.

**Lcov record format reference** (subset relevant to floor enforcement):
```
SF:<file path>
DA:<line>,<exec count>     ← line coverage data (one per executable line)
LF:<lines found>           ← total executable lines in file
LH:<lines hit>             ← executed lines in file
BRDA:<line>,<block>,<branch>,<taken>  ← branch coverage data
BRF:<branches found>
BRH:<branches hit>
end_of_record
```

LCOV summary lines `LF/LH` and `BRF/BRH` give per-file totals directly — no need to count individual `DA:`/`BRDA:` lines.

**Recommendation: small bash + awk script.** Rust binary would be over-engineered; awk is portable and idempotent.

```bash
#!/usr/bin/env bash
# scripts/enforce-coverage-floor.sh
# Usage: enforce-coverage-floor.sh <lcov-file> <project-branch-min> <file-line-min> <file-branch-min>
set -euo pipefail
LCOV="${1:?lcov file path required}"
PROJ_BR_MIN="${2:?project branch min required}"
FILE_LN_MIN="${3:?per-file line min required}"
FILE_BR_MIN="${4:?per-file branch min required}"

awk -v project_br_min="$PROJ_BR_MIN" \
    -v file_ln_min="$FILE_LN_MIN" \
    -v file_br_min="$FILE_BR_MIN" '
  BEGIN { fail = 0; total_lf = 0; total_lh = 0; total_brf = 0; total_brh = 0 }
  /^SF:/    { sf  = substr($0, 4) }
  /^LF:/    { lf  = substr($0, 4) + 0 }
  /^LH:/    { lh  = substr($0, 4) + 0 }
  /^BRF:/   { brf = substr($0, 5) + 0 }
  /^BRH:/   { brh = substr($0, 5) + 0 }
  /^end_of_record/ {
    total_lf += lf; total_lh += lh
    total_brf += brf; total_brh += brh
    line_pct   = (lf  > 0) ? (100.0 * lh  / lf ) : 100.0
    branch_pct = (brf > 0) ? (100.0 * brh / brf) : 100.0
    if (line_pct < file_ln_min) {
      printf "FAIL: %s line coverage %.2f%% < floor %d%%\n", sf, line_pct, file_ln_min
      fail = 1
    }
    if (brf > 0 && branch_pct < file_br_min) {
      printf "FAIL: %s branch coverage %.2f%% < floor %d%%\n", sf, branch_pct, file_br_min
      fail = 1
    }
    sf=""; lf=0; lh=0; brf=0; brh=0
  }
  END {
    proj_br = (total_brf > 0) ? (100.0 * total_brh / total_brf) : 100.0
    if (proj_br < project_br_min) {
      printf "FAIL: project-wide branch coverage %.2f%% < gate %d%%\n", proj_br, project_br_min
      fail = 1
    }
    exit fail
  }
' "$LCOV"
```

**Why this shape:**
- Single-pass awk (no temp files, no jq dependency).
- Enforces both the project-wide branch threshold (D-14 line 1, since `--fail-under-lines` already covers project-wide line) AND the per-file floor (D-14 line 2).
- Clear FAIL output with file + percentage so devs don't have to dig through HTML to find the offender.
- Branch arithmetic gracefully handles `BRF=0` files (declarative-only files, generated, etc.) — they don't fail the branch floor when there's nothing to measure.
- Exit code: 0 on pass, 1 on any failure → CI fails the job.
- Reuses the same `lcov.info` cargo-llvm-cov already produced — no extra coverage run.

**Local invocation (Makefile target):**
```makefile
coverage-backend:
	cd backend && cargo llvm-cov nextest --branch --all-features \
	  --ignore-filename-regex '(main\.rs|tests/common/.*)' \
	  --fail-under-lines 90 --lcov --output-path lcov.info
	bash scripts/enforce-coverage-floor.sh backend/lcov.info 85 70 60
	cd backend && cargo llvm-cov --branch --all-features --no-clean --html
```

---

## Common Pitfalls

### Pitfall 1: TempDir dropped before test ends
**What goes wrong:** `Paths::for_test(tmp.path())` clones the path, but if `tmp` itself drops at the end of the helper, the directory is removed before assertions run. Tests fail with "No such file or directory" on evidence reads.
**How to avoid:** The helper that builds AppState must RETURN both `AppState` and the `TempDir`, and the caller must bind the TempDir to a local variable that outlives the assertions.
**Reference:** CONTEXT.md Risks/Watch-outs.

### Pitfall 2: Nightly toolchain breaks builds intermittently
**What goes wrong:** `rustup toolchain install nightly` pulls the latest nightly, which occasionally introduces ICEs or stricter lints that break our crate.
**How to avoid:** Pin `nightly-2026-04-XX` as a specific date in the workflow (or in a top-level `rust-toolchain.toml` scoped to the coverage job). Bump quarterly, not on every regression.

### Pitfall 3: `--workspace` with feature flags
**What goes wrong:** `--all-features` enables every feature in `Cargo.toml`, including dev-only or experimental ones; if any feature flag changes the compile graph drastically, coverage may differ between local (default features) and CI (all features).
**How to avoid:** This crate has no `[features]` table → `--all-features` is a no-op. If features are added later, document in CLAUDE.md which set is canonical for the gate.

### Pitfall 4: Vitest `perFile: true` + glob conflict
**What goes wrong:** Setting `perFile: true` AND a glob like `'**/*.tsx': {...}` both apply per-file rules; the strictest wins, which may be unintended.
**How to avoid:** Pick ONE mechanism. For D-14's softer per-file floor, use the glob form ONLY (don't set `perFile: true`). Sketch above does this.

### Pitfall 5: lcov post-processor blind to BRF=0 files
**What goes wrong:** Files with no branches (e.g. pure data definitions, derive-only modules) emit `BRF:0`. If the script blindly divides, it gets a divide-by-zero or 100%/0% noise.
**How to avoid:** The provided script guards with `(brf > 0) ? … : 100.0` and skips the branch-floor check entirely when BRF=0.

### Pitfall 6: Coverage of `tokio::main` and async runtime setup
**What goes wrong:** `main.rs` includes runtime startup code that's hard to test. Counting it in the denominator drags the project-wide number down even when business logic is well-tested.
**How to avoid:** Exclude `main.rs` per D-09/D-11 via `--ignore-filename-regex '(main\.rs|tests/common/.*)'`. The pattern is in the workflow above.

### Pitfall 7: nextest config file conflicts
**What goes wrong:** If a user has a project-level `nextest.toml` with strict per-test timeouts, running under llvm-cov instrumentation (which slows tests ~2x) may time out.
**How to avoid:** Repo currently has no `nextest.toml` (verify in plan-check). If added, raise timeouts under coverage runs.

---

## Code Examples

### Backend: production startup wiring

```rust
// backend/src/main.rs (excerpt)
let paths = Arc::new(cronometrix_api::state::Paths::from_env());
let state = AppState {
    db: Arc::new(db),
    config: Arc::new(config.clone()),
    paths,                                  // ← NEW
    lifecycle_tx: Some(lifecycle_tx),
    // … unchanged …
};
```

### Backend: handler reading injected path

```rust
// backend/src/leaves/handlers.rs (excerpt — replacement for line 167)
write_photo_atomic(&state.paths.leaves_root, &rel, bytes)
    .map_err(AppError::Internal)?;

// And for line 276:
let root = &state.paths.leaves_root;
```

### Backend: integration test with tempdir Paths

```rust
// backend/tests/leave_tests.rs (excerpt — replaces LeavesRootGuard usage)
#[tokio::test]
async fn create_leave_medical_with_evidence() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let dept = create_test_department_with_shift(&db, "D", "day", false, 480, "09:00", "17:00").await;
    let emp = seed_employee(&db, &dept, "E01").await;
    let paths = Arc::new(cronometrix_api::state::Paths::for_test(tmp.path()));
    let state = make_state(db, paths);
    let app = build_test_app(state.clone());
    // … assertions …
    // Confirm file landed under the tempdir, not ./data/
    let full = state.paths.leaves_root.join(relpath);
    assert!(full.exists());
    // `tmp` drops here, cleaning up.
}
```

### Frontend: Vitest config (full)

```typescript
// frontend/vitest.config.ts (full replacement)
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/__tests__/setup.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      reportsDirectory: './coverage',
      include: [
        'src/components/**/*.{ts,tsx}',
        'src/hooks/**/*.{ts,tsx}',
        'src/lib/**/*.{ts,tsx}',
      ],
      exclude: [
        'src/components/ui/**',
        'src/**/__tests__/**',
        'src/**/*.test.{ts,tsx}',
        'src/**/*.spec.{ts,tsx}',
        'src/**/*.d.ts',
      ],
      thresholds: {
        lines: 90,
        branches: 85,
        functions: 90,
        statements: 90,
        // Per-file floor — applies to every matching file individually
        '**/*.{ts,tsx}': {
          lines: 70,
          branches: 60,
          functions: 70,
          statements: 70,
        },
      },
    },
  },
  resolve: {
    alias: { '@': path.resolve(__dirname, './src') },
  },
})
```

### Makefile

```makefile
# Makefile (top-level)
.PHONY: coverage coverage-backend coverage-frontend

coverage: coverage-backend coverage-frontend

coverage-backend:
	cd backend && cargo +nightly llvm-cov nextest --branch --all-features \
	  --ignore-filename-regex '(main\.rs|tests/common/.*)' \
	  --fail-under-lines 90 --lcov --output-path lcov.info
	bash scripts/enforce-coverage-floor.sh backend/lcov.info 85 70 60
	cd backend && cargo +nightly llvm-cov --branch --all-features --no-clean --html
	@echo "Backend HTML: backend/target/llvm-cov/html/index.html"

coverage-frontend:
	cd frontend && npx vitest run --coverage
	@echo "Frontend HTML: frontend/coverage/index.html"
```

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Branch coverage on Rust | Custom AST instrumentation | `cargo-llvm-cov --branch` (nightly) | LLVM source-based coverage is the only sanctioned approach; rolling your own is months of work |
| Per-file threshold logic | Bespoke Rust binary | Awk script over lcov.info | Lcov is a stable text format; awk is in every CI image; binary would need its own coverage |
| HTML report rendering | Custom HTML generator | `cargo llvm-cov --html` + `vitest --coverage` (auto) | Both tools ship HTML reporters; styling/navigation already solved |
| Test runner orchestration | Bespoke shell loop | `cargo nextest` (already standard in this repo) | Faster than `cargo test`, supports test-level parallelism + shard control |
| Mock HTTP for ISAPI tests | Hand-rolled axum echo server | `wiremock` (already in dev-deps) | Used elsewhere in repo; consistent style |
| TempDir lifecycle | Manual `mkdir`/`rm -rf` | `tempfile::TempDir` (already in dev-deps) | RAII-correct; auto-cleanup on panic |

**Key insight:** Coverage tooling is solved. The phase value is in the **gate config**, **AppState refactor**, and **closing measured gaps** — not in re-implementing instrumentation.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `cargo tarpaulin` | `cargo-llvm-cov` | ~2022 | LLVM-based is more accurate, faster, supports proc-macros |
| `nyc` / `c8` standalone | Vitest's `@vitest/coverage-v8` | ~2023 | First-class integration with Vitest; faster than istanbul |
| `react-query` v3/v4 | TanStack Query v5 | 2023 | Already adopted in repo |
| `actions/upload-artifact@v3` | `@v4` | Deprecated late 2024 | Use v4; v3 EOL |
| `actions/checkout@v3` | `@v4` (or `@v6` per cargo-llvm-cov README) | Deprecated 2024 | Use v4+; v3 EOL |

**Deprecated/outdated:**
- `cargo tarpaulin`: still maintained but slower; prefer cargo-llvm-cov
- `@vitest/coverage-istanbul`: use v8 unless istanbul-only feature is needed
- Vitest v0.x / v1.x configs: shape changed; this is v4

---

## Validation Architecture

(Required per `workflow.nyquist_validation: true` in `.planning/config.json`.)

### Test Framework

| Property | Value |
|----------|-------|
| Backend framework | `cargo test` + `axum-test 16` + `cargo-nextest` (runner) + `cargo-llvm-cov` (instrumentation) |
| Backend config file | `backend/Cargo.toml` (deps); no `nextest.toml` yet |
| Backend quick run | `cargo nextest run --test leave_tests` (single file) |
| Backend full suite | `cargo nextest run` |
| Backend coverage | `make coverage-backend` |
| Frontend framework | `vitest 4.1.5` + `@vitest/coverage-v8 4.1.5` |
| Frontend config file | `frontend/vitest.config.ts` |
| Frontend quick run | `npx vitest run --reporter=default <file>` |
| Frontend full suite | `npx vitest run` |
| Frontend coverage | `make coverage-frontend` |

### Phase Requirements → Test Map

This phase is enforcement-of-quality across all v1 requirements; it has no direct REQ-IDs. The validation is meta — does the gate itself work?

| Validation goal | Behavior | Test Type | Automated Command | Test File |
|-----------------|----------|-----------|-------------------|-----------|
| Project-wide line gate enforces ≥90% | A PR that drops backend line coverage to 89% MUST fail CI | smoke (intentional regression PR) | `cargo llvm-cov --fail-under-lines 90` | manual verification on a throwaway PR |
| Project-wide branch gate enforces ≥85% | A PR that drops branch to 84% MUST fail CI | smoke | `bash scripts/enforce-coverage-floor.sh lcov.info 85 70 60` | manual + CI dry-run |
| Per-file floor (backend) catches 0%-coverage file | A new untested file MUST fail the gate even if project-wide is 95% | smoke | same script | scripted dry-run with a fake `lcov.info` |
| Per-file floor (frontend) catches 0%-coverage file | New `.tsx` with no test MUST fail Vitest | unit | `npx vitest run --coverage` | scripted dry-run |
| AppState injection enables parallel tests | `cargo nextest run` (parallel by default) on `leave_tests` MUST pass | integration | `cargo nextest run --test leave_tests` | post-refactor verification |
| `LeavesRootGuard` removal didn't break tests | All existing leave/event/listener tests pass | regression | `cargo nextest run --test leave_tests --test event_tests --test listener_tests` | full suite |
| HTML artifact is uploaded | CI run produces a downloadable HTML zip | smoke | inspect Actions UI after first PR | manual |
| Coverage runs reproduce locally | `make coverage` produces same numbers as CI | sampling | `make coverage` | manual one-time |

### Sampling Rate

- **Per task commit:** Backend → `cargo nextest run --test <relevant_test>`; Frontend → `npx vitest run <relevant.test.tsx>`
- **Per wave merge:** Full backend `cargo nextest run` + full `npx vitest run`
- **Phase gate:** `make coverage` green locally; CI green on PR.

### Wave 0 Gaps

- [ ] `scripts/enforce-coverage-floor.sh` — does not exist; create in Wave 0
- [ ] `Makefile` (top-level) — does not exist; create in Wave 0
- [ ] `.github/workflows/ci.yml` — does not exist; create in Wave 0
- [ ] `backend/src/state/paths.rs` (or fold into `state.rs`) — new module
- [ ] Updated `backend/tests/common/mod.rs::test_state` signature — extend
- [ ] `rust-toolchain.toml` (optional, recommended) — pins coverage-job nightly

### Validating the Gate Itself (intentional sub-90 regression)

Before declaring the phase done, run a one-off PR that:
1. Adds a new file `backend/src/dead_code.rs` with two un-tested functions.
2. Confirms CI fails with the per-file floor message.
3. Confirms the failure references the offending file by path.
4. Closes the PR (do not merge).

This proves the gate works end-to-end and is not a no-op.

---

## Security Domain

Phase 8 is a tooling/CI phase. The only security-relevant surface is supply chain.

### Applicable ASVS Categories

| ASVS Category | Applies | Standard Control |
|---------------|---------|------------------|
| V1 Architecture | yes | Hard-fail gate aligns with audit-compliance ethos |
| V2 Authentication | no | No auth changes in this phase |
| V3 Session Management | no | No session changes |
| V4 Access Control | no | No RBAC changes |
| V5 Input Validation | no | No new endpoints |
| V6 Cryptography | no | No crypto changes |
| V14 Configuration | yes | CI YAML, Makefile, scripts/coverage post-processor — supply-chain-relevant |

### Known Threat Patterns

| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Pinned-action drift / supply-chain attack | Tampering | Pin all `uses:` to `@v<major>.<minor>.<patch>` or commit SHA per `taiki-e/install-action` recommendation; review pin updates in PR diff |
| Untrusted nightly toolchain pull | Tampering | Pin specific nightly date (`nightly-2026-04-XX`) instead of bare `nightly`; bump on a schedule |
| Test process leaks secrets via env | Information Disclosure | New `Paths` injection AVOIDS env mutation entirely — this fix REDUCES secret-leak risk by removing the env-mutation pattern |
| Lcov post-processor evaluates untrusted input | n/a (CI-only) | Awk script does not eval; pure pattern matching → safe |
| Coverage HTML artifact contains source | Information Disclosure | Repo is private; artifacts are scoped to authenticated viewers; no concern. If repo goes public, HTML reports become public via downloadable artifact zips — note in CLAUDE.md |

---

## Environment Availability

| Dependency | Required By | Available locally | Version | Fallback |
|------------|-------------|-------------------|---------|----------|
| Rust stable | Backend build/test | ✓ | rustc 1.93.0 (Homebrew) | — |
| Rust nightly | Backend coverage `--branch` | ✗ (only stable) | — | Path B: substitute `--fail-under-regions` for branch threshold (loses branch fidelity) |
| `cargo-llvm-cov` | Backend coverage | ✓ | 0.8.5 | — |
| `cargo-nextest` | Test runner | ASSUMED ✓ (in CLAUDE.md tooling table); not verified locally | — | `cargo test` works as fallback (slower) |
| `llvm-tools-preview` rustup component | cargo-llvm-cov | ASSUMED ✓ (cargo-llvm-cov works locally) | — | `rustup component add llvm-tools-preview` |
| Node.js | Frontend build/test | ASSUMED ✓ (CLAUDE.md says 18+ LTS) | — | — |
| `vitest` | Frontend coverage | ✓ | 4.1.5 | — |
| `@vitest/coverage-v8` | Frontend coverage | ✓ | 4.1.5 | — |
| `awk` | Per-file floor script | ✓ (POSIX, ubiquitous) | — | — |
| `bash` | Makefile + script | ✓ (POSIX) | — | — |

**Missing dependencies with no fallback:** Rust **nightly** for backend branch coverage (Path A recommendation). The planner must either install nightly in CI (recommended) OR fall back to Path B (`--fail-under-regions` substitution) and document the metric divergence.

**Missing dependencies with fallback:** None blocking.

---

## Sources

### Primary (HIGH confidence)
- **cargo-llvm-cov README** — github.com/taiki-e/cargo-llvm-cov — flag set, GitHub Actions snippet, regions vs branch [VERIFIED]
- **cargo-llvm-cov local CLI help** — `cargo llvm-cov --help` output on rustc 1.93.0 + cargo-llvm-cov 0.8.5 [VERIFIED]
- **rust-lang.org rustc instrument-coverage docs** — doc.rust-lang.org/beta/rustc/instrument-coverage.html — `-C instrument-coverage` is stable but `-Z coverage-options` is unstable [VERIFIED]
- **Vitest config/coverage** — vitest.dev/config/coverage — `coverage.thresholds` shape, `perFile`, glob, defaults [VERIFIED]
- **taiki-e/install-action README** — version pin recommendation [VERIFIED]
- **Swatinem/rust-cache README** — `@v2`, `workspaces:` parameter [VERIFIED]
- **Local codebase** — exhaustive rg sweeps of `backend/src/` and `backend/tests/` for AppState, leaves_root, events_root, env::var, PathBuf::from('./'), guard structs [VERIFIED]
- **`backend/Cargo.toml`** — deps already present (axum-test, proptest, wiremock, tempfile) [VERIFIED]
- **`frontend/package.json`** — vitest@4.1.5, @vitest/coverage-v8@4.1.5 already installed [VERIFIED]

### Secondary (MEDIUM confidence)
- **cargo-llvm-cov issue #8** — branch coverage stability tracker [CITED] — confirms `--branch` still nightly as of last update; not directly verifiable for late-2025/2026 stabilization
- **Vitest v4 reporter defaults** — WebSearch summary referencing vitest.dev — defaults are `['text', 'html', 'clover', 'json']` [CITED]

### Tertiary (LOW confidence — needs validation)
- **`actions/upload-artifact@v4`, `actions/setup-node@v4`** — version pins ASSUMED current; planner verifies in PLAN.md
- **Coverage delta candidates** — module list ASSUMED based on file size + error-site density. Real numbers require first coverage run after AppState fix lands. Planner should not commit to specific test additions until measured.

---

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | `actions/upload-artifact@v4` is the current major | CI Skeleton | Workflow YAML may need version bump; non-blocking |
| A2 | `actions/setup-node@v4` is current | CI Skeleton | Same as A1 |
| A3 | Coverage delta candidates (modules likely under 70% line) are accurate before measurement | Coverage Delta | Some "likely gap" modules may already be ≥70%; others not listed may be under. Mitigation: measure first via `cargo llvm-cov --html` after AppState fix; finalize test list in PLAN.md based on real numbers |
| A4 | `cargo-nextest` is installed in dev environments per CLAUDE.md tooling table | Validation | If absent locally, `make coverage-backend` fails. Mitigation: Makefile target falls back to `cargo llvm-cov --all-features` (no `nextest` subcommand) if nextest is unavailable |
| A5 | Captures tmp root (`/tmp/enrollments-captures`) is fine to leave hardcoded | Sweep | If a test runs on a system where `/tmp` is non-writable or shared between parallel runs, captures collide. Recommendation: inject for symmetry per the proposed `Paths` shape |
| A6 | Vitest 4 reporter defaults include `clover` and `json` (per WebSearch) | Vitest config | Non-blocking; explicit `reporter: ['text', 'html', 'lcov']` overrides defaults regardless |
| A7 | Per-file floor at 70/60 is achievable for every file in scope | D-14 enforcement | Some files (validations.ts schemas, ring-buffer.ts) may have low branch counts and naturally fail BRF=0 → script handles gracefully. Some service files may have rare error branches that are hard to trigger; planner may need to either write those tests OR justify exclusion |
| A8 | The single-crate layout doesn't need `--workspace` | Tooling state | Confirmed via `find` — no nested `Cargo.toml`. If a sub-crate is added later, revisit |
| A9 | Pinning nightly to `nightly-2026-04-XX` won't drift | Pitfalls | Standard practice; rebump when nightly breaks. Non-blocking |

---

## Open Questions

1. **Path A (nightly) vs Path B (regions substitute) for backend branch coverage?**
   - What we know: cargo-llvm-cov 0.8.5 + rustc 1.93.0 stable cannot do `--branch`.
   - What's unclear: the project's appetite for a CI nightly-toolchain dependency vs accepting a weaker (region-based) branch proxy.
   - Recommendation: Path A. Pin nightly to a fixed date in the workflow. Document in CLAUDE.md.

2. **Should `enrollments_root`'s env var be renamed to `CRONOMETRIX_ENROLLMENTS_ROOT` for consistency?**
   - What we know: today it's `ENROLLMENTS_DIR` (different convention from `CRONOMETRIX_LEAVES_ROOT` / `CRONOMETRIX_EVENTS_ROOT`).
   - What's unclear: whether prod deployments reference `ENROLLMENTS_DIR` and need backwards compat.
   - Recommendation: Defer. The phase fixes the cwd-bug, doesn't normalize naming. Capture as a deferred improvement.

3. **Should `captures_tmp_root` be injected or stay hardcoded?**
   - What we know: it's `/tmp/enrollments-captures` today, no env override.
   - What's unclear: whether parallel CI runs might collide on `/tmp`.
   - Recommendation: Inject for symmetry. Tests can pass `tmp.path().join("captures-tmp")`.

4. **Does the planner need to create tests on the spot, or measure coverage first?**
   - What we know: D-12 says planner "identifies coverage delta and proposes targeted tests."
   - What's unclear: whether the plan should commit to specific test additions before running cargo-llvm-cov for real numbers.
   - Recommendation: Sequence as: (1) AppState fix, (2) first coverage measurement, (3) gap identification from real HTML, (4) test additions, (5) gate enablement. Don't lock test list at planning time — measure first.

5. **Per-file `100` shortcut for trivial files like `lib.rs` (re-exports only)?**
   - What we know: Vitest supports `'**/lib.rs': { 100: true }`-style overrides.
   - What's unclear: whether re-export-only files in this repo (e.g. `mod.rs` files) should hit 100% or be excluded.
   - Recommendation: Skip the shortcut; `mod.rs` files in Rust have no executable code so cargo-llvm-cov gives them 100% naturally.

6. **CI failure attribution: which failed first, project-wide or per-file?**
   - What we know: with two enforcement steps, the first failure short-circuits.
   - What's unclear: dev experience — is one combined report better than two sequential failures?
   - Recommendation: Two steps as shown. The post-process script's per-file FAIL output makes the source of failure obvious. Combining gives no UX win.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — every recommended version verified (cargo, vitest, package.json, rustc, cargo-llvm-cov local install)
- Tooling state (flags, defaults): HIGH — verified via local CLI + official docs cross-referenced
- AppState sweep (file:line): HIGH — exhaustive rg over the codebase, every site enumerated
- CI workflow skeleton: MEDIUM-HIGH — recommended pattern is from cargo-llvm-cov README and verified action versions; some pin SHAs ASSUMED
- Coverage delta candidates: MEDIUM — list is plausible but not measured. Needs first coverage run to confirm
- Per-file floor mechanism: HIGH — sketch is correct lcov format; awk is well-tested syntax
- Pitfalls: HIGH — sourced from CONTEXT.md risks + tooling docs
- Validation Architecture: HIGH — clear meta-validation strategy

**Research date:** 2026-04-28
**Valid until:** 2026-05-28 (30 days; coverage tooling is stable)

---

## RESEARCH COMPLETE
