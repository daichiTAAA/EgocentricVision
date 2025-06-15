use crate::config::Config;
use crate::error::RecordError;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use gstreamer::prelude::*;
use gstreamer::{
    event, Bin, Element, ElementFactory, GhostPad, PadProbeReturn,
    PadProbeType, Pipeline, State, StateChangeError,
    PadDirection, MessageView,
};
use tracing::{info, warn, error, debug};
use crate::models::StreamStatus;
use glib::BoolError;
use std::collections::HashMap;
use gstreamer_rtsp::prelude::*;
use gstreamer_rtsp::RTSPUrl;
use tokio::sync::mpsc;
use std::time::Duration;
use tokio::time::sleep;
use glib::ControlFlow;

/// Stores the logical state of the stream.
#[derive(Debug, Clone, Default)]
pub struct StreamState {
    pub is_connected: bool,
    pub is_recording: bool,
    pub protocol: Option<String>,
    pub url: Option<String>,
    pub current_recording_id: Option<Uuid>,
    pub is_tee_ready: bool,
}

/// Manages the GStreamer pipeline and stream state.
pub struct StreamManager {
    state: Arc<Mutex<StreamState>>,
    pipeline: Arc<Mutex<Option<Pipeline>>>,
    tee: Arc<Mutex<Option<Element>>>,
    config: Config,
    recording_pads: Arc<Mutex<HashMap<Uuid, gstreamer::Pad>>>,
    is_tee_ready: Arc<AtomicBool>,
}

