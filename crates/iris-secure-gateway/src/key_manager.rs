//! 密钥对管理 - 带同步过期机制
//!
//! 管理密钥对、路由映射和 WASM 的生命周期
//! 所有组件按约定时间同时过期

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::WasmGenerator;

/// 密钥对结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPair {
    /// 唯一标识符
    pub id: Uuid,
    /// AES-256 密钥 (32 字节)
    pub key: Vec<u8>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 过期时间
    pub expires_at: DateTime<Utc>,
    /// 密钥用途
    pub purpose: KeyPurpose,
}

/// 密钥用途
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum KeyPurpose {
    /// 文件加密
    File,
    /// API 加密
    Api,
    /// WASM 配置加密
    WasmConfig,
}

impl KeyPair {
    /// 创建新的密钥对
    pub fn new(validity_hours: i64, purpose: KeyPurpose) -> Self {
        let now = Utc::now();
        let key = generate_random_key();

        Self {
            id: Uuid::new_v4(),
            key,
            created_at: now,
            expires_at: now + Duration::hours(validity_hours),
            purpose,
        }
    }

    /// 检查是否已过期
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// 获取剩余有效期（秒）
    pub fn remaining_seconds(&self) -> i64 {
        let remaining = self.expires_at - Utc::now();
        remaining.num_seconds().max(0)
    }

    /// 获取 hex 编码的密钥
    pub fn key_hex(&self) -> String {
        hex::encode(&self.key)
    }

    /// 获取过期时间戳（秒）
    pub fn expires_timestamp(&self) -> i64 {
        self.expires_at.timestamp()
    }
}

/// 安全配置 - 包含所有需要同步过期的组件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureConfig {
    /// 路由映射 (真实路径 -> 加密路径)
    pub routes: HashMap<String, RouteInfo>,
    /// 当前密钥对
    pub key_pair: KeyPair,
    /// WASM 过期时间 (与密钥同步)
    pub wasm_expiry: i64,
    /// 配置版本
    pub version: String,
    /// 创建时间
    pub created_at: i64,
}

/// 路由信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    /// 加密后的路径
    pub encrypted_path: String,
    /// 目标 URL (用于 API 代理)
    pub target_url: Option<String>,
    /// HTTP 方法
    pub methods: Vec<String>,
}

impl SecureConfig {
    /// 创建新的安全配置
    pub fn new(validity_hours: i64) -> Self {
        let now = Utc::now();
        let key_pair = KeyPair::new(validity_hours, KeyPurpose::WasmConfig);

        Self {
            routes: HashMap::new(),
            key_pair: key_pair.clone(),
            wasm_expiry: key_pair.expires_at.timestamp(),
            version: Uuid::new_v4().to_string(),
            created_at: now.timestamp(),
        }
    }

    /// 添加路由映射
    pub fn add_route(&mut self, real_path: &str, encrypted_path: &str, target_url: Option<String>) {
        self.routes.insert(
            real_path.to_string(),
            RouteInfo {
                encrypted_path: encrypted_path.to_string(),
                target_url,
                methods: vec!["GET".to_string(), "POST".to_string()],
            },
        );
    }

    /// 检查配置是否过期
    pub fn is_expired(&self) -> bool {
        self.key_pair.is_expired() || Utc::now().timestamp() > self.wasm_expiry
    }

    /// 获取配置年龄（秒）
    pub fn age_seconds(&self) -> i64 {
        Utc::now().timestamp() - self.created_at
    }

    /// 获取距离过期的秒数
    pub fn expires_in_seconds(&self) -> i64 {
        self.key_pair.remaining_seconds()
    }
}

/// 密钥管理器 - 管理密钥对的生成和轮换
pub struct KeyManager {
    /// 当前密钥对
    current_key: Option<KeyPair>,
    /// 密钥有效期（小时）
    validity_hours: i64,
    /// 轮换提前量（小时）- 在过期前多久开始轮换
    rotation_margin_hours: i64,
    /// 历史密钥（用于解密旧数据）
    key_history: Vec<KeyPair>,
}

impl KeyManager {
    /// 创建新的密钥管理器
    pub fn new(validity_hours: i64, rotation_margin_hours: i64) -> Self {
        Self {
            current_key: None,
            validity_hours,
            rotation_margin_hours,
            key_history: Vec::new(),
        }
    }

