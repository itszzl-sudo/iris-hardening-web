# GitHub Workflow 方案评估

## 方案对比

### 方案 A：iris-wasm-gateway 定时服务（现有）

**架构：**
```
iris-wasm-gateway (服务器)
    ↓ 定时检查（每 30 分钟）
    ↓ 生成 WASM + 混淆密钥
    ↓ 上传 Cloudflare Pages API
Cloudflare Pages
```

**优势：**
- ✅ 完全自主控制
- ✅ 实时响应，精确调度
- ✅ 可立即触发密钥轮换
- ✅ 灵活配置检查间隔
- ✅ 状态监控和 API 管理

**劣势：**
- ❌ 需要持续运行服务器
- ❌ 服务器运维成本
- ❌ 需要手动部署服务
- ❌ 需要管理 Cloudflare API Token
- ❌ 单点故障风险

**成本：**
- 服务器：$5-20/月（VPS/云服务器）
- 运维时间：每月 2-4 小时

---

### 方案 B：GitHub Actions Workflow（推荐）

**架构：**
```
GitHub Actions (定时触发)
    ↓ cron: 每 30 分钟检查
    ↓ 密钥即将过期？
    ↓ 构建 WASM + 混淆密钥
    ↓ 部署到 Cloudflare Pages
Cloudflare Pages
```

**优势：**
- ✅ 无需持续运行服务器
- ✅ GitHub Actions 免费额度充足
- ✅ 原生 Secrets 管理（安全）
- ✅ 构建日志可追溯
- ✅ 与 Git 仓库深度集成
- ✅ 无单点故障
- ✅ 多环境支持
- ✅ 社区 Actions 生态

**劣势：**
- ❌ 定时精度限制（cron 最小 5 分钟）
- ❌ 构建启动延迟（1-2 分钟）
- ❌ 依赖 GitHub 平台
- ❌ 大型构建可能超时（6 小时限制）

**成本：**
- GitHub Actions：免费（公开仓库）/ $0.008/分钟（私有仓库）
- Cloudflare Pages：免费额度充足
- 预估：$0-5/月

---

### 方案 C：Cloudflare Pages 原生 CI/CD

**架构：**
```
Git Push/定时
    ↓
Cloudflare Pages Build
    ↓ 运行构建命令
    ↓ 生成 WASM + 混淆密钥
    ↓ 自动部署
Cloudflare Pages
```

**优势：**
- ✅ 最简化架构
- ✅ Cloudflare 原生集成
- ✅ 全球 CDN 分发
- ✅ 免费
- ✅ 自动 HTTPS

**劣势：**
- ❌ 定时触发支持有限
- ❌ 构建环境限制
- ❌ 密钥管理不如 GitHub Secrets 方便
- ❌ 自定义构建步骤受限

---

## 推荐：方案 B（GitHub Actions）

### 实现方案

#### 1. Workflow 配置

`.github/workflows/wasm-deploy.yml`:

```yaml
name: WASM Deploy

on:
  schedule:
    - cron: '*/30 * * * *'  # 每 30 分钟
  workflow_dispatch:        # 手动触发
  push:
    branches: [main]
    paths:
      - 'crates/iris-wasm/**'

jobs:
  check-and-deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown

      - name: Install wasm-pack
        run: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

      - name: Check key expiry
        id: key-check
        run: |
          # 检查密钥是否即将过期
          EXPIRY=$(cat keys/metadata.json | jq -r '.current.expires_at')
          NOW=$(date -u +%Y-%m-%dT%H:%M:%SZ)
          THRESHOLD=$(date -u -d "+2 hours" +%Y-%m-%dT%H:%M:%SZ)
          
          if [[ "$EXPIRY" < "$THRESHOLD" ]]; then
            echo "need_rotation=true" >> $GITHUB_OUTPUT
          else
            echo "need_rotation=false" >> $GITHUB_OUTPUT
          fi

      - name: Generate WASM
        if: steps.key-check.outputs.need_rotation == 'true'
        run: |
          cd crates/iris-wasm-gateway
          cargo run --bin iris-wasm-cli -- generate \
            --config config.toml \
            --output iris.wasm

      - name: Deploy to Cloudflare Pages
        if: steps.key-check.outputs.need_rotation == 'true'
        uses: cloudflare/wrangler-action@v3
        with:
          apiToken: ${{ secrets.CLOUDFLARE_API_TOKEN }}
          accountId: ${{ secrets.CLOUDFLARE_ACCOUNT_ID }}
          command: pages deploy ./iris.wasm --project-name=iris-wasm

      - name: Notify on success
        if: success()
        run: |
          echo "WASM deployed successfully"
          # 可选：发送 Slack/邮件通知
```

