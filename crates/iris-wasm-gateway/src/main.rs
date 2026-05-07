//! Iris WASM Gateway 服务器

use iris_wasm_gateway::{Config, KeyManager, WasmScheduler};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".to_string());

    tracing::info!("Loading config from: {}", config_path);
    let config = Config::from_file(&config_path)?;

    tracing::info!("Starting Iris WASM Gateway v{}", iris_wasm_gateway::VERSION);
    tracing::info!("Gateway: {}:{}", config.server.host, config.server.port);
    tracing::info!("Key validity: {} hours", config.key.validity_hours);

    let cloudflare_config = config.load_cloudflare_config()?;
    if cloudflare_config.is_some() {
        tracing::info!("Cloudflare config loaded");
    }

    let key_manager = KeyManager::new(
        config.key.key_dir.clone(),
        config.validity_duration()
    );

    let scheduler_config = iris_wasm_gateway::scheduler::SchedulerConfig {
        check_interval_minutes: config.scheduler.check_interval_minutes,
        generation_lead_time_hours: config.scheduler.generation_lead_time_hours,
        wasm_output_dir: config.scheduler.wasm_output_dir.clone(),
    };

    let cf_config = cloudflare_config.map(|cf| {
        iris_wasm_gateway::cloudflare::CloudflareConfig {
            api_token: cf.api_token,
            account_id: cf.account_id,
            project_name: cf.project_name,
            deployment_branch: cf.deployment_branch,
        }
    });

    let scheduler = Arc::new(WasmScheduler::new(
        key_manager,
        cf_config,
        scheduler_config,
        config.encrypt_service.url.clone(),
    ));

    tokio::spawn(async move {
        if let Err(e) = scheduler.start().await {
            tracing::error!("Scheduler error: {}", e);
        }
    });

    tracing::info!("WASM Gateway started - scheduler running in background");

    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down...");

    Ok(())
}
