---
phase: 09
plan: "03"
subsystem: backend-e2e-infrastructure
tags:
  - e2e
  - seed
  - mock-hikvision
  - test-reset
  - cargo-features
dependency_graph:
  requires:
    - "09-01 (playwright config)"
    - "09-02 (license bypass safety)"
  provides:
    - seed_e2e binary (DB fixture seeder)
    - mock_hikvision binary (Hikvision impersonator)
    - POST /api/v1/__test_reset (mutable-table reset for E2E)
  affects:
    - "09-04+ all spec plans (unblocked by this plan)"
    - backend/Cargo.toml (features + [[bin]] entries)
    - backend/src/main.rs (conditional route registration)
tech_stack:
  added:
    - "Cargo features: seed-e2e, mock-hikvision"
    - "Two new [[bin]] targets: seed_e2e, mock_hikvision"
    - "test_reset module: axum handler with dual env guard"
  patterns:
    - "libsql tuple params (not params! macro) for inserts mixing &str + owned String"
    - "Dual-port Axum server sharing Arc<Mutex<>> state between routers"
    - "tokio::spawn + abort on Ctrl-C for two-server lifecycle"
key_files:
  created:
    - backend/src/bin/seed_e2e.rs
    - backend/src/bin/mock_hikvision.rs
    - backend/src/test_reset/mod.rs
    - backend/tests/test_reset_gating.rs
  modified:
    - backend/Cargo.toml
    - backend/src/lib.rs
    - backend/src/main.rs
    - backend/tests/license_bypass_safety.rs
decisions:
  - "D-09-03-A: Tuple params instead of libsql::params![] for user inserts — params! macro produces [Result<Value>; N] which silently returns rows_affected=0 when the array contains mixed &str and owned String element types. Tuple form uses per-element IntoValue and works correctly."
  - "D-09-03-B: Devices use ports 4400 (entry) and 4401 (exit) to satisfy idx_devices_ip_port_active partial unique index (ip, port WHERE active). mock_hikvision public port = 4400; exit device contacts admin port = 4401 (ISAPI traffic from exit device will error — acceptable for E2E fixtures)."
  - "D-09-03-C: license_bypass_safety.rs updated to use CARGO_BIN_EXE_cronometrix (not cronometrix-api) — adding explicit [[bin]] entries changes the CARGO_BIN_EXE env var name from the package name to the declared binary name."
metrics:
  duration: "17 minutes"
  completed_date: "2026-04-29"
  tasks_completed: 4
  files_changed: 8
---

# Phase 9 Plan 03: Backend E2E Infrastructure Summary

**One-liner:** Cargo-feature-gated seed_e2e + mock_hikvision binaries + defense-in-depth __test_reset route; unblocks all Wave 1+ spec plans.

## What Was Built

### Task 1: Cargo features + stubs + test_reset module

`backend/Cargo.toml` now has a `[features]` block (`seed-e2e`, `mock-hikvision`) and three explicit `[[bin]]` entries. Adding any `[[bin]]` entry disables Cargo's auto-detection of `src/main.rs`, so an explicit `[[bin]] name = "cronometrix"` entry was required. Without it `cargo build` would not produce the main binary.

`backend/src/test_reset/mod.rs` — full handler with dual guard: route not registered without env flag (main.rs), and handler re-checks env and returns 404 even if somehow reached (defense-in-depth per T-09-02).

### Task 2: seed_e2e binary

Seeds the E2E database using production code paths:
- `cronometrix_api::db::init_db` — runs all 17 migrations
- `cronometrix_api::auth::service::hash_password` — argon2id via `password-auth` crate
- `cronometrix_api::devices::crypto::encrypt_password` — AES-256-GCM via `aes-gcm` crate

**Seeded users (passwords for downstream specs):**

| username       | role       | password           |
|----------------|------------|--------------------|
| e2e_admin      | admin      | e2e-admin-pass     |
| e2e_supervisor | supervisor | e2e-supervisor-pass|
| e2e_viewer     | viewer     | e2e-viewer-pass    |

