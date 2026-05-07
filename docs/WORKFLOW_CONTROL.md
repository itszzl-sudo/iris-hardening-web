# Workflow 启用/禁用指南

## 当前状态：禁用 ⏸️

### 已禁用的触发方式

```yaml
on:
  # ❌ 定时触发已禁用
  # schedule:
  #   - cron: '*/30 * * * *'
  
  # ✅ 手动触发可用
  workflow_dispatch:
  
  # ❌ 代码推送触发已禁用
  # push:
  #   branches: [main]
```

### 当前行为

- ✅ **手动触发可用** - 通过 GitHub UI 或 CLI 手动运行
- ❌ **定时触发禁用** - 不会自动运行
- ❌ **推送触发禁用** - 代码推送不会触发

## 启用方式

### 方式 1：启用定时触发

编辑 `.github/workflows/wasm-deploy.yml`，取消注释：

```yaml
on:
  schedule:
    - cron: '*/30 * * * *'  # 每 30 分钟
  
  workflow_dispatch:
```

### 方式 2：启用推送触发

```yaml
on:
  push:
    branches: [main]
    paths:
      - 'crates/iris-wasm/**'
      - 'crates/iris-wasm-gateway/**'
  
  workflow_dispatch:
```

### 方式 3：全部启用

```yaml
on:
  schedule:
    - cron: '*/30 * * * *'
  
  workflow_dispatch:
    inputs:
      force_rotation:
        description: 'Force key rotation'
        type: boolean
  
  push:
    branches: [main]
    paths:
      - 'crates/iris-wasm/**'
      - 'crates/iris-wasm-gateway/**'
```

## 手动触发方法

### GitHub UI

```
1. 进入仓库页面
2. 点击 Actions 标签
3. 选择 "WASM Build and Deploy"
4. 点击 "Run workflow"
5. 可选：勾选 "force_rotation"
6. 点击绿色 "Run workflow" 按钮
```

### GitHub CLI

```bash
# 正常运行
gh workflow run wasm-deploy.yml

# 强制轮换密钥
gh workflow run wasm-deploy.yml -f force_rotation=true

# 查看运行状态
gh run list --workflow=wasm-deploy.yml --limit 5
```

### 使用 iris-wasm-cli

```bash
cd crates/iris-wasm-gateway

# 手动生成 WASM
cargo run --bin iris-wasm-cli -- generate --output iris.wasm

# 手动部署
cargo run --bin iris-wasm-cli -- deploy

# 查看状态
cargo run --bin iris-wasm-cli -- status
```

## 禁用原因

常见禁用原因：

1. **测试阶段** - 先验证手动触发正常
2. **成本控制** - 减少构建次数
3. **开发调试** - 避免频繁自动构建
4. **维护窗口** - 临时停止自动部署

## 启用建议

### 测试完成后启用

```bash
# 1. 验证手动触发成功
gh run list --workflow=wasm-deploy.yml --limit 1

# 2. 检查部署成功
curl https://iris-wasm.pages.dev/iris.wasm

# 3. 启用自动触发
# 编辑 .github/workflows/wasm-deploy.yml
# 取消注释 schedule 和 push
```

### 生产环境建议配置

```yaml
on:
  schedule:
    - cron: '*/30 * * * *'    # 每 30 分钟检查
  
  workflow_dispatch:           # 保留手动触发
  
  push:
    branches: [main]           # 仅 main 分支
    paths:
      - 'crates/iris-wasm/**'
      - 'crates/iris-wasm-gateway/**'
```

### 开发环境建议配置

```yaml
on:
  workflow_dispatch:           # 仅手动触发
  
  pull_request:                # PR 触发预览
    branches: [main]
```

## 监控与验证

### 验证 workflow 是否启用

```bash
# 查看仓库 Actions 状态
gh api repos/{owner}/{repo}/actions/workflows

# 查看 workflow 配置
cat .github/workflows/wasm-deploy.yml | grep -A 10 "^on:"
```

### 查看运行历史

```bash
# 最近 10 次运行
gh run list --workflow=wasm-deploy.yml --limit 10

# 特定状态
gh run list --workflow=wasm-deploy.yml --status success
gh run list --workflow=wasm-deploy.yml --status failure
```

## 临时启用/禁用

### 临时禁用（不修改文件）

```
GitHub → Actions → WASM Build and Deploy
→ 点击右上角 "..." 
→ 选择 "Disable workflow"
```

### 重新启用

```
GitHub → Actions → WASM Build and Deploy
→ 点击右上角 "..." 
→ 选择 "Enable workflow"
```

## 最佳实践

### 推荐启用顺序

1. ✅ **阶段 1**：仅手动触发（当前状态）
   - 验证构建流程
   - 测试部署功能

2. ✅ **阶段 2**：启用推送触发
   ```yaml
   on:
     workflow_dispatch:
     push:
       branches: [main]
   ```

3. ✅ **阶段 3**：启用定时触发
   ```yaml
   on:
     schedule:
       - cron: '*/30 * * * *'
     workflow_dispatch:
     push:
       branches: [main]
   ```

### 安全检查

启用前确认：

- [ ] GitHub Secrets 已配置
- [ ] Cloudflare Pages 项目已创建
- [ ] 手动触发测试成功
- [ ] 部署结果验证通过
- [ ] 团队已知晓自动部署

## 当前配置总结

| 触发方式 | 状态 | 说明 |
|---------|------|------|
| 手动触发 | ✅ 启用 | 随时可手动运行 |
| 定时触发 | ❌ 禁用 | 注释掉 schedule |
| 推送触发 | ❌ 禁用 | 注释掉 push |

**需要启用时，取消对应配置的注释即可。**
