//! HTTP 服务器

use warp::{Filter, Reply, Rejection, http::{StatusCode, Method}};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use crate::{Config, FileEncryptor, ApiProxy, PathTransformer, Result};

#[derive(Debug, Deserialize)]
struct UpdateKeyRequest {
    key_id: String,
    key: String,
    expires_at: String,
}

#[derive(Debug, Serialize)]
struct UpdateKeyResponse {
    success: bool,
    message: String,
}

#[derive(Debug, Deserialize)]
struct DecryptRequest {
    data: String,
}

#[derive(Debug, Serialize)]
struct DecryptResponse {
    data: String,
}

pub struct SecureGateway {
    config: Arc<Config>,
    encryptor: Arc<RwLock<FileEncryptor>>,
    proxy: Arc<ApiProxy>,
    path_transformer: Arc<PathTransformer>,
}

impl SecureGateway {
    pub fn new(config: Config) -> Result<Self> {
        let key = config.load_key()?;
        let encryptor = FileEncryptor::new(&key)?;
        let path_transformer = PathTransformer::new(config.file_mappings.clone());
        
        Ok(Self {
            config: Arc::new(config),
            encryptor: Arc::new(RwLock::new(encryptor)),
            proxy: Arc::new(ApiProxy::new()?),
            path_transformer: Arc::new(path_transformer),
        })
    }
    
    pub async fn run(&self) -> Result<()> {
        let config = self.config.clone();
        let encryptor = self.encryptor.clone();
        let proxy = self.proxy.clone();
        let path_transformer = self.path_transformer.clone();
        
        let update_key_route = warp::path("internal")
            .and(warp::path("update-key"))
            .and(warp::post())
            .and(warp::body::json())
            .then({
                let encryptor = encryptor.clone();
                move |req: UpdateKeyRequest| {
                    let encryptor = encryptor.clone();
                    async move {
                        handle_update_key(req, encryptor).await
                    }
                }
            });
        
        let decrypt_route = warp::path("internal")
            .and(warp::path("decrypt"))
            .and(warp::post())
            .and(warp::body::json())
            .then({
                let encryptor = encryptor.clone();
                move |req: DecryptRequest| {
                    let encryptor = encryptor.clone();
                    async move {
                        handle_decrypt(req, encryptor).await
                    }
                }
            });
        
        let encrypt_path_route = warp::path("internal")
            .and(warp::path("encrypt-path"))
            .and(warp::post())
            .and(warp::body::json())
            .then({
                let path_transformer = path_transformer.clone();
                move |req: serde_json::Value| {
                    let path_transformer = path_transformer.clone();
                    async move {
                        handle_encrypt_path(req, path_transformer).await
                    }
                }
            });
        
        let decrypt_path_route = warp::path("internal")
            .and(warp::path("decrypt-path"))
            .and(warp::post())
            .and(warp::body::json())
            .then({
                let path_transformer = path_transformer.clone();
                move |req: serde_json::Value| {
                    let path_transformer = path_transformer.clone();
                    async move {
                        handle_decrypt_path(req, path_transformer).await
                    }
                }
            });
        
        let file_route = warp::path::full()
            .and(warp::method())
            .and(warp::header::headers_cloned())
            .and(warp::body::bytes())
            .then(move |path, method, headers, body| {
                let config = config.clone();
                let encryptor = encryptor.clone();
                let proxy = proxy.clone();
                let path_transformer = path_transformer.clone();
                async move {
                    handle_request(path, method, headers, body, config, encryptor, proxy, path_transformer).await
                }
            });
        
        let routes = update_key_route
            .or(decrypt_route)
            .or(encrypt_path_route)
            .or(decrypt_path_route)
            .or(file_route)
            .recover(handle_rejection);
        
        let addr: std::net::SocketAddr = format!("{}:{}", self.config.server.host, self.config.server.port)
            .parse()
            .map_err(|e| crate::Error::Http(format!("Invalid address: {}", e)))?;
        
        tracing::info!("Secure Gateway listening on {}", addr);
        
        warp::serve(routes)
            .run(addr)
            .await;
        
        Ok(())
    }
}

