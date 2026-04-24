mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use axum::routing::{delete, get, patch, post};
use cronometrix_api::auth;
use cronometrix_api::departments;
use cronometrix_api::employees;
use cronometrix_api::state::AppState;
use cronometrix_api::config::Config;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use http_body_util::BodyExt;

/// Build a test app with employee + department routes (department needed for FK constraint tests).
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
        timezone: "America/Caracas".parse().unwrap(),
    });

    let state = AppState {
        db: Arc::new(db),
        config,
        lifecycle_tx: None,
        recompute_tx: None,
        event_broadcast: None,
    };

    // Read-only routes for any authenticated user
    let viewer_routes = Router::new()
        .route("/employees", get(employees::handlers::list_employees))
        .route("/employees/{id}", get(employees::handlers::get_employee))
        .route("/departments", get(departments::handlers::list_departments))
        .route("/departments/{id}", get(departments::handlers::get_department))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ));

    // Supervisor+ routes
    let supervisor_routes = Router::new()
        .route("/employees", post(employees::handlers::create_employee))
        .route("/employees/{id}", patch(employees::handlers::update_employee))
        .route("/departments", post(departments::handlers::create_department))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_supervisor_or_above,
        ));

    // Admin-only routes
    let admin_routes = Router::new()
        .route("/employees/{id}", delete(employees::handlers::deactivate_employee))
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

/// Collect response body into JSON.
async fn body_to_json(body: Body) -> Value {
    let bytes = body.collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(json!(null))
}

/// Helper: create a department via the API and return its ID.
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

#[tokio::test]
async fn crud_employee_endpoints() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let app = build_test_app(db).await;

    // Create a department first (FK requirement)
    let dept_id = create_test_department(&app, &token, "Engineering").await;

    // POST — create an employee
    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/employees")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "employee_code": "EMP001",
                "name": "Alice Smith",
                "department_id": dept_id
            })
            .to_string(),
        ))
        .unwrap();

    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(
        create_resp.status(),
        StatusCode::CREATED,
        "POST /employees should return 201"
    );

    let created = body_to_json(create_resp.into_body()).await;
    let emp_id = created["id"].as_str().expect("id should be present").to_string();
    assert_eq!(created["employee_code"], "EMP001");
    assert_eq!(created["name"], "Alice Smith");
    assert_eq!(created["department_id"], dept_id);
    assert_eq!(created["status"], "active");
    assert_eq!(created["version"], 1);
    assert!(created["created_at"].is_string(), "created_at should be ISO 8601 string");
    assert!(created["updated_at"].is_string(), "updated_at should be ISO 8601 string");

    // GET list — employee should appear
    let list_req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/employees")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let list_resp = app.clone().oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = body_to_json(list_resp.into_body()).await;
    assert!(list_body["total"].as_i64().unwrap() >= 1, "Total should be >= 1");
    let names: Vec<&str> = list_body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["name"].as_str())
        .collect();
    assert!(names.contains(&"Alice Smith"), "Alice Smith should appear in list");

    // GET by ID
    let get_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/employees/{}", emp_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let get_resp = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(get_resp.status(), StatusCode::OK);
    let got = body_to_json(get_resp.into_body()).await;
    assert_eq!(got["id"], emp_id);
    assert_eq!(got["name"], "Alice Smith");

    // PATCH — update name
    let patch_req = Request::builder()
        .method(Method::PATCH)
        .uri(format!("/api/v1/employees/{}", emp_id))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "name": "Alice Johnson",
                "version": 1
            })
            .to_string(),
        ))
        .unwrap();

    let patch_resp = app.clone().oneshot(patch_req).await.unwrap();
    assert_eq!(patch_resp.status(), StatusCode::OK, "PATCH should return 200");
    let patched = body_to_json(patch_resp.into_body()).await;
    assert_eq!(patched["name"], "Alice Johnson", "Name should be updated");
    assert_eq!(patched["version"], 2, "Version should increment to 2");
}

