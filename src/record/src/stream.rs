use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::error::RecordError;
use crate::models::StreamStatus;

#[derive(Debug, Clone)]
pub struct StreamState {
    pub is_connected: bool,
    pub protocol: Option<String>,
    pub url: Option<String>,
    pub is_recording: bool,
    pub connected_at: Option<DateTime<Utc>>,
    pub current_recording_id: Option<Uuid>,
}

impl Default for StreamState {
    fn default() -> Self {
        Self {
            is_connected: false,
            protocol: None,
            url: None,
            is_recording: false,
            connected_at: None,
            current_recording_id: None,
        }
    }
}

impl From<StreamState> for StreamStatus {
    fn from(state: StreamState) -> Self {
        Self {
            is_connected: state.is_connected,
            protocol: state.protocol,
            url: state.url,
            is_recording: state.is_recording,
            connected_at: state.connected_at,
        }
    }
}

pub struct StreamManager {
    state: Arc<Mutex<StreamState>>,
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(StreamState::default())),
        }
    }

    pub async fn get_status(&self) -> StreamStatus {
        let state = self.state.lock().await;
        state.clone().into()
    }

    pub async fn connect(&self, protocol: String, url: String) -> Result<(), RecordError> {
        let mut state = self.state.lock().await;
        
        if state.is_connected {
            return Err(RecordError::StreamError("Already connected to a stream".to_string()));
        }

        // TODO: Implement actual GStreamer pipeline creation
        // For now, we'll just simulate the connection
        state.is_connected = true;
        state.protocol = Some(protocol);
        state.url = Some(url);
        state.connected_at = Some(Utc::now());

        Ok(())
    }

    pub async fn disconnect(&self) -> Result<(), RecordError> {
        let mut state = self.state.lock().await;
        
        if !state.is_connected {
            return Err(RecordError::NotConnected);
        }

        // Stop recording if currently recording
        if state.is_recording {
            state.is_recording = false;
            state.current_recording_id = None;
        }

        // TODO: Implement actual GStreamer pipeline cleanup
        state.is_connected = false;
        state.protocol = None;
        state.url = None;
        state.connected_at = None;

        Ok(())
    }

    pub async fn start_recording(&self, recording_id: Uuid) -> Result<(), RecordError> {
        let mut state = self.state.lock().await;
        
        if !state.is_connected {
            return Err(RecordError::NotConnected);
        }
        
        if state.is_recording {
            return Err(RecordError::AlreadyRecording);
        }

        // TODO: Implement actual GStreamer recording pipeline
        state.is_recording = true;
        state.current_recording_id = Some(recording_id);

        Ok(())
    }

    pub async fn stop_recording(&self) -> Result<Uuid, RecordError> {
        let mut state = self.state.lock().await;
        
        if !state.is_recording {
            return Err(RecordError::StreamError("Not currently recording".to_string()));
        }

        let recording_id = state.current_recording_id
            .ok_or_else(|| RecordError::InternalError("No recording ID found".to_string()))?;

        // TODO: Implement actual GStreamer recording pipeline stop
        state.is_recording = false;
        state.current_recording_id = None;

        Ok(recording_id)
    }

    pub async fn is_connected(&self) -> bool {
        let state = self.state.lock().await;
        state.is_connected
    }

    pub async fn is_recording(&self) -> bool {
        let state = self.state.lock().await;
        state.is_recording
    }
}