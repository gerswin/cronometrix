// M-06: all three `employees` audit triggers (`audit_employees_insert`,
// `audit_employees_update`, `audit_employees_delete`) silently dropped
// columns every time they were recreated to add one new column
// (018_employees_base_salary.sql dropped `position`, `hire_date`, `face_id`
// from all three; migrations 024 and 026 then added `salary_kind` and
// `terminated_on` without any of the three being revisited).
// Migration 028_employee_audit_full_columns.sql fixes this by putting every
// column of `employees` (verified via PRAGMA table_info, not assumed) back
// into all three triggers' JSON snapshots — `new` only for insert, `old`
// only for delete, both for update.
//
// These tests exercise the real HTTP layer where practical (salary_kind,
// position, hire_date, terminated_on) and drop to direct SQL for face_id and
// the remaining columns that have no PATCH-able API surface, to confirm the
// trigger itself — not just today's API shape — captures every column.
mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::{delete, get, patch, post};
use axum::Router;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::departments;
use cronometrix_api::employees;
use cronometrix_api::state::AppState;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

/// Build AppState with employees + departments wired in. Returns (AppState, TempDir)
/// per Plan 08-02 D-20: caller binds the TempDir to a local that outlives every
/// assertion (Pitfall 1 in 08-RESEARCH.md). `state.db` (`Arc<libsql::Database>`) is
/// cloned out by the caller *before* the state is handed to `build_test_app`, so
/// audit_log can still be queried directly after the router takes ownership of state.
fn make_state(db: libsql::Database) -> (AppState, tempfile::TempDir) {
    let config = Arc::new(Config {
        database_path: "test".to_string(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: common::TEST_JWT_SECRET.to_string(),
        server_host: "127.0.0.1".to_string(),
        server_port: 3001,
        turso_sync_interval_secs: 300,
        device_creds_key: common::test_device_creds_key(),
        timezone: "America/Caracas".parse().unwrap(),
        license_jwt_path: String::new(),
        do_functions_activate_url: String::new(),
        do_functions_renew_url: String::new(),
        cors_allowed_origins: Vec::new(),
        cookie_secure: false,
        device_push_base_url: String::new(),
    });

    common::test_state_with_tmpdir(Arc::new(db), config)
}

/// Build a test app with employee + department routes. Mirrors employee_tests.rs's
/// helper — each `tests/*.rs` file is its own binary, so it can't be shared directly.
fn build_test_app(state: AppState) -> Router {
    let viewer_routes = Router::new()
        .route("/employees", get(employees::handlers::list_employees))
        .route("/employees/{id}", get(employees::handlers::get_employee))
        .route("/departments", get(departments::handlers::list_departments))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ));

    let supervisor_routes = Router::new()
        .route("/employees", post(employees::handlers::create_employee))
        .route(
            "/employees/{id}",
            patch(employees::handlers::update_employee),
        )
        .route(
            "/departments",
            post(departments::handlers::create_department),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_supervisor_or_above,
        ));

    let admin_routes = Router::new()
        .route(
            "/employees/{id}",
            delete(employees::handlers::deactivate_employee),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_admin,
        ));

    Router::new()
        .nest(
            "/api/v1",
            viewer_routes.merge(supervisor_routes).merge(admin_routes),
        )
        .with_state(state)
}

async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

async fn create_test_department(app: &Router, token: &str, name: &str) -> String {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/departments")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "name": name,
                "base_salary_cents": 100000,
                "shift_start_time": "08:00",
                "shift_end_time": "17:00",
                "lunch_mode": "fixed",
                "lunch_duration_min": 60
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let body = body_to_json(resp.into_body()).await;
    body["id"].as_str().unwrap().to_string()
}

