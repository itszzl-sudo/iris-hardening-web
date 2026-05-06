//! Iris WASM Gateway 服务器

use iris_wasm_gateway::{Config, KeyManager};

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
    
    let key_manager = KeyManager::new(
        config.key.key_dir.clone(),
        config.validity_duration()
    );
    
    let key_pair = key_manager.generate_key_pair()?;
    tracing::info!("Generated key pair: id={}", key_pair.id);
    
    Ok(())
}
