//! Cloudflare Pages API 集成

use reqwest::multipart;
use serde::{Deserialize, Serialize};
use std::path::Path;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareConfig {
    pub api_token: String,
    pub account_id: String,
    pub project_name: String,
    pub deployment_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentResult {
    pub id: String,
    pub url: String,
    pub created_on: String,
    pub modified_on: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareResponse<T> {
    pub result: T,
    pub success: bool,
    #[serde(default)]
    pub errors: Vec<CloudflareError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareError {
    pub code: i32,
    pub message: String,
}

pub struct CloudflareUploader {
    config: CloudflareConfig,
    client: reqwest::Client,
}

impl CloudflareUploader {
    pub fn new(config: CloudflareConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client");

        Self { config, client }
    }

    pub async fn upload_wasm(&self, wasm_path: &Path) -> Result<DeploymentResult> {
        let wasm_data = std::fs::read(wasm_path)?;
        let wasm_filename = wasm_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("iris.wasm");

        tracing::info!("Uploading WASM to Cloudflare Pages: {} ({} bytes)", 
            wasm_filename, wasm_data.len());

        let form = multipart::Form::new()
            .part("file", multipart::Part::bytes(wasm_data)
                .file_name(wasm_filename.to_string())
                .mime_str("application/wasm")
                .map_err(|e| crate::Error::Config(format!("MIME error: {}", e)))?);

        let branch = self.config.deployment_branch.as_deref().unwrap_or("main");
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/pages/{}/deployments",
            self.config.account_id,
            self.config.project_name
        );

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .multipart(form)
            .query(&[("branch", branch)])
            .send()
            .await
            .map_err(|e| crate::Error::Config(format!("Upload failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::Error::Config(format!(
                "Cloudflare API error: {} - {}", status, body
            )));
        }

        let cf_response: CloudflareResponse<DeploymentResult> = response.json().await
            .map_err(|e| crate::Error::Config(format!("Failed to parse response: {}", e)))?;

        if !cf_response.success {
            let errors: Vec<String> = cf_response.errors.iter()
                .map(|e| format!("{}: {}", e.code, e.message))
                .collect();
            return Err(crate::Error::Config(format!(
                "Deployment failed: {}", errors.join(", ")
            )));
        }

        tracing::info!("WASM uploaded successfully: {}", cf_response.result.url);
        Ok(cf_response.result)
    }

    pub async fn get_latest_deployment(&self) -> Result<Option<DeploymentResult>> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/pages/{}/deployments",
            self.config.account_id,
            self.config.project_name
        );

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_token))
            .send()
            .await
            .map_err(|e| crate::Error::Config(format!("API request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(crate::Error::Config(format!(
                "Failed to get deployments: {}", response.status()
            )));
        }

        #[derive(Debug, Deserialize)]
        struct DeploymentList {
            result: Vec<DeploymentResult>,
        }

        let list: DeploymentList = response.json().await
            .map_err(|e| crate::Error::Config(format!("Failed to parse response: {}", e)))?;

        Ok(list.result.into_iter().next())
    }

    pub fn get_wasm_url(&self, deployment_id: &str) -> String {
        format!(
            "https://{}.pages.dev/iris.wasm",
            self.config.project_name
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cloudflare_config_serialization() {
        let config = CloudflareConfig {
            api_token: "test_token".to_string(),
            account_id: "test_account".to_string(),
            project_name: "test_project".to_string(),
            deployment_branch: Some("production".to_string()),
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("test_token"));
    }
}
