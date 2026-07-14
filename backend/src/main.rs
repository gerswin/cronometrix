use std::sync::Arc;

use anyhow::Context as _;
use axum::{
    extract::State,
    routing::{delete, get, patch, post},
    Json, Router,
};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{AllowOrigin, CorsLayer};

use cronometrix_api::anomalies;
use cronometrix_api::audit;
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
use cronometrix_api::http_trace;
use cronometrix_api::leaves;
use cronometrix_api::license;
use cronometrix_api::recompute::{self, RecomputeRequest};
use cronometrix_api::reports;
use cronometrix_api::rules;
use cronometrix_api::setup;
use cronometrix_api::state::{AppState, AttendanceEventSSEPayload};
use cronometrix_api::supervisor::{watchdog, Supervisor};
use cronometrix_api::tenant_info;
use cronometrix_api::users;
use cronometrix_api::workers::{
    backfill::BackfillWorker, capture_cleanup, db_write, purge::PurgeWorker,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env before anything else (ok if file doesn't exist)
    dotenvy::dotenv().ok();

    // Initialize tracing — pretty format for development
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
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

    // Bounded single-writer channel for SQLite/libSQL mutations.
    let (db_write, db_write_rx) =
        cronometrix_api::db::write_queue::DbWriteQueue::channel(Default::default());

    // Event broadcast: attendance service -> SSE stream clients. Buffer 256 events;
    // lagged subscribers (slow clients) simply drop missed events — non-fatal for a
    // live activity feed.
    let (event_tx, _) = broadcast::channel::<AttendanceEventSSEPayload>(256);

    let shutdown = CancellationToken::new();
    // Recompute remains alive after producer cancellation so accepted database
    // callbacks can publish their final work before its own drain begins.
    let recompute_shutdown = CancellationToken::new();
    // Capture cleanup remains alive until every tracked capture task is joined
    // and its final compensation pass has completed.
    let capture_cleanup_shutdown = CancellationToken::new();

    // Phase 6: license gate. load_and_validate_license is fail-closed —
    // if file missing, signature invalid, or fingerprint mismatched, the
    // flag stays false and only public_routes (including /setup/activate)
    // are reachable. The setup wizard step 0 is /setup/activate.
    //
    // Phase 9 D-13: evaluate_bypass runs FIRST. If CRONOMETRIX_LICENSE_BYPASS
    // is set without CRONOMETRIX_E2E the binary aborts immediately with exit code 2.
    // This prevents the test-only flag from leaking into a production deploy and
    // silently disabling the hardware-bound license gate (LIC-05).
    // The exit code 2 contract is locked by backend/tests/license_bypass_safety.rs.
    let license_valid = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let e2e = std::env::var("CRONOMETRIX_E2E").as_deref() == Ok("true");
    let test_reset_enabled =
        e2e && std::env::var("CRONOMETRIX_TEST_RESET_ENABLED").as_deref() == Ok("true");
    let bypass = std::env::var("CRONOMETRIX_LICENSE_BYPASS").as_deref() == Ok("true");
    match license::service::evaluate_bypass(e2e, bypass) {
        license::service::BypassDecision::AbortMisconfigured => {
            tracing::error!(
                "FATAL: CRONOMETRIX_LICENSE_BYPASS set without CRONOMETRIX_E2E. \
                 Refusing to start — bypass flag is test-only and must not appear in production env."
            );
            eprintln!("FATAL: CRONOMETRIX_LICENSE_BYPASS set without CRONOMETRIX_E2E. Aborting.");
            std::process::exit(2);
        }
        license::service::BypassDecision::AllowBypass => {
            license_valid.store(true, std::sync::atomic::Ordering::Relaxed);
            tracing::warn!(
                "license bypass active (CRONOMETRIX_E2E=true, CRONOMETRIX_LICENSE_BYPASS=true) — TEST/DEV ONLY"
            );
        }
        license::service::BypassDecision::NormalPath => {
            if license::service::load_and_validate_license(&config.license_jwt_path).await {
                license_valid.store(true, std::sync::atomic::Ordering::Relaxed);
                tracing::info!("license valid — system fully operational");
            } else {
                tracing::warn!(
                    "license invalid or missing — system gated, /setup/activate available"
                );
            }
        }
    }

    // Phase 7: purge and backfill worker channels.
    let (purge_tx, purge_rx) =
        mpsc::unbounded_channel::<cronometrix_api::workers::purge::PurgeRequest>();
    let (backfill_tx, backfill_rx) =
        mpsc::unbounded_channel::<cronometrix_api::workers::backfill::BackfillRequest>();

    let paths = Arc::new(cronometrix_api::state::Paths::from_env());

    let state = AppState {
        db: Arc::new(db),
        config: Arc::new(config.clone()),
        paths,
        db_write,
        lifecycle_tx: Some(lifecycle_tx),
        recompute_tx: Some(recompute_tx),
        event_broadcast: Some(event_tx),
        license_valid: license_valid.clone(),
        purge_tx: Some(purge_tx),
        backfill_tx: Some(backfill_tx),
        captures: cronometrix_api::enrollments::handlers::new_captures_map(),
        enrollment_tasks: cronometrix_api::enrollments::pusher::EnrollmentTaskTracker::new(),
        enrollment_dispatcher: cronometrix_api::enrollments::dispatcher::EnrollmentDispatcher::new(
        ),
        e2e_enabled: e2e,
        test_reset_enabled,
    };

    // The single writer must be alive for every bootstrap reconciliation.
    // Nothing that can produce a device call starts until these scans finish.
    let db_write_handle = tokio::spawn({
        let db = state.db.clone();
        async move { db_write::run(db, db_write_rx).await }
    });

    // Crash recovery is a startup prerequisite: fail before HTTP/workers if
    // capture orphan inspection cannot complete. The periodic owner only
    // expires live in-memory sessions and never re-runs this bootstrap sweep.
    let bootstrap_result = async {
        // `run_write_worker` opens and configures its own SQLite connection
        // before it receives commands. Waiting on a FIFO barrier prevents the
        // recovery readers below from racing those connection-local PRAGMAs
        // immediately after the migration connection is released.
        state
            .db_write
            .flush()
            .await
            .map_err(|error| anyhow::anyhow!("initialize database writer: {error}"))?;
        capture_cleanup::startup_sweep(&state, capture_cleanup::CleanupNow::now()).await?;
        cronometrix_api::enrollments::dispatcher::recover_startup_checkpoints(&state).await?;
        anyhow::Ok(())
    }
    .await;
    if let Err(error) = bootstrap_result {
        let _ = state.db_write.close_and_flush().await;
        let _ = db_write_handle.await;
        return Err(error);
    }
    let enrollment_dispatcher_handle = state.enrollment_dispatcher.start(state.clone()).await?;

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
    let recompute_worker =
        recompute::worker::RecomputeWorker::new(state.clone(), recompute_shutdown.clone());
    let recompute_handle = tokio::spawn(async move {
        recompute_worker.run(recompute_rx).await;
    });

    let capture_cleanup_handle = tokio::spawn(capture_cleanup::run(
        state.clone(),
        capture_cleanup_shutdown.clone(),
    ));

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
        .route(
            "/departments/{id}",
            get(departments::handlers::get_department),
        )
        .route("/rules", get(rules::handlers::get_rules))
        .route("/devices", get(devices::handlers::list_devices))
        .route("/devices/{id}", get(devices::handlers::get_device))
        .route("/events", get(events::handlers::list_events))
        .route("/events/{id}", get(events::handlers::get_event))
        .route("/events/{id}/photo", get(events::handlers::get_event_photo))
        .route(
            "/daily-records",
            get(daily_records::handlers::list_daily_records),
        )
        .route(
            "/daily-records/{id}",
            get(daily_records::handlers::get_daily_record),
        )
        .route("/leaves", get(leaves::handlers::list_leaves))
        .route("/leaves/{id}", get(leaves::handlers::get_leave))
        .route(
            "/leaves/{id}/evidence",
            get(leaves::handlers::get_leave_evidence),
        )
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
        .route("/audit", get(audit::handlers::list_audit)) // NEW Plan 09-04
        .route("/audit/actors", get(audit::handlers::list_actors))
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
        .route(
            "/employees/{id}",
            patch(employees::handlers::update_employee),
        )
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
        .route_layer(tower_http::timeout::TimeoutLayer::with_status_code(
            axum::http::StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(60),
        ))
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
    // Capture start/poll are read/trigger paths with small bodies
    // and are included in the same limit for simplicity.
    let enrollment_routes = Router::new()
        .route(
            "/enrollments",
            get(enrollments::handlers::list_enrollments)
                .post(enrollments::handlers::create_enrollment),
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
        // Fail closed for the obsolete capture URL. Without this narrow
        // tombstone Axum matches `{id}` and returns 405 instead of 404.
        .route(
            "/enrollments/capture-from-device",
            post(|| async { axum::http::StatusCode::NOT_FOUND }),
        )
        .route_layer(tower_http::limit::RequestBodyLimitLayer::new(
            3 * 1024 * 1024,
        ))
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
        .route(
            "/employees/{id}",
            delete(employees::handlers::deactivate_employee),
        )
        .route(
            "/departments",
            post(departments::handlers::create_department),
        )
        .route(
            "/departments/{id}",
            patch(departments::handlers::update_department),
        )
        .route("/rules", patch(rules::handlers::update_rules))
        .route("/devices", post(devices::handlers::create_device))
        .route("/devices/{id}", patch(devices::handlers::update_device))
        .route(
            "/devices/{id}",
            delete(devices::handlers::deactivate_device),
        )
        .route(
            "/devices/{id}/commands",
            post(devices::handlers::dispatch_command),
        )
        .route("/leaves", post(leaves::handlers::create_leave))
        .route("/leaves/{id}", delete(leaves::handlers::cancel_leave))
        .route(
            "/daily-records/{id}/overrides",
            post(daily_records::handlers::create_override),
        )
        .route(
            "/tenant-info",
            patch(tenant_info::handlers::patch_tenant_info),
        )
        .route("/users", post(users::handlers::create_user))
        .route("/users", get(users::handlers::list_users))
        .route("/users/{id}", get(users::handlers::get_user))
        .route("/users/{id}", patch(users::handlers::update_user))
        .route("/users/{id}", delete(users::handlers::deactivate_user))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_admin,
        ))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            license::middleware::require_license,
        ));

    // Phase 9: __test_reset route — gated at registration time by two distinct
    // startup-captured capabilities. The handler re-checks both as defense in
    // depth. Without both flags the route does not exist, so clients receive
    // 404 from the router. Locked by backend/tests/test_reset_gating.rs (T-09-02).
    let mut api_v1 = public_routes
        .merge(cookie_auth_routes)
        .merge(viewer_routes)
        .merge(supervisor_read_routes)
        .merge(supervisor_routes)
        .merge(report_routes)
        .merge(admin_routes)
        .merge(enrollment_routes);

    if state.e2e_enabled && state.test_reset_enabled {
        tracing::warn!(
            "registering /__test_reset route — E2E + test-reset capabilities enabled; \
             this route MUST NOT be reachable in production"
        );
        api_v1 = api_v1.merge(Router::new().route(
            "/__test_reset",
            post(cronometrix_api::test_reset::test_reset),
        ));
    }

    let cors = build_cors_layer(&config.cors_allowed_origins);

    let app = Router::new()
        .nest("/api/v1", api_v1)
        .with_state(state.clone())
        .layer(http_trace::layer())
        .layer(cors);

    let addr = format!("{}:{}", config.server_host, config.server_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("Cronometrix API listening on {}", addr);
    axum::serve(listener, app)
        .with_graceful_shutdown({
            let shutdown = shutdown.clone();
            async move {
                match cronometrix_api::workers::shutdown_signal().await {
                    Ok(source) => tracing::info!(?source, "shutdown signal received"),
                    Err(error) => tracing::error!(%error, "failed to install shutdown signal"),
                }
                shutdown.cancel();
            }
        })
        .await?;

    fn remember_shutdown_error(
        first: &mut Option<anyhow::Error>,
        result: anyhow::Result<()>,
        phase: &'static str,
    ) {
        if let Err(error) = result.context(phase) {
            tracing::error!(%error, phase, "shutdown phase failed");
            if first.is_none() {
                *first = Some(error);
            }
        }
    }

    let mut first_shutdown_error = None;
    // Stop every producer before the two-stage writer/recompute drain. A panic
    // in one owner is recorded but never prevents the remaining owners from
    // draining.
    remember_shutdown_error(
        &mut first_shutdown_error,
        supervisor_handle.await.map_err(anyhow::Error::from),
        "join supervisor",
    );
    remember_shutdown_error(
        &mut first_shutdown_error,
        watchdog_handle.await.map_err(anyhow::Error::from),
        "join watchdog",
    );
    remember_shutdown_error(
        &mut first_shutdown_error,
        nightly_handle.await.map_err(anyhow::Error::from),
        "join nightly reconcile",
    );
    remember_shutdown_error(
        &mut first_shutdown_error,
        renewal_handle.await.map_err(anyhow::Error::from),
        "join license renewal",
    );
    remember_shutdown_error(
        &mut first_shutdown_error,
        purge_handle.await.map_err(anyhow::Error::from),
        "join purge worker",
    );
    remember_shutdown_error(
        &mut first_shutdown_error,
        backfill_handle.await.map_err(anyhow::Error::from),
        "join backfill worker",
    );

    // No new HTTP requests or worker retries can be admitted now. Await each
    // accepted enrollment operation through its device result and terminal DB
    // write before capture/recompute and the single writer are drained.
    remember_shutdown_error(
        &mut first_shutdown_error,
        state.db_write.flush().await.map_err(anyhow::Error::from),
        "flush accepted writes before enrollment drain",
    );
    remember_shutdown_error(
        &mut first_shutdown_error,
        state.enrollment_dispatcher.close(),
        "close enrollment dispatcher",
    );
    let dispatcher_result = match enrollment_dispatcher_handle.await {
        Ok(result) => result,
        Err(error) => Err(error.into()),
    };
    remember_shutdown_error(
        &mut first_shutdown_error,
        dispatcher_result,
        "drain enrollment dispatcher",
    );

    // The HTTP server is no longer accepting requests. Stop and await all
    // capture tasks, remove their state/JPEGs, then stop the periodic owner.
    // SIGKILL bypasses this block; the next startup orphan sweep recovers it.
    let capture_shutdown_result = capture_cleanup::shutdown_captures(&state).await;
    capture_cleanup_shutdown.cancel();
    let capture_cleanup_result = match capture_cleanup_handle.await {
        Ok(result) => result,
        Err(error) => Err(error.into()),
    };

    let recompute_result = recompute::worker::shutdown_after_producers(
        state.db_write.clone(),
        recompute_shutdown,
        recompute_handle,
        db_write_handle,
    )
    .await;
    remember_shutdown_error(
        &mut first_shutdown_error,
        capture_shutdown_result,
        "shutdown capture tasks",
    );
    remember_shutdown_error(
        &mut first_shutdown_error,
        capture_cleanup_result,
        "shutdown capture cleanup owner",
    );
    remember_shutdown_error(
        &mut first_shutdown_error,
        recompute_result,
        "shutdown recompute and database writer",
    );
    if let Some(error) = first_shutdown_error {
        return Err(error);
    }
    tracing::info!("shutdown complete");

    Ok(())
}

