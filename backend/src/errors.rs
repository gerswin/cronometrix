use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

/// Application error type. Converts to structured JSON HTTP responses per D-11.
///
/// Response body format:
/// ```json
/// {"error": {"code": "ERROR_CODE", "message": "Human readable", "status": 404}}
/// ```
#[derive(Error, Debug)]
pub enum AppError {
    #[error("not found")]
    NotFound {
        code: &'static str,
        message: String,
    },

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("conflict")]
    Conflict {
        code: &'static str,
        message: String,
    },

    #[error("validation failed")]
    Validation {
        code: &'static str,
        message: String,
    },

    #[error("gateway timeout")]
    Timeout {
        code: &'static str,
        message: String,
    },

    #[error("bad gateway")]
    BadGateway {
        code: &'static str,
        message: String,
    },

    #[error("calculation failed")]
    CalcError {
        code: &'static str,
        message: String,
    },

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::NotFound { code, message } => {
                (StatusCode::NOT_FOUND, *code, message.clone())
            }
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
            AppError::Conflict { code, message } => {
                (StatusCode::CONFLICT, *code, message.clone())
            }
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
