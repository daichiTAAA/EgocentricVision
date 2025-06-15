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
use glib::LogLevel;
use gstreamer::DebugLevel;
use crate::models::DebugStatus;

/// Stores the logical state of the stream.
#[derive(Debug, Clone, Default)]
pub struct StreamState {
    pub is_connected: bool,
    pub is_recording: bool,
    pub protocol: Option<String>,
    pub url: Option<String>,
    pub current_recording_id: Option<String>,
    pub is_tee_ready: bool,
    pub pipeline: Option<Pipeline>,
    pub tee: Option<Element>,
}

impl StreamState {
    pub fn new() -> Self {
        Self {
            is_connected: false,
            is_recording: false,
            protocol: None,
            url: None,
            current_recording_id: None,
            is_tee_ready: false,
            pipeline: None,
            tee: None,
        }
    }

    pub async fn start_recording(&mut self, recording_id: &str, location: &str) -> Result<(), RecordError> {
        if !self.is_connected {
            return Err(RecordError::StreamError("Stream not connected".to_string()));
        }

        if self.is_recording {
            return Err(RecordError::StreamError("Already recording".to_string()));
        }

        let pipeline = self.pipeline.as_ref().ok_or_else(|| {
            error!("Pipeline not initialized");
            RecordError::StreamError("Pipeline not initialized".to_string())
        })?;

        let tee = self.tee.as_ref().ok_or_else(|| {
            error!("Tee element not initialized");
            RecordError::StreamError("Tee element not initialized".to_string())
        })?;

        // Create recording bin
        let rec_bin = gstreamer::Bin::new();
        rec_bin.set_property("name", &format!("rec-bin-{}", recording_id));
        
        // Create elements
        let queue = gstreamer::ElementFactory::make("queue")
            .name("queue")
            .build()
            .map_err(|e| {
                error!("Failed to create queue element: {}", e);
                RecordError::StreamError(format!("Failed to create queue element: {}", e))
            })?;

        let h264parse = gstreamer::ElementFactory::make("h264parse")
            .name("h264parse")
            .property("config-interval", -1)  // すべてのSPS/PPSを保持
            .property("disable-passthrough", true)  // パススルーを無効化
            .build()
            .map_err(|e| {
                error!("Failed to create h264parse element: {}", e);
                RecordError::StreamError(format!("Failed to create h264parse element: {}", e))
            })?;

        let mp4mux = gstreamer::ElementFactory::make("mp4mux")
            .name("mp4mux")
            .property("reserved-moov-update-period", 1u64)  // より頻繁なmoov更新
            .property("fragment-duration", 0)  // フラグメント化を無効化
            .build()
            .map_err(|e| {
                error!("Failed to create mp4mux element: {}", e);
                RecordError::StreamError(format!("Failed to create mp4mux element: {}", e))
            })?;

        let filesink = gstreamer::ElementFactory::make("filesink")
            .name("filesink")
            .property("location", location)
            .build()
            .map_err(|e| {
                error!("Failed to create filesink element: {}", e);
                RecordError::StreamError(format!("Failed to create filesink element: {}", e))
            })?;

        // Add elements to bin
        rec_bin.add_many(&[&queue, &h264parse, &mp4mux, &filesink])
            .map_err(|e| {
                error!("Failed to add elements to recording bin: {}", e);
                RecordError::StreamError(format!("Failed to add elements to recording bin: {}", e))
            })?;

        // Link elements
        queue.link(&h264parse)
            .map_err(|e| {
                error!("Failed to link queue to h264parse: {}", e);
                RecordError::StreamError(format!("Failed to link queue to h264parse: {}", e))
            })?;

        h264parse.link(&mp4mux)
            .map_err(|e| {
                error!("Failed to link h264parse to mp4mux: {}", e);
                RecordError::StreamError(format!("Failed to link h264parse to mp4mux: {}", e))
            })?;

        mp4mux.link(&filesink)
            .map_err(|e| {
                error!("Failed to link mp4mux to filesink: {}", e);
                RecordError::StreamError(format!("Failed to link mp4mux to filesink: {}", e))
            })?;

        // Add buffer probes
        let rec_id_probe = recording_id.to_string();
        let rec_id_probe2 = rec_id_probe.clone();
        if let Some(queue_sink_pad) = queue.static_pad("sink") {
            queue_sink_pad.add_probe(PadProbeType::BUFFER, move |_, _| {
                tracing::info!("[recording {}] [rec_bin] queue.sink BUFFER probe: buffer arrived", rec_id_probe);
                PadProbeReturn::Ok
            });
        }
        if let Some(queue_src_pad) = queue.static_pad("src") {
            queue_src_pad.add_probe(PadProbeType::BUFFER, move |_, _| {
                tracing::info!("[recording {}] [rec_bin] queue.src BUFFER probe: buffer arrived", rec_id_probe2);
                PadProbeReturn::Ok
            });
        }

        // Add bus watch for recording bin
        let rec_id_state = recording_id.to_string();
        let bus = pipeline.bus().ok_or_else(|| RecordError::StreamError("Failed to get bus from pipeline".into()))?;
        let bin_clone = rec_bin.clone();
        let _watch_id = bus.add_watch(move |_, msg| {
            match msg.view() {
                MessageView::Error(err) => {
                    error!("[recording {}] Error from recording bin: {}", rec_id_state, err.error());
                    if let Some(debug) = err.debug() {
                        error!("[recording {}] Debug info: {:?}", rec_id_state, debug(&err));
                    }
                    // エラーが発生した場合、パイプラインを停止
                    if let Some(element) = err.src() {
                        if let Some(pipeline) = element.parent() {
                            if let Some(pipeline) = pipeline.downcast_ref::<gstreamer::Pipeline>() {
                                let _ = pipeline.set_state(gstreamer::State::Null);
                            }
                        }
                    }
                }
                MessageView::Warning(warn) => {
                    warn!("[recording {}] Warning from recording bin: {}", rec_id_state, warn.error());
                    if let Some(debug) = warn.debug() {
                        warn!("[recording {}] Debug info: {:?}", rec_id_state, debug(&warn));
                    }
                }
                MessageView::Eos(_) => {
                    info!("[recording {}] End of stream from recording bin", rec_id_state);
                    // EOSが発生した場合、パイプラインを停止
                    if let Some(pipeline) = bin_clone.parent() {
                        if let Some(pipeline) = pipeline.downcast_ref::<gstreamer::Pipeline>() {
                            let _ = pipeline.set_state(gstreamer::State::Null);
                        }
                    }
                }
                MessageView::StateChanged(state) => {
                    let old = state.old();
                    let new = state.current();
                    let pending = state.pending();
                    info!("[recording {}] State changed from {:?} to {:?} (pending: {:?})", rec_id_state, old, new, pending);
                    if new == gstreamer::State::Playing {
                        info!("[recording {}] Recording bin is now playing", rec_id_state);
                    }
                }
                MessageView::Buffering(buffering) => {
                    info!("[recording {}] Buffering: {}%", rec_id_state, buffering.percent());
                }
                _ => {}
            }
            glib::ControlFlow::Continue
        }).unwrap();

        // Add recording bin to pipeline
        pipeline.add(&rec_bin)
            .map_err(|e| {
                error!("Failed to add recording bin to pipeline: {}", e);
                RecordError::StreamError(format!("Failed to add recording bin to pipeline: {}", e))
            })?;

        // Link tee to recording bin
        let tee_src_pad = tee.request_pad_simple("src_%u")
            .ok_or_else(|| {
                error!("Failed to get tee src pad");
                RecordError::StreamError("Failed to get tee src pad".to_string())
            })?;

        let rec_bin_sink_pad = queue.static_pad("sink")
            .ok_or_else(|| {
                error!("Failed to get recording bin sink pad");
                RecordError::StreamError("Failed to get recording bin sink pad".to_string())
            })?;

        // Check caps compatibility
        let tee_src_caps = tee_src_pad.current_caps()
            .ok_or_else(|| {
                error!("Failed to get tee src pad caps");
                RecordError::StreamError("Failed to get tee src pad caps".to_string())
            })?;

        let bin_caps = rec_bin_sink_pad.current_caps().or_else(|| {
            warn!("[recording {}] recording_bin_sink_pad.current_caps() is None, fallback to query_caps(None)", recording_id);
            Some(rec_bin_sink_pad.query_caps(None))
        }).ok_or_else(|| {
            error!("[recording {}] Failed to get recording bin sink pad caps", recording_id);
            RecordError::StreamError("Failed to get recording bin sink pad caps".into())
        })?;

        if !tee_src_caps.can_intersect(&bin_caps) {
            error!("Caps not compatible between tee src pad and recording bin sink pad");
            return Err(RecordError::StreamError("Caps not compatible between tee src pad and recording bin sink pad".to_string()));
        }

        tee_src_pad.link(&rec_bin_sink_pad)
            .map_err(|e| {
                error!("Failed to link tee to recording bin: {}", e);
                RecordError::StreamError(format!("Failed to link tee to recording bin: {}", e))
            })?;

        // Sync recording bin state with pipeline
        rec_bin.sync_children_states()
            .map_err(|e| {
                error!("Failed to sync recording bin state: {}", e);
                RecordError::StreamError(format!("Failed to sync recording bin state: {}", e))
            })?;

        // Set recording bin state to Playing
        rec_bin.set_state(gstreamer::State::Playing)
            .map_err(|e| {
                error!("Failed to set recording bin state to Playing: {}", e);
                RecordError::StreamError(format!("Failed to set recording bin state to Playing: {}", e))
            })?;

        // Log state of each element
        info!("[recording {}] Recording bin state: {:?}", recording_id, rec_bin.current_state());
        info!("[recording {}] Queue state: {:?}", recording_id, queue.current_state());
        info!("[recording {}] H264Parse state: {:?}", recording_id, h264parse.current_state());
        info!("[recording {}] MP4Mux state: {:?}", recording_id, mp4mux.current_state());
        info!("[recording {}] FileSink state: {:?}", recording_id, filesink.current_state());

        self.is_recording = true;
        self.current_recording_id = Some(recording_id.to_string());

        Ok(())
    }
}

