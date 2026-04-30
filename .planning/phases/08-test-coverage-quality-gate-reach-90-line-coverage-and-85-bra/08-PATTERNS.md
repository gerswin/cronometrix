# Phase 8: Test Coverage & Quality Gate — Pattern Map

**Mapped:** 2026-04-28
**Files analyzed:** 16 (backend src: 5, backend tests: 4, frontend: 1, top-level/CI: 4, docs: 2)
**Analogs found:** 13 / 16 (3 files are greenfield CI/build artifacts with no in-repo analog — pattern drawn from RESEARCH.md § CI Workflow Skeleton + § Per-File Floor Mechanism)

---

## File Classification

### Backend — AppState refactor + path injection

| New / Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `backend/src/state.rs` | state struct (modified — add `paths: Arc<Paths>` field) | request-response (state injection) | `backend/src/state.rs` itself (existing `config: Arc<Config>` field) | exact (self-pattern) |
| `backend/src/state/paths.rs` (NEW; promote `state.rs` → `state/mod.rs` if module split) | config substruct + `from_env()` + `for_test()` | config | `backend/src/config.rs` §`Config::from_env` (lines 56–119) | exact (env-or-default + Debug-redact pattern) |
| `backend/src/leaves/service.rs` (modified — remove `pub fn leaves_root()` lines 28–32) | service | file-I/O | `backend/src/leaves/service.rs` itself (the function being deleted) | self-replace |
| `backend/src/leaves/handlers.rs` (modified — lines 167, 276 read `state.paths.leaves_root`) | controller | request-response + file-I/O | `backend/src/devices/handlers.rs` (existing `State<AppState>` field reads) | exact |
| `backend/src/events/service.rs` (modified — remove `pub fn events_root()` lines 74–78; remove inline `EventsRootGuard` lines 365–404) | service + inline test refactor | file-I/O | self (delete pattern) | self-replace |
| `backend/src/events/handlers.rs` (modified — line 105 reads `state.paths.events_root`) | controller | request-response + file-I/O | `backend/src/devices/handlers.rs` | exact |
| `backend/src/enrollments/service.rs` (modified — remove `enrollments_root()` lines 29–33 + `captures_tmp_root()` lines 38–40) | service | file-I/O | self | self-replace |
| `backend/src/daily_records/handlers.rs` (modified — replace inline env read at lines 201–203 with `state.paths.overrides_root`) | controller | request-response + file-I/O | `backend/src/leaves/handlers.rs` line 167 (post-fix shape) | exact |
| `backend/src/main.rs` (modified — line 86 AppState literal grows `paths: Arc::new(Paths::from_env())`) | app entrypoint | startup | `backend/src/main.rs` itself (existing `config: Arc::new(config.clone())` line 88) | exact (self-pattern) |

### Backend — test-side cleanup

| New / Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `backend/tests/common/mod.rs` (modified — extend `pub fn test_state` signature with `paths: Arc<Paths>` arg) | test harness | request-response | `backend/tests/common/mod.rs` itself §`test_state` (lines 455–471) | self-extend |
| `backend/tests/common/test_state.rs` OR inline in `mod.rs` (NEW helper — `make_state_with_tmpdir() -> (AppState, TempDir)`) | test fixture builder | file-I/O | `backend/tests/common/mod.rs::test_state` (lines 455–471) + `backend/tests/leave_tests.rs::LeavesRootGuard` (lines 45–70 — pattern to replace) | role-match |
| `backend/tests/leave_tests.rs` (modified — delete `LeavesRootGuard` lines 45–70; switch 9 call sites to tempdir-Paths pattern) | integration tests | file-I/O | `backend/tests/event_tests.rs` (post-fix shape — same change) | parallel-refactor |
| `backend/tests/event_tests.rs` (modified — delete `ENV_GUARD` line 33 + `EventsRootGuard` lines 35–65; switch 16+ call sites) | integration tests | file-I/O | `backend/tests/leave_tests.rs` (post-fix shape) | parallel-refactor |
| `backend/tests/listener_tests.rs` (modified — delete `ENV_GUARD` line 27 + `EventsRootGuard<'a>` lines 29–56; switch 12 call sites) | integration tests | file-I/O | `backend/tests/event_tests.rs` (post-fix shape) | parallel-refactor |

