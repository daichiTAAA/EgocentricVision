use thiserror::Error;
use axum::{
    http::StatusCode,
    response::{Response, IntoResponse},
    Json,
};
use serde_json::json;

#[derive(Error, Debug)]
pub enum RecordError {
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    
    #[error("Migration error: {0}")]
    MigrationError(#[from] sqlx::migrate::MigrateError),
    
    #[error("Stream error: {0}")]
    StreamError(String),
    
    #[error("Recording not found: {0}")]
    RecordingNotFound(String),
    
    #[error("Already recording")]
    AlreadyRecording,
    
    #[error("Not connected to stream")]
    NotConnected,
    
    #[error("Pipeline error: {0}")]
    PipelineError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Internal server error: {0}")]
    InternalError(String),
}

impl IntoResponse for RecordError {
    fn into_response(self) -> Response {
        let (status, error_code, message) = match self {
            RecordError::ConfigError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "CONFIG_ERROR",
                msg,
            ),
            RecordError::DatabaseError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "DB_ERROR",
                err.to_string(),
            ),
            RecordError::MigrationError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "MIGRATION_ERROR",
                err.to_string(),
            ),
            RecordError::StreamError(msg) => (
                StatusCode::BAD_REQUEST,
                "STREAM_ERROR",
                msg,
            ),
            RecordError::RecordingNotFound(id) => (
                StatusCode::NOT_FOUND,
                "RESOURCE_NOT_FOUND",
                format!("Recording with ID {} not found", id),
            ),
            RecordError::AlreadyRecording => (
                StatusCode::CONFLICT,
                "ALREADY_RECORDING",
                "Stream is already being recorded".to_string(),
            ),
            RecordError::NotConnected => (
                StatusCode::CONFLICT,
                "NOT_CONNECTED",
                "Not connected to stream".to_string(),
            ),
            RecordError::PipelineError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "PIPELINE_ERROR",
                msg,
            ),
            RecordError::IoError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "IO_ERROR",
                err.to_string(),
            ),
            RecordError::InternalError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_SERVER_ERROR",
                msg,
            ),
        };

        let body = Json(json!({
            "error_code": error_code,
            "message": message
        }));

        (status, body).into_response()
    }
}