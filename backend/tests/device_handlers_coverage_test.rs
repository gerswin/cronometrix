mod common;

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use cronometrix_api::auth::models::{Claims, Role};
use cronometrix_api::auth::rbac::AuthUser;
use cronometrix_api::config::Config;
use cronometrix_api::devices::handlers;
use cronometrix_api::devices::models::{CommandRequest, CreateDeviceRequest, UpdateDeviceRequest};
use cronometrix_api::errors::AppError;
use tokio::sync::mpsc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn config() -> Arc<Config> {
    Arc::new(Config {
        database_path: "test.db".into(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: common::TEST_JWT_SECRET.into(),
        server_host: "127.0.0.1".into(),
        server_port: 0,
        turso_sync_interval_secs: 300,
        device_creds_key: common::test_device_creds_key(),
        timezone: "America/Caracas".parse().unwrap(),
        license_jwt_path: String::new(),
        do_functions_activate_url: String::new(),
        do_functions_renew_url: String::new(),
        cors_allowed_origins: Vec::new(),
        cookie_secure: false,
    })
}

fn create_request(ip: &str, port: i64) -> CreateDeviceRequest {
    CreateDeviceRequest {
        name: "Coverage Device".into(),
        ip: ip.into(),
        port,
        scheme: "http".into(),
        username: "admin".into(),
        password: "secret".into(),
        direction: "entry".into(),
        allow_insecure_tls: false,
    }
}

fn admin_claims(actor_id: &str) -> Claims {
    let now = chrono::Utc::now().timestamp();
    Claims {
        sub: actor_id.into(),
        role: Role::Admin,
        exp: now + 3600,
        iat: now,
        jti: uuid::Uuid::new_v4().to_string(),
        token_type: "access".into(),
    }
}

#[tokio::test]
async fn create_publishes_backfill_even_when_lifecycle_receiver_is_closed() {
    let db = common::test_db().await;
    let (mut state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config());
    let (lifecycle_tx, lifecycle_rx) = mpsc::unbounded_channel();
    drop(lifecycle_rx);
    state.lifecycle_tx = Some(lifecycle_tx);
    let (backfill_tx, mut backfill_rx) = mpsc::unbounded_channel();
    state.backfill_tx = Some(backfill_tx);

    let (_, Json(created)) =
        handlers::create_device(State(state), Json(create_request("10.77.0.1", 8443)))
            .await
            .unwrap();

    let backfill = backfill_rx.recv().await.expect("backfill request");
    assert_eq!(backfill.device_id, created.id);
}

#[tokio::test]
async fn update_rejects_invalid_payload_before_database_mutation() {
    let db = common::test_db().await;
    let (state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), config());
    let result = handlers::update_device(
        State(state),
        Path("missing".into()),
        Json(UpdateDeviceRequest {
            name: Some(String::new()),
            ip: None,
            port: None,
            scheme: None,
            username: None,
            password: None,
            direction: None,
            allow_insecure_tls: None,
            status: None,
            version: 1,
        }),
    )
    .await;

    assert!(matches!(result, Err(AppError::Validation { .. })));
}

#[tokio::test]
async fn dispatch_routes_reboot_and_enrollment_mode_and_audits_both() {
    let db = common::test_db().await;
    let db = Arc::new(db);
    let (state, _tmp) = common::test_state_with_tmpdir(db.clone(), config());
    let mock = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/ISAPI/System/reboot"))
        .respond_with(ResponseTemplate::new(200).set_body_string("rebooting"))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/ISAPI/AccessControl/CaptureFaceData"))
        .respond_with(ResponseTemplate::new(200).set_body_string("capture-ready"))
        .mount(&mock)
        .await;

    let address = mock.address();
    let (_, Json(created)) = handlers::create_device(
        State(state.clone()),
        Json(create_request(
            &address.ip().to_string(),
            address.port() as i64,
        )),
    )
    .await
    .unwrap();

    let actor_id = uuid::Uuid::new_v4().to_string();
    db.connect()
        .unwrap()
        .execute(
            "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
             VALUES (?1, ?2, 'Coverage Admin', 'hash', 'admin', 'active', 1, unixepoch(), unixepoch())",
            libsql::params![actor_id.clone(), format!("admin-{}", &actor_id[..8])],
        )
        .await
        .unwrap();
    let claims = admin_claims(&actor_id);

    for (command, expected_body) in [
        ("reboot", "rebooting"),
        ("enrollment_mode", "capture-ready"),
    ] {
        let Json(result) = handlers::dispatch_command(
            State(state.clone()),
            AuthUser(claims.clone()),
            Path(created.id.clone()),
            Json(CommandRequest {
                command: command.into(),
            }),
        )
        .await
        .unwrap();
        assert_eq!(result.outcome, "ok");
        assert_eq!(result.device_response, expected_body);
    }

    let count: i64 = db
        .connect()
        .unwrap()
        .query(
            "SELECT COUNT(*) FROM command_audit_log WHERE device_id = ?1",
            libsql::params![created.id],
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    assert_eq!(count, 2);
}