### Frontend — Vitest coverage config

| New / Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `frontend/vitest.config.ts` (modified — extend `test:` with `coverage: { provider, reporter, include, exclude, thresholds }`) | config | build-time | `frontend/vitest.config.ts` itself (current 14-line minimal config) | self-extend |

### Top-level — build + CI + scripts (greenfield)

| New / Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `Makefile` (NEW, top-level) | build orchestration | n/a | none in repo (no Makefile or justfile exists) — use RESEARCH § Makefile target sketch (lines 678–685) | none |
| `scripts/enforce-coverage-floor.sh` (NEW) | CI helper (lcov post-processor) | transform (text → exit-code) | none in repo (`scripts/` directory does not exist) — use RESEARCH § Per-File Floor Mechanism awk script (lines 624–667) | none |
| `.github/workflows/ci.yml` (NEW) | CI workflow | event-driven (push/PR) | none in repo (`.github/workflows/` does not exist) — use RESEARCH § CI Workflow Skeleton (lines 491–570) | none |

### Documentation

| New / Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---|---|---|---|---|
| `CLAUDE.md` (modified — add "Test Coverage" section after Conventions block; extend Conventions with path-injection rule) | docs | n/a | `CLAUDE.md` Conventions block (lines 185–189, GSD-managed `<!-- GSD:conventions-start -->` markers) | exact (self-extend within marker) |

---

## Pattern Assignments

### `backend/src/state/paths.rs` (NEW — config substruct + env-or-default constructors)

**Analog:** `backend/src/config.rs` §`Config` + `Config::from_env`

**Why this analog:** `Paths` plays the same role as `Config` — values read from env at startup, behind `Arc` on `AppState`, mutable in tests via a sibling constructor. The `from_env` + manual-`Debug` + secret-redaction + module-private helper pattern transfers directly. Filesystem paths carry no secrets, so `Debug` does not need redaction; the rest of the shape is reused verbatim.

**Imports pattern** (`backend/src/config.rs:1–4`):
```rust
use std::fmt;

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
```
For `Paths` the imports trim to `use std::path::PathBuf;` plus an optional `Result`-returning `from_env`.

**Struct + manual Debug** (`backend/src/config.rs:8–53`):
```rust
#[derive(Clone)]
pub struct Config {
    pub database_path: String,
    pub turso_url: String,
    // ... fields ...
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("database_path", &self.database_path)
            // redacted secrets ...
            .finish()
    }
}
```
For `Paths`, prefer `#[derive(Clone, Debug)]` directly (no secrets to redact). Same `Clone`-bound shape so it slots into `Arc<Paths>` cheaply.

**`from_env` pattern with env-or-default** (`backend/src/config.rs:57–119`):
```rust
impl Config {
    pub fn from_env() -> Result<Self> {
        let database_path = std::env::var("CRONOMETRIX_DB_PATH")
            .unwrap_or_else(|_| "cronometrix.db".to_string());

        let turso_url = std::env::var("TURSO_DATABASE_URL").unwrap_or_default();
        // ...
        let server_port = std::env::var("SERVER_PORT")
            .unwrap_or_else(|_| "3001".to_string())
            .parse::<u16>()
            .context("SERVER_PORT must be a valid port number")?;
        // ...
        Ok(Config { /* fields */ })
    }
}
```
Apply directly: every `Paths` field has an env var + a string default. `from_env()` is infallible (all paths) so it can return `Self` directly — no `Result` wrapper needed.

