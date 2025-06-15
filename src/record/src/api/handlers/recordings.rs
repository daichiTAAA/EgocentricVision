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
use std::panic::{AssertUnwindSafe, catch_unwind};
use axum::response::IntoResponse;

pub async fn start(
    State(app_state): State<Arc<AppState>>,
) -> Result<Json<StartRecordingResponse>, RecordError> {
    info!("Starting recording...");
    let recording_id = Uuid::new_v4().to_string();
    info!("[recording {}] Generated recording ID", recording_id);

    let location = format!("/var/data/recordings/{}.mp4", recording_id);
    info!("[recording {}] Recording file location: {}", recording_id, location);

    let start_time = Utc::now();
    let file_name = format!("{}.mp4", recording_id);
    // DBに録画情報を登録
    app_state.database.create_recording(
        Uuid::parse_str(&recording_id).unwrap(),
        file_name.clone(),
        location.clone(),
        start_time,
    ).await?;

    let recording_id2 = recording_id.clone();
    let location2 = location.clone();
    let app_state2 = app_state.clone();
    let result = tokio::task::spawn_blocking(AssertUnwindSafe(move || {
        futures::executor::block_on(app_state2.stream_manager.start_recording(&recording_id2, &location2))
    })).await;

    match result {
        Ok(Ok(_)) => {
            info!("[recording {}] Successfully started recording", recording_id);
            Ok(Json(StartRecordingResponse {
                recording_id,
                location,
                message: "Recording started successfully".to_string(),
                status: "RECORDING".to_string(),
            }))
        },
        Ok(Err(e)) => {
            error!("[recording {}] Failed to start recording: {}", recording_id, e);
            Err(e)
        },
        Err(e) => {
            error!("[recording {}] Panic occurred in start_recording: {:?}", recording_id, e);
            Err(RecordError::StreamError(format!("Panic occurred in start_recording: {:?}", e)))
        }
    }
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
    let uuid1 = Uuid::parse_str(&recording_id).map_err(|e| RecordError::StreamError(e.to_string()))?;
    let recording = app_state.database.get_recording(uuid1).await.map_err(|e| {
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
    let uuid2 = Uuid::parse_str(&recording_id).map_err(|e| RecordError::StreamError(e.to_string()))?;
    let _updated_recording = app_state.database.update_recording_completed(
        uuid2,
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