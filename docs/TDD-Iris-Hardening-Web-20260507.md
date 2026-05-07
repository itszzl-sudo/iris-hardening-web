# 技术设计文档 (TDD) - Iris Hardening Web

**版本**: 0.1.1  
**日期**: 2026-05-07  
**作者**: Iris Team  
**状态**: 已完成

---

## 1. 概述

### 1.1 项目背景

Iris Hardening Web 是一个安全的 WebAssembly (WASM) 网关解决方案,旨在为现代 Web 应用提供端到端的加密通信和安全 WASM 执行环境。项目通过多层安全机制保护数据传输和代码执行,适用于对安全性要求较高的 Web 应用场景。

### 1.2 目标

- 提供安全的 WASM 模块执行环境,防止恶意代码攻击
- 实现端到端加密通信,保护数据在传输过程中的安全
- 支持动态密钥轮换,增强系统安全性
- 提供高性能的加密代理服务,最小化性能开销
- 简化集成流程,提供易用的 API 接口

### 1.3 范围

本文档涵盖以下内容:
- 系统架构设计
- 核心模块详细设计
- 安全机制实现
- 性能优化策略
- 部署方案

---

## 2. 系统架构

### 2.1 整体架构

Iris Hardening Web 采用分层架构设计,包含三个核心 crate:

```
┌─────────────────────────────────────────────────────┐
│                  Browser Client                      │
│                (iris.wasm + Config)                  │
└────────────────────┬────────────────────────────────┘
                     │ HTTPS
┌────────────────────▼────────────────────────────────┐
│           iris-wasm-gateway (Port 9090)              │
│        - WASM 分发服务                               │
│        - 密钥轮换管理                                 │
│        - 密钥对生成                                   │
└────────────────────┬────────────────────────────────┘
                     │ HTTP (Internal)
┌────────────────────▼────────────────────────────────┐
│         iris-secure-gateway (Port 8080)              │
│        - 文件加密代理                                 │
│        - API 加密转发                                 │
│        - 路径转换                                     │
│        - 密钥管理                                     │
└─────────────────────────────────────────────────────┘
```

### 2.2 模块关系

```
iris-hardening-web (Workspace)
├── iris-wasm
│   ├── WASM 绑定层
│   ├── JavaScript 接口
│   └── WebGL/WebGPU 渲染
├── iris-secure-gateway
│   ├── HTTP 服务端
│   ├── 加密/解密引擎
│   ├── 文件映射管理
│   └── API 路由转发
└── iris-wasm-gateway
    ├── WASM 分发服务
    ├── 密钥生命周期管理
    └── 通知机制
```

### 2.3 技术栈

| 层级 | 技术选型 | 版本 | 说明 |
|------|---------|------|------|
| 核心语言 | Rust | 1.78+ | 高性能、内存安全 |
| 异步运行时 | Tokio | 1.x | 高性能异步 I/O |
| HTTP 框架 | Warp | 0.3 | 轻荷、高性能 |
| WASM 绑定 | wasm-bindgen | 0.2.89 | Rust ↔ JavaScript 桥梁 |
| 加密算法 | AES-GCM | 0.10 | 认证加密 |
| 序列化 | serde / serde_json | 1.0 | 零拷贝序列化 |
| 日志 | tracing | 0.1 | 结构化日志 |

---

## 3. 核心模块设计

### 3.1 iris-wasm

#### 3.1.1 职责

- 提供 Rust 引擎的 WebAssembly 绑定
- 支持 WebGL/WebGPU 渲染
- 暴露 JavaScript API
- 处理浏览器端事件

#### 3.1.2 架构设计

