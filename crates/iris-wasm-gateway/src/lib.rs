//! Iris WASM Gateway - WASM 代理分发和密钥管理

pub mod config;
pub mod key_manager;
pub mod notify;
pub mod server;
pub mod wasm_generator;
pub mod wasm_proxy;

pub use config::Config;
pub use key_manager::{KeyManager, KeyPair};
pub use notify::Notifier;
pub use server::GatewayServer;
pub use wasm_generator::WasmGenerator;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Key error: {0}")]
    Key(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_key_pair_creation() {
        let key_manager = KeyManager::new(std::path::PathBuf::from("test_keys"), chrono::Duration::hours(24));
        let key_pair = key_manager.generate_key_pair();
        
        assert!(key_pair.is_ok());
        let kp = key_pair.unwrap();
        assert_eq!(kp.key.len(), 32);
        assert!(kp.expires_at > kp.created_at);
    }
    
    #[test]
    fn test_key_encryption_roundtrip() {
        let key_manager = KeyManager::new(std::path::PathBuf::from("test_keys"), chrono::Duration::hours(24));
        let key_pair = key_manager.generate_key_pair().unwrap();
        
        let plaintext = b"Test data";
        let encrypted = key_pair.encrypt(plaintext).unwrap();
        let decrypted = key_pair.decrypt(&encrypted).unwrap();
        
        assert_eq!(plaintext.to_vec(), decrypted);
    }
}