/// Fetch the most recent audit_log row for (table_name, record_id, operation)
/// and return (old_data, new_data) parsed as JSON. Ordered by rowid (not
/// created_at, which only has 1-second resolution) so it is the true last write.
async fn latest_audit_row(
    db: &libsql::Database,
    table_name: &str,
    record_id: &str,
    operation: &str,
) -> (Value, Value) {
    let conn = db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT old_data, new_data FROM audit_log \
             WHERE table_name = ?1 AND record_id = ?2 AND operation = ?3 \
             ORDER BY rowid DESC LIMIT 1",
            libsql::params![
                table_name.to_string(),
                record_id.to_string(),
                operation.to_string()
            ],
        )
        .await
        .unwrap();
    let row = rows
        .next()
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("no {operation} audit_log row for {table_name}/{record_id}"));
    let old_data: Option<String> = row.get(0).unwrap();
    let new_data: Option<String> = row.get(1).unwrap();
    let old: Value = old_data
        .map(|s| serde_json::from_str(&s).unwrap())
        .unwrap_or(Value::Null);
    let new: Value = new_data
        .map(|s| serde_json::from_str(&s).unwrap())
        .unwrap_or(Value::Null);
    (old, new)
}

/// M-06: changing salary_kind changes what an employee is paid by a factor of
/// thirty (daily vs. monthly). Before migration 028 this was invisible to the
/// audit trail — `audit_employees_update` didn't have the column at all.
#[tokio::test]
async fn changing_salary_kind_is_audited() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let (state, _tmp) = make_state(db);
    let db_for_audit = state.db.clone();
    let app = build_test_app(state);

    let dept_id = create_test_department(&app, &token, "Payroll Dept").await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/employees")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "employee_code": "SK001",
                "name": "Kind Employee",
                "department_id": dept_id,
                "base_salary_cents": 5000,
                "salary_kind": "daily"
            })
            .to_string(),
        ))
        .unwrap();
    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let created = body_to_json(create_resp.into_body()).await;
    let emp_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["salary_kind"], "daily");

    // PATCH salary_kind daily -> monthly
    let patch_req = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/api/v1/employees/{}", emp_id))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "salary_kind": "monthly",
                "version": 1
            })
            .to_string(),
        ))
        .unwrap();
    let patch_resp = app.clone().oneshot(patch_req).await.unwrap();
    assert_eq!(patch_resp.status(), StatusCode::OK);
    let patched = body_to_json(patch_resp.into_body()).await;
    assert_eq!(patched["salary_kind"], "monthly");

    // audit_log must have the row with old and new salary_kind.
    let (old, new) = latest_audit_row(&db_for_audit, "employees", &emp_id, "UPDATE").await;
    assert_eq!(
        old["salary_kind"], "daily",
        "old_data missing/wrong salary_kind: {old:?}"
    );
    assert_eq!(
        new["salary_kind"], "monthly",
        "new_data missing/wrong salary_kind: {new:?}"
    );
}

/// The rest of the columns the trigger lost when it was recreated in
/// 018_employees_base_salary.sql: `position`, `hire_date` (both present
/// before 018, dropped by it) and `terminated_on` (added by migration 026,
/// never added to the trigger at all).
#[tokio::test]
async fn position_hire_date_and_terminated_on_are_audited() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let (state, _tmp) = make_state(db);
    let db_for_audit = state.db.clone();
    let app = build_test_app(state);

    let dept_id = create_test_department(&app, &token, "Tenure Dept").await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/employees")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "employee_code": "PH001",
                "name": "Tenure Employee",
                "department_id": dept_id,
                "position": "Clerk",
                "hire_date": "2024-01-15",
                "base_salary_cents": 5000,
                "salary_kind": "daily"
            })
            .to_string(),
        ))
        .unwrap();
    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let created = body_to_json(create_resp.into_body()).await;
    let emp_id = created["id"].as_str().unwrap().to_string();

    // PATCH position + hire_date
    let patch_req = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/api/v1/employees/{}", emp_id))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "position": "Supervisor",
                "hire_date": "2024-06-01",
                "version": 1
            })
            .to_string(),
        ))
        .unwrap();
    let patch_resp = app.clone().oneshot(patch_req).await.unwrap();
    assert_eq!(patch_resp.status(), StatusCode::OK);

    let (old, new) = latest_audit_row(&db_for_audit, "employees", &emp_id, "UPDATE").await;
    assert_eq!(
        old["position"], "Clerk",
        "old_data missing position: {old:?}"
    );
    assert_eq!(
        new["position"], "Supervisor",
        "new_data missing position: {new:?}"
    );
    assert!(
        old["hire_date"].is_number(),
        "old_data missing hire_date: {old:?}"
    );
    assert!(
        new["hire_date"].is_number(),
        "new_data missing hire_date: {new:?}"
    );
    assert_ne!(
        old["hire_date"], new["hire_date"],
        "hire_date should have changed between old and new"
    );

    // Deactivating (DELETE) is the one place terminated_on is stamped
    // (service::deactivate_queued, C-05) — it is not PATCH-able directly.
    let delete_req = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/api/v1/employees/{}", emp_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let delete_resp = app.clone().oneshot(delete_req).await.unwrap();
    assert_eq!(delete_resp.status(), StatusCode::NO_CONTENT);

    let (old, new) = latest_audit_row(&db_for_audit, "employees", &emp_id, "UPDATE").await;
    assert!(
        old["terminated_on"].is_null(),
        "old_data terminated_on should have been null before deactivation: {old:?}"
    );
    assert!(
        new["terminated_on"].is_number(),
        "new_data missing terminated_on after deactivation: {new:?}"
    );
    assert_eq!(old["status"], "active");
    assert_eq!(new["status"], "inactive");
}

