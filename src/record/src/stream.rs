use crate::config::Config;
use crate::error::RecordError;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;
use gstreamer::prelude::*;
use gstreamer::{Bin, Element, ElementFactory, GhostPad, Pipeline, State};
use gstreamer::{StateChangeError};
use gstreamer::parse;
use tracing::{info, warn};
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
#[derive(Debug)]
pub struct StreamManager {
    state: Arc<Mutex<StreamState>>,
    pipeline: Arc<Mutex<Option<Pipeline>>>,
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
        let mut pipeline_lock = self.pipeline.lock().await;

        if state.is_connected {
            return Err(RecordError::StreamError(
                "Already connected to a stream".to_string(),
            ));
        }

        info!(%url, "Connecting to stream");

        // パイプラインを手動で構築し、rtspsrcのpad-addedシグナルをハンドル
        let pipeline = Pipeline::new();
        let src = ElementFactory::make("rtspsrc").property("location", &url).property("latency", &0u32).build()?;
        let depay = ElementFactory::make("rtph264depay").build()?;
        let parse = ElementFactory::make("h264parse").build()?;
        let mux = ElementFactory::make("mp4mux").build()?;
        let sink = ElementFactory::make("filesink").property("location", "/tmp/test.mp4").build()?;

        pipeline.add_many(&[&src, &depay, &parse, &mux, &sink])?;
        Element::link_many(&[&depay, &parse, &mux, &sink])?;

        // rtspsrcのpad-addedシグナルでdepayチェーンに接続
        let depay_clone = depay.downgrade();
        src.connect_pad_added(move |src, src_pad| {
            if let Some(depay) = depay_clone.upgrade() {
                let sink_pad = depay.static_pad("sink").unwrap();
                if sink_pad.is_linked() {
                    return;
                }
                let _ = src_pad.link(&sink_pad);
            }
        });

        pipeline.set_state(State::Paused)
            .map_err(|e| RecordError::StreamError(e.to_string()))?;

        info!("Successfully created and paused pipeline.");

        *pipeline_lock = Some(pipeline);
        state.is_connected = true;
        state.protocol = Some(protocol);
        state.url = Some(url);