**Module-private helper for env-or-default reads** (`backend/src/config.rs:136–153`):
```rust
fn load_device_creds_key() -> Result<[u8; 32]> {
    let raw = std::env::var("DEVICE_CREDS_KEY")
        .context("DEVICE_CREDS_KEY environment variable is required")?;
    // ...
}
```
The Phase-8 equivalent is `fn env_or_default(key: &str, default: &str) -> PathBuf` (RESEARCH lines 396–398) — same module-private one-liner shape.

**Existing helper-function bodies to migrate verbatim into `Paths::from_env`:**
- `backend/src/leaves/service.rs:28–32` — `CRONOMETRIX_LEAVES_ROOT` → `./data/leaves`
- `backend/src/events/service.rs:74–78` — `CRONOMETRIX_EVENTS_ROOT` → `./data/events`
- `backend/src/enrollments/service.rs:29–33` — `ENROLLMENTS_DIR` → `./data/enrollments`
- `backend/src/enrollments/service.rs:38–40` — `captures_tmp_root` → `/tmp/enrollments-captures` (no env var today; planner adds `CRONOMETRIX_CAPTURES_TMP` for symmetry per RESEARCH line 380)
- `backend/src/daily_records/handlers.rs:201–203` — `DATA_DIR` → `./data` joined with `overrides`

---

### `backend/src/state.rs` (modified — add `paths: Arc<Paths>`)

**Analog:** `backend/src/state.rs` itself, existing `config: Arc<Config>` field (line 51).

**Why this analog:** The struct already carries one `Arc<Config>` field threaded from startup into every handler. Adding `paths: Arc<Paths>` is mechanically identical — same `Arc` wrap, same `from_env` initialiser, same field comment style.

**Existing field-with-comment pattern** (`backend/src/state.rs:48–68`):
```rust
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<libsql::Database>,
    pub config: Arc<Config>,
    pub lifecycle_tx: Option<LifecycleTx>,
    // ... 7 more fields with leading doc comments ...
}
```

**Add new field with doc-comment** following the existing rhythm of `lifecycle_tx`/`recompute_tx`/`license_valid` per-field rationales (lines 28–47, 56–65). Suggested position: directly after `config: Arc<Config>` since `Paths` is a peer of `Config`. Suggested doc comment style:

```rust
/// Phase 8 (08-XX-yy): filesystem roots for evidence and event JPEGs.
/// Injected via `Paths::from_env()` at startup; overridden in tests with a
/// per-test `TempDir`. Reading from env at use-site (the old
/// `leaves_root()` / `events_root()` helpers) was cwd-dependent and
/// process-globally racy — see CLAUDE.md Conventions § Filesystem-root injection.
pub paths: Arc<Paths>,
```

---

### `backend/src/main.rs` (modified — populate `paths` at startup)

**Analog:** `backend/src/main.rs` itself, existing `config: Arc::new(config.clone())` at line 88.

**Existing AppState construction** (`backend/src/main.rs:86–96`):
```rust
let state = AppState {
    db: Arc::new(db),
    config: Arc::new(config.clone()),
    lifecycle_tx: Some(lifecycle_tx),
    recompute_tx: Some(recompute_tx),
    event_broadcast: Some(event_tx),
    license_valid: license_valid.clone(),
    purge_tx: Some(purge_tx),
    backfill_tx: Some(backfill_tx),
    captures: cronometrix_api::enrollments::handlers::new_captures_map(),
};
```

**Insert one line** following the same pattern (RESEARCH line 728 sketch):
```rust
let paths = Arc::new(cronometrix_api::state::Paths::from_env());
let state = AppState {
    db: Arc::new(db),
    config: Arc::new(config.clone()),
    paths,                              // ← NEW (peer of config)
    // ...
};
```

---

### `backend/src/leaves/handlers.rs` + `backend/src/events/handlers.rs` + `backend/src/daily_records/handlers.rs` (modified — read paths from `state`)

**Analog:** `backend/src/leaves/handlers.rs` itself line 167 (the call site being replaced).

