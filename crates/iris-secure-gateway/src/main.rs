//! Iris Secure Gateway 服务器

use iris_secure_gateway::SecureGateway;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.toml".to_string());

    tracing::info!("Loading config from: {}", config_path);
    let config = iris_secure_gateway::Config::from_file(&config_path)?;

    tracing::info!("Starting Iris Secure Gateway v{}", iris_secure_gateway::VERSION);
    tracing::info!("Server: {}:{}", config.server.host, config.server.port);
    tracing::info!("Base dir: {:?}", config.server.base_dir);
    tracing::info!("Assets dir: {:?}", config.server.assets_dir);

    if let Some(ref url) = config.config_server_url {
        tracing::info!("Config server: {}", url);
        tracing::info!("Poll interval: {}s", config.poll_interval_secs);
    }

    let gateway = SecureGateway::new(config)?;
    gateway.run().await?;

    Ok(())
}
