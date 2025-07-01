use crate::config::Config;
use crate::error::RecordError;
use crate::models::DebugStatus;
use crate::models::StreamStatus;
use crate::recording::start_recording_impl;
use crate::webrtc::start_webrtc_streaming_impl;
use glib::BoolError;
use glib::ControlFlow;
use gstreamer::prelude::*;
use gstreamer::{Element, ElementFactory, MessageView, Pipeline, State, StateChangeError};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::MutexGuard;
use tracing::{error, info, warn};

/// ストリームを識別するためのID
pub type StreamId = String;

/// Stores the logical state of the stream.
#[derive(Debug, Clone, Default)]
pub struct StreamState {
    pub is_connected: bool,
    pub is_recording: bool,
    pub protocol: Option<String>,
    pub url: Option<String>,
    pub current_recording_id: Option<String>,
    // pub is_tee_ready: bool, // 未使用のためコメントアウト
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
            // is_tee_ready: false, // 未使用のためコメントアウト
            pipeline: None,
            tee: None,
        }
    }

    #[allow(dead_code)]
    pub async fn start_recording(
        &mut self,
        _recording_id: &str,
        _location: &str,
    ) -> Result<(), RecordError> {
        unimplemented!("StreamState::start_recordingはStreamManager経由で呼び出してください");
    }

    #[allow(dead_code)]
    pub async fn start_webrtc_streaming(&mut self) -> Result<gstreamer::Element, RecordError> {
        start_webrtc_streaming_impl(self.is_connected, self.pipeline.as_ref(), self.tee.as_ref())
            .await
    }
}

/// Manages the GStreamer pipeline and stream state.
#[allow(dead_code)]
pub struct StreamManager {
    streams: Arc<Mutex<HashMap<StreamId, StreamState>>>,
    #[allow(dead_code)]
    config: Config,
    recording_pads: Arc<Mutex<HashMap<String, gstreamer::Pad>>>,
    #[allow(dead_code)]
    is_tee_ready: Arc<AtomicBool>,
}