**Current call shape** (`backend/src/leaves/handlers.rs:163–172`):
```rust
let evidence_relpath = if let (Some(bytes), Some(ext)) =
    (evidence_bytes.as_ref(), evidence_ext)
{
    let rel = format!("{}.{}", Uuid::new_v4(), ext);
    write_photo_atomic(&service::leaves_root(), &rel, bytes)
        .map_err(AppError::Internal)?;
    Some(rel)
} else {
    None
};
```

**Post-fix shape** (RESEARCH lines 740–747 reference):
```rust
write_photo_atomic(&state.paths.leaves_root, &rel, bytes)
    .map_err(AppError::Internal)?;
```

`state` is already in scope as a `State<AppState>` axum extractor in every handler in these three files — no new arg threading required. Apply verbatim to:
- `backend/src/leaves/handlers.rs:167` (create_leave) — `service::leaves_root()` → `&state.paths.leaves_root`
- `backend/src/leaves/handlers.rs:276` (get_leave_evidence) — `let root = service::leaves_root();` → `let root = &state.paths.leaves_root;`
- `backend/src/events/handlers.rs:105` (get_event_photo) — same pattern → `&state.paths.events_root`
- `backend/src/daily_records/handlers.rs:201–204` — replace the 4-line inline env block with `&state.paths.overrides_root`

---

### `backend/src/events/service.rs` + `backend/src/leaves/service.rs` + `backend/src/enrollments/service.rs` (modified — delete `*_root()` helpers)

**No external analog needed** — these are pure deletions of:
- `backend/src/leaves/service.rs:28–32` `pub fn leaves_root() -> PathBuf`
- `backend/src/events/service.rs:74–78` `pub fn events_root() -> PathBuf`
- `backend/src/enrollments/service.rs:29–33` `pub fn enrollments_root() -> PathBuf`
- `backend/src/enrollments/service.rs:38–40` `pub fn captures_tmp_root() -> PathBuf`

**Inline `#[cfg(test)] mod tests` cleanup in `backend/src/events/service.rs:365–404`** — the `ENV_GUARD: Mutex<()>` + `EventsRootGuard` struct become unused once `events_root()` is gone. Inline tests that called the helper now construct a `PathBuf` from a per-test `TempDir` and pass it directly into `write_photo_atomic`:

**Pattern source (analog):** `backend/src/events/service.rs:156` — the existing `pub fn write_photo_atomic(root: &Path, relpath: &str, bytes: &[u8])` already takes `root` as a parameter, so passing a `tmp.path()` is the natural shape. The mutex existed solely because of env mutation.

```rust
// Replacement inline-test idiom (no env, no mutex):
let tmp = tempfile::TempDir::new().expect("tempdir");
write_photo_atomic(tmp.path(), "2026-04-28/abc.jpg", &MINI_JPEG).expect("write");
assert!(tmp.path().join("2026-04-28/abc.jpg").exists());
// `tmp` drops at end of test scope — auto-cleanup.
```

---

### `backend/tests/common/mod.rs` (modified — extend `test_state` signature)

**Analog:** `backend/tests/common/mod.rs` itself §`test_state` (lines 455–471).

**Current signature:**
```rust
#[allow(dead_code)]
pub fn test_state(
    db: std::sync::Arc<libsql::Database>,
    config: std::sync::Arc<cronometrix_api::config::Config>,
) -> cronometrix_api::state::AppState {
    cronometrix_api::state::AppState {
        db,
        config,
        lifecycle_tx: None,
        // ...
        captures: cronometrix_api::enrollments::handlers::new_captures_map(),
    }
}
```

**Post-fix signature** (RESEARCH lines 412–417):
```rust
pub fn test_state(
    db: Arc<libsql::Database>,
    config: Arc<cronometrix_api::config::Config>,
    paths: Arc<cronometrix_api::state::Paths>,    // ← NEW required arg
) -> cronometrix_api::state::AppState {
    AppState { db, config, paths, /* unchanged optional fields */ }
}
```

