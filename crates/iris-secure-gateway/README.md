# Iris Secure Gateway

安全网关，提供文件和 API 的加密访问代理。

## 功能

- 文件名加密映射
- 文件内容加密/解密
- API 请求/响应加密解密
- 路径转换
- HTTP API 请求转发
- 运行时密钥更新

## 配置

`config.toml`:

```toml
[server]
host = "127.0.0.1"
port = 8080
base_dir = "./data"

[encryption]
key_file = "key.txt"
algorithm = "aes-256-gcm"

[file_mappings]
"abc123" = "secret/document.pdf"
"def456" = "private/data.json"

[[api_routes]]
pattern = "^/api/v1/.*"
target = "http://localhost:3000"
methods = ["GET", "POST"]
```

## 密钥文件

`key.txt` (hex 编码 32 字节):

```
0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
```

生成新密钥:

```bash
openssl rand -hex 32 > key.txt
```

## 运行

```bash
iris-secure-gateway config.toml
```

## API

### 文件请求

`GET /abc123`
- 解析路径: `abc123 -> secret/document.pdf`
- 读取文件
- 加密内容并返回

### API 请求（支持加密）

`POST /api/v1/users`
- 正则匹配: `^/api/v1/.*`
- 加密请求 body
- 转发到: `http://localhost:3000/api/v1/users`
- 解密响应 body
- 返回解密后的响应

### 内部 API

#### 密钥更新

`POST /internal/update-key`

```json
{
  "key_id": "uuid",
  "key": "hex-encoded-32-bytes",
  "expires_at": "2024-01-02T00:00:00Z"
}
```

#### 数据解密

`POST /internal/decrypt`

```json
{
  "data": "base64-encoded-encrypted-data"
}
```

响应:
```json
{
  "data": "base64-encoded-decrypted-data"
}
```

#### 路径加密

`POST /internal/encrypt-path`

```json
{
  "path": "/secret/file.pdf"
}
```

响应:
```json
{
  "encrypted": "abc123"
}
```

#### 路径解密

`POST /internal/decrypt-path`

```json
{
  "encrypted": "abc123"
}
```

响应:
```json
{
  "path": "secret/file.pdf"
}
```

## 架构

```
Request
   │
   ├─> /internal/update-key ─> 更新密钥
   ├─> /internal/decrypt ─> 解密数据
   ├─> /internal/encrypt-path ─> 加密路径
   ├─> /internal/decrypt-path ─> 解密路径
   │
   ├─> API Route?
   │     └─> 加密请求 -> 转发 -> 解密响应 -> 返回
   │
   └─> File Mapping?
         └─> 读取文件 -> 加密 -> 返回
```

## 与 iris-wasm-gateway 集成

iris-wasm-gateway 会自动在密钥轮换时调用 `/internal/update-key` 更新此服务的密钥。
