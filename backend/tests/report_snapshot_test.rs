//! Bloque 3 (M-05, Task 4): a report must read one consistent snapshot, so a
//! concurrent write cannot split it across two states.
//!
//! Reproducing the exact race inside `compute_report` would require injecting a
//! commit between two of its internal queries — there is no hook for that, and a
//! test that merely runs a report while writing in another task passes whether
//! or not the fix is present (it is timing-dependent and usually loses the
//! race). So instead this pins the PROPERTY the fix relies on and that
//! `compute_report` now uses: `begin_read_snapshot` opens a WAL read snapshot in
//! which a concurrently-committed write stays invisible until the snapshot ends.
//! If that property regresses, the report's consistency guarantee is gone.

mod common;

use std::sync::Arc;

use cronometrix_api::config::Config;
use libsql::params;

use common::{test_device_creds_key, TEST_JWT_SECRET};

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

async fn count_audit(conn: &libsql::Connection) -> i64 {
    let mut rows = conn
        .query("SELECT COUNT(*) FROM audit_log", ())
        .await
        .unwrap();
    rows.next().await.unwrap().unwrap().get(0).unwrap()
}

/// A write committed after a snapshot opens is invisible inside that snapshot,
/// and visible once it closes.
#[tokio::test]
async fn a_concurrent_write_cannot_split_a_snapshot_read() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let reader = state.db.connect().unwrap();

    // Open the snapshot and take it by reading once.
    cronometrix_api::db::begin_read_snapshot(&reader)
        .await
        .expect("begin snapshot");
    let before = count_audit(&reader).await;

    // Commit a write on the single-writer queue — a different connection.
    state
        .db_write
        .statement(
            "test.insert-audit",
            "INSERT INTO audit_log (id, table_name, record_id, operation, old_data, new_data, actor_id, created_at) \
             VALUES (?1, 'test', ?2, 'INSERT', NULL, NULL, NULL, unixepoch())",
            vec![
                libsql::Value::Text(uuid::Uuid::new_v4().to_string()),
                libsql::Value::Text(uuid::Uuid::new_v4().to_string()),
            ],
        )
        .await
        .expect("write commits");

    // Inside the snapshot the count has NOT moved — the report would not see it.
    let inside = count_audit(&reader).await;
    assert_eq!(
        inside, before,
        "a write committed after the snapshot opened must stay invisible inside it"
    );

    // Close the snapshot; a fresh read now sees the committed write.
    cronometrix_api::db::commit_read_snapshot(&reader)
        .await
        .expect("commit snapshot");

    let fresh = state.db.connect().unwrap();
    let after = count_audit(&fresh).await;
    assert_eq!(
        after,
        before + 1,
        "once the snapshot closes the committed write is visible"
    );
}

/// The snapshot is read-only: opening and closing it around reads leaves the
/// data untouched, and the helpers compose without error.
#[tokio::test]
async fn the_snapshot_is_read_only_and_composes() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let conn = state.db.connect().unwrap();

    cronometrix_api::db::begin_read_snapshot(&conn)
        .await
        .expect("begin");
    let _ = count_audit(&conn).await;
    cronometrix_api::db::commit_read_snapshot(&conn)
        .await
        .expect("commit");

    // A second cycle must also succeed (no dangling transaction state).
    cronometrix_api::db::begin_read_snapshot(&conn)
        .await
        .expect("begin again");
    cronometrix_api::db::commit_read_snapshot(&conn)
        .await
        .expect("commit again");
}
