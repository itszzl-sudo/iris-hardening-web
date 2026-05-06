//! 文件加密/解密

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use sha2::{Sha256, Digest};
use std::path::{Path, PathBuf};
use crate::Result;

pub struct FileEncryptor {
    cipher: Aes256Gcm,
}

#[derive(Debug, Clone)]
pub struct EncryptedPath {
    pub encrypted: String,
    pub real: PathBuf,
}

impl FileEncryptor {
    pub fn new(key: &[u8]) -> Result<Self> {
        let key_array: [u8; 32] = key.try_into()
            .map_err(|_| crate::Error::Encryption("Key must be 32 bytes".to_string()))?;
        
        let cipher = Aes256Gcm::new_from_slice(&key_array)
            .map_err(|e| crate::Error::Encryption(format!("Failed to create cipher: {}", e)))?;
        
        Ok(Self { cipher })
    }
    
    pub fn encrypt_filename(&self, filename: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(filename.as_bytes());
        let hash = hasher.finalize();
        BASE64.encode(hash)
    }
    
    pub fn encrypt_data(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(&[0u8; 12]);
        
        self.cipher.encrypt(nonce, plaintext)
            .map_err(|e| crate::Error::Encryption(format!("Encryption failed: {}", e)))
    }
    
    pub fn decrypt_data(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(&[0u8; 12]);
        
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
    
    pub fn encrypt_file_in_memory(&self, input: &Path) -> Result<Vec<u8>> {
        let plaintext = std::fs::read(input)?;
        self.encrypt_data(&plaintext)
    }
    
    pub fn decrypt_file_in_memory(&self, input: &Path) -> Result<Vec<u8>> {
        let ciphertext = std::fs::read(input)?;
        self.decrypt_data(&ciphertext)
    }
}

impl EncryptedPath {
    pub fn new(encrypted: String, real: PathBuf) -> Self {
        Self { encrypted, real }
    }
}
