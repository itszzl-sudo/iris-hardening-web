# 🚀 快速开始

## 5 分钟快速部署

### 1️⃣ 配置 Secrets（2 分钟）

在 GitHub 仓库中添加两个 Secrets：

```
Settings → Secrets and variables → Actions → New repository secret
```

| Secret | 值 | 获取方式 |
|--------|---|---------|
| `CLOUDFLARE_API_TOKEN` | 你的 API Token | Cloudflare Dashboard → My Profile → API Tokens |
| `CLOUDFLARE_ACCOUNT_ID` | 你的 Account ID | Cloudflare Dashboard → 右上角 |

### 2️⃣ 创建 Pages 项目（1 分钟）

```bash
npm install -g wrangler
wrangler login
wrangler pages project create iris-wasm
```

### 3️⃣ 推送代码（1 分钟）

```bash
git add .
git commit -m "feat: implement GitHub Actions deployment"
git push
```

### 4️⃣ 手动触发部署（1 分钟）

```
GitHub → Actions → WASM Build and Deploy → Run workflow
```

✅ 完成！

---

## 📊 Workflow 触发方式

### 当前状态：仅手动触发 ⏸️

| 触发方式 | 状态 | 说明 |
|---------|------|------|
| 🖱️ 手动触发 | ✅ 启用 | 通过 GitHub UI 或 CLI |
| ⏰ 定时触发 | ❌ 禁用 | 已注释 |
| 📤 Git Push | ❌ 禁用 | 已注释 |

### 手动触发方法

**方法 1：GitHub UI**
```
Actions → WASM Build and Deploy → Run workflow → Run
```

**方法 2：GitHub CLI**
```bash
gh workflow run wasm-deploy.yml
```

**方法 3：强制轮换密钥**
```bash
gh workflow run wasm-deploy.yml -f force_rotation=true
```

### 启用自动触发

编辑 `.github/workflows/wasm-deploy.yml`，取消注释：

```yaml
on:
  schedule:
    - cron: '*/30 * * * *'  # 启用定时触发
  
  workflow_dispatch:
  
  push:
    branches: [main]        # 启用推送触发
```

详见：[Workflow 控制指南](WORKFLOW_CONTROL.md)

## 🔍 验证部署

**GitHub Actions:**
```
https://github.com/[username]/[repo]/actions
```

**Cloudflare Pages:**
```
https://iris-wasm.pages.dev/iris.wasm
```

## 📖 详细文档

- [完整实施指南](IMPLEMENTATION_GUIDE.md)
- [密钥混淆说明](KEY_OBFUSCATION.md)
- [配置安全指南](CONFIG_SECURITY.md)
- [方案对比分析](COMPARISON_B_VS_C.md)

## ⚡ 常用命令

```bash
# 强制轮换密钥
gh workflow run wasm-deploy.yml -f force_rotation=true

# 查看最近构建
gh run list --workflow=wasm-deploy.yml --limit 5

# 查看密钥状态
cat crates/iris-wasm-gateway/keys/metadata.json | jq .
```

## 🆘 遇到问题？

查看 [故障排查指南](IMPLEMENTATION_GUIDE.md#-故障排查)