/// M-06 also names `face_id` — assigned during facial enrollment, not through
/// UpdateEmployeeRequest. Exercised directly against the DB (the same layer
/// the enrollment service writes through) so the test isn't coupled to the
/// enrollment HTTP flow, only to the fact that an UPDATE touching face_id
/// fires the trigger and is captured.
#[tokio::test]
async fn face_id_change_is_audited() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();

    let dept_id = common::create_test_department_with_shift(
        &db,
        "Face Dept",
        "day",
        false,
        480,
        "08:00",
        "17:00",
    )
    .await;
    let emp_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, 'FACE001', 'Face Employee', ?2, 'active', 1, unixepoch(), unixepoch())",
        libsql::params![emp_id.clone(), dept_id],
    )
    .await
    .unwrap();

    conn.execute(
        "UPDATE employees SET face_id = ?1, version = version + 1 WHERE id = ?2",
        libsql::params!["face-uuid-123".to_string(), emp_id.clone()],
    )
    .await
    .unwrap();

    let (old, new) = latest_audit_row(&db, "employees", &emp_id, "UPDATE").await;
    assert!(
        old["face_id"].is_null(),
        "old_data face_id should be null before assignment: {old:?}"
    );
    assert_eq!(
        new["face_id"], "face-uuid-123",
        "new_data missing/wrong face_id: {new:?}"
    );
}

