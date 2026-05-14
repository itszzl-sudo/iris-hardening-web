#!/bin/bash
# 独立测试脚本 - 不依赖 iris-engine

set -e

echo "========================================="
echo "  Iris WASM Gateway 独立测试"
echo "========================================="
echo ""

# 检查工作区配置
echo "📦 检查工作区配置..."
if grep -q '# "crates/iris-wasm"' Cargo.toml; then
    echo "✅ iris-wasm 已临时移除"
else
    echo "❌ 请先注释掉 Cargo.toml 中的 iris-wasm"
    exit 1
fi

echo ""
echo "🔨 编译检查..."
cd crates/iris-wasm-gateway

# 清理
echo "  清理缓存..."
cargo clean

# 检查编译
echo "  检查编译..."
cargo check --release

echo ""
echo "✅ 编译检查通过"
echo ""

# 运行单元测试
echo "🧪 运行单元测试..."
cargo test --lib 2>&1 | grep -E "test|passed|failed" || true

echo ""
echo "📊 测试密钥混淆功能..."

# 生成测试密钥
cargo run --release --bin iris-wasm-cli -- generate \
    --config config.example.toml \
    --output test-iris.wasm 2>&1 | tail -5

# 检查生成的文件
if [ -f "test-iris.wasm" ]; then
    SIZE=$(wc -c < test-iris.wasm)
    echo "✅ WASM 文件已生成: test-iris.wasm ($SIZE bytes)"
    
    # 显示文件内容（前 10 行）
    echo ""
    echo "📄 文件内容预览:"
    head -10 test-iris.wasm
else
    echo "❌ WASM 文件生成失败"
    exit 1
fi

# 检查密钥元数据
echo ""
echo "🔐 检查密钥元数据..."
if [ -f "keys/metadata.json" ]; then
    echo "✅ 密钥元数据已生成"
    cat keys/metadata.json | jq .
else
    echo "⚠️ 密钥元数据不存在（首次运行可能需要生成）"
fi

# 检查 CLI 功能
echo ""
echo "🖥️ 测试 CLI 功能..."
cargo run --release --bin iris-wasm-cli -- status 2>&1 | tail -10

echo ""
echo "========================================="
echo "  ✅ 独立测试完成"
echo "========================================="
echo ""
echo "下一步:"
echo "1. 推送代码到 GitHub"
echo "2. 手动触发 workflow 测试"
echo "   gh workflow run wasm-deploy.yml"
echo ""
echo "恢复 iris-wasm:"
echo "   编辑 Cargo.toml，取消注释 iris-wasm 成员"
