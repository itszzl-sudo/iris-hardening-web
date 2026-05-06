//! Iris Secure Gateway - 安全网关

pub mod config;
pub mod encrypt;

pub use config::Config;
pub use encrypt::FileEncryptor;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Encryption error: {0}")]
    Encryption(String),
    
    #[error("Config error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
    }
    
    #[test]
    fn test_encryptor_key_size() {
        let key = vec![0u8; 32];
        let encryptor = FileEncryptor::new(&key);
        assert!(encryptor.is_ok());
    }
    
    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = vec![1u8; 32];
        let encryptor = FileEncryptor::new(&key).unwrap();
        
        let plaintext = b"Hello, World!";
        let encrypted = encryptor.encrypt_data(plaintext).unwrap();
        let decrypted = encryptor.decrypt_data(&encrypted).unwrap();
        
        assert_eq!(plaintext.to_vec(), decrypted);
    }
    
    #[test]
    fn test_filename_encryption() {
        let key = vec![2u8; 32];
        let encryptor = FileEncryptor::new(&key).unwrap();
        
        let filename = "test/document.pdf";
        let encrypted1 = encryptor.encrypt_filename(filename);
        let encrypted2 = encryptor.encrypt_filename(filename);
        
        assert_eq!(encrypted1, encrypted2);
        assert!(!encrypted1.is_empty());
    }
}
