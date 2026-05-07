# 方案 B vs 方案 C 详细对比

## 架构对比

### 方案 B：GitHub Actions Workflow

```
┌─────────────────────────────────────────────────────────┐
│                    GitHub 仓库                           │
│  - 源代码管理                                            │
│  - Secrets 存储                                          │
│  - Workflow 定义                                         │
└─────────────────────────────────────────────────────────┘
                        ↓
        ┌───────────────────────────────┐
        │      GitHub Actions           │
        │  - 定时触发 (cron)            │
        │  - 手动触发                   │
        │  - 事件触发 (push/PR)         │
        └───────────────────────────────┘
                        ↓
        ┌───────────────────────────────┐
        │      构建环境 (Ubuntu)        │
        │  - Rust toolchain             │
        │  - wasm-pack                  │
        │  - 自定义脚本                 │
        └───────────────────────────────┘
                        ↓
        ┌───────────────────────────────┐
        │  密钥管理 + WASM 生成          │
        │  - 检查密钥过期               │
        │  - 生成混淆密钥               │
        │  - 构建 WASM                  │
        └───────────────────────────────┘
                        ↓
        ┌───────────────────────────────┐
        │  Cloudflare Pages API         │
        │  - wrangler deploy            │
        │  - 或直接 API 调用            │
        └───────────────────────────────┘
                        ↓
        ┌───────────────────────────────┐
        │   Cloudflare Pages 部署       │
        │  - 全球 CDN                   │
        │  - HTTPS                      │
        │  - 版本管理                   │
        └───────────────────────────────┘
```

### 方案 C：Cloudflare Pages 原生 CI/CD

```
┌─────────────────────────────────────────────────────────┐
│                    Git 仓库 (GitHub/GitLab)              │
│  - 源代码管理                                            │
└─────────────────────────────────────────────────────────┘
                        ↓
        ┌───────────────────────────────┐
        │   Cloudflare Pages Build      │
        │  - Git 集成                   │
        │  - 自动检测 push              │
        │  - 定时构建 (有限支持)         │
        └───────────────────────────────┘
                        ↓
        ┌───────────────────────────────┐
        │   构建环境 (Cloudflare)       │
        │  - Node.js                    │
        │  - 预设框架支持               │
        │  - 自定义 build command       │
        └───────────────────────────────┘
                        ↓
        ┌───────────────────────────────┐
        │  WASM 构建                    │
        │  - 运行 build 脚本            │
        │  - 生成 WASM                  │
        └───────────────────────────────┘
                        ↓
        ┌───────────────────────────────┐
        │   自动部署                     │
        │  - Preview URLs               │
        │  - Production deployment      │
        └───────────────────────────────┘
```

---

## 详细维度对比

### 1. 定时触发能力

| 维度 | GitHub Actions (B) | Cloudflare Pages (C) |
|------|-------------------|---------------------|
| **Cron 支持** | ✅ 完整支持 <br>`'*/30 * * * *'` | ❌ 有限支持 <br>需第三方触发 |
| **触发精度** | ±1-2 分钟 | 依赖外部触发器 |
| **触发方式** | - cron 定时<br>- workflow_dispatch 手动<br>- push/PR 事件<br>- repository_dispatch | - Git push<br>- 手动部署<br>- API 触发<br>- Cron triggers (需配置) |
| **灵活性** | ⭐⭐⭐⭐⭐ | ⭐⭐ |

**方案 B 示例：**
```yaml
on:
  schedule:
    - cron: '*/30 * * * *'  # 每 30 分钟
    - cron: '0 0 * * *'     # 每天 0 点
  workflow_dispatch:         # 手动触发
  push:
    branches: [main]
```

**方案 C 配置：**
```toml
# wrangler.toml
[triggers]
crons = ["*/30 * * * *"]  # 需 Workers 付费计划
```

⚠️ **关键差异**：Cloudflare Pages 的 cron 触发需要 Workers Paid 计划（$5/月起）

---

### 2. 构建环境