#### 2. Secrets 配置

在 GitHub 仓库设置中添加：

```
Settings > Secrets and variables > Actions > New repository secret

- CLOUDFLARE_API_TOKEN
- CLOUDFLARE_ACCOUNT_ID
```

#### 3. 密钥持久化

使用 Git 存储密钥元数据：

```yaml
- name: Commit key metadata
  run: |
    git config user.name "GitHub Actions"
    git config user.email "actions@github.com"
    git add keys/metadata.json
    git commit -m "Update key metadata [skip ci]"
    git push
```

或使用 Artifacts/Cache：

```yaml
- name: Cache keys
  uses: actions/cache@v3
  with:
    path: keys/
    key: wasm-keys-${{ github.run_id }}
    restore-keys: wasm-keys-
```

---

## 详细对比

| 维度 | Gateway 服务 | GitHub Actions | Cloudflare Pages |
|------|-------------|----------------|-------------------|
| **成本** | $5-20/月 | $0-5/月 | 免费 |
| **运维** | 高（需管理服务器） | 低（只需配置） | 极低 |
| **可用性** | 单点故障 | 高可用 | 高可用 |
| **安全性** | 自行管理 Token | GitHub Secrets | Cloudflare Secrets |
| **灵活性** | 高 | 中 | 低 |
| **监控** | 需自建 | 内置日志 | 内置日志 |
| **触发方式** | 定时/手动 | 定时/事件/手动 | Git 事件 |
| **构建时间** | 实时 | 1-2 分钟启动 | 1-2 分钟启动 |
| **适用场景** | 企业生产 | 开源/中小项目 | 简单项目 |

---

## 迁移建议

### 阶段 1：试点（推荐）

```yaml
# 同时运行两种方案，验证 GitHub Actions
on:
  schedule:
    - cron: '0 */6 * * *'  # 每 6 小时试点
```

### 阶段 2：并行运行

- Gateway 服务继续运行
- GitHub Actions 作为备份
- 对比两种方案的输出

### 阶段 3：完全迁移

- 禁用 Gateway 定时任务
- GitHub Actions 全面接管
- 保留 Gateway 用于手动操作

---

## 实施路线图

### Week 1：准备工作
- [ ] 创建 `.github/workflows/wasm-deploy.yml`
- [ ] 配置 GitHub Secrets
- [ ] 测试手动触发
- [ ] 验证构建输出

### Week 2：并行运行
- [ ] 调整 cron 为每 6 小时
- [ ] 监控构建日志
- [ ] 对比两种方案结果
- [ ] 优化构建时间

### Week 3：增加频率
- [ ] 调整为每 2 小时
- [ ] 增加失败通知
- [ ] 添加健康检查

### Week 4：完全迁移
- [ ] 调整为每 30 分钟
- [ ] 停止 Gateway 服务
- [ ] 文档更新
- [ ] 团队培训

---

## 风险与应对

### 风险 1：GitHub Actions 延迟

**应对：**
- 提前生成时间设为 3-4 小时
- 添加 workflow_dispatch 手动触发
- 监控构建时间

### 风险 2：GitHub 平台故障

**应对：**
- 保留 Gateway 服务作为备份
- 文档化手动部署流程
- 准备离线构建脚本

### 风险 3：密钥同步

**应对：**
- 使用 Git 存储 key metadata
- 或使用外部存储（S3/R2）
- 添加校验步骤

---

## 结论

**推荐采用 GitHub Actions 方案**，理由：

1. ✅ **成本更低**：几乎免费
2. ✅ **运维更简单**：无需管理服务器
3. ✅ **更安全**：GitHub Secrets 管理
4. ✅ **可追溯**：完整的构建日志
5. ✅ **无单点故障**：GitHub 高可用

**适用场景：**
- ✅ 开源项目
- ✅ 中小型项目
- ✅ 团队协作项目
- ❌ 需要毫秒级响应（保留 Gateway）
- ❌ 完全离线环境（使用 Gateway）

**下一步行动：**
1. 创建 GitHub Workflow 配置
2. 配置 Secrets
3. 测试手动触发
4. 并行运行验证
5. 逐步迁移
