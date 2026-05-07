# 方案 B 实施指南

## 🎯 实施目标

使用 GitHub Actions 实现：
1. ✅ 每 30 分钟自动检查密钥过期
2. ✅ 自动生成带混淆密钥的 WASM
3. ✅ 自动部署到 Cloudflare Pages
4. ✅ 密钥状态 Git 持久化
5. ✅ 完整的构建日志和审计追踪

## 📋 实施步骤

### Step 1: 配置 GitHub Secrets

**在 GitHub 仓库中添加 Secrets：**

```
仓库 → Settings → Secrets and variables → Actions → New repository secret
```

#### 必需的 Secrets

| Secret 名称 | 获取方式 | 说明 |
|------------|---------|------|
| `CLOUDFLARE_API_TOKEN` | Cloudflare Dashboard → My Profile → API Tokens → Create Token<br>权限：Pages - Edit | Cloudflare API 令牌 |
| `CLOUDFLARE_ACCOUNT_ID` | Cloudflare Dashboard → 右上角账户信息 | Cloudflare 账户 ID |

#### 可选的 Secrets

| Secret 名称 | 说明 |
|------------|------|
| `SLACK_WEBHOOK` | Slack 通知 Webhook URL |

### Step 2: 准备 Cloudflare Pages 项目

```bash
# 安装 wrangler
npm install -g wrangler

# 登录 Cloudflare
wrangler login

# 创建 Pages 项目
wrangler pages project create iris-wasm

# 验证项目创建成功
wrangler pages project list
```

### Step 3: 推送代码到 GitHub

```bash
# 确保 workflow 文件存在
ls -la .github/workflows/wasm-deploy.yml

# 提交所有更改
git add .
git commit -m "feat: implement GitHub Actions WASM deployment"
git push origin main
```

### Step 4: 手动触发测试

**在 GitHub 页面：**

```
Actions → WASM Build and Deploy → Run workflow
```

可选：
- 勾选 `force_rotation` 强制轮换密钥
- 点击 `Run workflow`

### Step 5: 验证构建

**检查构建日志：**

```
Actions → 选择运行记录 → 查看详细步骤
```

验证内容：
- ✅ `check-keys` job 成功
- ✅ `build-and-deploy` job 成功
- ✅ WASM 文件生成
- ✅ Cloudflare Pages 部署成功
- ✅ key metadata 已提交

### Step 6: 查看部署结果

**Cloudflare Pages Dashboard：**

```
https://dash.cloudflare.com/[account-id]/pages/view/iris-wasm
```

验证：
- ✅ 最新部署成功
- ✅ WASM 文件可访问：`https://iris-wasm.pages.dev/iris.wasm`

## 📁 文件清单

### 已创建的文件

```
iris-hardening-web/
├── .github/
│   └── workflows/
│       └── wasm-deploy.yml           # GitHub Actions 工作流
├── crates/iris-wasm-gateway/
│   ├── config.example.toml            # 配置示例
│   ├── cloudflare.example.toml        # Cloudflare 配置示例
│   ├── src/
│   │   ├── key_obfuscation.rs         # 密钥混淆实现
│   │   ├── wasm_generator.rs          # WASM 生成器
│   │   ├── cloudflare.rs              # Cloudflare API 集成
│   │   ├── scheduler.rs               # 调度器
│   │   └── cli.rs                     # 命令行工具
│   └── Cargo.toml                      # 依赖配置
└── docs/
    ├── KEY_OBFUSCATION.md              # 密钥混淆文档
    ├── SCHEDULER.md                    # 调度器文档
    ├── CONFIG_SECURITY.md              # 配置安全文档
    ├── GITHUB_WORKFLOW_EVALUATION.md   # 方案评估
    ├── COMPARISON_B_VS_C.md            # 方案对比
    └── IMPLEMENTATION_GUIDE.md         # 本文档
```

### 工作流功能

| 功能 | 触发方式 | 说明 |
|------|---------|------|
| **定时检查** | `cron: '*/30 * * * *'` | 每 30 分钟自动检查 |
| **手动触发** | `workflow_dispatch` | 支持强制轮换 |
| **代码推送** | `push: main` | 相关代码变更时触发 |

## 🔧 工作流程详解

### Job 1: check-keys

**功能：检查密钥是否需要轮换**