```
┌─────────────────────────────────────────┐
│         JavaScript Runtime               │
│    (import from iris_wasm.js)            │
└──────────────┬──────────────────────────┘
               │ wasm-bindgen
┌──────────────▼──────────────────────────┐
│          iris-wasm (cdylib)              │
│  ┌─────────────────────────────────┐    │
│  │   IrisEngineWasm                 │    │
│  │   - render(canvas_id)            │    │
│  │   - handle_event(type, data)     │    │
│  │   - is_initialized()             │    │
│  └─────────────────────────────────┘    │
│  ┌─────────────────────────────────┐    │
│  │   Utility Functions              │    │
│  │   - init()                       │    │
│  │   - get_version()                │    │
│  │   - create_engine()              │    │
│  └─────────────────────────────────┘    │
└──────────────┬──────────────────────────┘
               │ FFI
┌──────────────▼──────────────────────────┐
│          iris-engine (Rust)              │
│  - iris-core                            │
│  - iris-gpu                             │
│  - iris-layout                          │
│  - iris-dom                             │
│  - iris-js                              │
│  - iris-sfc                             │
└─────────────────────────────────────────┘
```

#### 3.1.3 关键接口

**JavaScript API**:

```javascript
import init, { 
  IrisEngineWasm, 
  get_version,
  create_engine 
} from './pkg/iris_wasm.js';

await init();
const engine = new IrisEngineWasm();
await engine.render('canvas');
engine.handle_event('click', '{"x": 100, "y": 200}');
```

**Rust 绑定**:

```rust
#[wasm_bindgen]
pub struct IrisEngineWasm {
    engine: RefCell<IrisEngine>,
}

#[wasm_bindgen]
impl IrisEngineWasm {
    pub fn new() -> Self;
    pub fn render(&self, canvas_id: &str);
    pub fn handle_event(&self, event_type: &str, data: &str);
    pub fn is_initialized(&self) -> bool;
}

#[wasm_bindgen]
pub fn get_version() -> String;

#[wasm_bindgen]
pub fn create_engine() -> IrisEngineWasm;
```

#### 3.1.4 优化策略

- **内存管理**: 使用 `wee_alloc` 减少内存占用
- **错误处理**: `console_error_panic_hook` 提供友好的错误信息
- **日志**: `tracing-wasm` 支持浏览器 console 日志
- **性能**: Release 配置优化 (opt-level="z", lto=true, strip=true)

### 3.2 iris-secure-gateway

#### 3.2.1 职责

- 提供加密文件访问代理
- 提供加密 API 请求转发
- 管理文件路径映射
- 支持运行时密钥更新

#### 3.2.2 架构设计

```
┌─────────────────────────────────────────┐
│         HTTP Request Handler             │
│              (Warp)                      │
└──────────────┬──────────────────────────┘
               │
       ┌───────▼────────┐
       │  Route Matcher  │
       └───────┬────────┘
               │
    ┌──────────┼──────────┬────────────┐
    │          │          │            │
┌───▼───┐  ┌───▼───┐  ┌───▼────┐  ┌───▼──────┐
│Internal│  │ File  │  │  API   │  │  Static  │
│  API   │  │ Proxy │  │ Proxy  │  │  Files   │
└───┬───┘  └───┬───┘  └───┬────┘  └──────────┘
    │          │          │
    │          │          │
┌───▼──────────▼──────────▼──────────────────┐
│         Encryption Engine                  │
│  - AES-GCM encrypt/decrypt                 │
│  - Path transformation                     │
│  - Key management                          │
└────────────────────────────────────────────┘
```

#### 3.2.3 配置结构

```toml
[server]
host = "127.0.0.1"
port = 8080
base_dir = "./data"

[encryption]
key_file = "key.txt"
algorithm = "aes-256-gcm"

[file_mappings]
"abc123" = "secret/document.pdf"
"def456" = "private/data.json"

[[api_routes]]
pattern = "^/api/v1/.*"
target = "http://localhost:3000"
methods = ["GET", "POST"]
```

#### 3.2.4 请求处理流程

**文件请求流程**:

```
GET /abc123
  │
  ├─> 路径解析: abc123 → secret/document.pdf
  │
  ├─> 文件读取: base_dir/secret/document.pdf
  │
  ├─> 内容加密: AES-GCM encrypt(file_content, key)
  │
  └─> 返回加密数据
```

**API 请求流程**:

```
POST /api/v1/users
  │
  ├─> 路由匹配: pattern="^/api/v1/.*"
  │
  ├─> 请求加密: encrypt(request_body, key)
  │
  ├─> 转发请求: POST http://localhost:3000/api/v1/users
  │
  ├─> 响应解密: decrypt(response_body, key)
  │
  └─> 返回明文响应
```

