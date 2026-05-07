# WASM 定时生成与上传

## 功能概述

iris-wasm-gateway 现在支持：
1. 定期自动生成新的 iris.wasm（带混淆密钥）
2. 自动上传到 Cloudflare Pages
3. 密钥轮换管理

## 配置

### 1. 主配置文件 (config.toml)

```toml
[server]
host = "127.0.0.1"
port = 9090
wasm_path = "iris.wasm"

[key]
key_dir = "keys"
validity_hours = 24                    # 密钥有效期（小时）
rotation_margin_hours = 2              # 提前轮换时间（小时）

[encrypt_service]
url = "http://127.0.0.1:8080"
update_key_endpoint = "/internal/update-key"

[scheduler]
check_interval_minutes = 30            # 检查间隔（分钟）
generation_lead_time_hours = 2         # 提前生成时间（小时）
wasm_output_dir = "wasm"               # WASM 输出目录

# Cloudflare 配置文件路径（敏感信息分离）
cloudflare_config_path = "cloudflare.secret.toml"
```

### 2. Cloudflare 敏感配置 (cloudflare.secret.toml)

**⚠️ 此文件包含敏感信息，已在 .gitignore 中排除**

```toml
# Cloudflare Pages 配置
api_token = "your-cloudflare-api-token"
account_id = "your-account-id"
project_name = "iris-wasm"
deployment_branch = "production"
```

#### 获取 Cloudflare 凭证

1. **API Token**
   - 登录 Cloudflare Dashboard
   - My Profile > API Tokens > Create Token
   - 选择 "Cloudflare Pages" 模板
   - 权限：Pages - Edit

2. **Account ID**
   - Cloudflare Dashboard > 右上角账户信息
   - 复制 Account ID

3. **创建 Pages 项目**
   ```bash
   # 通过 Wrangler CLI
   wrangler pages project create iris-wasm
   ```

### 3. 初始化配置

```bash
# 复制示例配置
cp config.example.toml config.toml
cp cloudflare.example.toml cloudflare.secret.toml

# 编辑 cloudflare.secret.toml 填入真实凭据
vim cloudflare.secret.toml
```

## 运行

### 启动网关

```bash
# 使用默认配置
iris-wasm-gateway config.toml

# 或使用环境变量
export CLOUDFLARE_API_TOKEN=your-token
iris-wasm-gateway
```

### 手动生成 WASM

```rust
use iris_wasm_gateway::{KeyManager, WasmGenerator, ManualRotation};

let key_manager = KeyManager::new("keys", Duration::hours(24));
let generator = WasmGenerator::new();

let wasm_path = ManualRotation::generate_wasm_once(
    &key_manager,
    &generator,
    "http://localhost:8080",
    &PathBuf::from("iris.wasm")
)?;
```

### 手动上传

```rust
use iris_wasm_gateway::{CloudflareUploader, CloudflareConfig, ManualRotation};

let config = CloudflareConfig {
    api_token: "your-token".to_string(),
    account_id: "your-account-id".to_string(),
    project_name: "iris-wasm".to_string(),
    deployment_branch: Some("production".to_string()),
};

let uploader = CloudflareUploader::new(config);
let result = ManualRotation::upload_wasm_once(&uploader, &wasm_path).await?;

println!("Deployed to: {}", result.url);
```

## 工作流程

```
启动 → 加载/生成密钥 → 生成 WASM → 上传 Cloudflare
  ↓
定时检查 (每 30 分钟)
  ↓
密钥即将过期？ (提前 2 小时)
  ↓ 是
生成新密钥 → 生成新 WASM → 上传 Cloudflare
  ↓
更新状态 → 清理旧文件
```

## WASM 文件命名

生成的 WASM 文件使用 UUID 命名：
```
wasm/
├── iris-550e8400-e29b-41d4-a716-446655440000.wasm  (当前)
├── iris-6ba7b810-9dad-11d1-80b4-00c04fd430c8.wasm  (历史)
└── ...
```

最多保留 5 个历史版本，自动清理旧文件。

## API 端点

### 查看调度状态

```bash
curl http://localhost:9090/api/scheduler/status
```

响应：
```json
{
  "current_key_id": "550e8400-e29b-41d4-a716-446655440000",
  "next_rotation": "2026-05-08T10:00:00Z",
  "last_rotation": "2026-05-07T10:00:00Z",
  "total_rotations": 5,
  "current_deployment": {
    "id": "abc123",
    "url": "https://iris-wasm.pages.dev",
    "status": "success"
  }
}
```

### 强制轮换

```bash
curl -X POST http://localhost:9090/api/scheduler/rotate
```

## 监控日志

```
2026-05-07 10:00:00 INFO  Starting WASM scheduler
2026-05-07 10:00:00 INFO  Generated WASM: wasm/iris-xxx.wasm
2026-05-07 10:00:01 INFO  Uploading WASM to Cloudflare Pages: iris.wasm (10240 bytes)
2026-05-07 10:00:02 INFO  WASM uploaded successfully: https://iris-wasm.pages.dev
2026-05-07 10:00:02 INFO  Scheduler initialized with key: xxx-xxx-xxx

# 每 30 分钟检查
2026-05-07 10:30:00 DEBUG Not yet time for rotation. Next: 2026-05-08T08:00:00Z

# 密钥轮换
2026-05-08 08:00:00 INFO  Starting key rotation
2026-05-08 08:00:00 INFO  Key rotated: id=yyy-yyy-yyy, next_rotation=2026-05-09T08:00:00Z
```

## 安全建议

1. **API Token 保护**
   - 不要提交到 Git
   - 使用环境变量或密钥管理服务
   - 定期轮换 Token

2. **密钥管理**
   - `validity_hours` 建议 24-48 小时
   - `rotation_margin_hours` 建议 1-2 小时
   - 密钥文件存储在 `keys/` 目录，应加密或使用密钥库

3. **访问控制**
   - 限制 `/api/scheduler/rotate` 端点访问
   - 添加认证中间件

## 故障排查

### 上传失败

```
Cloudflare API error: 403 - Unauthorized
```
- 检查 API Token 权限
- 验证 Account ID 正确

### 密钥验证失败

```
Key validation failed
```
- 检查密钥文件完整性
- 确认密钥未过期

### WASM 生成失败

```
Failed to generate WASM
```
- 检查 `wasm_output_dir` 写权限
- 验证密钥管理器配置
