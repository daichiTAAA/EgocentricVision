use crate::app::AppState;
use crate::error::RecordError;
use crate::models::{
    ConnectRequest, ConnectResponse, DebugStatus, DisconnectResponse, StreamStatus,
};
use crate::stream::StreamId;
use axum::{
    extract::{Path, State},
    Json,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct StartWebRTCQuery {
    // pub signaling_url: String, // 未使用のためコメントアウト
}

pub async fn connect(
    State(app_state): State<Arc<AppState>>,
    Json(request): Json<ConnectRequest>,
) -> Result<Json<ConnectResponse>, RecordError> {
    info!(
        "Received stream connect request: protocol={}, url={}",
        request.protocol, request.url
    );

    // Validate protocol
    if request.protocol != "rtsp" && request.protocol != "webrtc" {
        return Err(RecordError::StreamError(format!(
            "Unsupported protocol: {}",
            request.protocol
        )));
    }

    // Generate stream ID
    let stream_id = Uuid::new_v4().to_string();

    // Attempt to connect
    app_state
        .stream_manager
        .connect(
            stream_id.clone(),
            request.protocol.clone(),
            request.url.clone(),
        )
        .await?;

    info!(
        "Successfully initiated connection to stream: {}",
        request.url
    );

    Ok(Json(ConnectResponse {
        stream_id,
        status: "CONNECTING".to_string(),
        message: format!(
            "Stream connection initiated for protocol: {}",
            request.protocol
        ),
    }))
}

pub async fn disconnect(
    State(app_state): State<Arc<AppState>>,
    Path(stream_id): Path<StreamId>,
) -> Result<Json<DisconnectResponse>, RecordError> {
    info!(
        "Received stream disconnect request for stream: {}",
        stream_id
    );

    app_state.stream_manager.disconnect(&stream_id).await?;

    info!("Successfully disconnected from stream: {}", stream_id);

    Ok(Json(DisconnectResponse {
        status: "DISCONNECTING".to_string(),
        message: format!("Stream disconnection initiated for stream: {}", stream_id),
    }))
}

pub async fn status(
    State(app_state): State<Arc<AppState>>,
    Path(stream_id): Path<StreamId>,
) -> Result<Json<StreamStatus>, RecordError> {
    let state = app_state
        .stream_manager
        .get_status(&stream_id)
        .await
        .ok_or_else(|| RecordError::StreamError(format!("Stream {} not found", stream_id)))?;
    let status: StreamStatus = (&state).into();
    Ok(Json(status))
}

pub async fn list_statuses(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<HashMap<StreamId, StreamStatus>>, RecordError> {
    let statuses = app_state.stream_manager.get_all_statuses().await;
    Ok(Json(statuses))
}

pub async fn debug_status(
    State(app_state): State<Arc<AppState>>,
    Path(stream_id): Path<StreamId>,
) -> Result<Json<DebugStatus>, RecordError> {
    info!("Received debug status request for stream: {}", stream_id);
    let detailed_status = app_state
        .stream_manager
        .get_detailed_status(&stream_id)
        .await
        .ok_or_else(|| RecordError::StreamError(format!("Stream {} not found", stream_id)))?;
    info!(
        "Debug status for stream {}: {:?}",
        stream_id, detailed_status
    );
    Ok(Json(detailed_status))
}
