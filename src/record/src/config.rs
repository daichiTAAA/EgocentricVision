use figment::{Figment, providers::{Format, Yaml, Env}};
use serde::Deserialize;
use std::path::PathBuf;
use crate::error::RecordError;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub recording_directory: PathBuf,
    pub database: DatabaseConfig,
    // pub stream: StreamConfig, // 未使用のためコメントアウト
    pub server: ServerConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

// #[derive(Debug, Deserialize, Clone)]
// pub struct StreamConfig {
//     pub default_rtsp_url: Option<String>, // 未使用のためコメントアウト
// }

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3000
}

impl Config {
    pub fn load() -> Result<Self, RecordError> {
        let config: Config = Figment::new()
            .merge(Yaml::file("config/record.yaml"))
            .merge(Env::prefixed("RECORD_"))
            .extract()
            .map_err(|e| RecordError::ConfigError(e.to_string()))?;

        // Ensure recording directory exists
        if !config.recording_directory.exists() {
            std::fs::create_dir_all(&config.recording_directory)
                .map_err(|e| RecordError::ConfigError(format!("Failed to create recording directory: {}", e)))?;
        }

        Ok(config)
    }
}