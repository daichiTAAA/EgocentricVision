mod api;
mod app;
mod config;
mod database;
mod error;
mod models;
mod recording;
mod stream;
mod webrtc;

use anyhow::Result;
use config::Config;
use database::Database;
use std::io::stderr;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // パニック時に必ずstderrにバックトレースを出力
    std::panic::set_hook(Box::new(|panic_info| {
        let bt = std::backtrace::Backtrace::force_capture();
        eprintln!("\n=== PANIC ===\n{}\nBacktrace:\n{:?}\n", panic_info, bt);
    }));

    // Initialize tracing (stderr明示)
    tracing_subscriber::fmt().with_writer(stderr).json().init();

    info!("Starting record service");

    // Load configuration
    let config = Config::load().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    // Initialize database with retry logic
    let database = {
        let mut attempts = 0;
        let max_attempts = 5; // 最大5回まで試行
        let retry_delay = Duration::from_secs(5); // 5秒待ってリトライ

        loop {
            attempts += 1;
            match Database::new(&config.database.url).await {
                Ok(db) => {
                    info!("Successfully connected to the database.");
                    break db;
                }
                Err(e) => {
                    if attempts >= max_attempts {
                        error!(
                            "Failed to initialize database after {} attempts: {}",
                            attempts, e
                        );
                        return Err(e.into());
                    }
                    warn!(
                        "Failed to initialize database (attempt {}/{}). Retrying in {:?}... Error: {}",
                        attempts, max_attempts, retry_delay, e
                    );
                    sleep(retry_delay).await;
                }
            }
        }
    };

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