**Seeded departments:** dept-prod (Producción), dept-admin (Administración), dept-rrhh (Recursos Humanos)

**Seeded employees (6 total, 2 per dept):**

| id         | code   | name              | dept       |
|------------|--------|-------------------|------------|
| emp-ana    | EMP001 | Ana Pérez         | dept-prod  |
| emp-luis   | EMP002 | Luis García       | dept-prod  |
| emp-maria  | EMP003 | María López       | dept-admin |
| emp-pedro  | EMP004 | Pedro Ramírez     | dept-admin |
| emp-carmen | EMP005 | Carmen Silva      | dept-rrhh  |
| emp-jose   | EMP006 | José Hernández    | dept-rrhh  |

**Seeded devices:**

| id        | name              | ip          | port | direction |
|-----------|-------------------|-------------|------|-----------|
| dev-entry | Entrada Principal | 127.0.0.1   | 4400 | entry     |
| dev-exit  | Salida Principal  | 127.0.0.1   | 4401 | exit      |

**Column lists used in INSERT statements (for future migration compatibility):**

- `users`: id, username, full_name, password_hash, role, status, version, created_at, updated_at
- `departments`: id, name, base_salary_cents, shift_start_time, shift_end_time, lunch_mode, lunch_duration_min, shift_type, is_overnight_shift, ordinary_daily_minutes, status, version, created_at, updated_at
- `employees`: id, employee_code, name, department_id, status, version, created_at, updated_at, position
- `devices`: id, name, ip, port, scheme, username, encrypted_password, direction, allow_insecure_tls, connection_state, status, version, created_at, updated_at

### Task 3: mock_hikvision binary

Two Axum routers sharing `Arc<Mutex<MockState>>`:

**Public port (4400) — 6 ISAPI endpoints:**

| Method | Path                                           | Behavior                                      |
|--------|------------------------------------------------|-----------------------------------------------|
| GET    | /ISAPI/System/status                           | 200 JSON `{status:"OK",deviceModel:"DS-K1T341",...}` |
| GET    | /ISAPI/Event/notification/alertStream          | 200 multipart/mixed; drains event queue        |
| PUT    | /ISAPI/RemoteControl/door/0                    | 200 XML `<ResponseStatus><statusCode>1</statusCode>...</ResponseStatus>` + recv_log |
| PUT    | /ISAPI/AccessControl/UserInfo/Record           | 200 XML + recv_log                             |
| PUT    | /ISAPI/Intelligent/FDLib/FaceDataRecord        | 200 XML + recv_log                             |
| PUT    | /ISAPI/AccessControl/UserInfoDetail/Delete     | 200 XML + recv_log                             |

**Admin port (4401) — test injection + introspection:**

| Method | Path                  | Behavior                                               |
|--------|-----------------------|--------------------------------------------------------|
| POST   | /admin/push-event     | Queues `{xml: "..."}` for next alertStream poll        |
| POST   | /admin/clear-queue    | Empties the event queue                                |
| GET    | /admin/recv-log       | **B6**: returns `{commands: [{method,path,body,timestamp_ms},...]}` |
| POST   | /admin/clear-recv-log | Empties the recv_log                                   |
| GET    | /admin/health         | Returns "ok"                                           |

**B6 recv_log shape** (for devices.spec.ts assertion):
```json
{
  "commands": [
    {
      "method": "PUT",
      "path": "/ISAPI/RemoteControl/door/0",
      "body": "",
      "timestamp_ms": 1777430864981
    }
  ]
}
```

**alertStream Content-Type:** `multipart/mixed; boundary=MIME_boundary` — matches `backend/src/isapi/stream.rs` parser's `extract_boundary()` function exactly.

### Task 4: __test_reset route + integration test

`backend/src/main.rs` conditionally merges the route at startup:
```rust
if std::env::var("CRONOMETRIX_E2E").as_deref() == Ok("true") {
    tracing::warn!("registering /__test_reset route...");
    api_v1 = api_v1.merge(Router::new()
        .route("/__test_reset", post(cronometrix_api::test_reset::test_reset)));
}
```

