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

/// ストリームの論理的な状態を保持します。
#[derive(Debug, Clone, Default)]
pub struct StreamState {
    pub is_connected: bool,
    pub is_recording: bool,
    pub protocol: Option<String>,
    pub url: Option<String>,
    pub current_recording_id: Option<Uuid>,
    pub is_tee_ready: bool, // 追加
}

/// GStreamerパイプラインとストリーム状態を管理します。
// `tee` 要素を保持するために StreamManager を修正
pub struct StreamManager {
    state: Arc<Mutex<StreamState>>,
    pipeline: Arc<Mutex<Option<Pipeline>>>,
    // tee要素を保持するためのフィールドを追加
    tee: Arc<Mutex<Option<Element>>>,
    config: Config,
    recording_pads: Arc<Mutex<HashMap<Uuid, gstreamer::Pad>>>,
    is_tee_ready: Arc<AtomicBool>, // 追加
}

impl StreamManager {
    /// 新しいStreamManagerインスタンスを作成し、GStreamerを初期化します。
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
            is_tee_ready: Arc::new(AtomicBool::new(false)), // 追加
        }
    }

    /// 現在のストリーム状態を返します。
    pub async fn get_status(&self) -> StreamState {
        self.state.lock().await.clone()
    }

    /// ストリーム接続状態を返す
    pub async fn is_connected(&self) -> bool {
        self.state.lock().await.is_connected
    }
    /// 録画状態を返す
    pub async fn is_recording(&self) -> bool {
        self.state.lock().await.is_recording
    }

    /// RTSPストリームに接続し、再生準備ができたパイプラインを構築します。
    pub async fn connect(&self, protocol: String, url: String) -> Result<(), RecordError> {
        let mut state = self.state.lock().await;
        if state.is_connected {
            return Err(RecordError::StreamError("Already connected to a stream".to_string()));
        }

        info!(%url, "Connecting to stream and creating base pipeline");

        // パイプラインを構築: rtspsrc -> rtph264depay -> h264parse -> tee
        let pipeline = Pipeline::new();
        let src = ElementFactory::make("rtspsrc")
            .property("location", &url)
            .property("latency", &0u32)
            .build()?;
        let depay = ElementFactory::make("rtph264depay").build()?;
        let parse = ElementFactory::make("h264parse").build()?;
        let tee = ElementFactory::make("tee").name("t").build()?;
        pipeline.add_many(&[&src, &depay, &parse, &tee])?;
        // h264parse→teeのリンク時にcapフィルタを使用
        use gstreamer::Caps;
        let caps = Caps::builder("video/x-h264")
            .field("stream-format", &"avc")
            .field("alignment", &"au")
            .build();
        Element::link_many(&[&depay, &parse])?;
        parse.link_filtered(&tee, &caps)?;

        // bus監視を追加（StateChangedも全要素で出す）
        let bus = pipeline.bus().unwrap();
        let _ = bus.add_watch(move |_, msg| {
            use gstreamer::MessageView;
            match msg.view() {
                MessageView::Error(err) => {
                    error!(
                        "Error from element {}: {} ({})",
                        err.src().map_or_else(|| "None".to_string(), |s| s.path_string().to_string()),
                        err.error(),
                        err.debug().unwrap_or_default()
                    );
                }
                MessageView::Warning(warn) => {
                    warn!(
                        "Warning from element {}: {} ({})",
                        warn.src().map_or_else(|| "None".to_string(), |s| s.path_string().to_string()),
                        warn.error(),
                        warn.debug().unwrap_or_default()
                    );
                }
                MessageView::Eos(..) => {
                    debug!("Received EOS");
                }
                MessageView::StateChanged(state_changed) => {
                    if let Some(src) = state_changed.src() {
                        debug!(
                            "Element {} state changed from {:?} to {:?} to {:?}",
                            src.path_string(),
                            state_changed.old(),
                            state_changed.current(),
                            state_changed.pending()
                        );
                    }
                }
                _ => (),
            }
            glib::ControlFlow::Continue
        }).expect("Failed to add bus watch");

        // pipeline_weakを事前に作成し、クロージャにはそれだけmoveする
        let depay_clone = depay.clone();
        let is_tee_ready_clone = self.is_tee_ready.clone();
        src.connect_pad_added(move |src_elem, src_pad| {
            info!("Received new pad '{}' from '{}'", src_pad.name(), src_elem.name());
            let new_pad_caps = match src_pad.current_caps() {
                Some(caps) => caps,
                None => {
                    warn!("No caps on new pad, ignoring");
                    return;
                }
            };
            let new_pad_struct = match new_pad_caps.structure(0) {
                Some(s) => s,
                None => {
                    warn!("No structure in caps, ignoring");
                    return;
                }
            };
            info!("New pad created with caps: {}", new_pad_struct.to_string());
            if new_pad_struct.name() == "application/x-rtp"
                && new_pad_struct.get::<&str>("media").unwrap_or("") == "video"
                && new_pad_struct.get::<&str>("encoding-name").unwrap_or("") == "H264"
            {
                let depay_sink = depay_clone.static_pad("sink").unwrap();
                if depay_sink.is_linked() {
                    warn!("Depay sink pad already linked. Ignoring.");
                    return;
                }
                match src_pad.link(&depay_sink) {
                    Ok(_) => info!("Linked src_pad to depay sink"),
                    Err(e) => error!("Failed to link src_pad to depay sink: {:?}", e),
                }
                is_tee_ready_clone.store(true, Ordering::SeqCst); // ここでセット
            } else {
                info!("Ignoring non-H264 video pad.");
            }
        });

        // パイプラインをPLAYINGに設定してストリーム受信を開始
        pipeline.set_state(State::Playing)
            .map_err(|e| RecordError::StreamError(format!("Failed to set pipeline to PLAYING: {}", e)))?;

        info!("Base pipeline created and set to PLAYING.");
        *self.pipeline.lock().await = Some(pipeline);
        *self.tee.lock().await = Some(tee);
        state.is_connected = true;
        state.protocol = Some(protocol);
        state.url = Some(url.clone());
        self.is_tee_ready.store(false, Ordering::SeqCst); // 初期化
        Ok(())
    }

    /// 録画を開始します。
    pub async fn start_recording(&self, recording_id: Uuid) -> Result<(), RecordError> {
        let state = self.state.lock().await;
        
        if !state.is_connected {
            return Err(RecordError::NotConnected);
        }
        if state.is_recording {
            return Err(RecordError::AlreadyRecording);
        }
        drop(state); // ここで一度ロックを外す
        // teeまでのリンクが完了するまで待機
        let mut wait_count = 0;
        while !self.is_tee_ready.load(Ordering::SeqCst) {
            if wait_count > 20 {
                return Err(RecordError::StreamError("Tee is not ready for recording".into()));
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            wait_count += 1;
        }
        let mut state = self.state.lock().await;

        let pipeline_lock = self.pipeline.lock().await;
        let pipeline = pipeline_lock.as_ref().ok_or_else(|| RecordError::StreamError("Pipeline not found".into()))?;

        let tee_lock = self.tee.lock().await;
        let tee = tee_lock.as_ref().ok_or_else(|| RecordError::StreamError("Tee element not found".into()))?;

        // ファイルパスを生成
        let mut path = PathBuf::from(&self.config.recording_directory);
        tokio::fs::create_dir_all(&path).await?;
        path.push(format!("{}.mp4", recording_id));
        let location = path.to_str().ok_or_else(|| RecordError::StreamError("Invalid file path".into()))?;

        info!(%location, "Starting recording");

        // 録画用Binを作成: queue -> mp4mux -> filesink
        let rec_bin_name = format!("rec-bin-{}", recording_id);
        let rec_bin = {
            let bin = Bin::with_name(&rec_bin_name);
            let queue = ElementFactory::make("queue").build()?;
            let capsfilter = ElementFactory::make("capsfilter")
                .property("caps", &gstreamer::Caps::builder("video/x-h264")
                    .field("stream-format", &"avc")
                    .field("alignment", &"au")
                    .build())
                .build()?;
            let mux = ElementFactory::make("mp4mux").build()?;
            let sink = ElementFactory::make("filesink").property("location", location).build()?;

            bin.add_many(&[&queue, &capsfilter, &mux, &sink])?;
            Element::link_many(&[&queue, &capsfilter, &mux, &sink])?;

            let pad = queue.static_pad("sink").ok_or_else(|| RecordError::StreamError("Queue has no sink pad".into()))?;
            let ghost_pad = GhostPad::with_target(&pad)?;
            bin.add_pad(&ghost_pad)?;
            bin
        };

        // パイプラインに録画Binを追加
        pipeline.add(&rec_bin)?;
        // teeのrequest padを取得し、録画binのsinkにリンク
        let tee_src_pad = tee.request_pad_simple("src_%u").ok_or_else(|| RecordError::StreamError("Failed to get tee request pad".into()))?;
        let rec_bin_sink_pad = rec_bin.static_pad("sink").ok_or_else(|| RecordError::StreamError("Recording bin has no sink pad".into()))?;
        tee_src_pad.link(&rec_bin_sink_pad).map_err(|_| RecordError::StreamError(format!("Failed to link elements 't' and '{}'", rec_bin_name)))?;
        // padを記録
        self.recording_pads.lock().await.insert(recording_id, tee_src_pad);
        // 録画Binを親パイプラインの状態に同期
        rec_bin.sync_state_with_parent()?;
        
        state.is_recording = true;
        state.current_recording_id = Some(recording_id);
        info!("Successfully started recording with ID: {}", recording_id);

        Ok(())
    }

    /// 録画を停止します。
    pub async fn stop_recording(&self) -> Result<Uuid, RecordError> {
        let mut state = self.state.lock().await;
        if !state.is_recording {
            return Err(RecordError::StreamError("No recording is in progress".into()));
        }
        let recording_id = state.current_recording_id.take().ok_or_else(|| RecordError::StreamError("No current recording id found".into()))?;
        info!(%recording_id, "Stopping recording");
        let pipeline_lock = self.pipeline.lock().await;
        let pipeline = pipeline_lock.as_ref().ok_or_else(|| RecordError::StreamError("Pipeline not found.".into()))?;
        let rec_bin_name = format!("rec-bin-{}", recording_id);
        let rec_bin = pipeline.by_name(&rec_bin_name)
            .ok_or_else(|| RecordError::StreamError(format!("Could not find bin '{}'", rec_bin_name)))?;
        let tee_lock = self.tee.lock().await;
        let tee = tee_lock.as_ref().ok_or_else(|| RecordError::StreamError("Tee element not found.".into()))?;
        let bus = pipeline.bus().unwrap();
        let (eos_tx, mut eos_rx) = tokio::sync::mpsc::channel(1);
        let rec_bin_name_clone = rec_bin_name.clone();
        let _filesink_name = format!("rec-bin-{}", recording_id); // bin名
        let _rec_bin_elem = rec_bin.clone();
        // 別タスクでbusを監視
        tokio::spawn(async move {
            for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
                use gstreamer::MessageView;
                if let MessageView::Eos(_eos) = msg.view() {
                    // 録画bin配下のEOSかどうか判定
                    if let Some(src) = msg.src() {
                        let path = src.path_string();
                        if path.contains(&rec_bin_name_clone) || path.contains(&_filesink_name) {
                            let _ = eos_tx.send(()).await;
                            break;
                        }
                    }
                }
            }
        });
        // teeから録画Binへのパッドを取得し、ブロックプローブを追加してEOSを送信
        let _sink_pad = rec_bin.static_pad("sink").unwrap();
        let tee_src_pad = self.recording_pads.lock().await.remove(&recording_id)
            .ok_or_else(|| RecordError::StreamError("Could not get tee request pad for recording".into()))?;
        tee_src_pad.add_probe(PadProbeType::BLOCK_DOWNSTREAM, move |pad, _| {
            info!("Sending EOS to recording bin to finalize file.");
            let peer_pad = pad.peer().unwrap();
            peer_pad.send_event(event::Eos::new());
            PadProbeReturn::Remove
        });
        // busからEOSを受信するまで待つ
        eos_rx.recv().await;
        tee.release_request_pad(&tee_src_pad);
        pipeline.remove(&rec_bin)?;
        rec_bin.set_state(State::Null)?;
        info!(%recording_id, "Recording bin removed and file saved.");
        state.is_recording = false;
        Ok(recording_id)
    }

    /// ストリームから切断し、パイプラインを停止・破棄します。
    pub async fn disconnect(&self) -> Result<(), RecordError> {
        let mut state = self.state.lock().await;
        if !state.is_connected {
            warn!("Not connected, nothing to do.");
            return Ok(());
        }

        // 録画中であれば停止する
        if state.is_recording {
            warn!("Recording was in progress during disconnect. Stopping it first.");
            // Mutexのロックを一度解放してstop_recordingを呼べるようにする
            drop(state);
            if let Err(e) = self.stop_recording().await {
                 error!("Failed to stop recording during disconnect: {}", e);
            }
            state = self.state.lock().await;
        }
        
        info!("Disconnecting from stream and stopping pipeline...");
        
        let mut pipeline_lock = self.pipeline.lock().await;
        if let Some(pipeline) = pipeline_lock.take() {
            pipeline.set_state(State::Null)?;
            info!("Pipeline stopped and destroyed successfully.");
        }

        *state = StreamState::default();
        *self.tee.lock().await = None; // teeもクリア

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