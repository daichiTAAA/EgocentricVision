use axum::{
    extract::{State, Path},
    Json,
    http::{StatusCode, header::{CONTENT_TYPE, CONTENT_DISPOSITION}},
    response::Response,
    body::Body,
};
use std::sync::Arc;
use std::path::PathBuf;
use uuid::Uuid;
use chrono::Utc;
use tracing::{info, error};
use tokio_util::io::ReaderStream;
use crate::app::AppState;
use crate::error::RecordError;
use crate::models::{
    StartRecordingResponse, StopRecordingResponse, RecordingListItem, RecordingDetails,
};

pub async fn start(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<StartRecordingResponse>, RecordError> {
    info!("Received recording start request");

    // Check if stream is connected
    if !app_state.stream_manager.is_connected().await {
        error!("Failed to start recording: Stream is not connected");
        return Err(RecordError::NotConnected);
    }

    // Check if already recording
    if app_state.stream_manager.is_recording().await {
        error!("Failed to start recording: Already recording");
        return Err(RecordError::AlreadyRecording);
    }

    let recording_id = Uuid::new_v4();
    let start_time = Utc::now();
    let file_name = format!("rec_{}.mp4", start_time.format("%Y%m%d_%H%M%S"));
    let file_path = app_state.config.recording_directory.join(&file_name);

    info!("Creating recording entry in database: id={}, file={}", recording_id, file_path.display());

    // Create recording entry in database
    let _recording = app_state.database.create_recording(
        recording_id,
        file_name,
        file_path.to_string_lossy().to_string(),
        start_time,
    ).await.map_err(|e| {
        error!("Failed to create recording entry in database: {}", e);
        e
    })?;

    info!("Starting recording in stream manager: id={}", recording_id);

    // Start recording in stream manager
    app_state.stream_manager.start_recording(recording_id).await.map_err(|e| {
        error!("Failed to start recording in stream manager: {} (recording_id={})", e, recording_id);
        e
    })?;

    info!("Successfully started recording with ID: {}", recording_id);

    Ok(Json(StartRecordingResponse {
        recording_id,
        status: "RECORDING_STARTED".to_string(),
        message: "Recording has been initiated.".to_string(),
    }))
}

pub async fn stop(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<StopRecordingResponse>, RecordError> {
    info!("Received recording stop request");

    // Stop recording in stream manager
    let recording_id = app_state.stream_manager.stop_recording().await.map_err(|e| {
        error!("Failed to stop recording in stream manager: {}", e);
        e
    })?;

    let end_time = Utc::now();
    
    info!("Getting recording details from database: id={}", recording_id);
    
    // Get the recording to calculate duration and file size
    let recording = app_state.database.get_recording(recording_id).await.map_err(|e| {
        error!("Failed to get recording details from database: {}", e);
        e
    })?;
    
    let duration = (end_time - recording.start_time).num_seconds();
    
    // Get file size
    let file_size = match std::fs::metadata(&recording.file_path) {
        Ok(metadata) => {
            let size = metadata.len() as i64;
            info!("Recording file size: {} bytes", size);
            size
        },
        Err(e) => {
            error!("Failed to get recording file metadata: {} (path={})", e, recording.file_path);
            0 // Default to 0 if file doesn't exist yet
        }
    };

    info!("Updating recording as completed: id={}, duration={}s, size={} bytes", recording_id, duration, file_size);

    // Update recording as completed
    let _updated_recording = app_state.database.update_recording_completed(
        recording_id,
        end_time,
        duration,
        file_size,
    ).await.map_err(|e| {
        error!("Failed to update recording as completed: {} (recording_id={})", e, recording_id);
        e
    })?;

    info!("Successfully stopped recording with ID: {}", recording_id);

    Ok(Json(StopRecordingResponse {
        recording_id,
        status: "RECORDING_STOPPED".to_string(),
        message: "Recording has been stopped and saved.".to_string(),
    }))
}

pub async fn list(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<Vec<RecordingListItem>>, RecordError> {
    let recordings = app_state.database.list_recordings().await?;
    let items: Vec<RecordingListItem> = recordings.into_iter().map(Into::into).collect();
    Ok(Json(items))
}

pub async fn get(
    State(app_state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<RecordingDetails>, RecordError> {
    let recording = app_state.database.get_recording(id).await?;
    Ok(Json(recording.into()))
}

pub async fn download(
    State(app_state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Response<Body>, RecordError> {
    let recording = app_state.database.get_recording(id).await?;
    
    let file_path = PathBuf::from(&recording.file_path);
    if !file_path.exists() {
        return Err(RecordError::RecordingNotFound(format!("File not found for recording {}", id)));
    }

    let file = tokio::fs::File::open(&file_path).await?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let mut response = Response::new(body);
    response.headers_mut().insert(
        CONTENT_TYPE,
        "video/mp4".parse().unwrap(),
    );
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", recording.file_name).parse().unwrap(),
    );

    Ok(response)
}

pub async fn delete(
    State(app_state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, RecordError> {
    let recording = app_state.database.get_recording(id).await?;
    
    // Delete file from filesystem
    let file_path = PathBuf::from(&recording.file_path);
    if file_path.exists() {
        tokio::fs::remove_file(&file_path).await?;
    }

    // Delete from database
    app_state.database.delete_recording(id).await?;

    info!("Successfully deleted recording with ID: {}", id);

    Ok(StatusCode::NO_CONTENT)
}