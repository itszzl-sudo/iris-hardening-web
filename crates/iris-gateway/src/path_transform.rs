//! 路径转换
//!
//! 双向映射: 真实路径 <-> 加密路径
//! - `mappings`: real_path -> encrypted_path
//! - `reverse_mappings`: encrypted_path -> real_path

use sha2::{Sha256, Digest};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use std::collections::HashMap;
use crate::Result;

pub struct PathTransformer {
    mappings: HashMap<String, String>,
    reverse_mappings: HashMap<String, String>,
}

impl PathTransformer {
    pub fn new(mappings: HashMap<String, String>) -> Self {
        let reverse_mappings = mappings
            .iter()
            .map(|(k, v)| (v.clone(), k.clone()))
            .collect();

        Self { mappings, reverse_mappings }
    }

    /// 真实路径 -> 加密路径
    pub fn encrypt_path(&self, real_path: &str) -> String {
        if let Some(encrypted) = self.mappings.get(real_path) {
            return encrypted.clone();
        }

        let hash = Sha256::digest(real_path.as_bytes());
        URL_SAFE_NO_PAD.encode(hash)
    }

    /// 真实路径 -> 加密路径，仅当路径在映射表中时返回 Some
    /// 未映射的路径不会被加密（用于 HTML URI 替换，只替换已映射的资源）
    pub fn encrypt_path_if_mapped(&self, real_path: &str) -> Option<String> {
        self.mappings.get(real_path).cloned()
    }

    /// 加密路径 -> 真实路径
    pub fn decrypt_path(&self, encrypted_path: &str) -> Option<String> {
        self.reverse_mappings.get(encrypted_path).cloned()
    }

    /// API 路径加密
    pub fn encrypt_api_path(&self, api_path: &str) -> String {
        let hash = Sha256::digest(api_path.as_bytes());
        format!("api/{}", URL_SAFE_NO_PAD.encode(hash))
    }

    /// 添加映射 (真实路径, 加密路径)
    pub fn add_mapping(&mut self, real: String, encrypted: String) {
        self.mappings.insert(real.clone(), encrypted.clone());
        self.reverse_mappings.insert(encrypted, real);
    }

    /// 删除映射
    pub fn remove_mapping(&mut self, real: &str) {
        if let Some(encrypted) = self.mappings.remove(real) {
            self.reverse_mappings.remove(&encrypted);
        }
    }
}

impl Default for PathTransformer {
    fn default() -> Self {
        Self::new(HashMap::new())
    }
}
