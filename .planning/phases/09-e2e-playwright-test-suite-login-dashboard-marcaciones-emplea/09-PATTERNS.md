# Phase 9: E2E Playwright Test Suite — Pattern Map

**Mapped:** 2026-04-28
**Files analyzed:** 22 (12 new + 10 modified)
**Analogs found:** 19 / 22 (3 are pure greenfield — Playwright-only artifacts)

> Source for "files to create/modify": `09-CONTEXT.md` (Integration Points + Addendum) + `09-RESEARCH.md` § Wave 0 Gaps (lines 916–931) + `09-VALIDATION.md` § Wave 0 Requirements.

---

## File Classification

| New / Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---------------------|------|-----------|----------------|---------------|
| `frontend/playwright.config.ts` | config (Playwright) | request-response (test runner ↔ webServer) | — (greenfield) | none |
| `frontend/e2e/setup/00-build-and-seed.setup.ts` | Playwright setup project | batch (DB seed + storageState) | `backend/tests/common/mod.rs` (test_db + create_test_admin) | role-match (test seed) |
| `frontend/e2e/login.spec.ts` | Playwright spec | request-response (UI) | `frontend/src/components/__tests__/*` (RTL assertions) | partial (different runner) |
| `frontend/e2e/dashboard.spec.ts` | Playwright spec | request-response + SSE | — (greenfield, but `frontend/src/lib/use-sse.ts` shows SSE shape) | partial |
| `frontend/e2e/timesheet.spec.ts` | Playwright spec | CRUD + audit assertion | `backend/tests/daily_record_tests.rs` (asserts audit_log on mutate) | role-match (cross-stack) |
| `frontend/e2e/employees.spec.ts` | Playwright spec | CRUD + audit assertion | same as above | role-match |
| `frontend/e2e/devices.spec.ts` | Playwright spec | CRUD + ISAPI dispatch | `backend/tests/multi_device_push_test.rs` + `backend/tests/common/mock_hikvision.rs` | role-match |
| `frontend/e2e/reports.spec.ts` | Playwright spec | request-response + file I/O | `backend/tests/reports_excel_test.rs` (calamine assertions) | role-match (cross-stack) |
| `frontend/e2e/audit.spec.ts` | Playwright spec | request-response (read-only list) | `backend/tests/leaves_handlers_extra_test.rs` (paginated list) | role-match |
| `frontend/e2e/rbac.spec.ts` | Playwright spec | request-response (RBAC negative) | `backend/tests/auth_handlers_extra_test.rs` (403 assertions) | role-match |
| `frontend/e2e/fixtures/api.ts` | Playwright fixture | helper / utility | `frontend/src/lib/api.ts` (axios client) | role-match (different consumer) |
| `frontend/e2e/fixtures/selectors.ts` | Playwright fixture | helper / utility | — (greenfield, test-id constants) | none |
| `frontend/e2e/fixtures/time.ts` | Playwright fixture | helper / utility | `backend/tests/calc_tests.rs` (frozen-time fixtures) | partial |
| `frontend/e2e/fixtures/hikvision-events/*.xml` | Playwright fixture (data) | static asset | `02-RESEARCH § alertStream Multipart Format` (XML samples) | partial |
| `frontend/e2e/global-teardown.ts` | Playwright teardown | teardown | — (greenfield) | none |
| `backend/src/bin/seed_e2e.rs` | Rust binary (helper) | batch (DB seed) | `backend/src/main.rs` (only existing bin entry) | role-match (different lifecycle) |
| `backend/src/bin/mock_hikvision.rs` | Rust binary (helper) | event-driven (HTTP server) | `backend/tests/common/mock_hikvision.rs` (in-process mock) | exact (just promoted to bin) |
| `backend/tests/license_bypass_safety.rs` | Rust integration test | unit + process spawn | `backend/tests/license_tests.rs` (license_module_is_reachable + AppError mapping) | exact |
| `backend/src/audit/mod.rs` | Rust module (router exports) | — | `backend/src/employees/mod.rs` | exact |
| `backend/src/audit/handlers.rs` + `models.rs` + `service.rs` | Rust handler / models / service | request-response (paginated list) | `backend/src/employees/{handlers,models,service}.rs` | exact |
| `frontend/src/app/(dashboard)/audit/page.tsx` | Next.js page | request-response + table render | `frontend/src/app/(dashboard)/employees/page.tsx` | exact |
| `frontend/src/components/audit/audit-table.tsx` (new) | React component | render-only | `frontend/src/components/employees/employee-table.tsx` | exact |
| `backend/src/main.rs` | Rust binary (modify) | — | `backend/src/main.rs` (self) | exact |
| `backend/src/license/service.rs` (modify) | Rust service (extend) | gate / fail-closed | `backend/src/license/service.rs` (load_and_validate_license) | exact |
| `backend/Cargo.toml` (modify) | manifest | — | self | exact |
| `frontend/package.json` (modify) | manifest | — | self | exact |
| `.github/workflows/ci.yml` (modify) | CI workflow patch | CI orchestration | `.github/workflows/ci.yml` (Frontend Coverage job) | exact |
| `.gitignore` (modify) | gitignore patch | — | self | exact |
| `Makefile` (modify) | build orchestration | — | `Makefile` (`coverage-*` targets) | exact |
| `CLAUDE.md` (modify) | project docs | — | `CLAUDE.md` § Test Coverage subsection | exact |

