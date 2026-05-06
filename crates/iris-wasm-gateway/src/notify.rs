//! 通知 iris-encrypt 更新密钥

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use crate::{KeyPair, Result};

pub struct Notifier {
    client: Client,
    encrypt_url: String,
    endpoint: String,
}

#[derive(Debug, Serialize)]
struct UpdateKeyRequest {
    key_id: String,
    key: String,
    expires_at: String,
}

#[derive(Debug, Deserialize)]
struct UpdateKeyResponse {
    success: bool,
    message: String,
}

impl Notifier {
    pub fn new(encrypt_url: String, endpoint: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| crate::Error::Http(format!("Failed to create client: {}", e)))?;
        
        Ok(Self { client, encrypt_url, endpoint })
    }
    
    pub async fn notify_key_update(&self, key_pair: &KeyPair) -> Result<()> {
        let url = format!("{}{}", self.encrypt_url, self.endpoint);
        
        let request = UpdateKeyRequest {
            key_id: key_pair.id.to_string(),
            key: key_pair.to_hex(),
            expires_at: key_pair.expires_at.to_rfc3339(),
        };
        
        tracing::info!("Notifying iris-encrypt: {}", url);
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| crate::Error::Http(format!("Request failed: {}", e)))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::Error::Http(format!("Update failed: {} - {}", status, body)));
        }
        
        let result: UpdateKeyResponse = response.json()
            .await
            .map_err(|e| crate::Error::Http(format!("Invalid response: {}", e)))?;
        
        if result.success {
            tracing::info!("Key update successful: {}", result.message);
        } else {
            return Err(crate::Error::Http(format!("Key update failed: {}", result.message)));
        }
        
        Ok(())
    }
}
