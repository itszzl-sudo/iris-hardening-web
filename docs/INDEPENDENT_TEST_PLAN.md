# 脱离 iris-engine 独立测试方案

## 问题分析

### 当前依赖关系

```
iris-hardening-web/
├── iris-wasm              ← 依赖 iris-engine (外部路径，不存在)
│   └── iris-engine (path = "../../../iris-repo/crates/iris-engine")
│
└── iris-wasm-gateway      ← 不依赖 iris-engine ✅ 可独立测试
    ├── key_obfuscation
    ├── wasm_generator
    ├── cloudflare
    ├── scheduler
    └── cli
```

### 编译错误

```
error: failed to read `C:\Users\a\Documents\codearts\iris\iris-repo\crates\iris-engine\Cargo.toml`
系统找不到指定的路径。 (os error 3)
```

**原因：** iris-engine 路径不存在，导致整个工作区无法编译。

---

## 解决方案

### 方案 A：临时移除 iris-wasm（推荐）

**优势：**
- ✅ 最简单快速
- ✅ iris-wasm-gateway 完全独立
- ✅ 不影响现有代码

**步骤：**

#### 1. 修改工作区配置

编辑 `Cargo.toml`：

```toml
[workspace]
members = [
  # "crates/iris-wasm",              # 临时注释掉
  "crates/iris-secure-gateway",
  "crates/iris-wasm-gateway",
]
resolver = "2"
```

#### 2. 独立测试 iris-wasm-gateway

```bash
cd crates/iris-wasm-gateway

# 编译
cargo build --release

# 运行测试
cargo test

# 运行 CLI
cargo run --bin iris-wasm-cli -- --help
```

#### 3. 恢复时取消注释

测试完成后，恢复 iris-wasm：
```toml
members = [
  "crates/iris-wasm",              # 恢复
  "crates/iris-secure-gateway",
  "crates/iris-wasm-gateway",
]
```

---

### 方案 B：创建 mock iris-engine

**优势：**
- ✅ 保留工作区结构
- ✅ 可以测试 iris-wasm 接口

**步骤：**

#### 1. 创建 mock 目录

```bash
mkdir -p crates/mock-iris-engine/src
```

#### 2. 创建 mock Cargo.toml

```toml
# crates/mock-iris-engine/Cargo.toml
[package]
name = "iris-engine"
version = "0.1.0"
edition = "2021"

[dependencies]
```

#### 3. 创建 mock 实现

```rust
// crates/mock-iris-engine/src/lib.rs

pub const VERSION: &str = "0.1.0-mock";

pub fn init() {
    // Mock implementation
}

pub fn render() {
    // Mock implementation
}
```

#### 4. 修改 iris-wasm 依赖

```toml
# crates/iris-wasm/Cargo.toml
[dependencies]
# iris-engine = { path = "../../../iris-repo/crates/iris-engine" }
iris-engine = { path = "../mock-iris-engine" }
```

---

### 方案 C：Feature Flag 控制

**优势：**
- ✅ 灵活切换
- ✅ 编译时控制

**步骤：**

#### 1. 添加 feature

```toml
# crates/iris-wasm/Cargo.toml
[features]
default = ["iris-engine-dep"]
iris-engine-dep = []

[dependencies]
iris-engine = { path = "../../../iris-repo/crates/iris-engine", optional = true }
```

#### 2. 条件编译

```rust
// crates/iris-wasm/src/lib.rs

#[cfg(feature = "iris-engine-dep")]
use iris_engine;

#[cfg(not(feature = "iris-engine-dep"))]
mod mock_iris_engine {
    pub const VERSION: &str = "0.1.0-mock";
    pub fn init() {}
}

#[cfg(feature = "iris-engine-dep")]
pub fn init() {
    iris_engine::init();
}

#[cfg(not(feature = "iris-engine-dep"))]
pub fn init() {
    mock_iris_engine::init();
}
```

#### 3. 无 iris-engine 编译

```bash
cargo build --no-default-features
```

---

### 方案 D：独立测试项目

**优势：**
- ✅ 完全隔离
- ✅ 不修改原项目

**步骤：**

#### 1. 创建测试工作区

```bash
mkdir iris-wasm-gateway-test
cd iris-wasm-gateway-test

# 创建 Cargo.toml
cat > Cargo.toml << EOF
[workspace]
members = ["iris-wasm-gateway"]
resolver = "2"
EOF

# 复制代码
cp -r ../iris-hardening-web/crates/iris-wasm-gateway .
```

#### 2. 独立编译测试

```bash
cargo build
cargo test
```

---

## 推荐方案：方案 A

### 原因

1. ✅ **最简单**：只需注释一行
2. ✅ **iris-wasm-gateway 已独立**：不依赖 iris-engine
3. ✅ **快速验证**：可以立即开始测试
4. ✅ **易恢复**：取消注释即可

