# Iris WASM Gateway

分发加密 WASM 代理并管理密钥轮换。

## 功能

- HTTP 服务提供 `iris.wasm` 下载
- 密钥对生成和管理
- 可配置密钥有效期
- 自动密钥轮换
- 通知 iris-secure-gateway 更新密钥

## 配置

`config.toml`:

```toml
[server]
host = "127.0.0.1"
port = 9090

[key]
key_dir = "keys"
validity_hours = 24
rotation_margin_hours = 2

[encrypt_service]
url = "http://127.0.0.1:8080"
update_key_endpoint = "/internal/update-key"
```

## 运行

```bash
iris-wasm-gateway config.toml
```

## API

### GET /iris.wasm

下载 WASM 代理

### GET /status

查看密钥状态

```json
{
  "status": "ok",
  "key_id": "...",
  "expires_at": "..."
}
```

## 密钥轮换流程

```
1. 检测密钥即将过期 (当前时间 >= 过期时间 - rotation_margin)
2. 生成新密钥对
3. 保存新密钥
4. 通知 iris-secure-gateway 更新密钥
5. 重新生成 iris.wasm (嵌入新密钥)
```

## iris.wasm 功能

浏览器端代理：
- 拦截请求
- 替换 URL 为加密地址
- 解密响应数据

```javascript
import init, { proxy_request, get_encrypted_url } from './iris.wasm';

await init(config);

// 自动代理
const response = await proxy_request(request);

// 手动转换
const encryptedUrl = get_encrypted_url('https://api.example.com/data');
```