/// Manages the GStreamer pipeline and stream state.
pub struct StreamManager {
    state: Arc<Mutex<StreamState>>,
    config: Config,
    recording_pads: Arc<Mutex<HashMap<String, gstreamer::Pad>>>,
    is_tee_ready: Arc<AtomicBool>,
}

impl StreamManager {
    /// Creates a new StreamManager instance and initializes GStreamer.
    pub fn new(config: Config) -> Self {
        if let Err(e) = gstreamer::init() {
            panic!("Failed to initialize GStreamer: {}", e);
        }
        Self {
            state: Arc::new(Mutex::new(StreamState::new())),
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
    pub async fn get_detailed_status(&self) -> DebugStatus {
        let state = self.state.lock().await;
        let pipeline_state = state.pipeline.as_ref().map(|p| p.state(gstreamer::ClockTime::ZERO));
        let tee_state = state.tee.as_ref().map(|t| t.state(gstreamer::ClockTime::ZERO));
        
        let (pipeline_current, pipeline_pending) = if let Some((_, current, pending)) = pipeline_state {
            (Some(format!("{:?}", current)), Some(format!("{:?}", pending)))
        } else {
            (None, None)
        };

        let (tee_current, tee_pending) = if let Some((_, current, pending)) = tee_state {
            (Some(format!("{:?}", current)), Some(format!("{:?}", pending)))
        } else {
            (None, None)
        };

        let recording_pads = self.recording_pads.lock().await;
        
        DebugStatus {
            is_connected: state.is_connected,
            is_recording: state.is_recording,
            protocol: state.protocol.clone(),
            url: state.url.clone(),
            tee_ready: self.is_tee_ready.load(Ordering::SeqCst),
            pipeline_state: pipeline_current,
            pipeline_pending_state: pipeline_pending,
            tee_state: tee_current,
            tee_pending_state: tee_pending,
            active_recording_pads: recording_pads.len(),
        }
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
                    error!("[BUS ERROR] from {:?}: {}", err.src().map(|s| s.path_string()), err.error());
                    ControlFlow::Break
                }
                MessageView::Warning(warn) => {
                    warn!("[BUS WARNING] from {:?}: {}", warn.src().map(|s| s.path_string()), warn.error());
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
        let state_clone = self.state.clone();
        src.connect_pad_added(move |src_elem, src_pad| {
            info!("[pad-added] event fired: src_elem={}, src_pad={}", src_elem.name(), src_pad.name());
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
            let is_rtp = new_pad_struct.name().starts_with("application/x-rtp");
            let is_video = new_pad_struct.get::<&str>("media").unwrap_or("") == "video";
            let is_h264 = new_pad_struct.get::<&str>("encoding-name").unwrap_or("").to_uppercase() == "H264";
            info!("  - Is RTP: {}", is_rtp);
            info!("  - Is video: {}", is_video);
            info!("  - Is H264: {}", is_h264);
            if !is_rtp { info!("[pad-added] pad is not RTP"); }
            if is_rtp && !is_video { info!("[pad-added] pad is RTPだがvideoでない"); }
            if is_rtp && is_video && !is_h264 { info!("[pad-added] padはvideoだがH264でない"); }
            if is_rtp && is_video && is_h264 {
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

        // パイプラインの状態がPLAYINGになるまで待機
        let (_, current_state, pending_state) = pipeline.state(gstreamer::ClockTime::from_seconds(10));
        if current_state != State::Playing {
            error!("Pipeline failed to reach PLAYING state: current={:?}, pending={:?}", current_state, pending_state);
            return Err(RecordError::StreamError("Failed to start pipeline".into()));
        }

        // パイプラインが安定するまで待機
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        info!("Base pipeline created and set to PLAYING.");
        state.pipeline = Some(pipeline);
        state.tee = Some(tee);
        state.protocol = Some(protocol);
        state.url = Some(url.clone());
        state.is_connected = true;  // 接続状態を設定
        self.is_tee_ready.store(false, Ordering::SeqCst); // 初期化
        Ok(())
    }

    /// Starts recording.
    pub async fn start_recording(&self, recording_id: &str, location: &str) -> Result<(), RecordError> {
        info!("[recording {}] Starting recording at location: {}", recording_id, location);
        let mut waited = 0;
        let mut state = self.state.lock().await;
        info!("[recording {}] [debug] start_recording: is_connected={} is_recording={}", recording_id, state.is_connected, state.is_recording);
        // 5秒間（100msごとに）is_connectedをチェック
        while !state.is_connected && waited < 50 {
            drop(state);
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            waited += 1;
            state = self.state.lock().await;
            info!("[recording {}] [debug] waiting: is_connected={} is_recording={}", recording_id, state.is_connected, state.is_recording);
        }
        if !state.is_connected {
            error!("[recording {}] Cannot start recording: stream is not connected", recording_id);
            return Err(RecordError::StreamError("Stream is not connected".into()));
        }

        if state.is_recording {
            error!("[recording {}] Cannot start recording: already recording", recording_id);
            return Err(RecordError::StreamError("Already recording".into()));
        }

        let pipeline = state.pipeline.as_ref().ok_or_else(|| {
            error!("[recording {}] Cannot start recording: pipeline is not initialized", recording_id);
            RecordError::StreamError("Pipeline is not initialized".into())
        })?;

        let tee = state.tee.as_ref().ok_or_else(|| {
            error!("[recording {}] Cannot start recording: tee element is not initialized", recording_id);
            RecordError::StreamError("Tee element is not initialized".into())
        })?;

        // 親パイプラインの状態を確認
        let (_, parent_state, _) = pipeline.state(gstreamer::ClockTime::from_seconds(1));
        if parent_state != State::Playing {
            error!("[recording {}] Parent pipeline is not in PLAYING state: {:?}", recording_id, parent_state);
            return Err(RecordError::StreamError("Parent pipeline is not in PLAYING state".into()));
        }

        info!("[recording {}] Creating recording pipeline elements", recording_id);
        let queue = ElementFactory::make("queue").name(&format!("rec_queue_{}", recording_id))
            .property("max-size-buffers", &10000u32)  // バッファ数を増やす
            .property("max-size-bytes", &104857600u32)  // 100MBに増やす
            .property("max-size-time", &30000000000u64)  // 30秒に増やす
            .property_from_str("leaky", "downstream")
            .property("silent", &false)
            .build()
            .map_err(|e| {
                error!("[recording {}] Failed to create queue element: {}", recording_id, e);
                RecordError::StreamError(format!("Failed to create queue element: {}", e))
            })?;
        info!("[recording {}] Created queue element: name={}", recording_id, queue.name());

        let parse = ElementFactory::make("h264parse").name(&format!("rec_h264parse_{}", recording_id))
            .property("config-interval", &-1i32)
            .property("disable-passthrough", &true)
            .build()
            .map_err(|e| {
                error!("[recording {}] Failed to create h264parse element: {}", recording_id, e);
                RecordError::StreamError(format!("Failed to create h264parse element: {}", e))
            })?;
        info!("[recording {}] Created h264parse element: name={}", recording_id, parse.name());

        let mux = ElementFactory::make("mp4mux")
            .property("faststart", true)
            .property("reserved-moov-update-period", 1u64)
            .property("streamable", true)
            .property("fragment-duration", 1u32)
            .build()
            .map_err(|e| {
                error!("[recording {}] Failed to create mp4mux element: {}", recording_id, e);
                RecordError::StreamError(format!("Failed to create mp4mux element: {}", e))
            })?;
        info!("[recording {}] Created mp4mux element: name={}", recording_id, mux.name());

        let sink = ElementFactory::make("filesink").name(&format!("rec_filesink_{}", recording_id))
            .property("location", &location)
            .property("sync", &false)
            .property("async", &false)
            .build()
            .map_err(|e| {
                error!("[recording {}] Failed to create filesink element: {}", recording_id, e);
                RecordError::StreamError(format!("Failed to create filesink element: {}", e))
            })?;
        info!("[recording {}] Created filesink element: name={}", recording_id, sink.name());

        // Create recording Bin
        let bin = Bin::new();
        bin.set_property("name", &format!("rec-bin-{}", recording_id));
        info!("[recording {}] Created recording bin: name={}", recording_id, bin.name());

        // Add elements to bin
        bin.add_many(&[&queue, &parse, &mux, &sink])
            .map_err(|e| {
                error!("[recording {}] Failed to add elements to recording bin: {}", recording_id, e);
                RecordError::StreamError(format!("Failed to add elements to recording bin: {}", e))
            })?;
        info!("[recording {}] Added elements to recording bin", recording_id);

        // Link elements
        Element::link_many(&[&queue, &parse, &mux, &sink])
            .map_err(|e| {
                error!("[recording {}] Failed to link elements in recording bin: {}", recording_id, e);
                RecordError::StreamError(format!("Failed to link elements in recording bin: {}", e))
            })?;
        info!("[recording {}] Linked elements in recording bin", recording_id);

        // パッドのリンク状態を確認
        if let Some(queue_sink) = queue.static_pad("sink") {
            info!("[recording {}] queue.sink pad state: is_linked={}, caps={:?}", 
                recording_id, queue_sink.is_linked(), queue_sink.current_caps());
        }
        if let Some(queue_src) = queue.static_pad("src") {
            info!("[recording {}] queue.src pad state: is_linked={}, caps={:?}", 
                recording_id, queue_src.is_linked(), queue_src.current_caps());
        }
        if let Some(parse_sink) = parse.static_pad("sink") {
            info!("[recording {}] parse.sink pad state: is_linked={}, caps={:?}", 
                recording_id, parse_sink.is_linked(), parse_sink.current_caps());
        }
        if let Some(parse_src) = parse.static_pad("src") {
            info!("[recording {}] parse.src pad state: is_linked={}, caps={:?}", 
                recording_id, parse_src.is_linked(), parse_src.current_caps());
        }
        if let Some(mux_sink) = mux.static_pad("sink") {
            info!("[recording {}] mux.sink pad state: is_linked={}, caps={:?}", 
                recording_id, mux_sink.is_linked(), mux_sink.current_caps());
        }
        if let Some(mux_src) = mux.static_pad("src") {
            info!("[recording {}] mux.src pad state: is_linked={}, caps={:?}", 
                recording_id, mux_src.is_linked(), mux_src.current_caps());
        }
        if let Some(sink_sink) = sink.static_pad("sink") {
            info!("[recording {}] sink.sink pad state: is_linked={}, caps={:?}", 
                recording_id, sink_sink.is_linked(), sink_sink.current_caps());
        }

        // Create ghost pad
        let queue_sink_pad = queue.static_pad("sink").ok_or_else(|| {
            error!("[recording {}] Failed to get queue sink pad", recording_id);
            RecordError::StreamError("Failed to get queue sink pad".into())
        })?;
        let ghost_pad = GhostPad::with_target(&queue_sink_pad).map_err(|e| {
            error!("[recording {}] Failed to create ghost pad: {}", recording_id, e);
            RecordError::StreamError(format!("Failed to create ghost pad: {}", e))
        })?;
        ghost_pad.set_active(true).map_err(|e| {
            error!("[recording {}] Failed to activate ghost pad: {}", recording_id, e);
            RecordError::StreamError(format!("Failed to activate ghost pad: {}", e))
        })?;
        bin.add_pad(&ghost_pad).map_err(|e| {
            error!("[recording {}] Failed to add ghost pad to recording bin: {}", recording_id, e);
            RecordError::StreamError(format!("Failed to add ghost pad to recording bin: {}", e))
        })?;
        info!("[recording {}] Added ghost pad to recording bin: name={}, is_active={}, is_linked={}, caps={:?}",
            recording_id,
            ghost_pad.name(),
            ghost_pad.is_active(),
            ghost_pad.is_linked(),
            ghost_pad.current_caps()
        );

        // Add buffer probes to all pads
        let rec_id_probe = recording_id.to_string();
        let rec_id_probe2 = rec_id_probe.clone();
        if let Some(queue_sink_pad) = queue.static_pad("sink") {
            queue_sink_pad.add_probe(PadProbeType::BUFFER, move |_, _| {
                tracing::info!("[recording {}] [rec_bin] queue.sink BUFFER probe: buffer arrived", rec_id_probe);
                PadProbeReturn::Ok
            });
            info!("[recording {}] Added buffer probe to queue.sink pad: is_linked={}, caps={:?}",
                recording_id,
                queue_sink_pad.is_linked(),
                queue_sink_pad.current_caps()
            );
        }
        let rec_id_probe = recording_id.to_string();
        if let Some(parse_sink_pad) = parse.static_pad("sink") {
            parse_sink_pad.add_probe(PadProbeType::BUFFER, move |_, _| {
                tracing::info!("[recording {}] [rec_bin] h264parse.sink BUFFER probe: buffer arrived", rec_id_probe);
                PadProbeReturn::Ok
            });
            info!("[recording {}] Added buffer probe to h264parse.sink pad: is_linked={}, caps={:?}",
                recording_id,
                parse_sink_pad.is_linked(),
                parse_sink_pad.current_caps()
            );
        }
        let rec_id_probe = recording_id.to_string();
        if let Some(parse_src_pad) = parse.static_pad("src") {
            parse_src_pad.add_probe(PadProbeType::BUFFER, move |_, _| {
                tracing::info!("[recording {}] [rec_bin] h264parse.src BUFFER probe: buffer arrived", rec_id_probe);
                PadProbeReturn::Ok
            });
            info!("[recording {}] Added buffer probe to h264parse.src pad: is_linked={}, caps={:?}",
                recording_id,
                parse_src_pad.is_linked(),
                parse_src_pad.current_caps()
            );
        }
        let rec_id_probe = recording_id.to_string();
        if let Some(mux_sink_pad) = mux.static_pad("sink") {
            mux_sink_pad.add_probe(PadProbeType::BUFFER, move |_, _| {
                tracing::info!("[recording {}] [rec_bin] mp4mux.sink BUFFER probe: buffer arrived", rec_id_probe);
                PadProbeReturn::Ok
            });
            info!("[recording {}] mp4mux.sink pad details: is_linked={}, caps={:?}, peer={:?}, direction={:?}",
                recording_id,
                mux_sink_pad.is_linked(),
                mux_sink_pad.current_caps(),
                mux_sink_pad.peer().map(|p| p.name()),
                mux_sink_pad.direction()
            );
        }
        if let Some(mux_src_pad) = mux.static_pad("src") {
            mux_src_pad.add_probe(PadProbeType::BUFFER, move |_, _| {
                tracing::info!("[recording {}] [rec_bin] mp4mux.src BUFFER probe: buffer arrived", rec_id_probe2);
                PadProbeReturn::Ok
            });
            info!("[recording {}] mp4mux.src pad details: is_linked={}, caps={:?}, peer={:?}, direction={:?}",
                recording_id,
                mux_src_pad.is_linked(),
                mux_src_pad.current_caps(),
                mux_src_pad.peer().map(|p| p.name()),
                mux_src_pad.direction()
            );
        }
        let rec_id_probe = recording_id.to_string();
        if let Some(filesink_sink_pad) = sink.static_pad("sink") {
            filesink_sink_pad.add_probe(PadProbeType::BUFFER, move |_, _| {
                tracing::info!("[recording {}] [rec_bin] filesink.sink BUFFER probe: buffer arrived", rec_id_probe);
                PadProbeReturn::Ok
            });
            info!("[recording {}] filesink.sink pad details: is_linked={}, caps={:?}, peer={:?}, direction={:?}",
                recording_id,
                filesink_sink_pad.is_linked(),
                filesink_sink_pad.current_caps(),
                filesink_sink_pad.peer().map(|p| p.name()),
                filesink_sink_pad.direction()
            );
        }

        // Add bus watch for recording bin
        let rec_id_state = recording_id.to_string();
        let bus = pipeline.bus().ok_or_else(|| RecordError::StreamError("Failed to get bus from pipeline".into()))?;
        let bin_clone = bin.clone();
        let _watch_id = bus.add_watch(move |_, msg| {
            match msg.view() {
                MessageView::Error(err) => {
                    error!("[recording {}] Error from recording bin: {}", rec_id_state, err.error());
                    if let Some(debug) = err.debug() {
                        error!("[recording {}] Debug info: {:?}", rec_id_state, debug(&err));
                    }
                    // エラーが発生した場合、パイプラインを停止
                    if let Some(element) = err.src() {
                        if let Some(pipeline) = element.parent() {
                            if let Some(pipeline) = pipeline.downcast_ref::<gstreamer::Pipeline>() {
                                let _ = pipeline.set_state(gstreamer::State::Null);
                            }
                        }
                    }
                }
                MessageView::Warning(warn) => {
                    warn!("[recording {}] Warning from recording bin: {}", rec_id_state, warn.error());
                    if let Some(debug) = warn.debug() {
                        warn!("[recording {}] Debug info: {:?}", rec_id_state, debug(&warn));
                    }
                }
                MessageView::Eos(_) => {
                    info!("[recording {}] End of stream from recording bin", rec_id_state);
                    // EOSが発生した場合、パイプラインを停止
                    if let Some(pipeline) = bin_clone.parent() {
                        if let Some(pipeline) = pipeline.downcast_ref::<gstreamer::Pipeline>() {
                            let _ = pipeline.set_state(gstreamer::State::Null);
                        }
                    }
                }
                MessageView::StateChanged(state) => {
                    let old = state.old();
                    let new = state.current();
                    let pending = state.pending();
                    info!("[recording {}] State changed from {:?} to {:?} (pending: {:?})", rec_id_state, old, new, pending);
                    if new == gstreamer::State::Playing {
                        info!("[recording {}] Recording bin is now playing", rec_id_state);
                    }
                }
                MessageView::Buffering(buffering) => {
                    info!("[recording {}] Buffering: {}%", rec_id_state, buffering.percent());
                }
                _ => {}
            }
            glib::ControlFlow::Continue
        }).unwrap();

        // Add recording Bin to the pipeline
        pipeline.add(&bin).map_err(|e| {
            error!("[recording {}] Failed to add recording bin to pipeline: {}", recording_id, e);
            RecordError::StreamError(format!("Failed to add recording bin to pipeline: {}", e))
        })?;
        info!("[recording {}] Added recording bin to pipeline", recording_id);

        // Link the request pad of tee to the sink of the recording bin
        let tee_src_pad = tee.request_pad_simple("src_%u").ok_or_else(|| {
            error!("[recording {}] Failed to request tee src pad", recording_id);
            RecordError::StreamError("Failed to request tee src pad".into())
        })?;

        let recording_bin_sink_pad = bin.static_pad("sink").ok_or_else(|| {
            error!("[recording {}] Failed to get recording bin sink pad", recording_id);
            RecordError::StreamError("Failed to get recording bin sink pad".into())
        })?;

        // パッドのリンク前にキャプスを確認
        info!("[recording {}] Before linking pads:", recording_id);
        info!("  - tee_src_pad caps: {:?}", tee_src_pad.current_caps());
        info!("  - recording_bin_sink_pad caps: {:?}", recording_bin_sink_pad.current_caps());

        // キャプスの互換性を確認
        let tee_caps = tee_src_pad.current_caps().or_else(|| {
            warn!("[recording {}] tee_src_pad.current_caps() is None, fallback to query_caps(None)", recording_id);
            Some(tee_src_pad.query_caps(None))
        }).ok_or_else(|| {
            error!("[recording {}] Failed to get tee src pad caps", recording_id);
            RecordError::StreamError("Failed to get tee src pad caps".into())
        })?;

        let bin_caps = recording_bin_sink_pad.current_caps().or_else(|| {
            warn!("[recording {}] recording_bin_sink_pad.current_caps() is None, fallback to query_caps(None)", recording_id);
            Some(recording_bin_sink_pad.query_caps(None))
        }).ok_or_else(|| {
            error!("[recording {}] Failed to get recording bin sink pad caps", recording_id);
            RecordError::StreamError("Failed to get recording bin sink pad caps".into())
        })?;

        info!("[recording {}] Checking caps compatibility:", recording_id);
        info!("  - tee_src_pad caps structure: {:?}", tee_caps.structure(0));
        info!("  - recording_bin_sink_pad caps structure: {:?}", bin_caps.structure(0));

        // キャプスの互換性を確認
        if !tee_caps.can_intersect(&bin_caps) {
            error!("[recording {}] Caps are not compatible: tee={:?}, bin={:?}", 
                recording_id, tee_caps, bin_caps);
            return Err(RecordError::StreamError(format!(
                "Caps are not compatible: tee={:?}, bin={:?}",
                tee_caps, bin_caps
            )));
        }

        match tee_src_pad.link(&recording_bin_sink_pad) {
            Ok(_) => {
                info!("[recording {}] Successfully linked tee src pad to recording bin sink pad", recording_id);
                info!("[recording {}] Link details after successful linking:", recording_id);
                info!("  - tee_src_pad: name={}, direction={:?}, is_linked={}, caps={:?}, peer={:?}",
                    tee_src_pad.name(),
                    tee_src_pad.direction(),
                    tee_src_pad.is_linked(),
                    tee_src_pad.current_caps(),
                    tee_src_pad.peer().map(|p| p.name())
                );
                info!("  - recording_bin_sink_pad: name={}, direction={:?}, is_linked={}, caps={:?}, peer={:?}",
                    recording_bin_sink_pad.name(),
                    recording_bin_sink_pad.direction(),
                    recording_bin_sink_pad.is_linked(),
                    recording_bin_sink_pad.current_caps(),
                    recording_bin_sink_pad.peer().map(|p| p.name())
                );
            }
            Err(e) => {
                error!("[recording {}] Failed to link pads: {}", recording_id, e);
                return Err(RecordError::StreamError(format!(
                    "Failed to link pads: {} (tee_caps={:?}, bin_caps={:?})",
                    e, tee_caps, bin_caps
                )));
            }
        }

        // Record the pad (for use when stopping)
        self.recording_pads.lock().await.insert(recording_id.to_string(), tee_src_pad.clone());
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

        // パイプラインの状態を確認
        let (_, current_state, pending_state) = bin.state(gstreamer::ClockTime::from_seconds(30));  // 待機時間を30秒に増やす
        if current_state != State::Playing {
            error!("[recording {}] Recording bin failed to reach PLAYING state: current={:?}, pending={:?}", 
                recording_id, current_state, pending_state);
            return Err(RecordError::StreamError(format!(
                "Failed to start recording bin: current={:?}, pending={:?}",
                current_state, pending_state
            )));
        }

        // 各要素の状態を確認
        let (_, queue_state, _) = queue.state(gstreamer::ClockTime::from_seconds(1));
        let (_, parse_state, _) = parse.state(gstreamer::ClockTime::from_seconds(1));
        let (_, mux_state, _) = mux.state(gstreamer::ClockTime::from_seconds(1));
        let (_, sink_state, _) = sink.state(gstreamer::ClockTime::from_seconds(1));
        
        info!(
            "[recording {}] Element states: queue={:?}, parse={:?}, mux={:?}, sink={:?}",
            recording_id,
            queue_state,
            parse_state,
            mux_state,
            sink_state
        );

        // すべての要素がPLAYING状態であることを確認
        if queue_state != State::Playing || parse_state != State::Playing || 
           mux_state != State::Playing || sink_state != State::Playing {
            error!("[recording {}] Some elements failed to reach PLAYING state", recording_id);
            return Err(RecordError::StreamError("Some elements failed to reach PLAYING state".into()));
        }

        state.is_recording = true;
        state.current_recording_id = Some(recording_id.to_string());
        info!("[recording {}] Successfully started recording", recording_id);
        Ok(())
    }

    /// Stops recording.
    pub async fn stop_recording(&self) -> Result<String, RecordError> {
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
        
        let pipeline = state.pipeline.as_ref().ok_or_else(|| {
            error!("[recording {}] Cannot stop recording: pipeline is not initialized", &recording_id);
            RecordError::StreamError("Pipeline is not initialized".into())
        })?;

        // EOSイベントの処理を改善
        let sink_pad = match pipeline.by_name(&format!("rec-bin-{}", recording_id)) {
            Some(bin) => bin.static_pad("sink").ok_or_else(|| {
                error!("[recording {}] rec_bin sink pad not found in stop_recording for recording_id={}", recording_id, recording_id);
                RecordError::StreamError("rec_bin sink pad not found in stop_recording".to_string())
            })?,
            None => {
                tracing::warn!("rec_bin not found in pipeline for recording_id={}", recording_id);
                return Err(RecordError::StreamError("rec_bin not found in pipeline".to_string()));
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
        let bin_name = format!("rec-bin-{}", recording_id);
        if let Some(bin) = pipeline.by_name(&bin_name) {
            pipeline.remove(&bin)?;
        }
        
        // rec_binの状態をNULLに設定
        pipeline.set_state(State::Null)?;

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
        
        if let Some(p) = state.pipeline.take() {
            p.set_state(State::Null)?;
            info!("Pipeline stopped and destroyed successfully.");
        }

        *state = StreamState::new();
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