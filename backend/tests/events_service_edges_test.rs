mod common;

use std::sync::Arc;

use cronometrix_api::config::Config;
use cronometrix_api::events::models::{EventListQuery, NewAttendanceEvent, PersistOutcome};
use cronometrix_api::events::service;

fn make_config() -> Arc<Config> {
    Arc::new(Config {
        database_path: "test".into(),
        turso_url: String::new(),
        turso_token: String::new(),
        jwt_secret: common::TEST_JWT_SECRET.to_string(),
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

async fn seed_directory(conn: &libsql::Connection) {
    conn.execute(
        "INSERT INTO departments \
         (id, name, base_salary_cents, shift_start_time, shift_end_time, lunch_mode, \
          lunch_duration_min, status, version, created_at, updated_at) \
         VALUES ('dept-events', 'Events', 0, '08:00', '17:00', 'fixed', 60, \
                 'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO employees \
         (id, employee_code, name, department_id, status, version, created_at, updated_at) \
         VALUES ('emp-events', 'EMP-EVENTS', 'Event Employee', 'dept-events', \
                 'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
    conn.execute(
        "INSERT INTO devices \
         (id, name, ip, port, scheme, username, encrypted_password, direction, \
          allow_insecure_tls, connection_state, status, version, created_at, updated_at) \
         VALUES ('dev-events', 'Events Device', '127.0.0.1', 29341, 'http', 'admin', \
                 'ciphertext', 'entry', 0, 'offline', 'active', 1, unixepoch(), unixepoch())",
        (),
    )
    .await
    .unwrap();
}

fn event(id: &str, employee_id: Option<&str>, captured_at: i64) -> NewAttendanceEvent {
    NewAttendanceEvent {
        id: id.to_string(),
        employee_id: employee_id.map(str::to_string),
        device_id: "dev-events".to_string(),
        direction: "entry".to_string(),
        captured_at,
        is_unknown: employee_id.is_none(),
        face_id: Some("face-events".to_string()),
        employee_no_string: Some("EMP-EVENTS".to_string()),
        raw_xml: "<EventNotificationAlert/>".to_string(),
        photo_bytes: None,
    }
}

#[tokio::test]
async fn sse_enrichment_handles_missing_employee_and_query_failure() {
    let db = common::test_db().await;
    let (mut state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    let (tx, mut rx) = tokio::sync::broadcast::channel(4);
    state.event_broadcast = Some(tx);
    let missing = event(
        "evt-missing-employee",
        Some("missing-employee"),
        1_700_000_000,
    );

    service::publish_sse_event(&state, &missing, &Some("photo.jpg".into())).await;
    let payload = rx.recv().await.unwrap();
    assert_eq!(payload.id, "evt-missing-employee");
    assert_eq!(payload.employee_name, None);
    assert!(payload.has_photo);

    state
        .db
        .connect()
        .unwrap()
        .execute(
            "ALTER TABLE departments RENAME TO unavailable_departments",
            (),
        )
        .await
        .unwrap();
    service::publish_sse_event(&state, &missing, &None).await;
    let fallback = rx.recv().await.unwrap();
    assert_eq!(fallback.id, "evt-missing-employee");
    assert_eq!(fallback.department, None);
    assert!(!fallback.has_photo);
}

#[tokio::test]
async fn lookup_falls_back_to_active_employee_and_ignores_empty_identifier() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed_directory(&conn).await;

    let found = service::lookup_employee_for_event(
        &conn,
        "dev-events",
        Some("unmapped-face"),
        Some("EMP-EVENTS"),
    )
    .await
    .unwrap();
    assert_eq!(found.as_deref(), Some("emp-events"));

    let absent =
        service::lookup_employee_for_event(&conn, "dev-events", Some("unmapped-face"), Some(""))
            .await
            .unwrap();
    assert_eq!(absent, None);
}

#[tokio::test]
async fn closed_recompute_channel_does_not_undo_committed_event() {
    let db = common::test_db().await;
    let (mut state, _tmp) = common::test_state_with_tmpdir(Arc::new(db), make_config());
    seed_directory(&state.db.connect().unwrap()).await;
    let (recompute_tx, recompute_rx) = tokio::sync::mpsc::unbounded_channel();
    drop(recompute_rx);
    state.recompute_tx = Some(recompute_tx);

    let outcome = service::persist_attendance_event_queued(
        &state,
        &state.paths.events_root,
        event("evt-closed-recompute", Some("emp-events"), 1_700_000_000),
    )
    .await
    .unwrap();
    assert_eq!(outcome, PersistOutcome::Inserted { photo_path: None });

    let stored: i64 = state
        .db
        .connect()
        .unwrap()
        .query(
            "SELECT COUNT(*) FROM attendance_events WHERE id='evt-closed-recompute'",
            (),
        )
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap()
        .get(0)
        .unwrap();
    assert_eq!(stored, 1);
}

#[tokio::test]
async fn list_and_single_record_reads_cover_unknown_and_photo_edges() {
    let db = common::test_db().await;
    let conn = db.connect().unwrap();
    seed_directory(&conn).await;
    conn.execute(
        "INSERT INTO attendance_events \
         (id, employee_id, device_id, direction, captured_at, bucket_30s, is_unknown, \
          face_id, employee_no_string, raw_xml, photo_path, created_at) VALUES \
         ('known-event', 'emp-events', 'dev-events', 'entry', 1000, 33, 0, \
          'known-face', 'EMP-EVENTS', '<known/>', '2026/known.jpg', unixepoch()), \
         ('unknown-event', NULL, 'dev-events', 'exit', 2000, 66, 1, \
          'unknown-face', NULL, '<unknown/>', NULL, unixepoch())",
        (),
    )
    .await
    .unwrap();

    let page = service::list(
        &conn,
        EventListQuery {
            limit: Some(500),
            offset: Some(-4),
            employee_id: Some("emp-events".into()),
            device_id: Some("dev-events".into()),
            from: Some(900),
            to: Some(1100),
            include_unknown: Some(false),
        },
    )
    .await
    .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.limit, 100);
    assert_eq!(page.offset, 0);
    assert_eq!(page.data[0].id, "known-event");

    let known = service::get_by_id(&conn, "known-event").await.unwrap();
    assert_eq!(known.employee_id.as_deref(), Some("emp-events"));
    assert_eq!(
        service::get_photo_path(&conn, "known-event").await.unwrap(),
        "2026/known.jpg"
    );

    let no_photo = service::get_photo_path(&conn, "unknown-event")
        .await
        .unwrap_err();
    assert!(matches!(
        no_photo,
        cronometrix_api::errors::AppError::NotFound {
            code: "EVENT_PHOTO_NOT_FOUND",
            ..
        }
    ));
    let missing = service::get_photo_path(&conn, "missing-event")
        .await
        .unwrap_err();
    assert!(matches!(
        missing,
        cronometrix_api::errors::AppError::NotFound {
            code: "EVENT_NOT_FOUND",
            ..
        }
    ));
}
