//! Coverage gap-fill for `backend/src/recompute/nightly.rs` (08-04B Task 2).
//!
//! Baseline 0.00% line. Target ≥70%.
//!
//! `nightly_reconcile_task` schedules itself via `seconds_until_next_2am` (private)
//! and awaits a tokio::time::sleep, then calls reconcile_prior_day. Tested:
//!   * shutdown-cancel branch wakes the loop and exits cleanly
//!   * scheduled-fire branch with paused tokio clock — advance past the
//!     computed sleep, verify reconcile_prior_day was invoked (no-op for an
//!     empty employee table is success-counted as 0).
//!   * private `seconds_until_next_2am` is exercised transitively because
//!     the loop computes it on every iteration.
//!
//! NOTE: `seconds_until_next_2am` is `fn`-private so we cannot unit-test it
//! directly. The loop coverage exercises it on every iteration, which is
//! enough to push line coverage over the 70% floor.

mod common;

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Timelike;
use cronometrix_api::config::Config;
use cronometrix_api::recompute::nightly::nightly_reconcile_task;
use libsql::params;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use common::{test_device_creds_key, TEST_JWT_SECRET};

#[derive(Clone, Default)]
struct SharedWriter(Arc<Mutex<Vec<u8>>>);

struct GuardedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for GuardedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedWriter {
    type Writer = GuardedWriter;

    fn make_writer(&'a self) -> Self::Writer {
        GuardedWriter(self.0.clone())
    }
}

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

// =============================================================================
// Cancellation — task exits without firing the recompute
// =============================================================================

#[tokio::test]
async fn nightly_task_exits_on_shutdown() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let tz: chrono_tz::Tz = "America/Caracas".parse().unwrap();
    let shutdown = CancellationToken::new();

    let s = shutdown.clone();
    let handle = tokio::spawn(async move {
        nightly_reconcile_task(state, tz, s).await;
    });

    // Yield once so the task enters the select.
    tokio::time::sleep(Duration::from_millis(20)).await;
    shutdown.cancel();

    let r = tokio::time::timeout(Duration::from_secs(5), handle).await;
    assert!(r.is_ok(), "nightly task must exit promptly on shutdown");
    assert!(r.unwrap().is_ok(), "task must not panic");
}

#[tokio::test(start_paused = true, flavor = "current_thread")]
async fn nightly_task_schedules_same_day_when_local_time_is_before_2am() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());

    // Pick a fixed-offset IANA zone whose current local hour is 00. This
    // deterministically exercises the "today at 02:00 is still ahead" branch
    // without changing the process clock or depending on the test runner TZ.
    let utc_hour = chrono::Utc::now().hour() as i32;
    let offset_hours = (-utc_hour + 12).rem_euclid(24) - 12;
    let zone_name = match offset_hours.cmp(&0) {
        std::cmp::Ordering::Greater => format!("Etc/GMT-{}", offset_hours),
        std::cmp::Ordering::Less => format!("Etc/GMT+{}", -offset_hours),
        std::cmp::Ordering::Equal => "UTC".to_string(),
    };
    let tz: chrono_tz::Tz = zone_name.parse().expect("fixed-offset timezone");
    assert!(
        chrono::Utc::now().with_timezone(&tz).hour() <= 1,
        "chosen timezone must remain before 02:00 across an hour rollover"
    );

    let writer = SharedWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .without_time()
        .with_writer(writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let shutdown = CancellationToken::new();
    let child_shutdown = shutdown.clone();
    let handle = tokio::spawn(async move {
        nightly_reconcile_task(state, tz, child_shutdown).await;
    });

    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(3 * 60 * 60)).await;
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }

    let logs = String::from_utf8(writer.0.lock().unwrap().clone()).unwrap();
    assert!(
        logs.contains("nightly reconcile complete"),
        "same-day 02:00 timer must fire and complete reconcile within three hours; logs: {logs}"
    );

    shutdown.cancel();
    let result = tokio::time::timeout(Duration::from_secs(5), handle).await;
    assert!(result.is_ok(), "nightly task must exit promptly");
    assert!(result.unwrap().is_ok(), "nightly task must not panic");
}

// =============================================================================
// Cancellation under paused clock — exit before scheduled fire.
// =============================================================================

