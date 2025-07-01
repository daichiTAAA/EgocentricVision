use crate::app::AppState;
use crate::error::RecordError;
use crate::models::{
    RecordingDetails, RecordingListItem, StartRecordingResponse, StopRecordingResponse,
};
use crate::stream::StreamId;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{
        header::{CONTENT_DISPOSITION, CONTENT_TYPE},
        StatusCode,
    },
    response::Response,
    Json,
};
use chrono::Utc;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::io::ReaderStream;
use tracing::{error, info};
use uuid::Uuid;

pub async fn start(
    State(app_state): State<Arc<AppState>>,
    Path((stream_id,)): Path<(StreamId,)>,
) -> Result<Json<StartRecordingResponse>, RecordError> {
    info!("Starting recording for stream: {}", stream_id);
    let recording_id = Uuid::new_v4().to_string();
    info!("[recording {}] Generated recording ID", recording_id);

    let location = format!("/var/data/recordings/{}.mp4", recording_id);
    info!(
        "[recording {}] Recording file location: {}",
        recording_id, location
    );

    let start_time = Utc::now();
    let file_name = format!("{}.mp4", recording_id);
    // DBに録画情報を登録
    app_state
        .database
        .create_recording(
            Uuid::parse_str(&recording_id).unwrap(),
            file_name.clone(),
            location.clone(),
            start_time,
        )
        .await?;

    let recording_id2 = recording_id.clone();
    let location2 = location.clone();
    let app_state2 = app_state.clone();
    let stream_id2 = stream_id.clone();
    let result = tokio::task::spawn_blocking(AssertUnwindSafe(move || {
        futures::executor::block_on(app_state2.stream_manager.start_recording(
            &stream_id2,
            &recording_id2,
            &location2,
        ))
    }))
    .await;

    match result {
        Ok(Ok(_)) => {
            info!(
                "[recording {}] Successfully started recording for stream: {}",
                recording_id, stream_id
            );
            Ok(Json(StartRecordingResponse {
                recording_id,
                stream_id: stream_id.clone(),
                location,
                message: format!("Recording started successfully for stream: {}", stream_id),
                status: "RECORDING".to_string(),
            }))
        }
        Ok(Err(e)) => {
            error!(
                "[recording {}] Failed to start recording for stream {}: {}",
                recording_id, stream_id, e
            );
            Err(e)
        }
        Err(e) => {
            error!(
                "[recording {}] Panic occurred in start_recording for stream {}: {:?}",
                recording_id, stream_id, e
            );
            Err(RecordError::StreamError(format!(
                "Panic occurred in start_recording: {:?}",
                e
            )))
        }
    }
}

pub async fn stop(
    State(app_state): State<Arc<AppState>>,
    Path(stream_id): Path<String>,
) -> Result<Json<StopRecordingResponse>, RecordError> {
    info!("Received recording stop request for stream: {}", stream_id);
    // Stop recording in stream manager
    let recording_id = app_state
        .stream_manager
        .stop_recording(&stream_id)
        .await
        .map_err(|e| {
            error!(
                "Failed to stop recording in stream manager for stream {}: {}",
                stream_id, e
            );
            e
        })?;
    let end_time = Utc::now();
    info!(
        "Getting recording details from database: id={}",
        recording_id
    );
    let uuid1 =
        Uuid::parse_str(&recording_id).map_err(|e| RecordError::StreamError(e.to_string()))?;
    let recording = app_state.database.get_recording(uuid1).await.map_err(|e| {
        error!("Failed to get recording details from database: {}", e);
        e
    })?;
    let duration = (end_time - recording.start_time).num_seconds();
    let file_size = match std::fs::metadata(&recording.file_path) {
        Ok(metadata) => {
            let size = metadata.len() as i64;
            info!("Recording file size: {} bytes", size);
            size
        }
        Err(e) => {
            error!(
                "Failed to get recording file metadata: {} (path={})",
                e, recording.file_path
            );
            0
        }
    };
    info!(
        "Updating recording as completed: id={}, duration={}s, size={} bytes",
        recording_id, duration, file_size
    );
    let uuid2 =
        Uuid::parse_str(&recording_id).map_err(|e| RecordError::StreamError(e.to_string()))?;
    let _updated_recording = app_state
        .database
        .update_recording_completed(uuid2, end_time, duration, file_size)
        .await
        .map_err(|e| {
            error!(
                "Failed to update recording as completed: {} (recording_id={})",
                e, recording_id
            );
            e
        })?;
    info!(
        "Successfully stopped recording with ID: {} for stream: {}",
        recording_id, stream_id
    );
    Ok(Json(StopRecordingResponse {
        recording_id,
        stream_id: stream_id.clone(),
        status: "RECORDING_STOPPED".to_string(),
        message: format!(
            "Recording has been stopped and saved for stream: {}",
            stream_id
        ),
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
    Path(recording_id): Path<Uuid>,
) -> Result<Json<RecordingDetails>, RecordError> {
    let recording = app_state.database.get_recording(recording_id).await?;
    Ok(Json(recording.into()))
}

pub async fn download(
    State(app_state): State<Arc<AppState>>,
    Path(recording_id): Path<Uuid>,
) -> Result<Response<Body>, RecordError> {
    let recording = app_state.database.get_recording(recording_id).await?;

    let file_path = PathBuf::from(&recording.file_path);
    if !file_path.exists() {
        return Err(RecordError::RecordingNotFound(format!(
            "File not found for recording {}",
            recording_id
        )));
    }

    let file = tokio::fs::File::open(&file_path).await?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let mut response = Response::new(body);
    response
        .headers_mut()
        .insert(CONTENT_TYPE, "video/mp4".parse().unwrap());
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        format!("attachment; filename=\"{}\"", recording.file_name)
            .parse()
            .unwrap(),
    );

    Ok(response)
}

pub async fn delete(
    State(app_state): State<Arc<AppState>>,
    Path(recording_id): Path<Uuid>,
) -> Result<StatusCode, RecordError> {
    let recording = app_state.database.get_recording(recording_id).await?;

    // Delete file from filesystem
    let file_path = PathBuf::from(&recording.file_path);
    if file_path.exists() {
        tokio::fs::remove_file(&file_path).await?;
    }

    // Delete from database
    app_state.database.delete_recording(recording_id).await?;

    info!("Successfully deleted recording with ID: {}", recording_id);

    Ok(StatusCode::NO_CONTENT)
}