async fn handle_update_key(
    req: UpdateKeyRequest,
    encryptor: Arc<RwLock<FileEncryptor>>,
) -> impl Reply {
    tracing::info!("Received key update request: key_id={}", req.key_id);
    
    match hex::decode(&req.key) {
        Ok(key_bytes) => {
            match FileEncryptor::new(&key_bytes) {
                Ok(new_encryptor) => {
                    *encryptor.write().await = new_encryptor;
                    tracing::info!("Key updated successfully");
                    
                    let response = UpdateKeyResponse {
                        success: true,
                        message: format!("Key {} updated", req.key_id),
                    };
                    warp::reply::json(&response)
                }
                Err(e) => {
                    tracing::error!("Failed to create encryptor: {}", e);
                    let response = UpdateKeyResponse {
                        success: false,
                        message: format!("Failed to create encryptor: {}", e),
                    };
                    warp::reply::with_status(
                        warp::reply::json(&response),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Invalid key format: {}", e);
            let response = UpdateKeyResponse {
                success: false,
                message: format!("Invalid key format: {}", e),
            };
            warp::reply::with_status(
                warp::reply::json(&response),
                StatusCode::BAD_REQUEST,
            )
        }
    }
}

async fn handle_decrypt(
    req: DecryptRequest,
    encryptor: Arc<RwLock<FileEncryptor>>,
) -> impl Reply {
    match base64::engine::general_purpose::STANDARD.decode(&req.data) {
        Ok(encrypted_data) => {
            let enc = encryptor.read().await;
            match enc.decrypt_data(&encrypted_data) {
                Ok(decrypted) => {
                    let response = DecryptResponse {
                        data: base64::engine::general_purpose::STANDARD.encode(&decrypted),
                    };
                    warp::reply::json(&response)
                }
                Err(e) => {
                    warp::reply::with_status(
                        format!("Decryption error: {}", e),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )
                }
            }
        }
        Err(e) => {
            warp::reply::with_status(
                format!("Invalid base64: {}", e),
                StatusCode::BAD_REQUEST,
            )
        }
    }
}

async fn handle_encrypt_path(
    req: serde_json::Value,
    path_transformer: Arc<PathTransformer>,
) -> impl Reply {
    if let Some(path) = req.get("path").and_then(|v| v.as_str()) {
        let encrypted = path_transformer.encrypt_path(path);
        warp::reply::json(&serde_json::json!({ "encrypted": encrypted }))
    } else {
        warp::reply::with_status(
            "Missing path field",
            StatusCode::BAD_REQUEST,
        )
    }
}

async fn handle_decrypt_path(
    req: serde_json::Value,
    path_transformer: Arc<PathTransformer>,
) -> impl Reply {
    if let Some(encrypted) = req.get("encrypted").and_then(|v| v.as_str()) {
        if let Some(real) = path_transformer.decrypt_path(encrypted) {
            warp::reply::json(&serde_json::json!({ "path": real }))
        } else {
            warp::reply::with_status(
                "Path not found",
                StatusCode::NOT_FOUND,
            )
        }
    } else {
        warp::reply::with_status(
            "Missing encrypted field",
            StatusCode::BAD_REQUEST,
        )
    }
}

async fn handle_request(
    path: warp::path::FullPath,
    method: Method,
    headers: warp::http::HeaderMap,
    body: bytes::Bytes,
    config: Arc<Config>,
    encryptor: Arc<RwLock<FileEncryptor>>,
    proxy: Arc<ApiProxy>,
    path_transformer: Arc<PathTransformer>,
) -> impl Reply {
    let path_str = path.as_str();
    let method_str = method.as_str();
    
    tracing::debug!("Request: {} {}", method_str, path_str);
    
    if let Some(route) = config.match_api_route(path_str, method_str) {
        tracing::info!("API route matched: {} -> {}", path_str, route.target);
        
        let headers_vec: Vec<(String, String)> = headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        
        let body_to_forward = if !body.is_empty() {
            let enc = encryptor.read().await;
            match enc.encrypt_data(&body) {
                Ok(encrypted) => Some(encrypted),
                Err(e) => {
                    tracing::error!("Body encryption error: {}", e);
                    return warp::reply::with_status(
                        format!("Encryption error: {}", e),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    );
                }
            }
        } else {
            None
        };
        
        match proxy.forward(route, path_str, method_str, body_to_forward.as_deref(), headers_vec).await {
            Ok(resp) => {
                if !resp.body.is_empty() {
                    let enc = encryptor.read().await;
                    match enc.decrypt_data(&resp.body) {
                        Ok(decrypted) => {
                            return warp::reply::with_status(
                                decrypted,
                                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::OK),
                            );
                        }
                        Err(e) => {
                            tracing::error!("Response decryption error: {}", e);
                            return warp::reply::with_status(
                                resp.body,
                                StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                            );
                        }
                    }
                }
                warp::reply::with_status(
                    resp.body,
                    StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                )
            }
            Err(e) => {
                tracing::error!("Proxy error: {}", e);
                warp::reply::with_status(
                    format!("Proxy error: {}", e),
                    StatusCode::BAD_GATEWAY,
                )
            }
        }
    }
    
    let clean_path = path_str.trim_start_matches('/');
    
    if let Some(real_path) = path_transformer.decrypt_path(clean_path) {
        let full_path = config.server.base_dir.join(&real_path);
        
        tracing::info!("File request: {} -> {:?}", clean_path, full_path);
        
        match std::fs::read(&full_path) {
            Ok(content) => {
                let enc = encryptor.read().await;
                match enc.encrypt_data(&content) {
                    Ok(encrypted) => {
                        return warp::reply::with_status(
                            encrypted,
                            StatusCode::OK,
                        );
                    }
                    Err(e) => {
                        tracing::error!("Encryption error: {}", e);
                        return warp::reply::with_status(
                            format!("Encryption error: {}", e),
                            StatusCode::INTERNAL_SERVER_ERROR,
                        );
                    }
                }
            }
            Err(e) => {
                tracing::error!("File read error: {}", e);
                return warp::reply::with_status(
                    format!("File not found: {}", clean_path),
                    StatusCode::NOT_FOUND,
                );
            }
        }
    }
    
    warp::reply::with_status(
        format!("Path not found: {}", path_str),
        StatusCode::NOT_FOUND,
    )
}

async fn handle_rejection(err: Rejection) -> Result<impl Reply, Rejection> {
    if err.is_not_found() {
        Ok(warp::reply::with_status(
            "Not found",
            StatusCode::NOT_FOUND,
        ))
    } else {
        Err(err)
    }
}