    /// 初始化或获取当前密钥
    pub fn get_or_create_key(&mut self) -> &KeyPair {
        if self.current_key.is_none() {
            self.current_key = Some(KeyPair::new(self.validity_hours, KeyPurpose::WasmConfig));
        }

        // 检查是否需要轮换
        if self.should_rotate() {
            self.rotate_key();
        }

        self.current_key.as_ref().unwrap()
    }

    /// 检查是否需要轮换密钥
    pub fn should_rotate(&self) -> bool {
        if let Some(ref key) = self.current_key {
            let margin = Duration::hours(self.rotation_margin_hours);
            let expires_with_margin = key.expires_at - margin;
            Utc::now() > expires_with_margin
        } else {
            true
        }
    }

    /// 轮换密钥
    pub fn rotate_key(&mut self) {
        // 保存当前密钥到历史
        if let Some(current) = self.current_key.take() {
            self.key_history.push(current);
        }

        // 生成新密钥
        self.current_key = Some(KeyPair::new(self.validity_hours, KeyPurpose::WasmConfig));
    }

    /// 获取当前密钥
    pub fn get_current_key(&self) -> Option<&KeyPair> {
        self.current_key.as_ref()
    }

    /// 获取用于解密的密钥列表（当前 + 历史）
    pub fn get_decryption_keys(&self) -> Vec<&KeyPair> {
        let mut keys = Vec::new();
        if let Some(ref current) = self.current_key {
            keys.push(current);
        }
        keys.extend(self.key_history.iter().filter(|k| !k.is_expired()));
        keys
    }

    /// 生成安全配置
    pub fn generate_config(&mut self, routes: HashMap<String, RouteInfo>) -> SecureConfig {
        let key = self.get_or_create_key().clone();

        SecureConfig {
            routes,
            key_pair: key.clone(),
            wasm_expiry: key.expires_at.timestamp(),
            version: Uuid::new_v4().to_string(),
            created_at: Utc::now().timestamp(),
        }
    }

    /// 检查指定版本的配置是否仍然有效
    pub fn is_config_valid(&self, version: &str) -> bool {
        if let Some(ref key) = self.current_key {
            if key.is_expired() {
                return false;
            }
        }
        true
    }

    /// 检查是否需要预生成下一批配置
    /// 在过期前 `pre_generate_margin_hours` 小时开始生成
    pub fn should_pre_generate(&self, margin_hours: i64) -> bool {
        if let Some(ref key) = self.current_key {
            let margin = Duration::hours(margin_hours);
            let pre_generate_time = key.expires_at - margin;
            Utc::now() >= pre_generate_time
        } else {
            true
        }
    }

