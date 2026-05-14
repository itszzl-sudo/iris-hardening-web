use std::sync::Arc;
use tracing_subscriber::FmtSubscriber;

mod config;
mod database;
mod nginx_gen;
mod routes;

use crate::config::Config;
use crate::database::Database;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_env_filter("info")
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");

    // Load configuration
    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "config.toml".to_string());

    let config = Config::load(&config_path)
        .unwrap_or_else(|_| {
            tracing::info!("Using default configuration");
            Config::default()
        });

    tracing::info!("Starting Iris Secure Config service");
    tracing::info!("Database: {}", config.database.path);
    tracing::info!("Server: {}:{}", config.server.host, config.server.port);

    // Initialize database
    let db = Arc::new(Database::new(&config.database.path)?);

    // Build routes
    let gateway_url = config.gateway.url.clone();
    let routes = routes::routes(db, gateway_url);

    tracing::info!("Server listening on {}:{}", config.server.host, config.server.port);

    warp::serve(routes).run(([0, 0, 0, 0], config.server.port)).await;

    Ok(())
}