**New helper to add (RESEARCH § Caveat lines 419–426):** A `make_state_with_tmpdir(db, config) -> (AppState, TempDir)` convenience that owns the TempDir lifetime, so callers do not have to manage two bindings. Returning the `TempDir` is critical — the existing `LeavesRootGuard::_tmp` already demonstrates the same lifetime concern (`backend/tests/leave_tests.rs:46`).

```rust
// Suggested addition to backend/tests/common/mod.rs (or new test_state.rs)
#[allow(dead_code)]
pub fn test_state_with_tmpdir(
    db: Arc<libsql::Database>,
    config: Arc<cronometrix_api::config::Config>,
) -> (cronometrix_api::state::AppState, tempfile::TempDir) {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let paths = Arc::new(cronometrix_api::state::Paths::for_test(tmp.path()));
    let state = test_state(db, config, paths);
    (state, tmp)  // Caller MUST bind `tmp` to a local that outlives assertions.
}
```

---

### `backend/tests/leave_tests.rs` (modified — delete `LeavesRootGuard`, switch 9 call sites)

**Analog:** `backend/tests/leave_tests.rs` itself, current `LeavesRootGuard` (lines 45–70) — the pattern being deleted; replacement is the post-fix shape sketched in RESEARCH lines 752–768.

**Delete block (lines 43–70):**
```rust
struct LeavesRootGuard {
    prev: Option<String>,
    _tmp: tempfile::TempDir,
}
impl LeavesRootGuard {
    fn new() -> Self {
        let prev = std::env::var("CRONOMETRIX_LEAVES_ROOT").ok();
        let tmp = tempfile::TempDir::new().expect("tempdir");
        std::env::set_var("CRONOMETRIX_LEAVES_ROOT", tmp.path());
        LeavesRootGuard { prev, _tmp: tmp }
    }
}
impl Drop for LeavesRootGuard { /* restore prev */ }
```

**Replacement at every `let _guard = LeavesRootGuard::new();` call site (9 occurrences):**
```rust
let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
let app = build_test_app(state.clone());
// ... assertions ...
// `_tmp` drops at end of #[tokio::test] fn scope — auto-cleanup.
```

**Update `make_state` helper (lines 72–88)** — current signature `fn make_state(db: libsql::Database) -> AppState` becomes `fn make_state(db: libsql::Database) -> (AppState, TempDir)` OR is replaced inline by `test_state_with_tmpdir`. Planner picks; either works.

**Verifying the new shape works:** assertions that previously checked file existence under the env-var-rooted path now check `state.paths.leaves_root.join(relpath)` per RESEARCH line 765.

---

### `backend/tests/event_tests.rs` (modified — delete `ENV_GUARD` + `EventsRootGuard`, switch 16+ call sites)

**Analog:** Same as `leave_tests.rs` post-fix shape — both files end up in identical canonical form once their guard structs are gone.

**Delete block (lines 32–65):** `static ENV_GUARD: Mutex<()>` + `struct EventsRootGuard` + impls. Removing the `MutexGuard<'static, ()>` field is the central simplification — env mutation was the only reason for the mutex.

**Replacement pattern at `let guard = EventsRootGuard::new();` call sites:**
```rust
let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);
// `state.paths.events_root` is the per-test tempdir's `events` subdir.
// Direct file assertions: `assert!(state.paths.events_root.join("…").exists())`.
```

**Note:** `event_tests.rs::build_test_app` (lines 67–101) currently constructs config + state inline — refactor to call `test_state_with_tmpdir` instead, returning `(Router, AppState, TempDir)` so the caller owns the tempdir.

---

### `backend/tests/listener_tests.rs` (modified — delete `ENV_GUARD` + `EventsRootGuard<'a>`, switch 12 call sites)

**Analog:** Same as `event_tests.rs` post-fix shape.

**Delete block (lines 25–56):** `static ENV_GUARD: Mutex<()>` + `struct EventsRootGuard<'a>` (lifetime variant — uses `MutexGuard<'a, ()>`) + impls. Same simplification.