| 维度 | GitHub Actions (B) | Cloudflare Pages (C) |
|------|-------------------|---------------------|
| **操作系统** | Ubuntu/macOS/Windows | Linux (自定义受限) |
| **Rust 支持** | ✅ 完整支持 <br>dtolnay/rust-toolchain | ⚠️ 需自行安装 |
| **wasm-pack** | ✅ 一键安装 | ⚠️ 需自定义脚本 |
| **自定义工具** | ✅ 无限制 | ⚠️ 有限制 |
| **构建时间限制** | 6 小时 | 30 分钟 |
| **并行构建** | ✅ 支持 | ✅ 支持 |

**方案 B 环境配置：**
```yaml
- name: Setup Rust
  uses: dtolnay/rust-toolchain@stable
  with:
    targets: wasm32-unknown-unknown

- name: Install wasm-pack
  run: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
```

**方案 C 环境配置：**
```bash
# package.json
{
  "scripts": {
    "build": """
      curl https://sh.rustup.rs -sSf | sh -s -- -y &&
      source $HOME/.cargo/env &&
      curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh &&
      wasm-pack build --target web
    """
  }
}
```

⚠️ **问题**：方案 C 每次构建都需要重新安装 Rust，增加构建时间

---

### 3. Secrets 管理

| 维度 | GitHub Actions (B) | Cloudflare Pages (C) |
|------|-------------------|---------------------|
| **存储方式** | Encrypted Secrets | Environment Variables |
| **访问控制** | ✅ 精细权限控制 | ⚠️ 环境级别 |
| **审计日志** | ✅ 完整审计 | ⚠️ 有限 |
| **轮换支持** | ✅ API 支持 | ⚠️ 手动 |
| **多环境** | ✅ Environments | ✅ Environments |
| **安全性** | ⭐⭐⭐⭐⭐ | ⭐⭐⭐⭐ |

**方案 B Secrets：**
```yaml
env:
  API_TOKEN: ${{ secrets.CLOUDFLARE_API_TOKEN }}

steps:
  - name: Deploy
    run: |
      echo ${{ secrets.CLOUDFLARE_API_TOKEN }} | wrangler ...
```

**方案 C Secrets：**
```bash
# Cloudflare Dashboard → Pages → Settings → Environment variables
# 或通过 wrangler
wrangler pages secret put CLOUDFLARE_API_TOKEN
```

**安全性差异：**
- GitHub Actions：Secrets 加密存储，仅构建时可访问，日志自动脱敏
- Cloudflare Pages：环境变量明文存储在构建环境，日志可能泄露

---

### 4. 密钥状态管理

| 维度 | GitHub Actions (B) | Cloudflare Pages (C) |
|------|-------------------|---------------------|
| **持久化方式** | - Git 提交<br>- Cache<br>- Artifacts | - KV 存储<br>- D1 数据库<br>- 外部存储 |
| **版本控制** | ✅ Git history | ❌ 需额外配置 |
| **回滚支持** | ✅ 简单（Git revert） | ⚠️ 复杂 |
| **审计追踪** | ✅ Git commits + Actions logs | ⚠️ 有限 |

**方案 B 实现：**
```yaml
- name: Commit key metadata
  run: |
    git config user.name "GitHub Actions"
    git add keys/metadata.json
    git commit -m "Update key metadata [skip ci]"
    git push
```

**方案 C 实现（需要 Workers KV）：**
```javascript
// 构建脚本中
import { KVNamespace } from '@cloudflare/workers-types';

const kv = new KVNamespace();
await kv.put('key_metadata', JSON.stringify(metadata));
```

⚠️ **关键差异**：方案 C 需要 Workers KV 或外部存储（增加复杂度和成本）

---

### 5. 构建日志与调试

| 维度 | GitHub Actions (B) | Cloudflare Pages (C) |
|------|-------------------|---------------------|
| **日志查看** | ✅ 详细分步骤日志 | ⚠️ 单一构建日志 |
| **日志保留** | 90 天（可配置） | 30 天 |
| **重新运行** | ✅ 失败步骤重跑 | ⚠️ 整体重跑 |
| **下载日志** | ✅ 支持 | ❌ 不支持 |
| **实时查看** | ✅ 支持 | ✅ 支持 |