```
1. Checkout 代码
2. 恢复 keys cache
3. 检查 keys/metadata.json 是否存在
   - 不存在 → 需要生成
   - 存在 → 检查过期时间
4. 比较过期时间与阈值（当前时间 + 2小时）
5. 输出 need_rotation 和 current_key_id
```

### Job 2: build-and-deploy

**功能：构建 WASM 并部署**

```
1. Checkout 代码
2. 安装 Rust toolchain (wasm32-unknown-unknown)
3. 安装 wasm-pack
4. 恢复 cargo cache 和 keys cache
5. 构建 iris-wasm-gateway
6. 运行 iris-wasm-cli generate
   - 生成混淆密钥
   - 构建 WASM
7. 部署到 Cloudflare Pages
8. 保存 keys cache
9. 提交 key metadata 到 Git
10. 上传 WASM artifact
```

### Job 3: notify-failure

**功能：失败通知**

```
仅在 build-and-deploy 失败时运行
输出失败信息到 GITHUB_STEP_SUMMARY
```

## 📊 监控与日志

### 查看构建状态

**GitHub Actions Dashboard：**

```
https://github.com/[username]/[repo]/actions
```

**关键指标：**
- ✅ 成功率
- ⏱️ 平均构建时间
- 📈 构建频率

### 查看详细日志

**点击具体运行记录：**

```
Run #123
├── check-keys (2m 30s)
│   ├── Checkout
│   ├── Setup Rust
│   ├── Check key expiry ← 关键步骤
│   └── Force rotation check
├── build-and-deploy (8m 15s)
│   ├── Setup Rust
│   ├── Install wasm-pack
│   ├── Build iris-wasm-gateway
│   ├── Generate WASM ← 关键步骤
│   ├── Deploy to Cloudflare Pages
│   ├── Commit key metadata
│   └── Summary
└── notify-failure (skipped)
```

### 查看部署历史

**Cloudflare Pages：**

```
https://dash.cloudflare.com/[account-id]/pages/view/iris-wasm/deployments
```

**信息包括：**
- 部署时间
- 提交 SHA
- 状态（成功/失败）
- Preview URL

## 🔒 安全配置

### Secrets 权限

**限制 Secrets 访问：**

```
Settings → Secrets and variables → Actions → [Secret Name] → Update
→ Repository permissions
```

**推荐配置：**
- 仅允许 `main` 分支访问
- 仅允许特定 workflow 访问

### Branch Protection

**配置分支保护规则：**

```
Settings → Branches → Add branch protection rule
```

**推荐配置：**
- ✅ Require status checks to pass before merging
- ✅ Require branches to be up to date before merging
- ✅ Status check: `build-and-deploy`

## 🐛 故障排查

### 问题 1: Workflow 未触发

**症状：** 定时任务未运行

**检查：**
```bash
# 1. cron 表达式是否正确
'*/30 * * * *'  # 正确

# 2. workflow 文件位置是否正确
ls .github/workflows/wasm-deploy.yml

# 3. 仓库 Actions 是否启用
Settings → Actions → General → Allow all actions
```

**解决：**
- 修复 cron 表达式
- 确保 workflow 文件在正确位置
- 启用 Actions 权限

### 问题 2: 密钥检查失败

**症状：** `Check key expiry` 步骤失败

**检查：**
```bash
# 查看日志
Actions → [Run] → check-keys → Check key expiry

# 常见错误
- keys/metadata.json 格式错误
- jq 解析失败
- 时间格式不正确
```

**解决：**
- 检查 metadata.json 格式
- 安装 jq：`sudo apt-get install -y jq`
- 验证时间格式：RFC3339

### 问题 3: Rust 构建失败

**症状：** `Build iris-wasm-gateway` 失败

**检查：**
```bash
# 本地测试
cargo build --release

# 常见错误
- 依赖缺失
- 版本冲突
- wasm32 target 未安装
```

**解决：**
- 更新 Cargo.lock：`cargo update`
- 清理缓存：`cargo clean`
- 检查 Rust toolchain

### 问题 4: Cloudflare 部署失败

**症状：** `Deploy to Cloudflare Pages` 失败

**检查：**
```bash
# 验证 Secrets
Settings → Secrets → CLOUDFLARE_API_TOKEN
Settings → Secrets → CLOUDFLARE_ACCOUNT_ID

# 本地测试
wrangler pages deploy ./wasm --project-name=iris-wasm
```