**Replacement:** Identical to `event_tests.rs`. The lifetime parameter `<'a>` was tied to the static-mutex guard — once removed, no lifetime juggling.

---

### `frontend/vitest.config.ts` (modified — add `coverage` block)

**Analog:** `frontend/vitest.config.ts` itself (current 14-line minimal config).

**Current full file:**
```typescript
import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/__tests__/setup.ts'],
  },
  resolve: {
    alias: { '@': path.resolve(__dirname, './src') },
  },
})
```

**Post-fix shape** (RESEARCH lines 779–818, with the per-file-floor glob form preferred over `perFile: true` per RESEARCH § Pitfall 4):
```typescript
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
        'src/components/ui/**',          // shadcn vendored copies (D-10)
        'src/**/__tests__/**',
        'src/**/*.test.{ts,tsx}',
        'src/**/*.spec.{ts,tsx}',
        'src/**/*.d.ts',
      ],
      thresholds: {
        // Project-wide gate (D-14 line 1)
        lines: 90,
        branches: 85,
        functions: 90,
        statements: 90,
        // Per-file floor (D-14 line 2 — softer than project gate)
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

**Critical:** Do NOT set `perFile: true` AND a glob entry — RESEARCH § Pitfall 4 (lines 704–706) flags this. Use glob form only.

---

### `Makefile` (NEW, top-level — no in-repo analog)

**Analog:** None in repo; pattern source is RESEARCH § Per-File Floor Mechanism (lines 678–685) + standard 3-target Make idiom.

**Pattern excerpt to copy from RESEARCH:**
```makefile
.PHONY: coverage coverage-backend coverage-frontend

coverage: coverage-backend coverage-frontend

coverage-backend:
	cd backend && cargo llvm-cov nextest --branch --all-features \
	  --ignore-filename-regex '(main\.rs|tests/common/.*)' \
	  --fail-under-lines 90 --lcov --output-path lcov.info
	bash scripts/enforce-coverage-floor.sh backend/lcov.info 85 70 60
	cd backend && cargo llvm-cov --branch --all-features --no-clean --html

coverage-frontend:
	cd frontend && npx vitest run --coverage
```

**Notes for planner:**
- `cargo llvm-cov nextest` subcommand is the cargo-nextest-aware variant per RESEARCH line 194; `cargo-nextest` is already in CLAUDE.md tooling table.
- `--branch` requires nightly (RESEARCH § Branch coverage path decision lines 284–301). Either pin nightly globally for the coverage target or guard with a local `cargo +nightly` invocation; planner picks.
- `bash scripts/…` invocation uses absolute repo-root path because `cd backend &&` changes cwd. Either drop the `cd` and use `cargo llvm-cov --manifest-path backend/Cargo.toml`, or use `bash $(CURDIR)/scripts/enforce-coverage-floor.sh backend/lcov.info 85 70 60` to be cwd-safe.

---

### `scripts/enforce-coverage-floor.sh` (NEW — no in-repo analog)

**Analog:** None in repo (`scripts/` directory does not exist). Pattern source is RESEARCH § Per-File Floor Mechanism (lines 624–667).

**Full script body to copy verbatim** (planner may rename variables; logic is the canonical lcov post-processor shape):
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

**Notes:**
- `set -euo pipefail` is mandatory — RESEARCH line 628.
- Pipefail-safe: awk emits to stdout, exit code propagates via `exit fail`.
- File mode after creation: `chmod +x scripts/enforce-coverage-floor.sh`.

---

### `.github/workflows/ci.yml` (NEW — no in-repo analog)

**Analog:** None in repo (`.github/workflows/` does not exist). Pattern source is RESEARCH § CI Workflow Skeleton (lines 491–570).

**Skeleton to copy** (verbatim from RESEARCH; planner pins exact action SHAs at plan time per RESEARCH line 578):
```yaml
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
    defaults:
      run:
        working-directory: backend
    steps:
      - uses: actions/checkout@v6
      - name: Install nightly Rust (--branch is unstable)
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
      - run: npx vitest run --coverage
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: frontend-coverage-html
          path: frontend/coverage
          retention-days: 14