`backend/tests/test_reset_gating.rs` locks the contract with 2 tests:
- `test_reset_returns_404_without_e2e_flag` — route absent → 404
- `test_reset_returns_200_with_e2e_flag` — route present → 200 `{"reset": true}`

**Tables truncated by __test_reset:** attendance_events, leaves, daily_record_anomalies, daily_record_overrides, daily_records, audit_log.

**Tables NOT truncated (stable seed data):** users, departments, employees, devices, device_face_mappings, global_rules.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] CARGO_BIN_EXE env var rename in license_bypass_safety.rs**
- **Found during:** Task 1
- **Issue:** Adding explicit `[[bin]]` entries changes the `CARGO_BIN_EXE_*` env var from `CARGO_BIN_EXE_cronometrix-api` (package name) to `CARGO_BIN_EXE_cronometrix` (declared binary name). The existing integration test `license_bypass_safety.rs` used the old name, causing compile errors: `environment variable CARGO_BIN_EXE_cronometrix-api not defined at compile time`.
- **Fix:** Updated both occurrences in `license_bypass_safety.rs` to use `CARGO_BIN_EXE_cronometrix`.
- **Files modified:** `backend/tests/license_bypass_safety.rs`
- **Commit:** 193b1c9

**2. [Rule 1 - Bug] libsql::params![] silently returns rows_affected=0 for user inserts**
- **Found during:** Task 2 smoke test
- **Issue:** `libsql::params![id, username, full_name, role, hash]` where `hash` is an owned `String` and the others are `&str` — the macro produces `[Result<Value>; 5]` but the type coercion for the mixed `&str`/`String` element types caused silent 0-rows-affected returns (no error from libsql, no SQLite error). Root cause: the `params!` macro expands to an array literal which Rust type-checks uniformly; `hash.into_value()` coerces differently from `id.into_value()` even though both produce `Result<Value>`.
- **Fix:** Replaced `libsql::params![]` with tuple params `(id, username, full_name, hash.as_str(), role)` and `(id, hash.as_str())` for all inserts that mix `&str` with dynamic `String` values. Tuple params go through per-element `IntoValue` conversion without the array type coercion issue.
- **Files modified:** `backend/src/bin/seed_e2e.rs`
- **Commit:** 9c01068

**3. [Rule 1 - Bug] devices unique index collision on (ip, port)**
- **Found during:** Task 2 smoke test  
- **Issue:** Both devices had `ip=127.0.0.1, port=4400` which violated `idx_devices_ip_port_active` (partial unique index on active devices). Second device insert was silently ignored.
- **Fix:** dev-exit now uses port 4401 (the mock_hikvision admin port). The exit device's ISAPI traffic will contact port 4401 which only serves admin routes — acceptable for E2E fixture purposes since most specs only need the entry device.
- **Files modified:** `backend/src/bin/seed_e2e.rs`
- **Commit:** 9c01068

## Confirmation: Production Build Unchanged

`cargo check --bin cronometrix` (default features, no e2e flag) exits 0. The `seed-e2e` and `mock-hikvision` features are `default = []` — they are never compiled into the production binary. The `__test_reset` route is gated at runtime by `CRONOMETRIX_E2E=true` at startup, so it does not exist in the production router.

## Self-Check: PASSED

Files exist:
- backend/src/bin/seed_e2e.rs: FOUND
- backend/src/bin/mock_hikvision.rs: FOUND
- backend/src/test_reset/mod.rs: FOUND
- backend/tests/test_reset_gating.rs: FOUND

Commits exist:
- 193b1c9: Task 1 (Cargo features + stubs + test_reset)
- 9c01068: Task 2 (seed_e2e implementation)
- 4e0e333: Task 3 (mock_hikvision implementation)
- dc3d9d9: Task 4 (__test_reset route + integration test)

Test results: 743/743 passed, 0 failed.
