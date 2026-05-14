#!/bin/bash
# 本地测试服务器

echo "🚀 启动本地测试服务器..."
echo ""
echo "📋 测试文件:"
echo "  - test-page.html (测试页面)"
echo "  - test-iris.wasm (生成的 WASM)"
echo "  - keys/metadata.json (密钥元数据)"
echo ""
echo "🌐 访问地址:"
echo "  http://localhost:8080/test-page.html"
echo ""
echo "Press Ctrl+C to stop"
echo ""

python -m http.server 8080