#### 3.2.5 加密引擎

```rust
pub struct EncryptionEngine {
    key: [u8; 32],
    nonce_counter: AtomicU64,
}

impl EncryptionEngine {
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>>;
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>>;
    pub fn update_key(&mut self, new_key: [u8; 32]);
    pub fn encrypt_path(&self, path: &str) -> Result<String>;
    pub fn decrypt_path(&self, encrypted: &str) -> Result<String>;
}
```

**密钥要求**:
- 长度: 32 字节 (256 bits)
- 格式: Hex 编码
- 生成: `openssl rand -hex 32 > key.txt`

#### 3.2.6 内部 API

| 端点 | 方法 | 功能 | 请求体 |
|------|------|------|--------|
| `/internal/update-key` | POST | 更新加密密钥 | `{key_id, key, expires_at}` |
| `/internal/decrypt` | POST | 解密数据 | `{data}` (base64) |
| `/internal/encrypt-path` | POST | 加密路径 | `{path}` |
| `/internal/decrypt-path` | POST | 解密路径 | `{encrypted}` |

### 3.3 iris-wasm-gateway

#### 3.3.1 职责

- 分发加密的 WASM 代理文件
- 管理密钥对生命周期
- 执行自动密钥轮换
- 通知下游服务更新密钥

#### 3.3.2 架构设计

```
┌─────────────────────────────────────────┐
│         HTTP Server (Port 9090)          │
│              (Warp)                      │
└──────────────┬──────────────────────────┘
               │
       ┌───────▼────────┐
       │  Route Handler │
       └───────┬────────┘
               │
    ┌──────────┴──────────┐
    │                     │
┌───▼───────┐      ┌──────▼─────┐
│ GET       │      │  GET       │
│ /iris.wasm│      │  /status   │
└───┬───────┘      └────────────┘
    │
┌───▼─────────────────────────────┐
│      WASM Builder                │
│  - 嵌入当前密钥                   │
│  - 编译 iris.wasm                 │
│  - 返回二进制                     │
└───┬─────────────────────────────┘
    │
┌───▼─────────────────────────────┐
│   Key Rotation Manager           │
│  - 检测过期时间                   │
│  - 生成新密钥                     │
│  - 通知 iris-secure-gateway      │
└─────────────────────────────────┘
```

#### 3.3.3 配置结构

```toml
[server]
host = "127.0.0.1"
port = 9090

[key]
key_dir = "keys"
validity_hours = 24
rotation_margin_hours = 2

[encrypt_service]
url = "http://127.0.0.1:8080"
update_key_endpoint = "/internal/update-key"
```

#### 3.3.4 密钥轮换流程

```
┌─────────────────────────────────────────┐
│  1. 定时检查 (每小时)                    │
│     current_time >= expires_at - margin  │
└──────────────┬──────────────────────────┘
               │ Yes
┌──────────────▼──────────────────────────┐
│  2. 生成新密钥对                         │
│     - key_id: UUID v4                   │
│     - key: random 32 bytes              │
│     - expires_at: now + validity_hours  │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  3. 持久化密钥                           │
│     - 保存到 key_dir/{key_id}.key       │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  4. 通知下游服务                         │
│     POST http://127.0.0.1:8080/          │
│          internal/update-key             │
│     Body: {key_id, key, expires_at}      │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  5. 重新生成 iris.wasm                   │
│     - 嵌入新密钥                         │
│     - 编译 WASM 二进制                   │
│     - 更新分发版本                       │
└─────────────────────────────────────────┘
```

#### 3.3.5 密钥管理

```rust
pub struct KeyManager {
    current_key: Arc<RwLock<Key>>,
    key_dir: PathBuf,
    validity_duration: Duration,
    rotation_margin: Duration,
}

pub struct Key {
    id: Uuid,
    key: [u8; 32],
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl KeyManager {
    pub async fn rotate_key(&self) -> Result<Key>;
    pub fn get_current_key(&self) -> Key;
    pub async fn notify_downstream(&self, key: &Key) -> Result<()>;
}
```

#### 3.3.6 WASM 构建流程

