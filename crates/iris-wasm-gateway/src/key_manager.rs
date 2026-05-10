//! 密钥管理

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use chrono::{DateTime, Utc, Duration};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use crate::Result;

/// Nonce size for AES-256-GCM (96 bits / 12 bytes)
const NONCE_SIZE: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyPair {
    pub id: Uuid,
    pub key: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub algorithm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    pub current: KeyPair,
    pub history: Vec<KeyPair>,
}

pub struct KeyManager {
    key_dir: PathBuf,
    validity: Duration,
}

impl KeyManager {
    pub fn new(key_dir: PathBuf, validity: Duration) -> Self {
        Self { key_dir, validity }
    }
    
    pub fn generate_key_pair(&self) -> Result<KeyPair> {
        let id = Uuid::new_v4();
        let key = Self::generate_random_key();
        let created_at = Utc::now();
        let expires_at = created_at + self.validity;
        
        Ok(KeyPair {
            id,
            key,
            created_at,
            expires_at,
            algorithm: "aes-256-gcm".to_string(),
        })
    }
    
    pub fn load_current(&self) -> Result<KeyPair> {
        let metadata_path = self.key_dir.join("metadata.json");
        
        if !metadata_path.exists() {
            return Err(crate::Error::Key("No key metadata found".to_string()));
        }
        
        let content = std::fs::read_to_string(&metadata_path)?;
        let metadata: KeyMetadata = serde_json::from_str(&content)
            .map_err(|e| crate::Error::Key(format!("Failed to parse metadata: {}", e)))?;
        
        Ok(metadata.current)
    }
    
    pub fn save_key_pair(&self, key_pair: &KeyPair) -> Result<()> {
        std::fs::create_dir_all(&self.key_dir)?;
        
        let metadata_path = self.key_dir.join("metadata.json");
        let key_file_path = self.key_dir.join("key.txt");
        
        let metadata = if metadata_path.exists() {
            let content = std::fs::read_to_string(&metadata_path)?;
            let mut m: KeyMetadata = serde_json::from_str(&content)
                .map_err(|e| crate::Error::Key(format!("Failed to parse metadata: {}", e)))?;
            m.history.push(m.current);
            m.current = key_pair.clone();
            m
        } else {
            KeyMetadata {
                current: key_pair.clone(),
                history: Vec::new(),
            }
        };
        
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        std::fs::write(&metadata_path, metadata_json)?;
        
        let key_hex = hex::encode(&key_pair.key);
        std::fs::write(&key_file_path, key_hex)?;
        
        tracing::info!("Key saved: id={}, expires={}", key_pair.id, key_pair.expires_at);
        Ok(())
    }
    
    pub fn is_expiring(&self, key_pair: &KeyPair, margin: Duration) -> bool {
        let now = Utc::now();
        let threshold = key_pair.expires_at - margin;
        now >= threshold
    }
    
    pub fn is_expired(&self, key_pair: &KeyPair) -> bool {
        Utc::now() >= key_pair.expires_at
    }
    
    fn generate_random_key() -> Vec<u8> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..32).map(|_| rng.gen::<u8>()).collect()
    }
}

impl KeyPair {
    pub fn to_hex(&self) -> String {
        hex::encode(&self.key)
    }

    /// Encrypt with random nonce. Output: [12-byte nonce][ciphertext]
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let key_array: [u8; 32] = self.key.clone().try_into()
            .map_err(|_| crate::Error::Key("Invalid key length".to_string()))?;

        let cipher = Aes256Gcm::new_from_slice(&key_array)
            .map_err(|e| crate::Error::Key(format!("Failed to create cipher: {}", e)))?;

        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, plaintext)
            .map_err(|e| crate::Error::Key(format!("Encryption failed: {}", e)))?;

        let mut output = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        output.extend_from_slice(&nonce_bytes);
        output.extend_from_slice(&ciphertext);

        Ok(output)
    }

    /// Decrypt with embedded nonce. Input: [12-byte nonce][ciphertext]
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        let key_array: [u8; 32] = self.key.clone().try_into()
            .map_err(|_| crate::Error::Key("Invalid key length".to_string()))?;

        let cipher = Aes256Gcm::new_from_slice(&key_array)
            .map_err(|e| crate::Error::Key(format!("Failed to create cipher: {}", e)))?;

        if data.len() < NONCE_SIZE {
            return Err(crate::Error::Key("Data too short to contain nonce".to_string()));
        }

        let (nonce_bytes, ciphertext) = data.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        cipher.decrypt(nonce, ciphertext)
            .map_err(|e| crate::Error::Key(format!("Decryption failed: {}", e)))
    }
}
