use std::sync::Arc;

use axum::{
    extract::State,
    routing::{delete, get, patch, post},
    Json, Router,
};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use cronometrix_api::anomalies;
use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::daily_records;
use cronometrix_api::db;
use cronometrix_api::departments;
use cronometrix_api::devices;
use cronometrix_api::employees;
use cronometrix_api::enrollments;
use cronometrix_api::errors::AppError;
use cronometrix_api::events;
use cronometrix_api::leaves;
use cronometrix_api::license;
use cronometrix_api::recompute::{self, RecomputeRequest};
use cronometrix_api::reports;
use cronometrix_api::rules;
use cronometrix_api::setup;
use cronometrix_api::state::{AppState, AttendanceEventSSEPayload};
use cronometrix_api::supervisor::{watchdog, Supervisor};
use cronometrix_api::tenant_info;
use cronometrix_api::workers::{backfill::BackfillWorker, purge::PurgeWorker};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env before anything else (ok if file doesn't exist)
    dotenvy::dotenv().ok();

    // Initialize tracing — pretty format for development
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Config::from_env()?;

    tracing::info!("Initializing database...");
    let db = db::init_db(&config).await?;

    // Lifecycle channel: CRUD handlers -> Supervisor. Unbounded is safe because
    // admin actions are human-rate (1 event per CRUD), and the supervisor
    // drains the channel in its biased select loop.
    let (lifecycle_tx, lifecycle_rx) = mpsc::unbounded_channel();

    // Recompute channel: events/service -> RecomputeWorker. Unbounded is safe
    // because the worker drains with HashSet dedup; event burst collapses to a
    // single recompute per (employee_id, anchor_date).
    let (recompute_tx, recompute_rx) = mpsc::unbounded_channel::<RecomputeRequest>();

    // Event broadcast: attendance service -> SSE stream clients. Buffer 256 events;
    // lagged subscribers (slow clients) simply drop missed events — non-fatal for a
    // live activity feed.
    let (event_tx, _) = broadcast::channel::<AttendanceEventSSEPayload>(256);

    let shutdown = CancellationToken::new();

    // Phase 6: license gate. load_and_validate_license is fail-closed —
    // if file missing, signature invalid, or fingerprint mismatched, the
    // flag stays false and only public_routes (including /setup/activate)
    // are reachable. The setup wizard step 0 is /setup/activate.
    let license_valid = std::sync::Arc::new(
        std::sync::atomic::AtomicBool::new(false)
    );
    if license::service::load_and_validate_license(&config.license_jwt_path).await {
        license_valid.store(true, std::sync::atomic::Ordering::Relaxed);
        tracing::info!("license valid — system fully operational");
    } else {
        tracing::warn!("license invalid or missing — system gated, /setup/activate available");
    }

    // Phase 7: purge and backfill worker channels.
    let (purge_tx, purge_rx) = mpsc::unbounded_channel::<cronometrix_api::workers::purge::PurgeRequest>();
    let (backfill_tx, backfill_rx) = mpsc::unbounded_channel::<cronometrix_api::workers::backfill::BackfillRequest>();

    let state = AppState {
        db: Arc::new(db),
        config: Arc::new(config.clone()),
        lifecycle_tx: Some(lifecycle_tx),
        recompute_tx: Some(recompute_tx),
        event_broadcast: Some(event_tx),
        license_valid: license_valid.clone(),
        purge_tx: Some(purge_tx),
        backfill_tx: Some(backfill_tx),
        captures: cronometrix_api::enrollments::handlers::new_captures_map(),
    };

    // Start the supervisor: one tokio task per active device for alertStream
    // consumption. Reconcile loop watches the lifecycle channel.
    let supervisor = Supervisor::new(state.clone(), shutdown.clone());
    let supervisor_handle = tokio::spawn(async move {
        supervisor.run(lifecycle_rx).await;
    });

    // Start the watchdog: sweeps stale devices -> offline every 10s.
    let watchdog_handle = tokio::spawn({
        let s = state.clone();
        let c = shutdown.clone();
        async move {
            watchdog::watchdog_task(s, c).await;
        }
    });

    // Start the Phase 3 recompute worker (mpsc + 500ms debounce + HashSet dedup).
    let recompute_worker = recompute::worker::RecomputeWorker::new(state.clone(), shutdown.clone());
    let recompute_handle = tokio::spawn(async move {
        recompute_worker.run(recompute_rx).await;
    });

    // Phase 7: spawn PurgeWorker (D-15) and BackfillWorker (D-16).
    let purge_worker = PurgeWorker::new(state.clone(), shutdown.clone());
    let purge_handle = tokio::spawn(async move {
        purge_worker.run(purge_rx).await;
    });

    let backfill_worker = BackfillWorker::new(state.clone(), shutdown.clone());
    let backfill_handle = tokio::spawn(async move {
        backfill_worker.run(backfill_rx).await;
    });

    // Start the nightly reconcile task (tokio::time::sleep to next 02:00 local).
    let nightly_handle = tokio::spawn({
        let s = state.clone();
        let c = shutdown.clone();
        let tz = state.config.timezone;
        async move {
            recompute::nightly::nightly_reconcile_task(s, tz, c).await;
        }
    });

    // License renewal: silent best-effort, runs every 24h, only when within
    // 30 days of expiry AND DO Functions URL configured. Failures are logged,
    // never fatal — system stays licensed via cached JWT (DEPL-04, D-09).
    let renewal_handle = tokio::spawn({
        let path = state.config.license_jwt_path.clone();
        let url = state.config.do_functions_renew_url.clone();
        let lv = state.license_valid.clone();
        let c = shutdown.clone();
        async move {
            license::service::renewal_task(path, url, lv, c).await;
        }
    });

    // Public routes — no auth required
    let public_routes = Router::new()
        .route("/health", get(health))
        .route("/auth/login", post(auth::handlers::login))
        .route("/setup/status", get(setup::handlers::setup_status))
        .route("/setup/init", post(setup::handlers::setup_init))
        .route("/setup/activate", post(setup::handlers::setup_activate))
        // SSE stream: EventSource cannot send Bearer headers (T-4-02), so auth is
        // handled inside the handler via ?token=<jwt> query param.
        .route("/events/stream", get(events::handlers::events_stream));

    // Cookie-authenticated routes (refresh/logout validate via refresh cookie, not Bearer)
    // License gate is applied here too: an unlicensed install must not refresh sessions.
    let cookie_auth_routes = Router::new()
        .route("/auth/refresh", post(auth::handlers::refresh))
        .route("/auth/logout", post(auth::handlers::logout))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            license::middleware::require_license,
        ));

    // Read-only routes: any authenticated role can access (Viewer can read per D-09)
    let viewer_routes = Router::new()
        .route("/employees", get(employees::handlers::list_employees))
        .route("/employees/{id}", get(employees::handlers::get_employee))
        .route("/departments", get(departments::handlers::list_departments))
        .route("/departments/{id}", get(departments::handlers::get_department))
        .route("/rules", get(rules::handlers::get_rules))
        .route("/devices", get(devices::handlers::list_devices))
        .route("/devices/{id}", get(devices::handlers::get_device))
        .route("/events", get(events::handlers::list_events))
        .route("/events/{id}", get(events::handlers::get_event))
        .route("/events/{id}/photo", get(events::handlers::get_event_photo))
        .route("/daily-records", get(daily_records::handlers::list_daily_records))
        .route("/daily-records/{id}", get(daily_records::handlers::get_daily_record))
        .route("/leaves", get(leaves::handlers::list_leaves))
        .route("/leaves/{id}", get(leaves::handlers::get_leave))
        .route("/leaves/{id}/evidence", get(leaves::handlers::get_leave_evidence))
        .route("/tenant-info", get(tenant_info::handlers::get_tenant_info))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            license::middleware::require_license,
        ));

    // Supervisor-or-above read routes: supervisor queue for anomalies (T-3-04).
    let supervisor_read_routes = Router::new()
        .route("/anomalies", get(anomalies::handlers::list_anomalies))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_supervisor_or_above,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            license::middleware::require_license,
        ));

    // Supervisor+ routes: create/edit employees
    let supervisor_routes = Router::new()
        .route("/employees", post(employees::handlers::create_employee))
        .route("/employees/{id}", patch(employees::handlers::update_employee))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_supervisor_or_above,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            license::middleware::require_license,
        ));

    // Report routes: Admin + Supervisor only (D-20). Wrapped with a 60s timeout
    // (D-25, T-05-10) to bound aggregation latency for very large periods. The
    // 60s budget is empirically generous — 1000-employee monthly reports run
    // under 5s with rust_xlsxwriter.
    let report_routes = Router::new()
        .route("/reports/json", post(reports::handlers::generate_json))
        .route("/reports/excel", post(reports::handlers::generate_excel))
        .route_layer(tower_http::timeout::TimeoutLayer::new(std::time::Duration::from_secs(60)))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_supervisor_or_above,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            license::middleware::require_license,
        ));

    // Phase 7: enrollment routes (admin-only, D-18). Wrapped with a 3 MB body
    // limit (RequestBodyLimitLayer) to bound multipart photo uploads (T-7-03).
    // capture_from_device and get_capture are read/trigger paths with small bodies
    // and are included in the same limit for simplicity.
    let enrollment_routes = Router::new()
        .route(
            "/enrollments",
            post(enrollments::handlers::create_enrollment),
        )
        .route(
            "/enrollments/{id}",
            get(enrollments::handlers::get_enrollment),
        )
        .route(
            "/enrollments/{enrollment_id}/pushes/{device_id}/retry",
            post(enrollments::handlers::retry_push),
        )
        .route(
            "/enrollments/captures",
            post(enrollments::handlers::capture_from_device),
        )
        .route(
            "/enrollments/captures/{capture_id}",
            get(enrollments::handlers::get_capture),
        )
        .route_layer(tower_http::limit::RequestBodyLimitLayer::new(3 * 1024 * 1024))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_admin,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            license::middleware::require_license,
        ));

    // Admin-only routes: delete employees, manage departments and rules, manage devices + command dispatch
    let admin_routes = Router::new()
        .route("/employees/{id}", delete(employees::handlers::deactivate_employee))
        .route("/departments", post(departments::handlers::create_department))
        .route("/departments/{id}", patch(departments::handlers::update_department))
        .route("/rules", patch(rules::handlers::update_rules))
        .route("/devices", post(devices::handlers::create_device))
        .route("/devices/{id}", patch(devices::handlers::update_device))
        .route("/devices/{id}", delete(devices::handlers::deactivate_device))
        .route("/devices/{id}/commands", post(devices::handlers::dispatch_command))
        .route("/leaves", post(leaves::handlers::create_leave))
        .route("/leaves/{id}", delete(leaves::handlers::cancel_leave))
        .route("/daily-records/{id}/overrides", post(daily_records::handlers::create_override))
        .route("/tenant-info", patch(tenant_info::handlers::patch_tenant_info))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_admin,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            license::middleware::require_license,
        ));

    let app = Router::new()
        .nest(
            "/api/v1",
            public_routes
                .merge(cookie_auth_routes)
                .merge(viewer_routes)
                .merge(supervisor_read_routes)
                .merge(supervisor_routes)
                .merge(report_routes)
                .merge(admin_routes)
                .merge(enrollment_routes),
        )
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    let addr = format!("{}:{}", config.server_host, config.server_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("Cronometrix API listening on {}", addr);
    axum::serve(listener, app)
        .with_graceful_shutdown({
            let shutdown = shutdown.clone();
            async move {
                let _ = tokio::signal::ctrl_c().await;
                tracing::info!("ctrl_c received, initiating graceful shutdown");
                shutdown.cancel();
            }
        })
        .await?;

    // Await supervisor + watchdog shutdown so all child reqwest streams drain
    // before process exit. Also drain the Phase 3 recompute worker, the
    // nightly reconcile task, and Phase 7 workers so their last ops commit.
    let _ = supervisor_handle.await;
    let _ = watchdog_handle.await;
    let _ = recompute_handle.await;
    let _ = nightly_handle.await;
    let _ = renewal_handle.await;
    let _ = purge_handle.await;
    let _ = backfill_handle.await;
    tracing::info!("shutdown complete");

    Ok(())
}

/// Health check endpoint. Performs a SELECT 1 database connectivity check
/// to verify the database is reachable, not just HTTP liveness.
async fn health(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;

    conn.execute("SELECT 1", ())
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(serde_json::json!({
        "status": "ok",
        "database": "connected"
    })))
}
