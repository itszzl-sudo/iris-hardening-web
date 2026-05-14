//! AES-256-GCM 加密/解密
//!
//! 每次加密使用随机 nonce，输出格式: [12字节 nonce][密文]

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use sha2::{Sha256, Digest};
use std::path::Path;
use crate::Result;

/// AES-256-GCM nonce 大小 (96 位 / 12 字节)
const NONCE_SIZE: usize = 12;

pub struct FileEncryptor {
    cipher: Aes256Gcm,
}

impl FileEncryptor {
    pub fn new(key: &[u8]) -> Result<Self> {
        let key_array: [u8; 32] = key.try_into()
            .map_err(|_| crate::Error::Encryption("Key must be 32 bytes".to_string()))?;

        let cipher = Aes256Gcm::new_from_slice(&key_array)
            .map_err(|e| crate::Error::Encryption(format!("Failed to create cipher: {}", e)))?;

        Ok(Self { cipher })
    }

    /// 从 hex 编码的密钥创建
    pub fn from_hex(hex_key: &str) -> Result<Self> {
        let key_bytes = hex::decode(hex_key)
            .map_err(|e| crate::Error::Encryption(format!("Invalid hex key: {}", e)))?;
        Self::new(&key_bytes)
    }

    /// 用当前密钥重新初始化（密钥轮换时使用）
    pub fn rotate(&mut self, key: &[u8]) -> Result<()> {
        let key_array: [u8; 32] = key.try_into()
            .map_err(|_| crate::Error::Encryption("Key must be 32 bytes".to_string()))?;

        self.cipher = Aes256Gcm::new_from_slice(&key_array)
            .map_err(|e| crate::Error::Encryption(format!("Failed to create cipher: {}", e)))?;

        Ok(())
    }

    /// Clone the inner cipher so callers can decrypt with the same key
    /// without holding the RwLock read guard across an await point.
    pub fn clone_cipher(&self) -> Self {
        // Aes256Gcm implements Clone
        Self { cipher: self.cipher.clone() }
    }

    /// 文件名加密（确定性 SHA-256 + base64）
    pub fn encrypt_filename(&self, filename: &str) -> String {
        let hash = Sha256::digest(filename.as_bytes());
        BASE64.encode(hash)
    }

    /// 加密数据，使用随机 nonce。
    /// 输出格式: [12字节 nonce][密文]
    pub fn encrypt_data(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self.cipher.encrypt(nonce, plaintext)
            .map_err(|e| crate::Error::Encryption(format!("Encryption failed: {}", e)))?;

        let mut output = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);

        Ok(output)
    }

    /// 解密数据。
    /// 输入格式: [12字节 nonce][密文]
    pub fn decrypt_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < NONCE_SIZE {
            return Err(crate::Error::Encryption("Data too short to contain nonce".to_string()));
        }

        let (nonce_bytes, ciphertext) = data.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        self.cipher.decrypt(nonce, ciphertext)
            .map_err(|e| crate::Error::Encryption(format!("Decryption failed: {}", e)))
    }

    pub fn encrypt_file(&self, input: &Path, output: &Path) -> Result<()> {
        let plaintext = std::fs::read(input)?;
        let ciphertext = self.encrypt_data(&plaintext)?;
        std::fs::write(output, ciphertext)?;
        Ok(())
    }

    pub fn decrypt_file(&self, input: &Path, output: &Path) -> Result<()> {
        let ciphertext = std::fs::read(input)?;
        let plaintext = self.decrypt_data(&ciphertext)?;
        std::fs::write(output, plaintext)?;
        Ok(())
    }
}
