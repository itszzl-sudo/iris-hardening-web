//! Iris Secure Gateway 服务器

use iris_secure_gateway::{Config, FileEncryptor};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Duration;

pub struct GatewayContext {
    config: Config,
    encryptor: FileEncryptor,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".to_string());
    
    tracing::info!("Loading config from: {}", config_path);
    let config = Config::from_file(&config_path)?;
    
    tracing::info!("Starting Iris Secure Gateway v{}", iris_secure_gateway::VERSION);
    tracing::info!("Server: {}:{}", config.server.host, config.server.port);
    tracing::info!("Base dir: {:?}", config.server.base_dir);
    tracing::info!("Assets dir: {:?}", config.server.assets_dir);
    
    let key = config.load_key()?;
    let encryptor = FileEncryptor::new(&key)?;
    
    let context = Arc::new(RwLock::new(GatewayContext {
        config: config.clone(),
        encryptor,
    }));
    
    if config.key_rotation.enabled {
        let ctx = context.clone();
        let cfg = config.clone();
        tokio::spawn(async move {
            key_rotation_task(ctx, cfg).await;
        });
    }
    
    tracing::info!("Secure Gateway initialized successfully");
    tracing::info!("Access iris.wasm at: http://{}:{}/iris.wasm", config.server.host, config.server.port);
    tracing::info!("Access index.html at: http://{}:{}/index.html", config.server.host, config.server.port);
    
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down...");
    
    Ok(())
}

async fn key_rotation_task(
    context: Arc<RwLock<GatewayContext>>,
    config: iris_secure_gateway::Config,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(config.key_rotation.check_interval_seconds));
    
    loop {
        interval.tick().await;
        
        tracing::info!("Checking for iris.wasm updates from iris-wasm-gateway...");
        
        match fetch_iris_wasm(&config.key_rotation.wasm_gateway_url).await {
            Ok(wasm_data) => {
                let assets_dir = config.server.assets_dir.clone();
                let wasm_path = assets_dir.join("iris.wasm");
                
                match std::fs::write(&wasm_path, &wasm_data) {
                    Ok(_) => {
                        tracing::info!("Updated iris.wasm from gateway");
                    }
                    Err(e) => {
                        tracing::error!("Failed to write iris.wasm: {}", e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to fetch iris.wasm from gateway: {}", e);
            }
        }
    }
}

async fn fetch_iris_wasm(gateway_url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let url = format!("{}/iris.wasm", gateway_url);
    
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    
    let response = client.get(&url).send().await?;
    
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()).into());
    }
    
    let bytes = response.bytes().await?;
    Ok(bytes.to_vec())
}
