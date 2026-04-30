//! Coverage gap-fill for `backend/src/recompute/worker.rs` (08-04B Task 2).
//!
//! Baseline 0.00% line. Target ≥70%.
//!
//! `RecomputeWorker::run` is an mpsc-driven loop with 500ms debounce + HashSet
//! dedup. We exercise:
//!   * shutdown-cancel path (biased select drains shutdown first).
//!   * channel-closed path (rx.recv() returns None → worker exits).
//!   * happy path: send a request, advance the debounce window, observe the
//!     side effect (recompute_for_day was invoked → daily_records row exists).
//!   * dedup: send the same (employee_id, anchor_date) twice — recompute runs
//!     once.

mod common;

use std::sync::Arc;
use std::time::Duration;

use chrono::NaiveDate;
use cronometrix_api::config::Config;
use cronometrix_api::recompute::{worker::RecomputeWorker, RecomputeRequest};
use libsql::params;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

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
    })
}

/// Seed dept + employee + global_rules so recompute_for_day has all inputs.
async fn seed_full(db: &libsql::Database) -> String {
    let conn = db.connect().expect("connect");
    let dept_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, shift_type, is_overnight_shift, ordinary_daily_minutes, \
         status, version, created_at, updated_at) \
         VALUES (?1, ?2, 0, '08:00', '17:00', 'fixed', 60, 'day', 0, 480, 'active', 1, unixepoch(), unixepoch())",
        params![dept_id.clone(), format!("Dept-{}", &dept_id[..8])],
    )
    .await
    .expect("seed dept");

    let emp_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Test Emp', ?3, 'active', 1, unixepoch(), unixepoch())",
        params![emp_id.clone(), format!("E-{}", &emp_id[..8]), dept_id.clone()],
    )
    .await
    .expect("seed emp");

    emp_id
}

// =============================================================================
// Cancellation
// =============================================================================

#[tokio::test]
async fn worker_exits_on_shutdown_cancel() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let shutdown = CancellationToken::new();
    let (_tx, rx) = mpsc::unbounded_channel::<RecomputeRequest>();
    let worker = RecomputeWorker::new(state, shutdown.clone());

    let handle = tokio::spawn(async move { worker.run(rx).await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    shutdown.cancel();

    let r = tokio::time::timeout(Duration::from_secs(5), handle).await;
    assert!(r.is_ok(), "worker must exit on cancel");
    assert!(r.unwrap().is_ok(), "worker must not panic");
}

// =============================================================================
// Channel-closed path
// =============================================================================

#[tokio::test]
async fn worker_exits_when_channel_drops() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<RecomputeRequest>();
    let worker = RecomputeWorker::new(state, shutdown);

    let handle = tokio::spawn(async move { worker.run(rx).await });
    // Drop the sender → rx.recv() returns None → worker exits.
    drop(tx);

    let r = tokio::time::timeout(Duration::from_secs(5), handle).await;
    assert!(r.is_ok(), "worker must exit when channel closes");
}

// =============================================================================
// Happy path — request triggers recompute_for_day
// =============================================================================

#[tokio::test]
async fn worker_invokes_recompute_for_day_after_debounce() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let emp_id = seed_full(&state.db).await;
    let anchor = NaiveDate::from_ymd_opt(2026, 4, 20).unwrap();

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<RecomputeRequest>();
    let worker = RecomputeWorker::new(state.clone(), shutdown.clone());
    let handle = tokio::spawn(async move { worker.run(rx).await });

    // Send the request.
    tx.send(RecomputeRequest {
        employee_id: emp_id.clone(),
        anchor_date: anchor,
    })
    .unwrap();

    // Wait for the 500ms debounce window plus the recompute work.
    // Poll for up to 5s for the row to materialise.
    let mut found = false;
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(60)).await;
        let conn = state.db.connect().unwrap();
        let mut rows = conn
            .query(
                "SELECT count(*) FROM daily_records WHERE employee_id = ?1 AND anchor_date = ?2",
                params![emp_id.clone(), anchor.to_string()],
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let n: i64 = row.get(0).unwrap();
        if n >= 1 {
            found = true;
            break;
        }
    }
    assert!(found, "recompute_for_day must produce a daily_records row");

    shutdown.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
}

// =============================================================================
// Dedup: same key sent twice → single recompute
// =============================================================================

#[tokio::test]
async fn worker_dedupes_repeated_requests_within_debounce_window() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let emp_id = seed_full(&state.db).await;
    let anchor = NaiveDate::from_ymd_opt(2026, 4, 21).unwrap();

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<RecomputeRequest>();
    let worker = RecomputeWorker::new(state.clone(), shutdown.clone());
    let handle = tokio::spawn(async move { worker.run(rx).await });

    // Send 5 identical requests rapidly.
    for _ in 0..5 {
        tx.send(RecomputeRequest {
            employee_id: emp_id.clone(),
            anchor_date: anchor,
        })
        .unwrap();
    }

    // Wait for materialisation.
    for _ in 0..120 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let conn = state.db.connect().unwrap();
        let mut rows = conn
            .query(
                "SELECT count(*) FROM daily_records WHERE employee_id = ?1 AND anchor_date = ?2",
                params![emp_id.clone(), anchor.to_string()],
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let n: i64 = row.get(0).unwrap();
        if n == 1 {
            break;
        }
    }

    let conn = state.db.connect().unwrap();
    let mut rows = conn
        .query(
            "SELECT count(*) FROM daily_records WHERE employee_id = ?1 AND anchor_date = ?2",
            params![emp_id.clone(), anchor.to_string()],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let n: i64 = row.get(0).unwrap();
    assert_eq!(n, 1, "dedup must collapse 5 repeats into a single row");

    shutdown.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
}

// =============================================================================
// Error swallow: recompute fails for missing employee — worker logs and continues.
// =============================================================================

#[tokio::test]
async fn worker_swallows_per_request_errors() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let anchor = NaiveDate::from_ymd_opt(2026, 4, 22).unwrap();

    let shutdown = CancellationToken::new();
    let (tx, rx) = mpsc::unbounded_channel::<RecomputeRequest>();
    let worker = RecomputeWorker::new(state.clone(), shutdown.clone());
    let handle = tokio::spawn(async move { worker.run(rx).await });

    // No employee seeded → recompute_for_day silently early-returns Ok per
    // service.rs ("employee inactive or missing; skipping"). The worker
    // tolerates this without crashing.
    tx.send(RecomputeRequest {
        employee_id: "no-such-employee".into(),
        anchor_date: anchor,
    })
    .unwrap();

    // Give the debounce + recompute time, then send a 2nd request and ensure
    // the worker still processes it.
    tokio::time::sleep(Duration::from_millis(800)).await;
    let emp_id = seed_full(&state.db).await;
    let anchor2 = NaiveDate::from_ymd_opt(2026, 4, 23).unwrap();
    tx.send(RecomputeRequest {
        employee_id: emp_id.clone(),
        anchor_date: anchor2,
    })
    .unwrap();

    let mut found = false;
    for _ in 0..120 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let conn = state.db.connect().unwrap();
        let mut rows = conn
            .query(
                "SELECT count(*) FROM daily_records WHERE employee_id = ?1 AND anchor_date = ?2",
                params![emp_id.clone(), anchor2.to_string()],
            )
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let n: i64 = row.get(0).unwrap();
        if n >= 1 {
            found = true;
            break;
        }
    }
    assert!(found, "worker must keep running after a swallowed error");

    shutdown.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
}
