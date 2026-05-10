//! 浏览器端 WASM 代理模板
//!
//! 编译后嵌入 iris-wasm-gateway

use wasm_bindgen::prelude::*;
use js_sys::{Object, Reflect, Uint8Array};
use web_sys::{Request, Response, ResponseInit, Headers, RequestInit};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

static mut CONFIG: Option<ProxyConfig> = None;

#[derive(Debug, serde::Deserialize)]
struct ProxyConfig {
    key_id: String,
    key: String,
    encrypt_service_url: String,
    expires_at: String,
}

#[wasm_bindgen]
pub fn init(config_json: &str) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    
    let config: ProxyConfig = serde_json::from_str(config_json)
        .map_err(|e| JsValue::from_str(&format!("Config error: {}", e)))?;
    
    unsafe {
        CONFIG = Some(config);
    }
    
    log("Iris WASM Proxy initialized");
    Ok(())
}

#[wasm_bindgen]
pub fn get_encrypted_url(original_url: &str) -> String {
    unsafe {
        if let Some(config) = &CONFIG {
            let hash = sha256(original_url);
            format!("{}/{}", config.encrypt_service_url, hash)
        } else {
            original_url.to_string()
        }
    }
}

#[wasm_bindgen]
pub async fn proxy_request(request: Request) -> Result<Response, JsValue> {
    let url = request.url();
    let encrypted_url = get_encrypted_url(&url);
    
    log(&format!("Proxying: {} -> {}", url, encrypted_url));
    
    let mut opts = RequestInit::new();
    opts.method("GET");
    
    let request = Request::new_with_str_and_init(&encrypted_url, &opts)?;
    
    let headers = Headers::new()?;
    headers.set("X-Iris-Key-Id", &get_key_id())?;
    Reflect::set(&opts, &JsValue::from_str("headers"), &headers)?;
    
    let window = js_sys::global();
    let fetch = Reflect::get(&window, &JsValue::from_str("fetch"))
        .map_err(|_| JsValue::from_str("fetch not found"))?;
    
    let fetch_promise = js_sys::Function::from(fetch)
        .call1(&JsValue::NULL, &opts)?;
    
    let response = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(fetch_promise))
        .await?;
    
    let response = Response::from(response);
    
    let body = wasm_bindgen_futures::JsFuture::from(response.array_buffer()?)
        .await?;
    
    let encrypted_data = js_sys::Uint8Array::new(&body).to_vec();
    
    let mut decrypted_data = decrypt_data(&encrypted_data)?;
    
    let mut init = ResponseInit::new();
    init.status(200);
    init.headers(&headers);
    
    Response::new_with_opt_u8_array_and_init(Some(&mut decrypted_data), &init)
}

#[wasm_bindgen]
pub fn decrypt_data(encrypted: &[u8]) -> Result<Vec<u8>, JsValue> {
    unsafe {
        if let Some(config) = &CONFIG {
            let key_bytes = hex::decode(&config.key)
                .map_err(|e| JsValue::from_str(&format!("Key error: {}", e)))?;
            
            let decrypted = aes_decrypt(&key_bytes, encrypted)?;
            Ok(decrypted)
        } else {
            Err(JsValue::from_str("Not initialized"))
        }
    }
}

fn get_key_id() -> String {
    unsafe {
        CONFIG.as_ref().map(|c| c.key_id.clone()).unwrap_or_default()
    }
}

fn sha256(input: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash = hasher.finalize();
    hex::encode(hash)
}

fn aes_decrypt(key: &[u8], data: &[u8]) -> Result<Vec<u8>, JsValue> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };

    const NONCE_SIZE: usize = 12;

    let key_array: [u8; 32] = key.try_into()
        .map_err(|_| JsValue::from_str("Invalid key"))?;

    let cipher = Aes256Gcm::new_from_slice(&key_array)
        .map_err(|e| JsValue::from_str(&format!("Cipher error: {}", e)))?;

    if data.len() < NONCE_SIZE {
        return Err(JsValue::from_str("Data too short to contain nonce"));
    }

    let (nonce_bytes, ciphertext) = data.split_at(NONCE_SIZE);
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher.decrypt(nonce, ciphertext)
        .map_err(|e| JsValue::from_str(&format!("Decryption error: {}", e)))
}