```rust
pub struct WasmBuilder {
    template_path: PathBuf,
    output_path: PathBuf,
}

impl WasmBuilder {
    pub fn build_with_key(&self, key: &Key) -> Result<Vec<u8>> {
        // 1. 读取 WASM 模板
        // 2. 注入密钥配置
        // 3. 编译为 WASM
        // 4. 优化二进制大小
        // 5. 返回二进制数据
    }
}
```

---

## 4. 安全机制

### 4.1 加密策略

#### 4.1.1 算法选择

**AES-256-GCM**:
- 提供认证加密 (Authenticated Encryption)
- 保证数据机密性和完整性
- 防止篡改攻击
- 性能优秀,硬件加速支持

#### 4.1.2 密钥管理

| 方面 | 策略 | 说明 |
|------|------|------|
| 密钥长度 | 256 bits | 符合 AES-256 标准 |
| 密钥生成 | CSPRNG | 使用 `rand` crate 的安全随机数生成器 |
| 密钥存储 | 文件系统 | hex 编码存储,权限 0600 |
| 密钥轮换 | 24 小时 | 可配置,默认 24 小时过期 |
| 轮换窗口 | 2 小时 | 提前 2 小时开始轮换,避免服务中断 |

#### 4.1.3 Nonce 管理

```rust
pub struct NonceManager {
    counter: AtomicU64,
}

impl NonceManager {
    pub fn generate_nonce(&self) -> [u8; 12] {
        let nonce_value = self.counter.fetch_add(1, Ordering::SeqCst);
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&nonce_value.to_le_bytes()[..12]);
        nonce
    }
}
```

**Nonce 要求**:
- 长度: 12 字节 (GCM 标准)
- 唯一性: 通过原子计数器保证
- 持久化: 重启后从配置文件恢复

### 4.2 路径加密

#### 4.2.1 映射表

```rust
pub struct PathMapping {
    mappings: HashMap<String, String>,
    reverse_mappings: HashMap<String, String>,
}

impl PathMapping {
    pub fn encrypt_path(&self, plain: &str) -> String;
    pub fn decrypt_path(&self, encrypted: &str) -> Result<String>;
}
```

**配置示例**:

```toml
[file_mappings]
"abc123" = "secret/document.pdf"
"def456" = "private/data.json"
"xyz789" = "confidential/report.xlsx"
```

#### 4.2.2 优势

- 隐藏真实文件路径
- 防止路径遍历攻击
- 支持动态映射
- 便于访问控制

### 4.3 WASM 沙箱

#### 4.3.1 隔离机制

- **内存隔离**: WASM 线性内存独立于宿主内存
- **功能隔离**: 仅暴露必要的 JavaScript API
- **网络隔离**: WASM 模块无法直接访问网络

#### 4.3.2 权限控制

```rust
#[wasm_bindgen]
pub fn proxy_request(request: JsValue) -> Promise {
    // 仅允许通过加密网关访问
    // 拦截所有直接请求
}
```

#### 4.3.3 代码签名

- WASM 二进制嵌入当前密钥
- 客户端验证密钥有效性
- 防止中间人替换

### 4.4 传输安全

#### 4.4.1 HTTPS 强制

- 所有生产环境通信强制使用 HTTPS
- 证书验证严格模式
- HSTS 头部启用

#### 4.4.2 证书固定

```javascript
// iris.wasm 中固定证书指纹
const CERT_FINGERPRINT = "sha256/AAAA...";

function verify_certificate(cert) {
    if (cert.fingerprint !== CERT_FINGERPRINT) {
        throw new Error("Certificate mismatch");
    }
}
```

### 4.5 输入验证

#### 4.5.1 路径验证

```rust
pub fn validate_path(path: &str) -> Result<()> {
    // 防止路径遍历
    if path.contains("..") {
        return Err(Error::PathTraversal);
    }
    
    // 防止绝对路径
    if path.starts_with('/') {
        return Err(Error::AbsolutePath);
    }
    
    // 仅允许白名单字符
    if !path.chars().all(|c| c.is_alphanumeric() || c == '/' || c == '.') {
        return Err(Error::InvalidCharacters);
    }
    
    Ok(())
}
```

#### 4.5.2 请求验证

