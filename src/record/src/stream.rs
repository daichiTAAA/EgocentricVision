use crate::config::Config;
use crate::error::RecordError;
use std::path::PathBuf;
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

/// ストリームの論理的な状態を保持します。
#[derive(Debug, Clone, Default)]
pub struct StreamState {
    pub is_connected: bool,
    pub is_recording: bool,
    pub protocol: Option<String>,
    pub url: Option<String>,
    pub current_recording_id: Option<Uuid>,
}

/// GStreamerパイプラインとストリーム状態を管理します。
// `tee` 要素を保持するために StreamManager を修正
pub struct StreamManager {
    state: Arc<Mutex<StreamState>>,
    pipeline: Arc<Mutex<Option<Pipeline>>>,
    // tee要素を保持するためのフィールドを追加
    tee: Arc<Mutex<Option<Element>>>,
    config: Config,
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
            tee: Arc::new(Mutex::new(None)), // teeを初期化
            config,
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

        // パイプラインを構築: rtspsrc -> ... -> tee
        let pipeline = Pipeline::new();
        let src = ElementFactory::make("rtspsrc")
            .property("location", &url)
            .property("latency", &0u32)
            .build()?;
        let tee = ElementFactory::make("tee").name("t").build()?;

        pipeline.add_many(&[&src, &tee])?;

        // pipeline_weakを事前に作成し、クロージャにはそれだけmoveする
        let pipeline_weak = pipeline.downgrade();
        let tee_clone = tee.clone();
        src.connect_pad_added(move |src_elem, src_pad| {
            info!("Received new pad '{}' from '{}'", src_pad.name(), src_elem.name());

            let sink_pad = tee_clone.static_pad("sink").expect("Tee should have a sink pad");
            if sink_pad.is_linked() {
                info!("Tee sink pad is already linked. Ignoring.");
                return;
            }

            // このサンプルではH.264を想定
            let depay = ElementFactory::make("rtph264depay").build().unwrap();
            let parse = ElementFactory::make("h264parse").build().unwrap();
            if let Some(p) = pipeline_weak.upgrade() {
                 p.add_many(&[&depay, &parse]).unwrap();
                 Element::link_many(&[&depay, &parse, &tee_clone]).unwrap();
                 src_pad.link(&depay.static_pad("sink").unwrap()).unwrap();

                 // 動的に追加した要素をSYNCING状態にする
                 depay.sync_state_with_parent().unwrap();
                 parse.sync_state_with_parent().unwrap();
                 
                 info!("Successfully linked rtspsrc to tee via depay and parse.");
            }
        });

        // パイプラインをPLAYINGに設定してストリーム受信を開始
        pipeline.set_state(State::Playing)
            .map_err(|e| RecordError::StreamError(format!("Failed to set pipeline to PLAYING: {}", e)))?;

        info!("Base pipeline created and set to PLAYING.");
        
        // 状態を更新
        *self.pipeline.lock().await = Some(pipeline);
        *self.tee.lock().await = Some(tee);
        state.is_connected = true;
        state.protocol = Some(protocol);
        state.url = Some(url.clone());
        
        Ok(())
    }

    /// 録画を開始します。
    pub async fn start_recording(&self, recording_id: Uuid) -> Result<(), RecordError> {
        let mut state = self.state.lock().await;
        
        if !state.is_connected {
            return Err(RecordError::NotConnected);
        }
        if state.is_recording {
            return Err(RecordError::AlreadyRecording);
        }

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
            let mux = ElementFactory::make("mp4mux").build()?;
            let sink = ElementFactory::make("filesink").property("location", location).build()?;

            bin.add_many(&[&queue, &mux, &sink])?;
            Element::link_many(&[&queue, &mux, &sink])?;

            let pad = queue.static_pad("sink").ok_or_else(|| RecordError::StreamError("Queue has no sink pad".into()))?;
            let ghost_pad = GhostPad::with_target(&pad)?;
            bin.add_pad(&ghost_pad)?;
            bin
        };

        // パイプラインに録画Binを追加し、teeとリンク
        pipeline.add(&rec_bin)?;
        tee.link(&rec_bin)?;

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

        // teeから録画Binへのパッドを取得し、ブロックプローブを追加してEOSを送信
        let sink_pad = rec_bin.static_pad("sink").unwrap();
        let tee_src_pad = sink_pad.peer().ok_or_else(|| RecordError::StreamError("Could not get peer pad from recording bin".into()))?;

        tee_src_pad.add_probe(PadProbeType::BLOCK_DOWNSTREAM, move |pad, _| {
            info!("Sending EOS to recording bin to finalize file.");
            // EOSの送信はpadではなく、要素のsink padに対して行う
            let peer_pad = pad.peer().unwrap();
            peer_pad.send_event(event::Eos::new());
            PadProbeReturn::Remove
        });

        // ファイル終端処理のために少し待機
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        // teeからBinを切り離し、パイプラインから削除
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