use std::sync::Arc;

use axum::{extract::State, routing::{get, post}, Json, Router};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use cronometrix_api::auth;
use cronometrix_api::config::Config;
use cronometrix_api::errors::AppError;
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

    let app = Router::new()
        .nest("/api/v1", public_routes.merge(cookie_auth_routes))
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
