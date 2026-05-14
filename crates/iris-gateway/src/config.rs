//! 统一配置
//!
//! 合并 iris-secure-gateway 和 iris-wasm-gateway 的配置，
//! 适配 nginx <-> iris-gateway <-> 内部服务器 的部署场景。

use chrono::Duration;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub encryption: EncryptionConfig,
    pub key: KeyConfig,
    pub file_mappings: HashMap<String, String>,
    pub api_routes: Vec<ApiRoute>,
    /// 内部 API 认证令牌，用于保护 /internal/* 端点
    #[serde(default = "default_internal_token")]
    pub internal_token: String,
    /// 健康检查路径（供给 nginx upstream_check 使用）
    #[serde(default = "default_health_check_path")]
    pub health_check_path: String,
    /// 预编译的正则路由
    #[serde(skip)]
    pub compiled_routes: Option<Arc<Vec<CompiledRoute>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    /// 静态文件根目录（需加密保护的文件）
    pub base_dir: PathBuf,
    #[serde(default = "default_assets_dir")]
    pub assets_dir: PathBuf,
    /// 工作线程数，0 = 自动（等于 CPU 核心数）
    #[serde(default)]
    pub workers: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    /// 初始密钥文件路径（hex 编码的 32 字节密钥）
    pub key_file: PathBuf,
    pub algorithm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyConfig {
    /// 密钥持久化目录
    pub key_dir: PathBuf,
    /// 密钥有效期（小时）
    pub validity_hours: u64,
    /// 密钥轮换提前量（小时）
    pub rotation_margin_hours: u64,
    /// 轮换检查间隔（秒）
    #[serde(default = "default_rotation_check_interval")]
    pub rotation_check_interval_seconds: u64,
}

/// 预编译的路由
#[derive(Debug)]
pub struct CompiledRoute {
    pub pattern: Regex,
    pub target: String,
    pub methods: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRoute {
    pub pattern: String,
    pub target: String,
    pub methods: Vec<String>,
}

// --- 默认值函数 ---

fn default_assets_dir() -> PathBuf { PathBuf::from("./assets") }
fn default_internal_token() -> String { "change-me-in-production".to_string() }
fn default_health_check_path() -> String { "/health".to_string() }
fn default_rotation_check_interval() -> u64 { 60 }

impl Config {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::Error::Config(format!("Failed to read config: {}", e)))?;
        let mut config: Config = toml::from_str(&content)
            .map_err(|e| crate::Error::Config(format!("Failed to parse config: {}", e)))?;
        config.compile_routes()?;
        Ok(config)
    }

    pub fn from_toml(toml_str: &str) -> Result<Self> {
        let mut config: Config = toml::from_str(toml_str)
            .map_err(|e| crate::Error::Config(format!("Failed to parse config: {}", e)))?;
        config.compile_routes()?;
        Ok(config)
    }

    /// 预编译所有正则路由
    pub fn compile_routes(&mut self) -> Result<()> {
        let mut compiled = Vec::with_capacity(self.api_routes.len());
        for route in &self.api_routes {
            let pattern = Regex::new(&route.pattern)
                .map_err(|e| crate::Error::Config(format!("Invalid regex '{}': {}", route.pattern, e)))?;
            compiled.push(CompiledRoute {
                pattern,
                target: route.target.clone(),
                methods: route.methods.clone(),
            });
        }
        self.compiled_routes = Some(Arc::new(compiled));
        Ok(())
    }

    /// 从密钥文件加载初始密钥
    pub fn load_initial_key(&self) -> Result<Vec<u8>> {
        let key_content = std::fs::read_to_string(&self.encryption.key_file)
            .map_err(|e| crate::Error::Config(format!("Failed to read key file: {}", e)))?;
        hex::decode(key_content.trim())
            .map_err(|e| crate::Error::Config(format!("Invalid key format: {}", e)))
    }

    /// 匹配 API 路由（使用预编译正则）
    pub fn match_api_route(&self, path: &str, method: &str) -> Option<&CompiledRoute> {
        self.compiled_routes.as_ref()?.iter().find(|route| {
            route.pattern.is_match(path)
                && route.methods.iter().any(|m| m.eq_ignore_ascii_case(method))
        })
    }

    pub fn validity_duration(&self) -> Duration {
        Duration::hours(self.key.validity_hours as i64)
    }

    pub fn rotation_margin(&self) -> Duration {
        Duration::hours(self.key.rotation_margin_hours as i64)
    }

    /// Whether the gateway has encryption fully configured and active.
    /// In zero-config mode (no key file, no mappings), encryption is inactive
    /// and the gateway acts as a transparent reverse proxy.
    pub fn is_encryption_active(&self) -> bool {
        self.encryption.key_file.exists()
            || !self.file_mappings.is_empty()
            || !self.api_routes.is_empty()
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                base_dir: PathBuf::from("./data"),
                assets_dir: PathBuf::from("./assets"),
                workers: 0,
            },
            encryption: EncryptionConfig {
                key_file: PathBuf::from(""),  // Empty = no key file, zero-config mode
                algorithm: "aes-256-gcm".to_string(),
            },
            key: KeyConfig {
                key_dir: PathBuf::from("./keys"),
                validity_hours: 24,
                rotation_margin_hours: 2,
                rotation_check_interval_seconds: 60,
            },
            file_mappings: HashMap::new(),
            api_routes: Vec::new(),
            internal_token: default_internal_token(),
            health_check_path: default_health_check_path(),
            compiled_routes: None,
        }
    }
}
