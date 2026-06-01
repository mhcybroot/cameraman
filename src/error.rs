use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Config(String),
    Io(std::io::Error),
    Multipart(axum::extract::multipart::MultipartError),
    Network(reqwest::Error),
    Serialization(serde_json::Error),
    AiProvider(String),
    InvalidPayload(String),
    Database(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Config(msg) => write!(f, "Configuration error: {}", msg),
            AppError::Io(err) => write!(f, "IO error: {}", err),
            AppError::Multipart(err) => write!(f, "Multipart error: {}", err),
            AppError::Network(err) => write!(f, "Network error: {}", err),
            AppError::Serialization(err) => write!(f, "Serialization error: {}", err),
            AppError::AiProvider(msg) => write!(f, "AI Provider error: {}", msg),
            AppError::InvalidPayload(msg) => write!(f, "Invalid payload error: {}", msg),
            AppError::Database(msg) => write!(f, "Database error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

// Implement conversion from common error types to AppError
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}

impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(err: axum::extract::multipart::MultipartError) -> Self {
        AppError::Multipart(err)
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Network(err)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Serialization(err)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        AppError::Database(err.to_string())
    }
}

// Convert AppError into an Axum response
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::Config(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::Io(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            AppError::Multipart(err) => (StatusCode::BAD_REQUEST, err.to_string()),
            AppError::Network(err) => (StatusCode::BAD_GATEWAY, err.to_string()),
            AppError::Serialization(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
            AppError::AiProvider(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::InvalidPayload(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Database(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(json!({
            "status": "error",
            "message": error_message,
        }));

        (status, body).into_response()
    }
}