```rust
pub fn validate_request(req: &Request) -> Result<()> {
    // 请求大小限制
    if req.body().len() > MAX_REQUEST_SIZE {
        return Err(Error::RequestTooLarge);
    }
    
    // Content-Type 验证
    if !is_allowed_content_type(&req.content_type()) {
        return Err(Error::InvalidContentType);
    }
    
    Ok(())
}
```

---

## 5. 性能优化

### 5.1 编译优化

**Cargo.toml**:

```toml
[profile.release]
opt-level = "z"        # 优化二进制大小
lto = true             # 链接时优化
codegen-units = 1      # 单个代码生成单元
strip = true           # 移除符号信息
```

### 5.2 运行时优化

#### 5.2.1 异步 I/O

- 使用 Tokio 异步运行时
- 非阻塞文件读取
- 并发请求处理

#### 5.2.2 零拷贝

```rust
use bytes::Bytes;

pub fn encrypt_zero_copy(data: Bytes) -> Bytes {
    // 避免数据复制
    // 直接在原缓冲区上操作
}
```

#### 5.2.3 内存池

```rust
use bytes::BytesMut;

pub struct BufferPool {
    pool: Vec<BytesMut>,
}

impl BufferPool {
    pub fn acquire(&mut self) -> BytesMut;
    pub fn release(&mut self, buffer: BytesMut);
}
```

### 5.3 WASM 优化

#### 5.3.1 大小优化

- 移除未使用代码 (DCE)
- 压缩 WASM 二进制
- 使用 `wee_alloc` 减少堆开销

#### 5.3.2 加载优化

- 启用流式编译
- 使用 WebAssembly.instantiateStreaming
- 实现渐进式加载

```javascript
const response = await fetch('iris.wasm');
const { instance } = await WebAssembly.instantiateStreaming(response);
```

### 5.4 性能指标

| 指标 | 目标 | 说明 |
|------|------|------|
| 加密吞吐量 | > 100 MB/s | AES-GCM 硬件加速 |
| 请求延迟 | < 5 ms | 不含网络往返 |
| WASM 加载 | < 500 ms | 初次加载 |
| 内存占用 | < 10 MB | 单个服务实例 |

---

## 6. 部署方案

### 6.1 架构部署

```
┌────────────────────────────────────────────────┐
│              Load Balancer (Nginx)              │
│            https://iris.example.com             │
└─────────────┬──────────────────┬───────────────┘
              │                  │
    ┌─────────▼────────┐  ┌──────▼─────────┐
    │  iris-wasm-      │  │  iris-secure-  │
    │  gateway:9090    │  │  gateway:8080  │
    │  (Instance 1..N) │  │  (Instance 1..N)│
    └──────────────────┘  └────────────────┘
              │                  │
    ┌─────────▼──────────────────▼────────────┐
    │         Shared Storage (NFS/S3)          │
    │         - Encrypted files                │
    │         - Key files                      │
    │         - WASM binaries                  │
    └─────────────────────────────────────────┘
```

### 6.2 配置管理

#### 6.2.1 环境变量

```bash
# iris-wasm-gateway
export IRIS_WASM_GATEWAY_CONFIG=/etc/iris/wasm-gateway.toml
export IRIS_KEY_DIR=/var/lib/iris/keys
export IRIS_LOG_LEVEL=info

# iris-secure-gateway
export IRIS_SECURE_GATEWAY_CONFIG=/etc/iris/secure-gateway.toml
export IRIS_DATA_DIR=/var/lib/iris/data
export IRIS_ENCRYPTION_KEY=/etc/iris/key.txt
```

#### 6.2.2 Systemd 服务

**iris-wasm-gateway.service**:

```ini
[Unit]
Description=Iris WASM Gateway
After=network.target

[Service]
Type=simple
User=iris
Group=iris
Environment="IRIS_WASM_GATEWAY_CONFIG=/etc/iris/wasm-gateway.toml"
ExecStart=/usr/bin/iris-wasm-gateway
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
```

**iris-secure-gateway.service**:

```ini
[Unit]
Description=Iris Secure Gateway
After=network.target

[Service]
Type=simple
User=iris
Group=iris
Environment="IRIS_SECURE_GATEWAY_CONFIG=/etc/iris/secure-gateway.toml"
ExecStart=/usr/bin/iris-secure-gateway
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
```

