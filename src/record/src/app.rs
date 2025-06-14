use crate::config::Config;
use crate::database::Database;
use crate::stream::StreamManager;

pub struct AppState {
    pub config: Config,
    pub database: Database,
    pub stream_manager: StreamManager,
}

impl AppState {
    pub fn new(config: Config, database: Database) -> Self {
        Self {
            config: config.clone(),
            database,
            stream_manager: StreamManager::new(config),
        }
    }
}