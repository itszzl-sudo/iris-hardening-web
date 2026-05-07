//! 浏览器端 WASM 代理模板
//!
//! 编译后嵌入 iris-wasm-gateway
//! 使用混淆密钥机制，防止静态分析提取密钥
//!
//! 注意：此模块仅用于 wasm32 target

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use js_sys::{Object, Reflect};
use web_sys::{Request, Response, ResponseInit, Headers};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

static mut OBFUSCATED_KEY: Option<ObfuscatedKeyStore> = None;

#[derive(Debug, serde::Deserialize)]
struct ObfuscatedKeyStore {
    shards: Vec<KeyShardStore>,
    checksum: u32,
    version: u32,
    encrypt_service_url: String,
    key_id: String,
    expires_at: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct KeyShardStore {
    index: usize,
    data: Vec<u8>,
    xor_mask: Vec<u8>,
}

#[wasm_bindgen]
pub fn init(config_json: &str) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let config: ObfuscatedKeyStore = serde_json::from_str(config_json)
        .map_err(|e| JsValue::from_str(&format!("Config error: {}", e)))?;

    unsafe {
        OBFUSCATED_KEY = Some(config);
    }

    log("Iris WASM Proxy initialized with obfuscated key");
    Ok(())
}

#[wasm_bindgen]
pub fn get_encrypted_url(original_url: &str) -> String {
    unsafe {
        if let Some(config) = &OBFUSCATED_KEY {
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

    let mut opts = Request::new_with_str_and_init(&encrypted_url, &request)?;

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

    let decrypted_data = decrypt_data(&encrypted_data)?;

    let decrypted_array = js_sys::Uint8Array::new_with_length(decrypted_data.len() as u32);
    decrypted_array.copy_from(&decrypted_data);

    let init = ResponseInit::new();
    init.status(200);
    init.headers(&headers);

    Response::new_with_opt_u8_array_and_init(Some(&decrypted_array), &init)
}

#[wasm_bindgen]
pub fn decrypt_data(encrypted: &[u8]) -> Result<Vec<u8>, JsValue> {
    unsafe {
        if let Some(config) = &OBFUSCATED_KEY {
            let key_bytes = reconstruct_key(&config.shards, config.checksum)?;

            let decrypted = aes_decrypt(&key_bytes, encrypted)?;
            Ok(decrypted)
        } else {
            Err(JsValue::from_str("Not initialized"))
        }
    }
}

fn reconstruct_key(shards: &[KeyShardStore], expected_checksum: u32) -> Result<Vec<u8>, JsValue> {
    let mut key = Vec::with_capacity(32);

    let mut sorted_shards = shards.to_vec();
    sorted_shards.sort_by_key(|s| s.index);

    let _dummy1 = (0x45 ^ 0x23) as u8;
    let _dummy2 = [0x12, 0x34, 0x56].iter().sum::<u8>();

    for shard in sorted_shards.iter() {
        let original: Vec<u8> = shard.data.iter()
            .zip(shard.xor_mask.iter())
            .map(|(d, m)| d ^ m)
            .collect();
        key.extend_from_slice(&original);
    }

    let _dummy3 = "decoy".len() ^ 0xFF;

    let checksum = calculate_checksum(&key);
    if checksum != expected_checksum {
        return Err(JsValue::from_str("Key validation failed"));
    }

    Ok(key)
}

fn calculate_checksum(key: &[u8]) -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish() as u32
}

fn get_key_id() -> String {
    unsafe {
        OBFUSCATED_KEY.as_ref().map(|c| c.key_id.clone()).unwrap_or_default()
    }
}

fn sha256(input: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash = hasher.finalize();
    hex::encode(hash)
}

fn aes_decrypt(key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, JsValue> {
    use aes_gcm::{
        aead::{Aead, KeyInit},
        Aes256Gcm, Nonce,
    };

    let key_array: [u8; 32] = key.try_into()
        .map_err(|_| JsValue::from_str("Invalid key"))?;

    let cipher = Aes256Gcm::new_from_slice(&key_array)
        .map_err(|e| JsValue::from_str(&format!("Cipher error: {}", e)))?;

    let nonce = Nonce::from_slice(&[0u8; 12]);

    cipher.decrypt(nonce, ciphertext)
        .map_err(|e| JsValue::from_str(&format!("Decryption error: {}", e)))
}
