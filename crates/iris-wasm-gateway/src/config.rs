//! 配置

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use chrono::Duration;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub key: KeyConfig,
    pub encrypt_service: EncryptServiceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub wasm_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyConfig {
    pub key_dir: PathBuf,
    pub validity_hours: u64,
    pub rotation_margin_hours: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptServiceConfig {
    pub url: String,
    pub update_key_endpoint: String,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::Error::Config(format!("Failed to read config: {}", e)))?;
        
        toml::from_str(&content)
            .map_err(|e| crate::Error::Config(format!("Failed to parse config: {}", e)))
    }
    
    pub fn validity_duration(&self) -> Duration {
        Duration::hours(self.key.validity_hours as i64)
    }
    
    pub fn rotation_margin(&self) -> Duration {
        Duration::hours(self.key.rotation_margin_hours as i64)
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 9090,
                wasm_path: PathBuf::from("iris.wasm"),
            },
            key: KeyConfig {
                key_dir: PathBuf::from("keys"),
                validity_hours: 24,
                rotation_margin_hours: 2,
            },
            encrypt_service: EncryptServiceConfig {
                url: "http://127.0.0.1:8080".to_string(),
                update_key_endpoint: "/internal/update-key".to_string(),
            },
        }
    }
}