```

**Notes:**
- Two parallel jobs per D-05; both required for the gate — set as required status checks in branch protection (manual GitHub UI step, not a file).
- Triggers per D-03: `push: ['**']` + `pull_request: [main]`.
- `actions/upload-artifact@v4` with `if: always()` per RESEARCH § Pitfall 6 (uploads even on red so devs can drill into the failing report).

---

### `CLAUDE.md` (modified — add "Test Coverage" section + extend Conventions)

**Analog:** `CLAUDE.md` Conventions block (lines 185–189) — uses GSD-managed markers `<!-- GSD:conventions-start source:CONVENTIONS.md -->` and `<!-- GSD:conventions-end -->`.

**Insertion approach:**

1. **Extend Conventions block** (replace placeholder line 188 "Conventions not yet established. …") with a Filesystem-root injection rule per D-23:
   ```markdown
   ### Filesystem-root injection
   Code that needs a filesystem root (evidence dir, photo dir, override dir, kiosk
   capture tmp) MUST read it from `state.paths.<field>` — never via
   `std::env::var(...)` at use-site, and never via `PathBuf::from("./data/…")`.
   The `Paths` substruct on `AppState` is populated once at startup by
   `Paths::from_env()` and overridden in tests via `Paths::for_test(tempdir)`.
   This eliminates cwd-dependence and the env-var process-global race.
   ```

2. **Add new top-level "Test Coverage" section** between Conventions and Architecture (or after Architecture — planner picks; existing GSD markers do not preclude either). Use D-22's required content list:
   - Install commands: `cargo install cargo-llvm-cov --version 0.8.5 --locked` + `rustup component add llvm-tools-preview` + nightly install per RESEARCH line 152.
   - Local commands: `make coverage`, `make coverage-backend`, `make coverage-frontend`.
   - Thresholds table: project-wide 90/85, per-file 70/60 (lines/branches).
   - Exclusion policy with rationale (per D-09/D-10/D-11).
   - HTML report locations: `backend/target/llvm-cov/html/index.html` and `frontend/coverage/index.html`.
   - CI gate: workflow file path + job names + how to read failing reports.

**Existing CLAUDE.md style to mirror:**
- Use `## Section / ### Subsection` heading depth (matches lines 24, 27, 36, 58 — Technology Stack subsections).
- Use Markdown tables for thresholds (matches the dozen tables already in the file, e.g. lines 28–35 Core Technologies table).
- Cite RESEARCH.md once at the section foot ("Source: `.planning/phases/08-…/08-RESEARCH.md`") — matches the Sources block at line 164.

---

## Shared Patterns

### State injection via `State<AppState>` extractor

**Source:** `backend/src/state.rs` (existing struct with `Arc<Config>` field) + every handler function in `backend/src/{leaves,events,daily_records,enrollments,devices}/handlers.rs`.

**Apply to:** Every handler that currently calls `service::*_root()` or reads a `*_DIR`/`*_ROOT` env var.

**Canonical reading pattern** (axum 0.8.x, single-arg function — Phase-7+ style):
```rust
pub async fn handler_name(
    State(state): State<AppState>,
    // ... other extractors ...
) -> Result<impl IntoResponse, AppError> {
    // Read injected path:
    write_photo_atomic(&state.paths.events_root, &rel, bytes)?;
    // ...
}
```

**Why this is the universal answer:** Three subsystems (leaves, events, daily_records) currently bypass the state pattern in three different shapes (helper fn, helper fn, inline env read). One pattern, applied to all three, deletes ~25 lines of duplicated env-var-reading boilerplate and fixes one whole class of bugs.

---

### TempDir lifetime ownership in tests