**解决：**
- 重新生成 API Token
- 验证 Account ID
- 确保 Token 有 Pages 权限
- 创建 Pages 项目：`wrangler pages project create iris-wasm`

### 问题 5: Git 推送失败

**症状：** `Commit key metadata` 失败

**检查：**
```
错误信息：Permission to [repo] denied
```

**解决：**
```yaml
# 添加 token
- uses: actions/checkout@v4
  with:
    token: ${{ secrets.GITHUB_TOKEN }}  # 已添加
```

## 📈 性能优化

### 优化构建时间

**当前耗时：** ~10 分钟

**优化方案：**

1. **优化 Cache**
```yaml
# 使用更精确的 cache key
key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
```

2. **减少依赖**
```toml
# Cargo.toml
[profile.release]
opt-level = "z"      # 优化大小
lto = true           # Link-time optimization
codegen-units = 1    # 更好的优化
```

3. **使用预构建镜像**
```yaml
runs-on: ubuntu-latest
container:
  image: rust:latest  # 预装 Rust
```

### 优化触发频率

**调整 cron：**

```yaml
schedule:
  - cron: '0 * * * *'    # 每小时（推荐）
  # - cron: '*/30 * * * *'  # 每 30 分钟（当前）
```

**成本对比：**
- 每 30 分钟：~7200 分钟/月
- 每小时：~3600 分钟/月（节省 50%）

## 🔄 运维操作

### 强制轮换密钥

**方法 1：GitHub UI**

```
Actions → WASM Build and Deploy → Run workflow
→ force_rotation: true → Run workflow
```

**方法 2：GitHub CLI**

```bash
gh workflow run wasm-deploy.yml -f force_rotation=true
```

### 查看当前密钥状态

**方法 1：GitHub CLI**

```bash
gh run list --workflow=wasm-deploy.yml --limit 1
```

**方法 2：查看 Git 提交**

```bash
git log --oneline --grep="key metadata" -n 5
```

**方法 3：查看 keys/metadata.json**

```bash
cat crates/iris-wasm-gateway/keys/metadata.json | jq .
```

### 回滚到历史版本

**方法 1：Git Revert**

```bash
# 找到需要回滚的提交
git log --oneline --grep="key metadata" -n 10

# 回滚
git revert [commit-hash]
git push
```

**方法 2：Cloudflare Pages Dashboard**

```
https://dash.cloudflare.com/[account-id]/pages/view/iris-wasm/deployments
→ 选择历史部署 → Rollback to this deployment
```

## 📚 参考文档

### 内部文档
- [密钥混淆实现](KEY_OBFUSCATION.md)
- [调度器文档](SCHEDULER.md)
- [配置安全](CONFIG_SECURITY.md)
- [方案对比](COMPARISON_B_VS_C.md)

### 外部文档
- [GitHub Actions 文档](https://docs.github.com/en/actions)
- [Cloudflare Pages 文档](https://developers.cloudflare.com/pages/)
- [wasm-pack 文档](https://rustwasm.github.io/wasm-pack/)

## ✅ 实施检查清单

### 部署前检查

- [ ] GitHub Secrets 已配置
  - [ ] `CLOUDFLARE_API_TOKEN`
  - [ ] `CLOUDFLARE_ACCOUNT_ID`
- [ ] Cloudflare Pages 项目已创建
- [ ] workflow 文件已提交
- [ ] 手动触发测试成功
- [ ] 构建日志正常
- [ ] WASM 文件可访问

### 功能验证

- [ ] 定时触发工作正常
- [ ] 密钥过期检测正确
- [ ] WASM 构建成功
- [ ] Cloudflare 部署成功
- [ ] key metadata 已提交
- [ ] Artifact 已上传

### 安全检查

- [ ] Secrets 权限最小化
- [ ] workflow 无硬编码密钥
- [ ] 构建日志无敏感信息泄露
- [ ] .gitignore 配置正确

### 监控配置

- [ ] 失败通知已配置（可选）
- [ ] Branch protection 已配置（可选）
- [ ] 部署历史可追溯

## 🎉 完成确认

完成所有检查后，方案 B 实施成功！

**下一步：**
1. 监控首次自动触发
2. 优化构建时间
3. 配置失败通知（可选）
4. 团队培训文档

**支持：**
- GitHub Issues：[repo]/issues
- 文档：`docs/` 目录
