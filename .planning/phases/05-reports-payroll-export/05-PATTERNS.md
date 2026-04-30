# Phase 5: Reports & Payroll Export - Pattern Map

**Mapped:** 2026-04-25
**Files analyzed:** 30 (24 backend + frontend, plus Bruno + Cargo + package manifests)
**Analogs found:** 27 / 30 (3 first-of-kind in this codebase)

> **First-of-kind in this codebase:** `rust_xlsxwriter` workbook builder, axum binary `Vec<u8>` response with `Content-Disposition: attachment`, jspdf-autotable PDF rendering. For these the analog quality is "structural-only" — copy AppError + handler signature shape from existing handlers, copy library API verbatim from RESEARCH.md.

---

## File Classification

| New / Modified File | Role | Data Flow | Closest Analog | Match Quality |
|---------------------|------|-----------|----------------|---------------|
| `backend/src/db/migrations/013_tenant_info.sql` | migration (DDL) | schema | `backend/src/db/migrations/001_initial_schema.sql` (`global_rules` singleton block, lines 56-68) | exact (singleton with seed row) |
| `backend/src/db/migrations/014_phase5_audit_triggers.sql` | migration (triggers) | event-driven | `backend/src/db/migrations/011_phase3_audit_triggers.sql` (audit_leaves_update, lines 30-44) | exact |
| `backend/src/db/migrations/015_employees_position_hire_date.sql` | migration (ALTER) | schema | `backend/src/db/migrations/012_shift_type_to_departments.sql` | exact (additive ALTER + DEFAULT) |
| `backend/src/tenant_info/mod.rs` | module index | n/a | `backend/src/rules/mod.rs` | exact (singleton domain) |
| `backend/src/tenant_info/models.rs` | DTO/model | n/a | `backend/src/rules/models.rs` (`GlobalRules` + `UpdateRulesRequest`) | exact |
| `backend/src/tenant_info/service.rs` | service | CRUD (singleton read/update) | `backend/src/rules/handlers.rs` `update_rules` (no service.rs file in `rules/` — handler inlines SQL) | role-match (refactor pattern: lift into service.rs like `employees/service.rs`) |
| `backend/src/tenant_info/handlers.rs` | controller | request-response | `backend/src/rules/handlers.rs` (singleton GET + Admin PATCH) | exact |
| `backend/src/employees/models.rs` (modify) | DTO | n/a | self (extend `Employee`, `CreateEmployeeRequest`, `UpdateEmployeeRequest` with `position`, `hire_date`) | self-extend |
| `backend/src/employees/service.rs` (modify) | service | CRUD | self (extend INSERT + UPDATE column lists to include `position`, `hire_date`) | self-extend |
| `backend/src/employees/handlers.rs` (modify) | controller | request-response | self (no signature changes — only pass-through to service) | self-extend |
| `backend/src/main.rs` (modify) | bootstrap/router | n/a | self (add `tenant_info::handlers::*` and `reports::handlers::*` to existing route groups) | self-extend |
| `backend/tests/tenant_info_test.rs` | integration test | request-response | `backend/tests/leave_tests.rs` (build_test_app + role-based tests) | role-match (no multipart needed) |
| `bruno/cronometrix/tenant-info/01_get.bru` + `02_patch.bru` | API contract tests | request-response | `bruno/cronometrix/employees/01_list.bru` + `02_create.bru` | exact |
| `backend/src/reports/mod.rs` | module index | n/a | `backend/src/calc/mod.rs` (re-export pattern) | exact |
| `backend/src/reports/models.rs` | DTO/model | n/a | `backend/src/leaves/models.rs` (`LeaveResponse` + `*Request` validator-derive) | exact |
| `backend/src/reports/money.rs` | pure-function library | transform | `backend/src/calc/overtime.rs` (pure module + inline `#[cfg(test)] mod tests`) | role-match |
| `backend/src/reports/periods.rs` | pure-function library | transform | `backend/src/calc/aggregation.rs` (chrono `Datelike::weekday().num_days_from_monday()`, lines 145-148 of `daily_records/service.rs`) | role-match |
| `backend/src/reports/service.rs` | service | batch / aggregation | `backend/src/anomalies/handlers.rs` (dynamic predicate JOIN, lines 52-108) + `backend/src/daily_records/service.rs` (override-merge query patterns) | role-match (combine both) |
| `backend/src/reports/handlers.rs` (JSON) | controller | request-response | `backend/src/rules/handlers.rs` `update_rules` (Json body + validate + AppError) + `backend/src/devices/service.rs` `write_command_audit` (lines 497-545) for app-code audit insert | role-match |
| `backend/src/reports/excel.rs` | binary builder | transform | First-of-kind — no existing analog. Copy API verbatim from RESEARCH Pattern 4 (lines 588-680). Use `AppError::Internal(anyhow::anyhow!(...))` like `daily_records/handlers.rs::create_override` line 204. | structural-only |
| `backend/src/reports/handlers.rs` (Excel) (modify) | controller | request-response (binary) | `backend/src/leaves/handlers.rs` `get_leave_evidence` (lines 305-329 — `(StatusCode, HeaderMap, Vec<u8>).into_response()`) | role-match (different content type + Content-Disposition) |
| `backend/Cargo.toml` (modify) | manifest | n/a | self (`[dependencies]` block lines 7-38; `[dev-dependencies]` lines 40-49) | self-extend |
| `backend/tests/reports_test.rs` | integration test | request-response | `backend/tests/leave_tests.rs` (build_test_app, body_to_json, seed helpers) + `backend/tests/calc_tests.rs` (per-function unit-style tests) | role-match |
| `backend/tests/fixtures/reports/` | golden fixtures | data | `backend/tests/fixtures/` (existing structure) | role-match |
| `backend/tests/reports_excel_test.rs` | integration test | request-response (binary) | `backend/tests/leave_tests.rs` (HTTP harness) + calamine library docs | structural-only |
| `bruno/cronometrix/reports/01_json.bru` + `02_excel.bru` | API contract tests | request-response | `bruno/cronometrix/employees/02_create.bru` (POST + json body + bearer) | exact |
| `frontend/src/app/(dashboard)/reports/page.tsx` (replace) | page component | request-response | `frontend/src/app/(dashboard)/timesheet/page.tsx` (filter+TanStack Query+modal pattern) + `frontend/src/app/(dashboard)/employees/page.tsx` (filter row composition) | exact |
| `frontend/src/components/reports/period-picker.tsx` | component | event-driven | `frontend/src/components/timesheet/week-navigator.tsx` (date-fns + onChange) | role-match (preset+custom range vs week nav) |
| `frontend/src/components/reports/filters-bar.tsx` | component | event-driven | `frontend/src/app/(dashboard)/employees/page.tsx` lines 44-89 (filter row inline composition) | exact |
| `frontend/src/components/reports/summary-table.tsx` | component | request-response | `frontend/src/components/timesheet/timesheet-table.tsx` + `frontend/src/components/employees/employee-table.tsx` (TanStack Table v8) | exact (extend with synthetic subtotal rows per RESEARCH Pattern 7) |
| `frontend/src/components/reports/drill-down-dialog.tsx` | component | request-response | `frontend/src/components/timesheet/novedad-modal.tsx` (shadcn Dialog + useQuery) | role-match (read-only modal vs form) |
| `frontend/src/components/reports/export-buttons.tsx` | component | event-driven | `frontend/src/components/timesheet/novedad-modal.tsx` (useMutation + isPending + sonner toast) | role-match |
| `frontend/src/lib/reports/pdf.ts` | pure utility | transform | First-of-kind — no jsPDF analog. Copy from RESEARCH Pattern 6 (lines 750-857). | structural-only |
| `frontend/src/lib/format/currency.ts` | pure utility | transform | `frontend/src/lib/utils.ts` (existing 166B `cn()` helper) — minimal pattern | role-match |
| `frontend/src/app/(dashboard)/settings/tenant-info/page.tsx` | page component | request-response | `frontend/src/app/(dashboard)/employees/page.tsx` (Admin-gated Add button) + `novedad-modal.tsx` (react-hook-form + zod + useMutation) | role-match |
| `frontend/src/components/layout/sidebar.tsx` (modify) | component | n/a | self (extend `NAV_ITEMS` array lines 9-17; add Admin-only filter using `useAuth().role`) | self-extend |
| `frontend/src/lib/api.ts` (modify) | bootstrap | request-response | self (no API client changes — only add new query keys in callers; `api` axios instance + 401 interceptor unchanged) | self-extend |
| `frontend/package.json` (modify) | manifest | n/a | self (add `jspdf` 4.2.1 + `jspdf-autotable` 5.0.7 to `dependencies`) | self-extend |

---

## Pattern Assignments

### `backend/src/db/migrations/013_tenant_info.sql` (migration, schema)

**Analog:** `backend/src/db/migrations/001_initial_schema.sql` lines 56-68 (`global_rules` singleton)

