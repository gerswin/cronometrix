mod common;

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use cronometrix_api::config::Config;
use cronometrix_api::enrollments::handlers::CaptureState;
use cronometrix_api::workers::capture_cleanup;
use tokio_util::sync::CancellationToken;

use common::{test_device_creds_key, TEST_JWT_SECRET};

fn make_config() -> Arc<Config> {
    Arc::new(Config {
        database_path: "test".into(),
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

async fn state() -> (cronometrix_api::state::AppState, tempfile::TempDir) {
    let db = common::test_db().await;
    common::test_state_with_tmpdir(Arc::new(db), make_config())
}

fn capture(status: &str, created_at: Instant, terminal_at: Option<Instant>) -> CaptureState {
    CaptureState {
        status: status.to_string(),
        source_device_id: "device-1".into(),
        photo_path: None,
        error_message: None,
        created_at,
        terminal_at,
    }
}

#[tokio::test]
async fn capture_cleanup_expires_stuck_capturing_at_45_seconds() {
    let (state, _tmp) = state().await;
    let now = Instant::now();
    state.captures.write().await.insert(
        "stuck".into(),
        capture("capturing", now - Duration::from_secs(45), None),
    );

    capture_cleanup::cleanup_once(
        &state,
        capture_cleanup::CleanupNow::new(now, SystemTime::now()),
    )
    .await
    .unwrap();

    let entry = state.captures.read().await.get("stuck").cloned().unwrap();
    assert_eq!(entry.status, "timeout");
    assert_eq!(entry.terminal_at, Some(now));
}

#[tokio::test]
async fn capture_cleanup_removes_terminal_state_and_jpeg_after_5_minutes() {
    let (state, _tmp) = state().await;
    let now = Instant::now();
    let terminal_at = now - Duration::from_secs(300);
    tokio::fs::create_dir_all(&state.paths.captures_tmp_root)
        .await
        .unwrap();
    let captured_path = state.paths.captures_tmp_root.join("captured.jpg");
    tokio::fs::write(&captured_path, b"jpeg").await.unwrap();

    let mut captured = capture("captured", terminal_at, Some(terminal_at));
    captured.photo_path = Some(captured_path.to_string_lossy().into_owned());
    let mut map = state.captures.write().await;
    map.insert("captured".into(), captured);
    map.insert(
        "timeout".into(),
        capture("timeout", terminal_at, Some(terminal_at)),
    );
    map.insert(
        "error".into(),
        capture("error", terminal_at, Some(terminal_at)),
    );
    drop(map);

    capture_cleanup::cleanup_once(
        &state,
        capture_cleanup::CleanupNow::new(now, SystemTime::now()),
    )
    .await
    .unwrap();

    assert!(!captured_path.exists(), "JPEG must be deleted before state");
    assert!(state.captures.read().await.is_empty());
}

#[tokio::test]
async fn cleanup_compare_remove_preserves_state_replaced_after_file_delete() {
    let (state, _tmp) = state().await;
    let now = Instant::now();
    let terminal_at = now - capture_cleanup::TERMINAL_TTL;
    tokio::fs::create_dir_all(&state.paths.captures_tmp_root)
        .await
        .unwrap();
    let path = state.paths.captures_tmp_root.join("raced.jpg");
    tokio::fs::write(&path, b"jpeg").await.unwrap();
    let mut old = capture("captured", terminal_at, Some(terminal_at));
    old.photo_path = Some(path.to_string_lossy().into_owned());
    state.captures.write().await.insert("raced".into(), old);
    let replacement_state = state.clone();

    capture_cleanup::cleanup_once_with_after_delete(
        &state,
        capture_cleanup::CleanupNow::new(now, SystemTime::now()),
        move |capture_id| {
            let state = replacement_state.clone();
            async move {
                state.captures.write().await.insert(
                    capture_id,
                    capture("error", Instant::now(), Some(Instant::now())),
                );
            }
        },
    )
    .await
    .unwrap();

    assert!(!path.exists());
    let replacement = state.captures.read().await.get("raced").cloned().unwrap();
    assert_eq!(replacement.status, "error");
}

#[tokio::test]
async fn capture_cleanup_retains_state_when_jpeg_delete_fails() {
    let (state, _tmp) = state().await;
    let now = Instant::now();
    let terminal_at = now - Duration::from_secs(300);
    let undeletable = state.paths.captures_tmp_root.join("directory.jpg");
    tokio::fs::create_dir_all(&undeletable).await.unwrap();
    let mut captured = capture("captured", terminal_at, Some(terminal_at));
    captured.photo_path = Some(undeletable.to_string_lossy().into_owned());
    state
        .captures
        .write()
        .await
        .insert("directory".into(), captured);

    capture_cleanup::cleanup_once(
        &state,
        capture_cleanup::CleanupNow::new(now, SystemTime::now()),
    )
    .await
    .unwrap();

    assert!(state.captures.read().await.contains_key("directory"));
    assert!(undeletable.exists());
}

#[tokio::test]
async fn capture_cleanup_keeps_fresh_orphan_jpeg_without_map_entry() {
    let (state, _tmp) = state().await;
    tokio::fs::create_dir_all(&state.paths.captures_tmp_root)
        .await
        .unwrap();
    let orphan = state.paths.captures_tmp_root.join("orphan.jpg");
    tokio::fs::write(&orphan, b"jpeg").await.unwrap();

    capture_cleanup::cleanup_once(
        &state,
        capture_cleanup::CleanupNow::new(Instant::now(), SystemTime::now()),
    )
    .await
    .unwrap();

    assert!(orphan.exists());
}

#[tokio::test]
async fn capture_cleanup_removes_expired_orphan_jpeg_without_map_entry() {
    let (state, _tmp) = state().await;
    tokio::fs::create_dir_all(&state.paths.captures_tmp_root)
        .await
        .unwrap();
    let orphan = state.paths.captures_tmp_root.join("orphan-expired.jpg");
    tokio::fs::write(&orphan, b"jpeg").await.unwrap();
    let modified = tokio::fs::metadata(&orphan)
        .await
        .unwrap()
        .modified()
        .unwrap();

    capture_cleanup::cleanup_once(
        &state,
        capture_cleanup::CleanupNow::new(Instant::now(), modified + capture_cleanup::TERMINAL_TTL),
    )
    .await
    .unwrap();

    assert!(!orphan.exists());
}

#[tokio::test]
async fn capture_shutdown_awaits_tasks_and_removes_state_and_jpegs() {
    let (state, _tmp) = state().await;
    let now = Instant::now();
    tokio::fs::create_dir_all(&state.paths.captures_tmp_root)
        .await
        .unwrap();
    let path = state.paths.captures_tmp_root.join("shutdown.jpg");
    tokio::fs::write(&path, b"jpeg").await.unwrap();
    let mut captured = capture("captured", now, Some(now));
    captured.photo_path = Some(path.to_string_lossy().into_owned());
    state
        .captures
        .write()
        .await
        .insert("shutdown".into(), captured);
    state
        .captures
        .spawn(async { std::future::pending::<()>().await })
        .await
        .unwrap();

    let cleanup_shutdown = CancellationToken::new();
    let cleanup_handle = tokio::spawn(capture_cleanup::run(
        state.clone(),
        cleanup_shutdown.clone(),
    ));
    capture_cleanup::shutdown_captures(&state).await.unwrap();
    cleanup_shutdown.cancel();
    cleanup_handle.await.unwrap().unwrap();

    assert!(state.captures.read().await.is_empty());
    let mut entries = tokio::fs::read_dir(&state.paths.captures_tmp_root)
        .await
        .unwrap();
    assert!(entries.next_entry().await.unwrap().is_none());
}

#[tokio::test]
async fn capture_admission_abort_while_map_is_blocked_leaves_no_state_or_task() {
    let (state, _tmp) = state().await;
    let entries_guard = state.captures.write().await;
    let captures = state.captures.clone();
    let now = Instant::now();
    let handle = tokio::spawn(async move {
        captures
            .admit("cancelled".into(), capture("capturing", now, None), async {
                std::future::pending::<()>().await
            })
            .await
    });
    tokio::task::yield_now().await;
    handle.abort();
    assert!(handle.await.unwrap_err().is_cancelled());
    drop(entries_guard);

    assert!(state.captures.read().await.is_empty());
    state.captures.stop_and_join().await;
}

#[tokio::test]
async fn startup_sweep_fails_before_runtime_when_capture_root_is_not_a_directory() {
    let (state, _tmp) = state().await;
    if let Some(parent) = state.paths.captures_tmp_root.parent() {
        tokio::fs::create_dir_all(parent).await.unwrap();
    }
    tokio::fs::write(&state.paths.captures_tmp_root, b"not-a-directory")
        .await
        .unwrap();

    capture_cleanup::startup_sweep(&state, capture_cleanup::CleanupNow::now())
        .await
        .expect_err("startup must fail closed when orphan inspection cannot start");
}
