use crate::error::RecordError;
use crate::stream::{StreamId, StreamState};
use glib::prelude::ObjectExt;
use gstreamer::prelude::*;
use gstreamer::{Bin, Element, ElementFactory, State};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex};
use tracing::{error, info};
use uuid::Uuid;

/// 録画開始ロジック
pub async fn start_recording_impl(
    streams: Arc<Mutex<HashMap<StreamId, StreamState>>>,
    stream_id: &StreamId,
    recording_id: Uuid,
    recording_pads: &Arc<Mutex<HashMap<String, gstreamer::Pad>>>, // 追加
) -> Result<(), RecordError> {
    let mut streams = streams.lock().await;
    let state = streams
        .get_mut(stream_id)
        .ok_or_else(|| RecordError::StreamError(format!("Stream {} not found", stream_id)))?;

    let is_connected = state.is_connected;
    if !is_connected {
        return Err(RecordError::StreamError("Stream not connected".to_string()));
    }

    let pipeline = state
        .pipeline
        .as_ref()
        .ok_or_else(|| RecordError::StreamError("Pipeline not initialized".to_string()))?;

    let tee = state
        .tee
        .as_ref()
        .ok_or_else(|| RecordError::StreamError("Tee not initialized".to_string()))?;

    // --- 新しい録画Bin構築手順 ---
    // 1. 各要素を生成
    let queue = ElementFactory::make("queue").build()?;
    let h264parse = ElementFactory::make("h264parse").build()?;
    let mp4mux = ElementFactory::make("mp4mux").build()?;
    mp4mux.set_property("faststart", true);
    let filesink = ElementFactory::make("filesink").build()?;
    filesink.set_property(
        "location",
        format!("/var/data/recordings/{}.mp4", recording_id),
    );
    filesink.set_property("sync", false);
    filesink.set_property("async", false);
    queue.set_property("max-size-buffers", 100u32);
    queue.set_property("max-size-bytes", 0u32);
    queue.set_property("max-size-time", 0u64);

    // 2. Binを作成し要素を追加
    let recording_bin = Bin::new();
    recording_bin.set_property("name", format!("rec-bin-{}", recording_id));
    recording_bin.add_many([&queue, &h264parse, &mp4mux, &filesink])?;
    Element::link_many([&queue, &h264parse, &mp4mux, &filesink])?;

    // 3. GhostPadをqueueのsinkパッドでactive化してBinに追加
    let queue_sink_pad = queue
        .static_pad("sink")
        .ok_or_else(|| RecordError::StreamError("Failed to get queue sink pad".to_string()))?;
    let ghost_sink = gstreamer::GhostPad::with_target(&queue_sink_pad)?;
    ghost_sink.set_active(true)?;
    recording_bin.add_pad(&ghost_sink)?;

    // 4. Binをパイプラインに追加
    pipeline.add(&recording_bin)?;

    // 5. teeのsrcパッドとBinのsinkパッド（GhostPad）をリンク
    let tee_src_pad = tee
        .request_pad_simple("src_%u")
        .ok_or_else(|| RecordError::StreamError("Failed to request tee src pad".to_string()))?;
    {
        let mut pads = recording_pads.lock().await;
        pads.insert(recording_id.to_string(), tee_src_pad.clone());
        info!(
            "Inserted tee_src_pad into recording_pads: recording_id={}",
            recording_id
        );
    }
    let rec_bin_sink_pad = recording_bin.static_pad("sink").ok_or_else(|| {
        RecordError::StreamError("Failed to get recording_bin sink pad".to_string())
    })?;
    tee_src_pad.link(&rec_bin_sink_pad).map_err(|e| {
        error!("Failed to link tee_src_pad to rec_bin_sink_pad: {}", e);
        RecordError::StreamError(format!("Failed to link tee_src_pad: {}", e))
    })?;

    // 6. Binの状態を親パイプラインと同期し、PLAYINGに遷移
    recording_bin.sync_children_states()?;
    recording_bin.set_state(State::Playing)?;

    // current_recording_idを必ずセット
    state.current_recording_id = Some(recording_id.to_string());
    state.is_recording = true;

    Ok(())
}

// /// 録画停止ロジック
// #[allow(dead_code)]
// pub async fn stop_recording_impl(
//     state: &mut StreamState,
//     pipeline: &Pipeline,
//     recording_pads: &mut MutexGuard<'_, HashMap<String, gstreamer::Pad>>,
// ) -> Result<String, RecordError> {
//     let recording_id = state.current_recording_id.clone().ok_or_else(|| {
//         error!("[recording] Cannot stop recording: no active recording");
//         RecordError::StreamError("No active recording".into())
//     })?;

//     info!("[recording] Stopping recording {}", recording_id);

//     let eos_event = gstreamer::event::Eos::new();
//     if !pipeline.send_event(eos_event) {
//         error!("[recording] Failed to send EOS event");
//         return Err(RecordError::StreamError("Failed to send EOS event".into()));
//     }

//     if let Some(pad) = recording_pads.remove(&recording_id) {
//         info!("[recording] Unlinking recording pad for {}", recording_id);

//         info!(
//             "[recording] Pad state before unlinking: linked={}, caps={:?}",
//             pad.is_linked(),
//             pad.current_caps()
//         );

//         if let Some(peer) = pad.peer() {
//             if let Err(e) = pad.set_active(false) {
//                 error!("[recording] Failed to deactivate pad: {}", e);
//                 return Err(RecordError::StreamError(format!(
//                     "Failed to deactivate pad: {}",
//                     e
//                 )));
//             }

//             if let Err(e) = pad.unlink(&peer) {
//                 error!("[recording] Failed to unlink pad: {}", e);
//                 return Err(RecordError::StreamError(format!(
//                     "Failed to unlink pad: {}",
//                     e
//                 )));
//             }

//             if let Err(e) = peer.set_active(false) {
//                 error!("[recording] Failed to deactivate peer pad: {}", e);
//                 return Err(RecordError::StreamError(format!(
//                     "Failed to deactivate peer pad: {}",
//                     e
//                 )));
//             }
//         }

//         info!(
//             "[recording] Pad state after unlinking: linked={}, caps={:?}",
//             pad.is_linked(),
//             pad.current_caps()
//         );
//     }

//     state.is_recording = false;
//     state.current_recording_id = None;

//     info!(
//         "[recording] Recording {} stopped successfully",
//         recording_id
//     );
//     Ok(recording_id)
// }
