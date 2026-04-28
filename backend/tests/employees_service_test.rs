//! Service-layer tests for `employees::service`. Targets the 61.29% baseline
//! gap from Plan 03 (08-04A bucket row 11). The existing `employee_tests.rs`
//! covers handler-layer happy paths; this file targets:
//!   - hire_date parsing edge cases (empty, malformed, valid YYYY-MM-DD)
//!   - department-active check error branches
//!   - Conflict on duplicate employee_code
//!   - update() empty-PATCH no-op return
//!   - update() VERSION_CONFLICT vs NOT_FOUND distinction
//!   - update() department change resolves department-not-found
//!   - update() hire_date clear (empty string) and set
//!   - deactivate() 404 on already-inactive

mod common;

use cronometrix_api::departments::models::CreateDepartmentRequest;
use cronometrix_api::departments::service as dept_service;
use cronometrix_api::employees::models::{
    CreateEmployeeRequest, EmployeeListQuery, UpdateEmployeeRequest,
};
use cronometrix_api::employees::service as emp_service;

async fn create_active_dept(conn: &libsql::Connection, name: &str) -> String {
    dept_service::create(
        conn,
        CreateDepartmentRequest {
            name: name.into(),
            base_salary_cents: 0,
            shift_start_time: "09:00".into(),
            shift_end_time: "17:00".into(),
            lunch_mode: "punch".into(),
            lunch_duration_min: None,
        },
    )
    .await
    .unwrap()
    .id
}

#[tokio::test]
async fn create_employee_happy_path_no_optional_fields() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let dept_id = create_active_dept(&conn, "DeptA").await;
    let emp = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "E001".into(),
            name: "Alice".into(),
            department_id: dept_id.clone(),
            position: None,
            hire_date: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(emp.employee_code, "E001");
    assert_eq!(emp.position, "");
    assert!(emp.hire_date.is_none());
}

#[tokio::test]
async fn create_employee_with_hire_date_yyyy_mm_dd() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let dept_id = create_active_dept(&conn, "DeptB").await;
    let emp = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "E002".into(),
            name: "Bob".into(),
            department_id: dept_id,
            position: Some("Engineer".into()),
            hire_date: Some("2024-01-15".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(emp.position, "Engineer");
    assert_eq!(emp.hire_date.as_deref(), Some("2024-01-15"));
}

#[tokio::test]
async fn create_employee_rejects_malformed_hire_date() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let dept_id = create_active_dept(&conn, "DeptC").await;
    let err = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "E003".into(),
            name: "Carol".into(),
            department_id: dept_id,
            position: None,
            hire_date: Some("15-01-2024".into()), // wrong format
        },
    )
    .await
    .expect_err("malformed hire_date must reject");
    let s = err.to_string();
    assert!(s.contains("validation") || s.contains("hire_date"), "err: {s}");
}

#[tokio::test]
async fn create_employee_empty_hire_date_treated_as_null() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let dept_id = create_active_dept(&conn, "DeptD").await;
    let emp = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "E004".into(),
            name: "Dave".into(),
            department_id: dept_id,
            position: None,
            hire_date: Some("".into()),
        },
    )
    .await
    .unwrap();
    assert!(emp.hire_date.is_none());
}

#[tokio::test]
async fn create_employee_404_when_department_unknown() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let err = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "E_OOPS".into(),
            name: "Nobody".into(),
            department_id: "no-such-dept".into(),
            position: None,
            hire_date: None,
        },
    )
    .await
    .expect_err("missing dept must 404");
    let s = err.to_string();
    assert!(s.contains("not found"), "err: {s}");
}

#[tokio::test]
async fn create_employee_duplicate_employee_code_conflicts() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let dept_id = create_active_dept(&conn, "DeptDup").await;
    let _first = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "DUP_CODE".into(),
            name: "First".into(),
            department_id: dept_id.clone(),
            position: None,
            hire_date: None,
        },
    )
    .await
    .unwrap();

    let err = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "DUP_CODE".into(),
            name: "Second".into(),
            department_id: dept_id,
            position: None,
            hire_date: None,
        },
    )
    .await
    .expect_err("dup code must conflict");
    let s = err.to_string();
    assert!(
        s.contains("conflict") || s.contains("EXISTS") || s.contains("already"),
        "err: {s}"
    );
}

#[tokio::test]
async fn list_filters_by_name_partial_match() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let dept_id = create_active_dept(&conn, "DeptList").await;
    let _ = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "LA1".into(),
            name: "Alice Wonder".into(),
            department_id: dept_id.clone(),
            position: None,
            hire_date: None,
        },
    )
    .await
    .unwrap();
    let _ = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "LB1".into(),
            name: "Bob Builder".into(),
            department_id: dept_id,
            position: None,
            hire_date: None,
        },
    )
    .await
    .unwrap();

    let result = emp_service::list(
        &conn,
        EmployeeListQuery {
            limit: None,
            offset: None,
            name: Some("Wonder".into()),
            department_id: None,
            status: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.data[0].name, "Alice Wonder");
}

#[tokio::test]
async fn list_filters_by_department() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let dept_a = create_active_dept(&conn, "DeptListA").await;
    let dept_b = create_active_dept(&conn, "DeptListB").await;
    let _ = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "LDA".into(),
            name: "InA".into(),
            department_id: dept_a.clone(),
            position: None,
            hire_date: None,
        },
    )
    .await
    .unwrap();
    let _ = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "LDB".into(),
            name: "InB".into(),
            department_id: dept_b,
            position: None,
            hire_date: None,
        },
    )
    .await
    .unwrap();

    let result = emp_service::list(
        &conn,
        EmployeeListQuery {
            limit: None,
            offset: None,
            name: None,
            department_id: Some(dept_a.clone()),
            status: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.total, 1);
    assert_eq!(result.data[0].department_id, dept_a);
}

