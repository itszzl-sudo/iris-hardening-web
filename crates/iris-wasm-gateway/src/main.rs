//! Iris WASM Gateway 服务器

use iris_wasm_gateway::GatewayServer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".to_string());

    tracing::info!("Loading config from: {}", config_path);
    let config = iris_wasm_gateway::Config::from_file(&config_path)?;

    tracing::info!("Starting Iris WASM Gateway v{}", iris_wasm_gateway::VERSION);
    tracing::info!("Gateway: {}:{}", config.server.host, config.server.port);
    tracing::info!("Key validity: {} hours", config.key.validity_hours);

    let server = GatewayServer::new(config)?;
    server.run().await?;

    Ok(())
}
