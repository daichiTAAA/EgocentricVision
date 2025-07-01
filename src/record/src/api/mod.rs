mod handlers;

use crate::app::AppState;
use crate::error::RecordError;
use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::Level;

pub async fn serve(app_state: Arc<AppState>) -> Result<(), RecordError> {
    let app = create_router(app_state.clone());

    let listener = tokio::net::TcpListener::bind(format!(
        "{}:{}",
        app_state.config.server.host, app_state.config.server.port
    ))
    .await
    .map_err(|e| RecordError::InternalError(format!("Failed to bind to address: {}", e)))?;

    tracing::info!(
        "Server listening on {}:{}",
        app_state.config.server.host,
        app_state.config.server.port
    );

    axum::serve(listener, app)
        .await
        .map_err(|e| RecordError::InternalError(format!("Server error: {}", e)))?;

    Ok(())
}

fn create_router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/api/v1/streams/connect", post(handlers::streams::connect))
        .route(
            "/api/v1/streams/status",
            get(handlers::streams::list_statuses),
        )
        .route(
            "/api/v1/streams/:stream_id/status",
            get(handlers::streams::status),
        )
        .route(
            "/api/v1/streams/:stream_id/debug",
            get(handlers::streams::debug_status),
        )
        .route(
            "/api/v1/streams/:stream_id/disconnect",
            post(handlers::streams::disconnect),
        )
        .route(
            "/api/v1/streams/:stream_id/webrtc",
            post(handlers::webrtcs::webrtc_signaling),
        )
        .route(
            "/api/v1/recordings/:stream_id/start",
            post(handlers::recordings::start),
        )
        .route(
            "/api/v1/recordings/:stream_id/stop",
            post(handlers::recordings::stop),
        )
        .route("/api/v1/recordings", get(handlers::recordings::list))
        .route(
            "/api/v1/recordings/:recording_id",
            get(handlers::recordings::get),
        )
        .route(
            "/api/v1/recordings/:recording_id/download",
            get(handlers::recordings::download),
        )
        .route(
            "/api/v1/recordings/:recording_id",
            delete(handlers::recordings::delete),
        )
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                        .on_response(DefaultOnResponse::new().level(Level::INFO)),
                )
                .layer(CorsLayer::permissive()),
        )
        .with_state(app_state)
}
