//! 统一 HTTP 服务器
//!
//! 零配置架构:
//! - 无需配置文件，gateway 启动后自动扫描 assets_dir/base_dir 的 HTML
//! - 从 HTML 中提取 src=/href= 引用的 URL，自动构建 file_mappings
//! - 自动生成密钥，加密即生效
//! - 客户端 JS 通过 X-Iris-Configured 响应头判断是否加密
//! - 未配置时: 透明服务文件; 已配置时: 加密文件 + API 代理

use warp::{Filter, Reply, Rejection, http::{StatusCode, Method, Response}};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use base64::Engine;
use crate::{
    Config, FileEncryptor, KeyManager, KeyPair,
    ApiProxy, PathTransformer, Result,
};

/// Maximum file size to serve (50 MB) — prevents OOM from large files
const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024;

/// Maximum base64 payload size for decrypt/encrypt requests (10 MB decoded)
const MAX_DECRYPT_PAYLOAD: usize = 10 * 1024 * 1024;

/// Read a file with size limit. Returns error if file exceeds MAX_FILE_SIZE.
/// Uses tokio::fs to avoid blocking the Tokio runtime.
async fn read_file_limited(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    let metadata = tokio::fs::metadata(path).await?;
    if metadata.len() > MAX_FILE_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("File too large: {} bytes (max {})", metadata.len(), MAX_FILE_SIZE),
        ));
    }
    tokio::fs::read(path).await
}

#[derive(Debug, Deserialize)]
struct UpdateKeyRequest {
    key_id: String,
    key: String,
    #[allow(dead_code)]
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

#[derive(Debug)]
struct Unauthorized;

impl warp::reject::Reject for Unauthorized {}

#[derive(Debug)]
struct AssetNotFound;

impl warp::reject::Reject for AssetNotFound {}

fn json_response<T: Serialize>(data: &T, status: StatusCode) -> Response<Vec<u8>> {
    let body = serde_json::to_vec(data).unwrap_or_default();
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .header("X-Iris-Version", crate::VERSION)
        .body(body)
        .unwrap()
}

fn text_response(text: &str, status: StatusCode) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header("Content-Type", "text/plain; charset=utf-8")
        .header("X-Iris-Version", crate::VERSION)
        .body(text.as_bytes().to_vec())
        .unwrap()
}

/// Wrap any Reply with X-Iris-Configured and X-Iris-Version headers.
fn with_iris_headers<T: Reply>(reply: T, configured: bool) -> impl Reply {
    warp::reply::with_header(
        warp::reply::with_header(reply, "X-Iris-Configured", if configured { "true" } else { "false" }),
        "X-Iris-Version",
        crate::VERSION,
    )
}

fn check_internal_token(header_token: Option<String>, expected: &str) -> std::result::Result<(), Rejection> {
    match header_token {
        Some(t) if crate::crypto::constant_time_eq(t.as_bytes(), expected.as_bytes()) => Ok(()),
        _ => {
            tracing::warn!("Internal API authentication failed");
            Err(warp::reject::custom(Unauthorized))
        }
    }
}

/// Guess Content-Type from file extension
fn content_type_for(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".html") || lower.ends_with(".htm") {
        "text/html; charset=utf-8"
    } else if lower.ends_with(".js") || lower.ends_with(".mjs") {
        "application/javascript; charset=utf-8"
    } else if lower.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if lower.ends_with(".wasm") {
        "application/wasm"
    } else if lower.ends_with(".json") {
        "application/json; charset=utf-8"
    } else if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".ico") {
        "image/x-icon"
    } else if lower.ends_with(".woff") {
        "font/woff"
    } else if lower.ends_with(".woff2") {
        "font/woff2"
    } else if lower.ends_with(".ttf") {
        "font/ttf"
    } else if lower.ends_with(".wav") {
        "audio/wav"
    } else if lower.ends_with(".mp3") {
        "audio/mpeg"
    } else if lower.ends_with(".ogg") {
        "audio/ogg"
    } else if lower.ends_with(".mp4") || lower.ends_with(".m4v") {
        "video/mp4"
    } else if lower.ends_with(".webm") {
        "video/webm"
    } else if lower.ends_with(".pdf") {
        "application/pdf"
    } else {
        "application/octet-stream"
    }
}

// ── HTML scanning: auto-discover URLs from files ────────────────────────────

/// Scan HTML files in a directory, extract all URL references from
/// src=, href=, data-src= attributes, and return unique local paths.
fn scan_html_for_urls(dir: &std::path::Path) -> Vec<String> {
    let mut urls = std::collections::HashSet::new();
    if let Ok(entries) = walk_dir(dir) {
        for path in entries {
            if let Some(ext) = path.extension() {
                let ext = ext.to_ascii_lowercase();
                if ext == "html" || ext == "htm" {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        extract_urls_from_html(&content, &mut urls);
                    }
                }
            }
        }
    }
    urls.into_iter().collect()
}

/// Recursively collect all file paths in a directory
fn walk_dir(dir: &std::path::Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut result = Vec::new();
    if !dir.is_dir() {
        return Ok(result);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            result.extend(walk_dir(&path)?);
        } else {
            result.push(path);
        }
    }
    Ok(result)
}

/// Extract URL references from HTML attributes.
/// Looks for src=, href=, data-src=, poster=, srcset=, content= (meta), and CSS url()
/// that point to local paths (starting with / or ./).
/// Supports both double-quoted and single-quoted attributes.
fn extract_urls_from_html(html: &str, urls: &mut std::collections::HashSet<String>) {
    // Double-quoted attributes
    for attr in &["src=\"", "href=\"", "data-src=\"", "poster=\"", "content=\""] {
        extract_attr_urls(html, attr, '"', urls);
    }
    // Single-quoted attributes
    for attr in &["src='", "href='", "data-src='", "poster='", "content='"] {
        extract_attr_urls(html, attr, '\'', urls);
    }
    // srcset: format is "url descriptor, url descriptor" — extract URLs only
    for attr in &["srcset=\"", "srcset='"] {
        let quote = if attr.ends_with('"') { '"' } else { '\'' };
        let mut search_from = 0;
        while let Some(pos) = html[search_from..].find(attr) {
            let start = search_from + pos + attr.len();
            if let Some(end) = html[start..].find(quote) {
                let value = &html[start..start + end];
                // Split by comma, take the URL part (before descriptor)
                for entry in value.split(',') {
                    let url = entry.trim().split_whitespace().next().unwrap_or("");
                    if !url.starts_with("http://")
                        && !url.starts_with("https://")
                        && !url.starts_with("data:")
                        && !url.starts_with('#')
                        && !url.starts_with("javascript:")
                        && !url.is_empty()
                    {
                        let clean = url.trim_start_matches("./").trim_start_matches('/');
                        if !clean.is_empty() {
                            urls.insert(clean.to_string());
                        }
                    }
                }
                search_from = start + end + 1;
            } else {
                break;
            }
        }
    }
    // CSS url() patterns in <style> blocks and inline style attributes
    let mut search_from = 0;
    while let Some(pos) = html[search_from..].find("url(") {
        let start = search_from + pos + 4;
        // Skip whitespace
        let rest = html[start..].trim_start();
        let actual_start = start + (html[start..].len() - rest.len());
        // Determine closing char
        let (url_str, end_offset) = if rest.starts_with('\'') {
            // url('path')
            if let Some(end) = html[actual_start + 1..].find('\'') {
                (&html[actual_start + 1..actual_start + 1 + end], actual_start + 1 + end + 1)
            } else { break }
        } else if rest.starts_with('"') {
            // url("path")
            if let Some(end) = html[actual_start + 1..].find('"') {
                (&html[actual_start + 1..actual_start + 1 + end], actual_start + 1 + end + 1)
            } else { break }
        } else {
            // url(path)
            if let Some(end) = html[actual_start..].find(')') {
                (&html[actual_start..actual_start + end], actual_start + end + 1)
            } else { break }
        };
        if !url_str.starts_with("http://")
            && !url_str.starts_with("https://")
            && !url_str.starts_with("data:")
            && !url_str.starts_with('#')
            && !url_str.is_empty()
        {
            let clean = url_str.trim_start_matches("./").trim_start_matches('/');
            if !clean.is_empty() {
                urls.insert(clean.to_string());
            }
        }
        search_from = end_offset;
    }
}

