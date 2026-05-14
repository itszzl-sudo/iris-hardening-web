//! Iris Gateway - 零配置安全网关
//!
//! 部署架构: nginx <-> iris-gateway <-> 内部服务器
//!
//! 零配置模式:
//! - 无需配置文件即可启动，默认行为：透明反向代理
//! - 客户端 JS 通过 X-Iris-Configured 响应头判断是否加密
//! - 通过 API 动态配置加密、映射、路由

use iris_gateway::IrisGateway;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = match std::env::args().nth(1) {
        Some(path) => {
            // Explicit config path: try to load it, but fall back to defaults if missing
            match iris_gateway::Config::from_file(&path) {
                Ok(c) => {
                    tracing::info!("Loaded config from: {}", path);
                    c
                }
                Err(e) => {
                    tracing::warn!("Config file '{}' not found ({}), using zero-config defaults", path, e);
                    iris_gateway::Config::default()
                }
            }
        }
        None => {
            // No arg: try config.toml in current dir, but don't fail if missing
            match iris_gateway::Config::from_file("config.toml") {
                Ok(c) => {
                    tracing::info!("Loaded config from: config.toml");
                    c
                }
                Err(_) => {
                    tracing::info!("No config file found, starting in zero-config mode");
                    iris_gateway::Config::default()
                }
            }
        }
    };

    let configured = config.is_encryption_active();
    tracing::info!("Starting Iris Gateway v{}", iris_gateway::VERSION);
    tracing::info!("Server: {}:{}", config.server.host, config.server.port);
    tracing::info!("Encryption: {}", if configured { "ACTIVE" } else { "INACTIVE (zero-config)" });
    tracing::info!("Base dir: {:?}", config.server.base_dir);
    tracing::info!("File mappings: {}", config.file_mappings.len());
    tracing::info!("API routes: {}", config.api_routes.len());

    let gateway = IrisGateway::new(config)?;
    gateway.run().await?;

    Ok(())
}
