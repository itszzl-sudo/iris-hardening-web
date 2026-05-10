//! HTTP 服务器

use warp::{Filter, Reply, Rejection, http::{StatusCode, Method, Response}};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use base64::Engine;
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

#[derive(Debug)]
struct Unauthorized;

impl warp::reject::Reject for Unauthorized {}

fn json_response<T: Serialize>(data: &T, status: StatusCode) -> Response<Vec<u8>> {
    let body = serde_json::to_vec(data).unwrap_or_default();
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(body)
        .unwrap()
}

fn text_response(text: &str, status: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(text.as_bytes().to_vec())
        .unwrap()
}

/// Check internal token from header
fn check_internal_token(header_token: Option<String>, expected: &str) -> std::result::Result<(), Rejection> {
    match header_token {
        Some(t) if crate::crypto::constant_time_eq(t.as_bytes(), expected.as_bytes()) => Ok(()),
        _ => {
            tracing::warn!("Internal API authentication failed");
            Err(warp::reject::custom(Unauthorized))
        }
    }
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
        let internal_token = Arc::new(self.config.internal_token.clone());

        let update_key_route = warp::path("internal")
            .and(warp::path("update-key"))
            .and(warp::post())
            .and(warp::header::optional("X-Internal-Token"))
            .and(warp::body::json())
            .and_then({
                let encryptor = encryptor.clone();
                let internal_token = internal_token.clone();
                move |header_token: Option<String>, req: UpdateKeyRequest| {
                    let encryptor = encryptor.clone();
                    let internal_token = internal_token.clone();
                    async move {
                        check_internal_token(header_token, &internal_token)?;
                        Ok::<_, Rejection>(handle_update_key(req, encryptor).await)
                    }
                }
            });

        let decrypt_route = warp::path("internal")
            .and(warp::path("decrypt"))
            .and(warp::post())
            .and(warp::header::optional("X-Internal-Token"))
            .and(warp::body::json())
            .and_then({
                let encryptor = encryptor.clone();
                let internal_token = internal_token.clone();
                move |header_token: Option<String>, req: DecryptRequest| {
                    let encryptor = encryptor.clone();
                    let internal_token = internal_token.clone();
                    async move {
                        check_internal_token(header_token, &internal_token)?;
                        Ok::<_, Rejection>(handle_decrypt(req, encryptor).await)
                    }
                }
            });

        let encrypt_path_route = warp::path("internal")
            .and(warp::path("encrypt-path"))
            .and(warp::post())
            .and(warp::header::optional("X-Internal-Token"))
            .and(warp::body::json())
            .and_then({
                let path_transformer = path_transformer.clone();
                let internal_token = internal_token.clone();
                move |header_token: Option<String>, req: serde_json::Value| {
                    let path_transformer = path_transformer.clone();
                    let internal_token = internal_token.clone();
                    async move {
                        check_internal_token(header_token, &internal_token)?;
                        Ok::<_, Rejection>(handle_encrypt_path(req, path_transformer).await)
                    }
                }
            });

        let decrypt_path_route = warp::path("internal")
            .and(warp::path("decrypt-path"))
            .and(warp::post())
            .and(warp::header::optional("X-Internal-Token"))
            .and(warp::body::json())
            .and_then({
                let path_transformer = path_transformer.clone();
                move |header_token: Option<String>, req: serde_json::Value| {
                    let path_transformer = path_transformer.clone();
                    let internal_token = internal_token.clone();
                    async move {
                        check_internal_token(header_token, &internal_token)?;
                        Ok::<_, Rejection>(handle_decrypt_path(req, path_transformer).await)
                    }
                }
            });

        let file_route = warp::path::full()
            .and(warp::method())
            .and(warp::header::headers_cloned())
            .and(warp::body::bytes())
            .and_then(move |path, method, headers, body| {
                let config = config.clone();
                let encryptor = encryptor.clone();
                let proxy = proxy.clone();
                let path_transformer = path_transformer.clone();
                async move {
                    Ok::<_, Rejection>(handle_request(path, method, headers, body, config, encryptor, proxy, path_transformer).await)
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

        let (_, server) = warp::serve(routes)
            .bind_with_graceful_shutdown(addr, async {
                tokio::signal::ctrl_c()
                    .await
                    .expect("Failed to install Ctrl+C handler");
                tracing::info!("Received shutdown signal, shutting down gracefully...");
            });

        server.await;

        Ok(())
    }
}

async fn handle_update_key(
    req: UpdateKeyRequest,
    encryptor: Arc<RwLock<FileEncryptor>>,
) -> Response<Vec<u8>> {
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
                    json_response(&response, StatusCode::OK)
                }
                Err(e) => {
                    tracing::error!("Failed to create encryptor: {}", e);
                    let response = UpdateKeyResponse {
                        success: false,
                        message: format!("Failed to create encryptor: {}", e),
                    };
                    json_response(&response, StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(e) => {
            tracing::error!("Invalid key format: {}", e);
            let response = UpdateKeyResponse {
                success: false,
                message: format!("Invalid key format: {}", e),
            };
            json_response(&response, StatusCode::BAD_REQUEST)
        }
    }
}

async fn handle_decrypt(
    req: DecryptRequest,
    encryptor: Arc<RwLock<FileEncryptor>>,
) -> Response<Vec<u8>> {
    match base64::engine::general_purpose::STANDARD.decode(&req.data) {
        Ok(encrypted_data) => {
            let enc = encryptor.read().await;
            match enc.decrypt_data(&encrypted_data) {
                Ok(decrypted) => {
                    let response = DecryptResponse {
                        data: base64::engine::general_purpose::STANDARD.encode(&decrypted),
                    };
                    json_response(&response, StatusCode::OK)
                }
                Err(e) => {
                    text_response(&format!("Decryption error: {}", e), StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(e) => {
            text_response(&format!("Invalid base64: {}", e), StatusCode::BAD_REQUEST)
        }
    }
}

async fn handle_encrypt_path(
    req: serde_json::Value,
    path_transformer: Arc<PathTransformer>,
) -> Response<Vec<u8>> {
    if let Some(path) = req.get("path").and_then(|v| v.as_str()) {
        let encrypted = path_transformer.encrypt_path(path);
        json_response(&serde_json::json!({ "encrypted": encrypted }), StatusCode::OK)
    } else {
        text_response("Missing path field", StatusCode::BAD_REQUEST)
    }
}

async fn handle_decrypt_path(
    req: serde_json::Value,
    path_transformer: Arc<PathTransformer>,
) -> Response<Vec<u8>> {
    if let Some(encrypted) = req.get("encrypted").and_then(|v| v.as_str()) {
        if let Some(real) = path_transformer.decrypt_path(encrypted) {
            json_response(&serde_json::json!({ "path": real }), StatusCode::OK)
        } else {
            text_response("Path not found", StatusCode::NOT_FOUND)
        }
    } else {
        text_response("Missing encrypted field", StatusCode::BAD_REQUEST)
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
) -> Response<Vec<u8>> {
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
                    return text_response(&format!("Encryption error: {}", e), StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        } else {
            None
        };

        match proxy.forward(&route.target, path_str, method_str, body_to_forward.as_deref(), headers_vec).await {
            Ok(resp) => {
                if !resp.body.is_empty() {
                    let enc = encryptor.read().await;
                    match enc.decrypt_data(&resp.body) {
                        Ok(decrypted) => {
                            return Response::builder()
                                .status(StatusCode::from_u16(resp.status).unwrap_or(StatusCode::OK))
                                .body(decrypted)
                                .unwrap();
                        }
                        Err(e) => {
                            tracing::error!("Response decryption error: {}", e);
                            return Response::builder()
                                .status(StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
                                .body(resp.body)
                                .unwrap();
                        }
                    }
                }
                Response::builder()
                    .status(StatusCode::from_u16(resp.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
                    .body(resp.body)
                    .unwrap()
            }
            Err(e) => {
                tracing::error!("Proxy error: {}", e);
                text_response(&format!("Proxy error: {}", e), StatusCode::BAD_GATEWAY)
            }
        }
    } else {
        let clean_path = path_str.trim_start_matches('/');

        if let Some(real_path) = path_transformer.decrypt_path(clean_path) {
            let full_path = config.server.base_dir.join(&real_path);

            tracing::info!("File request: {} -> {:?}", clean_path, full_path);

            match std::fs::read(&full_path) {
                Ok(content) => {
                    let enc = encryptor.read().await;
                    match enc.encrypt_data(&content) {
                        Ok(encrypted) => {
                            Response::builder()
                                .status(StatusCode::OK)
                                .body(encrypted)
                                .unwrap()
                        }
                        Err(e) => {
                            tracing::error!("Encryption error: {}", e);
                            text_response(&format!("Encryption error: {}", e), StatusCode::INTERNAL_SERVER_ERROR)
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("File read error: {}", e);
                    text_response(&format!("File not found: {}", clean_path), StatusCode::NOT_FOUND)
                }
            }
        } else {
            text_response(&format!("Path not found: {}", path_str), StatusCode::NOT_FOUND)
        }
    }
}

async fn handle_rejection(err: Rejection) -> std::result::Result<impl Reply, Rejection> {
    if err.is_not_found() {
        Ok(warp::reply::with_status("Not found", StatusCode::NOT_FOUND))
    } else if err.find::<Unauthorized>().is_some() {
        Ok(warp::reply::with_status("Unauthorized", StatusCode::UNAUTHORIZED))
    } else {
        Err(err)
    }
}
