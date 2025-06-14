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
        let bus = match pipeline.bus() {
            Some(bus) => bus,
            None => {
                tracing::warn!("GStreamer pipeline bus not found");
                return Err(RecordError::StreamError("Pipeline bus not found".to_string()));
            }
        };
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

            // --- 追加: 各要素にPadProbeType::BUFFERでprobeを追加しバッファ到達をtracing出力 ---
            let rec_id_probe_parse = recording_id.clone();
            if let Some(parse_sink_pad) = parse.static_pad("sink") {
                parse_sink_pad.add_probe(PadProbeType::BUFFER, move |_, _| {
                    tracing::info!("[recording {}] [rec_bin] h264parse.sink BUFFER probe: buffer arrived", rec_id_probe_parse);
                    PadProbeReturn::Ok
                });
            } else {
                tracing::warn!("[record bin] h264parse sink pad not found at probe setup");
            }
            // mp4muxのvideo_0 padは1回だけ取得し、以降使い回す
            let mux_video_0_pad = mux.request_pad_simple("video_0");
            let rec_id_probe_mux = recording_id.clone();
            if let Some(ref mux_sink_pad) = mux_video_0_pad {
                mux_sink_pad.add_probe(PadProbeType::BUFFER, move |_, _| {
                    tracing::info!("[recording {}] [rec_bin] mp4mux.video_0 BUFFER probe: buffer arrived", rec_id_probe_mux);
                    PadProbeReturn::Ok
                });
            } else {
                tracing::warn!("[record bin] mp4mux video_0 pad not found at probe setup");
            }
            let rec_id_probe_identity = recording_id.clone();
            if let Some(identity_sink_pad) = identity.static_pad("sink") {
                identity_sink_pad.add_probe(PadProbeType::BUFFER, move |_, _| {
                    tracing::info!("[recording {}] [rec_bin] identity.sink BUFFER probe: buffer arrived", rec_id_probe_identity);
                    PadProbeReturn::Ok
                });
            } else {
                tracing::warn!("[record bin] identity sink pad not found at probe setup");
            }
            let rec_id_probe_filesink = recording_id.clone();
            // filesinkのsink padにPadProbeを追加（padが取得できた場合のみ）
            if let Some(filesink_sink_pad) = sink.static_pad("sink") {
                let _ = filesink_sink_pad.add_probe(PadProbeType::BUFFER, |pad, info| {
                    tracing::info!("[record bin] filesink sink pad: buffer arrived");
                    PadProbeReturn::Ok
                });
            } else {
                tracing::warn!("[record bin] filesink sink pad not found at probe setup");
            }
            // --- 追加ここまで ---

            let bin = Bin::with_name(&format!("rec-bin-{}", recording_id));
            let add_result = bin.add_many(&[&queue, &identity_pre_parse, &parse, &mux, &identity, &sink]);
            tracing::info!("[recording {}] bin.add_many result: {:?}", recording_id, add_result);

            // ghost pad生成・add・set_active・add_pad・link_manyの順序を厳密化
            // mp4muxのvideo_0 padはrequest_pad_simpleで1回だけ取得し使い回す
            // caps negotiation失敗時はtracingで警告
            let queue_sink_pad = match queue.static_pad("sink") {
                Some(pad) => pad,
                None => {
                    tracing::warn!("[recording {}] queue.sink pad not found at ghost pad setup", recording_id);
                    return Err(RecordError::StreamError("queue.sink pad not found".to_string()));
                }
            };
            tracing::info!("[recording {}] [before ghost] queue.sink: is_active={}, is_linked={}, caps={:?}, peer={:?}",
                recording_id,
                queue_sink_pad.is_active(),
                queue_sink_pad.is_linked(),
                queue_sink_pad.current_caps(),
                queue_sink_pad.peer().map(|p| p.name())
            );

            // ghost pad生成
            let ghost_pad = match GhostPad::with_target(&queue_sink_pad) {
                Ok(pad) => pad,
                Err(e) => {
                    tracing::warn!("[recording {}] ghost pad creation failed: {}", recording_id, e);
                    return Err(RecordError::StreamError("ghost pad creation failed".to_string()));
                }
            };
            tracing::info!("[recording {}] [after ghost create] ghost_pad: is_active={}, is_linked={}, target={:?}, direction={:?}",
                recording_id,
                ghost_pad.is_active(),
                ghost_pad.is_linked(),
                ghost_pad.target().map(|p| p.name()),
                ghost_pad.direction()
            );
            // active化
            let _ = ghost_pad.set_active(true);
            tracing::info!("[recording {}] [after ghost set_active] ghost_pad: is_active={}, is_linked={}, target={:?}, direction={:?}",
                recording_id,
                ghost_pad.is_active(),
                ghost_pad.is_linked(),
                ghost_pad.target().map(|p| p.name()),
                ghost_pad.direction()
            );
            let add_pad_res = bin.add_pad(&ghost_pad);
            tracing::info!("[recording {}] [after bin.add_pad] ghost_pad: is_active={}, is_linked={}, target={:?}, direction={:?}, add_pad_res={:?}",
                recording_id,
                ghost_pad.is_active(),
                ghost_pad.is_linked(),
                ghost_pad.target().map(|p| p.name()),
                ghost_pad.direction(),
                add_pad_res
            );

            let rec_bin = bin.name().to_string();
            tracing::info!("[recording {}] rec_bin created: {}", recording_id, rec_bin);

            // 直列リンク
            let link_result = Element::link_many(&[&queue, &identity_pre_parse, &parse, &mux, &identity, &sink]);
            tracing::info!("[recording {}] Element::link_many result: {:?}", recording_id, link_result);
            // link_many直後のqueue.sink, identity_pre_parse.sink, parse.sink, mux.video_0, identity.sink, filesink.sinkのpad状態を出力
            let pads = [
                ("queue.sink", queue.static_pad("sink")),
                ("identity_pre_parse.sink", identity_pre_parse.static_pad("sink")),
                ("parse.sink", parse.static_pad("sink")),
                ("mux.video_0", mux_video_0_pad.clone()),
                ("identity.sink", identity.static_pad("sink")),
                ("filesink.sink", sink.static_pad("sink")),
            ];
            for (name, pad_opt) in pads.iter() {
                if let Some(pad) = pad_opt {
                    tracing::info!("[recording {}] [after link_many] {}: is_active={}, is_linked={}, caps={:?}, peer={:?}",
                        recording_id,
                        name,
                        pad.is_active(),
                        pad.is_linked(),
                        pad.current_caps(),
                        pad.peer().map(|p| p.name())
                    );
                } else {
                    tracing::warn!("[recording {}] [after link_many] {}: pad not found", recording_id, name);
                }
            }

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
        let tee_src_pad = match tee.request_pad_simple("src_%u") {
            Some(pad) => pad,
            None => {
                tracing::warn!("[recording {}] tee src pad request failed", recording_id);
                return Err(RecordError::StreamError("tee src pad request failed".to_string()));
            }
        };
        // 追加: tee_src_padのpad mode/capsをtracing出力
        let tee_mode = tee_src_pad.mode();
        let tee_caps = tee_src_pad.current_caps();
        tracing::info!("[recording {}] tee_src_pad: mode={:?}, caps={:?}", recording_id, tee_mode, tee_caps);
        // 追加: tee_src_padにPadProbeType::BUFFERでprobe
        let recording_id_clone_probe = recording_id.clone();
        tee_src_pad.add_probe(PadProbeType::BUFFER, move |_, _| {
            tracing::info!("[recording {}] tee_src_pad BUFFER probe: buffer arrived", recording_id_clone_probe);
            PadProbeReturn::Ok
        });
        // 追加: tee_src_pad/rec_bin_sink_padのactivation mode, capsをtracing出力
        let tee_src_pad_mode = tee_src_pad.mode();
        tracing::info!("[recording {}] tee_src_pad: mode={:?}, caps={:?}", recording_id, tee_src_pad_mode, tee_src_pad.current_caps());
        let rec_bin_sink_pad = match rec_bin.static_pad("sink") {
            Some(pad) => pad,
            None => {
                tracing::warn!("[recording {}] rec_bin sink pad not found after add", recording_id);
                return Err(RecordError::StreamError("rec_bin sink pad not found".to_string()));
            }
        };
        // 追加: rec_bin_sink_padのpad mode/capsをtracing出力
        let rec_bin_mode = rec_bin_sink_pad.mode();
        let rec_bin_caps = rec_bin_sink_pad.current_caps();
        tracing::info!("[recording {}] rec_bin_sink_pad: mode={:?}, caps={:?}", recording_id, rec_bin_mode, rec_bin_caps);
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

        let tee_src_pad = match self.recording_pads.lock().await.remove(&recording_id) {
            Some(pad) => pad,
            None => {
                tracing::warn!("tee_src_pad not found for recording_id={}", recording_id);
                return Err(RecordError::StreamError("tee_src_pad not found".to_string()));
            }
        };

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