### 实施步骤

#### Step 1: 修改工作区

```toml
# Cargo.toml
[workspace]
members = [
  # "crates/iris-wasm",              # 临时移除
  "crates/iris-secure-gateway",
  "crates/iris-wasm-gateway",
]
resolver = "2"
```

#### Step 2: 验证编译

```bash
# 清理缓存
cargo clean

# 编译整个工作区
cargo build --release

# 或仅编译 iris-wasm-gateway
cd crates/iris-wasm-gateway
cargo build --release
```

#### Step 3: 运行测试

```bash
# 运行单元测试
cargo test

# 测试密钥混淆
cargo test key_obfuscation

# 测试 WASM 生成
cargo run --bin iris-wasm-cli -- generate --output test.wasm

# 测试部署流程（模拟）
cargo run --bin iris-wasm-cli -- status
```

#### Step 4: GitHub Actions 测试

```bash
# 手动触发 workflow
gh workflow run wasm-deploy.yml

# 查看运行状态
gh run list --workflow=wasm-deploy.yml --limit 1
```

---

## 测试范围

### 可测试的功能

✅ **iris-wasm-gateway 核心功能**
- 密钥混淆生成
- 密钥重建验证
- WASM 生成器
- Cloudflare API 集成
- 调度器逻辑
- CLI 工具

✅ **GitHub Actions**
- Workflow 执行
- 密钥过期检查
- WASM 构建
- 部署流程

### 不可测试的功能

❌ **iris-wasm 相关**
- WebGL/WebGPU 渲染
- 实际 WASM 执行
- iris-engine 集成

---

## 测试清单

### 单元测试

```bash
cd crates/iris-wasm-gateway

# 密钥混淆测试
cargo test key_obfuscation
# - test_key_obfuscation
# - test_shard_xor
# - test_checksum
# - test_obfuscated_code_generation

# 配置测试
cargo test config
# - test_cloudflare_config_serialization

# 调度器测试
cargo test scheduler
# - test_scheduler_config
```

### 集成测试

```bash
# 1. 生成密钥
cargo run --bin iris-wasm-cli -- generate --output test.wasm
ls -lh test.wasm

# 2. 检查密钥状态
cargo run --bin iris-wasm-cli -- status

# 3. 验证密钥文件
cat keys/metadata.json | jq .

# 4. 验证 WASM 内容
head -20 test.wasm
```

### GitHub Actions 测试

```bash
# 推送代码
git add .
git commit -m "test: independent test without iris-engine"
git push

# 手动触发
gh workflow run wasm-deploy.yml

# 监控运行
gh run watch
```

---

## 恢复步骤

测试完成后，恢复 iris-wasm：

```toml
# Cargo.toml
[workspace]
members = [
  "crates/iris-wasm",              # 恢复
  "crates/iris-secure-gateway",
  "crates/iris-wasm-gateway",
]
resolver = "2"
```

确保 `iris-engine` 路径正确：
```bash
ls ../../../iris-repo/crates/iris-engine/Cargo.toml
```

---

## 文件变更

### 需要修改的文件

仅 1 个文件：

```diff
# Cargo.toml
[workspace]
members = [
-  "crates/iris-wasm",
+  # "crates/iris-wasm",              # 临时移除，测试 iris-wasm-gateway
  "crates/iris-secure-gateway",
  "crates/iris-wasm-gateway",
]
```

### 不需要修改的文件

- ❌ `crates/iris-wasm-gateway/Cargo.toml` - 已独立
- ❌ `crates/iris-secure-gateway/Cargo.toml` - 已独立
- ❌ 任何源代码文件

---

## 验证命令

### 编译验证

```bash
# 清理
cargo clean

# 检查依赖
cargo tree --depth 1

# 编译检查
cargo check --release

# 完整构建
cargo build --release
```

### 功能验证

```bash
# 测试所有功能
cargo test --all

# 运行 CLI
cargo run --bin iris-wasm-cli -- --help

# 检查 workflow
cat .github/workflows/wasm-deploy.yml
```

---

## 总结

**推荐：方案 A（临时移除 iris-wasm）**

**原因：**
1. ✅ 最简单：仅修改 1 行
2. ✅ iris-wasm-gateway 完全独立
3. ✅ 不影响测试目标功能
4. ✅ 易于恢复

**测试范围：**
- ✅ 密钥混淆与重建
- ✅ WASM 生成器
- ✅ Cloudflare 集成
- ✅ GitHub Actions
- ✅ 完整部署流程

**下一步：**
1. 注释 `Cargo.toml` 中的 iris-wasm
2. `cargo clean`
3. `cargo build --release`
4. 运行测试验证
