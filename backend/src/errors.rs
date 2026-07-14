use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

use crate::db::write_queue::DbWriteError;

/// Application error type. Converts to structured JSON HTTP responses per D-11.
///
/// Response body format:
/// ```json
/// {"error": {"code": "ERROR_CODE", "message": "Human readable", "status": 404}}
/// ```
#[derive(Error, Debug)]
pub enum AppError {
    #[error("not found")]
    NotFound { code: &'static str, message: String },

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("system not licensed")]
    Unlicensed,

    #[error("conflict")]
    Conflict { code: &'static str, message: String },

    #[error("validation failed")]
    Validation { code: &'static str, message: String },

    #[error("gateway timeout")]
    Timeout { code: &'static str, message: String },

    #[error("bad gateway")]
    BadGateway { code: &'static str, message: String },

    #[error("calculation failed")]
    CalcError { code: &'static str, message: String },

    #[error("leave conflict")]
    LeaveConflict { code: &'static str, message: String },

    #[error(transparent)]
    DbWrite(DbWriteError),

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::NotFound { code, message } => (StatusCode::NOT_FOUND, *code, message.clone()),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "UNAUTHORIZED",
                "Authentication required".to_string(),
            ),
            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "Insufficient permissions".to_string(),
            ),
            AppError::Unlicensed => (
                StatusCode::FORBIDDEN,
                "UNLICENSED",
                "License required".to_string(),
            ),
            AppError::Conflict { code, message } => (StatusCode::CONFLICT, *code, message.clone()),
            AppError::Validation { code, message } => {
                (StatusCode::UNPROCESSABLE_ENTITY, *code, message.clone())
            }
            AppError::Timeout { code, message } => {
                (StatusCode::GATEWAY_TIMEOUT, *code, message.clone())
            }
            AppError::BadGateway { code, message } => {
                (StatusCode::BAD_GATEWAY, *code, message.clone())
            }
            AppError::CalcError { code, message } => {
                (StatusCode::INTERNAL_SERVER_ERROR, *code, message.clone())
            }
            AppError::LeaveConflict { code, message } => {
                (StatusCode::CONFLICT, *code, message.clone())
            }
            AppError::DbWrite(DbWriteError::Busy) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "DB_WRITE_QUEUE_BUSY",
                "Database write queue is busy".to_string(),
            ),
            AppError::DbWrite(DbWriteError::Closed | DbWriteError::WorkerStopped) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "DB_WRITE_QUEUE_UNAVAILABLE",
                "Database write queue is unavailable".to_string(),
            ),
            AppError::DbWrite(DbWriteError::Job(error)) => {
                tracing::error!("Database write job failed: {:?}", error);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "An unexpected error occurred".to_string(),
                )
            }
            AppError::Internal(e) => {
                // Log the internal error but don't expose details to clients
                tracing::error!("Internal server error: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "INTERNAL_ERROR",
                    "An unexpected error occurred".to_string(),
                )
            }
        };

        let body = Json(json!({
            "error": {
                "code": code,
                "message": message,
                "status": status.as_u16()
            }
        }));

        (status, body).into_response()
    }
}

impl From<DbWriteError> for AppError {
    fn from(error: DbWriteError) -> Self {
        match error {
            DbWriteError::Job(error) => match error.downcast::<AppError>() {
                Ok(domain_error) => domain_error,
                Err(error) => AppError::DbWrite(DbWriteError::Job(error)),
            },
            error => AppError::DbWrite(error),
        }
    }
}
