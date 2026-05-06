# Iris WASM

Iris Engine 的 WebAssembly 绑定，用于在浏览器中运行 iris-engine。

## 构建

```bash
# 安装 wasm-pack
cargo install wasm-pack

# 构建 WASM
build-wasm.bat
```

## 使用

构建后，`pkg/` 目录包含：
- `iris_wasm.js` - JavaScript 绑定
- `iris_wasm_bg.wasm` - WebAssembly 二进制

在 `index.html` 中引入并初始化：

```javascript
import init, { IrisEngineWasm, get_version } from './pkg/iris_wasm.js';

await init();
const engine = new IrisEngineWasm();
console.log('Version:', get_version());
await engine.render('canvas');
```

## API

### `IrisEngineWasm`

主引擎类。

#### 方法

- `new()` - 创建引擎实例
- `render(canvas_id: string)` - 渲染到指定 canvas
- `handle_event(type: string, data: string)` - 处理用户事件
- `is_initialized()` - 检查是否已初始化

### Functions

- `init()` - 初始化引擎
- `get_version()` - 获取版本号
- `create_engine()` - 创建引擎实例

## 架构

```
index.html
    │
    └─> iris_wasm.js (JS 绑定)
            │
            └─> iris_wasm_bg.wasm (WASM 二进制)
                    │
                    └─> iris-engine (Rust 引擎)
                            │
                            ├─> iris-core
                            ├─> iris-gpu
                            ├─> iris-layout
                            ├─> iris-dom
                            ├─> iris-js
                            └─> iris-sfc
```
