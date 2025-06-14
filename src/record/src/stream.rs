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
};
use tracing::{info, warn, error, debug};
use crate::models::StreamStatus;
use glib::BoolError;
use std::collections::HashMap;

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
            .property("timeout", &20000u64)  // 20 second timeout (u64型に変更)
            .property("retry", &3u32)     // Retry 3 times
            .property("do-retransmission", &true)  // Enable retransmission
            .build()?;
        let identity_src = ElementFactory::make("identity").property("signal-handoffs", &true).build()?;
        let depay = ElementFactory::make("rtph264depay").build()?;
        let parse = ElementFactory::make("h264parse")
            .property("config-interval", &-1i32)  // Send SPS/PPS with every IDR frame
            .build()?;
        let tee = ElementFactory::make("tee").name("t").build()?;
        
        pipeline.add_many(&[&src, &identity_src, &depay, &parse, &tee])?;
        Element::link_many(&[&identity_src, &depay, &parse, &tee])?;

        // identity_src handoff
        identity_src.connect("handoff", false, |_values| {
            tracing::info!("[base pipeline] identity_src handoff: buffer arrived");
            None
        });

        // Add bus watch (including StateChanged for all elements)
        let bus = pipeline.bus().unwrap();
        bus.add_watch_local(move |_, msg| {
            use gstreamer::MessageView;
            match msg.view() {
                MessageView::Error(err) => {
                    error!(
                        "GStreamer Error from element {}: {} (debug: {})",
                        err.src().map_or_else(|| "Unknown".to_string(), |s| s.path_string().to_string()),
                        err.error(),
                        err.debug().unwrap_or_default()
                    );
                }
                MessageView::Warning(warn) => {
                    warn!(
                        "GStreamer Warning from element {}: {} (debug: {})",
                        warn.src().map_or_else(|| "Unknown".to_string(), |s| s.path_string().to_string()),
                        warn.error(),
                        warn.debug().unwrap_or_default()
                    );
                }
                MessageView::Eos(..) => {
                    debug!("GStreamer: Received EOS");
                }
                MessageView::StateChanged(state_changed) => {
                    if let Some(src) = state_changed.src() {
                        debug!(
                            "GStreamer: Element {} state changed from {:?} to {:?}",
                            src.path_string(),
                            state_changed.old(),
                            state_changed.current()
                        );
                    }
                }
                MessageView::StreamStatus(stream_status) => {
                    debug!(
                        "GStreamer: Stream status from {}: {:?}",
                        stream_status.src().map_or_else(|| "Unknown".to_string(), |s| s.path_string().to_string()),
                        stream_status.stream_status_object()
                    );
                }
                MessageView::NewClock(new_clock) => {
                    if let Some(clock) = new_clock.clock() {
                        debug!("GStreamer: New clock selected: {}", clock.name());
                    }
                }
                _ => {
                    info!("GStreamer: Other bus message: {:?}", msg);
                }
            }
            glib::ControlFlow::Continue
        })
        .expect("Failed to add bus watch");

        // Create pipeline_weak in advance and move only it to the closure
        let depay_clone = depay.clone();
        let identity_src_clone = identity_src.clone();
        let is_tee_ready_clone = self.is_tee_ready.clone();
        src.connect_pad_added(move |src_elem, src_pad| {
            info!("Received new pad '{}' from '{}'", src_pad.name(), src_elem.name());
            let new_pad_caps = src_pad.current_caps().expect("Failed to get caps of new pad.");
            let new_pad_struct = new_pad_caps.structure(0).expect("Failed to get structure of new pad.");

            if new_pad_struct.name().starts_with("application/x-rtp") &&
               new_pad_struct.get::<&str>("media").unwrap_or("") == "video" &&
               new_pad_struct.get::<&str>("encoding-name").unwrap_or("").to_uppercase() == "H264" {
                let identity_src_sink = identity_src_clone.static_pad("sink").expect("Failed to get sink pad from identity_src");
                if identity_src_sink.is_linked() {
                    warn!("identity_src sink pad already linked. Ignoring new pad.");
                    return;
                }
                info!("Attempting to link H264 video pad to identity_src");
                match src_pad.link(&identity_src_sink) {
                    Ok(_) => {
                        info!("Successfully linked src_pad '{}' to identity_src sink", src_pad.name());
                        is_tee_ready_clone.store(true, Ordering::SeqCst);
                    },
                    Err(e) => {
                        error!("Failed to link src_pad '{}' to identity_src sink: {:?}", src_pad.name(), e);
                    }
                }
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
        let mut state = self.state.lock().await;

        if !state.is_connected {
            return Err(RecordError::NotConnected);
        }
        if state.is_recording {
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
            return Err(RecordError::StreamError(
                "Tee is not ready for recording. Check if RTSP stream is providing H264 video data.".into()
            ));
        }

        let pipeline = self.pipeline.lock().await;
        let pipeline = pipeline.as_ref().ok_or_else(|| RecordError::StreamError("Pipeline not found".into()))?;

        let tee = self.tee.lock().await;
        let tee = tee.as_ref().ok_or_else(|| RecordError::StreamError("Tee element not found".into()))?;

        // Generate file path
        let mut path = PathBuf::from(&self.config.recording_directory);
        tokio::fs::create_dir_all(&path).await?;
        path.push(format!("{}.mp4", recording_id));
        let location = path.to_str().ok_or_else(|| RecordError::StreamError("Invalid file path".into()))?;

        info!("Creating recording pipeline for file: {}", location);

        // Create recording Bin: identity_tee -> queue -> identity_pre_parse -> h264parse -> mp4mux -> identity -> filesink
        let rec_bin = {
            let queue = ElementFactory::make("queue").build()?;
            let identity_pre_parse = ElementFactory::make("identity").property("signal-handoffs", &true).build()?;
            let parse = ElementFactory::make("h264parse").build()?;
            let mux = ElementFactory::make("mp4mux").property("streamable", &true).build()?;
            let identity = ElementFactory::make("identity").property("signal-handoffs", &true).build()?;
            let sink = ElementFactory::make("filesink").property("location", location).property("async", &false).build()?;

            // handoffシグナル: identity_pre_parse のみ
            let recording_id_clone2 = recording_id.clone();
            identity_pre_parse.connect("handoff", false, move |_values| {
                tracing::info!("[recording {}] [rec_bin] identity_pre_parse handoff: buffer arrived", recording_id_clone2);
                None
            });

            let bin = Bin::with_name(&format!("rec-bin-{}", recording_id));
            let add_result = bin.add_many(&[&queue, &identity_pre_parse, &parse, &mux, &identity, &sink]);
            tracing::info!("[recording {}] bin.add_many result: {:?}", recording_id, add_result);

            // ghost pad生成（queue.sink padをtargetに）
            // ghost pad生成前のqueue.sink pad状態を詳細出力
            let queue_sink_pad = queue.static_pad("sink").unwrap();
            tracing::info!("[recording {}] [before ghost] queue.sink: is_active={}, is_linked={}, caps={:?}",
                recording_id,
                queue_sink_pad.is_active(),
                queue_sink_pad.is_linked(),
                queue_sink_pad.current_caps()
            );

            // ghost pad生成（引数修正）
            let ghost_pad = GhostPad::with_target(&queue_sink_pad).unwrap();
            // 念のためactive化
            let _ = ghost_pad.set_active(true);
            tracing::info!("[recording {}] [after ghost] ghost_pad: is_active={}, is_linked={}, target={:?}, direction={:?}",
                recording_id,
                ghost_pad.is_active(),
                ghost_pad.is_linked(),
                ghost_pad.target().map(|p| p.name()),
                ghost_pad.direction()
            );
            let _ = bin.add_pad(&ghost_pad);

            let rec_bin = bin.name().to_string();
            tracing::info!("[recording {}] rec_bin created: {}", recording_id, rec_bin);

            // 直列リンク
            let link_result = Element::link_many(&[&queue, &identity_pre_parse, &parse, &mux, &identity, &sink]);
            tracing::info!("[recording {}] Element::link_many result: {:?}", recording_id, link_result);

            // dotファイル出力
            let dot_path = format!("/tmp/rec_bin_{}.dot", recording_id);
            bin.debug_to_dot_file(gstreamer::DebugGraphDetails::all(), &dot_path);
            if !std::path::Path::new(&dot_path).exists() {
                tracing::error!("[recording {}] rec_bin dot file not found after debug_to_dot_file: {}", recording_id, dot_path);
            } else {
                tracing::info!("[recording {}] rec_bin dot file written: {}", recording_id, dot_path);
            }
            bin
        };

        // Add recording Bin to the pipeline
        pipeline.add(&rec_bin)?;
        tracing::info!("[recording {}] rec_bin added to pipeline", recording_id);
        // Link the request pad of tee to the sink of the recording bin
        let tee_src_pad = tee.request_pad_simple("src_%u").unwrap();
        // 追加: tee_src_padにPadProbeType::BUFFERでprobe
        let recording_id_clone_probe = recording_id.clone();
        tee_src_pad.add_probe(PadProbeType::BUFFER, move |_, _| {
            tracing::info!("[recording {}] tee_src_pad BUFFER probe: buffer arrived", recording_id_clone_probe);
            PadProbeReturn::Ok
        });
        let rec_bin_sink_pad = rec_bin.static_pad("sink").unwrap();
        tracing::info!("[recording {}] tee_src_pad: name={}, is_linked={}", recording_id, tee_src_pad.name(), tee_src_pad.is_linked());
        tracing::info!("[recording {}] rec_bin_sink_pad: name={}, is_linked={}", recording_id, rec_bin_sink_pad.name(), rec_bin_sink_pad.is_linked());
        let link_res = tee_src_pad.link(&rec_bin_sink_pad);
        tracing::info!("[recording {}] tee_src_pad.link(rec_bin_sink_pad) result: {:?}", recording_id, link_res);
        tracing::info!("[recording {}] after link: tee_src_pad.is_linked={}, rec_bin_sink_pad.is_linked={}", recording_id, tee_src_pad.is_linked(), rec_bin_sink_pad.is_linked());
        // 追加: rec_bin_sink_padの詳細状態
        tracing::info!(
            "[recording {}] rec_bin_sink_pad after link: name={}, is_linked={}, is_active={}, peer={:?}, caps={:?}, direction={:?}",
            recording_id,
            rec_bin_sink_pad.name(),
            rec_bin_sink_pad.is_linked(),
            rec_bin_sink_pad.is_active(),
            rec_bin_sink_pad.peer().map(|p| p.name().to_string()),
            rec_bin_sink_pad.current_caps().map(|c| c.to_string()),
            rec_bin_sink_pad.direction()
        );
        // tee→rec_binリンク直後のpad状態を詳細出力
        tracing::info!("[recording {}] [after link] tee_src_pad: is_linked={}, caps={:?}, peer={:?}",
            recording_id,
            tee_src_pad.is_linked(),
            tee_src_pad.current_caps(),
            tee_src_pad.peer().map(|p| p.name())
        );
        tracing::info!("[recording {}] [after link] rec_bin_sink_pad: is_linked={}, is_active={}, caps={:?}, peer={:?}",
            recording_id,
            rec_bin_sink_pad.is_linked(),
            rec_bin_sink_pad.is_active(),
            rec_bin_sink_pad.current_caps(),
            rec_bin_sink_pad.peer().map(|p| p.name())
        );
        if link_res.is_err() {
            error!("Failed to link tee to recording bin: {:?}", link_res);
            return Err(RecordError::StreamError(format!("Failed to link tee to recording bin: {:?}", link_res)));
        }
        // Record the pad (for use when stopping)
        self.recording_pads.lock().await.insert(recording_id, tee_src_pad);
        // Sync the state of the recording Bin with the parent pipeline
        let sync_res = rec_bin.sync_state_with_parent();
        tracing::info!("[recording {}] rec_bin.sync_state_with_parent() result: {:?}", recording_id, sync_res);
        // 追加: sync_state_with_parent直後のrec_bin_sink_pad状態
        tracing::info!(
            "[recording {}] rec_bin_sink_pad after sync: name={}, is_linked={}, is_active={}, peer={:?}, caps={:?}, direction={:?}",
            recording_id,
            rec_bin_sink_pad.name(),
            rec_bin_sink_pad.is_linked(),
            rec_bin_sink_pad.is_active(),
            rec_bin_sink_pad.peer().map(|p| p.name().to_string()),
            rec_bin_sink_pad.current_caps().map(|c| c.to_string()),
            rec_bin_sink_pad.direction()
        );
        // dotファイル出力（状態遷移後に実施）
        let dot_path = format!("/tmp/rec_bin_{}.dot", recording_id);
        rec_bin.debug_to_dot_file(gstreamer::DebugGraphDetails::all(), &dot_path);
        if !std::path::Path::new(&dot_path).exists() {
            tracing::error!("[recording {}] rec_bin dot file not found after debug_to_dot_file: {}", recording_id, dot_path);
        } else {
            tracing::info!("[recording {}] rec_bin dot file written: {}", recording_id, dot_path);
        }
        // pipeline全体のdotファイルも出力
        let pipeline_dot_path = format!("/tmp/pipeline_{}.dot", recording_id);
        pipeline.debug_to_dot_file(gstreamer::DebugGraphDetails::all(), &pipeline_dot_path);
        if !std::path::Path::new(&pipeline_dot_path).exists() {
            tracing::error!("[recording {}] pipeline dot file not found after debug_to_dot_file: {}", recording_id, pipeline_dot_path);
        } else {
            tracing::info!("[recording {}] pipeline dot file written: {}", recording_id, pipeline_dot_path);
        }

        state.is_recording = true;
        state.current_recording_id = Some(recording_id);
        info!("Successfully started recording with ID: {}", recording_id);
        Ok(())
    }

    /// Stops recording.
    pub async fn stop_recording(&self) -> Result<Uuid, RecordError> {
        let mut state = self.state.lock().await;
        if !state.is_recording {
            return Err(RecordError::StreamError("No recording is in progress".into()));
        }
        let recording_id = state.current_recording_id.take().unwrap();
        info!(%recording_id, "Stopping recording");
        
        let pipeline = self.pipeline.lock().await;
        let pipeline = pipeline.as_ref().unwrap();

        let rec_bin = pipeline.by_name(&format!("rec-bin-{}", recording_id)).unwrap();

        let tee_src_pad = self.recording_pads.lock().await.remove(&recording_id).unwrap();

        let sink_pad = rec_bin.static_pad("sink").unwrap();
        let tee_peer_pad = sink_pad.peer().unwrap();

        tee_peer_pad.add_probe(PadProbeType::BLOCK_DOWNSTREAM, move |pad, _| {
            pad.send_event(event::Eos::new());
            PadProbeReturn::Remove
        });
        
        // This part needs to be synchronous to ensure the file is written
        std::thread::sleep(std::time::Duration::from_millis(500)); 

        pipeline.remove(&rec_bin)?;
        rec_bin.set_state(State::Null)?;

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