/// Comprehensive check that every column PRAGMA table_info(employees) reports
/// today is present in the trigger's JSON — not just the five named in the
/// finding. Written directly against the DB layer (not the HTTP API, which
/// doesn't expose `deleted_at`/`created_at`/`updated_at`/
/// `current_face_enrollment_id` for direct mutation) so it verifies the
/// trigger itself, independent of whatever the API happens to expose today.
#[tokio::test]
async fn every_employees_column_is_captured_by_the_trigger() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();

    // Confirm our column list matches the live schema, so this test fails
    // loudly (not silently under-covers) if a column is renamed or removed.
    let mut cols = conn
        .query("PRAGMA table_info(employees)", ())
        .await
        .unwrap();
    let mut actual_columns = Vec::new();
    while let Some(row) = cols.next().await.unwrap() {
        let name: String = row.get(1).unwrap();
        actual_columns.push(name);
    }
    let expected_columns = vec![
        "id",
        "employee_code",
        "name",
        "department_id",
        "status",
        "deleted_at",
        "version",
        "created_at",
        "updated_at",
        "position",
        "hire_date",
        "face_id",
        "current_face_enrollment_id",
        "base_salary_cents",
        "salary_kind",
        "terminated_on",
    ];
    assert_eq!(
        actual_columns, expected_columns,
        "employees schema changed — update this test AND migration 028's trigger"
    );

    let dept_id = common::create_test_department_with_shift(
        &db,
        "Full Column Dept",
        "day",
        false,
        480,
        "08:00",
        "17:00",
    )
    .await;

    let user_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Enroller', 'hash', 'admin', 'active', 1, unixepoch(), unixepoch())",
        libsql::params![user_id.clone(), format!("enroller-{}", &user_id[..8])],
    )
    .await
    .unwrap();

    let emp_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, 'FULL001', 'Full Column Employee', ?2, 'active', 1, unixepoch(), unixepoch())",
        libsql::params![emp_id.clone(), dept_id],
    )
    .await
    .unwrap();

    let enrollment_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO face_enrollments (id, employee_id, captured_via, photo_path, created_by, created_at) \
         VALUES (?1, ?2, 'upload', 'x.jpg', ?3, unixepoch())",
        libsql::params![enrollment_id.clone(), emp_id.clone(), user_id],
    )
    .await
    .unwrap();

    // One UPDATE touching every previously-uncovered column at once.
    conn.execute(
        "UPDATE employees SET \
            position = 'Manager', \
            hire_date = 1700000000, \
            face_id = 'face-full-1', \
            current_face_enrollment_id = ?1, \
            salary_kind = 'monthly', \
            terminated_on = 1800000000, \
            deleted_at = 1800000000, \
            status = 'inactive', \
            version = version + 1 \
         WHERE id = ?2 AND version = 1",
        libsql::params![enrollment_id.clone(), emp_id.clone()],
    )
    .await
    .unwrap();

    let (old, new) = latest_audit_row(&db, "employees", &emp_id, "UPDATE").await;

    // old_data: pre-update state, one uniform "nothing set yet" baseline.
    assert_eq!(old["position"], "");
    assert!(old["hire_date"].is_null());
    assert!(old["face_id"].is_null());
    assert!(old["current_face_enrollment_id"].is_null());
    assert!(old["salary_kind"].is_null());
    assert!(old["terminated_on"].is_null());
    assert!(old["deleted_at"].is_null());
    assert_eq!(old["status"], "active");
    assert_eq!(old["version"], 1);
    assert!(old["created_at"].is_number());
    assert!(old["updated_at"].is_number());

    // new_data: every previously-dropped column reflects the update.
    assert_eq!(new["position"], "Manager");
    assert_eq!(new["hire_date"], 1700000000);
    assert_eq!(new["face_id"], "face-full-1");
    assert_eq!(new["current_face_enrollment_id"], enrollment_id);
    assert_eq!(new["salary_kind"], "monthly");
    assert_eq!(new["terminated_on"], 1800000000);
    assert_eq!(new["deleted_at"], 1800000000);
    assert_eq!(new["status"], "inactive");
    assert_eq!(new["version"], 2);
    assert!(new["created_at"].is_number());
    assert!(new["updated_at"].is_number());
}

/// M-06 follow-up: `audit_employees_insert` carries the identical defect as
/// `audit_employees_update` did — 018_employees_base_salary.sql recreated it
/// alongside `update` with the same trimmed-down column set, and it was
/// never given `position`/`hire_date`/`face_id`/`current_face_enrollment_id`/
/// `salary_kind`/`terminated_on`/`deleted_at`/`created_at`/`updated_at` at
/// any point. Creating an employee through the real HTTP API must produce an
/// INSERT audit row whose `new_data` carries every column employees has
/// today — an incomplete row here is exactly as misleading as an incomplete
/// UPDATE row would have been.
#[tokio::test]
async fn creating_an_employee_is_audited_with_every_column() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let (state, _tmp) = make_state(db);
    let db_for_audit = state.db.clone();
    let app = build_test_app(state);

    let dept_id = create_test_department(&app, &token, "Onboarding Dept").await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/employees")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "employee_code": "INS001",
                "name": "Insert Employee",
                "department_id": dept_id,
                "position": "Clerk",
                "hire_date": "2024-01-15",
                "base_salary_cents": 5000,
                "salary_kind": "daily"
            })
            .to_string(),
        ))
        .unwrap();
    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let created = body_to_json(create_resp.into_body()).await;
    let emp_id = created["id"].as_str().unwrap().to_string();

    let (old, new) = latest_audit_row(&db_for_audit, "employees", &emp_id, "INSERT").await;
    assert!(
        old.is_null(),
        "INSERT audit row must have NULL old_data: {old:?}"
    );

    // Columns the create request actually set — must appear with real values.
    assert_eq!(new["id"], emp_id);
    assert_eq!(new["employee_code"], "INS001");
    assert_eq!(new["name"], "Insert Employee");
    assert_eq!(new["department_id"], dept_id);
    assert_eq!(new["status"], "active");
    assert_eq!(new["version"], 1);
    assert_eq!(new["position"], "Clerk");
    assert!(new["hire_date"].is_number(), "missing hire_date: {new:?}");
    assert_eq!(new["base_salary_cents"], 5000);
    assert_eq!(new["salary_kind"], "daily");
    assert!(new["created_at"].is_number(), "missing created_at: {new:?}");
    assert!(new["updated_at"].is_number(), "missing updated_at: {new:?}");

    // Columns nothing sets at creation time — must still be present as keys
    // (null), proving the trigger snapshots them rather than omitting them.
    assert!(
        new.get("deleted_at").is_some() && new["deleted_at"].is_null(),
        "deleted_at key missing from INSERT audit row: {new:?}"
    );
    assert!(
        new.get("face_id").is_some() && new["face_id"].is_null(),
        "face_id key missing from INSERT audit row: {new:?}"
    );
    assert!(
        new.get("current_face_enrollment_id").is_some()
            && new["current_face_enrollment_id"].is_null(),
        "current_face_enrollment_id key missing from INSERT audit row: {new:?}"
    );
    assert!(
        new.get("terminated_on").is_some() && new["terminated_on"].is_null(),
        "terminated_on key missing from INSERT audit row: {new:?}"
    );
}

