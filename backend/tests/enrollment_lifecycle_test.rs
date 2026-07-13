//! Phase 7 — Enrollment lifecycle integration tests (Wave 0 scaffolds).
//!
//! Covers: re-enrollment (D-14), employee deactivation → purge (D-15),
//! PurgeWorker Pitfall-10 guard, new device → backfill (D-16), audit triggers (D-17).
//! Populated in Tasks 2, 5, and 6.

mod common;

use cronometrix_api::enrollments::models::EnrollmentListQuery;
use cronometrix_api::enrollments::service;

// ---------------------------------------------------------------------------
// Task 2 — audit trigger test (populated after migrations 016/017 land)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_audit_log_rows_written_for_enrollments_face_enrollments_device_face_mappings() {
    // Validates D-17: audit triggers fire on INSERT/UPDATE/DELETE for
    // enrollments, face_enrollments, and device_face_mappings.
    let db = common::test_db().await;
    let conn = db.connect().expect("connect");

    // -----------------------------------------------------------------------
    // Seed prerequisite rows (FK chain: users -> departments -> employees -> devices)
    // -----------------------------------------------------------------------
    let user_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Trigger Test Admin', 'hash', 'admin', 'active', 1, unixepoch(), unixepoch())",
        libsql::params![user_id.clone(), format!("trigtest-{}", &user_id[..8])],
    ).await.expect("seed user");

    let dept_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 0, '08:00', '17:00', 'fixed', 60, 'active', 1, unixepoch(), unixepoch())",
        libsql::params![dept_id.clone(), format!("Dept-{}", &dept_id[..8])],
    )
    .await
    .expect("seed department");

    let emp_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Trigger Test Employee', ?3, 'active', 1, unixepoch(), unixepoch())",
        libsql::params![emp_id.clone(), format!("EMP-{}", &emp_id[..8]), dept_id.clone()],
    ).await.expect("seed employee");

    let dev_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO devices (id, name, ip, port, scheme, username, encrypted_password, direction, \
         allow_insecure_tls, connection_state, status, version, created_at, updated_at) \
         VALUES (?1, 'Test Device', '192.168.1.99', 443, 'https', 'admin', 'enc', 'entry', 0, 'offline', 'active', 1, unixepoch(), unixepoch())",
        libsql::params![dev_id.clone()],
    ).await.expect("seed device");

    // -----------------------------------------------------------------------
    // 1. INSERT into face_enrollments -> triggers audit_face_enrollments_insert
    // -----------------------------------------------------------------------
    let fe_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO face_enrollments (id, employee_id, captured_via, source_device_id, photo_path, face_quality_score, created_by, created_at) \
         VALUES (?1, ?2, 'upload', NULL, ?3, NULL, ?4, unixepoch())",
        libsql::params![fe_id.clone(), emp_id.clone(), format!("{}/{}.jpg", emp_id, fe_id), user_id.clone()],
    ).await.expect("insert face_enrollment");

    // 2. INSERT into enrollments -> triggers audit_enrollments_insert
    let enr_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO enrollments (id, employee_id, face_enrollment_id, status, started_by, started_at, version) \
         VALUES (?1, ?2, ?3, 'in_progress', ?4, unixepoch(), 1)",
        libsql::params![enr_id.clone(), emp_id.clone(), fe_id.clone(), user_id.clone()],
    ).await.expect("insert enrollment");

    // 3. INSERT into device_face_mappings -> triggers audit_device_face_mappings_insert
    let mapping_id = uuid::Uuid::new_v4().to_string();
    let face_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO device_face_mappings (id, device_id, face_id, employee_id, state, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'active', 1, unixepoch(), unixepoch())",
        libsql::params![mapping_id.clone(), dev_id.clone(), face_id.clone(), emp_id.clone()],
    ).await.expect("insert device_face_mapping");

    // Assert: 3 audit rows (one INSERT per table)
    let mut rows = conn.query(
        "SELECT count(*) FROM audit_log WHERE table_name IN ('enrollments','face_enrollments','device_face_mappings')",
        (),
    ).await.expect("count audit_log");
    let row = rows.next().await.expect("next row").expect("has row");
    let count: i64 = row.get(0).expect("count col");
    assert_eq!(
        count, 3,
        "expected 3 audit rows after 3 INSERTs, got {count}"
    );

    // -----------------------------------------------------------------------
    // UPDATE each row -> +3 more audit rows
    // -----------------------------------------------------------------------
    conn.execute(
        "UPDATE enrollments SET status='success', version=version+1 WHERE id=?1",
        libsql::params![enr_id.clone()],
    )
    .await
    .expect("update enrollment");

    conn.execute(
        "UPDATE face_enrollments SET face_quality_score='{\"face_detected\":true}' WHERE id=?1",
        libsql::params![fe_id.clone()],
    )
    .await
    .expect("update face_enrollment");

    conn.execute(
        "UPDATE device_face_mappings SET state='pending_delete', version=version+1 WHERE id=?1",
        libsql::params![mapping_id.clone()],
    )
    .await
    .expect("update device_face_mapping");

    let mut rows = conn.query(
        "SELECT count(*) FROM audit_log WHERE table_name IN ('enrollments','face_enrollments','device_face_mappings')",
        (),
    ).await.expect("count audit_log after updates");
    let row = rows.next().await.expect("next row").expect("has row");
    let count: i64 = row.get(0).expect("count col");
    assert_eq!(
        count, 6,
        "expected 6 audit rows after 3 INSERTs + 3 UPDATEs, got {count}"
    );

    // -----------------------------------------------------------------------
    // DELETE each row -> +3 more audit rows (total 9)
    // DELETE in FK-safe order: enrollment_device_pushes has no rows, so
    // delete enrollments first (references face_enrollments), then face_enrollments,
    // then device_face_mappings.
    // -----------------------------------------------------------------------
    conn.execute(
        "DELETE FROM enrollments WHERE id=?1",
        libsql::params![enr_id.clone()],
    )
    .await
    .expect("delete enrollment");
    conn.execute(
        "DELETE FROM face_enrollments WHERE id=?1",
        libsql::params![fe_id.clone()],
    )
    .await
    .expect("delete face_enrollment");
    conn.execute(
        "DELETE FROM device_face_mappings WHERE id=?1",
        libsql::params![mapping_id.clone()],
    )
    .await
    .expect("delete device_face_mapping");

    let mut rows = conn.query(
        "SELECT count(*) FROM audit_log WHERE table_name IN ('enrollments','face_enrollments','device_face_mappings')",
        (),
    ).await.expect("count audit_log after deletes");
    let row = rows.next().await.expect("next row").expect("has row");
    let count: i64 = row.get(0).expect("count col");
    assert_eq!(
        count, 9,
        "expected 9 audit rows (3 INSERT + 3 UPDATE + 3 DELETE), got {count}"
    );
}

