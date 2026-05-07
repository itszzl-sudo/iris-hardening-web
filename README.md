# Iris Hardening Web

[English](#english) | [中文](#中文)

---

## English

### Introduction

Iris Hardening Web is a secure WebAssembly (WASM) gateway solution designed for modern web applications. It provides encrypted communication channels and secure WASM module execution environments.

### Features

- **Secure WASM Execution**: Safe execution of WebAssembly modules with sandbox isolation
- **Encrypted Gateway**: End-to-end encryption for web communication
- **High Performance**: Optimized Rust implementation with minimal overhead
- **Easy Integration**: Simple API for seamless integration into existing projects

### Architecture

The project consists of three main crates:

- **iris-wasm**: Core WASM execution and security layer
- **iris-secure-gateway**: Encrypted communication gateway
- **iris-wasm-gateway**: Integration layer combining WASM and gateway services

### Installation

```bash
# Clone the repository
git clone https://github.com/itszzl-sudo/iris.git
cd iris/iris-hardening-web

# Build the project
cargo build --release
```

### Usage

```rust
use iris_wasm::WasmExecutor;
use iris_secure_gateway::SecureGateway;

// Initialize the secure gateway
let gateway = SecureGateway::new()?;

// Execute WASM module securely
let executor = WasmExecutor::new();
let result = executor.execute_secure(&wasm_bytes)?;
```

### Building for Web

```bash
# Build WASM package for web deployment
cargo build --target wasm32-unknown-unknown --release

# Or use the release script
./build-release.bat
```

### Security

This project implements multiple security layers:

- AES-GCM encryption for data in transit
- WASM sandboxing for code isolation
- Input validation and sanitization
- Secure random number generation

### Contributing

Contributions are welcome! Please feel free to submit pull requests.

### License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## 中文

### 简介

Iris Hardening Web 是一个面向现代 Web 应用的安全 WebAssembly (WASM) 网关解决方案。它提供加密通信通道和安全的 WASM 模块执行环境。

### 特性

- **安全 WASM 执行**：在沙箱隔离环境中安全执行 WebAssembly 模块
- **加密网关**：端到端加密的 Web 通信
- **高性能**：优化的 Rust 实现，开销极低
- **易于集成**：简洁的 API，无缝集成到现有项目

### 架构

项目包含三个主要 crate：

- **iris-wasm**：核心 WASM 执行和安全层
- **iris-secure-gateway**：加密通信网关
- **iris-wasm-gateway**：WASM 和网关服务的集成层

### 安装

```bash
# 克隆仓库
git clone https://github.com/itszzl-sudo/iris.git
cd iris/iris-hardening-web

# 构建项目
cargo build --release
```

### 使用

```rust
use iris_wasm::WasmExecutor;
use iris_secure_gateway::SecureGateway;

// 初始化安全网关
let gateway = SecureGateway::new()?;

// 安全执行 WASM 模块
let executor = WasmExecutor::new();
let result = executor.execute_secure(&wasm_bytes)?;
```

### 构建 Web 版本

```bash
# 构建用于 Web 部署的 WASM 包
cargo build --target wasm32-unknown-unknown --release

# 或使用发布脚本
./build-release.bat
```

### 安全性

本项目实现了多层安全机制：

- AES-GCM 加密保护传输数据
- WASM 沙箱隔离代码执行
- 输入验证和过滤
- 安全随机数生成

### 贡献

欢迎贡献代码！请随时提交 Pull Request。

### 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。