### 6.3 容器化部署

#### 6.3.1 Dockerfile

```dockerfile
FROM rust:1.78 AS builder

WORKDIR /app
COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/iris-wasm-gateway /usr/bin/
COPY --from=builder /app/target/release/iris-secure-gateway /usr/bin/

EXPOSE 8080 9090

CMD ["iris-wasm-gateway", "/etc/iris/wasm-gateway.toml"]
```

#### 6.3.2 Docker Compose

```yaml
version: '3.8'

services:
  wasm-gateway:
    image: iris-hardening-web:latest
    command: iris-wasm-gateway /etc/iris/wasm-gateway.toml
    ports:
      - "9090:9090"
    volumes:
      - ./config/wasm-gateway.toml:/etc/iris/wasm-gateway.toml
      - ./keys:/var/lib/iris/keys
    depends_on:
      - secure-gateway

  secure-gateway:
    image: iris-hardening-web:latest
    command: iris-secure-gateway /etc/iris/secure-gateway.toml
    ports:
      - "8080:8080"
    volumes:
      - ./config/secure-gateway.toml:/etc/iris/secure-gateway.toml
      - ./data:/var/lib/iris/data
      - ./keys:/etc/iris/keys:ro

  nginx:
    image: nginx:alpine
    ports:
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf
      - ./certs:/etc/nginx/certs:ro
    depends_on:
      - wasm-gateway
      - secure-gateway
```

### 6.4 监控与日志

#### 6.4.1 日志格式

```rust
tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .json()
    .with_target(true)
    .with_thread_ids(true)
    .init();
```

**示例输出**:

```json
{
  "timestamp": "2026-05-07T10:30:15.123Z",
  "level": "INFO",
  "target": "iris_secure_gateway::handler",
  "threadId": 1,
  "message": "Request processed",
  "path": "/abc123",
  "method": "GET",
  "duration_ms": 3,
  "bytes_encrypted": 2048
}
```

#### 6.4.2 监控指标

| 指标 | 类型 | 说明 |
|------|------|------|
| `iris_requests_total` | Counter | 总请求数 |
| `iris_encryption_bytes_total` | Counter | 加密字节数 |
| `iris_key_rotations_total` | Counter | 密钥轮换次数 |
| `iris_request_duration_seconds` | Histogram | 请求延迟分布 |
| `iris_active_connections` | Gauge | 活跃连接数 |

### 6.5 高可用方案

#### 6.5.1 多实例部署

- 水平扩展 iris-wasm-gateway 和 iris-secure-gateway
- 负载均衡分发流量
- 共享存储保证数据一致性

#### 6.5.2 故障恢复

- 自动重启失败的服务
- 健康检查端点: `/status`
- 优雅关闭处理

```rust
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install Ctrl+C handler");
    
    println!("Shutdown signal received");
}
```

#### 6.5.3 密钥同步

- 密钥文件存储在共享存储 (NFS/S3)
- 所有实例读取同一密钥
- 轮换时自动通知所有实例

---

## 7. 测试策略

### 7.1 单元测试

**加密引擎测试**:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let engine = EncryptionEngine::new([0u8; 32]);
        let plaintext = b"test data";
        
        let ciphertext = engine.encrypt(plaintext).unwrap();
        let decrypted = engine.decrypt(&ciphertext).unwrap();
        
        assert_eq!(plaintext, decrypted.as_slice());
    }
    
    #[test]
    fn test_path_encryption() {
        let engine = EncryptionEngine::new([0u8; 32]);
        let path = "/secret/document.pdf";
        
        let encrypted = engine.encrypt_path(path).unwrap();
        let decrypted = engine.decrypt_path(&encrypted).unwrap();
        
        assert_eq!(path, decrypted);
    }
}
```

### 7.2 集成测试

**API 端点测试**:

```rust
#[tokio::test]
async fn test_file_request() {
    let app = spawn_app().await;
    
    let client = reqwest::Client::new();
    let response = client
        .get(&format!("{}/abc123", app.address))
        .send()
        .await
        .unwrap();
    
    assert!(response.status().is_success());
    
    let encrypted_data = response.bytes().await.unwrap();
    assert!(!encrypted_data.is_empty());
}
```

### 7.3 性能测试

```rust
use criterion::{Criterion, black_box, criterion_group};

