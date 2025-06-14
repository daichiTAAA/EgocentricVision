mod api;
mod app;
mod config;
mod database;
mod error;
mod models;
mod stream;

use anyhow::Result;
use config::Config;
use database::Database;
use std::sync::Arc;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .json()
        .init();

    info!("Starting record service");

    // Load configuration
    let config = Config::load().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    // Initialize database
    let database = Database::new(&config.database.url).await.map_err(|e| {
        error!("Failed to initialize database: {}", e);
        e
    })?;

    // Run migrations
    database.migrate().await.map_err(|e| {
        error!("Failed to run database migrations: {}", e);
        e
    })?;

    // Initialize application state
    let app_state = Arc::new(app::AppState::new(config, database));

    // Start the server
    api::serve(app_state).await.map_err(|e| {
        error!("Server error: {}", e);
        e
    })?;

    Ok(())
}