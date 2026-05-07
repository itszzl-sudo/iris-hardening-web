# GitHub Actions 快速实施指南

## 1. 配置 Secrets（必须）

在 GitHub 仓库中添加以下 Secrets：

```
仓库 → Settings → Secrets and variables → Actions → New repository secret
```

### 必需的 Secrets

| Secret 名称 | 说明 | 获取方式 |
|------------|------|---------|
| `CLOUDFLARE_API_TOKEN` | Cloudflare API Token | Dashboard → My Profile → API Tokens → Create Token |
| `CLOUDFLARE_ACCOUNT_ID` | Cloudflare Account ID | Dashboard → 右上角账户信息 |

### 可选的 Secrets

| Secret 名称 | 说明 |
|------------|------|
| `SLACK_WEBHOOK` | Slack 通知 Webhook URL |

## 2. 测试 Workflow

### 手动触发测试

```bash
# 在 GitHub 页面
Actions → WASM Build and Deploy → Run workflow → Run workflow
```

### 查看构建日志

```
Actions → 选择运行记录 → 查看详细日志
```

### 验证输出

检查以下内容：
- ✅ WASM 文件生成成功
- ✅ Cloudflare Pages 部署成功
- ✅ key metadata 已提交
- ✅ Artifact 已上传

## 3. 调整配置

### 修改检查间隔

编辑 `.github/workflows/wasm-deploy.yml`:

```yaml
on:
  schedule:
    - cron: '*/30 * * * *'  # 改为你需要的间隔
```

常用 cron 表达式：
```yaml
'*/15 * * * *'   # 每 15 分钟
'*/30 * * * *'   # 每 30 分钟
'0 * * * *'      # 每小时
'0 */6 * * *'    # 每 6 小时
'0 0 * * *'      # 每天 0 点
```

### 修改提前生成时间

编辑检查逻辑：

```bash
THRESHOLD_TS=$((NOW_TS + 7200))  # 提前 2 小时
```

可选值：
- `3600`：提前 1 小时
- `7200`：提前 2 小时（推荐）
- `14400`：提前 4 小时

## 4. 监控与通知

### 添加 Slack 通知

1. 创建 Slack Webhook
   ```
   Slack → Apps → Incoming Webhooks → Add to Slack
   ```

2. 添加 Secret
   ```
   Name: SLACK_WEBHOOK
   Value: https://hooks.slack.com/services/XXX/YYY/ZZZ
   ```

3. 修改 Workflow
   ```yaml
   - name: Notify on success
     if: success()
     run: |
       curl -X POST ${{ secrets.SLACK_WEBHOOK }} \
         -H 'Content-Type: application/json' \
         -d '{
           "text": "✅ WASM deployed successfully",
           "attachments": [{
             "fields": [
               {"title": "Run", "value": "${{ github.run_number }}", "short": true},
               {"title": "Key ID", "value": "${{ needs.check-keys.outputs.current_key_id }}", "short": true}
             ]
           }]
         }'
   ```

### 添加邮件通知

使用 GitHub Actions `dawidd6/action-send-mail`:

```yaml
- name: Send email notification
  uses: dawidd6/action-send-mail@v3
  if: failure()
  with:
    server_address: smtp.gmail.com
    server_port: 465
    username: ${{ secrets.EMAIL_USERNAME }}
    password: ${{ secrets.EMAIL_PASSWORD }}
    subject: WASM Deployment Failed
    to: team@example.com
    from: GitHub Actions
    body: |
      WASM deployment failed in run ${{ github.run_number }}
      Check: ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}
```

## 5. 故障排查

### Workflow 未触发

检查：
- [ ] cron 表达式是否正确
- [ ] workflow 文件在 `.github/workflows/` 目录
- [ ] 文件名以 `.yml` 或 `.yaml` 结尾
- [ ] 仓库设置允许 Actions 运行

### 密钥检查失败

检查：
- [ ] `keys/metadata.json` 格式正确
- [ ] jq 命令可用（Ubuntu 默认有）
- [ ] 时间格式正确（RFC3339）

### Cloudflare 部署失败

检查：
- [ ] `CLOUDFLARE_API_TOKEN` 正确
- [ ] `CLOUDFLARE_ACCOUNT_ID` 正确
- [ ] Token 有 Pages 权限
- [ ] 项目 `iris-wasm` 已创建

### 构建超时

优化：
- 使用缓存（已在 workflow 中）
- 减少 fetch-depth
- 拆分大型构建

## 6. 本地测试

在推送前本地测试：

```bash
# 安装 act（GitHub Actions 本地运行器）
brew install act  # macOS
# 或
curl https://raw.githubusercontent.com/nektos/act/master/install.sh | bash

# 运行 workflow
act -j build-and-deploy

# 使用特定 event
act -e .github/workflows/test-event.json
```

## 7. 迁移步骤

### 从 Gateway 服务迁移

1. **停止 Gateway 定时任务**
   ```bash
   # 停止服务
   systemctl stop iris-wasm-gateway
   # 或
   docker stop iris-wasm-gateway
   ```

2. **导出现有密钥**
   ```bash
   # 复制 keys 目录到仓库
   cp -r /path/to/gateway/keys ./keys/
   
   # 提交到 Git
   git add keys/
   git commit -m "Import existing keys"
   git push
   ```

3. **验证 GitHub Actions**
   - 手动触发一次 workflow
   - 确认密钥未轮换（仍在有效期内）
   - 检查部署成功

4. **启用定时触发**
   - 确认 workflow cron 配置正确
   - 监控第一次自动触发

5. **保留 Gateway 作为备份**
   - Gateway 可用于手动操作
   - 紧急情况可临时启用

## 8. 安全检查清单

部署前验证：

- [ ] Secrets 已正确配置
- [ ] 敏感文件在 .gitignore 中
- [ ] API Token 权限最小化
- [ ] workflow 文件无硬编码密钥
- [ ] 构建日志不泄露敏感信息

## 9. 成本估算

### GitHub Actions

| 项目 | 数量 | 单价 | 月成本 |
|------|------|------|--------|
| 构建分钟 | ~1000 分钟 | $0.008/分钟 | ~$8 |
| Storage | <1GB | $0.25/GB | ~$0.25 |
| **总计** | | | **~$8/月** |

**免费额度：**
- 公开仓库：无限制
- 私有仓库：2000 分钟/月

### Cloudflare Pages

- 免费：无限带宽
- 部署：不限次数

**总成本：$0-8/月**

## 10. 常见问题

**Q: 为什么使用 Git 存储密钥元数据？**
A: 保持状态一致性，方便审计和回滚

**Q: 如何强制轮换密钥？**
A: 手动触发 workflow，勾选 `force_rotation`

**Q: 构建时间太长怎么办？**
A: 优化缓存，使用 `actions/cache`，减少依赖

**Q: 如何回滚？**
A: 使用之前构建的 Artifact，手动部署

**Q: 支持 staging 环境吗？**
A: 支持，创建不同的 workflow 或使用环境变量

---

## 下一步

1. ✅ 配置 GitHub Secrets
2. ✅ 推送 workflow 文件
3. ✅ 手动触发测试
4. ✅ 验证部署成功
5. ✅ 启用定时触发
6. ✅ 监控首次自动构建
7. ✅ 迁移完成
