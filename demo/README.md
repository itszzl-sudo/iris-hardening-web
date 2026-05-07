# 演示网站部署指南

## 📁 文件结构

```
demo/
├── index.html              # 主页
├── api/
│   ├── status.json         # 密钥状态API
│   ├── health.json         # 健康检查API
│   └── wasm                # WASM文件
└── assets/
    └── logo.svg            # Logo
```

---

## 🌐 部署到 Cloudflare Pages

### 方式 1：手动上传

```bash
# 安装 wrangler
npm install -g wrangler

# 登录
wrangler login

# 部署
wrangler pages deploy demo --project-name=hardening-irisverse
```

### 方式 2：GitHub Actions（推荐）

已在 `.github/workflows/wasm-deploy.yml` 中配置

```yaml
# 手动触发
gh workflow run wasm-deploy.yml

# 或推送代码自动部署
git add demo/
git commit -m "feat: update demo site"
git push
```

---

## 🔧 本地测试

```bash
# 方式 1: Python HTTP服务器
cd demo
python -m http.server 8080

# 方式 2: Node.js
npx serve demo

# 方式 3: 使用wrangler本地模拟
wrangler pages dev demo
```

访问：`http://localhost:8080`

---

## 🌍 域名配置

### Cloudflare Pages 自定义域名

1. 进入 Cloudflare Pages 项目设置
2. Custom domains → Add custom domain
3. 输入：`harding.irisverse.org`
4. 添加 DNS 记录（自动）

### DNS 配置

```
类型: CNAME
名称: harding
内容: [project-name].pages.dev
代理: 已启用（橙色云朵）
```

---

## 📊 API 端点

部署后可访问：

### GET /api/status

```bash
curl https://harding.irisverse.org/api/status
```

响应：
```json
{
  "key_id": "663750b1-8c19-47a3-89a7-ef51c35b288c",
  "algorithm": "aes-256-gcm",
  "shards": 4,
  "status": "active"
}
```

### GET /api/health

```bash
curl https://harding.irisverse.org/api/health
```

响应：
```json
{
  "status": "healthy",
  "version": "0.1.1"
}
```

### GET /api/wasm

```bash
curl https://harding.irisverse.org/api/wasm -o iris.wasm
```

---

## 🎨 自定义配置

### 修改域名

编辑 `demo/index.html`：

```html
<span class="domain">your-domain.com</span>
```

### 修改API地址

```javascript
const response = await fetch('https://your-domain.com/api/status');
```

### 更新密钥状态

编辑 `demo/api/status.json`：

```json
{
  "key_id": "新的密钥ID",
  "expires_at": "新的过期时间"
}
```

---

## ✅ 部署检查清单

### 部署前

- [ ] 检查所有文件已创建
- [ ] API JSON格式正确
- [ ] WASM文件已复制到 api/ 目录
- [ ] 域名DNS已配置

### 部署后验证

- [ ] 主页可访问
- [ ] Logo正常显示
- [ ] API返回正确数据
- [ ] WASM文件可下载
- [ ] HTTPS证书有效
- [ ] CDN缓存正常

### 功能测试

- [ ] 密钥状态显示正确
- [ ] 分片信息完整
- [ ] 测试按钮可用
- [ ] API调用成功

---

## 🚀 快速部署命令

```bash
# 1. 安装依赖
npm install -g wrangler

# 2. 登录Cloudflare
wrangler login

# 3. 创建项目（首次）
wrangler pages project create hardening-irisverse

# 4. 部署
wrangler pages deploy demo --project-name=hardening-irisverse

# 5. 添加域名（首次）
# 进入 Cloudflare Dashboard → Pages → hardening-irisverse → Custom domains
# 添加: harding.irisverse.org

# 6. 验证
curl https://harding.irisverse.org/api/health
```

---

## 🔄 更新部署

### 更新WASM

```bash
# 生成新WASM
./target/release/iris-wasm-cli generate --output demo/api/wasm

# 重新部署
wrangler pages deploy demo --project-name=hardening-irisverse
```

### 更新API数据

```bash
# 编辑数据
vim demo/api/status.json

# 部署
wrangler pages deploy demo --project-name=hardening-irisverse
```

---

## 📸 截图位置

部署成功后，访问以下页面截图：

### 主页

```
https://harding.irisverse.org/
```

**截图要点：**
- Logo和域名显示清晰
- 功能卡片完整
- 渐变背景美观

### 演示区域

```
https://harding.irisverse.org/#demo
```

**截图要点：**
- 密钥ID和状态清晰
- 分片信息完整
- 测试按钮可见

### API响应

```bash
# 使用浏览器开发者工具
curl https://harding.irisverse.org/api/status
```

---

## 🎯 预期效果

部署成功后应该看到：

1. ✅ 精美的渐变背景
2. ✅ Logo和域名 "harding.irisverse.org" 清晰可见
3. ✅ 密钥状态实时显示
4. ✅ 分片信息完整展示
5. ✅ 测试功能正常工作
6. ✅ API返回正确数据
7. ✅ WASM文件可下载

---

## 🆘 故障排查

### 页面无法访问

```bash
# 检查部署状态
wrangler pages deployment list --project-name=hardening-irisverse

# 查看日志
wrangler pages deployment tail
```

### API返回404

```bash
# 检查文件是否存在
ls demo/api/status.json
ls demo/api/health.json

# 检查文件权限
chmod 644 demo/api/*.json
```

### 域名无法解析

```bash
# 检查DNS
nslookup harding.irisverse.org

# 检查Cloudflare设置
# Dashboard → DNS → 确认CNAME记录
```

---

## 📝 注意事项

1. **API格式**: JSON文件必须为有效JSON格式
2. **CORS**: 静态文件默认无CORS问题
3. **缓存**: Cloudflare自动缓存静态资源
4. **HTTPS**: Pages自动提供SSL证书
5. **大小限制**: 单文件<25MB，总大小<10GB

---

**现在可以部署到 harding.irisverse.org！**