impl StreamManager {
    /// Creates a new StreamManager instance and initializes GStreamer.
    pub fn new(config: Config) -> Self {
        if let Err(e) = gstreamer::init() {
            panic!("Failed to initialize GStreamer: {}", e);
        }
        Self {
            streams: Arc::new(Mutex::new(HashMap::new())),
            config,
            recording_pads: Arc::new(Mutex::new(HashMap::new())),
            is_tee_ready: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 指定したstream_idのStreamStateへのミュータブル参照を取得
    pub async fn get_stream_mut(
        &self,
        _stream_id: &StreamId,
    ) -> Option<MutexGuard<'_, HashMap<StreamId, StreamState>>> {
        Some(self.streams.lock().await)
    }

    /// Returns the current stream status for a specific stream.
    pub async fn get_status(&self, stream_id: &StreamId) -> Option<StreamState> {
        self.streams.lock().await.get(stream_id).cloned()
    }

    /// Returns all stream statuses.
    pub async fn get_all_statuses(&self) -> HashMap<StreamId, StreamStatus> {
        self.streams
            .lock()
            .await
            .iter()
            .map(|(id, state)| (id.clone(), StreamStatus::from(state)))
            .collect()
    }

    /// Returns the stream connection status for a specific stream
    #[allow(dead_code)]
    pub async fn is_connected(&self, stream_id: &StreamId) -> bool {
        self.streams
            .lock()
            .await
            .get(stream_id)
            .map(|state| state.is_connected)
            .unwrap_or(false)
    }

    /// Returns the recording status for a specific stream
    #[allow(dead_code)]
    pub async fn is_recording(&self, stream_id: &StreamId) -> bool {
        self.streams
            .lock()
            .await
            .get(stream_id)
            .map(|state| state.is_recording)
            .unwrap_or(false)
    }

    /// Returns detailed status of the pipeline and Tee for a specific stream
    pub async fn get_detailed_status(&self, stream_id: &StreamId) -> Option<DebugStatus> {
        let streams = self.streams.lock().await;
        let state = streams.get(stream_id)?;

        let pipeline_state = state
            .pipeline
            .as_ref()
            .map(|p| p.state(gstreamer::ClockTime::ZERO));
        let tee_state = state
            .tee
            .as_ref()
            .map(|t| t.state(gstreamer::ClockTime::ZERO));

        let (pipeline_current, pipeline_pending) =
            if let Some((_, current, pending)) = pipeline_state {
                (
                    Some(format!("{:?}", current)),
                    Some(format!("{:?}", pending)),
                )
            } else {
                (None, None)
            };

        let (tee_current, tee_pending) = if let Some((_, current, pending)) = tee_state {
            (
                Some(format!("{:?}", current)),
                Some(format!("{:?}", pending)),
            )
        } else {
            (None, None)
        };

        let recording_pads = self.recording_pads.lock().await;

        Some(DebugStatus {
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
        })
    }

    /// Connects to an RTSP stream and builds a pipeline ready for playback.
    pub async fn connect(
        &self,
        stream_id: StreamId,
        protocol: String,
        url: String,
    ) -> Result<(), RecordError> {
        let mut streams = self.streams.lock().await;

        if streams.contains_key(&stream_id) {
            return Err(RecordError::StreamError(format!(
                "Stream ID {} already exists",
                stream_id
            )));
        }

        info!(%stream_id, %url, "Connecting to stream and creating base pipeline");

        // Build the pipeline: rtspsrc -> identity_src -> rtph264depay -> h264parse -> tee
        let pipeline = Pipeline::new();
        let src = ElementFactory::make("rtspsrc")
            .property("location", &url)
            .property("latency", 0u32)
            .property("timeout", 120000u64) // タイムアウトを120秒に増やす
            .property("retry", 5u32) // リトライ回数を5回に増やす
            .property("do-retransmission", true)
            .property("ntp-sync", true)
            .property("drop-on-latency", true)
            .property("tcp-timeout", 10000000u64) // TCPタイムアウトを10秒に設定
            .property("user-id", "") // 認証情報が必要な場合は設定
            .property("user-pw", "") // 認証情報が必要な場合は設定
            .property("udp-buffer-size", 524288i32) // UDPバッファサイズを設定
            .build()?;

        // buffer-modeはset_propertyで設定
        src.set_property_from_str("buffer-mode", "auto");

        // バッファサイズを増やすためのqueue要素を追加
        let queue = ElementFactory::make("queue")
            .property("max-size-buffers", 1000u32)
            .property("max-size-bytes", 0u32)
            .property("max-size-time", 0u64)
            .build()?;

        // leakyはset_propertyで設定
        queue.set_property_from_str("leaky", "downstream");

        let identity_src = ElementFactory::make("identity")
            .property("signal-handoffs", true)
            .property("silent", false)
            .build()?;

        let depay = ElementFactory::make("rtph264depay")
            .property("wait-for-keyframe", true)
            .build()?;

        let parse = ElementFactory::make("h264parse")
            .property("config-interval", -1i32)
            .property("disable-passthrough", true)
            .build()?;

        let tee = ElementFactory::make("tee")
            .property("allow-not-linked", true)
            .property("silent", false)
            .build()?;

        // パイプラインに要素を追加
        pipeline.add_many([&src, &queue, &identity_src, &depay, &parse, &tee])?;

        // pad-addedシグナルでidentity_srcのsinkパッドにリンク
        let identity_src_clone = identity_src.clone();
        src.connect_pad_added(move |_src, src_pad| {
            let sink_pad = identity_src_clone.static_pad("sink").unwrap();
            if sink_pad.is_linked() {
                return;
            }
            match src_pad.link(&sink_pad) {
                Ok(_) => info!("Linked rtspsrc to identity"),
                Err(err) => error!("Failed to link rtspsrc to identity: {:?}", err),
            }
        });

        // 要素をリンク
        Element::link_many([&identity_src, &queue, &depay, &parse, &tee])?;

        // identity_src handoff
        let is_tee_ready_clone2 = self.is_tee_ready.clone();
        identity_src.connect("handoff", false, move |_values| {
            tracing::info!("[base pipeline] identity_src handoff: buffer arrived");
            is_tee_ready_clone2.store(true, Ordering::SeqCst);
            None
        });

        // Add bus watch
        let bus = pipeline.bus().unwrap();
        let pipeline_clone = pipeline.clone();
        let _watch_id = bus.add_watch(move |_, msg| match msg.view() {
            MessageView::Error(err) => {
                error!("Pipeline error: {}", err.error());
                ControlFlow::Continue
            }
            MessageView::Warning(warn) => {
                warn!("Pipeline warning: {}", warn.error());
                ControlFlow::Continue
            }
            MessageView::StateChanged(state) => {
                if state
                    .src()
                    .map(|s| std::ptr::eq(s, pipeline_clone.upcast_ref()))
                    .unwrap_or(false)
                {
                    let current = state.current();
                    let pending = state.pending();
                    info!("Pipeline state changed: {:?} -> {:?}", current, pending);
                }
                ControlFlow::Continue
            }
            MessageView::Eos(..) => {
                info!("Pipeline EOS");
                ControlFlow::Continue
            }
            _ => ControlFlow::Continue,
        })?;

        // パイプラインを開始
        pipeline.set_state(State::Playing)?;

        // 状態遷移の完了を待機
        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(30);
        let mut state_changed = false;

        while start_time.elapsed() < timeout {
            let (_, current_state, _) = pipeline.state(gstreamer::ClockTime::from_mseconds(100));
            if current_state == State::Playing {
                state_changed = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        if !state_changed {
            error!("Pipeline failed to reach PLAYING state within timeout");
            return Err(RecordError::StreamError(
                "Pipeline failed to reach PLAYING state within timeout".into(),
            ));
        }

        // ストリーム状態を更新
        let mut state = StreamState::new();
        state.is_connected = true;
        state.protocol = Some(protocol);
        state.url = Some(url);
        state.pipeline = Some(pipeline);
        state.tee = Some(tee);
        streams.insert(stream_id.clone(), state);

        Ok(())
    }

    /// Starts recording for a specific stream.
    pub async fn start_recording(
        &self,
        stream_id: &StreamId,
        recording_id: &str,
        _location: &str,
    ) -> Result<(), RecordError> {
        // tee_readyフラグがtrueになるまで待機
        let mut retry_count = 0;
        while !self.is_tee_ready.load(Ordering::SeqCst) {
            if retry_count >= 10 {
                return Err(RecordError::StreamError(
                    "Stream is not ready for recording".into(),
                ));
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            retry_count += 1;
        }
        // recording_idはUuid型に変換
        let recording_uuid = uuid::Uuid::parse_str(recording_id)
            .map_err(|e| RecordError::StreamError(format!("Invalid recording_id: {}", e)))?;
        // recording_padsを渡す
        start_recording_impl(
            self.streams.clone(),
            stream_id,
            recording_uuid,
            &self.recording_pads, // 追加
        )
        .await?;
        let mut streams = self.streams.lock().await;
        let state = streams
            .get_mut(stream_id)
            .ok_or_else(|| RecordError::StreamError(format!("Stream {} not found ", stream_id)))?;
        state.is_recording = true;
        state.current_recording_id = Some(recording_id.to_string());
        Ok(())
    }

    /// Stops recording for a specific stream.
    pub async fn stop_recording(&self, stream_id: &StreamId) -> Result<String, RecordError> {
        let mut streams = self.streams.lock().await;
        let pipeline;
        {
            let state = streams.get_mut(stream_id).ok_or_else(|| {
                error!("[recording] Cannot stop recording: pipeline is not initialized");
                RecordError::StreamError("Pipeline is not initialized".into())
            })?;
            pipeline = state
                .pipeline
                .as_ref()
                .ok_or_else(|| {
                    error!("[recording] Cannot stop recording: pipeline is not initialized");
                    RecordError::StreamError("Pipeline is not initialized".into())
                })?
                .clone();
        }

        // 現在の録画IDを取得
        let current_recording_id = {
            let streams = self.streams.lock().await;
            let state = streams.get(stream_id).ok_or_else(|| {
                RecordError::StreamError(format!("Stream {} not found", stream_id))
            })?;
            state
                .current_recording_id
                .clone()
                .ok_or_else(|| RecordError::StreamError("No recording ID found".to_string()))?
        };

        // 録画Binを取得
        let bin_name = format!("rec-bin-{}", current_recording_id);
        let rec_bin = pipeline.by_name(&bin_name).ok_or_else(|| {
            error!(
                "[recording {}] Recording bin not found",
                current_recording_id
            );
            RecordError::StreamError(format!("Recording bin '{}' not found", bin_name))
        })?;

        // teeと録画Binのリンクを解除
        let mut recording_pads = self.recording_pads.lock().await;
        let tee_src_pad = recording_pads
            .remove(&current_recording_id)
            .ok_or_else(|| {
                error!(
                    "[recording {}] Tee source pad not found",
                    current_recording_id
                );
                RecordError::StreamError("Tee source pad not found".to_string())
            })?;

        let rec_bin_sink_pad = rec_bin.static_pad("sink").ok_or_else(|| {
            error!(
                "[recording {}] Recording bin sink pad not found",
                current_recording_id
            );
            RecordError::StreamError("Recording bin sink pad not found".to_string())
        })?;

        info!(
            "[recording {}] Unlinking tee from recording bin...",
            current_recording_id
        );
        tee_src_pad.unlink(&rec_bin_sink_pad)?;        // 録画BinにEOSイベントを送信
        info!(
            "[recording {}] Sending EOS to recording bin sink pad...",
            current_recording_id
        );
        rec_bin_sink_pad.send_event(gstreamer::event::Eos::new());

        // バスでEOSメッセージを短いタイムアウトで待機
        let bus = pipeline.bus().ok_or_else(|| {
            error!(
                "[recording {}] Failed to get bus from pipeline",
                current_recording_id
            );
            RecordError::StreamError("Failed to get bus from pipeline".into())
        })?;

        let timeout = gstreamer::ClockTime::from_seconds(2); // 2秒の短いタイムアウト
        let eos_received = bus.timed_pop_filtered(
            Some(timeout),
            &[gstreamer::MessageType::Eos, gstreamer::MessageType::Error],
        );

        match eos_received {
            Some(msg) => match msg.view() {
                MessageView::Eos(_) => {
                    info!(
                        "[recording {}] EOS received from recording bin",
                        current_recording_id
                    );
                }
                MessageView::Error(err) => {
                    error!(
                        "[recording {}] Error during recording shutdown: {}",
                        current_recording_id,
                        err.error()
                    );
                    if let Some(debug_info) = err.debug() {
                        error!(
                            "[recording {}] Debug info: {}",
                            current_recording_id, debug_info
                        );
                    }
                }
                _ => {}
            },
            None => {
                warn!(
                    "[recording {}] Timeout waiting for EOS from recording bin, proceeding with cleanup",
                    current_recording_id
                );
            }
        }

        // 録画Binの状態をNULLに設定
        info!(
            "[recording {}] Setting recording bin state to NULL...",
            current_recording_id
        );
        rec_bin.set_state(gstreamer::State::Null)?;

        // パイプラインから録画Binを削除
        info!(
            "[recording {}] Removing recording bin from pipeline...",
            current_recording_id
        );
        pipeline.remove(&rec_bin)?;

        // teeから使わなくなったパッドを解放
        tee_src_pad.parent().and_then(|tee| {
            tee.downcast_ref::<gstreamer::Element>().map(|tee| {
                tee.release_request_pad(&tee_src_pad);
            })
        });

        info!(
            "[recording {}] Recording bin removed and file saved.",
            current_recording_id
        );

        // 状態を更新
        let result: Result<String, RecordError> = {
            let mut streams = self.streams.lock().await;
            let state = streams.get_mut(stream_id).ok_or_else(|| {
                RecordError::StreamError(format!("Stream {} not found", stream_id))
            })?;

            state.is_recording = false;
            state.current_recording_id = None;

            Ok(current_recording_id)
        };

        result
    }

    /// Disconnects from a specific stream and stops/destroys its pipeline.
    pub async fn disconnect(&self, stream_id: &StreamId) -> Result<(), RecordError> {
        // まずロックを取得
        let mut streams = self.streams.lock().await;
        let is_recording = if let Some(state) = streams.get(stream_id) {
            state.is_recording
        } else {
            return Ok(());
        };

        // 録画中ならロックを一旦解放してstop_recordingを呼ぶ
        if is_recording {
            drop(streams);
            self.stop_recording(stream_id).await?;
            // 再度ロックを取得
            streams = self.streams.lock().await;
        }

        // パイプライン停止・削除処理
        if let Some(mut state) = streams.remove(stream_id) {
            if let Some(p) = state.pipeline.take() {
                // EOSを送信し、バスでEOS到達を待つ
                use gstreamer::MessageView;
                p.send_event(gstreamer::event::Eos::new());
                if let Some(bus) = p.bus() {
                    let mut eos_received = false;
                    for _ in 0..10 {
                        // 最大1秒待つ（100ms*10）
                        if let Some(msg) =
                            bus.timed_pop(Some(gstreamer::ClockTime::from_mseconds(100)))
                        {
                            match msg.view() {
                                MessageView::Eos(..) => {
                                    eos_received = true;
                                    break;
                                }
                                MessageView::Error(err) => {
                                    error!(%stream_id, "Pipeline error before EOS: {}", err.error());
                                    break;
                                }
                                _ => {}
                            }
                        }
                    }
                    if !eos_received {
                        warn!(%stream_id, "EOS not received before pipeline NULL transition, proceeding with cleanup");
                    }
                }
                // 状態遷移
                if let Err(e) = p.set_state(State::Null) {
                    let (_result, cur, pend) = p.state(None);
                    error!(%stream_id, "Failed to set pipeline to NULL: {:?}, current={:?}, pending={:?}", e, cur, pend);
                    return Err(RecordError::StreamError(format!(
                        "Failed to set pipeline to NULL: {:?}",
                        e
                    )));
                }
                info!(%stream_id, "Pipeline stopped and destroyed successfully.");
            }
            info!(%stream_id, "Disconnected from stream and stopped pipeline.");
        } else {
            warn!(%stream_id, "Not connected, nothing to do.");
        }
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
