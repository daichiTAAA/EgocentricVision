use crate::stream::StreamId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Recording {
    pub id: Uuid,
    pub file_name: String,
    pub file_path: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i64>,
    pub file_size_bytes: Option<i64>,
    pub status: RecordingStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, Eq)]
#[sqlx(type_name = "recording_status", rename_all = "UPPERCASE")]
pub enum RecordingStatus {
    Recording,
    Completed,
    Failed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectRequest {
    pub protocol: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectResponse {
    pub stream_id: StreamId,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DisconnectResponse {
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StreamStatus {
    pub is_connected: bool,
    pub protocol: Option<String>,
    pub url: Option<String>,
    pub is_recording: bool,
    pub connected_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartRecordingResponse {
    pub recording_id: String,
    pub stream_id: StreamId,
    pub location: String,
    pub message: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StopRecordingResponse {
    pub recording_id: String,
    pub stream_id: StreamId,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordingListItem {
    pub id: Uuid,
    pub file_name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration: Option<i64>,
    pub file_size: Option<i64>,
    pub stream_id: Option<StreamId>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordingDetails {
    pub id: Uuid,
    pub file_name: String,
    pub file_path: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration: Option<i64>,
    pub file_size: Option<i64>,
    pub stream_id: Option<StreamId>,
}

#[derive(Debug, Serialize)]
pub struct DebugStatus {
    pub is_connected: bool,
    pub is_recording: bool,
    pub protocol: Option<String>,
    pub url: Option<String>,
    pub tee_ready: bool,
    pub pipeline_state: Option<String>,
    pub pipeline_pending_state: Option<String>,
    pub tee_state: Option<String>,
    pub tee_pending_state: Option<String>,
    pub active_recording_pads: usize,
}

impl From<Recording> for RecordingListItem {
    fn from(recording: Recording) -> Self {
        Self {
            id: recording.id,
            file_name: recording.file_name,
            start_time: recording.start_time,
            end_time: recording.end_time,
            duration: recording.duration_seconds,
            file_size: recording.file_size_bytes,
            stream_id: None,
        }
    }
}

impl From<Recording> for RecordingDetails {
    fn from(recording: Recording) -> Self {
        Self {
            id: recording.id,
            file_name: recording.file_name,
            file_path: recording.file_path,
            start_time: recording.start_time,
            end_time: recording.end_time,
            duration: recording.duration_seconds,
            file_size: recording.file_size_bytes,
            stream_id: None,
        }
    }
}