fn encryption_benchmark(c: &mut Criterion) {
    let engine = EncryptionEngine::new([0u8; 32]);
    let data = vec![0u8; 1024 * 1024]; // 1 MB
    
    c.bench_function("encrypt_1mb", |b| {
        b.iter(|| engine.encrypt(black_box(&data)))
    });
}

criterion_group!(benches, encryption_benchmark);
```

### 7.4 安全测试

- 渗透测试模拟
- 路径遍历攻击测试
- 密钥泄露模拟
- 中间人攻击测试

---

## 8. 开发指南

### 8.1 本地开发环境

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装 wasm-pack
cargo install wasm-pack

# 克隆项目
git clone https://github.com/itszzl-sudo/iris.git
cd iris/iris-hardening-web

# 构建
cargo build

# 运行测试
cargo test

# 构建文档
cargo doc --open
```

### 8.2 构建命令

```bash
# 开发构建
cargo build

# Release 构建
cargo build --release

# 构建 WASM
./build-release.bat

# 或手动构建
cargo build --target wasm32-unknown-unknown --release
wasm-pack build crates/iris-wasm
```

### 8.3 代码规范

- 使用 `rustfmt` 格式化代码: `cargo fmt`
- 使用 `clippy` 静态分析: `cargo clippy -- -D warnings`
- 遵循 Rust API 设计指南
- 编写文档注释: `///` 和 `//!`

---

## 9. 故障排查

### 9.1 常见问题

| 问题 | 症状 | 解决方案 |
|------|------|---------|
| 密钥加载失败 | 服务启动失败 | 检查密钥文件路径和权限 |
| 加密失败 | "Invalid ciphertext" | 验证密钥是否一致 |
| WASM 加载失败 | 浏览器报错 | 检查 CORS 配置 |
| 路径解密失败 | 404 Not Found | 检查映射表配置 |
| 密钥轮换失败 | 日志显示通知失败 | 检查下游服务可达性 |

### 9.2 日志分析

```bash
# 查看错误日志
journalctl -u iris-wasm-gateway -p err

# 查看最近日志
journalctl -u iris-secure-gateway -n 100

# 实时日志
journalctl -u iris-wasm-gateway -f
```

### 9.3 性能调优

- 调整 Tokio 线程池大小
- 优化缓冲区大小
- 启用硬件加速 (AES-NI)

---

## 10. 未来规划

### 10.1 短期目标 (v0.2.0)

- [ ] 支持 RSA 密钥加密
- [ ] 添加 WebUI 管理界面
- [ ] 实现密钥版本管理
- [ ] 支持多种加密算法切换

### 10.2 中期目标 (v0.3.0)

- [ ] 分布式密钥管理
- [ ] 支持 Kubernetes 部署
- [ ] 实现 Prometheus 监控集成
- [ ] 添加 GraphQL API

### 10.3 长期目标 (v1.0.0)

- [ ] 硬件安全模块 (HSM) 集成
- [ ] 零知识证明支持
- [ ] 多租户隔离
- [ ] 国密算法支持 (SM4)

---

## 11. 参考资料

### 11.1 技术文档

- [Rust 官方文档](https://doc.rust-lang.org/)
- [WebAssembly 规范](https://webassembly.github.io/spec/)
- [AES-GCM 标准 (NIST SP 800-38D)](https://csrc.nist.gov/publications/detail/sp/800-38d/final)
- [wasm-bindgen 指南](https://rustwasm.github.io/wasm-bindgen/)

### 11.2 安全标准

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [NIST 加密标准](https://csrc.nist.gov/projects/cryptographic-standards-and-guidelines)
- [CWE/SANS Top 25](https://cwe.mitre.org/top25/)

---

**文档修订历史**:

| 版本 | 日期 | 修订人 | 说明 |
|------|------|--------|------|
| 0.1.0 | 2026-05-07 | Iris Team | 初始版本 |
| 0.1.1 | 2026-05-07 | Iris Team | 补充部署方案和监控章节 |
