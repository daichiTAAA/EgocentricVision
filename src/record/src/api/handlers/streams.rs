use axum::{
    extract::State,
    Json,
};
use std::sync::Arc;
use tracing::info;
use crate::app::AppState;
use crate::error::RecordError;
use crate::models::{
    ConnectRequest, ConnectResponse, DisconnectResponse, StreamStatus, DebugStatus,
};

pub async fn connect(
    State(app_state): State<Arc<AppState>>,
    Json(request): Json<ConnectRequest>,
) -> Result<Json<ConnectResponse>, RecordError> {
    info!("Received stream connect request: protocol={}, url={}", request.protocol, request.url);

    // Validate protocol
    if request.protocol != "rtsp" && request.protocol != "webrtc" {
        return Err(RecordError::StreamError(format!("Unsupported protocol: {}", request.protocol)));
    }

    // Attempt to connect
    app_state.stream_manager.connect(request.protocol.clone(), request.url.clone()).await?;

    info!("Successfully initiated connection to stream: {}", request.url);

    Ok(Json(ConnectResponse {
        status: "CONNECTING".to_string(),
        message: format!("Stream connection initiated for protocol: {}", request.protocol),
    }))
}

pub async fn disconnect(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<DisconnectResponse>, RecordError> {
    info!("Received stream disconnect request");

    app_state.stream_manager.disconnect().await?;

    info!("Successfully disconnected from stream");

    Ok(Json(DisconnectResponse {
        status: "DISCONNECTING".to_string(),
        message: "Stream disconnection initiated.".to_string(),
    }))
}

pub async fn status(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<StreamStatus>, RecordError> {
    let state = app_state.stream_manager.get_status().await;
    let status: StreamStatus = (&state).into();
    Ok(Json(status))
}

pub async fn debug_status(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<DebugStatus>, RecordError> {
    info!("Received debug status request");
    let detailed_status = app_state.stream_manager.get_detailed_status().await;
    info!("Debug status: {:?}", detailed_status);
    Ok(Json(detailed_status))
}