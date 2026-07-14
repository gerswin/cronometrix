//! Database-level guarantees for legal audit evidence.

mod common;

use libsql::params;
use uuid::Uuid;

type AuditSnapshot = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    i64,
);

async fn insert_audit(conn: &libsql::Connection, id: &str) {
    conn.execute(
        "INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at) \
         VALUES (?1, 'employees', 'employee-immutable', 'INSERT', NULL, '{\"name\":\"Original\"}', 'actor-original', 1770000000)",
        [id.to_string()],
    )
    .await
    .expect("audit INSERT remains allowed");
}

async fn snapshot(conn: &libsql::Connection, id: &str) -> AuditSnapshot {
    let row = conn
        .query(
            "SELECT id, table_name, record_id, operation, old_data, new_data, actor_id, created_at \
             FROM audit_log WHERE id = ?1",
            [id.to_string()],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .expect("audit row exists");
    (
        row.get(0).unwrap(),
        row.get(1).unwrap(),
        row.get(2).unwrap(),
        row.get(3).unwrap(),
        row.get(4).unwrap(),
        row.get(5).unwrap(),
        row.get(6).unwrap(),
        row.get(7).unwrap(),
    )
}

#[tokio::test]
async fn audit_insert_is_allowed() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let id = Uuid::new_v4().to_string();

    insert_audit(&conn, &id).await;

    assert_eq!(snapshot(&conn, &id).await.0, id);
}

#[tokio::test]
async fn audit_update_aborts_and_preserves_original_bytes() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let id = Uuid::new_v4().to_string();
    insert_audit(&conn, &id).await;
    let before = snapshot(&conn, &id).await;

    let err = conn
        .execute(
            "UPDATE audit_log SET actor_id = 'tampered', new_data = '{\"tampered\":true}' WHERE id = ?1",
            [id.clone()],
        )
        .await
        .expect_err("audit UPDATE must be rejected by SQLite");

    assert!(err.to_string().contains("audit_log is immutable"), "{err}");
    assert_eq!(snapshot(&conn, &id).await, before);
}

#[tokio::test]
async fn audit_delete_aborts_and_preserves_original_bytes() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let id = Uuid::new_v4().to_string();
    insert_audit(&conn, &id).await;
    let before = snapshot(&conn, &id).await;

    let err = conn
        .execute("DELETE FROM audit_log WHERE id = ?1", [id.clone()])
        .await
        .expect_err("audit DELETE must be rejected by SQLite");

    assert!(err.to_string().contains("audit_log is immutable"), "{err}");
    assert_eq!(snapshot(&conn, &id).await, before);
}

#[tokio::test]
async fn leave_and_override_audit_retain_actor_and_justification() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    let actor = common::create_test_admin(&db).await;
    let department = common::create_test_department_with_shift(
        &db, "Audit", "day", false, 480, "09:00", "17:00",
    )
    .await;
    let employee = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Audit Employee', ?3, 'active', 1, unixepoch(), unixepoch())",
        params![employee.clone(), format!("AUD-{}", &employee[..8]), department.clone()],
    )
    .await
    .unwrap();

    let leave_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO leaves (id, employee_id, from_date, to_date, leave_type, justification, evidence_path, created_by, status, version, created_at, updated_at) \
         VALUES (?1, ?2, '2026-07-01', '2026-07-01', 'manual', 'legal leave reason', NULL, ?3, 'active', 1, unixepoch(), unixepoch())",
        params![leave_id.clone(), employee.clone(), actor.clone()],
    )
    .await
    .unwrap();

    let create_audit = conn
        .query(
            "SELECT actor_id, json_extract(new_data, '$.created_by'), json_extract(new_data, '$.justification') \
             FROM audit_log WHERE table_name = 'leaves' AND record_id = ?1 AND operation = 'INSERT'",
            [leave_id.clone()],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(create_audit.get::<String>(0).unwrap(), actor);
    assert_eq!(create_audit.get::<String>(1).unwrap(), actor);
    assert_eq!(create_audit.get::<String>(2).unwrap(), "legal leave reason");

    conn.execute(
        "UPDATE leaves SET status = 'cancelled', cancelled_by = ?2, deleted_at = unixepoch(), version = version + 1 WHERE id = ?1",
        params![leave_id.clone(), actor.clone()],
    )
    .await
    .unwrap();
    let cancel_audit = conn
        .query(
            "SELECT actor_id, json_extract(new_data, '$.cancelled_by'), json_extract(new_data, '$.justification') \
             FROM audit_log WHERE table_name = 'leaves' AND record_id = ?1 AND operation = 'UPDATE' ORDER BY created_at DESC LIMIT 1",
            [leave_id],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cancel_audit.get::<String>(0).unwrap(), actor);
    assert_eq!(cancel_audit.get::<String>(1).unwrap(), actor);
    assert_eq!(cancel_audit.get::<String>(2).unwrap(), "legal leave reason");

    let daily_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO daily_records (id, employee_id, department_id, anchor_date, shift_type, work_minutes, overtime_minutes, late_minutes, early_departure_minutes, is_rest_day_worked, computed_at, created_at, updated_at) \
         VALUES (?1, ?2, ?3, '2026-07-02', 'day', 480, 0, 0, 0, 0, unixepoch(), unixepoch(), unixepoch())",
        params![daily_id.clone(), employee, department],
    )
    .await
    .unwrap();
    let override_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO daily_record_overrides (id, daily_record_id, override_work_minutes, justification, overridden_by, overridden_at, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 450, 'legal override reason', ?3, unixepoch(), 'active', 1, unixepoch(), unixepoch())",
        params![override_id.clone(), daily_id, actor.clone()],
    )
    .await
    .unwrap();
    let override_audit = conn
        .query(
            "SELECT actor_id, json_extract(new_data, '$.overridden_by'), json_extract(new_data, '$.justification') \
             FROM audit_log WHERE table_name = 'daily_record_overrides' AND record_id = ?1 AND operation = 'INSERT'",
            [override_id],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(override_audit.get::<String>(0).unwrap(), actor);
    assert_eq!(override_audit.get::<String>(1).unwrap(), actor);
    assert_eq!(
        override_audit.get::<String>(2).unwrap(),
        "legal override reason"
    );
}
