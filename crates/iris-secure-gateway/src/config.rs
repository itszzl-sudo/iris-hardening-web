//! 配置文件解析

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub encryption: EncryptionConfig,
    pub key_rotation: KeyRotationConfig,
    pub file_mappings: HashMap<String, String>,
    pub api_routes: Vec<ApiRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub base_dir: PathBuf,
    #[serde(default = "default_assets_dir")]
    pub assets_dir: PathBuf,
}

fn default_assets_dir() -> PathBuf {
    PathBuf::from("./assets")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    pub key_file: PathBuf,
    pub algorithm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_wasm_gateway_url")]
    pub wasm_gateway_url: String,
    #[serde(default = "default_check_interval")]
    pub check_interval_seconds: u64,
    #[serde(default = "default_update_before_expiry")]
    pub update_before_expiry_hours: u64,
}

fn default_true() -> bool { true }
fn default_wasm_gateway_url() -> String { "http://127.0.0.1:9090".to_string() }
fn default_check_interval() -> u64 { 3600 }
fn default_update_before_expiry() -> u64 { 2 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRoute {
    pub pattern: String,
    pub target: String,
    pub methods: Vec<String>,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::Error::Config(format!("Failed to read config: {}", e)))?;
        
        toml::from_str(&content)
            .map_err(|e| crate::Error::Config(format!("Failed to parse config: {}", e)))
    }
    
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        toml::from_str(toml_str)
            .map_err(|e| crate::Error::Config(format!("Failed to parse config: {}", e)))
    }
    
    pub fn load_key(&self) -> Result<Vec<u8>> {
        let key_content = std::fs::read_to_string(&self.encryption.key_file)
            .map_err(|e| crate::Error::Config(format!("Failed to read key file: {}", e)))?;
        
        hex::decode(key_content.trim())
            .map_err(|e| crate::Error::Config(format!("Invalid key format: {}", e)))
    }
    
    pub fn resolve_real_path(&self, encrypted_name: &str) -> Option<&String> {
        self.file_mappings.get(encrypted_name)
    }
    
    pub fn match_api_route(&self, path: &str, method: &str) -> Option<&ApiRoute> {
        self.api_routes.iter().find(|route| {
            let pattern_ok = regex::Regex::new(&route.pattern)
                .map(|re| re.is_match(path))
                .unwrap_or(false);
            let method_ok = route.methods.iter().any(|m| m.eq_ignore_ascii_case(method));
            pattern_ok && method_ok
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                base_dir: PathBuf::from("."),
                assets_dir: PathBuf::from("./assets"),
            },
            encryption: EncryptionConfig {
                key_file: PathBuf::from("key.txt"),
                algorithm: "aes-256-gcm".to_string(),
            },
            key_rotation: KeyRotationConfig {
                enabled: true,
                wasm_gateway_url: "http://127.0.0.1:9090".to_string(),
                check_interval_seconds: 3600,
                update_before_expiry_hours: 2,
            },
            file_mappings: HashMap::new(),
            api_routes: Vec::new(),
        }
    }
}
