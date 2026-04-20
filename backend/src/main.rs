use std::sync::Arc;

use axum::{
    extract::State,
    routing::{delete, get, patch, post},
    Json, Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::departments;
use cronometrix_api::devices;
use cronometrix_api::employees;
use cronometrix_api::errors::AppError;
use cronometrix_api::events;
use cronometrix_api::rules;
use cronometrix_api::setup;
use cronometrix_api::state::AppState;
use cronometrix_api::db;

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

    let state = AppState {
        db: Arc::new(db),
        config: Arc::new(config.clone()),
    };

    // Public routes — no auth required
    let public_routes = Router::new()
        .route("/health", get(health))
        .route("/auth/login", post(auth::handlers::login))
        .route("/setup/status", get(setup::handlers::setup_status))
        .route("/setup/init", post(setup::handlers::setup_init));

    // Cookie-authenticated routes (refresh/logout validate via refresh cookie, not Bearer)
    let cookie_auth_routes = Router::new()
        .route("/auth/refresh", post(auth::handlers::refresh))
        .route("/auth/logout", post(auth::handlers::logout));

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
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::middleware::require_auth,
        ));

    // Supervisor+ routes: create/edit employees
    let supervisor_routes = Router::new()
        .route("/employees", post(employees::handlers::create_employee))
        .route("/employees/{id}", patch(employees::handlers::update_employee))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_supervisor_or_above,
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
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::rbac::require_admin,
        ));

    let app = Router::new()
        .nest(
            "/api/v1",
            public_routes
                .merge(cookie_auth_routes)
                .merge(viewer_routes)
                .merge(supervisor_routes)
                .merge(admin_routes),
        )
        .with_state(state)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive());

    let addr = format!("{}:{}", config.server_host, config.server_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("Cronometrix API listening on {}", addr);
    axum::serve(listener, app).await?;

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
