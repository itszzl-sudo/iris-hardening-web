//! WASM 代理生成器

use std::path::Path;
use crate::{KeyPair, Result};

pub struct WasmGenerator;

impl WasmGenerator {
    pub fn new() -> Self {
        Self
    }
    
    pub fn generate(&self, key_pair: &KeyPair, encrypt_service_url: &str) -> Result<Vec<u8>> {
        let config = WasmConfig {
            key_id: key_pair.id.to_string(),
            key: key_pair.to_hex(),
            encrypt_service_url: encrypt_service_url.to_string(),
            expires_at: key_pair.expires_at.to_rfc3339(),
        };
        
        let config_json = serde_json::to_string(&config)?;
        
        let wasm_stub = format!(
            r#"// Iris WASM Proxy v{}
// Config: {}
// This is a stub - replace with actual compiled WASM
export function init(configJson) {{
    console.log("Iris WASM initialized with:", configJson);
}}

export function getEncryptedUrl(originalUrl) {{
    console.log("Encrypting URL:", originalUrl);
    return originalUrl;
}}

export async function proxyRequest(request) {{
    console.log("Proxying request");
    return request;
}}

export function decryptData(encrypted) {{
    console.log("Decrypting data");
    return encrypted;
}}
"#,
            crate::VERSION,
            config_json
        );
        
        tracing::info!("Generated WASM stub: {} bytes", wasm_stub.len());
        Ok(wasm_stub.into_bytes())
    }
    
    pub fn generate_to_file(&self, key_pair: &KeyPair, encrypt_service_url: &str, output: &Path) -> Result<()> {
        let wasm = self.generate(key_pair, encrypt_service_url)?;
        std::fs::write(output, wasm)?;
        tracing::info!("WASM written to: {:?}", output);
        Ok(())
    }
}

impl Default for WasmGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, serde::Serialize)]
struct WasmConfig {
    key_id: String,
    key: String,
    encrypt_service_url: String,
    expires_at: String,
}