        Ok(())
    }

    /// 録画を開始します。
    pub async fn start_recording(&self, recording_id: Uuid) -> Result<(), RecordError> {
        let mut state = self.state.lock().await;
        let pipeline_lock = self.pipeline.lock().await;

        if !state.is_connected {
            return Err(RecordError::StreamError(
                "Not connected to any stream".to_string(),
            ));
        }
        if state.is_recording {
            return Err(RecordError::StreamError(
                "Recording is already in progress".to_string(),
            ));
        }

        let pipeline = pipeline_lock.as_ref().ok_or_else(|| {
            RecordError::StreamError("Pipeline not found, connect to a stream first.".to_string())
        })?;

        // 設定ファイルから保存先パスを取得し、ファイル名を生成します。
        let mut path = PathBuf::from(&self.config.recording_directory);
        tokio::fs::create_dir_all(&path).await?; // 保存ディレクトリをなければ作成
        path.push(format!("{}.mp4", recording_id));
        let location = path.to_str().ok_or_else(|| {
            RecordError::StreamError("Invalid file path for recording".to_string())
        })?;

        info!(location, "Starting recording");

        // 録画用のパイプライン部品（Bin）を動的に作成します。
        let rec_bin = {
            let bin = Bin::new();
            let queue = ElementFactory::make("queue").build()?;
            let mux = ElementFactory::make("mp4mux").build()?;
            let sink = ElementFactory::make("filesink").build()?;
            sink.set_property("location", location);

            bin.add_many(&[&queue, &mux, &sink])?;
            Element::link_many(&[&queue, &mux, &sink])?;

            // Binの入力パッドを作成
            let pad = queue.static_pad("sink").ok_or_else(|| RecordError::StreamError("No static pad 'sink' on queue".to_string()))?;
            let ghost_pad = GhostPad::with_target(&pad)?;
            bin.add_pad(&ghost_pad)?;
            bin
        };

        pipeline.add(&rec_bin)?;

        // teeやrec_binの追加・リンク処理は不要
        // パイプラインがまだ再生中でなければ、PLAYING状態に移行します。
        if pipeline.current_state() != State::Playing {
            pipeline.set_state(State::Playing)?;
            info!("Pipeline state changed to PLAYING");
        }
        state.is_recording = true;
        state.current_recording_id = Some(recording_id);
        Ok(())
    }

    /// 録画を停止します。
    pub async fn stop_recording(&self) -> Result<Uuid, RecordError> {
        let mut state = self.state.lock().await;
        let pipeline_lock = self.pipeline.lock().await;

        if !state.is_recording {
            return Err(RecordError::StreamError(
                "No recording is currently in progress".to_string(),
            ));
        }

        let recording_id = state.current_recording_id.take().ok_or_else(|| RecordError::StreamError("No current recording id".to_string()))?;
        info!(%recording_id, "Stopping recording");

        let pipeline = pipeline_lock
            .as_ref()
            .ok_or_else(|| RecordError::StreamError("Pipeline not found.".to_string()))?;

        // 録画用Binとtee要素を取得
        let bin_name = format!("rec-bin-{}", recording_id);
        let rec_bin = pipeline
            .by_name(&bin_name)
            .ok_or_else(|| RecordError::StreamError(format!("Could not find bin '{}'", bin_name)))?;
        let tee = pipeline
            .by_name("t")
            .ok_or_else(|| RecordError::StreamError("Could not find 'tee' element".to_string()))?;

        // teeから録画Binへのデータフローをブロックし、EOS(End-of-Stream)イベントを送信します。
        let tee_src_pad = rec_bin.static_pad("sink").and_then(|p| p.peer()).ok_or_else(|| RecordError::StreamError("Could not get peer pad".to_string()))?;
        tee_src_pad.add_probe(gstreamer::PadProbeType::BLOCK_DOWNSTREAM, move |pad, _| {
            info!("Sending EOS to recording bin to finalize file.");
            pad.send_event(gstreamer::event::Eos::new());
            gstreamer::PadProbeReturn::Remove
        });

        // ファイル終端処理のために少し待機します。
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // teeからBinを切り離し、パイプラインから削除します。
        tee.unlink(&rec_bin);
        pipeline.remove(&rec_bin)?;
        rec_bin.set_state(State::Null)?;
        
        info!(%recording_id, "Recording bin removed and file saved.");

        // パイプラインにEOSイベントを送信
        pipeline.send_event(gstreamer::event::Eos::new());
        // BusでEOSメッセージを待つ
        if let Some(bus) = pipeline.bus() {
            for msg in bus.iter_timed(gstreamer::ClockTime::NONE) {
                if let gstreamer::MessageView::Eos(..) = msg.view() {
                    break;
                }
            }
        }
        pipeline.set_state(State::Null)?;
        info!(%recording_id, "Pipeline set to Null and file finalized.");
        state.is_recording = false;
        Ok(recording_id)
    }

    /// ストリームから切断し、パイプラインを停止・破棄します。
    pub async fn disconnect(&self) -> Result<(), RecordError> {
        let mut state = self.state.lock().await;
        let mut pipeline_lock = self.pipeline.lock().await;

        if !state.is_connected {
            warn!("Not connected, nothing to do.");
            return Ok(());
        }

        if let Some(pipeline) = pipeline_lock.take() {
            // 録画中であれば、まず録画を停止する
            if state.is_recording {
                warn!("Recording was in progress during disconnect. Stopping it first.");
                drop(state);
                drop(pipeline_lock);
                self.stop_recording().await?;
                state = self.state.lock().await;
            }

            info!("Disconnecting from stream and stopping pipeline...");
            pipeline.set_state(State::Null)?;
            info!("Pipeline stopped successfully.");
        }

        *state = StreamState::default();

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