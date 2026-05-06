# Iris WASM Tests

## Rust 单元测试

运行 Rust 单元测试：

```bash
cd crates/iris-wasm
cargo test
```

测试覆盖：
- `RenderTile` 创建和状态管理
- `FragmentedRenderer` 初始化和配置
- 分片顺序打乱
- 渲染状态追踪
- 进度计算
- 重置功能

## JavaScript 端到端测试

### 准备工作

1. 构建 WASM：
```bash
cd crates/iris-wasm
wasm-pack build --target web --release
```

2. 启动测试服务器：
```bash
python -m http.server 8080
# 或
npx serve .
```

3. 打开浏览器：
```
http://localhost:8080/test-e2e.html
```

### 测试套件

#### 1. 分片渲染测试
- WASM 模块加载
- 分片初始化 (4×3=12 tiles)
- 随机打乱验证
- 分片数量正确性
- 渲染进度追踪
- 完成状态检查

#### 2. 水印裁剪测试
测试不同尺寸画布：
- 正常画布 (800×600)
- 小画布 (400×300)
- 极小画布 (200×150)
- 临界画布 (100×56)
- 超小画布 (50×30)

验证：
- 固定字体大小 (56px)
- 裁剪区域计算
- 不溢出画布

#### 3. 截图检测测试
- PrintScreen 键监听
- visibilitychange 监听
- 水印渲染验证

#### 4. 完整渲染流程测试
- 引擎初始化
- 分片逐个渲染
- 水印叠加
- 最终验证

### 手动测试

#### 测试分片渲染
点击画布重新渲染，观察分片顺序随机性。

#### 测试截图保护
按 `PrintScreen` 键，观察水印透明度变化。

#### 测试 WebGPU 监控
查看右上角监控面板：
- 检查次数
- 失败次数
- 渲染进度
- 健康状态

拖动、最小化、关闭面板测试。

## 测试指标

- **分片数量**: 12 tiles (4×3)
- **水印字体**: 56px 固定
- **正常透明度**: 0.008
- **截图透明度**: 0.15
- **渲染延迟**: 50-150ms/tile

## 预期结果

所有测试通过时：
```
总计: 25+
通过: 25+
失败: 0
```

## 故障排除

### WASM 加载失败
```bash
# 确保已构建
wasm-pack build --target web
```

### 测试服务器问题
```bash
# 使用不同的端口
python -m http.server 9000
```

### 浏览器兼容性
- Chrome 89+
- Firefox 89+
- Safari 15+

需要 WebGPU 支持。