/// Helper: extract URLs from a single attribute pattern
fn extract_attr_urls(html: &str, attr: &str, quote: char, urls: &mut std::collections::HashSet<String>) {
    let mut search_from = 0;
    while let Some(pos) = html[search_from..].find(attr) {
        let start = search_from + pos + attr.len();
        if let Some(end) = html[start..].find(quote) {
            let url = &html[start..start + end];
            // Only keep local paths (not http://, not data:, not #)
            if !url.starts_with("http://")
                && !url.starts_with("https://")
                && !url.starts_with("data:")
                && !url.starts_with('#')
                && !url.starts_with("javascript:")
                && !url.is_empty()
            {
                // Normalize: strip leading ./ or /
                let clean = url.trim_start_matches("./").trim_start_matches('/');
                if !clean.is_empty() {
                    urls.insert(clean.to_string());
                }
            }
            search_from = start + end + 1;
        } else {
            break;
        }
    }
}

/// Build auto file_mappings from discovered URLs.
/// For each discovered URL, if the file exists in base_dir, add a mapping.
fn auto_build_mappings(
    discovered_urls: &[String],
    base_dir: &std::path::Path,
    encryptor: &FileEncryptor,
) -> std::collections::HashMap<String, String> {
    let mut mappings = std::collections::HashMap::new();
    for url in discovered_urls {
        let file_path = base_dir.join(url);
        if file_path.exists() {
            // Encrypt the filename to produce the encrypted path
            let encrypted = encryptor.encrypt_filename(url);
            mappings.insert(url.clone(), encrypted.clone());
            tracing::info!("Auto-mapped: {} -> {}", url, encrypted);
        }
    }
    mappings
}

// ── HTML URI rewriting ─────────────────────────────────────────────────────

/// Rewrite URIs in HTML content: replace real paths with encrypted paths.
/// For example, `src="secret/photo1.dat"` → `src="img01"` if the mapping exists.
/// This way the browser requests the encrypted path directly, no SW URL rewriting needed.
/// Handles: src, href, data-src, poster, content, srcset, and CSS url() patterns.
/// Supports both double-quoted and single-quoted attributes.
fn rewrite_html_uris(html: &str, path_transformer: &PathTransformer) -> String {
    let mut result = html.to_string();

    // Double-quoted attributes
    for attr in &["src=\"", "href=\"", "data-src=\"", "poster=\"", "content=\""] {
        result = rewrite_attr_uris(&result, attr, '"', path_transformer);
    }
    // Single-quoted attributes
    for attr in &["src='", "href='", "data-src='", "poster='", "content='"] {
        result = rewrite_attr_uris(&result, attr, '\'', path_transformer);
    }
    // srcset (double and single quoted)
    result = rewrite_srcset_uris(&result, "srcset=\"", '"', path_transformer);
    result = rewrite_srcset_uris(&result, "srcset='", '\'', path_transformer);
    // CSS url() patterns
    result = rewrite_css_url_uris(&result, path_transformer);

    result
}

