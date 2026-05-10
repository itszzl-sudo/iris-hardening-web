// Iris WASM Loader - 零配置模式
//
// 通过检查 HTTP 响应头 X-Iris-Configured 来判断网关是否已配置:
// - X-Iris-Configured: true  → 加密模式，需要解密
// - X-Iris-Configured: false → 透明代理，直接使用
//
// 如果 Service Worker (iris-sw.js) 已注册，解密由 SW 自动处理，
// 此脚本仅负责 WASM 模块的加载和初始化。

var irisWasmModule = null;
var loadAttempts = 0;
var _isConfigured = null;  // null = unknown, true/false = known

/** Check if the gateway is configured by probing a response header */
async function checkIrisConfigured() {
    if (_isConfigured !== null) return _isConfigured;

    try {
        var resp = await fetch('/status', { method: 'GET', cache: 'no-cache' });
        var header = resp.headers.get('X-Iris-Configured');
        if (header === 'true') {
            _isConfigured = true;
        } else if (header === 'false') {
            _isConfigured = false;
        } else {
            // Fallback: check response body
            var data = await resp.json();
            _isConfigured = data.configured === true;
        }
        console.log('[Iris] Gateway configured:', _isConfigured);
    } catch (e) {
        console.warn('[Iris] Cannot determine gateway status, assuming not configured');
        _isConfigured = false;
    }
    return _isConfigured;
}

/** Check if Service Worker is controlling this page */
function hasServiceWorker() {
    return typeof navigator !== 'undefined' && navigator.serviceWorker && navigator.serviceWorker.controller;
}

/** Get WASM sources from IRIS_CONFIG if available */
function getWasmSources() {
    if (window.IRIS_CONFIG && window.IRIS_CONFIG.wasmUrl) {
        return [window.IRIS_CONFIG.wasmUrl];
    }
    return ['./iris.wasm'];
}

/** Decrypt data via the gateway's /internal/decrypt endpoint */
async function decryptViaGateway(encryptedBuffer) {
    var decryptUrl = (window.IRIS_CONFIG && window.IRIS_CONFIG.decryptUrl) || '/internal/decrypt';
    var base64 = btoa(String.fromCharCode.apply(null, new Uint8Array(encryptedBuffer)));

    var response = await fetch(decryptUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ data: base64 })
    });

    if (!response.ok) {
        throw new Error('Decryption failed: ' + response.status);
    }

    var result = await response.json();
    return Uint8Array.from(atob(result.data), function(c) { return c.charCodeAt(0); }).buffer;
}

async function loadIrisWasm() {
    if (irisWasmModule) {
        return irisWasmModule;
    }

    var sources = getWasmSources();
    var errors = [];

    for (var i = 0; i < sources.length; i++) {
        var source = sources[i];
        loadAttempts = i + 1;

        try {
            console.log('[Iris] Loading from source ' + (i + 1) + '/' + sources.length + ': ' + source);

            var response = await fetch(source, {
                method: 'GET',
                cache: 'no-cache'
            });

            if (!response.ok) {
                throw new Error('HTTP ' + response.status + ': ' + response.statusText);
            }

            var wasmBuffer = await response.arrayBuffer();

            // Check WASM magic number (0x00 0x61 0x73 0x6d)
            var view = new Uint8Array(wasmBuffer, 0, Math.min(4, wasmBuffer.byteLength));
            var isWasm = view.length >= 4 &&
                view[0] === 0x00 && view[1] === 0x61 &&
                view[2] === 0x73 && view[3] === 0x6d;

            if (isWasm) {
                var wasmModule = await WebAssembly.instantiate(wasmBuffer);
                console.log('[Iris] WASM loaded from source ' + (i + 1));
                irisWasmModule = wasmModule;
                return wasmModule;
            }

            // Not valid WASM. Check if we need to decrypt.
            var configured = await checkIrisConfigured();

            if (!configured) {
                // Gateway not configured - the response is just not WASM
                throw new Error('Response is not valid WASM and gateway is not configured');
            }

            // Gateway is configured - try decryption
            if (!hasServiceWorker()) {
                console.log('[Iris] Non-WASM data, trying decryption...');
                try {
                    var decrypted = await decryptViaGateway(wasmBuffer);
                    var wasmModule = await WebAssembly.instantiate(decrypted);
                    console.log('[Iris] Decrypted and loaded from source ' + (i + 1));
                    irisWasmModule = wasmModule;
                    return wasmModule;
                } catch (decErr) {
                    throw new Error('Decryption failed: ' + decErr.message);
                }
            }

            // SW is active but response still isn't WASM
            throw new Error('Response is not valid WASM even after SW processing');

        } catch (error) {
            errors.push({ source: source, error: error.message });
            console.error('[Iris] Source ' + (i + 1) + ' failed:', error.message);
        }
    }

    var errorMessage = errors.map(function(e, i) {
        return 'Source ' + (i + 1) + ' (' + e.source + '): ' + e.error;
    }).join('\n');

    throw new Error('All WASM sources failed:\n' + errorMessage);
}

async function initIris(config) {
    config = config || {};
    try {
        if (config.key) {
            window.irisKey = config.key;
        }

        // Check gateway status first
        var configured = await checkIrisConfigured();
        console.log('[Iris] Starting init, configured:', configured, 'SW:', hasServiceWorker());

        var wasmModule = await loadIrisWasm();

        if (wasmModule.instance && wasmModule.instance.exports) {
            var exports = wasmModule.instance.exports;
            if (exports.init) {
                exports.init();
            }

            var mode = configured ? 'encrypted' : 'transparent';
            if (hasServiceWorker()) mode += '+SW';
            console.log('[Iris] Engine initialized (' + mode + ')');

            return {
                module: wasmModule,
                exports: exports,
                loadAttempts: loadAttempts,
                configured: configured,
                serviceWorker: hasServiceWorker()
            };
        }

        return wasmModule;

    } catch (error) {
        console.error('[Iris] Init failed:', error);
        throw error;
    }
}

// Export
if (typeof module !== 'undefined' && module.exports) {
    module.exports = { initIris, loadIrisWasm, checkIrisConfigured };
} else {
    window.initIris = initIris;
    window.loadIrisWasm = loadIrisWasm;
    window.checkIrisConfigured = checkIrisConfigured;
}
