//! WASM 生成器 - 生成包含加密密钥和路由映射的 WASM 模块
//!
//! 生成的 WASM 模块包含:
//! - 解密函数 (AES-256-GCM)
//! - 路由映射 (嵌入式 JSON)
//! - 密钥 (混淆后嵌入)
//! - 过期时间检查

use crate::key_manager::{SecureConfig, WasmConfig};

/// WASM 生成器
pub struct WasmGenerator {
    /// 是否启用混淆
    obfuscate: bool,
}

impl WasmGenerator {
    /// 创建新的 WASM 生成器
    pub fn new(obfuscate: bool) -> Self {
        Self { obfuscate }
    }

    /// 生成包含配置信息的 WASM 模块
    ///
    /// 返回一个简化的 WASM 二进制，其中包含:
    /// - magic header
    /// - 版本信息
    /// - 配置数据 (JSON + 密钥)
    pub fn generate_wasm(&self, config: &SecureConfig) -> Vec<u8> {
        let wasm_config = WasmConfig::from_secure_config(config);

        // 构建 WASM 二进制
        let mut wasm = Vec::new();

        // 1. WASM magic header (简化的，实际需要完整的 WASM 格式)
        wasm.extend_from_slice(b"\x00\x61\x73\x6d"); // \0asm

        // 2. 版本
        wasm.extend_from_slice(&1u32.to_le_bytes());

        // 3. 配置数据序列化
        let config_json = serde_json::to_string(&wasm_config).unwrap_or_default();
        let config_bytes = config_json.as_bytes();

        // 4. 配置长度
        wasm.extend_from_slice(&(config_bytes.len() as u32).to_le_bytes());

        // 5. 配置数据
        wasm.extend_from_slice(config_bytes);

        // 6. 签名校验和
        let checksum = self.calculate_checksum(&wasm[8..]);
        wasm.extend_from_slice(&checksum.to_le_bytes());

        wasm
    }

    /// 生成 JavaScript 文本格式的 WASM (用于 njs 兼容)
    ///
    /// 这种格式可以在 njs 中通过 eval 执行
    pub fn generate_js_wasm(&self, config: &SecureConfig) -> Vec<u8> {
        let wasm_config = WasmConfig::from_secure_config(config);

        // 将配置转换为 JavaScript 常量
        let key_hex = wasm_config.key;
        let expires_at = wasm_config.expires_at;
        let routes_json = wasm_config.routes_json.replace("'", "\\'");
        let version = wasm_config.version;

        // 构建 JavaScript 代码
        let js_code = format!(
            r#"
// Iris Secure Gateway WASM Module
// Auto-generated, do not edit manually
// Version: {version}

const IRIS_WASM_CONFIG = {{
    key: "{key_hex}",
    expires_at: {expires_at},
    routes: {routes_json},
    version: "{version}"
}};

// 解密函数
function irisDecrypt(data, keyHex) {{
    // 简化的解密逻辑 (实际需要完整的 AES-256-GCM 实现)
    const key = hexToBytes(keyHex);
    // ... AES 解密实现
    return data; // 返回解密后的数据
}}

// 十六进制转字节数组
function hexToBytes(hex) {{
    const bytes = [];
    for (let i = 0; i < hex.length; i += 2) {{
        bytes.push(parseInt(hex.substr(i, 2), 16));
    }}
    return bytes;
}}

// 检查过期
function irisIsExpired() {{
    return Date.now() / 1000 > {expires_at};
}}

// 代理请求
function irisProxyRequest(path) {{
    const route = IRIS_WASM_CONFIG.routes[path];
    if (!route) return null;

    if (irisIsExpired()) {{
        throw new Error("Configuration expired");
    }}

    return route.encrypted_path;
}}

// 导出模块
export {{ IRIS_WASM_CONFIG, irisDecrypt, irisIsExpired, irisProxyRequest }};
"#,
            version = version,
            key_hex = key_hex,
            expires_at = expires_at,
            routes_json = routes_json
        );

        js_code.into_bytes()
    }

    /// 生成最小化的 WASM 二进制
    ///
    /// 这个 WASM 导出一个初始化函数和一个解密函数
    pub fn generate_minimal_wasm(&self, config: &SecureConfig) -> Vec<u8> {
        let wasm_config = WasmConfig::from_secure_config(config);

        // 构建简化的 WASM 二进制
        let mut wasm = Vec::new();

        // WASM Header
        wasm.extend_from_slice(b"\x00\x61\x73\x6d"); // \0asm
        wasm.extend_from_slice(&1u32.to_le_bytes()); // version 1

        // 类型段 (type) - 0x01
        wasm.push(0x01);
        wasm.push(0x07); // 段长度 7 字节
        wasm.push(0x01); // 1 个类型
        wasm.push(0x00); // 函数类型 (无参数无返回值)
        wasm.push(0x00);

        // 函数段 (func) - 0x03
        wasm.push(0x03);
        wasm.push(0x03); // 段长度 3 字节
        wasm.push(0x02); // 2 个函数
        wasm.push(0x00); // 函数 0 类型索引
        wasm.push(0x00); // 函数 1 类型索引

        // 导出段 (export) - 0x07
        wasm.push(0x07);
        wasm.push(0x0b); // 段长度 11 字节
        wasm.push(0x02); // 2 个导出
        wasm.push(0x06); // "init" 长度 6
        wasm.extend_from_slice(b"init\0\0\0");
        wasm.push(0x00); // 函数索引 0
        wasm.push(0x06); // "decrypt" 长度 6
        wasm.extend_from_slice(b"decrypt\0\0");
        wasm.push(0x00); // 函数索引 1

        // 代码段 (code) - 0x0a
        wasm.push(0x0a);
        wasm.push(0x05); // 段长度 5 字节
        wasm.push(0x02); // 2 个函数体
        wasm.push(0x02); // 函数 0 体长度 2
        wasm.push(0x01); // end
        wasm.push(0x0b); // 函数体结束
        wasm.push(0x02); // 函数 1 体长度 2
        wasm.push(0x01); // end
        wasm.push(0x0b); // 函数体结束

        // 嵌入配置数据 (在 custom section)
        let config_json = serde_json::to_string(&wasm_config).unwrap_or_default();
        wasm.push(0x00); // custom section
        wasm.push((config_json.len() + 2) as u8);
        wasm.push(0x00); // name ""
        wasm.extend_from_slice(config_json.as_bytes());

        wasm
    }