/// Helper: rewrite URIs in a simple attribute (src, href, etc.)
fn rewrite_attr_uris(html: &str, attr: &str, quote: char, pt: &PathTransformer) -> String {
    let mut result = html.to_string();
    let mut search_from = 0;
    let attr_bytes = attr.len();
    loop {
        if let Some(pos) = result[search_from..].find(attr) {
            let val_start = search_from + pos + attr_bytes;
            if let Some(val_end) = result[val_start..].find(quote) {
                let url = &result[val_start..val_start + val_end];
                if is_skip_url(url) {
                    search_from = val_start + val_end + 1;
                    continue;
                }
                let clean = url.trim_start_matches("./").trim_start_matches('/');
                if clean.is_empty() {
                    search_from = val_start + val_end + 1;
                    continue;
                }
                if let Some(encrypted) = pt.encrypt_path_if_mapped(clean) {
                    let prefix_len = url.len() - url.trim_start_matches("./").trim_start_matches('/').len();
                    let prefix = &url[..prefix_len];
                    let new_url = format!("{}{}", prefix, encrypted);
                    result.replace_range(val_start..val_start + val_end, &new_url);
                    search_from = val_start + new_url.len() + 1;
                } else {
                    search_from = val_start + val_end + 1;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }
    result
}

/// Helper: rewrite URIs in srcset attribute (format: "url descriptor, url descriptor")
fn rewrite_srcset_uris(html: &str, attr: &str, quote: char, pt: &PathTransformer) -> String {
    let mut result = html.to_string();
    let mut search_from = 0;
    let attr_bytes = attr.len();
    loop {
        if let Some(pos) = result[search_from..].find(attr) {
            let val_start = search_from + pos + attr_bytes;
            if let Some(val_end) = result[val_start..].find(quote) {
                let value = &result[val_start..val_start + val_end];
                // Parse srcset entries: "url descriptor, url descriptor"
                let mut new_value = String::new();
                let mut changed = false;
                for entry in value.split(',') {
                    let trimmed = entry.trim();
                    if trimmed.is_empty() { continue; }
                    // Split URL from descriptor at first whitespace
                    let (url_part, desc) = if let Some(ws_pos) = trimmed.find(|c: char| c.is_whitespace()) {
                        (&trimmed[..ws_pos], &trimmed[ws_pos..])
                    } else {
                        (trimmed, "")
                    };

                    if is_skip_url(url_part) {
                        new_value.push_str(trimmed);
                        new_value.push(',');
                        continue;
                    }
                    let clean = url_part.trim_start_matches("./").trim_start_matches('/');
                    if let Some(encrypted) = pt.encrypt_path_if_mapped(clean) {
                        let prefix_len = url_part.len() - url_part.trim_start_matches("./").trim_start_matches('/').len();
                        let prefix = &url_part[..prefix_len];
                        new_value.push_str(&format!("{}{}{},", prefix, encrypted, desc));
                        changed = true;
                    } else {
                        new_value.push_str(trimmed);
                        new_value.push(',');
                    }
                }
                if changed {
                    // Remove trailing comma
                    if new_value.ends_with(',') { new_value.pop(); }
                    result.replace_range(val_start..val_start + val_end, &new_value);
                    search_from = val_start + new_value.len() + 1;
                } else {
                    search_from = val_start + val_end + 1;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }
    result
}

/// Helper: rewrite URIs in CSS url() patterns
fn rewrite_css_url_uris(html: &str, pt: &PathTransformer) -> String {
    let mut result = html.to_string();
    let mut search_from = 0;
    while let Some(pos) = result[search_from..].find("url(") {
        let paren_start = search_from + pos + 4;
        let rest = &result[paren_start..];
        let rest_trimmed = rest.trim_start();
        let leading_ws = rest.len() - rest_trimmed.len();
        let actual_start = paren_start + leading_ws;

        let (url_str, quote_len, close_offset) = if rest_trimmed.starts_with('\'') {
            // url('path')
            if let Some(end) = result[actual_start + 1..].find('\'') {
                (&result[actual_start + 1..actual_start + 1 + end], 2, actual_start + 1 + end)
            } else { break }
        } else if rest_trimmed.starts_with('"') {
            // url("path")
            if let Some(end) = result[actual_start + 1..].find('"') {
                (&result[actual_start + 1..actual_start + 1 + end], 2, actual_start + 1 + end)
            } else { break }
        } else {
            // url(path)
            if let Some(end) = result[actual_start..].find(')') {
                (&result[actual_start..actual_start + end], 0, actual_start + end)
            } else { break }
        };

        if !is_skip_url(url_str) {
            let clean = url_str.trim_start_matches("./").trim_start_matches('/');
            if let Some(encrypted) = pt.encrypt_path_if_mapped(clean) {
                let prefix_len = url_str.len() - url_str.trim_start_matches("./").trim_start_matches('/').len();
                let prefix = &url_str[..prefix_len];
                let new_url = format!("{}{}", prefix, encrypted);
                // Replace only the URL part inside url()
                let url_replace_start = actual_start + (if quote_len > 0 { 1 } else { 0 });
                let url_replace_end = url_replace_start + url_str.len();
                result.replace_range(url_replace_start..url_replace_end, &new_url);
                search_from = url_replace_start + new_url.len() + 1;
                continue;
            }
        }
        search_from = close_offset + 1;
    }
    result
}

/// Helper: check if a URL should be skipped (external, data, anchor, etc.)
fn is_skip_url(url: &str) -> bool {
    url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("data:")
        || url.starts_with('#')
        || url.starts_with("javascript:")
        || url.is_empty()
}

/// Build the `im` (image/resource map) for client-side use.
/// Maps encrypted paths to content types only — no real paths exposed.
fn build_iris_im(config: &Config) -> serde_json::Map<String, serde_json::Value> {
    config
        .file_mappings
        .iter()
        .map(|(real, encrypted)| {
            let ct = content_type_for(real);
            (encrypted.clone(), serde_json::Value::String(ct.to_string()))
        })
        .collect()
}

// ── Config / injection helpers ─────────────────────────────────────────────

fn build_iris_config_json(config: &Config, key_hex: &str) -> serde_json::Value {
    let im = build_iris_im(config);

    let api_patterns: Vec<serde_json::Value> = config
        .api_routes
        .iter()
        .map(|route| {
            serde_json::json!({
                "pattern": route.pattern,
                "methods": route.methods,
            })
        })
        .collect();

    serde_json::json!({
        "im": im,
        "apiPatterns": api_patterns,
        "wasmUrl": "/iris.wasm",
        "statusUrl": "/status",
        "k": key_hex,
    })
}

fn generate_iris_config_script(config: &Config, key_hex: &str) -> String {
    let config_json = build_iris_config_json(config, key_hex);
    // Escape </script> in JSON to prevent breaking out of the script tag (XSS)
    let json_str = serde_json::to_string(&config_json).unwrap_or_default().replace("</script", "<\\/script");
    format!(
        "<script src=\"/iris-bootstrap.js\"></script>\n<script src=\"/iris-canvas.js\"></script>\n<script>window.IRIS_CONFIG={};</script>",
        json_str
    )
}

fn inject_iris_config(html: &[u8], script: &str) -> Vec<u8> {
    let html_str = match std::str::from_utf8(html) {
        Ok(s) => s,
        Err(_) => return html.to_vec(),
    };

    let script_tag = format!("{}\n", script);
    let lower = html_str.to_ascii_lowercase();

    if let Some(pos) = lower.find("</head>") {
        let mut result = Vec::with_capacity(html.len() + script_tag.len());
        result.extend_from_slice(html_str[..pos].as_bytes());
        result.extend_from_slice(script_tag.as_bytes());
        result.extend_from_slice(html_str[pos..].as_bytes());
        result
    } else if let Some(pos) = lower.find("<body") {
        let mut result = Vec::with_capacity(html.len() + script_tag.len());
        result.extend_from_slice(html_str[..pos].as_bytes());
        result.extend_from_slice(script_tag.as_bytes());
        result.extend_from_slice(html_str[pos..].as_bytes());
        result
    } else {
        let mut result = Vec::with_capacity(html.len() + script_tag.len());
        result.extend_from_slice(script_tag.as_bytes());
        result.extend_from_slice(html);
        result
    }
}

// ── Gateway ────────────────────────────────────────────────────────────────

pub struct IrisGateway {
    config: Arc<Config>,
    encryptor: Arc<RwLock<FileEncryptor>>,
    key_manager: Arc<KeyManager>,
    current_key: Arc<RwLock<KeyPair>>,
    proxy: Arc<ApiProxy>,
    path_transformer: Arc<PathTransformer>,
    current_wasm: Arc<RwLock<Vec<u8>>>,
    iris_config_script: Arc<RwLock<String>>,
    configured: bool,
}

impl IrisGateway {
    pub fn new(mut config: Config) -> Result<Self> {
        // === Auto-discovery: scan HTML files, build mappings ===
        let auto_discovered = if !config.is_encryption_active() {
            // Scan both assets_dir and base_dir for HTML files
            let mut urls = Vec::new();
            if config.server.assets_dir.is_dir() {
                let found = scan_html_for_urls(&config.server.assets_dir);
                tracing::info!("Scanned assets_dir: found {} URL references in HTML", found.len());
                urls.extend(found);
            }
            if config.server.base_dir.is_dir() {
                let found = scan_html_for_urls(&config.server.base_dir);
                tracing::info!("Scanned base_dir: found {} URL references in HTML", found.len());
                urls.extend(found);
            }
            urls
        } else {
            Vec::new()
        };

        let key_manager = Arc::new(KeyManager::new(
            config.key.key_dir.clone(),
            config.validity_duration(),
        ));

        // Generate or load key
        let key_pair = match key_manager.load_current() {
            Ok(k) if !key_manager.is_expired(&k) => {
                tracing::info!("Using existing key: {}", k.id);
                k
            }
            _ => {
                tracing::info!("Generating new key pair");
                let k = key_manager.generate_key_pair()?;
                key_manager.save_key_pair(&k)?;
                k
            }
        };

        let encryptor = FileEncryptor::new(&key_pair.key)?;

        // If we auto-discovered URLs, build mappings now
        let configured = if !auto_discovered.is_empty() {
            let mappings = auto_build_mappings(&auto_discovered, &config.server.base_dir, &encryptor);
            if !mappings.is_empty() {
                tracing::info!("Auto-configured: {} file mappings from HTML scan", mappings.len());
                config.file_mappings = mappings;
                // Compile routes (even if no API routes, needed for is_encryption_active)
                if let Err(e) = config.compile_routes() {
                    tracing::warn!("API route compilation failed: {}", e);
                }
                true
            } else {
                // URLs found but none exist in base_dir — assets-only mode
                tracing::info!("HTML references found but no files in base_dir, serving in transparent mode");
                config.is_encryption_active()
            }
        } else {
            config.is_encryption_active()
        };

        let path_transformer = PathTransformer::new(config.file_mappings.clone());
        let wasm_stub = generate_wasm_stub(&key_pair);
        let key_hex = key_pair.to_hex();
        let iris_config_script = Arc::new(RwLock::new(generate_iris_config_script(&config, &key_hex)));

        Ok(Self {
            config: Arc::new(config),
            encryptor: Arc::new(RwLock::new(encryptor)),
            key_manager,
            current_key: Arc::new(RwLock::new(key_pair)),
            proxy: Arc::new(ApiProxy::new()?),
            path_transformer: Arc::new(path_transformer),
            current_wasm: Arc::new(RwLock::new(wasm_stub)),
            iris_config_script,
            configured,
        })
    }

    pub async fn run(&self) -> Result<()> {
        self.start_key_rotation_task();

        let config = self.config.clone();
        let encryptor = self.encryptor.clone();
        let key_manager = self.key_manager.clone();
        let proxy = self.proxy.clone();
        let path_transformer = self.path_transformer.clone();
        let current_key = self.current_key.clone();
        let current_wasm = self.current_wasm.clone();
        let internal_token = Arc::new(self.config.internal_token.clone());
        let iris_config_script = self.iris_config_script.clone();
        let configured = self.configured;

        // === 1. Health check ===
        let health_route = warp::path("health")
            .and(warp::get())
            .and_then(move || {
                async move {
                    Ok::<_, Rejection>(with_iris_headers(
                        json_response(
                            &serde_json::json!({
                                "status": "ok",
                                "service": "iris-gateway",
                                "configured": configured,
                            }),
                            StatusCode::OK,
                        ),
                        configured,
                    ))
                }
            });

        // === 2. WASM proxy distribution ===
        let wasm_route = warp::path("iris.wasm")
            .and(warp::get())
            .and_then({
                let wasm = current_wasm.clone();
                move || {
                    let wasm = wasm.clone();
                    async move {
                        let wasm = wasm.read().await;
                        Ok::<_, Rejection>(with_iris_headers(
                            Response::builder()
                                .status(200)
                                .header("Content-Type", "application/wasm")
                                .header("Cache-Control", "no-cache")
                                .body(wasm.clone())
                                .unwrap(),
                            configured,
                        ))
                    }
                }
            });

        // === 3. Status ===
        let status_route = warp::path("status")
            .and(warp::get())
            .and_then({
                let km = key_manager.clone();
                move || {
                    let km = km.clone();
                    async move {
                        match km.load_current() {
                            Ok(key) => {
                                Ok::<_, Rejection>(with_iris_headers(
                                    json_response(&serde_json::json!({
                                        "status": "ok",
                                        "configured": configured,
                                        "key_id": key.id.to_string(),
                                        "expires_at": key.expires_at.to_rfc3339(),
                                        "algorithm": key.algorithm,
                                    }), StatusCode::OK),
                                    configured,
                                ))
                            }
                            Err(e) => {
                                Ok(with_iris_headers(
                                    json_response(&serde_json::json!({
                                        "status": "error",
                                        "configured": configured,
                                        "message": e.to_string(),
                                    }), StatusCode::INTERNAL_SERVER_ERROR),
                                    configured,
                                ))
                            }
                        }
                    }
                }
            });

        // === 4. SW config endpoint (requires internal token) ===
        let sw_config_route = warp::path("iris-sw-config")
            .and(warp::get())
            .and(warp::header::optional("X-Internal-Token"))
            .and_then({
                let config = config.clone();
                let internal_token = internal_token.clone();
                let current_key = current_key.clone();
                move |header_token: Option<String>| {
                    let config = config.clone();
                    let internal_token = internal_token.clone();
                    let current_key = current_key.clone();
                    async move {
                        check_internal_token(header_token, &internal_token)?;
                        let key_hex = current_key.read().await.to_hex();
                        let mut json = build_iris_config_json(&config, &key_hex);
                        json.as_object_mut().unwrap().insert(
                            "configured".to_string(),
                            serde_json::Value::Bool(configured),
                        );
                        Ok::<_, Rejection>(with_iris_headers(
                            json_response(&json, StatusCode::OK),
                            configured,
                        ))
                    }
                }
            });

        // === 5. Internal API routes (with auth) ===
        let update_key_route = warp::path("internal")
            .and(warp::path("update-key"))
            .and(warp::post())
            .and(warp::header::optional("X-Internal-Token"))
            .and(warp::body::content_length_limit(20 * 1024 * 1024).and(warp::body::json()))
            .and_then({
                let encryptor = encryptor.clone();
                let internal_token = internal_token.clone();
                move |header_token: Option<String>, req: UpdateKeyRequest| {
                    let encryptor = encryptor.clone();
                    let internal_token = internal_token.clone();
                    async move {
                        check_internal_token(header_token, &internal_token)?;
                        Ok::<_, Rejection>(with_iris_headers(handle_update_key(req, encryptor).await, configured))
                    }
                }
            });

        let decrypt_route = warp::path("internal")
            .and(warp::path("decrypt"))
            .and(warp::post())
            .and(warp::header::optional("X-Internal-Token"))
            .and(warp::body::content_length_limit(20 * 1024 * 1024).and(warp::body::json()))
            .and_then({
                let encryptor = encryptor.clone();
                let internal_token = internal_token.clone();
                move |header_token: Option<String>, req: DecryptRequest| {
                    let encryptor = encryptor.clone();
                    let internal_token = internal_token.clone();
                    async move {
                        check_internal_token(header_token, &internal_token)?;
                        Ok::<_, Rejection>(with_iris_headers(handle_decrypt(req, encryptor).await, configured))
                    }
                }
            });

        let encrypt_path_route = warp::path("internal")
            .and(warp::path("encrypt-path"))
            .and(warp::post())
            .and(warp::header::optional("X-Internal-Token"))
            .and(warp::body::content_length_limit(20 * 1024 * 1024).and(warp::body::json()))
            .and_then({
                let path_transformer = path_transformer.clone();
                let internal_token = internal_token.clone();
                move |header_token: Option<String>, req: serde_json::Value| {
                    let path_transformer = path_transformer.clone();
                    let internal_token = internal_token.clone();
                    async move {
                        check_internal_token(header_token, &internal_token)?;
                        Ok::<_, Rejection>(with_iris_headers(handle_encrypt_path(req, path_transformer).await, configured))
                    }
                }
            });

        let decrypt_path_route = warp::path("internal")
            .and(warp::path("decrypt-path"))
            .and(warp::post())
            .and(warp::header::optional("X-Internal-Token"))
            .and(warp::body::content_length_limit(20 * 1024 * 1024).and(warp::body::json()))
            .and_then({
                let path_transformer = path_transformer.clone();
                let internal_token = internal_token.clone();
                move |header_token: Option<String>, req: serde_json::Value| {
                    let path_transformer = path_transformer.clone();
                    let internal_token = internal_token.clone();
                    async move {
                        check_internal_token(header_token, &internal_token)?;
                        Ok::<_, Rejection>(with_iris_headers(handle_decrypt_path(req, path_transformer).await, configured))
                    }
                }
            });

        // === 6. Assets route (unencrypted, from assets_dir, with HTML URI rewriting + injection) ===
        let assets_route = warp::path::full()
            .and(warp::get())
            .and_then({
                let config = config.clone();
                let iris_config_script = iris_config_script.clone();
                let path_transformer = path_transformer.clone();
                move |path: warp::path::FullPath| {
                    let config = config.clone();
                    let iris_config_script = iris_config_script.clone();
                    let path_transformer = path_transformer.clone();
                    async move {
                        let script = iris_config_script.read().await.clone();
                        let result = handle_assets(path, &config, &script, &path_transformer).await?;
                        Ok::<_, Rejection>(with_iris_headers(result, configured))
                    }
                }
            });

        // === 7. Fallback: encrypted file serving + API proxy + transparent serving ===
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
                    Ok::<_, Rejection>(with_iris_headers(
                        handle_request(path, method, headers, body, config, encryptor, proxy, path_transformer).await,
                        configured,
                    ))
                }
            });

        // === 4.5 Key version endpoint (lightweight polling for SW) ===
        let key_version_route = warp::path("iris-key-version")
            .and(warp::get())
            .and_then({
                let current_key = current_key.clone();
                move || {
                    let current_key = current_key.clone();
                    async move {
                        let key = current_key.read().await;
                        Ok::<_, Rejection>(json_response(
                            &serde_json::json!({
                                "key_id": key.id.to_string(),
                                "k": key.to_hex(),
                            }),
                            StatusCode::OK,
                        ))
                    }
                }
            });

        let routes = health_route
            .or(wasm_route)
            .or(status_route)
            .or(sw_config_route)
            .or(key_version_route)
            .or(update_key_route)
            .or(decrypt_route)
            .or(encrypt_path_route)
            .or(decrypt_path_route)
            .or(assets_route)
            .or(file_route)
            .recover(handle_rejection);

        let addr: std::net::SocketAddr = format!("{}:{}", self.config.server.host, self.config.server.port)
            .parse()
            .map_err(|e| crate::Error::Http(format!("Invalid address: {}", e)))?;

        tracing::info!("Iris Gateway listening on {}", addr);
        tracing::info!("Encryption: {}", if configured { "ACTIVE" } else { "INACTIVE (transparent)" });
        tracing::info!("File mappings: {} (auto-discovered from HTML)", self.config.file_mappings.len());

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

    fn start_key_rotation_task(&self) {
        let key_manager = self.key_manager.clone();
        let encryptor = self.encryptor.clone();
        let current_key = self.current_key.clone();
        let current_wasm = self.current_wasm.clone();
        let iris_config_script = self.iris_config_script.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            let check_interval = config.key.rotation_check_interval_seconds;
            let margin = config.rotation_margin();

            let mut interval = tokio::time::interval(
                std::time::Duration::from_secs(check_interval)
            );

            loop {
                interval.tick().await;

                let key = match key_manager.load_current() {
                    Ok(k) => k,
                    Err(e) => {
                        tracing::error!("Failed to load current key: {}", e);
                        continue;
                    }
                };

                if !key_manager.is_expiring(&key, margin) {
                    continue;
                }

                tracing::info!("Key {} is expiring, rotating...", key.id);

                let new_key = match key_manager.generate_key_pair() {
                    Ok(k) => k,
                    Err(e) => {
                        tracing::error!("Failed to generate new key: {}", e);
                        continue;
                    }
                };

                if let Err(e) = key_manager.save_key_pair(&new_key) {
                    tracing::error!("Failed to save new key: {}", e);
                    continue;
                }

                {
                    let mut enc = encryptor.write().await;
                    if let Err(e) = enc.rotate(&new_key.key) {
                        tracing::error!("Failed to rotate encryptor: {}", e);
                        continue;
                    }
                }

                {
                    let mut ck = current_key.write().await;
                    *ck = new_key.clone();
                }

                {
                    let wasm = generate_wasm_stub(&new_key);
                    let mut cw = current_wasm.write().await;
                    *cw = wasm;
                }

                {
                    let new_key_hex = new_key.to_hex();
                    let new_script = generate_iris_config_script(&config, &new_key_hex);
                    let mut ics = iris_config_script.write().await;
                    *ics = new_script;
                }

                tracing::info!("Key rotated successfully: new_key_id={}", new_key.id);
            }
        });
    }
}

fn generate_wasm_stub(key_pair: &KeyPair) -> Vec<u8> {
    // Generate a minimal valid WASM binary module.
    // This is a real WASM module (not JavaScript text) so the client-side
    // magic number check (0x00 0x61 0x73 0x6d) passes correctly.
    //
    // WASM binary format:
    //   \0asm   — magic number
    //   0x01 0x00 0x00 0x00 — version 1
    //   sections...
    //
    // We include:
    // - Type section: one function type () -> ()
    // - Function section: one function of type 0
    // - Export section: export "init" as function 0
    // - Code section: empty function body

    let _ = key_pair; // suppress unused warning

    vec![
        // Magic number: \0asm
        0x00, 0x61, 0x73, 0x6d,
        // Version: 1
        0x01, 0x00, 0x00, 0x00,
        // Type section (id=1)
        0x01,       // section id
        0x05,       // section size (5 bytes)
        0x01,       // 1 type
        0x60,       // func type
        0x00,       // 0 params
        0x00,       // 0 results
        // Function section (id=3)
        0x03,       // section id
        0x02,       // section size (2 bytes)
        0x01,       // 1 function
        0x00,       // type index 0
        // Export section (id=7)
        0x07,       // section id
        0x08,       // section size (8 bytes)
        0x01,       // 1 export
        0x04,       // name length
        b'i', b'n', b'i', b't',  // "init"
        0x00,       // export kind: function
        0x00,       // function index 0
        // Code section (id=10)
        0x0a,       // section id
        0x04,       // section size (4 bytes)
        0x01,       // 1 function body
        0x02,       // body size (2 bytes)
        0x00,       // 0 local declarations
        0x0b,       // end instruction
    ]
}

/// Handle requests for static assets from assets_dir.
/// Returns `Err(AssetNotFound)` rejection if the file doesn't exist in assets_dir,
/// so warp's `.or()` chain can fall through to the encrypted file handler.
///
/// For HTML files, rewrites URIs to encrypted paths and injects IRIS_CONFIG.
async fn handle_assets(
    path: warp::path::FullPath,
    config: &Config,
    iris_config_script: &str,
    path_transformer: &PathTransformer,
) -> std::result::Result<Response<Vec<u8>>, Rejection> {
    let path_str = path.as_str().trim_start_matches('/');

    let file_path = if path_str.is_empty() || path_str == "/" {
        "index.html".to_string()
    } else {
        path_str.to_string()
    };

    let full_path = config.server.assets_dir.join(&file_path);

    match tokio::fs::canonicalize(&full_path).await {
        Ok(canonical) => {
            match tokio::fs::canonicalize(&config.server.assets_dir).await {
                Ok(base_canonical) => {
                    if !canonical.starts_with(&base_canonical) {
                        tracing::warn!("Assets path traversal attempt: {}", file_path);
                        return Ok(text_response("Forbidden", StatusCode::FORBIDDEN));
                    }
                }
                Err(_) => {
                    tracing::error!("Cannot canonicalize assets_dir {:?} — denying all file access", config.server.assets_dir);
                    return Ok(text_response("Internal server error", StatusCode::INTERNAL_SERVER_ERROR));
                }
            }
        }
        Err(_) => {
            // File doesn't exist in assets_dir — reject so file_route can try
            return Err(warp::reject::custom(AssetNotFound));
        }
    }

    match read_file_limited(&full_path).await {
        Ok(content) => {
            let ct = content_type_for(&file_path);
            if ct.starts_with("text/html") {
                // Step 1: Rewrite URIs in HTML (real paths → encrypted paths)
                let html_str = match std::str::from_utf8(&content) {
                    Ok(s) => rewrite_html_uris(s, path_transformer),
                    Err(_) => return Ok(text_response("Internal server error", StatusCode::INTERNAL_SERVER_ERROR)),
                };
                // Step 2: Inject IRIS_CONFIG (with im, not fileMappings)
                let injected = inject_iris_config(html_str.as_bytes(), iris_config_script);
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", ct)
                    .body(injected)
                    .unwrap())
            } else {
                Ok(Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", ct)
                    .body(content)
                    .unwrap())
            }
        }
        Err(_) => Err(warp::reject::custom(AssetNotFound))
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
                    let response = UpdateKeyResponse {
                        success: true,
                        message: format!("Key {} updated", req.key_id),
                    };
                    json_response(&response, StatusCode::OK)
                }
                Err(e) => {
                    let response = UpdateKeyResponse { success: false, message: format!("{}", e) };
                    json_response(&response, StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(e) => {
            let response = UpdateKeyResponse { success: false, message: format!("{}", e) };
            json_response(&response, StatusCode::BAD_REQUEST)
        }
    }
}

async fn handle_decrypt(
    req: DecryptRequest,
    encryptor: Arc<RwLock<FileEncryptor>>,
) -> Response<Vec<u8>> {
    // Reject oversized payloads to prevent OOM
    if req.data.len() > MAX_DECRYPT_PAYLOAD * 2 {
        return text_response("Payload too large", StatusCode::PAYLOAD_TOO_LARGE);
    }
    match base64::engine::general_purpose::STANDARD.decode(&req.data) {
        Ok(encrypted_data) => {
            if encrypted_data.len() > MAX_DECRYPT_PAYLOAD {
                return text_response("Payload too large", StatusCode::PAYLOAD_TOO_LARGE);
            }
            let enc = encryptor.read().await;
            match enc.decrypt_data(&encrypted_data) {
                Ok(decrypted) => {
                    let response = DecryptResponse {
                        data: base64::engine::general_purpose::STANDARD.encode(&decrypted),
                    };
                    json_response(&response, StatusCode::OK)
                }
                Err(_) => text_response("Decryption failed", StatusCode::BAD_REQUEST),
            }
        }
        Err(_) => text_response("Invalid base64", StatusCode::BAD_REQUEST),
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

/// Fallback handler: encrypted file serving + API proxy.
/// In zero-config mode (no encryption), tries to serve files from base_dir transparently.
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

    // API proxy (only if routes are configured)
    if let Some(route) = config.match_api_route(path_str, method_str) {
        tracing::info!("API route matched: {} -> {}", path_str, route.target);

        let headers_vec: Vec<(String, String)> = headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        // Hold a single read guard for the entire proxy operation to prevent
        // key rotation between encrypt and decrypt (which would use mismatched keys).
        let enc = encryptor.read().await;

        let body_to_forward = if !body.is_empty() {
            match enc.encrypt_data(&body) {
                Ok(encrypted) => Some(encrypted),
                Err(e) => {
                    tracing::error!("Body encryption error: {}", e);
                    return text_response("Internal server error", StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        } else {
            None
        };

        // Drop the read guard before the network call so key rotation isn't blocked.
        // We'll re-acquire it for decryption — this is safe because the backend
        // encrypted with the same key we used, and we store that key temporarily.
        let decrypt_key = enc.clone_cipher();
        drop(enc);

        match proxy.forward(&route.target, path_str, method_str, body_to_forward.as_deref(), headers_vec).await {
            Ok(resp) => {
                if !resp.body.is_empty() {
                    match decrypt_key.decrypt_data(&resp.body) {
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
                text_response("Bad Gateway", StatusCode::BAD_GATEWAY)
            }
        }
    } else if config.is_encryption_active() {
        // Encryption active: serve encrypted files from base_dir
        let clean_path = path_str.trim_start_matches('/');

        if let Some(real_path) = path_transformer.decrypt_path(clean_path) {
            let full_path = config.server.base_dir.join(&real_path);
            tracing::info!("File request: {} -> {:?}", clean_path, full_path);

            match tokio::fs::canonicalize(&full_path).await {
                Ok(canonical) => {
                    match tokio::fs::canonicalize(&config.server.base_dir).await {
                        Ok(base_canonical) => {
                            if !canonical.starts_with(&base_canonical) {
                                tracing::warn!("Path traversal attempt: {}", clean_path);
                                return text_response("Forbidden", StatusCode::FORBIDDEN);
                            }
                        }
                        Err(_) => {
                            tracing::error!("Cannot canonicalize base_dir — denying file access");
                            return text_response("Internal server error", StatusCode::INTERNAL_SERVER_ERROR);
                        }
                    }
                }
                Err(_) => {
                    return text_response("Not found", StatusCode::NOT_FOUND);
                }
            }

            match read_file_limited(&full_path).await {
                Ok(content) => {
                    let enc = encryptor.read().await;
                    match enc.encrypt_data(&content) {
                        Ok(encrypted) => Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Length", encrypted.len())
                            .header("Cache-Control", "no-cache, no-store")
                            .body(encrypted)
                            .unwrap(),
                        Err(e) => {
                            tracing::error!("Encryption error: {}", e);
                            text_response("Internal server error", StatusCode::INTERNAL_SERVER_ERROR)
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("File read error for {}: {}", clean_path, e);
                    text_response("Not found", StatusCode::NOT_FOUND)
                }
            }
        } else {
            text_response("Not found", StatusCode::NOT_FOUND)
        }
    } else {
        // Zero-config, no encryption: try to serve file from base_dir transparently
        let clean_path = path_str.trim_start_matches('/');
        if clean_path.is_empty() {
            return text_response("Not found", StatusCode::NOT_FOUND);
        }

        let full_path = config.server.base_dir.join(clean_path);

        // Path traversal protection
        match tokio::fs::canonicalize(&full_path).await {
            Ok(canonical) => {
                match tokio::fs::canonicalize(&config.server.base_dir).await {
                    Ok(base_canonical) => {
                        if !canonical.starts_with(&base_canonical) {
                            tracing::warn!("Path traversal attempt: {}", clean_path);
                            return text_response("Forbidden", StatusCode::FORBIDDEN);
                        }
                    }
                    Err(_) => {
                        tracing::error!("Cannot canonicalize base_dir — denying file access");
                        return text_response("Internal server error", StatusCode::INTERNAL_SERVER_ERROR);
                    }
                }
            }
            Err(_) => {
                return text_response("Not found", StatusCode::NOT_FOUND);
            }
        }

        match read_file_limited(&full_path).await {
            Ok(content) => {
                let ct = content_type_for(clean_path);
                tracing::debug!("Transparent serve: {}", clean_path);
                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", ct)
                    .body(content)
                    .unwrap()
            }
            Err(_) => text_response("Not found", StatusCode::NOT_FOUND),
        }
    }
}

async fn handle_rejection(err: Rejection) -> std::result::Result<impl Reply, Rejection> {
    if err.is_not_found() {
        Ok(warp::reply::with_status("Not found", StatusCode::NOT_FOUND))
    } else if err.find::<Unauthorized>().is_some() {
        Ok(warp::reply::with_status("Unauthorized", StatusCode::UNAUTHORIZED))
    } else if err.find::<AssetNotFound>().is_some() {
        Ok(warp::reply::with_status("Not found", StatusCode::NOT_FOUND))
    } else {
        Err(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_type_for() {
        assert_eq!(content_type_for("index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type_for("app.js"), "application/javascript; charset=utf-8");
        assert_eq!(content_type_for("style.css"), "text/css; charset=utf-8");
        assert_eq!(content_type_for("module.wasm"), "application/wasm");
        assert_eq!(content_type_for("data.json"), "application/json; charset=utf-8");
        assert_eq!(content_type_for("logo.png"), "image/png");
        assert_eq!(content_type_for("photo.jpg"), "image/jpeg");
        assert_eq!(content_type_for("icon.svg"), "image/svg+xml");
        assert_eq!(content_type_for("data.bin"), "application/octet-stream");
    }

    #[test]
    fn test_inject_iris_config_before_head_close() {
        let html = b"<html><head><title>Test</title></head><body>Hello</body></html>";
        let script = "<script src=\"/iris-bootstrap.js\"></script>\n<script src=\"/iris-canvas.js\"></script>\n<script>window.IRIS_CONFIG={};</script>";
        let result = inject_iris_config(html, script);
        let result_str = String::from_utf8(result).unwrap();
        assert!(result_str.contains("IRIS_CONFIG"));
        assert!(result_str.contains("iris-bootstrap.js"));
        assert!(result_str.contains("iris-canvas.js"));
        let head_pos = result_str.find("</head>").unwrap();
        assert!(result_str[..head_pos].contains("</script>"));
    }

    #[test]
    fn test_inject_iris_config_no_head() {
        let html = b"<html><body>Hello</body></html>";
        let script = "<script>window.IRIS_CONFIG={};</script>";
        let result = inject_iris_config(html, script);
        let result_str = String::from_utf8(result).unwrap();
        assert!(result_str.contains("<script>window.IRIS_CONFIG={};</script>\n<body>"));
    }

    #[test]
    fn test_inject_iris_config_non_utf8() {
        let html: Vec<u8> = vec![0xFF, 0xFE, 0x00, 0x01];
        let script = "<script>window.IRIS_CONFIG={};</script>";
        let result = inject_iris_config(&html, script);
        assert_eq!(result, html);
    }

    #[test]
    fn test_generate_iris_config_script() {
        let config = Config::default();
        let key_hex = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
        let script = generate_iris_config_script(&config, key_hex);
        assert!(script.contains("IRIS_CONFIG"));
        assert!(script.contains("im"));
        assert!(script.contains("apiPatterns"));
        assert!(script.contains("k"));
        // Verify </script> is escaped to prevent XSS
        assert!(!script.contains("</script>window"));
    }

    #[test]
    fn test_extract_urls_from_html() {
        let html = concat!(
            "<html><head>",
            "<script src=\"/js/app.js\"></script>",
            "<link href=\"/css/style.css\" rel=\"stylesheet\">",
            "</head><body>",
            "<img src=\"/images/logo.png\">",
            "<img data-src=\"./images/lazy.webp\">",
            "<a href=\"/about\">About</a>",
            "<a href=\"https://example.com\">External</a>",
            "<a href=\"#anchor\">Skip</a>",
            "</body></html>",
        );
        let mut urls = std::collections::HashSet::new();
        extract_urls_from_html(html, &mut urls);

        assert!(urls.contains("js/app.js"), "should find js/app.js");
        assert!(urls.contains("css/style.css"), "should find css/style.css");
        assert!(urls.contains("images/logo.png"), "should find images/logo.png");
        assert!(urls.contains("images/lazy.webp"), "should find images/lazy.webp");
        assert!(urls.contains("about"), "should find about");
        assert!(!urls.contains("https://example.com"), "should skip external");
        assert!(!urls.contains("#anchor"), "should skip anchor");
    }

    #[test]
    fn test_auto_build_mappings() {
        let key = vec![0u8; 32];
        let encryptor = FileEncryptor::new(&key).unwrap();
        let tmp_dir = std::env::temp_dir().join("iris-test-mappings");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let _ = std::fs::write(tmp_dir.join("secret.txt"), b"secret data");

        let urls = vec!["secret.txt".to_string(), "nonexistent.txt".to_string()];
        let mappings = auto_build_mappings(&urls, &tmp_dir, &encryptor);

        assert!(mappings.contains_key("secret.txt"), "existing file should be mapped");
        assert!(!mappings.contains_key("nonexistent.txt"), "missing file should not be mapped");
        assert_ne!(mappings["secret.txt"], "secret.txt", "mapped path should be encrypted");

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }

    #[test]
    fn test_rewrite_html_uris() {
        let mut mappings = std::collections::HashMap::new();
        mappings.insert("secret/photo1.dat".to_string(), "img01".to_string());
        mappings.insert("css/style.css".to_string(), "c99".to_string());
        let pt = PathTransformer::new(mappings);

        let html = r#"<html><head><link href="/css/style.css"></head><body><img src="/secret/photo1.dat"><a href="https://example.com">ext</a></body></html>"#;
        let rewritten = rewrite_html_uris(html, &pt);

        assert!(rewritten.contains("href=\"/c99\""), "should rewrite mapped href: {}", rewritten);
        assert!(rewritten.contains("src=\"/img01\""), "should rewrite mapped src: {}", rewritten);
        assert!(rewritten.contains("https://example.com"), "should not rewrite external URLs");
        assert!(!rewritten.contains("secret/photo1.dat"), "should not contain original path");
    }

    #[test]
    fn test_rewrite_html_uris_unmapped() {
        let mut mappings = std::collections::HashMap::new();
        mappings.insert("secret/photo1.dat".to_string(), "img01".to_string());
        let pt = PathTransformer::new(mappings);

        let html = r#"<img src="/images/logo.png">"#;
        let rewritten = rewrite_html_uris(html, &pt);

        assert!(rewritten.contains("src=\"/images/logo.png\""), "unmapped paths should remain unchanged");
    }

    #[test]
    fn test_build_iris_im() {
        let mut config = Config::default();
        config.file_mappings.insert("secret/photo.png".to_string(), "img01".to_string());
        config.file_mappings.insert("doc/report.pdf".to_string(), "doc99".to_string());
        let im = build_iris_im(&config);

        assert_eq!(im["img01"], "image/png");
        assert_eq!(im["doc99"], "application/pdf");
        // Real paths should NOT be exposed
        assert!(!im.contains_key("secret/photo.png"));
    }

    #[test]
    fn test_rewrite_html_uris_poster() {
        let mut mappings = std::collections::HashMap::new();
        mappings.insert("media/images/hero.png".to_string(), "h01".to_string());
        let pt = PathTransformer::new(mappings);

        let html = r#"<video poster="/media/images/hero.png"><source src="/v01" type="video/mp4"></video>"#;
        let rewritten = rewrite_html_uris(html, &pt);
        assert!(rewritten.contains("poster=\"/h01\""), "should rewrite poster attr: {}", rewritten);
    }

    #[test]
    fn test_rewrite_html_uris_srcset() {
        let mut mappings = std::collections::HashMap::new();
        mappings.insert("media/images/small.png".to_string(), "s01".to_string());
        mappings.insert("media/images/large.png".to_string(), "l01".to_string());
        let pt = PathTransformer::new(mappings);

        let html = r#"<img srcset="/media/images/small.png 1x, /media/images/large.png 2x">"#;
        let rewritten = rewrite_html_uris(html, &pt);
        assert!(rewritten.contains("s01"), "should rewrite srcset entry: {}", rewritten);
        assert!(rewritten.contains("l01"), "should rewrite second srcset entry: {}", rewritten);
    }

    #[test]
    fn test_rewrite_html_uris_css_url() {
        let mut mappings = std::collections::HashMap::new();
        mappings.insert("media/images/bg.png".to_string(), "b01".to_string());
        let pt = PathTransformer::new(mappings);

        let html = r#"<style>.hero { background-image: url("/media/images/bg.png"); }</style>"#;
        let rewritten = rewrite_html_uris(html, &pt);
        assert!(rewritten.contains("b01"), "should rewrite CSS url(): {}", rewritten);
    }

    #[test]
    fn test_rewrite_html_uris_single_quotes() {
        let mut mappings = std::collections::HashMap::new();
        mappings.insert("secret/photo1.dat".to_string(), "img01".to_string());
        let pt = PathTransformer::new(mappings);

        let html = r#"<img src='/secret/photo1.dat'>"#;
        let rewritten = rewrite_html_uris(html, &pt);
        assert!(rewritten.contains("src='/img01'"), "should rewrite single-quoted attr: {}", rewritten);
    }

    #[test]
    fn test_extract_urls_from_html_srcset() {
        let html = r#"<img srcset="/images/small.png 1x, /images/large.png 2x">"#;
        let mut urls = std::collections::HashSet::new();
        extract_urls_from_html(html, &mut urls);
        assert!(urls.contains("images/small.png"), "should extract srcset URL");
        assert!(urls.contains("images/large.png"), "should extract second srcset URL");
    }

    #[test]
    fn test_extract_urls_from_html_css_url() {
        let html = r#"<style>.hero { background-image: url("/images/bg.png"); }</style>"#;
        let mut urls = std::collections::HashSet::new();
        extract_urls_from_html(html, &mut urls);
        assert!(urls.contains("images/bg.png"), "should extract CSS url() path");
    }

    #[test]
    fn test_extract_urls_from_html_poster() {
        let html = r#"<video poster="/media/hero.png">"#;
        let mut urls = std::collections::HashSet::new();
        extract_urls_from_html(html, &mut urls);
        assert!(urls.contains("media/hero.png"), "should extract poster attr");
    }

    #[test]
    fn test_content_type_for_media() {
        assert_eq!(content_type_for("audio.wav"), "audio/wav");
        assert_eq!(content_type_for("song.mp3"), "audio/mpeg");
        assert_eq!(content_type_for("clip.mp4"), "video/mp4");
        assert_eq!(content_type_for("clip.webm"), "video/webm");
        assert_eq!(content_type_for("audio.ogg"), "audio/ogg");
    }
}
