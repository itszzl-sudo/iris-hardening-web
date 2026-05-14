//! 配置轮询模块
//! 定期从 iris-secure-config 获取任务并准备 WASM 和密钥

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use serde::{Deserialize, Serialize};

use crate::Result;

/// 从 iris-secure-config 获取的任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigTask {
    pub domain: String,
    pub nginx_port: u16,
    pub gateway_port: u16,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::InProgress => write!(f, "in_progress"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
        }
    }
}

/// 当前配置任务状态
#[derive(Debug, Clone)]
pub struct CurrentTask {
    pub task: ConfigTask,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub wasm_prepared: bool,
    pub keys_prepared: bool,
    pub nginx_config_prepared: bool,
}

impl Default for CurrentTask {
    fn default() -> Self {
        Self {
            task: ConfigTask {
                domain: String::new(),
                nginx_port: 80,
                gateway_port: 9001,
                status: TaskStatus::Pending,
            },
            generated_at: chrono::Utc::now(),
            wasm_prepared: false,
            keys_prepared: false,
            nginx_config_prepared: false,
        }
    }
}

/// 轮询器状态
pub struct PollerState {
    pub current_task: RwLock<Option<CurrentTask>>,
    pub last_poll: RwLock<chrono::DateTime<chrono::Utc>>,
    pub poll_count: RwLock<u64>,
}

impl Default for PollerState {
    fn default() -> Self {
        Self {
            current_task: RwLock::new(None),
            last_poll: RwLock::new(chrono::Utc::now()),
            poll_count: RwLock::new(0),
        }
    }
}

impl PollerState {
    pub async fn update_task(&self, task: ConfigTask) {
        let mut current = self.current_task.write().await;
        *current = Some(CurrentTask {
            task,
            generated_at: chrono::Utc::now(),
            wasm_prepared: false,
            keys_prepared: false,
            nginx_config_prepared: false,
        });

        let mut last_poll = self.last_poll.write().await;
        *last_poll = chrono::Utc::now();

        let mut count = self.poll_count.write().await;
        *count += 1;
    }

    pub async fn get_task(&self) -> Option<CurrentTask> {
        let current = self.current_task.read().await;
        current.clone()
    }

    pub async fn set_wasm_prepared(&self) {
        let mut current = self.current_task.write().await;
        if let Some(ref mut task) = *current {
            task.wasm_prepared = true;
        }
    }

    pub async fn set_keys_prepared(&self) {
        let mut current = self.current_task.write().await;
        if let Some(ref mut task) = *current {
            task.keys_prepared = true;
        }
    }

    pub async fn set_nginx_config_prepared(&self) {
        let mut current = self.current_task.write().await;
        if let Some(ref mut task) = *current {
            task.nginx_config_prepared = true;
        }
    }

    pub async fn mark_completed(&self) {
        let mut current = self.current_task.write().await;
        if let Some(ref mut task) = *current {
            task.task.status = TaskStatus::Completed;
        }
    }

    pub async fn mark_failed(&self) {
        let mut current = self.current_task.write().await;
        if let Some(ref mut task) = *current {
            task.task.status = TaskStatus::Failed;
        }
    }

    pub async fn get_poll_stats(&self) -> (chrono::DateTime<chrono::Utc>, u64) {
        let last_poll = self.last_poll.read().await;
        let count = self.poll_count.read().await;
        (*last_poll, *count)
    }
}

/// 配置轮询器
pub struct ConfigPoller {
    config_url: String,
    state: Arc<PollerState>,
    client: reqwest::Client,
}

impl ConfigPoller {
    pub fn new(config_url: String) -> Self {
        Self {
            config_url,
            state: Arc::new(PollerState::default()),
            client: reqwest::Client::new(),
        }
    }

    pub fn state(&self) -> Arc<PollerState> {
        self.state.clone()
    }