/// M-06 follow-up: `audit_employees_delete` carries the identical defect.
/// Exercised directly against the DB with a real (hard) `DELETE FROM
/// employees` — the HTTP API only ever soft-deletes (see
/// `employee_tests::soft_delete_only_no_hard_delete`), so this is the only
/// way to fire `audit_employees_delete` at all; it is still the same trigger
/// the running system relies on (e.g. the purge worker's cleanup paths).
/// `current_face_enrollment_id` is deliberately left NULL here: setting it
/// would require a `face_enrollments` row whose own FK back to `employees`
/// would then block the DELETE this test needs to perform.
#[tokio::test]
async fn deleting_an_employee_is_audited_with_every_column() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();

    let dept_id = common::create_test_department_with_shift(
        &db,
        "Offboarding Dept",
        "day",
        false,
        480,
        "08:00",
        "17:00",
    )
    .await;

    let emp_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees ( \
            id, employee_code, name, department_id, status, deleted_at, version, \
            created_at, updated_at, position, hire_date, face_id, \
            base_salary_cents, salary_kind, terminated_on \
         ) VALUES ( \
            ?1, 'DEL001', 'Delete Employee', ?2, 'inactive', 1800000000, 3, \
            unixepoch(), unixepoch(), 'Manager', 1700000000, 'face-del-1', \
            5000, 'monthly', 1800000000 \
         )",
        libsql::params![emp_id.clone(), dept_id.clone()],
    )
    .await
    .unwrap();

    conn.execute(
        "DELETE FROM employees WHERE id = ?1",
        libsql::params![emp_id.clone()],
    )
    .await
    .unwrap();

    let (old, new) = latest_audit_row(&db, "employees", &emp_id, "DELETE").await;
    assert!(
        new.is_null(),
        "DELETE audit row must have NULL new_data: {new:?}"
    );

    assert_eq!(old["id"], emp_id);
    assert_eq!(old["employee_code"], "DEL001");
    assert_eq!(old["name"], "Delete Employee");
    assert_eq!(old["department_id"], dept_id);
    assert_eq!(old["status"], "inactive");
    assert_eq!(old["deleted_at"], 1800000000);
    assert_eq!(old["version"], 3);
    assert!(old["created_at"].is_number(), "missing created_at: {old:?}");
    assert!(old["updated_at"].is_number(), "missing updated_at: {old:?}");
    assert_eq!(old["position"], "Manager");
    assert_eq!(old["hire_date"], 1700000000);
    assert_eq!(old["face_id"], "face-del-1");
    assert!(
        old.get("current_face_enrollment_id").is_some()
            && old["current_face_enrollment_id"].is_null(),
        "current_face_enrollment_id key missing from DELETE audit row: {old:?}"
    );
    assert_eq!(old["base_salary_cents"], 5000);
    assert_eq!(old["salary_kind"], "monthly");
    assert_eq!(old["terminated_on"], 1800000000);
}
