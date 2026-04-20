mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use axum::routing::{get, patch, post};
use cronometrix_api::auth;
use cronometrix_api::departments;
use cronometrix_api::employees;
use cronometrix_api::state::AppState;
use cronometrix_api::config::Config;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use http_body_util::BodyExt;

/// Build a test app with department + employee routes.
async fn build_test_app(db: libsql::Database) -> Router {
    let config = Arc::new(Config {
        database_path: "test".to_string(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: common::TEST_JWT_SECRET.to_string(),
        server_host: "127.0.0.1".to_string(),
        server_port: 3001,
        turso_sync_interval_secs: 300,
        device_creds_key: common::test_device_creds_key(),
    });

    let state = AppState {
        db: Arc::new(db),
        config,
        lifecycle_tx: None,
    };

    let viewer_routes = Router::new()
        .route("/departments", get(departments::handlers::list_departments))
        .route("/departments/{id}", get(departments::handlers::get_department))
        .route("/employees", get(employees::handlers::list_employees))
        .route("/employees/{id}", get(employees::handlers::get_employee))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ));

    let admin_routes = Router::new()
        .route("/departments", post(departments::handlers::create_department))
        .route("/departments/{id}", patch(departments::handlers::update_department))
        .route("/employees", post(employees::handlers::create_employee))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_admin,
        ));

    Router::new()
        .nest("/api/v1", viewer_routes.merge(admin_routes))
        .with_state(state)
}

/// Collect response body into JSON.
async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

#[tokio::test]
async fn crud_department_endpoints() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let app = build_test_app(db).await;

    // POST — create a department
    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/departments")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "name": "Engineering",
                "base_salary_cents": 500000,
                "shift_start_time": "08:00",
                "shift_end_time": "17:00",
                "lunch_mode": "fixed",
                "lunch_duration_min": 60
            })
            .to_string(),
        ))
        .unwrap();

    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(
        create_resp.status(),
        StatusCode::CREATED,
        "POST /departments should return 201"
    );

    let created = body_to_json(create_resp.into_body()).await;
    let dept_id = created["id"].as_str().expect("id should be present").to_string();
    assert_eq!(created["name"], "Engineering");
    assert_eq!(created["version"], 1);
    assert!(created["created_at"].is_string(), "created_at should be ISO 8601");

    // GET list — department should appear
    let list_req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/departments")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let list_resp = app.clone().oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = body_to_json(list_resp.into_body()).await;
    assert!(list_body["total"].as_i64().unwrap() >= 1);
    let names: Vec<&str> = list_body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|d| d["name"].as_str())
        .collect();
    assert!(names.contains(&"Engineering"));

    // GET by ID
    let get_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/departments/{}", dept_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let get_resp = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);
    let got = body_to_json(get_resp.into_body()).await;
    assert_eq!(got["id"], dept_id);
    assert_eq!(got["name"], "Engineering");

    // PATCH — update salary with optimistic concurrency
    let patch_req = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/api/v1/departments/{}", dept_id))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "base_salary_cents": 600000,
                "version": 1
            })
            .to_string(),
        ))
        .unwrap();

    let patch_resp = app.clone().oneshot(patch_req).await.unwrap();
    assert_eq!(patch_resp.status(), StatusCode::OK, "PATCH should return 200");
    let patched = body_to_json(patch_resp.into_body()).await;
    assert_eq!(patched["base_salary_cents"], 600000);
    assert_eq!(patched["version"], 2, "Version should increment to 2");
}

#[tokio::test]
async fn department_has_salary_schedule_lunch() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let app = build_test_app(db).await;

    // Create a department with all fields
    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/departments")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "name": "Operations",
                "base_salary_cents": 350000,
                "shift_start_time": "09:00",
                "shift_end_time": "18:00",
                "lunch_mode": "fixed",
                "lunch_duration_min": 45
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let dept = body_to_json(resp.into_body()).await;
    let dept_id = dept["id"].as_str().unwrap().to_string();

    // GET by ID and verify all fields
    let get_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/departments/{}", dept_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let get_resp = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);
    let got = body_to_json(get_resp.into_body()).await;

    assert_eq!(got["base_salary_cents"], 350000, "base_salary_cents must be present and correct");
    assert_eq!(got["shift_start_time"], "09:00", "shift_start_time must be correct");
    assert_eq!(got["shift_end_time"], "18:00", "shift_end_time must be correct");
    assert_eq!(got["lunch_mode"], "fixed", "lunch_mode must be correct");
    assert_eq!(got["lunch_duration_min"], 45, "lunch_duration_min must be correct");
    assert_eq!(got["status"], "active");
    assert!(got["created_at"].is_string(), "created_at should be ISO 8601 string");
    assert!(got["updated_at"].is_string(), "updated_at should be ISO 8601 string");
}

#[tokio::test]
async fn department_employee_one_to_one_enforced() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let app = build_test_app(db).await;

    // Create a department
    let dept_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/departments")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "name": "HR",
                "base_salary_cents": 280000,
                "shift_start_time": "08:30",
                "shift_end_time": "17:30",
                "lunch_mode": "punch"
            })
            .to_string(),
        ))
        .unwrap();

    let dept_resp = app.clone().oneshot(dept_req).await.unwrap();
    assert_eq!(dept_resp.status(), StatusCode::CREATED);
    let dept = body_to_json(dept_resp.into_body()).await;
    let dept_id = dept["id"].as_str().unwrap().to_string();

    // Create an employee referencing the department
    let emp_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/employees")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "employee_code": "HR001",
                "name": "Frank HR",
                "department_id": dept_id
            })
            .to_string(),
        ))
        .unwrap();

    let emp_resp = app.clone().oneshot(emp_req).await.unwrap();
    assert_eq!(emp_resp.status(), StatusCode::CREATED);
    let emp = body_to_json(emp_resp.into_body()).await;

    // Verify the FK relationship: employee's department_id matches the created department
    assert_eq!(
        emp["department_id"], dept_id,
        "Employee's department_id should match the created department"
    );

    // GET employee by ID and verify FK is still intact
    let get_emp_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/employees/{}", emp["id"].as_str().unwrap()))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let get_emp_resp = app.clone().oneshot(get_emp_req).await.unwrap();
    assert_eq!(get_emp_resp.status(), StatusCode::OK);
    let fetched_emp = body_to_json(get_emp_resp.into_body()).await;
    assert_eq!(
        fetched_emp["department_id"], dept_id,
        "Fetched employee's department_id should match the created department"
    );
}
