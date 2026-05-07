# 配置文件安全指南

## 文件结构

```
iris-wasm-gateway/
├── config.example.toml              # 主配置示例（可提交）
├── config.toml                      # 主配置（不提交）
├── cloudflare.example.toml          # Cloudflare 配置示例（可提交）
└── cloudflare.secret.toml           # Cloudflare 敏感配置（不提交，包含 token）
```

## .gitignore 规则

以下文件/目录已自动排除：

```gitignore
# Sensitive configs
cloudflare.secret.toml
*.secret.toml
*.secret.json
keys/
*.key
*.pem
```

## 初始化步骤

### 1. 创建配置文件

```bash
cd crates/iris-wasm-gateway

# 复制示例配置
cp config.example.toml config.toml
cp cloudflare.example.toml cloudflare.secret.toml
```

### 2. 编辑敏感配置

编辑 `cloudflare.secret.toml`：

```toml
# Cloudflare Pages 配置
api_token = "your-real-api-token-here"
account_id = "your-account-id-here"
project_name = "iris-wasm"
deployment_branch = "production"
```

⚠️ **重要**：
- 不要将 `cloudflare.secret.toml` 提交到 Git
- 不要在代码中硬编码 API Token
- 定期轮换 API Token

### 3. 验证配置

```bash
# 检查状态
iris-wasm-cli status

# 应该看到
Configuration:
  Server: 127.0.0.1:9090
  Cloudflare config: "cloudflare.secret.toml"
    Project: iris-wasm
```

## 安全最佳实践

### API Token 管理

1. **最小权限原则**
   - 仅授予 Pages - Edit 权限
   - 不要使用 Global API Key

2. **定期轮换**
   - 建议每 90 天轮换一次
   - 使用短生命周期 Token

3. **审计日志**
   - 定期检查 Cloudflare 审计日志
   - 监控异常 API 调用

### 环境变量方式（可选）

也可以使用环境变量替代配置文件：

```bash
export CLOUDFLARE_API_TOKEN="your-token"
export CLOUDFLARE_ACCOUNT_ID="your-account-id"
```

在代码中读取：

```rust
let api_token = std::env::var("CLOUDFLARE_API_TOKEN")
    .expect("CLOUDFLARE_API_TOKEN not set");
```

### 密钥库集成（生产环境）

生产环境建议使用密钥管理服务：

- **AWS Secrets Manager**
- **HashiCorp Vault**
- **Azure Key Vault**
- **Google Secret Manager**

示例：

```rust
// 从 AWS Secrets Manager 读取
use aws_sdk_secretsmanager::Client;

async fn get_cloudflare_config() -> Result<CloudflareConfig> {
    let client = Client::new(&aws_config::load_from_env().await);
    let secret = client
        .get_secret_value()
        .secret_id("iris/cloudflare")
        .send()
        .await?;
    
    let config: CloudflareConfig = serde_json::from_str(
        secret.secret_string.as_deref().unwrap()
    )?;
    
    Ok(config)
}
```

## 泄露应对

如果 API Token 泄露：

1. **立即撤销**
   - Cloudflare Dashboard > My Profile > API Tokens
   - 点击 Revoke

2. **生成新 Token**
   - 创建新的 API Token
   - 更新 `cloudflare.secret.toml`

3. **审计影响**
   - 检查 Cloudflare 审计日志
   - 确认无异常部署

4. **通知团队**
   - 如有可疑活动，立即通知

## 文件权限

设置适当的文件权限：

```bash
# 仅所有者可读写
chmod 600 cloudflare.secret.toml

# 验证
ls -l cloudflare.secret.toml
# -rw------- 1 user user ... cloudflare.secret.toml
```

## 检查清单

部署前检查：

- [ ] `cloudflare.secret.toml` 不在 Git 中
- [ ] API Token 权限最小化
- [ ] 文件权限设置为 600
- [ ] 密钥文件已备份
- [ ] 有轮换计划