---

## Pattern Assignments

### `backend/src/audit/{mod,handlers,models,service}.rs` (Rust handler, paginated read-only list)

**Analog:** `backend/src/employees/{mod,handlers,models,service}.rs`

**Module skeleton** (`backend/src/employees/mod.rs` lines 1–3):
```rust
pub mod handlers;
pub mod models;
pub mod service;
```

**Read handler signature** (`backend/src/employees/handlers.rs` lines 35–43):
```rust
pub async fn list_employees(
    State(state): State<AppState>,
    Query(query): Query<EmployeeListQuery>,
) -> Result<Json<PaginatedResponse<Employee>>, AppError> {
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
    let result = service::list(&conn, query).await?;
    Ok(Json(result))
}
```

**PaginatedResponse contract** (`backend/src/common.rs` lines 6–11):
```rust
pub struct PaginatedResponse<T: Serialize> {
    pub data: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}
```

**Query model with filters** (`backend/src/employees/models.rs` lines 56–63):
```rust
#[derive(Debug, Deserialize)]
pub struct EmployeeListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub name: Option<String>,
    pub department_id: Option<String>,
    pub status: Option<String>,
}
```

**What's the same:** module layout, paginated response shape, AppError-based error mapping, `state.db.connect()` pattern, validator-derive on input (none here — read-only), `Query` extractor for filters.

**What's new for Phase 9:**
- Audit list query supports `actor_id` (user) + `from_date` / `to_date` (epoch range) + `table_name` filter (per Addendum D-04 resolution).
- `audit_log` rows include both `old_data` and `new_data` JSON columns (see migration excerpt below) — service must `serde_json::from_str` these into `serde_json::Value` so the frontend can render diff cells.
- Response is read-only by design — NO POST/PATCH/DELETE handlers; the table is append-only at the schema level (`backend/src/db/migrations/001_initial_schema.sql` lines 73–86 explicitly say "no UPDATE or DELETE triggers").

**Schema reference** (`backend/src/db/migrations/001_initial_schema.sql` lines 73–86):
```sql
CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY,
    table_name TEXT NOT NULL,
    record_id TEXT NOT NULL,
    operation TEXT NOT NULL CHECK(operation IN ('INSERT', 'UPDATE', 'DELETE')),
    old_data TEXT,
    new_data TEXT,
    actor_id TEXT,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_audit_log_table ON audit_log(table_name);
CREATE INDEX idx_audit_log_record ON audit_log(record_id);
CREATE INDEX idx_audit_log_created ON audit_log(created_at);
```

---

### `backend/src/main.rs` — register audit router + `__test_reset` route (modify)

**Analog:** existing route-group pattern in the same file.

**RBAC layering pattern** (`backend/src/main.rs` lines 206–215 — `supervisor_read_routes`):
```rust
let supervisor_read_routes = Router::new()
    .route("/anomalies", get(anomalies::handlers::list_anomalies))
    .route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        auth::rbac::require_supervisor_or_above,
    ))
    .route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        license::middleware::require_license,
    ));
```

**What's the same:** Router-per-RBAC-tier; `route_layer` ordering (require_auth/RBAC first, license last in code = first at runtime per axum 0.8 reverse semantics — see `backend/src/license/middleware.rs` lines 13–17 docstring); merge into `app.nest("/api/v1", ...)`.

**What's new for Phase 9:**
- Audit router registered at `GET /api/v1/audit` inside the existing `supervisor_read_routes` group (Addendum D-04: Admin + Supervisor read; Viewer 403 — `require_supervisor_or_above` already does exactly that).
- `__test_reset` route at `POST /api/v1/__test_reset`, gated by `if std::env::var("CRONOMETRIX_E2E").as_deref() == Ok("true")`. Truncates `attendance_events`, `leaves`, `audit_log` (and any time-calc derived tables). Per CONTEXT D-12, it MUST refuse to register at all when the env flag is unset — the route literally does not exist on a normal boot.
- Both new registrations happen AFTER `let state = AppState { ... };` is built (line 88) and BEFORE `let app = Router::new().nest("/api/v1", ...)` (line 305) — same place every other phase has added groups.

---

### `backend/src/license/service.rs` — bypass-flag check (modify)

**Analog:** `backend/src/license/service.rs::load_and_validate_license` (lines 62–86).

**Current production gate** (lines 62–86):
```rust
pub async fn load_and_validate_license(jwt_path: &str) -> bool {
    let token = match std::fs::read_to_string(jwt_path) {
        Ok(t) => t.trim().to_string(),
        Err(_) => return false,
    };
    if token.is_empty() { return false; }
    let claims = match verify_license_jwt(&token) {
        Ok(c) => c,
        Err(_) => return false,
    };
    let current_fp = match fingerprint::collect_fingerprint() {
        Ok(fp) => fp,
        Err(e) => {
            tracing::warn!("fingerprint collection failed: {}", e);
            return false;
        }
    };
    if claims.hardware_fingerprint != current_fp {
        tracing::error!("license fingerprint mismatch");
        return false;
    }
    true
}
```

