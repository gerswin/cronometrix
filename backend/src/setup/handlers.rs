use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
use validator::Validate;

use crate::{auth::service, errors::AppError, state::AppState};

/// POST /api/v1/setup/status
/// Returns {"initialized": true/false} based on whether any user exists.
/// This endpoint is PUBLIC — no auth required.
pub async fn setup_status(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let conn = state
        .db
        .connect()
        .map_err(|e| AppError::Internal(e.into()))?;

    let mut rows = conn
        .query("SELECT COUNT(*) FROM users", ())
        .await
        .map_err(|e| AppError::Internal(e.into()))?;

    let count: i64 = rows
        .next()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .map(|row| row.get::<i64>(0).unwrap_or(0))
        .unwrap_or(0);

    Ok(Json(json!({
        "initialized": count > 0,
        "licensed": state.license_valid.load(std::sync::atomic::Ordering::Relaxed)
    })))
}

/// Request body for POST /api/v1/setup/init
#[derive(Debug, Deserialize, Validate)]
pub struct SetupInitRequest {
    #[validate(length(min = 1, message = "Full name is required"))]
    pub full_name: String,
    #[validate(length(min = 1, message = "Username is required"))]
    pub username: String,
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}

/// POST /api/v1/setup/init
/// Creates the first admin user. Returns 409 if already initialized.
/// Per D-07: setup wizard endpoint blocked after first admin exists.
/// C-09: la comprobación de unicidad y el INSERT ocurren en la misma
/// transacción del escritor serializado. Un `SELECT COUNT(*)` previo por otra
/// conexión NO cierra la carrera: entre lectura y escritura corre el hash
/// Argon2, que abre una ventana de cientos de milisegundos.
///
/// (Argon2 DoS follow-up) `/setup/init` is unauthenticated and this backend
/// has no rate limiting anywhere. Both checks below MUST stay present at the
/// same time — they defend against different things and neither one alone is
/// enough:
///   1. The cheap `SELECT COUNT(*)` fast-path immediately below is a COST
///      GATE ONLY. It rejects an already-initialized system before we ever
///      touch Argon2, so a flood of requests against an initialized install
///      can't turn this route into a hashing grinder that starves the tokio
///      workers (and, transitively, the serialized `db_write` task — see
///      DbWriteQueue::DEFAULT_ENQUEUE_TIMEOUT). It does NOT close the
///      bootstrap race; two concurrent first-time requests can both read
///      count == 0 here.
///   2. The in-transaction `SELECT COUNT(*)` + INSERT inside `db_write.transact`
///      further down is what actually closes that race, because it runs
///      serialized on the single writer. Do NOT remove it and do NOT treat
///      step 1 as a replacement for it.
///
/// Do not "simplify" this down to one check.
///
/// The fast-path read below is routed through `state.db_write.job(...)`
/// (the writer's own connection, which carries `PRAGMA busy_timeout`) rather
/// than a bare `state.db.connect()`. An ad hoc connection has no busy_timeout
/// configured (nothing in this codebase can set one outside the
/// `check_db_write_queue.py` allowlist) and was observed to intermittently
/// fail with "database is locked" when racing the writer mid-transaction —
/// a plain COUNT(*) is microseconds, nowhere near the Argon2 cost the writer
/// must stay clear of, so sharing its connection for this one read is safe.
pub async fn setup_init(
    State(state): State<AppState>,
    Json(body): Json<SetupInitRequest>,
) -> Result<impl IntoResponse, AppError> {
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    // Cost-gate fast path (see doc comment above): reject already-initialized
    // systems on a cheap COUNT before we ever run Argon2. This is intentionally
    // duplicated with the in-transaction check below, not a substitute for it.
    let precheck_count: i64 = state
        .db_write
        .job("setup.count-check", |conn| {
            Box::pin(async move {
                let mut rows = conn.query("SELECT COUNT(*) FROM users", ()).await?;
                let count = rows
                    .next()
                    .await?
                    .map(|row| row.get::<i64>(0).unwrap_or(0))
                    .unwrap_or(0);
                Ok(count)
            })
        })
        .await
        .map_err(AppError::from)?;
    if precheck_count > 0 {
        return Err(AppError::Conflict {
            code: "SETUP_ALREADY_COMPLETE",
            message: "System has already been initialized. An admin user already exists."
                .to_string(),
        });
    }

    // Argon2 es caro a propósito: se calcula fuera de la transacción para no
    // ocupar el escritor serializado. La comprobación de unicidad ocurre dentro.
    // Runs in spawn_blocking so a hash computation can never occupy an async
    // worker thread (which would otherwise starve the db_write task sharing
    // the same runtime — see the DoS follow-up note above).
    let password = body.password.clone();
    let password_hash = tokio::task::spawn_blocking(move || service::hash_password(&password))
        .await
        .map_err(|e| AppError::Internal(e.into()))??;
    let user_id = Uuid::new_v4().to_string();
    let insert_id = user_id.clone();

    state
        .db_write
        .transact("setup.create-admin", move |tx| {
            Box::pin(async move {
                let mut rows = tx.query("SELECT COUNT(*) FROM users", ()).await?;
                let count: i64 = rows
                    .next()
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("COUNT returned no row"))?
                    .get(0)?;
                if count > 0 {
                    return Err(anyhow::Error::new(AppError::Conflict {
                        code: "SETUP_ALREADY_COMPLETE",
                        message:
                            "System has already been initialized. An admin user already exists."
                                .to_string(),
                    }));
                }
                tx.statement(
                    "INSERT INTO users (id, username, full_name, password_hash, role, status, version, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, 'admin', 'active', 1, unixepoch(), unixepoch())",
                    libsql::params![insert_id, body.username, body.full_name, password_hash],
                )
                .await?;
                Ok(())
            })
        })
        .await
        .map_err(AppError::from)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": user_id,
            "message": "Admin user created successfully"
        })),
    ))
}

