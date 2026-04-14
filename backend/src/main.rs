mod config;
mod db;
mod errors;
mod state;

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{config::Config, errors::AppError, state::AppState};

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

    let app = Router::new()
        .route("/api/v1/health", get(health))
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
