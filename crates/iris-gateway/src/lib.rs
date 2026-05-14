//! Iris Gateway - 统一安全网关
//!
//! 合并 iris-secure-gateway 和 iris-wasm-gateway 的能力，
//! 部署在 nginx 和内部服务器之间，提供:
//! - 加密文件分发和 API 代理
//! - 密钥生成、轮换和持久化
//! - iris.wasm 分发
//! - 健康检查（供 nginx upstream_check）

pub mod config;
pub mod crypto;
pub mod encrypt;
pub mod key_manager;
pub mod path_transform;
pub mod proxy;
pub mod server;

pub use config::Config;
pub use encrypt::FileEncryptor;
pub use key_manager::{KeyManager, KeyPair};
pub use path_transform::PathTransformer;
pub use proxy::{ApiProxy, ProxyResponse};
pub use server::IrisGateway;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Key error: {0}")]
    Key(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.health_check_path, "/health");
        assert!(!config.is_encryption_active());
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
    fn test_encryptor_rotate() {
        let key1 = vec![1u8; 32];
        let mut encryptor = FileEncryptor::new(&key1).unwrap();

        let plaintext = b"Test data";
        let encrypted = encryptor.encrypt_data(plaintext).unwrap();

        // 轮换到新密钥
        let key2 = vec![2u8; 32];
        encryptor.rotate(&key2).unwrap();

        // 旧密钥加密的数据无法用新密钥解密
        assert!(encryptor.decrypt_data(&encrypted).is_err());

        // 新密钥自己的加解密正常
        let encrypted2 = encryptor.encrypt_data(plaintext).unwrap();
        let decrypted2 = encryptor.decrypt_data(&encrypted2).unwrap();
        assert_eq!(plaintext.to_vec(), decrypted2);
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

    #[test]
    fn test_different_nonces() {
        let key = vec![3u8; 32];
        let encryptor = FileEncryptor::new(&key).unwrap();

        let plaintext = b"Same data";
        let encrypted1 = encryptor.encrypt_data(plaintext).unwrap();
        let encrypted2 = encryptor.encrypt_data(plaintext).unwrap();

        // 相同明文，不同 nonce，密文应该不同
        assert_ne!(encrypted1, encrypted2);

        // 但都能正确解密
        assert_eq!(plaintext.to_vec(), encryptor.decrypt_data(&encrypted1).unwrap());
        assert_eq!(plaintext.to_vec(), encryptor.decrypt_data(&encrypted2).unwrap());
    }

    #[test]
    fn test_path_transformer() {
        let mut mappings = std::collections::HashMap::new();
        mappings.insert("secret/doc.pdf".to_string(), "abc123".to_string());

        let transformer = PathTransformer::new(mappings);

        // 已知映射
        assert_eq!(transformer.encrypt_path("secret/doc.pdf"), "abc123");
        assert_eq!(transformer.decrypt_path("abc123"), Some("secret/doc.pdf".to_string()));

        // 未知路径 -> SHA-256 hash
        let encrypted = transformer.encrypt_path("unknown/path.txt");
        assert!(!encrypted.is_empty());
        assert_ne!(encrypted, "unknown/path.txt");
        // 未知路径的加密结果无法反向解密
        assert_eq!(transformer.decrypt_path(&encrypted), None);
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(crypto::constant_time_eq(b"hello", b"hello"));
        assert!(!crypto::constant_time_eq(b"hello", b"world"));
        assert!(!crypto::constant_time_eq(b"hello", b"hell"));
    }
}