/// Request body for POST /api/v1/setup/activate
#[derive(Debug, Deserialize, Validate)]
pub struct SetupActivateRequest {
    /// License key in XXXX-XXXX-XXXX-XXXX format (16 alphanumeric + 3 hyphens = 19 chars).
    /// Validator only enforces length here; the explicit split-check below
    /// catches non-alphanumeric segments and missing hyphens without pulling
    /// in a `regex` static (avoids the once_cell::Lazy dance).
    #[validate(length(
        min = 19,
        max = 19,
        message = "License key must be in XXXX-XXXX-XXXX-XXXX format"
    ))]
    pub license_key: String,
}

/// POST /api/v1/setup/activate
/// Public endpoint — first-run activation. Calls DO Functions, persists JWT,
/// flips state.license_valid to true on success.
/// Per LIC-01: this endpoint MUST stay public so unlicensed installations can
/// activate. Per T-06-19: idempotent — second call after success returns 409
/// ALREADY_ACTIVATED so the frontend can redirect through /setup → /login.
pub async fn setup_activate(
    State(state): State<AppState>,
    Json(body): Json<SetupActivateRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Cheap format validation (avoids round-tripping garbage to DO Functions)
    body.validate().map_err(|e| AppError::Validation {
        code: "VALIDATION_ERROR",
        message: e.to_string(),
    })?;

    // Format check: XXXX-XXXX-XXXX-XXXX (4 alphanumeric segments separated by hyphens).
    let key = body.license_key.trim();
    let parts: Vec<&str> = key.split('-').collect();
    if parts.len() != 4
        || parts
            .iter()
            .any(|p| p.len() != 4 || !p.chars().all(|c| c.is_ascii_alphanumeric()))
    {
        return Err(AppError::Validation {
            code: "VALIDATION_ERROR",
            message: "License key must be in XXXX-XXXX-XXXX-XXXX format".to_string(),
        });
    }

    // Idempotent guard (T-06-19): if already activated, return 409 ALREADY_ACTIVATED
    // so the frontend can redirect through /setup → /login.
    if state
        .license_valid
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        return Err(AppError::Conflict {
            code: "ALREADY_ACTIVATED",
            message: "This installation is already activated.".to_string(),
        });
    }

    let _claims = crate::license::service::activate_license(
        &body.license_key,
        &state.config.do_functions_activate_url,
        &state.config.license_jwt_path,
    )
    .await?;

    state
        .license_valid
        .store(true, std::sync::atomic::Ordering::Relaxed);

    Ok((StatusCode::OK, Json(json!({ "activated": true }))))
}
