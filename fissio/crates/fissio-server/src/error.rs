//! Application error types and Axum response conversion.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// Application-level errors with HTTP status code mapping.
#[derive(Debug)]
pub enum AppError {
    Internal(String),
}

impl AppError {
    /// Creates an Internal error from any error type.
    pub fn internal(e: impl std::fmt::Display) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl From<fissio_core::AgentError> for AppError {
    fn from(e: fissio_core::AgentError) -> Self {
        AppError::Internal(e.to_string())
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let AppError::Internal(message) = self;
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: message })).into_response()
    }
}