**Singleton pattern with seeded default row** (copy verbatim, change names):

```sql
-- 001_initial_schema.sql:56-68 — global_rules singleton
CREATE TABLE IF NOT EXISTS global_rules (
    id TEXT PRIMARY KEY DEFAULT 'singleton',
    late_arrival_tolerance_min INTEGER NOT NULL DEFAULT 10,
    early_departure_tolerance_min INTEGER NOT NULL DEFAULT 10,
    bonus_minutes INTEGER NOT NULL DEFAULT 0,
    effective_from INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    updated_at INTEGER NOT NULL
);

-- Seed the singleton row on first migration (INSERT OR IGNORE is idempotent)
INSERT OR IGNORE INTO global_rules (id, late_arrival_tolerance_min, early_departure_tolerance_min, bonus_minutes, effective_from, version, updated_at)
VALUES ('singleton', 10, 10, 0, unixepoch(), 1, unixepoch());
```

**Differences for `tenant_info` (per CONTEXT D-30):**
- PK is `INTEGER` with `CHECK (id = 1)` (not `TEXT DEFAULT 'singleton'`).
- All TEXT cols default to `''`.
- Add `address TEXT NOT NULL DEFAULT ''` per D-30.
- Use `INSERT INTO tenant_info (id) VALUES (1);` (not OR IGNORE — first-run is the only insert path).
- **Critical: also register the migration in `backend/src/db/mod.rs` `MIGRATIONS` array (lines 9-58) — append the tuple in order.**

---

### `backend/src/db/migrations/014_phase5_audit_triggers.sql` (migration, triggers)

**Analog:** `backend/src/db/migrations/011_phase3_audit_triggers.sql` lines 30-44 (`audit_leaves_update`)

