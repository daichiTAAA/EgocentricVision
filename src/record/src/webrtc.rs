use crate::error::RecordError;
use gstreamer::prelude::*;
use gstreamer::{Element, ElementFactory};

/// WebRTCストリーム開始処理
pub async fn start_webrtc_streaming_impl(
    is_connected: bool,
    pipeline: Option<&gstreamer::Pipeline>,
    tee: Option<&Element>,
) -> Result<Element, RecordError> {
    if !is_connected {
        return Err(RecordError::StreamError("Stream not connected".to_string()));
    }
    let pipeline =
        pipeline.ok_or_else(|| RecordError::StreamError("Pipeline not initialized".to_string()))?;
    let tee =
        tee.ok_or_else(|| RecordError::StreamError("Tee element not initialized".to_string()))?;

    // queueとwebrtcbinを作成
    let queue = ElementFactory::make("queue")
        .build()
        .map_err(|_| RecordError::StreamError("Failed to create queue".to_string()))?;
    let webrtcbin = ElementFactory::make("webrtcbin")
        .build()
        .map_err(|_| RecordError::StreamError("Failed to create webrtcbin".to_string()))?;

    // pipelineに追加
    pipeline
        .add_many([&queue, &webrtcbin])
        .map_err(|_| RecordError::StreamError("Failed to add elements to pipeline".to_string()))?;
    queue.sync_state_with_parent().ok();
    webrtcbin.sync_state_with_parent().ok();

    // Teeのsrc padをrequestし、queueにリンク
    let tee_src_pad = tee
        .request_pad_simple("src_%u")
        .ok_or_else(|| RecordError::StreamError("Failed to request tee src pad".to_string()))?;
    let queue_sink_pad = queue
        .static_pad("sink")
        .ok_or_else(|| RecordError::StreamError("Failed to get queue sink pad".to_string()))?;
    tee_src_pad
        .link(&queue_sink_pad)
        .map_err(|e| RecordError::StreamError(format!("Failed to link tee to queue: {}", e)))?;

    // queue→webrtcbinをリンク
    let queue_src_pad = queue
        .static_pad("src")
        .ok_or_else(|| RecordError::StreamError("Failed to get queue src pad".to_string()))?;
    let webrtcbin_sink_pad = webrtcbin.static_pad("sink_video_rtp").ok_or_else(|| {
        RecordError::StreamError("Failed to get webrtcbin sink_video_rtp pad".to_string())
    })?;
    queue_src_pad.link(&webrtcbin_sink_pad).map_err(|e| {
        RecordError::StreamError(format!("Failed to link queue to webrtcbin: {}", e))
    })?;

    Ok(webrtcbin)
}
