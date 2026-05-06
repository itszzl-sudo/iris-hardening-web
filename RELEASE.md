# Iris Hardening Web

Web 安全加固组件集合

## Crates

### iris-wasm
浏览器端 WASM 运行时，提供：
- 分片渲染（4×3 tiles）
- WebGPU hook 监控
- 水印保护
- 截图检测

### iris-secure-gateway
安全网关服务，提供：
- AES-256-GCM 文件/API 加密
- 路径转换和映射
- 内置资源服务
- 密钥自动更新

### iris-wasm-gateway
密钥管理服务，提供：
- 密钥对生成和轮换
- iris.wasm 分发
- 跨服务通知

## 构建

```bash
# 开发构建
cargo build

# 发布构建（包含 WASM）
build-release.bat
```

## 依赖

iris-wasm 依赖本地 iris-engine：
```toml
iris-engine = { path = "../../../iris-repo/crates/iris-engine" }
```

## 架构

```
浏览器 <---> iris-secure-gateway <---> iris-wasm-gateway
   |              |                        |
   v              v                        v
iris.wasm    加密文件/API            密钥管理
```
