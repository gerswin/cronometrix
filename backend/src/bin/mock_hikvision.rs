//! Phase 9 — Mock Hikvision device binary. Gated by Cargo feature "mock-hikvision".
//!
//! Two HTTP servers:
//! - Public port (MOCK_HIKVISION_PORT, default 4400): impersonates a Hikvision unit
//!   for the backend's outbound ISAPI client (alertStream + commands). Records every
//!   incoming PUT/POST in recv_log for test inspection (B6 contract).
//! - Admin port (MOCK_HIKVISION_ADMIN_PORT, default 4401): test-only API for specs
//!   to inject events into the alertStream queue AND to read recv_log (B6).
//!
//! The mock does NOT enforce digest auth — it answers all requests unconditionally.
//! This is intentional: E2E tests validate application logic, not auth protocol.
//!
//! alertStream format: multipart/mixed; boundary=MIME_boundary (matches backend/src/isapi/stream.rs)

use axum::{
    body::Body,
    extract::State,
    http::{header, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// One recorded call to the public surface (B6 contract for devices.spec.ts).
#[derive(Clone, Debug, Serialize)]
struct ReceivedCall {
    method: String,
    path: String,
    body: String,
    timestamp_ms: u128,
}

/// Shared state between the two Axum routers.
#[derive(Clone)]
struct MockState {
    /// XML payloads the alertStream endpoint will emit when polled.
    event_queue: Arc<Mutex<Vec<String>>>,
    /// Every PUT/POST that reached the public surface (B6).
    recv_log: Arc<Mutex<Vec<ReceivedCall>>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();

    let public_port: u16 = std::env::var("MOCK_HIKVISION_PORT")
        .unwrap_or_else(|_| "4400".into())
        .parse()?;
    let admin_port: u16 = std::env::var("MOCK_HIKVISION_ADMIN_PORT")
        .unwrap_or_else(|_| "4401".into())
        .parse()?;

    let state = MockState {
        event_queue: Arc::new(Mutex::new(Vec::new())),
        recv_log: Arc::new(Mutex::new(Vec::new())),
    };

    // -------- PUBLIC SURFACE (impersonates Hikvision unit) --------
    let public = Router::new()
        // Probe used by the backend's device healthcheck
        .route("/ISAPI/System/status", get(handle_status))
        // Long-lived alertStream that streams queued XML events
        .route(
            "/ISAPI/Event/notification/alertStream",
            get(handle_alert_stream),
        )
        // Outbound commands — record in recv_log + return canned 200 XML
        .route("/ISAPI/RemoteControl/door/0", put(handle_recorded_put))
        .route(
            "/ISAPI/AccessControl/UserInfo/Record",
            put(handle_recorded_put),
        )
        .route(
            "/ISAPI/Intelligent/FDLib/FaceDataRecord",
            put(handle_recorded_put),
        )
        .route(
            "/ISAPI/AccessControl/UserInfoDetail/Delete",
            put(handle_recorded_put),
        )
        .with_state(state.clone());

    // -------- ADMIN SURFACE (test injection + introspection) --------
    let admin = Router::new()
        .route("/admin/push-event", post(handle_push_event))
        .route("/admin/clear-queue", post(handle_clear_queue))
        .route("/admin/recv-log", get(handle_recv_log)) // B6
        .route("/admin/clear-recv-log", post(handle_clear_recv_log)) // B6
        .route("/admin/health", get(|| async { "ok" }))
        .with_state(state.clone());

    let public_listener = tokio::net::TcpListener::bind(("127.0.0.1", public_port)).await?;
    let admin_listener = tokio::net::TcpListener::bind(("127.0.0.1", admin_port)).await?;

    tracing::info!(
        "mock_hikvision: public on 127.0.0.1:{}, admin on 127.0.0.1:{}",
        public_port,
        admin_port
    );
    println!(
        "mock_hikvision listening (public={}, admin={})",
        public_port, admin_port
    );

    let public_task = tokio::spawn(async move { axum::serve(public_listener, public).await });
    let admin_task = tokio::spawn(async move { axum::serve(admin_listener, admin).await });

    // Wait for Ctrl-C then shut down both servers
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("mock_hikvision: shutting down");
    public_task.abort();
    admin_task.abort();
    Ok(())
}

// ── Public handlers ──────────────────────────────────────────────────────────

/// GET /ISAPI/System/status — device probe used by backend healthcheck.
async fn handle_status() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "OK",
        "deviceModel": "DS-K1T341",
        "firmwareVersion": "1.0.0"
    }))
}

/// GET /ISAPI/Event/notification/alertStream
///
/// Drain the event queue and emit each XML chunk as a multipart/mixed part.
/// Uses boundary "MIME_boundary" which matches the test fixtures in common/mod.rs
/// and the parser in backend/src/isapi/stream.rs.
async fn handle_alert_stream(State(state): State<MockState>) -> Response {
    let chunks: Vec<String> = {
        let mut queue = state.event_queue.lock().await;
        std::mem::take(&mut *queue)
    };

    let boundary = "MIME_boundary";
    let mut body = String::new();
    for xml in &chunks {
        body.push_str(&format!("--{}\r\n", boundary));
        body.push_str("Content-Type: application/xml\r\n");
        body.push_str(&format!("Content-Length: {}\r\n\r\n", xml.len()));
        body.push_str(xml);
        body.push_str("\r\n");
    }
    body.push_str(&format!("--{}--\r\n", boundary));

    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            format!("multipart/mixed; boundary={}", boundary),
        )
        .body(Body::from(body))
        .unwrap()
}

/// PUT /ISAPI/RemoteControl/door/0 and other command endpoints.
/// Records the call in recv_log (B6) and returns a canned 200 XML response.
async fn handle_recorded_put(State(state): State<MockState>, req: Request<Body>) -> Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let body_bytes = axum::body::to_bytes(req.into_body(), 64 * 1024)
        .await
        .unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body_bytes).to_string();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    state.recv_log.lock().await.push(ReceivedCall {
        method,
        path,
        body: body_str,
        timestamp_ms: ts,
    });

    let resp_xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                    <ResponseStatus>\
                    <statusCode>1</statusCode>\
                    <statusString>OK</statusString>\
                    </ResponseStatus>";
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/xml")
        .body(Body::from(resp_xml))
        .unwrap()
}

// ── Admin handlers ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PushEventPayload {
    xml: String,
}

/// POST /admin/push-event — queue an XML event for the next alertStream poll.
async fn handle_push_event(
    State(state): State<MockState>,
    Json(payload): Json<PushEventPayload>,
) -> Json<serde_json::Value> {
    state.event_queue.lock().await.push(payload.xml);
    Json(serde_json::json!({ "queued": true }))
}

/// POST /admin/clear-queue — empty the alertStream queue.
async fn handle_clear_queue(State(state): State<MockState>) -> Json<serde_json::Value> {
    state.event_queue.lock().await.clear();
    Json(serde_json::json!({ "cleared": true }))
}

/// GET /admin/recv-log — B6 contract: list every PUT/POST received on the public surface.
/// devices.spec.ts uses this to assert the backend dispatched the door-open command.
async fn handle_recv_log(State(state): State<MockState>) -> Json<serde_json::Value> {
    let log = state.recv_log.lock().await.clone();
    Json(serde_json::json!({ "commands": log }))
}

/// POST /admin/clear-recv-log — B6: empty the recv_log between tests.
async fn handle_clear_recv_log(State(state): State<MockState>) -> Json<serde_json::Value> {
    state.recv_log.lock().await.clear();
    Json(serde_json::json!({ "cleared": true }))
}
