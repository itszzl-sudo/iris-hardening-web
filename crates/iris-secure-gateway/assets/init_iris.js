// Iris WASM Loader - 多源下载策略
// 下载顺序：当前服务器 -> tls.irisverse.org -> tls.irisverse.org:8443

let irisWasmModule = null;
let loadAttempts = 0;

const WASM_SOURCES = [
    './iris.wasm',                           // 当前服务器
    'https://tls.irisverse.org/iris.wasm',   // 主 HTTPS 源
    'https://tls.irisverse.org:8443/iris.wasm' // 备用端口源
];

async function loadIrisWasm() {
    if (irisWasmModule) {
        return irisWasmModule;
    }

    const errors = [];

    for (let i = 0; i < WASM_SOURCES.length; i++) {
        const source = WASM_SOURCES[i];
        loadAttempts = i + 1;

        try {
            console.log(`[Iris] 尝试从源 ${i + 1}/${WASM_SOURCES.length} 加载: ${source}`);

            const response = await fetch(source, {
                method: 'GET',
                cache: 'no-cache'
            });

            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }

            const wasmBuffer = await response.arrayBuffer();
            
            // 检测是否加密（AES-256-GCM 加密数据通常以特定字节开头）
            const view = new Uint8Array(wasmBuffer, 0, Math.min(16, wasmBuffer.byteLength));
            const isEncrypted = view[0] !== 0x00 && view[0] !== 0x0a; // WASM 魔数
            
            let wasmData = wasmBuffer;
            
            if (isEncrypted) {
                console.log('[Iris] 检测到加密数据，正在解密...');
                // 解密逻辑需要密钥
                if (window.irisKey) {
                    wasmData = await decryptWasm(wasmBuffer, window.irisKey);
                } else {
                    console.warn('[Iris] 未提供密钥，尝试直接加载加密数据');
                }
            }

            // 加载 WASM 模块
            const wasmModule = await WebAssembly.instantiate(wasmData);
            
            console.log(`[Iris] 成功从源 ${i + 1} 加载 WASM`);
            
            irisWasmModule = wasmModule;
            return wasmModule;

        } catch (error) {
            errors.push({ source, error: error.message });
            console.error(`[Iris] 源 ${i + 1} 加载失败:`, error.message);
        }
    }

    // 所有源都失败
    const errorMessage = errors.map((e, i) => 
        `源 ${i + 1} (${e.source}): ${e.error}`
    ).join('\n');
    
    throw new Error(`所有 WASM 源加载失败:\n${errorMessage}`);
}

async function decryptWasm(encryptedBuffer, key) {
    // AES-256-GCM 解密实现
    // 实际解密需要 iris-secure-gateway 的密钥
    const response = await fetch('/internal/decrypt', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            data: btoa(String.fromCharCode(...new Uint8Array(encryptedBuffer)))
        })
    });

    if (!response.ok) {
        throw new Error(`解密失败: ${response.status}`);
    }

    const result = await response.json();
    return Uint8Array.from(atob(result.data), c => c.charCodeAt(0)).buffer;
}

async function initIris(config = {}) {
    try {
        // 设置全局密钥（如果提供）
        if (config.key) {
            window.irisKey = config.key;
        }

        // 加载 WASM
        const wasmModule = await loadIrisWasm();
        
        // 初始化引擎
        if (wasmModule.instance && wasmModule.instance.exports) {
            const exports = wasmModule.instance.exports;
            
            if (exports.init) {
                exports.init();
            }
            
            console.log('[Iris] 引擎初始化完成');
            
            return {
                module: wasmModule,
                exports: exports,
                loadAttempts: loadAttempts
            };
        }
        
        return wasmModule;

    } catch (error) {
        console.error('[Iris] 初始化失败:', error);
        throw error;
    }
}

// 导出
if (typeof module !== 'undefined' && module.exports) {
    module.exports = { initIris, loadIrisWasm };
} else {
    window.initIris = initIris;
    window.loadIrisWasm = loadIrisWasm;
}