/// Build the CORS layer. Frontend uses an httpOnly refresh cookie, so axios
/// sends `withCredentials: true`; browsers refuse `Access-Control-Allow-Origin: *`
/// in that combo. We must echo a specific allow-listed origin AND set
/// `Access-Control-Allow-Credentials: true`.
///
/// Origins come from `CORS_ALLOWED_ORIGINS` env var (comma-separated). If the
/// list is empty, no cross-origin requests are accepted (locked-down default).
fn build_cors_layer(origins: &[String]) -> CorsLayer {
    use axum::http::{
        header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE},
        HeaderValue, Method,
    };

    let parsed: Vec<HeaderValue> = origins
        .iter()
        .filter_map(|o| o.parse::<HeaderValue>().ok())
        .collect();

    if parsed.is_empty() {
        tracing::warn!("CORS_ALLOWED_ORIGINS is empty — no cross-origin requests will be accepted");
    } else {
        tracing::info!(
            "CORS allow-list ({} origin(s)): {:?}",
            parsed.len(),
            origins
        );
    }

    CorsLayer::new()
        .allow_origin(AllowOrigin::list(parsed))
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([AUTHORIZATION, CONTENT_TYPE, ACCEPT])
}

/// Health check endpoint. Performs a SELECT 1 database connectivity check
/// to verify the database is reachable, not just HTTP liveness.
async fn health(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut rows = conn
        .query("SELECT 1", ())
        .await
        .map_err(|e| AppError::Internal(e.into()))?;
    rows.next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(Json(serde_json::json!({
        "status": "ok",
        "database": "connected"
    })))
}