impl StreamManager {
    /// Creates a new StreamManager instance and initializes GStreamer.
    pub fn new(config: Config) -> Self {
        if let Err(e) = gstreamer::init() {
            panic!("Failed to initialize GStreamer: {}", e);
        }
        Self {
            state: Arc::new(Mutex::new(StreamState::default())),
            pipeline: Arc::new(Mutex::new(None)),
            tee: Arc::new(Mutex::new(None)),
            config,
            recording_pads: Arc::new(Mutex::new(HashMap::new())),
            is_tee_ready: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns the current stream status.
    pub async fn get_status(&self) -> StreamState {
        self.state.lock().await.clone()
    }

    /// Returns the stream connection status
    pub async fn is_connected(&self) -> bool {
        self.state.lock().await.is_connected
    }
    /// Returns the recording status
    pub async fn is_recording(&self) -> bool {
        self.state.lock().await.is_recording
    }

    /// Returns detailed status of the pipeline and Tee (for debugging)
    pub async fn get_detailed_status(&self) -> String {
        let state = self.state.lock().await;
        let pipeline_lock = self.pipeline.lock().await;
        let tee_lock = self.tee.lock().await;
        
        let mut status = format!(
            "StreamManager Status:\n  Connected: {}\n  Recording: {}\n  Protocol: {:?}\n  URL: {:?}\n",
            state.is_connected,
            state.is_recording,
            state.protocol,
            state.url
        );
        
        status.push_str(&format!("  Tee Ready: {}\n", self.is_tee_ready.load(Ordering::SeqCst)));
        
        if let Some(pipeline) = pipeline_lock.as_ref() {
            let (_, current_state, pending_state) = pipeline.state(gstreamer::ClockTime::ZERO);
            status.push_str(&format!("  Pipeline State: {:?} (pending: {:?})\n", current_state, pending_state));
        } else {
            status.push_str("  Pipeline: None\n");
        }
        
        if let Some(tee) = tee_lock.as_ref() {
            let (_, current_state, pending_state) = tee.state(gstreamer::ClockTime::ZERO);
            status.push_str(&format!("  Tee State: {:?} (pending: {:?})\n", current_state, pending_state));
        } else {
            status.push_str("  Tee: None\n");
        }
        
        let recording_pads = self.recording_pads.lock().await;
        status.push_str(&format!("  Active Recording Pads: {}\n", recording_pads.len()));
        
        status
    }

    /// Connects to an RTSP stream and builds a pipeline ready for playback.
    pub async fn connect(&self, protocol: String, url: String) -> Result<(), RecordError> {
        let mut state = self.state.lock().await;
        if state.is_connected {
            return Err(RecordError::StreamError("Already connected to a stream".to_string()));
        }

        info!(%url, "Connecting to stream and creating base pipeline");

        // Build the pipeline: rtspsrc -> identity_src -> rtph264depay -> h264parse -> tee
        let pipeline = Pipeline::new();
        let src = ElementFactory::make("rtspsrc")
            .property("location", &url)
            .property("latency", &0u32)
            .property("timeout", &20000u64)  // 20 second timeout
            .property("retry", &3u32)     // Retry 3 times
            .property("do-retransmission", &true)  // Enable retransmission
            .property("ntp-sync", &true)  // NTP同期を有効化
            .build()?;
        let identity_src = ElementFactory::make("identity")
            .property("signal-handoffs", &true)
            .property("silent", &false)  // デバッグ情報を有効化
            .build()?;
        let depay = ElementFactory::make("rtph264depay")
            .property("wait-for-keyframe", &true)  // キーフレームを待つ
            .build()?;
        let parse = ElementFactory::make("h264parse")
            .property("config-interval", &-1i32)  // Send SPS/PPS with every IDR frame
            .property("disable-passthrough", &true)  // パススルーを無効化
            .build()?;
        let tee = ElementFactory::make("tee")
            .property("allow-not-linked", &true)  // リンクされていないパッドを許可
            .build()?;
        
        pipeline.add_many(&[&src, &identity_src, &depay, &parse, &tee])?;
        Element::link_many(&[&identity_src, &depay, &parse, &tee])?;

        // identity_src handoff
        identity_src.connect("handoff", false, |_values| {
            tracing::info!("[base pipeline] identity_src handoff: buffer arrived");
            None
        });

        // Add bus watch
        let bus = pipeline.bus().unwrap();
        let _watch_id = bus.add_watch(move |_, msg| {
            match msg.view() {
                MessageView::Error(err) => {
                    error!("Error from {:?}: {}", err.src().map(|s| s.path_string()), err.error());
                    ControlFlow::Break
                }
                MessageView::Warning(warn) => {
                    warn!("Warning from {:?}: {}", warn.src().map(|s| s.path_string()), warn.error());
                    ControlFlow::Continue
                }
                MessageView::Eos(_) => {
                    info!("End of stream");
                    ControlFlow::Break
                }
                MessageView::StateChanged(state) => {
                    info!(
                        "Pipeline state changed from {:?} to {:?}",
                        state.old(),
                        state.current()
                    );
                    ControlFlow::Continue
                }
                MessageView::StreamStatus(status) => {
                    info!(
                        "Stream status: {:?}",
                        status.type_()
                    );
                    ControlFlow::Continue
                }
                MessageView::Buffering(buffering) => {
                    info!(
                        "Buffering: {}%",
                        buffering.percent()
                    );
                    ControlFlow::Continue
                }
                _ => ControlFlow::Continue,
            }
        }).unwrap();

        // Create pipeline_weak in advance and move only it to the closure
        let identity_src_clone = identity_src.clone();
        let is_tee_ready_clone = self.is_tee_ready.clone();
        src.connect_pad_added(move |src_elem, src_pad| {
            info!("Received new pad '{}' from '{}'", src_pad.name(), src_elem.name());
            
            // パッドの詳細情報をログ出力
            info!("Pad details:");
            info!("  - Direction: {:?}", src_pad.direction());
            info!("  - Is linked: {}", src_pad.is_linked());
            info!("  - Current caps: {:?}", src_pad.current_caps());
            info!("  - Template: {:?}", src_pad.pad_template());
            info!("  - Parent: {:?}", src_pad.parent().map(|e| e.name()));
            info!("  - Peer: {:?}", src_pad.peer().map(|p| p.name()));
            
            let new_pad_caps = match src_pad.current_caps() {
                Some(caps) => {
                    info!("  - Caps structure: {:?}", caps.structure(0));
                    caps
                },
                None => {
                    error!("Failed to get caps of new pad");
                    return;
                }
            };

            let new_pad_struct = match new_pad_caps.structure(0) {
                Some(s) => {
                    info!("  - Structure name: {}", s.name());
                    info!("  - Media type: {:?}", s.get::<&str>("media"));
                    info!("  - Encoding name: {:?}", s.get::<&str>("encoding-name"));
                    info!("  - Clock rate: {:?}", s.get::<i32>("clock-rate"));
                    info!("  - Payload type: {:?}", s.get::<i32>("payload"));
                    info!("  - All fields: {:?}", s.to_string());
                    s
                },
                None => {
                    error!("Failed to get structure of new pad");
                    return;
                }
            };

            info!("Checking pad compatibility:");
            info!("  - Is RTP: {}", new_pad_struct.name().starts_with("application/x-rtp"));
            info!("  - Is video: {}", new_pad_struct.get::<&str>("media").unwrap_or("") == "video");
            info!("  - Is H264: {}", new_pad_struct.get::<&str>("encoding-name").unwrap_or("").to_uppercase() == "H264");

            if new_pad_struct.name().starts_with("application/x-rtp") &&
               new_pad_struct.get::<&str>("media").unwrap_or("") == "video" &&
               new_pad_struct.get::<&str>("encoding-name").unwrap_or("").to_uppercase() == "H264" {
                let identity_src_sink = match identity_src_clone.static_pad("sink") {
                    Some(pad) => {
                        info!("Got identity_src sink pad: name={}, direction={:?}, is_linked={}, caps={:?}",
                            pad.name(),
                            pad.direction(),
                            pad.is_linked(),
                            pad.current_caps()
                        );
                        pad
                    },
                    None => {
                        error!("Failed to get sink pad from identity_src");
                        return;
                    }
                };

                if identity_src_sink.is_linked() {
                    warn!("identity_src sink pad already linked. Ignoring new pad.");
                    return;
                }

                info!("Attempting to link H264 video pad to identity_src");
                match src_pad.link(&identity_src_sink) {
                    Ok(_) => {
                        info!("Successfully linked src_pad '{}' to identity_src sink", src_pad.name());
                        info!("Link details after successful linking:");
                        info!("  - src_pad: name={}, direction={:?}, is_linked={}, caps={:?}, peer={:?}",
                            src_pad.name(),
                            src_pad.direction(),
                            src_pad.is_linked(),
                            src_pad.current_caps(),
                            src_pad.peer().map(|p| p.name())
                        );
                        info!("  - identity_src_sink: name={}, direction={:?}, is_linked={}, caps={:?}, peer={:?}",
                            identity_src_sink.name(),
                            identity_src_sink.direction(),
                            identity_src_sink.is_linked(),
                            identity_src_sink.current_caps(),
                            identity_src_sink.peer().map(|p| p.name())
                        );
                        is_tee_ready_clone.store(true, Ordering::SeqCst);
                        info!("Set is_tee_ready to true");
                    },
                    Err(e) => {
                        error!("Failed to link src_pad '{}' to identity_src sink: {:?}", src_pad.name(), e);
                        error!("Link failure details:");
                        error!("  - src_pad: name={}, direction={:?}, is_linked={}, caps={:?}",
                            src_pad.name(),
                            src_pad.direction(),
                            src_pad.is_linked(),
                            src_pad.current_caps()
                        );
                        error!("  - identity_src_sink: name={}, direction={:?}, is_linked={}, caps={:?}",
                            identity_src_sink.name(),
                            identity_src_sink.direction(),
                            identity_src_sink.is_linked(),
                            identity_src_sink.current_caps()
                        );
                    }
                }
            } else {
                info!("Pad does not match required criteria for H264 video");
            }
        });

        // Set the pipeline to PLAYING and start receiving the stream
        pipeline.set_state(State::Playing)?;

        // Wait for pipeline to reach PLAYING state
        let state_change_result = pipeline.state(gstreamer::ClockTime::from_seconds(10));
        match state_change_result {
            (_, State::Playing, State::VoidPending) => {
                info!("Pipeline successfully reached PLAYING state");
            },
            (_, current_state, pending_state) => {
                warn!("Pipeline state change incomplete: current={:?}, pending={:?}", current_state, pending_state);
                // Continue anyway, as some RTSP streams may take time to negotiate
            }
        }

        info!("Base pipeline created and set to PLAYING.");
        *self.pipeline.lock().await = Some(pipeline);
        *self.tee.lock().await = Some(tee);
        state.is_connected = true;
        state.protocol = Some(protocol);
        state.url = Some(url.clone());
        self.is_tee_ready.store(false, Ordering::SeqCst); // 初期化
        Ok(())
    }

    /// Starts recording.
    pub async fn start_recording(&self, recording_id: Uuid) -> Result<(), RecordError> {
        tracing::info!("[start_recording] called with recording_id={}", recording_id);
        let mut state = self.state.lock().await;

        if !state.is_connected {
            tracing::error!("[start_recording] Not connected to stream");
            return Err(RecordError::NotConnected);
        }
        if state.is_recording {
            tracing::error!("[start_recording] Already recording");
            return Err(RecordError::AlreadyRecording);
        }

        // Wait for the tee to be ready
        for _ in 0..100 { // Wait for up to 10 seconds
            if self.is_tee_ready.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        if !self.is_tee_ready.load(Ordering::SeqCst) {
            tracing::error!("[start_recording] Tee is not ready for recording");
            return Err(RecordError::StreamError(
                "Tee is not ready for recording. Check if RTSP stream is providing H264 video data.".into()
            ));
        }

        let pipeline = self.pipeline.lock().await;
        let pipeline = match pipeline.as_ref() {
            Some(p) => p,
            None => {
                tracing::error!("[start_recording] Pipeline not found");
                return Err(RecordError::StreamError("Pipeline not found".into()));
            }
        };

        let tee = self.tee.lock().await;
        let tee = match tee.as_ref() {
            Some(t) => t,
            None => {
                tracing::error!("[start_recording] Tee element not found");
                return Err(RecordError::StreamError("Tee element not found".into()));
            }
        };

        // Generate file path
        let mut path = PathBuf::from(&self.config.recording_directory);
        tokio::fs::create_dir_all(&path).await?;
        path.push(format!("{}.mp4", recording_id));
        let location = path.to_str().ok_or_else(|| RecordError::StreamError("Invalid file path".into()))?;

        info!("Creating recording pipeline for file: {}", location);

        // Create recording Bin
        info!("[recording {}] Creating recording pipeline elements", recording_id);
        let queue = ElementFactory::make("queue").name(&format!("rec_queue_{}", recording_id))
            .property("max-size-buffers", &1000u32)  // バッファサイズを増加
            .property("max-size-bytes", &10485760u32)  // 10MB
            .property("max-size-time", &5000000000u64)  // 5秒
            .property_from_str("leaky", "downstream")
            .property("silent", &false)  // デバッグ情報を有効化
            .build()?;
        info!("[recording {}] Created queue element: name={}", recording_id, queue.name());

        let parse = ElementFactory::make("h264parse").name(&format!("rec_h264parse_{}", recording_id))
            .property("config-interval", &-1i32)
            .property("disable-passthrough", &true)
            .build()?;
        info!("[recording {}] Created h264parse element: name={}", recording_id, parse.name());

        let mux = ElementFactory::make("mp4mux").name(&format!("rec_mp4mux_{}", recording_id))
            .property("faststart", &true)
            .property("reserved-moov-update-period", &10000000000u64)  // 10秒
            .build()?;
        info!("[recording {}] Created mp4mux element: name={}", recording_id, mux.name());

        let sink = ElementFactory::make("filesink").name(&format!("rec_filesink_{}", recording_id))
            .property("location", &location)
            .property("sync", &false)
            .property("async", &false)
            .build()?;
        info!("[recording {}] Created filesink element: name={}", recording_id, sink.name());

        // Create recording Bin
        let bin = Bin::new();
        bin.set_property("name", &format!("rec-bin-{}", recording_id));
        info!("[recording {}] Created recording bin: name={}", recording_id, bin.name());

        // Add elements to bin
        bin.add_many(&[&queue, &parse, &mux, &sink])?;
        info!("[recording {}] Added elements to recording bin", recording_id);

        // Link elements
        Element::link_many(&[&queue, &parse, &mux, &sink])?;
        info!("[recording {}] Linked elements in recording bin", recording_id);

        // Create ghost pad
        let queue_sink_pad = queue.static_pad("sink").expect("Failed to get queue sink pad");
        let ghost_pad = GhostPad::with_target(&queue_sink_pad).expect("Failed to create ghost pad");
        ghost_pad.set_active(true).expect("Failed to activate ghost pad");
        bin.add_pad(&ghost_pad)?;
        info!("[recording {}] Added ghost pad to recording bin: name={}, is_active={}, is_linked={}, caps={:?}",
            recording_id,
            ghost_pad.name(),
            ghost_pad.is_active(),
            ghost_pad.is_linked(),
            ghost_pad.current_caps()
        );

        // Add buffer probes to all pads
        let rec_id_probe = recording_id.clone();
        if let Some(queue_sink_pad) = queue.static_pad("sink") {
            queue_sink_pad.add_probe(PadProbeType::BUFFER, move |_, _| {
                tracing::info!("[recording {}] [rec_bin] queue.sink BUFFER probe: buffer arrived", rec_id_probe);
                PadProbeReturn::Ok
            });
            info!("[recording {}] Added buffer probe to queue.sink pad", recording_id);
        }

        // Add recording Bin to the pipeline
        pipeline.add(&bin)?;
        info!("[recording {}] Added recording bin to pipeline", recording_id);

        // Link the request pad of tee to the sink of the recording bin
        let tee_src_pad = tee.request_pad_simple("src_%u").ok_or_else(|| {
            error!("Failed to request tee src pad");
            RecordError::StreamError("Failed to request tee src pad".into())
        })?;

        let recording_bin_sink_pad = bin.static_pad("sink").ok_or_else(|| {
            error!("Failed to get recording bin sink pad");
            RecordError::StreamError("Failed to get recording bin sink pad".into())
        })?;

        match tee_src_pad.link(&recording_bin_sink_pad) {
            Ok(_) => {
                info!("Successfully linked tee src pad to recording bin sink pad");
            }
            Err(e) => {
                error!("Failed to link pads: {}", e);
                return Err(RecordError::StreamError(format!("Failed to link pads: {}", e)));
            }
        }

        // Record the pad (for use when stopping)
        self.recording_pads.lock().await.insert(recording_id, tee_src_pad.clone());
        info!("[recording {}] Recorded tee src pad for later use", recording_id);
        
        // Sync the state of the recording Bin with the parent pipeline
        let sync_res = bin.sync_state_with_parent();
        info!("[recording {}] rec_bin.sync_state_with_parent result: {:?}", recording_id, sync_res);
        
        // 録画パイプラインの状態をPLAYINGに設定
        let state_change = bin.set_state(State::Playing);
        info!(
            "[recording {}] Setting recording bin state to PLAYING: {:?}",
            recording_id,
            state_change
        );

        state.is_recording = true;
        state.current_recording_id = Some(recording_id);
        info!("[recording {}] Successfully started recording", recording_id);
        Ok(())
    }

    /// Stops recording.
    pub async fn stop_recording(&self) -> Result<Uuid, RecordError> {
        let mut state = self.state.lock().await;
        if !state.is_recording {
            return Err(RecordError::StreamError("No recording is in progress".into()));
        }
        let recording_id = match state.current_recording_id.take() {
            Some(id) => id,
            None => {
                tracing::warn!("No current recording_id to stop");
                return Err(RecordError::StreamError("No current recording_id to stop".to_string()));
            }
        };
        info!(%recording_id, "Stopping recording");
        
        let pipeline = self.pipeline.lock().await;
        let pipeline = match pipeline.as_ref() {
            Some(p) => p,
            None => {
                tracing::warn!("No pipeline found in stop_recording");
                return Err(RecordError::StreamError("No pipeline found in stop_recording".to_string()));
            }
        };

        let rec_bin = match pipeline.by_name(&format!("rec-bin-{}", recording_id)) {
            Some(bin) => bin,
            None => {
                tracing::warn!("rec_bin not found in pipeline for recording_id={}", recording_id);
                return Err(RecordError::StreamError("rec_bin not found in pipeline".to_string()));
            }
        };

        // EOSイベントの処理を改善
        let sink_pad = match rec_bin.static_pad("sink") {
            Some(pad) => pad,
            None => {
                tracing::warn!("rec_bin sink pad not found in stop_recording for recording_id={}", recording_id);
                return Err(RecordError::StreamError("rec_bin sink pad not found in stop_recording".to_string()));
            }
        };
        
        let tee_peer_pad = match sink_pad.peer() {
            Some(pad) => pad,
            None => {
                tracing::warn!("sink_pad.peer() not found in stop_recording for recording_id={}", recording_id);
                return Err(RecordError::StreamError("sink_pad.peer() not found in stop_recording".to_string()));
            }
        };

        // EOSイベントの送信を改善
        let eos_event = event::Eos::new();
        if !tee_peer_pad.send_event(eos_event) {
            tracing::warn!("Failed to send EOS event to tee peer pad");
        }

        // バッファが完全に処理されるまで待機
        std::thread::sleep(std::time::Duration::from_millis(5000));  // 5秒待機

        // パイプラインからrec_binを削除
        pipeline.remove(&rec_bin)?;
        
        // rec_binの状態をNULLに設定
        rec_bin.set_state(State::Null)?;

        // ファイルが完全に書き込まれるまで追加で待機
        std::thread::sleep(std::time::Duration::from_millis(2000));  // 2秒待機

        info!(%recording_id, "Recording bin removed and file saved.");
        state.is_recording = false;
        Ok(recording_id)
    }

    /// Disconnects from the stream and stops/destroys the pipeline.
    pub async fn disconnect(&self) -> Result<(), RecordError> {
        let mut state = self.state.lock().await;
        if !state.is_connected {
            warn!("Not connected, nothing to do.");
            return Ok(());
        }

        // Stop recording if in progress
        if state.is_recording {
            warn!("Recording was in progress during disconnect. Stopping it first.");
            drop(state);
            self.stop_recording().await?;
            state = self.state.lock().await;
        }
        
        info!("Disconnecting from stream and stopping pipeline...");
        
        let mut pipeline = self.pipeline.lock().await;
        if let Some(p) = pipeline.take() {
            p.set_state(State::Null)?;
            info!("Pipeline stopped and destroyed successfully.");
        }

        *state = StreamState::default();
        *self.tee.lock().await = None;

        info!("Disconnected from stream and stopped pipeline.");
        Ok(())
    }
}

// StreamState→StreamStatus変換
impl From<&StreamState> for StreamStatus {
    fn from(state: &StreamState) -> Self {
        StreamStatus {
            is_connected: state.is_connected,
            protocol: state.protocol.clone(),
            url: state.url.clone(),
            is_recording: state.is_recording,
            connected_at: None, // 必要なら状態に追加
        }
    }
}

// GStreamerのエラー型はglib::Errorなので、必要ならFrom<glib::Error> for RecordErrorを実装
impl From<glib::Error> for RecordError {
    fn from(err: glib::Error) -> Self {
        RecordError::StreamError(err.to_string())
    }
}
// GStreamer BoolError
impl From<BoolError> for RecordError {
    fn from(err: BoolError) -> Self {
        RecordError::StreamError(err.to_string())
    }
}
// GStreamer StateChangeError
impl From<StateChangeError> for RecordError {
    fn from(err: StateChangeError) -> Self {
        RecordError::StreamError(err.to_string())
    }
}