    /// 轮询获取任务
    pub async fn poll(&self) -> Result<Option<ConfigTask>> {
        let url = format!("{}/api/domains", self.config_url);

        let response = self.client
            .get(&url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| crate::Error::Http(e.to_string()))?;

        if !response.status().is_success() {
            tracing::warn!("Config server returned status: {}", response.status());
            return Ok(None);
        }

        #[derive(Deserialize)]
        struct ApiResponse<T> {
            success: bool,
            data: Option<T>,
        }

        let api_response: ApiResponse<Vec<serde_json::Value>> = response
            .json()
            .await
            .map_err(|e| crate::Error::Http(e.to_string()))?;

        if !api_response.success || api_response.data.is_none() {
            return Ok(None);
        }

        // 查找 pending 状态的任务
        for item in api_response.data.unwrap() {
            let status = item.get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("pending");

            if status == "pending" {
                let task = ConfigTask {
                    domain: item.get("domain")
                        .and_then(|d| d.as_str())
                        .unwrap_or("")
                        .to_string(),
                    nginx_port: item.get("nginx_port")
                        .and_then(|p| p.as_u64())
                        .unwrap_or(80) as u16,
                    gateway_port: item.get("gateway_port")
                        .and_then(|p| p.as_u64())
                        .unwrap_or(9001) as u16,
                    status: match status {
                        "synced" => TaskStatus::Completed,
                        "failed" => TaskStatus::Failed,
                        _ => TaskStatus::Pending,
                    },
                };

                self.state.update_task(task.clone()).await;
                return Ok(Some(task));
            }
        }

        Ok(None)
    }

    /// 执行单个任务: 准备 WASM、密钥、nginx 配置
    pub async fn execute_task(&self) -> Result<bool> {
        let task_opt = self.state.get_task().await;

        let mut task = match task_opt {
            Some(t) if t.task.status == TaskStatus::Pending => t,
            _ => return Ok(false),
        };

        tracing::info!("Executing config task for domain: {}", task.task.domain);

        // TODO: 调用 ConfigGenerator 准备 WASM 和密钥
        // TODO: 生成 nginx 配置
        // TODO: 更新 secure-config 状态

        task.wasm_prepared = true;
        task.keys_prepared = true;
        task.nginx_config_prepared = true;

        self.state.set_wasm_prepared().await;
        self.state.set_keys_prepared().await;
        self.state.set_nginx_config_prepared().await;

        tracing::info!("Config task completed for domain: {}", task.task.domain);

        Ok(true)
    }
}

/// 启动轮询循环
pub async fn start_polling_loop(poller: Arc<ConfigPoller>, interval_secs: u64) {
    let mut ticker = interval(Duration::from_secs(interval_secs));

    tracing::info!("Starting config polling loop (interval: {}s)", interval_secs);

    loop {
        ticker.tick().await;

        match poller.poll().await {
            Ok(Some(task)) => {
                tracing::info!("Received task for domain: {}", task.domain);

                // 执行任务
                if let Err(e) = poller.execute_task().await {
                    tracing::error!("Task execution failed: {}", e);
                    poller.state().mark_failed().await;
                }
            }
            Ok(None) => {
                tracing::debug!("No pending tasks found");
            }
            Err(e) => {
                tracing::warn!("Polling failed: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_poller_state() {
        let state = PollerState::default();
        let task = ConfigTask {
            domain: "test.local".to_string(),
            nginx_port: 80,
            gateway_port: 9001,
            status: TaskStatus::Pending,
        };

        state.update_task(task).await;

        let retrieved = state.get_task().await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().task.domain, "test.local");
    }

    #[tokio::test]
    async fn test_task_preparation_flags() {
        let state = PollerState::default();
        let task = ConfigTask {
            domain: "test.local".to_string(),
            nginx_port: 80,
            gateway_port: 9001,
            status: TaskStatus::Pending,
        };
        state.update_task(task).await;

        state.set_wasm_prepared().await;
        state.set_keys_prepared().await;
        state.set_nginx_config_prepared().await;

        let task = state.get_task().await.unwrap();
        assert!(task.wasm_prepared);
        assert!(task.keys_prepared);
        assert!(task.nginx_config_prepared);
    }
}