//! 命令行工具

use clap::{Parser, Subcommand};
use iris_wasm_gateway::{
    Config, KeyManager, WasmGenerator, 
    CloudflareUploader,
    scheduler::ManualRotation,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "iris-wasm-gateway")]
#[command(about = "Iris WASM Gateway - Generate and deploy WASM with obfuscated keys")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Start the gateway server with scheduler")]
    Serve {
        #[arg(short, long, default_value = "config.toml")]
        config: String,
    },

    #[command(about = "Generate WASM once without starting server")]
    Generate {
        #[arg(short, long, default_value = "config.toml")]
        config: String,
        #[arg(short, long, default_value = "iris.wasm")]
        output: String,
    },

    #[command(about = "Upload WASM to Cloudflare Pages")]
    Upload {
        #[arg(short, long)]
        wasm: String,
        #[arg(short, long, default_value = "config.toml")]
        config: String,
    },

    #[command(about = "Generate and upload in one command")]
    Deploy {
        #[arg(short, long, default_value = "config.toml")]
        config: String,
    },

    #[command(about = "Show current scheduler status")]
    Status {
        #[arg(short, long, default_value = "config.toml")]
        config: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { config } => {
            serve(config).await?;
        }
        Commands::Generate { config, output } => {
            generate(config, output).await?;
        }
        Commands::Upload { wasm, config } => {
            upload(wasm, config).await?;
        }
        Commands::Deploy { config } => {
            deploy(config).await?;
        }
        Commands::Status { config } => {
            status(config).await?;
        }
    }

    Ok(())
}

async fn serve(config_path: String) -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::from_file(&config_path)?;
    tracing::info!("Starting Iris WASM Gateway v{}", iris_wasm_gateway::VERSION);

    let key_manager = KeyManager::new(
        config.key.key_dir.clone(),
        config.validity_duration()
    );

    let scheduler_config = iris_wasm_gateway::scheduler::SchedulerConfig {
        check_interval_minutes: config.scheduler.check_interval_minutes,
        generation_lead_time_hours: config.scheduler.generation_lead_time_hours,
        wasm_output_dir: config.scheduler.wasm_output_dir.clone(),
    };

    let cloudflare_config = config.load_cloudflare_config()?.map(|cf| {
        iris_wasm_gateway::cloudflare::CloudflareConfig {
            api_token: cf.api_token,
            account_id: cf.account_id,
            project_name: cf.project_name,
            deployment_branch: cf.deployment_branch,
        }
    });

    let scheduler = std::sync::Arc::new(
        iris_wasm_gateway::WasmScheduler::new(
            key_manager,
            cloudflare_config,
            scheduler_config,
            config.encrypt_service.url.clone(),
        )
    );

    tokio::spawn(async move {
        if let Err(e) = scheduler.start().await {
            tracing::error!("Scheduler error: {}", e);
        }
    });

    tracing::info!("Gateway running - press Ctrl+C to stop");
    tokio::signal::ctrl_c().await?;

    Ok(())
}

async fn generate(config_path: String, output: String) -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::from_file(&config_path)?;
    let key_manager = KeyManager::new(
        config.key.key_dir.clone(),
        config.validity_duration()
    );
    let generator = WasmGenerator::new();
    let output_path = PathBuf::from(output);

    let wasm_path = ManualRotation::generate_wasm_once(
        &key_manager,
        &generator,
        &config.encrypt_service.url,
        &output_path,
    )?;

    tracing::info!("WASM generated: {:?}", wasm_path);
    Ok(())
}

async fn upload(wasm_path: String, config_path: String) -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::from_file(&config_path)?;
    let cf_config = config.load_cloudflare_config()?.ok_or_else(|| {
        anyhow::anyhow!("Cloudflare config not found")
    })?;

    let uploader = CloudflareUploader::new(iris_wasm_gateway::cloudflare::CloudflareConfig {
        api_token: cf_config.api_token,
        account_id: cf_config.account_id,
        project_name: cf_config.project_name,
        deployment_branch: cf_config.deployment_branch,
    });

    let wasm_path = PathBuf::from(wasm_path);
    let result = ManualRotation::upload_wasm_once(&uploader, &wasm_path).await?;

    println!("Uploaded to: {}", result.url);
    println!("Deployment ID: {}", result.id);
    println!("Status: {}", result.status);

    Ok(())
}

async fn deploy(config_path: String) -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::from_file(&config_path)?;
    let cf_config = config.load_cloudflare_config()?.ok_or_else(|| {
        anyhow::anyhow!("Cloudflare config not found")
    })?;

    let key_manager = KeyManager::new(
        config.key.key_dir.clone(),
        config.validity_duration()
    );
    let generator = WasmGenerator::new();

    let temp_path = PathBuf::from("temp-iris.wasm");
    let wasm_path = ManualRotation::generate_wasm_once(
        &key_manager,
        &generator,
        &config.encrypt_service.url,
        &temp_path,
    )?;

    let uploader = CloudflareUploader::new(iris_wasm_gateway::cloudflare::CloudflareConfig {
        api_token: cf_config.api_token,
        account_id: cf_config.account_id,
        project_name: cf_config.project_name,
        deployment_branch: cf_config.deployment_branch,
    });

    let result = ManualRotation::upload_wasm_once(&uploader, &wasm_path).await?;

    std::fs::remove_file(&wasm_path)?;

    println!("Deployed to: {}", result.url);
    println!("Deployment ID: {}", result.id);

    Ok(())
}

async fn status(config_path: String) -> anyhow::Result<()> {
    let config = Config::from_file(&config_path)?;

    println!("Configuration:");
    println!("  Server: {}:{}", config.server.host, config.server.port);
    println!("  Key validity: {} hours", config.key.validity_hours);
    println!("  Check interval: {} minutes", config.scheduler.check_interval_minutes);

    if let Some(ref cf_path) = config.cloudflare_config_path {
        println!("  Cloudflare config: {:?}", cf_path);
        
        if let Ok(Some(cf)) = config.load_cloudflare_config() {
            println!("    Project: {}", cf.project_name);
        }
    }

    let key_manager = KeyManager::new(
        config.key.key_dir.clone(),
        config.validity_duration()
    );

    match key_manager.load_current() {
        Ok(key) => {
            println!("\nCurrent key:");
            println!("  ID: {}", key.id);
            println!("  Created: {}", key.created_at);
            println!("  Expires: {}", key.expires_at);
            println!("  Algorithm: {}", key.algorithm);
        }
        Err(_) => {
            println!("\nNo active key found");
        }
    }

    Ok(())
}