**Audit trigger pattern (UPDATE only — D-30 doesn't reset/delete tenant_info):**

```sql
-- 011_phase3_audit_triggers.sql:30-44 — audit_leaves_update
CREATE TRIGGER IF NOT EXISTS audit_leaves_update
    AFTER UPDATE ON leaves
BEGIN
    INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at)
    VALUES (
        lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)),2) || '-' || substr('89ab',abs(random())%4+1,1) || substr(hex(randomblob(2)),2) || '-' || hex(randomblob(6))),
        'leaves',
        NEW.id,
        'UPDATE',
        json_object('id', OLD.id, 'employee_id', OLD.employee_id, /* ... */),
        json_object('id', NEW.id, 'employee_id', NEW.employee_id, /* ... */),
        NULL,
        unixepoch()
    );
END;
```

**Differences for `audit_tenant_info_update`:**
- `record_id` should be `CAST(NEW.id AS TEXT)` (id is INTEGER 1, audit_log.record_id is TEXT).
- `json_object` includes `client_name`, `client_rif`, `address`, `version`.
- `actor_id` stays NULL in trigger (Phase 1 audit pattern — app-code does NOT need a secondary entry for tenant_info; the trigger payload is sufficient).

**No employees migration update needed in 014.** Phase 1 trigger `audit_employees_update` (`002_audit_triggers.sql:29-43`) hashes ALL columns of NEW/OLD via `json_object` calls. **Pitfall:** the existing trigger only includes `id, employee_code, name, department_id, status, version`; the new `position` and `hire_date` columns will NOT be hashed. The CONTEXT D-30a comment ("audit triggers in `002_audit_triggers.sql` already hash the row — no trigger update needed") is **incorrect** — `json_object` is column-by-column, not auto-introspecting. Planner should add a `audit_employees_update` REPLACE trigger in 014 that includes the new columns. (Flag for plan-level decision; do not silently extend.)

---

### `backend/src/db/migrations/015_employees_position_hire_date.sql` (migration, ALTER)

**Analog:** `backend/src/db/migrations/012_shift_type_to_departments.sql` (entire 10-line file)

**ALTER TABLE pattern** (verbatim):

```sql
-- 012_shift_type_to_departments.sql
ALTER TABLE departments ADD COLUMN shift_type TEXT NOT NULL DEFAULT 'day'
    CHECK(shift_type IN ('day', 'night', 'mixed'));
ALTER TABLE departments ADD COLUMN is_overnight_shift INTEGER NOT NULL DEFAULT 0
    CHECK(is_overnight_shift IN (0,1));
ALTER TABLE departments ADD COLUMN ordinary_daily_minutes INTEGER NOT NULL DEFAULT 480;
```

**For `015_employees_position_hire_date.sql`** (per CONTEXT D-30a):

```sql
ALTER TABLE employees ADD COLUMN position TEXT NOT NULL DEFAULT '';
ALTER TABLE employees ADD COLUMN hire_date INTEGER;  -- nullable epoch seconds (UTC)
```

Same `NOT NULL DEFAULT ''` shape so existing rows pass the constraint; nullable `INTEGER` for `hire_date` since "unknown" is semantically distinct from "" or 0.

---

### `backend/src/tenant_info/{mod,models,service,handlers}.rs` (singleton CRUD)

**Analog file 1:** `backend/src/rules/mod.rs` (entire 2-line file) — module shape:

```rust
pub mod handlers;
pub mod models;
```

**Tenant_info follows the established `{mod, models, service, handlers}` convention (see `employees/`, `leaves/`, `daily_records/`).** The `rules/` module skipped `service.rs` and inlined SQL into the handler, but **prefer the `employees/service.rs` shape for tenant_info** so the integration tests can drive `service::get_tenant_info` directly.

**Analog file 2:** `backend/src/rules/models.rs` (entire 27 lines) — DTO shape:

```rust
// rules/models.rs:1-27
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Serialize)]
pub struct GlobalRules {
    pub late_arrival_tolerance_min: i64,
    pub early_departure_tolerance_min: i64,
    pub bonus_minutes: i64,
    pub effective_from: String,  // ISO 8601
    pub version: i64,
    pub updated_at: String,      // ISO 8601
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateRulesRequest {
    #[validate(range(min = 0, max = 60, message = "Tolerance must be 0-60 minutes"))]
    pub late_arrival_tolerance_min: Option<i64>,
    /* ... */
    pub version: i64,
}
```

**For `tenant_info/models.rs`** — apply same shape:
- `TenantInfo { client_name, client_rif, address, version, updated_at }` — strings + i64 version + ISO updated_at via `epoch_to_iso`.
- `UpdateTenantInfoRequest { client_name: Option<String>, client_rif: Option<String>, address: Option<String>, version: i64 }` with `#[validate(length(max=200))]` on each text field. RIF format validation (V/J/G + dash + digits) can use `#[validate(regex)]` or a custom function — defer regex to plan stage; loose `length` validation is acceptable per "minimal" scope (D-30).

**Analog file 3:** `backend/src/rules/handlers.rs` lines 14-48 (singleton GET):

```rust
// rules/handlers.rs:14-48
fn row_to_rules(row: libsql::Row) -> Result<GlobalRules, AppError> {
    Ok(GlobalRules {
        late_arrival_tolerance_min: row.get(0).map_err(|e| AppError::Internal(e.into()))?,
        /* ... */
    })
}

pub async fn get_rules(
    State(state): State<AppState>,
) -> Result<Json<GlobalRules>, AppError> {
    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;

    let row = conn
        .query(
            "SELECT late_arrival_tolerance_min, /* ... */ \
             FROM global_rules WHERE id = 'singleton'",
            (),
        )
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("global_rules singleton row missing")))?;

    Ok(Json(row_to_rules(row)?))
}
```

**For `tenant_info/handlers.rs`** apply the same Result→Json shape with `WHERE id = 1` instead of `WHERE id = 'singleton'`.

**Analog file 4:** `backend/src/rules/handlers.rs` lines 50-133 (singleton PATCH with optimistic concurrency):

```rust
// rules/handlers.rs:50-133 — update_rules
pub async fn update_rules(
    State(state): State<AppState>,
    Json(body): Json<UpdateRulesRequest>,
) -> Result<Json<GlobalRules>, AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let conn = state.db.connect().map_err(|e| AppError::Internal(e.into()))?;

    // Build dynamic SET clause
    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<libsql::Value> = Vec::new();

    if let Some(val) = body.late_arrival_tolerance_min {
        sets.push(format!("late_arrival_tolerance_min = ?{}", values.len() + 1));
        values.push(libsql::Value::Integer(val));
    }
    /* ... */

    if sets.is_empty() {
        return get_rules(State(state)).await;
    }

    sets.push("updated_at = unixepoch()".to_string());
    sets.push("version = version + 1".to_string());

    let set_clause = sets.join(", ");
    let version_param = values.len() + 1;
    values.push(libsql::Value::Integer(body.version));

    let sql = format!(
        "UPDATE global_rules SET {} WHERE id = 'singleton' AND version = ?{}",
        set_clause, version_param
    );

    let rows_affected = conn
        .execute(&sql, libsql::params_from_iter(values))
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    if rows_affected == 0 {
        return Err(AppError::Conflict {
            code: "VERSION_CONFLICT",
            message: "Rules were modified by another request. Fetch the latest version and retry.".to_string(),
        });
    }
    /* return refreshed singleton */
}
```

**For `tenant_info/handlers.rs::patch_tenant_info`** — apply same shape with these substitutions:
- Table: `tenant_info`, WHERE clause: `WHERE id = 1 AND version = ?` (per RESEARCH Pitfall 8 — always include `id = 1`).
- Conflict message: `"Tenant info was modified by another request. Fetch the latest version and retry."`
- After refactor, **lift this logic into `tenant_info/service.rs::update_tenant_info(conn, req)`** to mirror `employees/service.rs::update` (line 182-271) and keep handler thin. Service handles the `WHERE id = 1` SELECT-after-UPDATE returning `Result<TenantInfo>`.

---

### `backend/src/employees/{models,service,handlers}.rs` (modify — add position + hire_date)

**Analog:** Self. Refer to existing field shape and extend.

**`employees/models.rs:6-16` — `Employee` response struct (currently lacks `position` + `hire_date`):**

```rust
// employees/models.rs:6-16 — current Employee struct
#[derive(Debug, Serialize)]
pub struct Employee {
    pub id: String,
    pub employee_code: String,
    pub name: String,
    pub department_id: String,
    pub status: String,
    pub deleted_at: Option<String>,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}
```

**Add fields:**
```rust
pub position: String,             // NOT NULL DEFAULT '' — empty renders as '—'
pub hire_date: Option<String>,    // ISO 8601 date when present (use chrono::NaiveDate::format)
```

**`employees/models.rs:19-27` — `CreateEmployeeRequest`** (currently lacks position + hire_date):

```rust
#[derive(Debug, Deserialize, Validate)]
pub struct CreateEmployeeRequest {
    #[validate(length(min = 1, max = 50, message = "Employee code is required (1-50 chars)"))]
    pub employee_code: String,
    /* ... */
}
```

**Add optional fields** (CONTEXT D-30a says "extends create/update payloads to accept these fields (optional)"):

```rust
#[validate(length(max = 100))]
pub position: Option<String>,        // defaults to '' in DB if None
pub hire_date: Option<String>,        // YYYY-MM-DD; parse to epoch seconds at service layer
```

**`employees/service.rs:52-74` — `create` INSERT statement** must add `position` and `hire_date` columns:

```rust
// employees/service.rs:52-74 — current INSERT
let result = conn
    .execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'active', 1, unixepoch(), unixepoch())",
        params![id.clone(), req.employee_code.clone(), req.name.clone(), req.department_id.clone()],
    )
    .await;
```

**Extend** to include `position` and `hire_date` columns + bind values (parse ISO `hire_date` → epoch seconds via `chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d")?.and_hms_opt(0,0,0).timestamp()`).

**`employees/service.rs:208-244` — `update` dynamic SET pattern** is the canonical pattern to extend:

```rust
// employees/service.rs:208-244 — dynamic SET fragment to copy
let mut sets: Vec<String> = Vec::new();
let mut values: Vec<libsql::Value> = Vec::new();

if let Some(name) = req.name {
    sets.push(format!("name = ?{}", values.len() + 1));
    values.push(libsql::Value::Text(name));
}
/* extend with position + hire_date */
```

**Frontend `Employee` type (`frontend/src/types/api.ts:36-47`) already has `position: string` and `hire_date: string`** — no frontend type change needed; only backend serialization to match the existing wire shape.

---

### `backend/src/main.rs` (modify — register routes)

**Analog:** Self lines 144-177 (route group composition).

**Existing route group structure to extend:**

```rust
// main.rs:122-142 — viewer_routes (require_auth)
let viewer_routes = Router::new()
    .route("/employees", get(employees::handlers::list_employees))
    /* ... */
    .route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        auth::middleware::require_auth,
    ));

// main.rs:144-150 — supervisor_read_routes
let supervisor_read_routes = Router::new()
    .route("/anomalies", get(anomalies::handlers::list_anomalies))
    .route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        auth::rbac::require_supervisor_or_above,
    ));

// main.rs:162-177 — admin_routes
let admin_routes = Router::new()
    .route("/rules", patch(rules::handlers::update_rules))
    /* ... */
    .route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        auth::rbac::require_admin,
    ));

// main.rs:179-188 — composition
let app = Router::new()
    .nest(
        "/api/v1",
        public_routes
            .merge(cookie_auth_routes)
            .merge(viewer_routes)
            .merge(supervisor_read_routes)
            .merge(supervisor_routes)
            .merge(admin_routes),
    )
    .with_state(state)
    .layer(TraceLayer::new_for_http())
    .layer(build_cors_layer(&config.cors_allowed_origins));
```

**Phase 5 additions:**
- Add `cronometrix_api::reports;` and `cronometrix_api::tenant_info;` to use list (lines 16-31).
- `tenant_info::handlers::get_tenant_info` → append to `viewer_routes` (D-30: all roles read).
- `tenant_info::handlers::patch_tenant_info` → append to `admin_routes` (D-30: Admin-only).
- Reports endpoints go into `supervisor_read_routes` (D-20: Admin + Supervisor; uses `require_supervisor_or_above` extractor, NOT `require_auth`):
  - `.route("/reports/json", post(reports::handlers::generate_json))`
  - `.route("/reports/excel", post(reports::handlers::generate_excel))`
- Apply 60s timeout layer **only to the reports group** per D-25 (RESEARCH Pattern 5 lines 734-744):
  ```rust
  let report_routes = Router::new()
      .route("/reports/json", post(reports::handlers::generate_json))
      .route("/reports/excel", post(reports::handlers::generate_excel))
      .route_layer(tower_http::timeout::TimeoutLayer::new(std::time::Duration::from_secs(60)))
      .route_layer(axum::middleware::from_fn_with_state(
          state.clone(),
          auth::rbac::require_supervisor_or_above,
      ));
  ```
  Then `.merge(report_routes)` in the nest.

---

### `backend/src/reports/mod.rs` (module index)

**Analog:** `backend/src/calc/mod.rs` (re-export pattern, 14 lines):

Use a similar pub-mod + selective re-export shape so service/handlers/excel can be `use crate::reports::*` consumers.

```rust
pub mod handlers;
pub mod models;
pub mod money;
pub mod periods;
pub mod service;
pub mod excel;

pub use service::compute_report;
pub use models::{ReportPayload, EmployeeReportRow, ReportParamsRequest};
```

---

### `backend/src/reports/models.rs` (DTOs)

**Analog:** `backend/src/leaves/models.rs` (entire 62 lines) — `LeaveResponse` (Serialize), `CreateLeaveRequest` (Deserialize+Validate), `*ListQuery` (Deserialize).

**Key shapes for `reports/models.rs`:**

```rust
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct ReportParamsRequest {
    #[validate(length(equal = 10))]
    pub from_date: String,           // YYYY-MM-DD
    #[validate(length(equal = 10))]
    pub to_date: String,
    pub period_type: String,         // "weekly"|"biweekly_first"|"biweekly_second"|"monthly"|"custom"
    pub department_ids: Option<Vec<String>>,
    pub include_inactive: Option<bool>,
    pub employee_id: Option<String>,
    pub shift_type: Option<String>,  // "day"|"night"|"mixed"
}

#[derive(Debug, Serialize)]
pub struct ReportPayload {
    pub header: BrandingHeader,
    pub rows: Vec<EmployeeReportRow>,           // flat, sorted dept-then-name
    pub dept_subtotals: Vec<Aggregates>,        // one per dept (with dept_id)
    pub grand_total: Aggregates,
    pub departments_in_order: Vec<DeptSummary>, // for ordered iteration
}

#[derive(Debug, Serialize)]
pub struct BrandingHeader {
    pub client_name: String,            // empty → '—' at render
    pub client_rif: String,
    pub from_date: String,
    pub to_date: String,
    pub generated_at_iso: String,       // RFC 3339 via epoch_to_iso
}

#[derive(Debug, Serialize, Clone)]
pub struct EmployeeReportRow {
    pub employee_id: String,
    pub cedula: String,
    pub nombre: String,
    pub departamento: String,
    pub cargo: String,
    pub work_min: i64,
    pub ot_min: i64,
    pub late_min: i64,
    pub days_worked: i64,
    pub days_absent: i64,
    pub work_pay_cents: i64,
    pub ot_pay_cents: i64,
    pub night_premium_cents: i64,
    pub rest_day_surcharge_cents: i64,
    pub late_deduction_cents: i64,
    pub total_a_pagar_cents: i64,
    pub days_ivss: i64,
    pub days_vacation: i64,
    pub days_permission: i64,
    pub days_unpaid: i64,
    pub anomaly_codes: Vec<String>,
    pub anomaly_count: i64,             // derived from anomaly_codes.len() per RESEARCH Pitfall 4
}
```

Validation pattern from `leaves/models.rs` (`#[validate(length(equal = 10))]` for YYYY-MM-DD, `#[validate(length(min = 1, max = N))]` for free text). Sets `period_type` enum check via custom `#[validate(custom = "validate_period_type")]` or accept string + reject in service.

---

### `backend/src/reports/money.rs` (pure functions)

**Analog:** `backend/src/calc/overtime.rs` (pure module convention) + `backend/src/calc/aggregation.rs` lines for chrono helpers.

**Direct copy from RESEARCH Pattern 1 (lines 329-447).** All logic verified there with worked examples and test cases. Apply these conventions:

- Module sits in `reports/` (RESEARCH recommends own module — different domain than `calc/`).
- Inline `#[cfg(test)] mod tests` at bottom (matches `calc/overtime.rs` convention).
- All functions take `(work_minutes: i64, base_salary_cents: i64, ordinary_daily_minutes: i64) -> i64`.
- Use `checked_mul` then divide; `saturating_add`/`saturating_sub` in totals (RESEARCH Pitfall 2).
- Defensive `if ordinary_daily_minutes <= 0 { return 0; }` to handle misconfigured departments.

No analog in this codebase deviates from this — first-of-kind pure money math, but the structural conventions (pure fn + #[cfg(test)] inline) match `calc/overtime.rs`.

---

### `backend/src/reports/periods.rs` (period boundary math)

**Analog:** `backend/src/daily_records/service.rs` lines 145-148 (chrono ISO Monday math) + `calc/aggregation.rs`.

**Established ISO-week pattern** from `daily_records/service.rs`:

```rust
// daily_records/service.rs:144-148 — ISO Monday computation
let iso_week_monday = {
    let wd = anchor_date.weekday().num_days_from_monday();
    anchor_date - chrono::Duration::days(wd as i64)
};
```

**Apply to `reports/periods.rs::resolve_period`** with the full preset enum from RESEARCH Pattern 2 (lines 451-533). The "first of next month minus one day" pattern for EOM is `daily_records/service.rs:170` style:

```rust
// daily_records/service.rs:170 — Jan 1 of anchor_year
let year_start = NaiveDate::from_ymd_opt(anchor_date.year(), 1, 1).unwrap();
```

For "last day of month":
```rust
fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    let (next_year, next_month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    NaiveDate::from_ymd_opt(next_year, next_month, 1).unwrap() - chrono::Duration::days(1)
}
```

Per RESEARCH Pattern 2 — handles leap years correctly without per-month logic.

---

### `backend/src/reports/service.rs` (aggregation + audit insert)

**Analog 1:** `backend/src/anomalies/handlers.rs` lines 52-108 (dynamic predicate building with positional `?N` parameters):

```rust
// anomalies/handlers.rs:52-108 — pattern to copy
let mut predicates: Vec<String> = Vec::new();
let mut count_values: Vec<libsql::Value> = Vec::new();
let mut fetch_values: Vec<libsql::Value> = Vec::new();

if let Some(code) = &q.code {
    predicates.push(format!("dra.code = ?{}", predicates.len() + 1));
    count_values.push(libsql::Value::Text(code.clone()));
    fetch_values.push(libsql::Value::Text(code.clone()));
}
/* ... 4 more conditional predicates ... */

let where_clause = if predicates.is_empty() {
    String::new()
} else {
    format!("WHERE {}", predicates.join(" AND "))
};

let fetch_sql = format!(
    "SELECT dra.id, dra.daily_record_id, /* ... */ \
     FROM daily_record_anomalies dra \
     JOIN daily_records dr ON dr.id = dra.daily_record_id {} \
     ORDER BY dra.created_at DESC, dra.id ASC LIMIT ?{lim} OFFSET ?{off}",
    where_clause,
    lim = fetch_values.len() + 1,
    off = fetch_values.len() + 2,
);
```

**For dynamic IN clauses** (department_ids: Vec<String> per CONTEXT D-13), build placeholders manually:

```rust
// Pattern: turn Vec<String> into "?N,?N+1,?N+2,..." then params_from_iter
if let Some(dept_ids) = &q.department_ids {
    if !dept_ids.is_empty() {
        let placeholders: Vec<String> = dept_ids.iter().enumerate()
            .map(|(i, _)| format!("?{}", values.len() + 1 + i))
            .collect();
        predicates.push(format!("d.id IN ({})", placeholders.join(",")));
        for id in dept_ids {
            values.push(libsql::Value::Text(id.clone()));
        }
    }
}
```

(Established by daily_records/service.rs:439-462 + anomalies pattern; never use string concatenation per RESEARCH Pitfall 8 vector — same SQL injection avoidance applies.)

**Analog 2:** `backend/src/daily_records/service.rs` lines 116-142 (LEFT JOIN + override merge query shape):

The full report SQL is in RESEARCH Pattern 3 (lines 535-583). Apply daily_records' override-merge pattern: `LEFT JOIN daily_record_overrides dro ON dro.daily_record_id = dr.id AND dro.status = 'active'` then `COALESCE(dro.override_work_minutes, dr.work_minutes)` in Rust per-row.

**Analog 3 (audit insert):** `backend/src/devices/service.rs` lines 497-545 (`write_command_audit` — app-code audit row, not trigger):

```rust
// devices/service.rs:497-545 — app-code audit insert pattern
pub async fn write_command_audit(
    conn: &Connection,
    actor_id: &str,
    device_id: &str,
    command: Command,
    outcome: &CommandAuditOutcome,
    dispatched_at: i64,
    completed_at: i64,
) -> Result<(), AppError> {
    /* ... build outcome fields ... */
    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO command_audit_log (\
             id, actor_id, device_id, command, outcome, /* ... */\
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![id, actor_id.to_string(), /* ... */],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;

    Ok(())
}
```

**For Phase 5 audit insert** (D-21 — REPORT_EXPORT into shared `audit_log`, not `command_audit_log`):

```rust
// reports/service.rs — write_export_audit (NEW)
async fn write_export_audit(
    conn: &Connection,
    actor_id: &str,
    params: &ReportParamsRequest,
    format: &str,  // "json" | "excel"
) -> Result<(), AppError> {
    let id = Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "period_type": params.period_type,
        "from_date": params.from_date,
        "to_date": params.to_date,
        "filters": {
            "department_ids": params.department_ids,
            "include_inactive": params.include_inactive,
            "employee_id": params.employee_id,
            "shift_type": params.shift_type,
        },
        "format": format,
    });
    let synthetic_record_id = Uuid::new_v4().to_string();  // RESEARCH Don't Hand-Roll table

    conn.execute(
        "INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at) \
         VALUES (?1, 'reports', ?2, 'REPORT_EXPORT', NULL, ?3, ?4, unixepoch())",
        params![id, synthetic_record_id, payload.to_string(), actor_id.to_string()],
    )
    .await
    .map_err(|e| AppError::Internal(e.into()))?;
    Ok(())
}
```

**Note:** existing schema (`001_initial_schema.sql:73-82`) `audit_log.operation` has `CHECK(operation IN ('INSERT', 'UPDATE', 'DELETE'))`. Migration `014_phase5_audit_triggers.sql` MUST also relax this CHECK (or operation enum extension is needed). Migration approach: `CREATE TABLE audit_log_new` + copy data + drop old (SQLite has no `ALTER TABLE ... DROP CHECK`). **Flag for plan:** RESEARCH "Don't Hand-Roll" table proposes `operation='REPORT_EXPORT'` but the existing CHECK rejects this. Either (a) extend CHECK to include `'REPORT_EXPORT'` via table rebuild, or (b) use existing `'INSERT'` operation and put `REPORT_EXPORT` semantics in `payload_json`. Decision needed in plan stage.

**Order per RESEARCH Pitfall 7:** insert audit row AFTER `compute_report` succeeds and BEFORE the `Ok(...)` response is built. No phantom audit on backend errors.

---

### `backend/src/reports/handlers.rs` (JSON + Excel)

**Analog 1 (JSON handler):** `backend/src/rules/handlers.rs::update_rules` lines 54-133 (Json body + validate + AppError pattern).

```rust
// reports/handlers.rs::generate_json — apply rules.rs handler shape
pub async fn generate_json(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(params): Json<ReportParamsRequest>,
) -> Result<Json<ReportPayload>, AppError> {
    params.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    let payload = service::compute_report(&state, &claims.sub, &params, "json").await?;
    Ok(Json(payload))
}
```

**`AuthUser` extractor pattern** (`backend/src/auth/rbac.rs:11-27`) extracts JWT claims from request extensions inserted by middleware:

```rust
// auth/rbac.rs:11-27 — extractor to copy
pub struct AuthUser(pub Claims);

impl<S> FromRequestParts<S> for AuthUser
where S: Send + Sync,
{
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Claims>().cloned().map(AuthUser).ok_or(AppError::Unauthorized)
    }
}
```

Already imported as `use crate::auth::rbac::AuthUser;` in `daily_records/handlers.rs:11`.

**Analog 2 (Excel binary response):** `backend/src/leaves/handlers.rs::get_leave_evidence` lines 305-329:

```rust
// leaves/handlers.rs:305-329 — binary Vec<u8> response with custom headers
let bytes = tokio::fs::read(&canonical).await.map_err(|_| AppError::NotFound { /* ... */ })?;

let content_type = match PathBuf::from(&relpath).extension().and_then(|s| s.to_str()) {
    Some("pdf") => "application/pdf",
    /* ... */
};
let mut headers = HeaderMap::new();
headers.insert(
    header::CONTENT_TYPE,
    HeaderValue::from_static(/* ... */),
);
Ok((StatusCode::OK, headers, bytes).into_response())
```

**For `generate_excel`** apply the same `(StatusCode, HeaderMap, Vec<u8>).into_response()` shape; substitute MIME `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet` and add `Content-Disposition: attachment; filename="..."` per RESEARCH Pattern 5 (lines 696-730). Filename is server-built ASCII (`prenomina_YYYY-MM-DD_YYYY-MM-DD.xlsx`) so no UTF-8 encoding needed. Per RESEARCH Pitfall 9, ALWAYS double-quote: `format!("attachment; filename=\"{}\"", name)`.

**Wrap `excel::build_workbook` in `tokio::task::spawn_blocking`** per RESEARCH Pitfall 6:
```rust
let bytes = tokio::task::spawn_blocking(move || excel::build_workbook(&payload))
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!(e)))??;
```

---

### `backend/src/reports/excel.rs` (xlsx workbook builder)

**Analog:** First-of-kind in this codebase. Copy verbatim from RESEARCH Pattern 4 (lines 590-680).

**Structural conventions to mirror from existing handlers:**
- Function signature: `pub fn build_workbook(payload: &ReportPayload) -> Result<Vec<u8>, AppError>`.
- Error mapping: `.map_err(|e| AppError::Internal(anyhow::anyhow!("xlsx generation failed: {}", e)))` matches the `AppError::Internal(anyhow::anyhow!(...))` pattern from `daily_records/handlers.rs:203` (`write_photo_atomic` error mapping) and `rules/handlers.rs:45`.
- Pure (non-async) function — caller wraps in `spawn_blocking` per Pitfall 6.

**rust_xlsxwriter API (from RESEARCH lines 590-680):**
- `Workbook::new()`, `add_worksheet().set_name("Resumen")?`
- `Format::new().set_bold().set_bg_color(Color::RGB(0xE5E7EB))` (gray-200 for headers)
- Money cells: `set_num_format("$#,##0.00")` per D-33 (US/USD format)
- Anomaly rows: `sheet.set_row_format(row, &anomaly_tint)?` where `anomaly_tint = Format::new().set_bg_color(Color::RGB(0xFEF3C7))` (amber-100 — matches Phase 4 anomaly tint convention)
- `sheet.merge_range(0, 0, 0, 16, "Reporte Pre-Nómina", &header_title)?` for branding rows 1-3 (D-28)
- Per-dept subtotal + grand total rows (D-27): `Format::new().set_bold().set_top_border(FormatBorder::Thin)` for subtotal, `Format::new().set_bold().set_bg_color(Color::RGB(0xDBEAFE)).set_top_border(FormatBorder::Double)` for grand total (blue-100)
- `workbook.save_to_buffer()` → `Result<Vec<u8>, XlsxError>`

---

### `backend/Cargo.toml` (modify — add deps)

**Analog:** Self lines 7-49.

**Existing structure to extend:**

```toml
# Cargo.toml:6-49 (relevant blocks)
[dependencies]
# ... (alphabetical order — INSERT new dep alphabetically) ...
quick-xml = { version = "0.39.2", features = ["serialize"] }
rand = "0.8.5"
reqwest = { version = "0.13.2", default-features = false, features = ["rustls", "stream", "json"] }
# INSERT HERE: rust_xlsxwriter = "0.94.0"
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
axum-test = "16"
# INSERT HERE: calamine = "0.27"  (xlsx round-trip parsing for tests)
http-body-util = "0.1"
```

Per RESEARCH lines 159-166: `rust_xlsxwriter = "0.94.0"` (no `chrono` feature needed for v1 — chrono dates are formatted server-side as strings; no `zlib` feature initially).

---

### `backend/tests/tenant_info_test.rs` (integration test)

**Analog:** `backend/tests/leave_tests.rs` (entire harness — `make_state`, `build_test_app`, `body_to_json`, role-based tests).

**Test harness skeleton to copy:**

```rust
// leave_tests.rs:72-116 — test harness shape
fn make_state(db: libsql::Database) -> AppState {
    AppState {
        db: Arc::new(db),
        config: Arc::new(Config { /* ... */ jwt_secret: TEST_JWT_SECRET.to_string(), /* ... */ }),
        lifecycle_tx: None,
        recompute_tx: None,
        event_broadcast: None,
    }
}

fn build_test_app(state: AppState) -> Router {
    let viewer_routes = Router::new()
        .route("/tenant-info", get(tenant_info::handlers::get_tenant_info))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ));

    let admin_routes = Router::new()
        .route("/tenant-info", patch(tenant_info::handlers::patch_tenant_info))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_admin,
        ));

    Router::new()
        .nest("/api/v1", viewer_routes.merge(admin_routes))
        .with_state(state)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}
```

**Per-test pattern** (e.g., `viewer_can_get_tenant_info`):
```rust
let token = test_access_token(&user_id, "viewer", TEST_JWT_SECRET);
let req = Request::builder()
    .method(Method::GET)
    .uri("/api/v1/tenant-info")
    .header(header::AUTHORIZATION, format!("Bearer {}", token))
    .body(Body::empty()).unwrap();
let resp = app.clone().oneshot(req).await.unwrap();
assert_eq!(resp.status(), StatusCode::OK);
```

Tests required per RESEARCH lines 1061-1065: `get_returns_seed_row`, `admin_patch_succeeds`, `supervisor_blocked` (403), `version_conflict` (409), `audit_trigger_fires`.

---

### `backend/tests/reports_test.rs` (integration test)

**Analog:** `backend/tests/leave_tests.rs` (HTTP harness) + `backend/tests/calc_tests.rs` (per-function unit tests for money/periods).

Apply the same `make_state` + `build_test_app` skeleton with reports routes wired under `supervisor_read_routes` group. Tests required per RESEARCH lines 1037-1059 (15+ tests covering money math, period boundaries, override merge, medical leave exclusion, anomaly column, RBAC matrix, audit log entry on success / no entry on failure).

---

### `backend/tests/reports_excel_test.rs` (xlsx round-trip)

**Analog:** `backend/tests/leave_tests.rs` HTTP harness + `calamine = "0.27"` library docs (no codebase analog — first xlsx parser usage).

**Pattern:**
```rust
// 1. POST /reports/excel and capture binary body
let resp = app.oneshot(req).await.unwrap();
assert_eq!(resp.status(), StatusCode::OK);
let bytes = resp.into_body().collect().await.unwrap().to_bytes().to_vec();

// 2. Parse with calamine
let cursor = std::io::Cursor::new(bytes);
let mut workbook: calamine::Xlsx<_> = calamine::open_workbook_from_rs(cursor).unwrap();
let sheet = workbook.worksheet_range("Resumen").unwrap();

// 3. Assert header + row count + cell values
assert_eq!(sheet.get_value((0, 0)), Some(&calamine::Data::String("Reporte Pre-Nómina".into())));
```

Test list per RESEARCH lines 1048-1052: `excel_round_trip`, `excel_branding_header_present`, `excel_dept_subtotals_present`, `excel_response_headers`, `excel_anomaly_tint_snapshot`.

---

### `bruno/cronometrix/tenant-info/01_get.bru` + `02_patch.bru`

**Analog 1 (GET):** `bruno/cronometrix/employees/01_list.bru` (entire 26 lines):

```bru
meta {
  name: list employees
  type: http
  seq: 1
}

get {
  url: {{baseUrl}}/api/v1/employees?limit=20&offset=0
  body: none
  auth: bearer
}

params:query {
  limit: 20
  offset: 0
}

auth:bearer {
  token: {{access_token}}
}

script:post-response {
  if (res.body && res.body.data && res.body.data.length > 0) {
    bru.setEnvVar("sample_employee_id", res.body.data[0].id);
  }
}
```

For `tenant-info/01_get.bru`: drop pagination params, use `/api/v1/tenant-info`. Add post-response script to set `bru.setEnvVar("tenant_info_version", res.body.version)` so PATCH can use the latest version in optimistic concurrency.

**Analog 2 (PATCH):** `bruno/cronometrix/employees/02_create.bru` (entire 31 lines):

```bru
meta {
  name: create employee
  type: http
  seq: 2
}

post {
  url: {{baseUrl}}/api/v1/employees
  body: json
  auth: bearer
}

auth:bearer {
  token: {{access_token}}
}

body:json {
  {
    "cedula": "V-12345678",
    "name": "Juan Pérez",
    "department_id": "{{sample_department_id}}",
    "position": "Operario",
    "hire_date": "2026-01-15"
  }
}
```

For `tenant-info/02_patch.bru`: change `post` → `patch`, body to `{"client_name": "Acme Industria CA", "client_rif": "J-12345678-9", "address": "Caracas", "version": {{tenant_info_version}}}`.

---

### `bruno/cronometrix/reports/01_json.bru` + `02_excel.bru`

**Analog:** `bruno/cronometrix/employees/02_create.bru` (POST + json body + bearer pattern).

For `01_json.bru`:
- `post` to `{{baseUrl}}/api/v1/reports/json`
- `body:json` with `{"period_type": "monthly", "from_date": "2026-04-01", "to_date": "2026-04-30"}` (no filters initially).

For `02_excel.bru`:
- Same body but POST to `/api/v1/reports/excel`.
- Add `headers { Accept: application/vnd.openxmlformats-officedocument.spreadsheetml.sheet }` to make response inspection cleaner in Bruno UI.

---

### `frontend/src/app/(dashboard)/reports/page.tsx` (replace)

**Analog 1:** `frontend/src/app/(dashboard)/timesheet/page.tsx` (entire 96 lines — list+detail+filter+modal pattern).

**Page-level shape to copy** (RBAC gating, useState filters, useQuery, modal trigger):

```tsx
// timesheet/page.tsx:1-96 — copy this scaffold verbatim
'use client'
import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { startOfWeek, endOfWeek, format } from 'date-fns'
import { api } from '@/lib/api'
import { TopBar } from '@/components/layout/top-bar'
/* ... component imports ... */
import { useAuth } from '@/hooks/use-auth'

export default function ReportsPage() {
  const { role } = useAuth()
  const [filters, setFilters] = useState<ReportFilters>({ /* ... */ })
  const [drillDownEmployeeId, setDrillDownEmployeeId] = useState<string | null>(null)

  const { data, isLoading } = useQuery<ReportPayload>({
    queryKey: ['reports', filters],
    queryFn: () => api.post('/reports/json', filters).then(r => r.data),
    enabled: false,  // run on demand only — Emitir Reporte button triggers refetch
  })

  return (
    <div className="flex flex-col h-full">
      <TopBar title="Reportes" />
      <div className="p-6 space-y-4">
        <FiltersBar value={filters} onChange={setFilters} />
        {(role === 'admin' || role === 'supervisor') && (
          <ExportButtons payload={data} />
        )}
        <ReportSummaryTable
          payload={data}
          isLoading={isLoading}
          onDrillDown={setDrillDownEmployeeId}
        />
      </div>
      <DrillDownDialog
        employeeId={drillDownEmployeeId}
        from={filters.from_date}
        to={filters.to_date}
        onClose={() => setDrillDownEmployeeId(null)}
      />
    </div>
  )
}
```

**Analog 2 (filter row composition):** `frontend/src/app/(dashboard)/employees/page.tsx` lines 44-89 (search input + select + spacer + RBAC-gated buttons):

```tsx
// employees/page.tsx:44-89 — filter row pattern
<div className="flex items-center gap-3 flex-wrap">
  <input type="search" placeholder="Buscar…" /* ... */ className="rounded-md border border-slate-200 px-3 py-2 text-sm w-52" />
  <select /* ... */ className="rounded-md border border-slate-200 px-3 py-2 text-sm">
    {/* options */}
  </select>
  <div className="flex-1" />
  {(role === 'admin' || role === 'supervisor') && (
    <button className="px-4 py-2 border border-slate-200 text-sm rounded-md hover:bg-slate-50">
      Emitir Reporte
    </button>
  )}
</div>
```

---

### `frontend/src/components/reports/period-picker.tsx` (component)

**Analog:** `frontend/src/components/timesheet/week-navigator.tsx` (entire 35 lines).

```tsx
// week-navigator.tsx:1-35 — date-fns + onChange pattern
'use client'
import { startOfWeek, endOfWeek, addWeeks, subWeeks, format } from 'date-fns'

interface WeekNavigatorProps {
  currentDate: Date
  onChange: (date: Date) => void
}

export function WeekNavigator({ currentDate, onChange }: WeekNavigatorProps) {
  // Pitfall 7: always weekStartsOn: 1 (Monday) — Venezuela LOTTT work week
  const weekStart = startOfWeek(currentDate, { weekStartsOn: 1 })
  /* ... */
}
```

**For `period-picker.tsx`** — keep `weekStartsOn: 1` invariant (CONTEXT D-10 ISO Mon-Sun). Add a select dropdown for `Semanal` / `Quincenal` / `Mensual` / `Personalizado`. For `Personalizado` show two date inputs (HTML5 `<input type="date">` for v1 — shadcn DateRangePicker can come later if needed).

---

### `frontend/src/components/reports/summary-table.tsx` (TanStack Table v8)

**Analog:** `frontend/src/components/timesheet/timesheet-table.tsx` (entire 195 lines) + `frontend/src/components/employees/employee-table.tsx` (entire 162 lines).

**TanStack Table v8 setup pattern** (from `timesheet-table.tsx:132-145`):

```tsx
// timesheet-table.tsx:132-145 — table setup
const table = useReactTable({
  data,
  columns,
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

**Column definition pattern** (from `employee-table.tsx:37-82`):
```tsx
const columns: ColumnDef<Employee>[] = [
  { accessorKey: 'name', header: 'Nombre' },
  { accessorKey: 'cedula', header: 'Cédula' },
  /* ... custom cell renderer ... */
  {
    accessorKey: 'hire_date',
    header: 'Fecha Ingreso',
    cell: ({ getValue }) => {
      try { return format(new Date(getValue() as string), 'dd/MM/yyyy') } catch { return '—' }
    },
  },
  /* RBAC-gated columns */
  ...(role === 'admin' ? [{ /* ... */ }] : []),
]
```

**For Reports summary-table** apply RESEARCH Pattern 7 (lines 859-895) — synthetic subtotal rows with `_kind` discriminator:

```tsx
type RowKind = 'data' | 'subtotal' | 'grandtotal'
type TableRow = (EmployeeReportRow | AggregatesRow) & { _kind: RowKind, _key: string }

function buildTableRows(payload: ReportPayload): TableRow[] { /* ... */ }
```

Conditional row styling on `<tr>` based on `row.original._kind`:
- `data` + `anomaly_count > 0` → `bg-amber-50` (matches Phase 4 anomaly convention from `timesheet-table.tsx:18` `bg-amber-100`)
- `subtotal` → `font-semibold border-t`
- `grandtotal` → `font-bold bg-blue-50 border-t-2`

**Money cell renderer** uses `frontend/src/lib/format/currency.ts::fmtMoney(cents)` (new file).

---

### `frontend/src/components/reports/drill-down-dialog.tsx` (component)

**Analog:** `frontend/src/components/timesheet/novedad-modal.tsx` (entire 238 lines — shadcn Dialog wrapper + useQuery pattern).

**Dialog primitive usage** (`novedad-modal.tsx:76-90`):
```tsx
// novedad-modal.tsx:76-90 — Dialog shape
<Dialog open={open} onOpenChange={(o) => { if (!o) handleClose() }}>
  <DialogContent className="max-w-lg">
    <DialogHeader>
      <DialogTitle>Registrar Novedad</DialogTitle>
    </DialogHeader>
    <form onSubmit={handleSubmit((v) => mutation.mutate(v))} className="space-y-4">
      /* ... */
    </form>
  </DialogContent>
</Dialog>
```

**For `drill-down-dialog.tsx`** — same shape but read-only (no form), title `"Detalle por Día — {employee_name}"`, body shows TanStack Table populated by `useQuery({queryKey: ['daily-records', employeeId, from, to], queryFn: () => api.get('/daily-records', { params: { employee_id: employeeId, from_date: from, to_date: to } }).then(r => r.data)})`. CONTEXT D-15 — reuses existing endpoint. No new backend route.

---

### `frontend/src/components/reports/export-buttons.tsx` (component)

**Analog:** `frontend/src/components/timesheet/novedad-modal.tsx` lines 40-69 (`useMutation` + `isPending` + `sonner` toast).

**Mutation pattern** (`novedad-modal.tsx:40-69`):

```tsx
// novedad-modal.tsx:40-69 — useMutation pattern
const mutation = useMutation({
  mutationFn: async (values: NovedadFormData) => {
    const fd = new FormData()
    /* ... */
    await api.post('/daily-records/${record.id}/overrides', fd, {
      headers: { 'Content-Type': 'multipart/form-data' },
    })
  },
  onSuccess: () => {
    queryClient.invalidateQueries({ queryKey: ['daily-records'] })
    reset()
    onClose()
  },
})
/* ... */
<Button type="submit" disabled={isSubmitting || mutation.isPending}>
  {mutation.isPending ? 'Registrando…' : 'Registrar Novedad'}
</Button>
```

**For Excel export** (`exportExcelMutation`):
```tsx
const exportExcelMutation = useMutation({
  mutationFn: async () => {
    const resp = await api.post('/reports/excel', filters, { responseType: 'blob' })
    const blob = new Blob([resp.data], { type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `prenomina_${filters.from_date}_${filters.to_date}.xlsx`
    a.click()
    URL.revokeObjectURL(url)
  },
  onSuccess: () => toast.success('Reporte Excel descargado'),
  onError: () => toast.error('Error al generar el reporte'),
})
```

**For PDF export** — fetch JSON via `api.post('/reports/json', filters)`, then call `renderReportPdf(payload)` from `lib/reports/pdf.ts`. Sonner toast already imported via `frontend/src/lib/api.ts:3`.

**RBAC gating:** wrap buttons in `{(role === 'admin' || role === 'supervisor') && (...)}` per CONTEXT D-20. Pattern from `employees/page.tsx:78-82`.

---

### `frontend/src/lib/reports/pdf.ts` (utility)

**Analog:** First-of-kind. Copy verbatim from RESEARCH Pattern 6 (lines 750-857).

**Structural conventions:**
- Pure function, default export `renderReportPdf(payload: ReportPayload): void`.
- Uses `'helvetica'` font (WinAnsi covers all Spanish accents per RESEARCH Pitfall 5).
- `doc.save(fileName)` triggers download via the standard jsPDF API.
- Stable `creationDate` via `doc.setProperties({ creationDate: payload.header.generated_at })` for deterministic test snapshots (RESEARCH Reconciliation Invariant 9).

Add `frontend/src/lib/reports/pdf.test.ts` to satisfy RESEARCH lines 1054-1056 PDF test gates.

---

### `frontend/src/lib/format/currency.ts` (utility)

**Analog:** `frontend/src/lib/utils.ts` (existing 166B `cn()` helper) — minimal pure-function module.

**Per CONTEXT D-33** — Intl.NumberFormat with `en-US` for US-style USD parity with Excel `$#,##0.00`:

```ts
// frontend/src/lib/format/currency.ts
const usdFormatter = new Intl.NumberFormat('en-US', {
  style: 'currency',
  currency: 'USD',
})

/** Format integer cents as USD currency string per D-33 */
export function fmtMoney(cents: number | null | undefined): string {
  if (cents === null || cents === undefined) return '—'
  return usdFormatter.format(cents / 100)
}

/** Negative variant for late deduction column — already stored positive, displayed with leading '-' */
export function fmtMoneyNegative(cents: number): string {
  return '-' + fmtMoney(cents)
}
```

Tree-shakeable, no runtime cost beyond Intl which is built into the browser.

---

### `frontend/src/app/(dashboard)/settings/tenant-info/page.tsx` (Admin form)

**Analog 1:** `frontend/src/components/timesheet/novedad-modal.tsx` lines 1-69 (react-hook-form + zod + useMutation).

```tsx
// novedad-modal.tsx:29-69 — form scaffold
const {
  register,
  handleSubmit,
  control,
  reset,
  formState: { errors, isSubmitting },
} = useForm<NovedadFormData>({
  resolver: zodResolver(novedadSchema),
  defaultValues: { /* ... */ },
})

const mutation = useMutation({
  mutationFn: async (values: NovedadFormData) => { /* ... */ },
  onSuccess: () => {
    queryClient.invalidateQueries({ queryKey: ['daily-records'] })
    reset()
    onClose()
  },
})
```

**For tenant-info form:**
- Fetch current values with `useQuery({queryKey: ['tenant-info'], queryFn: () => api.get('/tenant-info').then(r => r.data)})`.
- Initialize form with `reset(data)` on query success (use `useEffect` watching `data`).
- Validate via `tenantInfoSchema` (new entry in `frontend/src/lib/validations.ts`):
  ```ts
  export const tenantInfoSchema = z.object({
    client_name: z.string().max(200),
    client_rif: z.string().regex(/^[VJG]-\d+-\d$/, 'RIF inválido (formato: J-12345678-9)').or(z.literal('')),
    address: z.string().max(500),
    version: z.number(),
  })
  ```
- PATCH on submit with `version` in body for optimistic concurrency. On 409 (`VERSION_CONFLICT`) refetch and toast `Esta información fue modificada por otro usuario; recargando…`.

**Analog 2 (RBAC read-only mode):** `frontend/src/app/(dashboard)/employees/page.tsx:78-89` (gated buttons):

```tsx
// employees/page.tsx:78-89 — Admin-only mutation controls
{(role === 'admin' || role === 'supervisor') && (
  <button>Emitir Reporte</button>
)}
{role === 'admin' && (
  <button>Nuevo Empleado</button>
)}
```

**For tenant-info page** — render fields as `<input disabled>` for non-Admin, hide submit button.

---

### `frontend/src/components/layout/sidebar.tsx` (modify — add nav item)

**Analog:** Self lines 9-17 (`NAV_ITEMS` array).

**Existing `NAV_ITEMS`** (lines 9-17):

```tsx
// sidebar.tsx:9-17
const NAV_ITEMS = [
  { href: '/dashboard', icon: LayoutDashboard, label: 'Dashboard' },
  { href: '/timesheet', icon: Clock, label: 'Marcaciones' },
  { href: '/employees', icon: Users, label: 'Empleados' },
  { href: '/devices', icon: Cpu, label: 'Dispositivos' },
  { href: '/enrollment', icon: UserCheck, label: 'Enrolamiento' },
  { href: '/reports', icon: BarChart2, label: 'Reportes' },
  { href: '/audit', icon: ShieldCheck, label: 'Auditoría' },
]
```

**Append** `{ href: '/settings/tenant-info', icon: Settings, label: 'Configuración' }` (import `Settings` from `lucide-react`).

**Admin-only filter** — currently `NAV_ITEMS` is rendered unconditionally; add a per-item `roles?: Role[]` field and filter in `<nav>` body using `useAuth().role`. Pattern:

```tsx
const visibleItems = NAV_ITEMS.filter(item => !item.roles || item.roles.includes(role))
```

Mark `Configuración` as `{ ..., roles: ['admin'] }`. Active-state regex (lines 31-34) already uses prefix-aware matching with WR-07 fix — no change needed.

---

### `frontend/src/lib/api.ts` (modify — no changes needed)

**Analog:** Self (entire 84 lines).

The existing `api` axios instance + 401 interceptor handles all Phase 5 endpoints. No code change needed beyond callers passing new query keys. **`responseType: 'blob'`** option is per-request (set in `export-buttons.tsx`'s mutationFn) and does NOT require interceptor changes.

---

### `frontend/package.json` (modify — add deps)

**Analog:** Self.

Add to `dependencies`:
```json
"jspdf": "^4.2.1",
"jspdf-autotable": "^5.0.7"
```

Per RESEARCH lines 167-170. Both ESM-first, framework-agnostic, no peer-dep conflicts with React 19/Next.js 16.

---

## Shared Patterns

### Authentication / RBAC

**Source:** `backend/src/auth/rbac.rs:11-77`
**Apply to:** All Phase 5 backend handlers

```rust
// rbac.rs:11-27 — extract JWT claims from request extensions
pub struct AuthUser(pub Claims);

impl<S> FromRequestParts<S> for AuthUser
where S: Send + Sync,
{
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Claims>().cloned().map(AuthUser).ok_or(AppError::Unauthorized)
    }
}

// rbac.rs:55-76 — middleware that requires Admin or Supervisor role
pub async fn require_supervisor_or_above(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = req.headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;
    let claims = service::verify_access_token(token, state.config.jwt_secret.as_bytes())?;
    match claims.role {
        Role::Admin | Role::Supervisor => {}
        Role::Viewer => return Err(AppError::Forbidden),
    }
    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}
```

**Phase 5 application:**
- `tenant_info::handlers::get_tenant_info` → `require_auth` (all roles)
- `tenant_info::handlers::patch_tenant_info` → `require_admin`
- `reports::handlers::generate_json` + `generate_excel` → `require_supervisor_or_above`
- All handlers extract `AuthUser(claims)` to capture `claims.sub` for audit logs.

### Error Handling

**Source:** `backend/src/errors.rs:1-125`
**Apply to:** All backend modules

**Variant catalog** (already covers Phase 5 needs — no new variant required):
- `AppError::Validation { code: "VALIDATION_ERROR", message }` → 422 (use for `.validate().map_err(...)` after Validator-derive)
- `AppError::Conflict { code: "VERSION_CONFLICT", message }` → 409 (use for tenant_info optimistic concurrency)
- `AppError::NotFound { code, message }` → 404
- `AppError::Forbidden` → 403 (auto-emitted by `require_supervisor_or_above`)
- `AppError::Internal(anyhow::anyhow!(...))` → 500 (catch-all for libsql, xlsxwriter, etc.)

**Conversion shape used everywhere:**
```rust
.map_err(|e| AppError::Internal(e.into()))?      // for libsql Errors
.map_err(|e| AppError::Validation { code: "VALIDATION_ERROR", message: e.to_string() })?
```

### Optimistic Concurrency (version column)

**Source:** `backend/src/employees/service.rs:208-269` and `backend/src/rules/handlers.rs:54-115`
**Apply to:** `tenant_info::service::update_tenant_info`

Pattern: `UPDATE ... SET version = version + 1 WHERE id = ? AND version = ?` → if `rows_affected == 0`, return `AppError::Conflict { code: "VERSION_CONFLICT", message: ... }`. Always include both `id` AND `version` in WHERE (RESEARCH Pitfall 8 — `WHERE id = 1 AND version = ?`).

### Dynamic SQL Predicate Building (no string concat)

**Source:** `backend/src/anomalies/handlers.rs:52-108` and `backend/src/daily_records/service.rs:439-462`
**Apply to:** `reports::service::compute_report` (filter combinations)

Pattern: `Vec<String>` for `predicates` + `Vec<libsql::Value>` for `values`, build `?N` placeholders by `values.len() + 1` index, join with ` AND `, pass via `libsql::params_from_iter(values)`. Never concatenate user input into SQL.

### App-Code Audit Insert (non-trigger audit)

**Source:** `backend/src/devices/service.rs:497-545` (`write_command_audit`)
**Apply to:** `reports::service::write_export_audit`

Pattern: insert into the audit table directly with `actor_id` from JWT claims, after the action succeeds. UUID v4 for the row id. Use `serde_json::json!(...)` then `.to_string()` for the payload.

### Validator-Derive DTOs

**Source:** `backend/src/leaves/models.rs:29-40` and `backend/src/rules/models.rs:17-26`
**Apply to:** `ReportParamsRequest`, `UpdateTenantInfoRequest`

Pattern: `#[derive(Debug, Deserialize, Validate)]` + `#[validate(length(...))]` / `#[validate(range(...))]` macros. Call `.validate()` at the top of every handler (before any DB call) and convert the error to `AppError::Validation`.

### TanStack Query + RBAC UI Gating

**Source:** `frontend/src/app/(dashboard)/timesheet/page.tsx:30-43` (useQuery) + `frontend/src/app/(dashboard)/employees/page.tsx:78-89` (RBAC button)
**Apply to:** All Phase 5 frontend screens

Pattern:
- Filters in `useState`, query key includes them so refetch triggers automatically.
- `(role === 'admin' || role === 'supervisor') && <Button>Emitir Reporte</Button>`.
- For mutations: `useMutation({ mutationFn, onSuccess: () => queryClient.invalidateQueries(...), onError: () => toast.error(...) })`.
- 401 retry handled centrally in `frontend/src/lib/api.ts:50-77`.

### TanStack Table v8 Setup

**Source:** `frontend/src/components/timesheet/timesheet-table.tsx:132-145` and `frontend/src/components/employees/employee-table.tsx:84-96`
**Apply to:** `frontend/src/components/reports/summary-table.tsx`

Pattern: `useReactTable({ data, columns, pageCount, state: { pagination }, onPaginationChange, getCoreRowModel: getCoreRowModel(), manualPagination: true, manualFiltering: true })`. Render via `flexRender(header.column.columnDef.header, header.getContext())`.

### Tailwind Color Tokens (anomaly tinting)

**Source:** `frontend/src/components/timesheet/timesheet-table.tsx:18,24,31,37` (status badge tints)
**Apply to:** `frontend/src/components/reports/summary-table.tsx`

Established palette:
- `bg-amber-100 text-amber-700` — anomaly / warning
- `bg-yellow-100 text-yellow-700` — justified
- `bg-green-100 text-green-700` — normal / success
- `bg-red-100 text-red-700` — absent / error
- `bg-blue-50` / `bg-blue-100` — informational / grand total

Excel mirror tokens (from RESEARCH Pattern 4):
- `Color::RGB(0xFEF3C7)` (amber-100) — anomaly row tint
- `Color::RGB(0xDBEAFE)` (blue-100) — grand total row
- `Color::RGB(0xE5E7EB)` (gray-200) — column headers

### Migration Registration

**Source:** `backend/src/db/mod.rs:9-58` (`MIGRATIONS` array)
**Apply to:** All three new SQL files

Pattern: append a new `(name, include_str!("migrations/NNN_name.sql"))` tuple to the `MIGRATIONS` const array, in numeric order. The `run_migrations` runner (line 139-185) is idempotent via `_migrations` tracking table — re-runs are safe.

---

## No Analog Found

| File | Role | Data Flow | Reason |
|------|------|-----------|--------|
| `backend/src/reports/excel.rs` | binary builder | transform | First-of-kind: no `rust_xlsxwriter` usage in codebase. Copy API verbatim from RESEARCH Pattern 4 (lines 590-680). Structural conventions (function signature, error mapping, pure non-async) borrowed from existing handlers. |
| `frontend/src/lib/reports/pdf.ts` | utility | transform | First-of-kind: no `jspdf` / `jspdf-autotable` usage in codebase. Copy verbatim from RESEARCH Pattern 6 (lines 750-857). |
| `backend/tests/reports_excel_test.rs` (calamine round-trip portion) | integration test | transform | First-of-kind: no `calamine` usage in codebase. Library API documented in RESEARCH Validation Architecture and standard cargo docs. HTTP-harness portion has analog in `backend/tests/leave_tests.rs`. |

---

## Phase-Specific Gotchas (cross-reference RESEARCH Pitfalls)

These items are pattern-adjacent and the planner MUST surface them in plan tasks:

1. **`audit_log.operation` CHECK constraint excludes `'REPORT_EXPORT'`** (`001_initial_schema.sql:77`). Either:
   - **Option A:** Migration 014 rebuilds the table with relaxed CHECK to include `'REPORT_EXPORT'`. SQLite requires CREATE TABLE _new + INSERT SELECT + DROP + RENAME.
   - **Option B:** App-code insert uses `operation='INSERT'` and stores semantic in `payload_json` (less clean but no schema rewrite).
   Recommend Option A in plan.

2. **`audit_employees_update` trigger** (`002_audit_triggers.sql:29-43`) hashes only the original Phase 1 columns. Adding `position` + `hire_date` requires DROP + RECREATE TRIGGER in migration 014 to ensure the new columns are captured in audit history. CONTEXT D-30a's claim "no trigger update needed" is incorrect.

3. **Excel binary response handler** must wrap `excel::build_workbook` in `tokio::task::spawn_blocking` per RESEARCH Pitfall 6. Direct sync call works for low concurrency but starves the runtime under burst load.

4. **Period date validation** must reject `to_date - from_date > 366 days` per RESEARCH Security V13 (DoS via unbounded period range). Use a custom validator or check at handler entry before calling service.

5. **Filename quoting in `Content-Disposition`** must use double quotes (`format!("attachment; filename=\"{}\"", name)`). RESEARCH Pitfall 9.

6. **Total `total_a_pagar` math** must use `saturating_add` / `saturating_sub`, not raw `+` / `-`. RESEARCH Pitfall 2.

7. **Currency display: `en-US` USD format** per CONTEXT D-33 — overrides the Spanish locale assumption. Excel num_format `$#,##0.00`, JS `Intl.NumberFormat('en-US', {style: 'currency', currency: 'USD'})`.

8. **Override merge in reports query** must use `LEFT JOIN daily_record_overrides ... AND status = 'active'` then `COALESCE` per RESEARCH Pitfall 3 — direct `daily_records.work_minutes` reads bypass operator edits.

---

## Metadata

**Analog search scope:**
- `backend/src/{auth,common,db,daily_records,departments,devices,employees,errors,leaves,main,rules,state,anomalies,calc}.rs`
- `backend/src/db/migrations/*.sql`
- `backend/tests/{leave,calc,daily_record,employee,department}_tests.rs`
- `bruno/cronometrix/{employees,leaves,daily-records}/*.bru`
- `frontend/src/app/(dashboard)/{employees,timesheet,reports}/page.tsx`
- `frontend/src/components/{timesheet,employees,layout,ui}/*`
- `frontend/src/{lib,hooks,contexts,types}/**`

**Files scanned:** 60+ source files reviewed
**Pattern extraction date:** 2026-04-25
