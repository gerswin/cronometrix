//! Bloque 3 (H-10, Task 2): the configurable retention sweep.
//!
//! The sweep's safety decision is its DEFAULT: with no period configured it must
//! delete nothing, however old the file. With a period set it deletes only what
//! exceeds it, and every deletion is audited. `now` is injected into `sweep_once`
//! so file age is deterministic without touching the wall clock.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use cronometrix_api::config::Config;
use cronometrix_api::workers::retention;
use libsql::params;
use tokio_util::sync::CancellationToken;

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

/// Materialise a `.jpg` under `root/rel` and backdate its mtime by `age`.
fn write_jpg_aged(root: &Path, rel: &str, age: Duration) -> PathBuf {
    let abs = root.join(rel);
    std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
    std::fs::write(&abs, common::MINI_JPEG).unwrap();
    let f = std::fs::OpenOptions::new().write(true).open(&abs).unwrap();
    f.set_modified(SystemTime::now() - age).unwrap();
    abs
}

async fn set_events_retention(state: &cronometrix_api::state::AppState, days: i64) {
    let conn = state.db.connect().unwrap();
    conn.execute(
        "UPDATE retention_policy SET events_retention_days = ?1 WHERE id = 1",
        params![days],
    )
    .await
    .unwrap();
}

/// The safe default: nothing is deleted, no matter how old.
#[tokio::test]
async fn the_default_policy_deletes_nothing() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    let ancient = write_jpg_aged(
        &state.paths.events_root,
        "emp-1/old.jpg",
        Duration::from_secs(3650 * 24 * 60 * 60), // ~10 years
    );

    let stats = retention::sweep_once(&state, SystemTime::now())
        .await
        .expect("sweep succeeds");

    assert_eq!(stats.deleted, 0, "default policy deletes nothing");
    assert!(ancient.exists(), "the file is preserved under the default policy");
}

/// With a configured period, only files older than it are deleted.
#[tokio::test]
async fn a_configured_period_deletes_only_what_exceeds_it() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    set_events_retention(&state, 1).await;

    let old = write_jpg_aged(
        &state.paths.events_root,
        "emp-1/old.jpg",
        Duration::from_secs(3 * 24 * 60 * 60),
    );
    let fresh = write_jpg_aged(&state.paths.events_root, "emp-1/fresh.jpg", Duration::ZERO);

    let stats = retention::sweep_once(&state, SystemTime::now())
        .await
        .expect("sweep succeeds");

    assert_eq!(stats.deleted, 1, "only the over-age file is deleted");
    assert_eq!(stats.kept, 1, "the fresh file is kept");
    assert!(!old.exists(), "the over-age file is gone");
    assert!(fresh.exists(), "the fresh file remains");
}

/// A zero or negative period is a misconfiguration — never read as "delete all".
#[tokio::test]
async fn a_non_positive_period_keeps_everything() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    set_events_retention(&state, 0).await;

    let old = write_jpg_aged(
        &state.paths.events_root,
        "emp-1/old.jpg",
        Duration::from_secs(365 * 24 * 60 * 60),
    );

    let stats = retention::sweep_once(&state, SystemTime::now())
        .await
        .expect("sweep succeeds");

    assert_eq!(stats.deleted, 0, "a zero period keeps everything");
    assert!(old.exists());
}

/// Non-JPEG evidence has no validated deletion path yet — it is kept, not
/// raw-unlinked, even when over-age.
#[tokio::test]
async fn non_jpeg_evidence_is_skipped() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    set_events_retention(&state, 1).await;

    let pdf = state.paths.events_root.join("emp-1/report.pdf");
    std::fs::create_dir_all(pdf.parent().unwrap()).unwrap();
    std::fs::write(&pdf, b"%PDF-1.4").unwrap();
    let f = std::fs::OpenOptions::new().write(true).open(&pdf).unwrap();
    f.set_modified(SystemTime::now() - Duration::from_secs(30 * 24 * 60 * 60)).unwrap();
    drop(f);

    let stats = retention::sweep_once(&state, SystemTime::now())
        .await
        .expect("sweep succeeds");

    assert_eq!(stats.deleted, 0, "no JPEG was eligible");
    assert!(stats.skipped_other >= 1, "the PDF was skipped, not deleted");
    assert!(pdf.exists(), "non-JPEG evidence is preserved");
}

/// Every retention deletion produces an audit_log entry.
#[tokio::test]
async fn every_deletion_is_audited() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    set_events_retention(&state, 1).await;

    write_jpg_aged(
        &state.paths.events_root,
        "emp-1/old.jpg",
        Duration::from_secs(5 * 24 * 60 * 60),
    );

    let stats = retention::sweep_once(&state, SystemTime::now())
        .await
        .expect("sweep succeeds");
    assert_eq!(stats.deleted, 1);

    // Audit row is written through the single-writer queue; poll briefly.
    let conn = state.db.connect().unwrap();
    let mut found = false;
    for _ in 0..50 {
        let mut rows = conn
            .query(
                "SELECT COUNT(*) FROM audit_log WHERE table_name = 'events' AND operation = 'DELETE'",
                (),
            )
            .await
            .unwrap();
        let n: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
        drop(rows);
        if n >= 1 {
            found = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(found, "the retention deletion is audited");
}

/// The worker exits promptly when its shutdown token is cancelled.
#[tokio::test]
async fn worker_exits_on_shutdown() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let shutdown = CancellationToken::new();

    let s = shutdown.clone();
    let handle = tokio::spawn(async move { retention::run(state, s).await });

    tokio::time::sleep(Duration::from_millis(20)).await;
    shutdown.cancel();

    let r = tokio::time::timeout(Duration::from_secs(5), handle).await;
    assert!(r.is_ok(), "worker must exit promptly on shutdown");
    assert!(r.unwrap().is_ok(), "worker must not panic");
}

/// The worker actually fires a sweep on its cadence. Driven with a real short
/// cadence and a generous real-time poll — deterministic under both nextest and
/// `cargo test` (a paused clock racing real filesystem/DB I/O is not).
#[tokio::test]
async fn a_sweep_fires_on_the_cadence() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    set_events_retention(&state, 1).await;

    let old = write_jpg_aged(
        &state.paths.events_root,
        "emp-1/old.jpg",
        Duration::from_secs(3 * 24 * 60 * 60),
    );

    let shutdown = CancellationToken::new();
    let child = shutdown.clone();
    let handle = tokio::spawn(async move {
        retention::run_with_cadence(state, child, Duration::from_millis(10)).await
    });

    let mut gone = false;
    for _ in 0..500 {
        if !old.exists() {
            gone = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(gone, "the scheduled sweep deleted the over-age file");

    shutdown.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
}