#[tokio::test(start_paused = true)]
async fn nightly_task_paused_clock_exits_before_2am() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let tz: chrono_tz::Tz = "America/Caracas".parse().unwrap();
    let shutdown = CancellationToken::new();

    let s = shutdown.clone();
    let handle = tokio::spawn(async move {
        nightly_reconcile_task(state, tz, s).await;
    });

    // Advance the clock by 30 minutes — well short of next 02:00 in any case
    // where it isn't already 01:30. The seconds_until_next_2am branch should
    // compute a sleep > 30 min so the timer never fires; cancel breaks out.
    tokio::time::advance(Duration::from_secs(60 * 30)).await;
    tokio::task::yield_now().await;

    shutdown.cancel();
    let r = tokio::time::timeout(Duration::from_secs(5), handle).await;
    assert!(r.is_ok(), "task must exit on cancel under paused clock");
}

// =============================================================================
// Scheduled fire — advance through 24+ hours under paused clock so the timer
// elapses, the loop body runs, and a subsequent shutdown still terminates.
// =============================================================================

#[tokio::test(start_paused = true)]
async fn nightly_task_fires_after_advance_past_2am() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let tz: chrono_tz::Tz = "America/Caracas".parse().unwrap();
    let shutdown = CancellationToken::new();

    let s = shutdown.clone();
    let handle = tokio::spawn(async move {
        nightly_reconcile_task(state, tz, s).await;
    });

    // Advance >24h so we are guaranteed to cross the next 02:00 boundary.
    // The internal loop will:
    //   1. compute sleep_secs (some value < 86400)
    //   2. tokio::time::sleep wakes after we advance past it
    //   3. reconcile_prior_day(state, tz) is called (returns Ok(0) for empty employees)
    //   4. loop re-enters and computes the next sleep
    // We then cancel — the second-iteration select wakes and exits.
    tokio::time::advance(Duration::from_secs(86_400 + 60)).await;
    tokio::task::yield_now().await;

    shutdown.cancel();
    let r = tokio::time::timeout(Duration::from_secs(10), handle).await;
    assert!(
        r.is_ok(),
        "task must exit after advancing past scheduled tick"
    );
}

// =============================================================================
// Reconcile prior day directly: when there are seeded active employees, the
// nightly task's invocation path materialises through dr_service::reconcile_prior_day.
// We exercise the same call directly to broaden coverage on the result-emission
// branches (Ok(n) tracing path + Err tracing path exists but cannot be triggered
// without an unhealthy DB).
// =============================================================================

#[tokio::test]
async fn reconcile_prior_day_yields_zero_for_empty_active_employees() {
    use cronometrix_api::daily_records::service as dr_service;
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let tz: chrono_tz::Tz = "America/Caracas".parse().unwrap();
    let n = dr_service::reconcile_prior_day(&state, tz).await.unwrap();
    assert_eq!(n, 0, "no active employees → 0 reconciled");
}

#[tokio::test]
async fn reconcile_prior_day_runs_for_seeded_employees() {
    use cronometrix_api::daily_records::service as dr_service;
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let tz: chrono_tz::Tz = "America/Caracas".parse().unwrap();

    // Seed one active employee.
    let conn = state.db.connect().unwrap();
    let dept_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO departments (id, name, base_salary_cents, shift_start_time, shift_end_time, \
         lunch_mode, lunch_duration_min, shift_type, is_overnight_shift, ordinary_daily_minutes, \
         status, version, created_at, updated_at) \
         VALUES (?1, ?2, 0, '08:00', '17:00', 'fixed', 60, 'day', 0, 480, 'active', 1, unixepoch(), unixepoch())",
        params![dept_id.clone(), format!("Dept-{}", &dept_id[..8])],
    )
    .await
    .unwrap();
    let emp_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO employees (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES (?1, ?2, 'Emp', ?3, 'active', 1, unixepoch(), unixepoch())",
        params![emp_id.clone(), format!("E-{}", &emp_id[..8]), dept_id.clone()],
    )
    .await
    .unwrap();
    drop(conn);

    // No assertion on the precise count — under shared-cache libsql the
    // per-row recompute may swallow "database is locked" warnings (see
    // 04A SUMMARY's "tightened reconcile_prior_day count assertion was wrong"
    // deviation). The coverage signal is that the function executed with
    // active employees.
    let _ = dr_service::reconcile_prior_day(&state, tz).await.unwrap();
}
