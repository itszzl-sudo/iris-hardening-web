# 密钥混淆隐藏机制

## 问题
iris-wasm 生成的 WASM 自带解密密钥用来解密文件，密钥以明文形式存储容易被静态分析提取。

## 解决方案

### 1. 密钥分片与XOR混淆

将32字节的密钥分成4个片段，每个片段使用随机生成的XOR掩码进行混淆：

```rust
// 原始密钥: [0, 1, 2, ..., 31]
// 分片 0: bytes[0..8]   XOR mask_0 -> obfuscated_0
// 分片 1: bytes[8..16]  XOR mask_1 -> obfuscated_1
// 分片 2: bytes[16..24] XOR mask_2 -> obfuscated_2
// 分片 3: bytes[24..32] XOR mask_3 -> obfuscated_3
```

每个分片包含：
- `index`: 分片序号（用于排序重组）
- `data`: 混淆后的数据
- `xor_mask`: XOR掩码

### 2. 校验和验证

计算密钥的哈希校验和，在运行时重建密钥后验证完整性：

```rust
fn calculate_checksum(key: &[u8]) -> u32 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish() as u32
}
```

### 3. 运行时重建

在 WASM 运行时动态重建密钥：

```javascript
function reconstructKey(shards, expectedChecksum) {
    const key = [];
    
    // 排序分片
    shards.sort((a, b) => a.index - b.index);
    
    // 干扰操作（混淆静态分析）
    const _dummy1 = (0x45 ^ 0x23);
    const _dummy2 = [0x12, 0x34, 0x56].reduce((a, b) => a + b, 0);
    
    // XOR还原
    for (const shard of shards) {
        const data = hexDecode(shard.data);
        const mask = hexDecode(shard.xor_mask);
        for (let i = 0; i < data.length; i++) {
            key.push(data[i] ^ mask[i]);
        }
    }
    
    // 更多干扰操作
    const _dummy3 = "decoy".length ^ 0xFF;
    
    // 校验和验证
    const checksum = calculateChecksum(key);
    if (checksum !== expectedChecksum) {
        throw new Error("Key validation failed");
    }
    
    return new Uint8Array(key);
}
```

### 4. 干扰代码

添加无意义的计算操作混淆静态分析器：

```javascript
const _dummy1 = (0x45 ^ 0x23);  // 0x66
const _dummy2 = [0x12, 0x34, 0x56].reduce((a, b) => a + b, 0);  // 0x9C
const _dummy3 = "decoy".length ^ 0xFF;  // 0xFB
```

## 安全特性

1. **分散存储**: 密钥不完整存储在任何位置
2. **XOR混淆**: 每个分片使用不同的随机掩码
3. **完整性验证**: 运行时校验防止篡改
4. **反静态分析**: 干扰代码增加逆向难度
5. **无明文密钥**: 配置中不包含原始密钥

## 实现文件

- `crates/iris-wasm-gateway/src/key_obfuscation.rs`: 密钥混淆核心实现
- `crates/iris-wasm-gateway/src/wasm_generator.rs`: WASM生成器（使用混淆密钥）
- `crates/iris-wasm-gateway/src/wasm_proxy.rs`: WASM代理模板（运行时重建）

## 测试

```bash
cargo test key_obfuscation -p iris-wasm-gateway
```

测试覆盖：
- 密钥混淆和还原
- XOR操作正确性
- 校验和验证
- 混淆代码生成

## 改进建议

1. **多层混淆**: 可以添加多层XOR或移位操作
2. **动态分片**: 随机选择分片数量和大小
3. **Web Crypto API**: 使用浏览器原生加密API增强安全性
4. **代码虚拟化**: 考虑使用JSFuck或其他混淆技术
5. **定时密钥轮换**: 配合密钥管理器定期更新
