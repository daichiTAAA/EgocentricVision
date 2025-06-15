use axum::{
    extract::State,
    Json,
};
use std::sync::Arc;
use tracing::info;
use crate::app::AppState;
use crate::error::RecordError;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    status: String,
    version: String,
    database_connected: bool,
}

pub async fn health(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<HealthResponse>, RecordError> {
    info!("Health check requested");

    // データベース接続の確認
    let database_connected = app_state.database.is_connected().await;

    Ok(Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        database_connected,
    }))
} 