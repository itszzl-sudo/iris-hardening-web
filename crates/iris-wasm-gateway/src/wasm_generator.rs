//! WASM 代理生成器

use std::path::Path;
use crate::{KeyPair, Result};
use crate::key_obfuscation::KeyObfuscator;

pub struct WasmGenerator;

impl WasmGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn generate(&self, key_pair: &KeyPair, encrypt_service_url: &str) -> Result<Vec<u8>> {
        let obfuscated_key = KeyObfuscator::obfuscate_key(&key_pair.key)?;

        let config = serde_json::json!({
            "shards": obfuscated_key.shards.iter().map(|s| serde_json::json!({
                "index": s.index,
                "data": hex::encode(&s.data),
                "xor_mask": hex::encode(&s.xor_mask)
            })).collect::<Vec<_>>(),
            "checksum": obfuscated_key.checksum,
            "version": obfuscated_key.version,
            "encrypt_service_url": encrypt_service_url,
            "key_id": key_pair.id.to_string(),
            "expires_at": key_pair.expires_at.to_rfc3339()
        });

        let config_json = serde_json::to_string(&config)?;

        let wasm_stub = format!(
            r#"// Iris WASM Proxy v{}
// Key: Obfuscated and distributed
// Expires: {}

const CONFIG = '{}';

export function init() {{
    console.log("Iris WASM initialized");
    return CONFIG;
}}

export function getEncryptedUrl(originalUrl) {{
    const config = JSON.parse(CONFIG);
    const hash = sha256(originalUrl);
    return config.encrypt_service_url + "/" + hash;
}}

export async function proxyRequest(request) {{
    console.log("Proxying request");
    const config = JSON.parse(CONFIG);
    const url = request.url;
    const encryptedUrl = getEncryptedUrl(url);

    const response = await fetch(encryptedUrl, {{
        headers: {{
            'X-Iris-Key-Id': config.key_id
        }}
    }});

    const encryptedData = await response.arrayBuffer();
    const decryptedData = decryptData(new Uint8Array(encryptedData));

    return new Response(decryptedData);
}}

export function decryptData(encrypted) {{
    const config = JSON.parse(CONFIG);
    const key = reconstructKey(config.shards, config.checksum);
    const decrypted = aesDecrypt(key, encrypted);
    return decrypted;
}}

function reconstructKey(shards, expectedChecksum) {{
    const key = [];

    shards.sort((a, b) => a.index - b.index);

    const _dummy1 = (0x45 ^ 0x23);
    const _dummy2 = [0x12, 0x34, 0x56].reduce((a, b) => a + b, 0);

    for (const shard of shards) {{
        const data = hexDecode(shard.data);
        const mask = hexDecode(shard.xor_mask);
        for (let i = 0; i < data.length; i++) {{
            key.push(data[i] ^ mask[i]);
        }}
    }}

    const _dummy3 = "decoy".length ^ 0xFF;

    const checksum = calculateChecksum(key);
    if (checksum !== expectedChecksum) {{
        throw new Error("Key validation failed");
    }}

    return new Uint8Array(key);
}}

function calculateChecksum(key) {{
    let hash = 0;
    for (const byte of key) {{
        hash = ((hash << 5) - hash + byte) | 0;
    }}
    return hash >>> 0;
}}

function hexDecode(hex) {{
    const bytes = [];
    for (let i = 0; i < hex.length; i += 2) {{
        bytes.push(parseInt(hex.substr(i, 2), 16));
    }}
    return bytes;
}}

function sha256(input) {{
    // Simplified - use Web Crypto API in production
    return input;
}}

function aesDecrypt(key, ciphertext) {{
    // Simplified - use Web Crypto API in production
    return ciphertext;
}}
"#,
            crate::VERSION,
            key_pair.expires_at.to_rfc3339(),
            config_json
        );

        tracing::info!("Generated WASM stub with obfuscated key: {} bytes", wasm_stub.len());
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