#[tokio::test]
async fn list_pagination_clamps() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let result = emp_service::list(
        &conn,
        EmployeeListQuery {
            limit: Some(999),
            offset: Some(-3),
            name: None,
            department_id: None,
            status: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(result.limit, 100);
    assert_eq!(result.offset, 0);
}

#[tokio::test]
async fn get_by_id_404_unknown() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let err = emp_service::get_by_id(&conn, "no-such-emp")
        .await
        .expect_err("must 404");
    let s = err.to_string();
    assert!(s.contains("not found"));
}

#[tokio::test]
async fn update_no_fields_returns_current_state_no_version_bump() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let dept_id = create_active_dept(&conn, "UpdNoOp").await;
    let emp = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "UN1".into(),
            name: "Una".into(),
            department_id: dept_id,
            position: None,
            hire_date: None,
        },
    )
    .await
    .unwrap();
    let updated = emp_service::update(
        &conn,
        &emp.id,
        UpdateEmployeeRequest {
            name: None,
            department_id: None,
            position: None,
            hire_date: None,
            version: emp.version,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.id, emp.id);
    assert_eq!(updated.version, emp.version);
}

#[tokio::test]
async fn update_404_unknown_id() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let err = emp_service::update(
        &conn,
        "no-such-emp",
        UpdateEmployeeRequest {
            name: Some("X".into()),
            department_id: None,
            position: None,
            hire_date: None,
            version: 1,
        },
    )
    .await
    .expect_err("must 404");
    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
async fn update_409_on_stale_version() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let dept_id = create_active_dept(&conn, "UpdVer").await;
    let emp = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "UV1".into(),
            name: "VerEmp".into(),
            department_id: dept_id,
            position: None,
            hire_date: None,
        },
    )
    .await
    .unwrap();
    let _ = emp_service::update(
        &conn,
        &emp.id,
        UpdateEmployeeRequest {
            name: Some("Once".into()),
            department_id: None,
            position: None,
            hire_date: None,
            version: emp.version,
        },
    )
    .await
    .unwrap();

    let err = emp_service::update(
        &conn,
        &emp.id,
        UpdateEmployeeRequest {
            name: Some("Twice".into()),
            department_id: None,
            position: None,
            hire_date: None,
            version: emp.version, // stale
        },
    )
    .await
    .expect_err("stale version must 409");
    assert!(err.to_string().contains("conflict") || err.to_string().contains("modified"));
}

#[tokio::test]
async fn update_404_when_changing_to_unknown_department() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let dept_id = create_active_dept(&conn, "UpdDept").await;
    let emp = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "UD1".into(),
            name: "DeptEmp".into(),
            department_id: dept_id,
            position: None,
            hire_date: None,
        },
    )
    .await
    .unwrap();
    let err = emp_service::update(
        &conn,
        &emp.id,
        UpdateEmployeeRequest {
            name: None,
            department_id: Some("no-such".into()),
            position: None,
            hire_date: None,
            version: emp.version,
        },
    )
    .await
    .expect_err("changing to unknown dept must 404");
    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
async fn update_can_clear_hire_date_via_empty_string() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let dept_id = create_active_dept(&conn, "UpdHire").await;
    let emp = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "UH1".into(),
            name: "HireEmp".into(),
            department_id: dept_id,
            position: None,
            hire_date: Some("2024-06-01".into()),
        },
    )
    .await
    .unwrap();
    assert!(emp.hire_date.is_some());
    let cleared = emp_service::update(
        &conn,
        &emp.id,
        UpdateEmployeeRequest {
            name: None,
            department_id: None,
            position: None,
            hire_date: Some("".into()),
            version: emp.version,
        },
    )
    .await
    .unwrap();
    assert!(cleared.hire_date.is_none(), "empty string clears hire_date");
}

#[tokio::test]
async fn update_rejects_malformed_hire_date() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let dept_id = create_active_dept(&conn, "UpdHireBad").await;
    let emp = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "UHB1".into(),
            name: "BadHire".into(),
            department_id: dept_id,
            position: None,
            hire_date: None,
        },
    )
    .await
    .unwrap();
    let err = emp_service::update(
        &conn,
        &emp.id,
        UpdateEmployeeRequest {
            name: None,
            department_id: None,
            position: None,
            hire_date: Some("not-a-date".into()),
            version: emp.version,
        },
    )
    .await
    .expect_err("bad hire_date must reject");
    assert!(err.to_string().contains("validation") || err.to_string().contains("hire_date"));
}

#[tokio::test]
async fn deactivate_marks_inactive_and_subsequent_call_404s() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let dept_id = create_active_dept(&conn, "Deactivate").await;
    let emp = emp_service::create(
        &conn,
        CreateEmployeeRequest {
            employee_code: "DACT".into(),
            name: "ToDeactivate".into(),
            department_id: dept_id,
            position: None,
            hire_date: None,
        },
    )
    .await
    .unwrap();
    emp_service::deactivate(&conn, &emp.id).await.unwrap();

    let after = emp_service::get_by_id(&conn, &emp.id).await.unwrap();
    assert_eq!(after.status, "inactive");
    assert!(after.deleted_at.is_some());

    let err = emp_service::deactivate(&conn, &emp.id)
        .await
        .expect_err("second deactivate must 404");
    assert!(err.to_string().contains("not found") || err.to_string().contains("already"));
}

#[tokio::test]
async fn deactivate_404_unknown_id() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let err = emp_service::deactivate(&conn, "no-such-emp")
        .await
        .expect_err("must 404");
    assert!(err.to_string().contains("not found"));
}
