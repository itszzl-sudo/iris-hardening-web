//! WASM 定时生成与上传调度器

use chrono::{DateTime, Duration, Utc};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::{KeyManager, KeyPair, WasmGenerator, Result};
use crate::cloudflare::{CloudflareUploader, CloudflareConfig, DeploymentResult};

#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub check_interval_minutes: u64,
    pub generation_lead_time_hours: u64,
    pub wasm_output_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SchedulerState {
    pub current_key_id: String,
    pub current_deployment: Option<DeploymentResult>,
    pub next_rotation: DateTime<Utc>,
    pub last_rotation: Option<DateTime<Utc>>,
    pub total_rotations: u64,
}

pub struct WasmScheduler {
    key_manager: Arc<KeyManager>,
    wasm_generator: WasmGenerator,
    cloudflare: Option<Arc<CloudflareUploader>>,
    config: SchedulerConfig,
    state: Arc<RwLock<SchedulerState>>,
    encrypt_service_url: String,
}

impl WasmScheduler {
    pub fn new(
        key_manager: KeyManager,
        cloudflare_config: Option<CloudflareConfig>,
        scheduler_config: SchedulerConfig,
        encrypt_service_url: String,
    ) -> Self {
        let state = SchedulerState {
            current_key_id: String::new(),
            current_deployment: None,
            next_rotation: Utc::now(),
            last_rotation: None,
            total_rotations: 0,
        };

        let cloudflare = cloudflare_config.map(|c| Arc::new(CloudflareUploader::new(c)));

        Self {
            key_manager: Arc::new(key_manager),
            wasm_generator: WasmGenerator::new(),
            cloudflare,
            config: scheduler_config,
            state: Arc::new(RwLock::new(state)),
            encrypt_service_url,
        }
    }

    pub async fn start(&self) -> Result<()> {
        tracing::info!("Starting WASM scheduler");
        
        self.initialize().await?;

        let interval_duration = std::time::Duration::from_secs(
            self.config.check_interval_minutes * 60
        );

        loop {
            tokio::time::sleep(interval_duration).await;

            if let Err(e) = self.check_and_rotate().await {
                tracing::error!("Rotation check failed: {}", e);
            }
        }
    }

    async fn initialize(&self) -> Result<()> {
        let key_pair = self.key_manager.load_current()
            .or_else(|_| self.key_manager.generate_key_pair())?;

        self.generate_and_upload(&key_pair).await?;

        let mut state = self.state.write().await;
        state.current_key_id = key_pair.id.to_string();
        state.next_rotation = key_pair.expires_at - Duration::hours(self.config.generation_lead_time_hours as i64);

        tracing::info!("Scheduler initialized with key: {}", key_pair.id);
        Ok(())
    }

    async fn check_and_rotate(&self) -> Result<()> {
        let state = self.state.read().await;
        let now = Utc::now();

        if now < state.next_rotation {
            tracing::debug!("Not yet time for rotation. Next: {}", state.next_rotation);
            return Ok(());
        }
        drop(state);

        tracing::info!("Starting key rotation");

        let new_key_pair = self.key_manager.generate_key_pair()?;
        self.key_manager.save_key_pair(&new_key_pair)?;

        self.generate_and_upload(&new_key_pair).await?;

        let mut state = self.state.write().await;
        state.current_key_id = new_key_pair.id.to_string();
        state.next_rotation = new_key_pair.expires_at - Duration::hours(self.config.generation_lead_time_hours as i64);
        state.last_rotation = Some(now);
        state.total_rotations += 1;

        tracing::info!(
            "Key rotated: id={}, next_rotation={}", 
            new_key_pair.id, 
            state.next_rotation
        );

        Ok(())
    }

    async fn generate_and_upload(&self, key_pair: &KeyPair) -> Result<()> {
        std::fs::create_dir_all(&self.config.wasm_output_dir)?;

        let wasm_filename = format!("iris-{}.wasm", key_pair.id);
        let wasm_path = self.config.wasm_output_dir.join(&wasm_filename);

        self.wasm_generator.generate_to_file(
            key_pair,
            &self.encrypt_service_url,
            &wasm_path
        )?;

        tracing::info!("Generated WASM: {:?}", wasm_path);

        if let Some(cloudflare) = &self.cloudflare {
            match cloudflare.upload_wasm(&wasm_path).await {
                Ok(deployment) => {
                    let mut state = self.state.write().await;
                    state.current_deployment = Some(deployment.clone());
                    tracing::info!("Uploaded to Cloudflare: {}", deployment.url);
                }
                Err(e) => {
                    tracing::error!("Cloudflare upload failed: {}", e);
                }
            }
        }

        self.cleanup_old_wasm_files().await?;

        Ok(())
    }

    async fn cleanup_old_wasm_files(&self) -> Result<()> {
        let entries: Vec<_> = std::fs::read_dir(&self.config.wasm_output_dir)?
            .filter_map(|e| e.ok())
            .collect();

        if entries.len() > 5 {
            let mut entries_with_time: Vec<_> = entries
                .into_iter()
                .filter_map(|e| {
                    let metadata = e.metadata().ok()?;
                    let modified = metadata.modified().ok()?;
                    Some((e, modified))
                })
                .collect();

            entries_with_time.sort_by_key(|(_, time)| std::cmp::Reverse(*time));

            for (entry, _) in entries_with_time.into_iter().skip(5) {
                if let Err(e) = std::fs::remove_file(entry.path()) {
                    tracing::warn!("Failed to remove old WASM: {}", e);
                }
            }
        }

        Ok(())
    }

    pub async fn get_state(&self) -> SchedulerState {
        self.state.read().await.clone()
    }

    pub async fn force_rotate(&self) -> Result<()> {
        tracing::info!("Force key rotation triggered");
        self.check_and_rotate().await
    }
}

pub struct ManualRotation;

impl ManualRotation {
    pub fn generate_wasm_once(
        key_manager: &KeyManager,
        wasm_generator: &WasmGenerator,
        encrypt_service_url: &str,
        output_path: &PathBuf,
    ) -> Result<PathBuf> {
        let key_pair = key_manager.generate_key_pair()?;
        key_manager.save_key_pair(&key_pair)?;

        wasm_generator.generate_to_file(&key_pair, encrypt_service_url, output_path)?;

        tracing::info!("Generated WASM: {:?}", output_path);
        Ok(output_path.clone())
    }

    pub async fn upload_wasm_once(
        cloudflare: &CloudflareUploader,
        wasm_path: &PathBuf,
    ) -> Result<DeploymentResult> {
        cloudflare.upload_wasm(wasm_path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_config() {
        let config = SchedulerConfig {
            check_interval_minutes: 30,
            generation_lead_time_hours: 2,
            wasm_output_dir: PathBuf::from("wasm"),
        };

        assert_eq!(config.check_interval_minutes, 30);
    }
}
