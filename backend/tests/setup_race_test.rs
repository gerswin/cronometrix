mod common;

use std::sync::Arc;

use cronometrix_api::config::Config;

/// C-09: dos altas simultáneas de primer admin. Exactamente una debe ganar.
/// Los nombres son distintos a propósito: un UNIQUE sobre `username` no
/// salvaría este caso, así que la prueba falla si la única defensa es ese índice.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_bootstrap_creates_exactly_one_admin() {
    let db = common::test_db().await;
    let config = Arc::new(Config {
        database_path: "test".to_string(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: common::TEST_JWT_SECRET.to_string(),
        server_host: "127.0.0.1".to_string(),
        server_port: 3001,
        turso_sync_interval_secs: 300,
        device_creds_key: common::test_device_creds_key(),
        timezone: "America/Caracas".parse().unwrap(),
        license_jwt_path: String::new(),
        do_functions_activate_url: String::new(),
        do_functions_renew_url: String::new(),
        cors_allowed_origins: Vec::new(),
        cookie_secure: false,
        device_push_base_url: String::new(),
    });
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);

    let a = {
        let state = state.clone();
        tokio::spawn(async move {
            cronometrix_api::setup::handlers::setup_init(
                axum::extract::State(state),
                axum::Json(cronometrix_api::setup::handlers::SetupInitRequest {
                    username: "admin_a".to_string(),
                    full_name: "Admin A".to_string(),
                    password: "correct horse battery".to_string(),
                }),
            )
            .await
            .is_ok()
        })
    };
    let b = {
        let state = state.clone();
        tokio::spawn(async move {
            cronometrix_api::setup::handlers::setup_init(
                axum::extract::State(state),
                axum::Json(cronometrix_api::setup::handlers::SetupInitRequest {
                    username: "admin_b".to_string(),
                    full_name: "Admin B".to_string(),
                    password: "correct horse battery".to_string(),
                }),
            )
            .await
            .is_ok()
        })
    };

    let wins = [a.await.unwrap(), b.await.unwrap()]
        .into_iter()
        .filter(|ok| *ok)
        .count();
    assert_eq!(wins, 1, "exactly one bootstrap must succeed");

    let conn = state.db.connect().unwrap();
    let mut rows = conn.query("SELECT COUNT(*) FROM users", ()).await.unwrap();
    let users: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(users, 1, "a second admin row must never exist");
}

/// Argon2 DoS follow-up (Finding 1): an already-initialized system must reject
/// `/setup/init` via the cheap pre-check fast path, not by running Argon2 and
/// then losing the in-transaction race.
///
/// What this test proves: the request is rejected with `SETUP_ALREADY_COMPLETE`
/// and no second user row is ever created, on an already-initialized system.
///
/// What this test does NOT prove: it does not measure wall-clock time and
/// makes no claim about hashing cost being avoided at runtime. That guarantee
/// comes from the source-level fact that the fast-path COUNT check in
/// `setup_init` runs and `return`s *before* the `service::hash_password` /
/// `spawn_blocking` call is ever reached — verified by code inspection, not by
/// this test. A timing-based assertion would be flaky (CPU contention on CI
/// runners), so it is deliberately not attempted here.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejects_second_init_without_creating_a_second_admin() {
    let db = common::test_db().await;
    let config = Arc::new(Config {
        database_path: "test".to_string(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: common::TEST_JWT_SECRET.to_string(),
        server_host: "127.0.0.1".to_string(),
        server_port: 3001,
        turso_sync_interval_secs: 300,
        device_creds_key: common::test_device_creds_key(),
        timezone: "America/Caracas".parse().unwrap(),
        license_jwt_path: String::new(),
        do_functions_activate_url: String::new(),
        do_functions_renew_url: String::new(),
        cors_allowed_origins: Vec::new(),
        cookie_secure: false,
        device_push_base_url: String::new(),
    });
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config);

    // First call bootstraps the system — this must succeed.
    let first = cronometrix_api::setup::handlers::setup_init(
        axum::extract::State(state.clone()),
        axum::Json(cronometrix_api::setup::handlers::SetupInitRequest {
            username: "admin_a".to_string(),
            full_name: "Admin A".to_string(),
            password: "correct horse battery".to_string(),
        }),
    )
    .await;
    assert!(first.is_ok(), "first /setup/init must bootstrap the admin");

    // Second call against an already-initialized system must be rejected by
    // the fast-path check with SETUP_ALREADY_COMPLETE.
    let second = cronometrix_api::setup::handlers::setup_init(
        axum::extract::State(state.clone()),
        axum::Json(cronometrix_api::setup::handlers::SetupInitRequest {
            username: "admin_b".to_string(),
            full_name: "Admin B".to_string(),
            password: "correct horse battery".to_string(),
        }),
    )
    .await;

    assert!(
        second.is_err(),
        "second /setup/init on an already-initialized system must be rejected"
    );
    match second.err().unwrap() {
        cronometrix_api::errors::AppError::Conflict { code, .. } => {
            assert_eq!(code, "SETUP_ALREADY_COMPLETE");
        }
        other => panic!("expected Conflict{{SETUP_ALREADY_COMPLETE}}, got {other:?}"),
    }

    let conn = state.db.connect().unwrap();
    let mut rows = conn.query("SELECT COUNT(*) FROM users", ()).await.unwrap();
    let users: i64 = rows.next().await.unwrap().unwrap().get(0).unwrap();
    assert_eq!(
        users, 1,
        "the rejected second /setup/init must not create a second admin row"
    );
}
