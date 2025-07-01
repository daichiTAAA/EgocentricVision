use crate::app::AppState;
use crate::stream::StreamId;
use crate::webrtc::start_webrtc_streaming_impl;
use axum::body::Bytes;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use tracing::error;

pub async fn webrtc_signaling(
    State(app_state): State<Arc<AppState>>,
    Path(stream_id): Path<StreamId>,
    body: Bytes,
) -> Response {
    let offer_sdp = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let mut streams_guard = app_state.stream_manager.get_stream_mut(&stream_id).await;
    let state = match streams_guard
        .as_mut()
        .and_then(|streams| streams.get_mut(&stream_id))
    {
        Some(s) => s,
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    let webrtcbin = match start_webrtc_streaming_impl(
        state.is_connected,
        state.pipeline.as_ref(),
        state.tee.as_ref(),
    )
    .await
    {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to start webrtc streaming: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    use gstreamer::prelude::*;
    use gstreamer::Promise;
    use gstreamer_sdp::SDPMessage;
    use gstreamer_webrtc::WebRTCSessionDescription;
    let sdp_msg = match SDPMessage::parse_buffer(offer_sdp.as_bytes()) {
        Ok(msg) => msg,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };
    let offer = WebRTCSessionDescription::new(gstreamer_webrtc::WebRTCSDPType::Offer, sdp_msg);
    let promise = Promise::new();
    webrtcbin.emit_by_name::<()>("set-remote-description", &[&offer, &promise]);
    let promise2 = Promise::new();
    webrtcbin.emit_by_name::<()>("create-answer", &[&None::<gstreamer::Structure>, &promise2]);
    match promise2.wait() {
        gstreamer::PromiseResult::Replied => {
            let answer_desc = webrtcbin.property::<WebRTCSessionDescription>("answer");
            let sdp_str = answer_desc.sdp().as_text().unwrap_or_default();
            (
                StatusCode::OK,
                [("Content-Type", "application/sdp")],
                sdp_str,
            )
                .into_response()
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