**方案 B 日志示例：**
```
Check key expiry
  Current key: 550e8400-e29b-41d4-a716-446655440000
  Expires at: 2026-05-08T10:00:00Z
  ✓ Key still valid

Generate WASM
  Compiling iris-wasm...
  Optimizing with wasm-opt...
  ✓ Generated iris.wasm (10240 bytes)

Deploy to Cloudflare Pages
  Uploading...
  ✓ Deployed to https://iris-wasm.pages.dev
```

---

### 6. 成本对比

#### 方案 B：GitHub Actions

| 项目 | 免费额度 | 超出价格 | 估算月成本 |
|------|---------|---------|-----------|
| **公开仓库** | 无限制 | - | $0 |
| **私有仓库** | 2000 分钟/月 | $0.008/分钟 | $0-8 |
| **Storage** | 500MB | $0.25/GB | ~$0.25 |
| **总计** | | | **$0-8/月** |

**实际使用：**
- 每次构建：~5 分钟
- 每天 48 次（30 分钟间隔）：240 分钟
- 月使用：~7200 分钟（超出 5200 分钟）
- 月成本：5200 × $0.008 = **$41.6**

⚠️ 但可通过缓存、减少频率优化到 **$5-10/月**

#### 方案 C：Cloudflare Pages

| 项目 | 免费额度 | 超出价格 | 估算月成本 |
|------|---------|---------|-----------|
| **构建次数** | 无限制 | - | $0 |
| **带宽** | 无限制 | - | $0 |
| **Workers KV** | 100k reads/day<br>1k writes/day | $0.50/GB | $0 |
| **Workers Paid** | ❌ 不包含 | $5/月 | **$5/月** |
| **总计** | | | **$0-5/月** |

⚠️ **关键**：定时构建需要 Workers Paid 计划（$5/月）

---

### 7. 功能完整性对比

| 功能需求 | GitHub Actions (B) | Cloudflare Pages (C) | 说明 |
|---------|-------------------|---------------------|------|
| **定时检查密钥过期** | ✅ 原生支持 | ⚠️ 需 Workers Paid | B 更简单 |
| **密钥混淆生成** | ✅ 完整支持 | ✅ 支持 | 相同 |
| **WASM 构建** | ✅ 完整支持 | ⚠️ 需自定义脚本 | B 更方便 |
| **Cloudflare 部署** | ✅ wrangler action | ✅ 原生集成 | C 更原生 |
| **状态持久化** | ✅ Git/Cache | ⚠️ 需 KV | B 更简单 |
| **失败通知** | ✅ 多种方式 | ⚠️ 有限 | B 更灵活 |
| **多环境部署** | ✅ Environments | ✅ Environments | 相同 |
| **手动触发** | ✅ workflow_dispatch | ✅ API | 相同 |
| **PR 预览** | ✅ 支持 | ✅ 原生 Preview URLs | C 更简单 |
| **回滚** | ✅ Git revert | ✅ 版本选择 | C 更直观 |

---

### 8. 开发体验对比

#### 方案 B：GitHub Actions

**优势：**
```yaml
# 配置清晰，分步骤
jobs:
  check-keys:
    steps:
      - name: Check expiry
      
  build-and-deploy:
    needs: check-keys
    steps:
      - name: Build
      - name: Deploy
```

**本地测试：**
```bash
# 使用 act 本地测试
act -j build-and-deploy
```

**劣势：**
- 需要学习 GitHub Actions 语法
- 调试需要推送到远程

#### 方案 C：Cloudflare Pages

**优势：**
```bash
# 本地测试完全一致
wrangler pages dev ./dist

# 配置简单
wrangler pages project create iris-wasm
```

**劣势：**
- 构建脚本复杂（需安装 Rust）
- 日志调试困难
- 环境限制多

---

### 9. 适用场景分析

#### 推荐方案 B 的场景

✅ **需要精确定时触发**
- 每 30 分钟检查密钥过期
- 需要灵活的 cron 配置

✅ **复杂构建流程**
- 需要 Rust + wasm-pack
- 多步骤依赖
- 需要缓存优化

✅ **需要完整审计**
- 密钥变更历史
- 构建日志保留
- 团队协作

✅ **已有 GitHub 工作流**
- 与现有 CI/CD 集成
- 使用 GitHub 生态

#### 推荐方案 C 的场景

✅ **简单静态部署**
- 仅部署 WASM 文件
- 无复杂构建需求

