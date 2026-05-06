//! HTTP 服务器

use warp::{Filter, Reply};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::path::PathBuf;
use crate::{Config, KeyManager, WasmGenerator, Notifier, KeyPair, Result};

pub struct GatewayServer {
    config: Arc<Config>,
    key_manager: Arc<KeyManager>,
    wasm_generator: Arc<WasmGenerator>,
    notifier: Arc<Notifier>,
    current_wasm: Arc<RwLock<Vec<u8>>>,
}

impl GatewayServer {
    pub fn new(config: Config) -> Result<Self> {
        let key_manager = KeyManager::new(
            config.key.key_dir.clone(),
            config.validity_duration(),
        );
        
        let notifier = Notifier::new(
            config.encrypt_service.url.clone(),
            config.encrypt_service.update_key_endpoint.clone(),
        )?;
        
        Ok(Self {
            config: Arc::new(config),
            key_manager: Arc::new(key_manager),
            wasm_generator: Arc::new(WasmGenerator::new()),
            notifier: Arc::new(notifier),
            current_wasm: Arc::new(RwLock::new(Vec::new())),
        })
    }
    
    pub async fn run(&self) -> Result<()> {
        self.initialize().await?;
        self.start_rotation_task();
        
        let wasm_route = warp::path("iris.wasm")
            .and(warp::get())
            .then({
                let wasm = self.current_wasm.clone();
                move || {
                    let wasm = wasm.clone();
                    async move {
                        let wasm = wasm.read().await;
                        warp::http::Response::builder()
                            .status(200)
                            .header("Content-Type", "application/wasm")
                            .header("Cache-Control", "no-cache")
                            .body(wasm.clone())
                            .unwrap()
                    }
                }
            });
        
        let status_route = warp::path("status")
            .and(warp::get())
            .then({
                let km = self.key_manager.clone();
                move || {
                    let km = km.clone();
                    async move {
                        match km.load_current() {
                            Ok(key) => {
                                let status = serde_json::json!({
                                    "status": "ok",
                                    "key_id": key.id.to_string(),
                                    "expires_at": key.expires_at.to_rfc3339(),
                                });
                                warp::reply::json(&status)
                            }
                            Err(e) => {
                                let status = serde_json::json!({
                                    "status": "error",
                                    "message": e.to_string(),
                                });
                                warp::reply::json(&status)
                            }
                        }
                    }
                }
            });
        
        let routes = wasm_route.or(status_route);
        
        let addr: std::net::SocketAddr = format!("{}:{}", self.config.server.host, self.config.server.port)
            .parse()
            .map_err(|e| crate::Error::Http(format!("Invalid address: {}", e)))?;
        
        tracing::info!("Gateway listening on {}", addr);
        
        warp::serve(routes).run(addr).await;
        
        Ok(())
    }
    
    async fn initialize(&self) -> Result<()> {
        let key_pair = match self.key_manager.load_current() {
            Ok(k) if !self.key_manager.is_expired(&k) => {
                tracing::info!("Using existing key: {}", k.id);
                k
            }
            _ => {
                tracing::info!("Generating new key pair");
                let k = self.key_manager.generate_key_pair()?;
                self.key_manager.save_key_pair(&k)?;
                k
            }
        };
        
        let wasm = self.wasm_generator.generate(
            &key_pair,
            &self.config.encrypt_service.url,
        )?;
        
        *self.current_wasm.write().await = wasm;
        
        Ok(())
    }
    
    fn start_rotation_task(&self) {
        let config = self.config.clone();
        let key_manager = self.key_manager.clone();
        let wasm_generator = self.wasm_generator.clone();
        let notifier = self.notifier.clone();
        let current_wasm = self.current_wasm.clone();
        
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
                
                match key_manager.load_current() {
                    Ok(key) => {
                        if key_manager.is_expiring(&key, config.rotation_margin()) {
                            tracing::info!("Key expiring, rotating...");
                            
                            match key_manager.generate_key_pair() {
                                Ok(new_key) => {
                                    if let Err(e) = key_manager.save_key_pair(&new_key) {
                                        tracing::error!("Failed to save new key: {}", e);
                                        continue;
                                    }
                                    
                                    if let Err(e) = notifier.notify_key_update(&new_key).await {
                                        tracing::error!("Failed to notify: {}", e);
                                    }
                                    
                                    match wasm_generator.generate(&new_key, &config.encrypt_service.url) {
                                        Ok(wasm) => {
                                            *current_wasm.write().await = wasm;
                                            tracing::info!("WASM regenerated with new key");
                                        }
                                        Err(e) => {
                                            tracing::error!("Failed to generate WASM: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Failed to generate new key: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to load key: {}", e);
                    }
                }
            }
        });
    }
}