**What's the same:** function returns `bool`, `tracing::error!` on hard-fail, fail-closed semantics.

**What's new for Phase 9 (D-13 — most critical):**
- New free function (or method on a `LicenseGate` struct — planner discretion) that runs FIRST in `main.rs` before `load_and_validate_license` is called. Pseudocode:
  ```rust
  // Phase 9 D-13: gated bypass — must abort if bypass set without e2e flag.
  let bypass = std::env::var("CRONOMETRIX_LICENSE_BYPASS").as_deref() == Ok("true");
  let e2e = std::env::var("CRONOMETRIX_E2E").as_deref() == Ok("true");
  if bypass && !e2e {
      tracing::error!("CRONOMETRIX_LICENSE_BYPASS set without CRONOMETRIX_E2E — aborting");
      std::process::exit(2); // exit code 2 — locked by license_bypass_safety test
  }
  if bypass && e2e {
      license_valid.store(true, Ordering::Relaxed);
      tracing::warn!("license bypass active (CRONOMETRIX_E2E=true) — DEV/TEST ONLY");
      // skip load_and_validate_license entirely
  } else {
      // ... existing path
  }
  ```
- Exit code 2 is a hard contract — `backend/tests/license_bypass_safety.rs` asserts it via `Command::new(...).status().code() == Some(2)`.
- Document this in CLAUDE.md "Phase 9 E2E" subsection (test-only flag, never appear in prod env).

---

### `backend/tests/license_bypass_safety.rs` (Rust integration test, locks D-13)

**Analog:** `backend/tests/license_tests.rs` (existing license-domain integration test file).

**Spawn-process integration test pattern** — there is no existing analog for spawning the binary from within a test, so the planner combines:
1. The license-test reachability pattern (`backend/tests/license_tests.rs` lines 39–45):
   ```rust
   #[test]
   fn license_module_is_reachable() {
       let _ = license::fingerprint::collect_fingerprint;
       let _ = license::service::verify_license_jwt;
   }
   ```
2. The AppError mapping pattern (lines 60–71):
   ```rust
   #[tokio::test]
   async fn unlicensed_error_maps_to_403_with_code_unlicensed() {
       use axum::response::IntoResponse;
       let resp = AppError::Unlicensed.into_response();
       assert_eq!(resp.status(), axum::http::StatusCode::FORBIDDEN);
       // ...
   }
   ```

