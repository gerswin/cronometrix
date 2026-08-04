//! H-10 (Bloque 3, Task 1): terminating an employee purges the enrolled face
//! template from disk, while attendance evidence (H-09) is preserved, and the
//! purge is recorded in the audit log.
//!
//! These exercise `enrollments::service::purge_enrolled_faces` directly — the
//! function the deactivation handler calls right after setting the row inactive.
//! The second test is the important one: it is what stops H-10 from being
//! "fixed" by deleting evidence H-09 requires kept.

mod common;

use std::sync::Arc;

use cronometrix_api::config::Config;
use cronometrix_api::enrollments::service as enrollment_service;
use libsql::params;

use common::{create_test_admin, test_device_creds_key, TEST_JWT_SECRET};

fn make_config() -> Arc<Config> {
    Arc::new(Config {
        database_path: "test.db".into(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: TEST_JWT_SECRET.to_string(),
        server_host: "127.0.0.1".into(),
        server_port: 0,
        turso_sync_interval_secs: 300,
        device_creds_key: test_device_creds_key(),
        timezone: "America/Caracas".parse().unwrap(),
        license_jwt_path: String::new(),
        do_functions_activate_url: String::new(),
        do_functions_renew_url: String::new(),
        cors_allowed_origins: Vec::new(),
        cookie_secure: false,
        device_push_base_url: String::new(),
    })
}

async fn seed_employee(conn: &libsql::Connection) -> String {
    let dept_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, shift_type, is_overnight_shift, ordinary_daily_minutes, \
         status, version, created_at, updated_at) \
         VALUES (?1, ?2, 0, '08:00', '17:00', 'fixed', 60, 'day', 0, 480, 'active', 1, unixepoch(), unixepoch())",
        params![dept_id.clone(), format!("Dept-{}", &dept_id[..8])],
    )
    .await
    .unwrap();
    let emp_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Emp', ?3, 'active', 1, unixepoch(), unixepoch())",
        params![emp_id.clone(), format!("E-{}", &emp_id[..8]), dept_id.clone()],
    )
    .await
    .unwrap();
    emp_id
}

/// Insert a face_enrollments row and materialise its photo on disk under
/// `enrollments_root/{emp}/{enr}.jpg`. Returns the absolute photo path.
async fn seed_enrolled_face(
    conn: &libsql::Connection,
    enrollments_root: &std::path::Path,
    employee_id: &str,
    created_by: &str,
) -> std::path::PathBuf {
    let enr_id = uuid::Uuid::new_v4().to_string();
    let relpath = format!("{employee_id}/{enr_id}.jpg");
    conn.execute(
        "INSERT INTO face_enrollments \
         (id, employee_id, captured_via, source_device_id, photo_path, face_quality_score, created_by, created_at) \
         VALUES (?1, ?2, 'upload', NULL, ?3, NULL, ?4, unixepoch())",
        params![
            enr_id.clone(),
            employee_id.to_string(),
            relpath.clone(),
            created_by.to_string()
        ],
    )
    .await
    .unwrap();
    conn.execute(
        "UPDATE employees SET face_id = ?1, current_face_enrollment_id = ?2 WHERE id = ?3",
        params![
            format!("face-{}", &enr_id[..8]),
            enr_id.clone(),
            employee_id.to_string()
        ],
    )
    .await
    .unwrap();

    let abs = enrollments_root.join(&relpath);
    std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
    std::fs::write(&abs, common::MINI_JPEG).unwrap();
    abs
}

/// H-10: after deactivation, the enrolled face template is gone from disk.
#[tokio::test]
async fn deactivating_an_employee_deletes_the_enrolled_face() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let conn = state.db.connect().unwrap();
    let emp = seed_employee(&conn).await;
    let photo = seed_enrolled_face(&conn, &state.paths.enrollments_root, &emp, &admin).await;
    drop(conn);

    assert!(
        photo.exists(),
        "precondition: the enrolled face photo exists on disk"
    );

    let removed = enrollment_service::purge_enrolled_faces(&state, &emp)
        .await
        .expect("purge succeeds");

    assert_eq!(removed, 1, "exactly one enrolled face was purged");
    assert!(
        !photo.exists(),
        "the enrolled face template no longer exists on disk"
    );
}

/// H-09: the attendance evidence of the employee's punches is NOT touched. This
/// is the test that stops H-10 being "solved" by deleting proof of work.
#[tokio::test]
async fn deactivating_an_employee_keeps_the_attendance_evidence() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let conn = state.db.connect().unwrap();
    let emp = seed_employee(&conn).await;
    seed_enrolled_face(&conn, &state.paths.enrollments_root, &emp, &admin).await;
    drop(conn);

    // A captured event photo and a leave-evidence file. Neither lives under
    // enrollments_root, so the purge must leave both alone.
    let event_photo = state.paths.events_root.join(format!("{emp}/punch-evidence.jpg"));
    std::fs::create_dir_all(event_photo.parent().unwrap()).unwrap();
    std::fs::write(&event_photo, common::MINI_JPEG).unwrap();

    std::fs::create_dir_all(&state.paths.leaves_root).unwrap();
    let leave_evidence = state.paths.leaves_root.join("leave-evidence.pdf");
    std::fs::write(&leave_evidence, b"%PDF-1.4 leave evidence").unwrap();

    enrollment_service::purge_enrolled_faces(&state, &emp)
        .await
        .expect("purge succeeds");

    assert!(
        event_photo.exists(),
        "H-09: attendance event evidence must be preserved"
    );
    assert!(
        leave_evidence.exists(),
        "H-09: leave evidence must be preserved"
    );
}

/// The purge is auditable: a DELETE row on face_enrollments for the employee.
#[tokio::test]
async fn the_purge_is_recorded_in_the_audit_log() {
    let db = common::test_db().await;
    let admin = create_test_admin(&db).await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let conn = state.db.connect().unwrap();
    let emp = seed_employee(&conn).await;
    seed_enrolled_face(&conn, &state.paths.enrollments_root, &emp, &admin).await;
    drop(conn);

    enrollment_service::purge_enrolled_faces(&state, &emp)
        .await
        .expect("purge succeeds");

    // The audit row is written through the single-writer queue; the statement
    // future resolves after commit, so it is visible immediately — poll briefly
    // only to be robust against WAL read-snapshot timing.
    let conn = state.db.connect().unwrap();
    let mut found = false;
    for _ in 0..50 {
        let mut rows = conn
            .query(
                "SELECT actor_id, old_data FROM audit_log \
                 WHERE table_name = 'face_enrollments' AND operation = 'DELETE' AND record_id = ?1",
                params![emp.clone()],
            )
            .await
            .unwrap();
        if let Some(row) = rows.next().await.unwrap() {
            let actor: Option<String> = row.get(0).unwrap();
            let old_data: Option<String> = row.get(1).unwrap();
            assert!(
                actor.is_none(),
                "system-initiated purge is recorded with a null actor"
            );
            assert!(
                old_data.unwrap_or_default().contains("H-10"),
                "old_data records the purge reason"
            );
            found = true;
            break;
        }
        drop(rows);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(found, "the purge produced an audit_log entry");
}