#[tokio::test]
async fn soft_delete_only_no_hard_delete() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let app = build_test_app(db).await;

    // Create department + employee
    let dept_id = create_test_department(&app, &token, "SoftDelete Dept").await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/employees")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "employee_code": "SD001",
                "name": "Bob Delete",
                "department_id": dept_id
            })
            .to_string(),
        ))
        .unwrap();

    let create_resp = app.clone().oneshot(create_req).await.unwrap();
    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let emp = body_to_json(create_resp.into_body()).await;
    let emp_id = emp["id"].as_str().unwrap().to_string();

    // DELETE (soft delete) — expect 204
    let delete_req = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/api/v1/employees/{}", emp_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let delete_resp = app.clone().oneshot(delete_req).await.unwrap();
    assert_eq!(
        delete_resp.status(),
        StatusCode::NO_CONTENT,
        "DELETE should return 204"
    );

    // Verify soft delete via API: GET by id still returns the employee (row not deleted)
    let get_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/employees/{}", emp_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let get_resp = app.clone().oneshot(get_req).await.unwrap();
    assert_eq!(
        get_resp.status(),
        StatusCode::OK,
        "GET by id should still find the employee after soft delete (row not removed)"
    );
    let fetched = body_to_json(get_resp.into_body()).await;
    assert_eq!(fetched["status"], "inactive", "Status should be inactive");
    assert!(
        fetched["deleted_at"].is_string(),
        "deleted_at should be set as ISO 8601 string after soft delete"
    );

    // Verify it does NOT appear in the default active listing
    let list_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/employees?status=active"))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let list_resp = app.clone().oneshot(list_req).await.unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_body = body_to_json(list_resp.into_body()).await;
    let ids: Vec<&str> = list_body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["id"].as_str())
        .collect();
    assert!(
        !ids.contains(&emp_id.as_str()),
        "Soft-deleted employee should not appear in active listing"
    );

    // Verify it DOES appear when filtering by status=inactive
    let inactive_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/employees?status=inactive"))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let inactive_resp = app.clone().oneshot(inactive_req).await.unwrap();
    assert_eq!(inactive_resp.status(), StatusCode::OK);
    let inactive_body = body_to_json(inactive_resp.into_body()).await;
    let inactive_ids: Vec<&str> = inactive_body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["id"].as_str())
        .collect();
    assert!(
        inactive_ids.contains(&emp_id.as_str()),
        "Soft-deleted employee should appear in inactive listing"
    );
}

#[tokio::test]
async fn employee_search_and_filter() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let app = build_test_app(db).await;

    // Create two departments
    let dept_a = create_test_department(&app, &token, "Alpha Dept").await;
    let dept_b = create_test_department(&app, &token, "Beta Dept").await;

    // Create employees in different departments
    for (code, name, dept) in [
        ("F001", "Carlos Alpha", dept_a.as_str()),
        ("F002", "Diana Alpha", dept_a.as_str()),
        ("F003", "Eve Beta", dept_b.as_str()),
    ] {
        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/employees")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::from(
                json!({
                    "employee_code": code,
                    "name": name,
                    "department_id": dept
                })
                .to_string(),
            ))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // Filter by name (partial match)
    let name_req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/employees?name=Alpha")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let name_resp = app.clone().oneshot(name_req).await.unwrap();
    assert_eq!(name_resp.status(), StatusCode::OK);
    let name_body = body_to_json(name_resp.into_body()).await;
    assert_eq!(
        name_body["total"], 2,
        "Should find 2 employees with 'Alpha' in name"
    );

    // Filter by department_id
    let dept_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/employees?department_id={}", dept_b))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let dept_resp = app.clone().oneshot(dept_req).await.unwrap();
    assert_eq!(dept_resp.status(), StatusCode::OK);
    let dept_body = body_to_json(dept_resp.into_body()).await;
    assert_eq!(
        dept_body["total"], 1,
        "Should find 1 employee in dept_b"
    );
    assert_eq!(dept_body["data"][0]["name"], "Eve Beta");

    // Filter by status=active (default) — all 3 should appear
    let active_req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/employees?status=active")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    let active_resp = app.clone().oneshot(active_req).await.unwrap();
    assert_eq!(active_resp.status(), StatusCode::OK);
    let active_body = body_to_json(active_resp.into_body()).await;
    assert_eq!(
        active_body["total"], 3,
        "Should find 3 active employees"
    );
}

#[tokio::test]
async fn employee_department_constraint() {
    let db = common::test_db().await;
    let admin_id = uuid::Uuid::new_v4().to_string();
    let token = common::test_access_token(&admin_id, "admin");
    let app = build_test_app(db).await;

    // Attempt to create an employee with a non-existent department_id
    let fake_dept_id = uuid::Uuid::new_v4().to_string();
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/employees")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(
            json!({
                "employee_code": "BAD001",
                "name": "Ghost Employee",
                "department_id": fake_dept_id
            })
            .to_string(),
        ))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "Creating employee with non-existent department_id should return 404"
    );

    let body = body_to_json(resp.into_body()).await;
    assert_eq!(
        body["error"]["code"], "DEPARTMENT_NOT_FOUND",
        "Error code should be DEPARTMENT_NOT_FOUND, got: {:?}",
        body
    );
}
