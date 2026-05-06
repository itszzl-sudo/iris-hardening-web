//! 路径转换

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
    
    pub fn encrypt_path(&self, real_path: &str) -> String {
        if let Some(encrypted) = self.mappings.get(real_path) {
            return encrypted.clone();
        }
        
        let hash = Sha256::digest(real_path.as_bytes());
        URL_SAFE_NO_PAD.encode(hash)
    }
    
    pub fn decrypt_path(&self, encrypted_path: &str) -> Option<String> {
        self.reverse_mappings.get(encrypted_path).cloned()
    }
    
    pub fn encrypt_api_path(&self, api_path: &str) -> String {
        let hash = Sha256::digest(api_path.as_bytes());
        format!("api/{}", URL_SAFE_NO_PAD.encode(hash))
    }
    
    pub fn add_mapping(&mut self, encrypted: String, real: String) {
        self.mappings.insert(encrypted.clone(), real.clone());
        self.reverse_mappings.insert(real, encrypted);
    }
    
    pub fn remove_mapping(&mut self, encrypted: &str) {
        if let Some(real) = self.mappings.remove(encrypted) {
            self.reverse_mappings.remove(&real);
        }
    }
}

impl Default for PathTransformer {
    fn default() -> Self {
        Self::new(HashMap::new())
    }
}