**Source:** `backend/tests/leave_tests.rs` `LeavesRootGuard::_tmp` field (line 47) — already shows the right idea (the guard owns the tempdir), but couples it to env-var mutation.

**Apply to:** Every `#[tokio::test]` that constructs an `AppState` for handler-level testing (every test in `leave_tests.rs`, `event_tests.rs`, `listener_tests.rs`, plus future tests).

**Canonical pattern** (RESEARCH § Caveat lines 419–426 + § Pitfall 1 lines 691–694):
```rust
#[tokio::test]
async fn my_handler_test() {
    let db = common::test_db().await;
    // ... seed users, departments, devices ...
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    //              ^^^^ MUST be bound to outlive the assertions below.
    let app = build_test_app(state.clone());

    // ... HTTP requests via tower::ServiceExt::oneshot ...
    // ... assertions, including filesystem checks against state.paths.* ...

    // `_tmp` drops here, removing the directory. Do NOT do
    //   `let _ = common::test_state_with_tmpdir(...).0;`
    // — that drops the TempDir immediately and the test fails non-deterministically.
}
```

**Why this is shared:** All three integration test files have ≥9 occurrences of the bad pattern; the fix is mechanically identical and benefits from a single shared helper.

---

### Env-or-default helper function

**Source:** `backend/src/config.rs:57–119` `Config::from_env` — every field follows the `std::env::var(KEY).unwrap_or_else(|_| default)` shape.

**Apply to:** `Paths::from_env` (RESEARCH lines 374–393).

```rust
fn env_or_default(key: &str, default: &str) -> PathBuf {
    std::env::var(key).map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(default))
}
```

**Why this is shared:** Every path field in the new `Paths` struct (5 fields) uses the same env-or-default shape. Factoring to one helper saves 5 × 3 = 15 lines and matches the existing `Config::from_env` rhythm.

---

### Manual `Debug` impl on AppState-attached structs (NOT NEEDED FOR `Paths`)

**Source:** `backend/src/config.rs:35–53` `impl Debug for Config` (manual redaction).

**Apply to:** `Paths` does NOT need this — none of its fields carry secrets. Use `#[derive(Clone, Debug)]` directly. This pattern is included for completeness so future struct additions follow the rule "if any field is a secret, manual Debug; else derive."

---

## No Analog Found

These three files are greenfield — no existing in-repo file plays the same role. Patterns drawn from RESEARCH.md only:

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `Makefile` | build orchestration | n/a | No Makefile or justfile in repo. Use RESEARCH § Per-File Floor Mechanism Makefile sketch (lines 678–685). |
| `scripts/enforce-coverage-floor.sh` | CI helper / lcov post-processor | text-transform → exit-code | No `scripts/` directory in repo. Use RESEARCH § Per-File Floor Mechanism awk script (lines 624–667) verbatim. |
| `.github/workflows/ci.yml` | CI workflow | event-driven | No `.github/workflows/` directory in repo. Use RESEARCH § CI Workflow Skeleton (lines 491–570) verbatim. |

---

## Metadata

**Analog search scope:**
- `backend/src/` — full sweep (state, config, leaves, events, daily_records, enrollments, main)
- `backend/tests/` — full sweep (common helpers, leave_tests, event_tests, listener_tests)
- `frontend/` — config files only (vitest.config.ts, package.json)
- Repo root — Makefile, scripts/, .github/ directory existence checks
- `.planning/phases/0[5-7]-*/0X-PATTERNS.md` — sibling phase PATTERNS for structural template (07 referenced)

**Files scanned:** 12 source files read in full or in targeted ranges; 4 directory existence checks; 1 sibling PATTERNS skim.

**Pattern extraction date:** 2026-04-28

**Sources cited:** 08-CONTEXT.md (D-01..D-23), 08-RESEARCH.md (CI skeleton, lcov post-processor, AppState sweep table, vitest config sketch, code examples lines 722–768), 08-VALIDATION.md (Wave 0 file list).