#[tokio::test]
async fn persisted_in_progress_enrollment_is_listable_from_fresh_connection() {
    let db = common::test_db().await;
    let user_id = common::create_test_admin(&db).await;
    let conn = db.connect().expect("initial connection");

    let dept_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, status, version, created_at, updated_at) \
         VALUES (?1, 'Lifecycle Department', 0, '08:00', '17:00', 'fixed', 60, \
                 'active', 1, unixepoch(), unixepoch())",
        libsql::params![dept_id.clone()],
    )
    .await
    .expect("seed department");

    let employee_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, 'LIFE-001', 'Persisted Employee', ?2, 'active', 1, unixepoch(), unixepoch())",
        libsql::params![employee_id.clone(), dept_id],
    )
    .await
    .expect("seed employee");

    let face_enrollment_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO face_enrollments \
         (id, employee_id, captured_via, source_device_id, photo_path, face_quality_score, created_by, created_at) \
         VALUES (?1, ?2, 'upload', NULL, 'persisted/photo.jpg', NULL, ?3, 1800000000)",
        libsql::params![
            face_enrollment_id.clone(),
            employee_id.clone(),
            user_id.clone()
        ],
    )
    .await
    .expect("seed face enrollment");

    let enrollment_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO enrollments \
         (id, employee_id, face_enrollment_id, status, started_by, started_at, completed_at, version) \
         VALUES (?1, ?2, ?3, 'in_progress', ?4, 1800000000, NULL, 1)",
        libsql::params![
            enrollment_id.clone(),
            employee_id.clone(),
            face_enrollment_id,
            user_id
        ],
    )
    .await
    .expect("persist enrollment");
    drop(conn);

    let fresh_conn = db.connect().expect("fresh connection");
    let page = service::list_enrollments(
        &fresh_conn,
        EnrollmentListQuery {
            status: Some("in_progress".into()),
            limit: None,
            offset: None,
        },
    )
    .await
    .expect("list persisted enrollment");

    assert_eq!(page.total, 1);
    assert_eq!(page.data.len(), 1);
    assert_eq!(page.data[0].id, enrollment_id);
    assert_eq!(page.data[0].employee_id, employee_id);
    assert_eq!(page.data[0].employee_name, "Persisted Employee");
    assert_eq!(page.data[0].employee_code, "LIFE-001");
    assert_eq!(page.data[0].status, "in_progress");
    assert!(page.data[0].device_pushes.is_empty());
}

// ---------------------------------------------------------------------------
// Task 5 — lifecycle tests (populated after pusher/workers land)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 4"]
async fn test_re_enrollment_keeps_face_id_constant() {
    todo!("implement after Task 4 start_enrollment + D-14 logic lands")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_employee_deactivation_publishes_purge_request() {
    todo!("implement after Task 5 purge_tx wired in deactivate_employee")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_purge_worker_calls_userinfodetail_delete_per_mapped_device() {
    todo!("implement after Task 5 PurgeWorker lands")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_purge_worker_aborts_if_employee_reactivated_mid_loop() {
    todo!("implement after Task 5 Pitfall-10 guard lands in PurgeWorker")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_new_device_registration_publishes_backfill_request() {
    todo!("implement after Task 5 backfill_tx wired in create_device")
}

#[tokio::test]
#[ignore = "wave-0 stub — populated in Task 5"]
async fn test_backfill_pushes_every_active_face_id_employee_to_new_device() {
    todo!("implement after Task 5 BackfillWorker lands")
}