**What's new for Phase 9:**
- Tests are not in-process — they invoke `cargo run --bin cronometrix` (or pre-built binary at `target/debug/cronometrix`) via `std::process::Command` with the bypass flag set but `CRONOMETRIX_E2E` unset, then assert the child exits with status code 2 within a timeout.
- A second positive test sets BOTH flags and asserts the binary at least gets past the gate (e.g., starts listening on the configured port). Use a short-lived child + SIGTERM so the test runs in <30s as VALIDATION.md mandates.
- File is at `backend/tests/license_bypass_safety.rs` (root of `tests/`) — matches the layout of every other integration test file (no `mod common;` needed unless the test reuses helpers; it doesn't).

---

### `backend/src/bin/seed_e2e.rs` (Rust helper binary, gated by `[features] seed-e2e`)

**Analog:** `backend/src/main.rs` (the only existing bin entry).

**Pattern excerpt** (`backend/src/main.rs` lines 34–50):
```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        )
        .init();
    let config = Config::from_env()?;
    tracing::info!("Initializing database...");
    let db = db::init_db(&config).await?;
    // ...
}
```

**Reused argon2 password-hash helper** lives in `backend/src/auth/service.rs` (used by `backend/src/auth/handlers.rs::login`). The seed binary MUST call into that same module so password params are identical to production (researcher flagged this in §Flake sources line 952: "Argon2 cost differences between dev seed and prod hash params").

**What's the same:** `#[tokio::main]`, `dotenvy::dotenv().ok()`, `tracing_subscriber::fmt()`, `Config::from_env()`, `db::init_db(&config)`, `run_migrations` already happens inside `init_db`.

**What's new for Phase 9:**
- Cargo.toml gating:
  ```toml
  [features]
  seed-e2e = []
  mock-hikvision = []

  [[bin]]
  name = "cronometrix"
  path = "src/main.rs"

  [[bin]]
  name = "seed_e2e"
  path = "src/bin/seed_e2e.rs"
  required-features = ["seed-e2e"]

  [[bin]]
  name = "mock_hikvision"
  path = "src/bin/mock_hikvision.rs"
  required-features = ["mock-hikvision"]
  ```
- Behaviour: read `CRONOMETRIX_DB_URL` (already part of `Config`), run migrations (idempotent — `CREATE TABLE IF NOT EXISTS`), then INSERT … ON CONFLICT for users `e2e_admin` / `e2e_supervisor` / `e2e_viewer` and the seed departments / employees / devices used by all specs.
- Reads `state.paths.*` is NOT relevant here (the binary doesn't open files), but it MUST honor the same env-var convention if it ever needs an evidence dir — this binary just runs SQL.

---

### `backend/src/bin/mock_hikvision.rs` (Rust helper binary, gated by `[features] mock-hikvision`)

**Analog:** `backend/tests/common/mock_hikvision.rs` (in-process tokio TCP server already used by Phase 2 tests).

**Plain-mock pattern** (`backend/tests/common/mock_hikvision.rs` lines 20–48):
```rust
pub async fn spawn_mock_hikvision_plain(body: Vec<u8>, boundary: &str) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let boundary = boundary.to_string();
    tokio::spawn(async move {
        if let Ok((mut sock, _)) = listener.accept().await {
            let mut buf = [0u8; 4096];
            let _ = sock.read(&mut buf).await;
            let response_head = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: multipart/mixed; boundary={}\r\n\
                 Connection: close\r\n\
                 Content-Length: {}\r\n\r\n",
                boundary, body.len()
            );
            let _ = sock.write_all(response_head.as_bytes()).await;
            let _ = sock.write_all(&body).await;
            let _ = sock.shutdown().await;
        }
    });
    addr
}
```

**Digest-auth-enforcing variant** (lines 64–120) is the model for the bin too — the bin just wraps these helpers and exposes them as a long-running process bound to a TCP port.

**What's the same:** TCP listener pattern, multipart/mixed response shape, digest-401-then-200 handshake, never validates the client digest hash (test scope only).

**What's new for Phase 9 (per Addendum D-14):**
- Long-running Axum (or hand-rolled tokio) server bound to a deterministic port (env: `MOCK_HIKVISION_PORT`, default 18080), not an ephemeral port — Playwright `webServer` needs to know the URL up-front to pass into the backend's device config.
- Serves `GET /ISAPI/Event/notification/alertStream` as a streaming multipart with canned `EventNotificationAlert` XML chunks pulled from `frontend/e2e/fixtures/hikvision-events/*.xml` (file-system source so specs can edit fixtures without recompiling).
- Also serves outbound endpoints: `/ISAPI/AccessControl/UserInfo/Record`, `/ISAPI/Intelligent/FDLib/FaceDataRecord`, `/ISAPI/AccessControl/UserInfoDetail/Delete`, `/ISAPI/RemoteControl/door/0`, `/ISAPI/System/status`. Each returns canned 200 / 401 / 503 keyed off a query param or path segment so tests can drive error-state branches.
- Exposes a small admin API on a separate port (`MOCK_HIKVISION_ADMIN_PORT`, default 18081) that lets specs push events into the alertStream queue (`POST /admin/push-event` with XML body).
- `tracing_subscriber::fmt().init()` for log visibility in CI artifacts.

---

### `backend/tests/common/` reuse — **Filesystem-root injection + DB seed**

**Source:** `backend/src/state/paths.rs` lines 35–43 + `backend/tests/common/mod.rs` lines 25–42.

**Paths::for_test pattern** (paths.rs lines 35–43):
```rust
pub fn for_test(tmp: &Path) -> Self {
    Self {
        leaves_root: tmp.join("leaves"),
        events_root: tmp.join("events"),
        enrollments_root: tmp.join("enrollments"),
        captures_tmp_root: tmp.join("captures-tmp"),
        overrides_root: tmp.join("overrides"),
    }
}
```

**Apply to:** Phase 9 webServer.env injection — the Playwright globalSetup (or setup project) creates a per-run tempdir with `RUN_ID` (PID locally, `GITHUB_RUN_ID` in CI per RESEARCH §Race conditions line 941) and exports:
```
CRONOMETRIX_LEAVES_ROOT=/tmp/cronometrix-e2e-${RUN_ID}/leaves
CRONOMETRIX_EVENTS_ROOT=/tmp/cronometrix-e2e-${RUN_ID}/events
ENROLLMENTS_DIR=/tmp/cronometrix-e2e-${RUN_ID}/enrollments
CRONOMETRIX_CAPTURES_TMP=/tmp/cronometrix-e2e-${RUN_ID}/captures-tmp
DATA_DIR=/tmp/cronometrix-e2e-${RUN_ID}
CRONOMETRIX_DB_URL=/tmp/cronometrix-e2e-${RUN_ID}.db
```
The binary's `Paths::from_env()` (`paths.rs` lines 19–30) already reads each env var with a default — no new code needed on the backend side. **DO NOT** call `std::env::var(...)` from any new audit/test-reset handler at use-site; always thread through `state.paths.*`. This is the load-bearing rule from the CLAUDE.md "Filesystem-root injection" convention.

---

### `frontend/src/app/(dashboard)/audit/page.tsx` (replace placeholder)

**Analog:** `frontend/src/app/(dashboard)/employees/page.tsx`.

**Page-level patterns to copy** (`employees/page.tsx` lines 1–10, 14–34):
```tsx
'use client'
import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api'
import { TopBar } from '@/components/layout/top-bar'
import { useAuth } from '@/hooks/use-auth'
import type { PaginatedResponse } from '@/types/api'
import type { PaginationState } from '@tanstack/react-table'

const PAGE_SIZE = 10

export default function EmployeesPage() {
  const { role } = useAuth()
  const [pagination, setPagination] = useState<PaginationState>({ pageIndex: 0, pageSize: PAGE_SIZE })
  const [search, setSearch] = useState('')
  // ...
  const { data, isLoading } = useQuery<PaginatedResponse<Employee>>({
    queryKey: ['employees', pagination.pageIndex, search, deptFilter, statusFilter],
    queryFn: () =>
      api.get('/employees', {
        params: { ...(search && { name: search }), limit: PAGE_SIZE, offset: pagination.pageIndex * PAGE_SIZE },
      }).then(r => r.data),
  })
```

**RBAC gating** (`employees/page.tsx` lines 79–90):
```tsx
{(role === 'admin' || role === 'supervisor') && (
  <button>Emitir Reporte</button>
)}
{role === 'admin' && (
  <button>Nuevo Empleado</button>
)}
```

**What's the same:** `'use client'` directive; useQuery + axios via `api` from `@/lib/api`; TopBar from `@/components/layout/top-bar`; useAuth role-derived hook; PaginationState pattern; Spanish copy ("Buscar…"); className conventions; PAGE_SIZE=10.

**What's new for Phase 9:**
- Page is read-only — no "Nuevo / Editar" buttons. RBAC: visible to Admin + Supervisor only (Viewer would 403 anyway from the API; but the page itself is reachable per dashboard route group — the table just renders empty data with a 403 toast). The planner may choose to wrap the page render in `if (role !== 'admin' && role !== 'supervisor') return <AccessRestricted />;` mirroring the pattern used elsewhere.
- Filters: `actor` (user dropdown — fetch users list), `from_date` / `to_date` date pickers, `table_name` dropdown (employees / leaves / daily_records / overrides / departments / rules / devices / tenant_info — anything with audit triggers).
- TanStack Table columns: timestamp / actor / table / operation / record_id / diff (rendered as `<DiffCell old={...} new={...} />` from JSON columns).
- Test IDs (per CONTEXT D-discretion + VALIDATION.md): `data-testid="audit-row-${id}"`, `data-testid="audit-filter-actor"`, `data-testid="audit-filter-from"`, `data-testid="audit-filter-to"`. PLAN.md should enumerate.

---

### `frontend/src/components/audit/audit-table.tsx` (new TanStack Table component)

**Analog:** `frontend/src/components/employees/employee-table.tsx`.

**Pattern excerpt** (`employee-table.tsx` lines 38–106):
```tsx
const columns: ColumnDef<Employee>[] = [
  { accessorKey: 'name', header: 'Nombre' },
  // ...
  {
    accessorKey: 'hire_date',
    header: 'Fecha Ingreso',
    cell: ({ getValue }) => {
      try { return format(new Date(getValue() as string), 'dd/MM/yyyy') } catch { return '—' }
    },
  },
]

const table = useReactTable({
  data, columns,
  pageCount: Math.ceil(total / PAGE_SIZE),
  state: { pagination },
  onPaginationChange: (updater) => {
    const next = typeof updater === 'function' ? updater(pagination) : updater
    onPaginationChange(next)
  },
  getCoreRowModel: getCoreRowModel(),
  manualPagination: true,
  manualFiltering: true,
})
```

**What's the same:** ColumnDef shape, `useReactTable` config, manual pagination + filtering (server-side), date-fns `format(new Date(...), 'dd/MM/yyyy')` for timestamps, empty-state `<tr>` with `colSpan={columns.length}` ("Sin entradas para los filtros seleccionados"), Anterior/Siguiente Spanish pagination footer.

**What's new for Phase 9:** no `Acciones` column (read-only); diff column renders `<details>` summarising changed fields; `data-testid` on each `<tr>` keyed off `row.original.id`.

---

### Playwright specs (login.spec.ts, dashboard.spec.ts, …)

**Analog:** No Playwright spec exists in the repo. The closest cross-stack analog for **what the spec asserts** is the Rust integration tests that already exercise the backend at full HTTP depth.

**Cross-stack analog 1 — auth + RBAC negative paths** (`backend/tests/auth_handlers_extra_test.rs`): asserts 401 / 403 on missing or wrong-role tokens. Phase 9 `rbac.spec.ts` asserts the SAME contract from the browser side (Viewer logs in, attempts to navigate to `/devices`, gets redirected or sees 403 toast).

**Cross-stack analog 2 — mutation→audit assertion** (`backend/tests/daily_record_tests.rs`): every mutate test reads back `SELECT * FROM audit_log WHERE record_id = ?` and asserts an entry exists. Phase 9 CRUD specs do the same via `request.get('/api/v1/audit?record_id=…')` after each UI mutation.

**Cross-stack analog 3 — Excel content verification** (`backend/tests/reports_excel_test.rs`): uses `calamine` to read generated XLSX and assert cell contents. Phase 9 `reports.spec.ts` does the equivalent in TypeScript via `xlsx` (SheetJS) — D-03 mandates "Excel + PDF export verification, not just download success."

**Selector-strategy rule (D-17, RESEARCH §Pitfalls):** prefer `getByRole('button', { name: 'Registrar Novedad' })` / `getByTestId('audit-row-…')` / `getByLabel('Username')`. Avoid CSS selectors and avoid `page.waitForTimeout()` (lint rule per RESEARCH line 948).

**Spanish vs English copy (D-19 + Addendum):** `login.spec.ts` matches CURRENT English copy ("Log in to Cronometrix", "Username", "Password", "Log in", "Invalid username or password.") per `frontend/src/app/login/page.tsx` lines 95, 120, 139, 182, 78. All other specs match Spanish copy ("Empleados", "Marcaciones", "Dispositivos", "Reportes", "Auditoría", "Sin empleados…").

> **2026-07-13 Phase 12 supersession:** The paragraph above records the Phase
> 9 implementation pattern and remains unchanged as historical evidence. For
> current `/login` work, Phase 12 supersedes the English-only D-19 addendum:
> use accessible Spanish selectors for `Iniciar Sesión`, `Usuario`,
> `Contraseña`, `Mostrar contraseña` / `Ocultar contraseña`, assert the exact
> Spanish error copy, and require root `<html lang="es-VE">`.

**Time-determinism rule (D-20):** every spec sets `timezoneId: 'America/Caracas'` on the browser context (Playwright `use.timezoneId` config) and the backend gets `TZ=America/Caracas` via `webServer.env`. NEVER use `NOW()` in DB seeds — always seed fixed epoch values (RESEARCH §Flake sources line 950).

---

### `frontend/playwright.config.ts` (greenfield)

**No analog.** Built from Playwright official docs (`playwright.dev/docs/test-webserver`, `…/test-projects`, `…/auth`) cited in RESEARCH §Sources lines 993–1003.

Key shape (planner copies verbatim into PLAN, adjusts ports):
```ts
import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,           // D-12 determinism
  workers: 1,                     // D-12 determinism
  retries: process.env.CI ? 1 : 0,
  reporter: [['html', { outputFolder: 'playwright-report' }], ['list']],
  use: {
    baseURL: 'http://localhost:3001',
    trace: 'on-first-retry',
    timezoneId: 'America/Caracas',
    locale: 'es-VE',
  },
  projects: [
    { name: 'setup', testMatch: /.*\/setup\/.*\.setup\.ts/ },
    { name: 'chromium', dependencies: ['setup'], use: { /* storageState assigned per spec */ } },
  ],
  webServer: [
    {
      command: 'cargo run --release --bin cronometrix --features seed-e2e',
      cwd: '../backend',
      url: 'http://127.0.0.1:4001/api/v1/health?deep=true',
      timeout: 180_000,
      env: {
        CRONOMETRIX_E2E: 'true',
        CRONOMETRIX_LICENSE_BYPASS: 'true',
        TZ: 'America/Caracas',
        // Filesystem-root injection (CLAUDE.md):
        CRONOMETRIX_LEAVES_ROOT: `/tmp/cronometrix-e2e-${process.pid}/leaves`,
        // ...
      },
    },
    {
      command: 'cargo run --release --bin mock_hikvision --features mock-hikvision',
      cwd: '../backend',
      url: 'http://127.0.0.1:18080/ISAPI/System/status',
      timeout: 60_000,
    },
    {
      command: 'next start -p 3001',
      cwd: '.',
      url: 'http://localhost:3001/login',
      timeout: 60_000,
    },
  ],
})
```

---

### `.github/workflows/ci.yml` — add `E2E Tests` job

**Analog:** `.github/workflows/ci.yml` lines 63–85 (`frontend-coverage` job).

**Pattern excerpt to copy** (lines 63–85):
```yaml
frontend-coverage:
  name: Frontend Coverage
  runs-on: ubuntu-latest
  defaults:
    run:
      working-directory: frontend
  steps:
    - uses: actions/checkout@v4
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

**Top-of-file constraints** (lines 7–14):
```yaml
on:
  push:
    branches: ['**']
  pull_request:
    branches: [main]

permissions:
  contents: read
```

**What's the same:** pinned actions (`actions/checkout@v4`, `actions/setup-node@v4`, `actions/upload-artifact@v4`); `permissions: contents: read` at workflow scope (T-08-15 least privilege); `if: always()` upload; `retention-days: 14`; `defaults.run.working-directory: frontend`.

**What's new for Phase 9 `E2E Tests` job:**
- Needs Rust toolchain + cargo cache → reuse `Swatinem/rust-cache@v2` (already pinned and used by `backend-coverage` lines 32–34).
- Runs `cargo build --release --bin cronometrix --features seed-e2e` and `cargo build --release --bin mock_hikvision --features mock-hikvision` BEFORE Playwright spawns webServer (so webServer.timeout doesn't blow past the 180s budget on cold compile).
- Installs Playwright browsers: `npx playwright install --with-deps chromium`. Per Playwright official guidance (RESEARCH line 1000) DO NOT cache `~/.cache/ms-playwright` — fresh install ~30s, comparable to cache restore.
- Run command: `cd frontend && npx playwright test`.
- Two artifact uploads, both `if: always()`, both 14-day retention:
  - `playwright-report` (HTML)
  - `test-results` (videos, traces, screenshots, retry attempts) — D-18 mandates "every CI run, not just failures"
- `env: TZ: America/Caracas` at job level so any non-webServer-spawned process inherits.
- Job name MUST be `E2E Tests` (matches CONTEXT D-15, branch-protection check name).

---

### `.gitignore` — add E2E artifacts (modify)

**Analog:** existing `.gitignore` lines 13–15 (Node) and 21–22 (Pencil).

**Current** (lines 13–15):
```
# Node (for frontend)
node_modules/
.next/
```

**Append:**
```
# Phase 9: Playwright E2E artifacts (regenerated per run; never commit)
frontend/e2e/.auth/
frontend/playwright-report/
frontend/test-results/
```

---

### `Makefile` — add `e2e` and `e2e-install` targets (modify)

**Analog:** `Makefile` lines 10–25 (`coverage`, `coverage-backend`, `coverage-frontend`).

**Pattern excerpt** (lines 23–25):
```makefile
coverage-frontend:
	cd frontend && npx vitest run --coverage
	@echo "Frontend HTML: frontend/coverage/index.html"
```

**What's the same:** PHONY declaration, `cd frontend && …` working-dir convention, trailing `@echo` pointing to the HTML report.

**What's new:**
```makefile
.PHONY: e2e e2e-install

e2e-install:
	cd frontend && npm ci && npx playwright install --with-deps chromium

e2e:
	cd frontend && npx playwright test
	@echo "E2E HTML: frontend/playwright-report/index.html"
```
Add to the existing `.PHONY` line up top per Make convention.

---

### `frontend/package.json` — add devDeps + scripts (modify)

**Analog:** existing `scripts` block (lines 5–10) and `devDependencies` (lines 38–54).

**What's new (RESEARCH §Standard Stack lines 850–853):**
```json
"scripts": {
  "dev": "next dev",
  "build": "next build",
  "start": "next start",
  "lint": "eslint",
  "e2e": "playwright test",
  "e2e:install": "playwright install --with-deps chromium"
}
```
```json
"devDependencies": {
  "@playwright/test": "^1.59.1",
  "xlsx": "^0.18.5",
  "pdf-parse": "^2.4.5",
  ...existing
}
```
Note: `jspdf` and `jspdf-autotable` are already production deps (lines 23–24), so the reports-spec PDF generation runs against the production code path; pdf-parse is only a test-side READER.

---

### `CLAUDE.md` — append "Phase 9 E2E" subsection

**Analog:** the existing CLAUDE.md `## Test Coverage` section (the entire section is the model — see how Phase 8 documented thresholds, exclusion policy, HTML reports, CI gate, and reading-a-failing-run).

**Pattern to mirror** (CLAUDE.md `## Test Coverage` H3 layout):
- `### Install (one-time per developer)` — `npm ci && npx playwright install --with-deps chromium` plus the Rust feature-flag note for `seed-e2e` / `mock-hikvision`.
- `### Local commands` — `make e2e`, `make e2e-install`.
- `### Test-only env flags (DEV/TEST ONLY — must NEVER appear in prod env)` — `CRONOMETRIX_E2E`, `CRONOMETRIX_LICENSE_BYPASS`, plus the abort contract from D-13.
- `### File layout` — `frontend/e2e/{login,dashboard,timesheet,employees,devices,reports,audit,rbac}.spec.ts`, `frontend/e2e/setup/`, `frontend/e2e/fixtures/`, `frontend/e2e/.auth/` (gitignored).
- `### CI gate` — required-status-check name (`E2E Tests`), pinned actions inheritance from Phase 8, retention 14 days, the deferred branch-protection follow-up mirroring Phase 8 Plan 05.

---

## Shared Patterns

### Authentication / RBAC layering (apply to all new backend handlers)
**Source:** `backend/src/main.rs` lines 178–203 (`viewer_routes`) + lines 206–215 (`supervisor_read_routes`) + `backend/src/auth/{middleware,rbac}.rs`.

The new `GET /api/v1/audit` endpoint goes inside `supervisor_read_routes` (Admin + Supervisor read; Viewer 403). The `__test_reset` route — when registered at all — has NO auth layer (it's gated by env, not by token; tests need it without negotiating JWTs).

### Error handling (apply to all new backend handlers + service)
**Source:** `backend/src/errors.rs` lines 15–48 (`AppError` enum) + `backend/src/employees/handlers.rs` lines 18–31 (validation→AppError mapping).

```rust
body.validate().map_err(|e| AppError::Validation {
    code: "VALIDATION_ERROR",
    message: e.to_string(),
})?;
let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;
```
For audit handlers: validation may not apply (read-only), but `state.db.connect().map_err(|e| AppError::Internal(e.into()))` is mandatory.

### Filesystem-root injection (apply everywhere on the backend, no exceptions)
**Source:** `backend/src/state/paths.rs` + CLAUDE.md `## Conventions § Filesystem-root injection`.

NO new audit/test-reset/seed/mock code may call `std::env::var("CRONOMETRIX_LEAVES_ROOT")` or `PathBuf::from("./data/...")` at use-site. Roots flow through `state.paths.*` (handler) or `Paths::from_env()` (binary boot, exactly once). The seed binary doesn't open files; the mock_hikvision binary may serve fixture XML — if so it reads `frontend/e2e/fixtures/hikvision-events/*.xml` from a path passed via env (`MOCK_HIKVISION_FIXTURES_DIR`) and threaded through its own struct, not via process-global env reads.

### Pinned-action CI policy (apply to the new `E2E Tests` job)
**Source:** `.github/workflows/ci.yml` (entire file) + Phase 8 T-08-15 in CLAUDE.md.

`actions/checkout@v4`, `actions/setup-node@v4`, `actions/upload-artifact@v4`, `taiki-e/install-action@v2`, `Swatinem/rust-cache@v2`. All pinned to major version (matches existing Phase 8 jobs). `permissions: contents: read` at workflow level — the Phase 9 job inherits; it does NOT need to elevate.

### Spec-side selectors (apply to all Playwright specs)
**Source:** Playwright official `getByRole` / `getByTestId` / `getByLabel` API. NO existing analog in the repo (specs are greenfield), but the rule is: D-17 + RESEARCH §Pitfalls 1.

```ts
// preferred:
await page.getByRole('button', { name: 'Registrar Novedad' }).click()
await page.getByTestId('kpi-empleados-presentes').toContainText('42')
// avoid:
// await page.locator('.btn-primary').click()  // CSS selector — fragile
// await page.waitForTimeout(2000)             // banned by lint rule
```

### Mutation→Audit assertion (apply to every Wave 2 CRUD spec)
**Source:** every Phase 8 backend test (`daily_record_tests.rs`, `device_tests.rs`, `leave_tests.rs`) reads `audit_log` via raw SQL after a mutation.

In Phase 9 specs (TypeScript), the equivalent is to call the new `GET /api/v1/audit?record_id=…` endpoint via Playwright's `request.get(...)` after each UI-driven mutation. CLAUDE.md non-negotiable: "every mutation to attendance records must generate an immutable audit log entry."

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `frontend/playwright.config.ts` | Playwright config | request-response | Playwright is greenfield in this repo — config is built directly from official docs. |
| `frontend/e2e/setup/00-build-and-seed.setup.ts` | Playwright setup | batch | Closest reference is `backend/tests/common/mod.rs::test_db` + `create_test_admin`, but the runtime context (Playwright vs cargo) is different enough that the planner cannot copy code line-for-line. Treat as "structural inspiration only." |
| `frontend/e2e/global-teardown.ts` | Playwright teardown | teardown | Pure cleanup; planner uses `fs.rm(/tmp/cronometrix-e2e-${RUN_ID}*, { recursive: true })`. No analog. |

---

## Metadata

**Analog search scope:** `backend/src/{auth,license,state,employees,devices,daily_records,db}/`, `backend/tests/`, `backend/Cargo.toml`, `backend/src/bin/` (none existed before Phase 9), `frontend/src/{app,components,lib,hooks}/`, `frontend/package.json`, `.github/workflows/ci.yml`, `Makefile`, `.gitignore`, `CLAUDE.md`.

**Files scanned:** ~35 representative files (out of ~250 in the repo). Stopped at 5 strong analogs per file class.

**Key cross-cutting findings:**
1. Backend has a complete Phase 8 test infrastructure (`tests/common/mod.rs`, `tests/common/mock_hikvision.rs`) that Phase 9 should reuse — particularly the digest-mock TCP-listener pattern and the `test_access_token` JWT helper. The `backend/src/bin/mock_hikvision.rs` binary is essentially this same code lifted into a long-running process.
2. The `audit_log` table already exists (since migration 001) and triggers populate it on every mutation (migrations 002, 006, 011, 014, 017). Phase 9 only needs the **read** endpoint — no triggers or writes are added.
3. The `Paths::for_test` filesystem-root convention is already pervasive on the backend; Phase 9's `webServer.env` injection slots into the existing `Paths::from_env()` reader without code changes on the backend side.
4. Existing Next.js dashboard pages (`employees/page.tsx`) are the perfect blueprint for the new `audit/page.tsx`: same TopBar, same useQuery+axios pattern, same TanStack Table component pair, same Spanish copy idioms.
5. The license-bypass safety test (D-13) is the highest-risk surface in Phase 9 — RESEARCH calls it out explicitly. The test pattern is novel (spawn binary as child process and assert exit code 2) but the AppError contract from `tests/license_tests.rs::unlicensed_error_maps_to_403_with_code_unlicensed` is the closest in-repo reference.

**Pattern extraction date:** 2026-04-28