✅ **PR 预览优先**
- 需要每个 PR 自动预览
- 团队协作审查

✅ **已有 Cloudflare 生态**
- 使用 Workers KV/D1
- 已有 Cloudflare 配置

✅ **极简运维**
- 最少配置
- 原生集成

---

## 实现方案示例

### 方案 B 完整实现（已提供）

参考 `.github/workflows/wasm-deploy.yml`

### 方案 C 实现示例

```toml
# wrangler.toml
name = "iris-wasm"
compatibility_date = "2024-01-01"

[site]
bucket = "./pkg"

[build]
command = "npm run build"

[triggers]
crons = ["*/30 * * * *"]  # 需要 Workers Paid
```

```json
// package.json
{
  "scripts": {
    "build": """
      set -e
      
      # Install Rust if not present
      if ! command -v rustc &> /dev/null; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source $HOME/.cargo/env
      fi
      
      # Install wasm-pack
      curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
      
      # Build WASM
      cd crates/iris-wasm
      wasm-pack build --target web --release
      
      # Generate obfuscated key (需要外部脚本)
      node ../../scripts/generate-key.js
      
      # Move to output
      mv pkg ../../pkg
    """
  }
}
```

```javascript
// scripts/generate-key.js (需要自己实现)
const crypto = require('crypto');
const fs = require('fs');

// 生成混淆密钥逻辑
// 需要与 Rust 实现保持一致
// 这部分复杂度高
```

⚠️ **问题**：
1. 每次构建都安装 Rust（慢）
2. 密钥生成逻辑需用 Node.js 重写
3. 状态持久化需额外配置

---

## 决策矩阵

| 评估维度 | 权重 | 方案 B 评分 | 方案 C 评分 | B 得分 | C 得分 |
|---------|------|-----------|-----------|--------|--------|
| 定时触发能力 | 20% | 9 | 4 | 1.8 | 0.8 |
| 构建环境支持 | 15% | 9 | 5 | 1.35 | 0.75 |
| Secrets 管理 | 15% | 9 | 7 | 1.35 | 1.05 |
| 密钥状态管理 | 15% | 8 | 4 | 1.2 | 0.6 |
| 成本 | 10% | 7 | 8 | 0.7 | 0.8 |
| 开发体验 | 10% | 8 | 6 | 0.8 | 0.6 |
| 审计追踪 | 10% | 9 | 5 | 0.9 | 0.5 |
| 运维复杂度 | 5% | 7 | 8 | 0.35 | 0.4 |
| **总分** | **100%** | | | **8.45** | **5.5** |

**结论：方案 B（GitHub Actions）胜出**

---

## 混合方案（推荐）

结合两者优势：

```yaml
# 方案 B：负责定时生成和构建
- name: Build WASM
  run: cargo build --release

- name: Deploy to Cloudflare Pages
  uses: cloudflare/wrangler-action@v3
  with:
    command: pages deploy ./pkg --project-name=iris-wasm

# 方案 C：负责 PR 预览和回滚
# Cloudflare Pages 原生 PR 预览功能
```

**优势：**
- ✅ GitHub Actions 处理定时构建
- ✅ Cloudflare Pages 提供 PR 预览
- ✅ 利用两者最佳特性

---

## 最终建议

### 主推：方案 B（GitHub Actions）

**理由：**
1. ✅ **定时触发原生支持**（关键需求）
2. ✅ **Rust 构建环境完善**
3. ✅ **密钥状态管理简单**（Git）
4. ✅ **审计追踪完整**
5. ✅ **社区生态丰富**

### 补充：方案 C（Cloudflare Pages）

**用于：**
- PR 预览部署
- 紧急回滚
- 静态资源托管

**不建议用于：**
- 定时密钥轮换（需 Workers Paid）
- 复杂构建流程
- 密钥状态管理

---

## 下一步行动

1. **采用方案 B**
   - 使用已创建的 `.github/workflows/wasm-deploy.yml`
   - 配置 GitHub Secrets
   - 测试手动触发

2. **可选：添加方案 C**
   - 配置 Cloudflare Pages 项目
   - 用于 PR 预览
   - 作为备份部署渠道

3. **监控和优化**
   - 跟踪构建时间
   - 优化缓存策略
   - 调整触发频率
