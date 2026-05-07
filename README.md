# Iris Hardening Web

[English](#english) | [中文](#中文)

---

## English

### Introduction

Iris Hardening Web is a secure WebAssembly (WASM) gateway solution designed for modern web applications. It provides encrypted communication channels and secure WASM module execution environments with **automatic key rotation** and **obfuscated key protection**.

### Features

- **Secure WASM Execution**: Safe execution of WebAssembly modules with sandbox isolation
- **Encrypted Gateway**: End-to-end encryption for web communication
- **Key Obfuscation**: XOR-based key sharding and runtime reconstruction
- **Automatic Deployment**: GitHub Actions powered CI/CD with Cloudflare Pages
- **Key Rotation**: Scheduled key rotation every 24 hours (configurable)
- **High Performance**: Optimized Rust implementation with minimal overhead
- **Easy Integration**: Simple API for seamless integration into existing projects

### Architecture

The project consists of three main crates:

- **iris-wasm**: Core WASM execution and security layer
- **iris-secure-gateway**: Encrypted communication gateway
- **iris-wasm-gateway**: Integration layer with key management and deployment

### Quick Start

#### 1. Configure GitHub Secrets

```
Settings → Secrets and variables → Actions → New repository secret
```

| Secret | Description |
|--------|-------------|
| `CLOUDFLARE_API_TOKEN` | Cloudflare API Token (Pages - Edit permission) |
| `CLOUDFLARE_ACCOUNT_ID` | Cloudflare Account ID |

#### 2. Create Cloudflare Pages Project

```bash
npm install -g wrangler
wrangler login
wrangler pages project create iris-wasm
```

#### 3. Push and Deploy

```bash
git add .
git commit -m "feat: implement secure WASM deployment"
git push
```

The GitHub Actions workflow will automatically:
- ✅ Check key expiry every 30 minutes
- ✅ Generate obfuscated keys
- ✅ Build WASM module
- ✅ Deploy to Cloudflare Pages

### Manual Deployment

```bash
# Generate WASM with obfuscated key
cd crates/iris-wasm-gateway
cargo run --bin iris-wasm-cli -- generate --output iris.wasm

# Deploy to Cloudflare
cargo run --bin iris-wasm-cli -- deploy
```

### Installation

```bash
# Clone the repository
git clone https://github.com/itszzl-sudo/iris.git
cd iris/iris-hardening-web

# Build the project
cargo build --release
```

### Security

This project implements multiple security layers:

- **AES-GCM encryption** for data in transit
- **WASM sandboxing** for code isolation
- **Key obfuscation** with XOR sharding (4 shards)
- **Runtime key reconstruction** with checksum validation
- **GitHub Secrets** for sensitive configuration
- **Automatic key rotation** (24-hour default)

### Documentation

- [Quick Start Guide](docs/QUICKSTART.md)
- [Implementation Guide](docs/IMPLEMENTATION_GUIDE.md)
- [Key Obfuscation](docs/KEY_OBFUSCATION.md)
- [Configuration Security](docs/CONFIG_SECURITY.md)

### License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 中文

### 简介

Iris Hardening Web 是一个面向现代 Web 应用的安全 WebAssembly (WASM) 网关解决方案。它提供加密通信通道、安全的 WASM 模块执行环境，并支持**自动密钥轮换**和**混淆密钥保护**。

### 特性

- **安全 WASM 执行**：在沙箱隔离环境中安全执行 WebAssembly 模块
- **加密网关**：端到端加密的 Web 通信
- **密钥混淆**：基于 XOR 的密钥分片和运行时重建
- **自动部署**：GitHub Actions 驱动的 CI/CD，自动部署到 Cloudflare Pages
- **密钥轮换**：每 24 小时自动轮换密钥（可配置）
- **高性能**：优化的 Rust 实现，开销极低
- **易于集成**：简洁的 API，无缝集成到现有项目

### 架构

项目包含三个主要 crate：

- **iris-wasm**：核心 WASM 执行和安全层
- **iris-secure-gateway**：加密通信网关
- **iris-wasm-gateway**：集成层，包含密钥管理和部署功能

### 快速开始

#### 1. 配置 GitHub Secrets

```
Settings → Secrets and variables → Actions → New repository secret
```

| Secret | 说明 |
|--------|------|
| `CLOUDFLARE_API_TOKEN` | Cloudflare API Token（需要 Pages - Edit 权限） |
| `CLOUDFLARE_ACCOUNT_ID` | Cloudflare Account ID |

#### 2. 创建 Cloudflare Pages 项目

```bash
npm install -g wrangler
wrangler login
wrangler pages project create iris-wasm
```

#### 3. 推送代码并自动部署

```bash
git add .
git commit -m "feat: 实现安全 WASM 部署"
git push
```

GitHub Actions 工作流将自动执行：
- ✅ 每 30 分钟检查密钥过期
- ✅ 生成混淆密钥
- ✅ 构建 WASM 模块
- ✅ 部署到 Cloudflare Pages

### 手动部署

```bash
# 生成带混淆密钥的 WASM
cd crates/iris-wasm-gateway
cargo run --bin iris-wasm-cli -- generate --output iris.wasm

# 部署到 Cloudflare
cargo run --bin iris-wasm-cli -- deploy
```

### 安装

```bash
# 克隆仓库
git clone https://github.com/itszzl-sudo/iris.git
cd iris/iris-hardening-web

# 构建项目
cargo build --release
```

### 安全性

本项目实现了多层安全机制：

- **AES-GCM 加密**保护传输数据
- **WASM 沙箱**隔离代码执行
- **密钥混淆**使用 XOR 分片（4 片）
- **运行时密钥重建**带校验和验证
- **GitHub Secrets** 管理敏感配置
- **自动密钥轮换**（默认 24 小时）

### 文档

- [快速开始指南](docs/QUICKSTART.md)
- [完整实施指南](docs/IMPLEMENTATION_GUIDE.md)
- [密钥混淆说明](docs/KEY_OBFUSCATION.md)
- [配置安全指南](docs/CONFIG_SECURITY.md)

### 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。