    /// 获取当前配置剩余的有效期百分比 (0.0 - 1.0)
    pub fn get_validity_percentage(&self) -> f64 {
        if let Some(ref key) = self.current_key {
            let total = key.expires_at - key.created_at;
            let remaining = key.expires_at - Utc::now();
            if total.num_seconds() > 0 {
                (remaining.num_seconds() as f64 / total.num_seconds() as f64).max(0.0)
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
}

/// 配置预生成器 - 在过期前预先生成下一批配置
pub struct ConfigGenerator {
    /// 当前活跃配置
    current_config: Option<SecureConfig>,
    /// 预生成的下一批配置 (备用)
    next_config: Option<SecureConfig>,
    /// WASM 预生成数据
    current_wasm: Option<Vec<u8>>,
    /// 下一批 WASM
    next_wasm: Option<Vec<u8>>,
    /// 配置有效期 (小时)
    validity_hours: i64,
    /// 预生成提前量 (小时)
    pre_generate_margin_hours: i64,
    /// 路由映射
    routes: HashMap<String, RouteInfo>,
}

impl ConfigGenerator {
    /// 创建新的配置生成器
    pub fn new(validity_hours: i64, pre_generate_margin_hours: i64) -> Self {
        Self {
            current_config: None,
            next_config: None,
            current_wasm: None,
            next_wasm: None,
            validity_hours,
            pre_generate_margin_hours,
            routes: HashMap::new(),
        }
    }

    /// 设置路由映射
    pub fn set_routes(&mut self, routes: HashMap<String, RouteInfo>) {
        self.routes = routes;
    }

    /// 添加路由
    pub fn add_route(&mut self, path: &str, encrypted: &str, target: Option<String>) {
        self.routes.insert(
            path.to_string(),
            RouteInfo {
                encrypted_path: encrypted.to_string(),
                target_url: target,
                methods: vec!["GET".to_string(), "POST".to_string()],
            },
        );
    }

    /// 初始化或生成当前配置
    pub fn get_or_create_current(&mut self) -> &SecureConfig {
        if self.current_config.is_none() || self.current_config.as_ref().map(|c| c.is_expired()).unwrap_or(true) {
            self.generate_current();
        }
        self.current_config.as_ref().unwrap()
    }

    /// 生成当前配置
    fn generate_current(&mut self) {
        let now = Utc::now();
        let key_pair = KeyPair::new(self.validity_hours, KeyPurpose::WasmConfig);

        let config = SecureConfig {
            routes: self.routes.clone(),
            key_pair,
            wasm_expiry: now.timestamp() + self.validity_hours * 3600,
            version: Uuid::new_v4().to_string(),
            created_at: now.timestamp(),
        };

        self.current_config = Some(config);
    }

    /// 检查并预生成下一批配置
    /// 在当前配置剩余有效期达到预生成阈值时自动调用
    pub fn check_and_pre_generate(&mut self, wasm_generator: &WasmGenerator) {
        if self.current_config.is_none() {
            self.generate_current();
            return;
        }

        let current = self.current_config.as_ref().unwrap();
        let margin_seconds = self.pre_generate_margin_hours * 3600;

        // 检查是否需要预生成
        let remaining = current.key_pair.expires_at - Utc::now();
        if remaining.num_seconds() <= margin_seconds && self.next_config.is_none() {
            tracing::info!(
                "Pre-generating next config. Remaining: {}s, margin: {}s",
                remaining.num_seconds(),
                margin_seconds
            );
            self.generate_next(wasm_generator);
        }
    }

    /// 生成下一批配置
    fn generate_next(&mut self, wasm_generator: &WasmGenerator) {
        let now = Utc::now();
        let key_pair = KeyPair::new(self.validity_hours, KeyPurpose::WasmConfig);

        let config = SecureConfig {
            routes: self.routes.clone(),
            key_pair: key_pair.clone(),
            wasm_expiry: now.timestamp() + self.validity_hours * 3600,
            version: Uuid::new_v4().to_string(),
            created_at: now.timestamp(),
        };

        // 预生成 WASM
        self.next_wasm = Some(wasm_generator.generate_minimal_wasm(&config));
        self.next_config = Some(config);
    }

    /// 切换到下一批配置
    /// 当当前配置过期时自动调用
    pub fn rotate(&mut self) -> Option<&SecureConfig> {
        if let Some(next) = self.next_config.take() {
            self.current_config = Some(next.clone());
            self.current_wasm = self.next_wasm.take();
            tracing::info!("Config rotated to version: {}", self.current_config.as_ref().unwrap().version);
            Some(self.current_config.as_ref().unwrap())
        } else {
            self.generate_current();
            self.current_config.as_ref()
        }
    }

    /// 获取当前配置
    pub fn get_current(&self) -> Option<&SecureConfig> {
        self.current_config.as_ref()
    }

    /// 获取下一批配置 (如果已预生成)
    pub fn get_next(&self) -> Option<&SecureConfig> {
        self.next_config.as_ref()
    }

    /// 检查当前配置是否即将过期
    pub fn is_current_expiring(&self, margin_hours: i64) -> bool {
        if let Some(ref current) = self.current_config {
            let margin = Duration::hours(margin_hours);
            current.key_pair.expires_at - Utc::now() <= margin
        } else {
            true
        }
    }

    /// 获取配置状态
    pub fn get_status(&self) -> ConfigStatus {
        let current = self.current_config.as_ref();
        let next = self.next_config.as_ref();

        ConfigStatus {
            current_version: current.map(|c| c.version.clone()),
            current_expires_at: current.map(|c| c.wasm_expiry),
            next_version: next.map(|c| c.version.clone()),
            next_expires_at: next.map(|c| c.wasm_expiry),
            is_pre_generated: self.next_config.is_some(),
            wasm_ready: self.current_wasm.is_some(),
            next_wasm_ready: self.next_wasm.is_some(),
        }
    }
}

/// 配置状态
#[derive(Debug, Clone, Serialize)]
pub struct ConfigStatus {
    pub current_version: Option<String>,
    pub current_expires_at: Option<i64>,
    pub next_version: Option<String>,
    pub next_expires_at: Option<i64>,
    pub is_pre_generated: bool,
    pub wasm_ready: bool,
    pub next_wasm_ready: bool,
}

/// 路由轮换器 - 管理路由映射的版本和切换
pub struct RouteRotator {
    /// 当前活跃路由
    current_routes: HashMap<String, RouteInfo>,
    /// 下一批路由 (预生成)
    next_routes: Option<HashMap<String, RouteInfo>>,
    /// 路由版本
    version: String,
}

impl RouteRotator {
    /// 创建新的路由轮换器
    pub fn new() -> Self {
        Self {
            current_routes: HashMap::new(),
            next_routes: None,
            version: Uuid::new_v4().to_string(),
        }
    }

    /// 设置当前路由
    pub fn set_routes(&mut self, routes: HashMap<String, RouteInfo>) {
        self.current_routes = routes;
        self.version = Uuid::new_v4().to_string();
    }

    /// 预生成下一批路由
    pub fn pre_generate_next(&mut self, routes: HashMap<String, RouteInfo>) {
        self.next_routes = Some(routes);
    }

    /// 切换到下一批路由
    pub fn rotate(&mut self) {
        if let Some(next) = self.next_routes.take() {
            self.current_routes = next;
            self.version = Uuid::new_v4().to_string();
        }
    }

    /// 获取当前路由
    pub fn get_routes(&self) -> &HashMap<String, RouteInfo> {
        &self.current_routes
    }

    /// 获取路由版本
    pub fn get_version(&self) -> &str {
        &self.version
    }

    /// 添加路由
    pub fn add_route(&mut self, path: &str, info: RouteInfo) {
        self.current_routes.insert(path.to_string(), info);
    }

    /// 移除路由
    pub fn remove_route(&mut self, path: &str) {
        self.current_routes.remove(path);
    }
}

impl Default for RouteRotator {
    fn default() -> Self {
        Self::new()
    }
}

/// 生成随机 AES-256 密钥
fn generate_random_key() -> Vec<u8> {
    use rand::RngCore;
    let mut key = vec![0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key);
    key
}

/// WASM 配置结构 - 用于嵌入到 WASM 中
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    /// 密钥 (hex 编码)
    pub key: String,
    /// 过期时间戳
    pub expires_at: i64,
    /// 路由映射 (JSON 字符串)
    pub routes_json: String,
    /// 版本
    pub version: String,
}

impl WasmConfig {
    /// 从 SecureConfig 创建
    pub fn from_secure_config(config: &SecureConfig) -> Self {
        let routes_json = serde_json::to_string(&config.routes).unwrap_or_default();

        Self {
            key: config.key_pair.key_hex(),
            expires_at: config.wasm_expiry,
            routes_json,
            version: config.version.clone(),
        }
    }

    /// 检查是否过期
    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp() > self.expires_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_pair_creation() {
        let key_pair = KeyPair::new(24, KeyPurpose::WasmConfig);
        assert!(!key_pair.key.is_empty());
        assert_eq!(key_pair.key.len(), 32);
        assert!(!key_pair.is_expired());
    }

    #[test]
    fn test_key_pair_expiry() {
        let mut key_pair = KeyPair::new(0, KeyPurpose::WasmConfig); // 0 小时有效期
        assert!(key_pair.is_expired());
    }

    #[test]
    fn test_secure_config_creation() {
        let mut config = SecureConfig::new(24);
        config.add_route("/api/users", "/enc/abc123", Some("http://backend/users".to_string()));

        assert!(!config.is_expired());
        assert_eq!(config.routes.len(), 1);
    }

    #[test]
    fn test_key_manager_rotation() {
        let mut manager = KeyManager::new(1, 0); // 1小时有效期，0小时 margin
        let key1 = manager.get_or_create_key().clone();
        assert_eq!(manager.current_key.as_ref().unwrap().id, key1.id);

        // 强制轮换
        manager.rotate_key();
        let key2 = manager.get_or_create_key().clone();
        assert_ne!(key1.id, key2.id);
    }

    #[test]
    fn test_wasm_config_from_secure_config() {
        let mut config = SecureConfig::new(24);
        config.add_route("/test", "/enc/xyz", None);

        let wasm_config = WasmConfig::from_secure_config(&config);
        assert!(!wasm_config.key.is_empty());
        assert!(!wasm_config.routes_json.is_empty());
        assert!(!wasm_config.is_expired());
    }
}