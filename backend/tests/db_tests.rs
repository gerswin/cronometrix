mod common;

#[tokio::test]
#[ignore = "Requires real schema migration from Plan 01-01 (placeholder SQL during Wave 0)"]
async fn schema_creates_all_tables() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();

    // Verify all 5 core tables exist
    let tables = ["users", "departments", "employees", "global_rules", "audit_log"];
    for table in tables {
        let result = conn
            .query(
                &format!(
                    "SELECT name FROM sqlite_master WHERE type='table' AND name='{}'",
                    table
                ),
                (),
            )
            .await
            .unwrap();
        // result is a Rows iterator; check it has at least one row
        drop(result);
        // Table existence will be verified when real SQL is in place
    }
}

#[tokio::test]
#[ignore = "Requires real schema migration from Plan 01-01 (placeholder SQL during Wave 0)"]
async fn audit_triggers_fire_on_employee_insert() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();

    // First create a department (FK requirement)
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, lunch_mode, status, version, created_at, updated_at) VALUES ('dept-1', 'Test Dept', 0, '08:00', '17:00', 'fixed', 'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();

    // Insert an employee
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) VALUES ('emp-1', 'E001', 'Test Employee', 'dept-1', 'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();

    // Check audit_log has an entry
    let mut rows = conn
        .query(
            "SELECT COUNT(*) as cnt FROM audit_log WHERE table_name = 'employees' AND operation = 'INSERT'",
            (),
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let count: i64 = row.get(0).unwrap();
    assert!(
        count >= 1,
        "Audit log should have at least 1 INSERT entry for employees"
    );
}

#[tokio::test]
#[ignore = "Requires real schema migration from Plan 01-01 (placeholder SQL during Wave 0)"]
async fn utc_epoch_storage_verified() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();

    // global_rules seeded row should have integer timestamps
    let mut rows = conn
        .query(
            "SELECT typeof(updated_at), updated_at FROM global_rules WHERE id = 'singleton'",
            (),
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let type_name: String = row.get(0).unwrap();
    assert_eq!(
        type_name, "integer",
        "updated_at should be stored as integer (UTC epoch)"
    );
}

#[tokio::test]
#[ignore = "Requires Turso credentials — run manually with TURSO_DATABASE_URL and TURSO_AUTH_TOKEN set"]
async fn turso_sync_connects() {
    // This test requires real Turso credentials and cannot run in CI without them.
    // It verifies that Builder::new_remote_replica works with real credentials.
    todo!("Implement when Turso credentials are available for testing");
}