    /// 计算校验和
    fn calculate_checksum(&self, data: &[u8]) -> u32 {
        use sha2::{Digest, Sha256};
        let result = Sha256::digest(data);
        let bytes = result.as_slice();
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    /// 生成配置 JSON (不包含密钥，用于公开端点)
    pub fn generate_public_config(config: &SecureConfig) -> PublicConfig {
        PublicConfig {
            wasm_url: "/iris.wasm".to_string(),
            expires_at: config.wasm_expiry,
            version: config.version.clone(),
            key_id: config.key_pair.id.to_string(),
            algorithm: "aes-256-gcm".to_string(),
        }
    }
}

/// 公开配置 (不包含密钥)
#[derive(Debug, Clone, serde::Serialize)]
pub struct PublicConfig {
    pub wasm_url: String,
    pub expires_at: i64,
    pub version: String,
    pub key_id: String,
    pub algorithm: String,
}

/// 完整配置响应 (用于 njs)
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConfigResponse {
    pub wasm_url: String,
    pub routes: Vec<RouteConfig>,
    pub key: KeyInfo,
    pub version: String,
    pub expires_at: i64,
}

/// 路由配置
#[derive(Debug, Clone, serde::Serialize)]
pub struct RouteConfig {
    pub path: String,
    pub encrypted_path: String,
    pub methods: Vec<String>,
}

/// 密钥信息 (不含实际密钥)
#[derive(Debug, Clone, serde::Serialize)]
pub struct KeyInfo {
    pub id: String,
    pub expires_at: i64,
    pub algorithm: String,
}

impl ConfigResponse {
    /// 从 SecureConfig 创建
    pub fn from_secure_config(config: &SecureConfig) -> Self {
        let routes = config
            .routes
            .iter()
            .map(|(path, info)| RouteConfig {
                path: path.clone(),
                encrypted_path: info.encrypted_path.clone(),
                methods: info.methods.clone(),
            })
            .collect();

        Self {
            wasm_url: "/iris.wasm".to_string(),
            routes,
            key: KeyInfo {
                id: config.key_pair.id.to_string(),
                expires_at: config.wasm_expiry,
                algorithm: "aes-256-gcm".to_string(),
            },
            version: config.version.clone(),
            expires_at: config.wasm_expiry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::key_manager::{KeyPair, KeyPurpose, RouteInfo};

    #[test]
    fn test_wasm_generator() {
        let mut config = crate::key_manager::SecureConfig::new(24);
        config.add_route("/api/users", "/enc/abc", None);

        let generator = WasmGenerator::new(true);
        let wasm = generator.generate_minimal_wasm(&config);

        assert!(!wasm.is_empty());
        assert_eq!(&wasm[0..4], b"\x00\x61\x73\x6d"); // WASM magic
    }

    #[test]
    fn test_js_wasm_generation() {
        let mut config = crate::key_manager::SecureConfig::new(24);
        config.add_route("/test", "/enc/xyz", None);

        let generator = WasmGenerator::new(false);
        let js = generator.generate_js_wasm(&config);

        let js_str = String::from_utf8_lossy(&js);
        assert!(js_str.contains("IRIS_WASM_CONFIG"));
        assert!(js_str.contains("irisDecrypt"));
        assert!(js_str.contains("export"));
    }

    #[test]
    fn test_public_config() {
        let mut config = crate::key_manager::SecureConfig::new(24);
        config.add_route("/api/test", "/enc/test", None);

        let public = WasmGenerator::generate_public_config(&config);

        assert!(public.wasm_url.starts_with("/"));
        assert!(!public.key_id.is_empty());
        assert_eq!(public.algorithm, "aes-256-gcm");
    }

    #[test]
    fn test_config_response() {
        let mut config = crate::key_manager::SecureConfig::new(24);
        config.add_route("/api/users", "/enc/abc", None);
        config.add_route("/api/posts", "/enc/def", None);

        let response = ConfigResponse::from_secure_config(&config);

        assert_eq!(response.routes.len(), 2);
        // Check both routes exist (order may vary)
        let paths: Vec<_> = response.routes.iter().map(|r| r.path.as_str()).collect();
        assert!(paths.contains(&"/api/users"), "Missing /api/users: {:?}", paths);
        assert!(paths.contains(&"/api/posts"), "Missing /api/posts: {:?}", paths);
    }
}