/**
 * Iris Secure Gateway - NJS Script
 *
 * 功能:
 * 1. 与 iris-secure-gateway 交互获取路由映射和加密密钥
 * 2. 管理配置生命周期 (过期检查)
 * 3. 代理加密请求
 * 4. 主动更新配置 (在过期前拉取新配置)
 * 5. 预拉取新版本 WASM
 *
 * 使用方式:
 * - 在 nginx.conf 中: js_import main from /etc/nginx/njs/iris_main.js
 */

// 预更新配置
var config = null;
var configExpiry = 0;
var configVersion = '';
var routes = {};

// WASM 缓存
var wasmCache = null;
var wasmExpiry = 0;

// 预更新定时器 (秒)
var UPDATE_MARGIN_SECONDS = 300;  // 过期前 5 分钟开始更新
var UPDATE_CHECK_INTERVAL = 60;   // 每 60 秒检查一次

/**
 * 加载配置
 * 从 iris-secure-gateway 获取完整配置
 */
function loadConfig() {
    var response = ngx.fetch('http://127.0.0.1:8080/api/config', {
        method: 'GET',
        headers: {
            'Accept': 'application/json'
        }
    });

    if (response.status == 200) {
        var data = JSON.parse(response.body);
        config = data;
        configExpiry = data.key.expires_at;
        configVersion = data.version;
        routes = {};

        // 构建路由映射
        for (var i = 0; i < data.routes.length; i++) {
            var route = data.routes[i];
            routes[route.path] = route.encrypted_path;
        }

        ngx.log(ngx.INFO, 'Iris: Config loaded, version=' + configVersion + ', expires=' + configExpiry);
        return true;
    }

    ngx.log(ngx.ERR, 'Iris: Failed to load config, status=' + response.status);
    return false;
}

/**
 * 主动更新配置
 * 在过期前 UPDATE_MARGIN_SECONDS 秒开始主动获取新配置
 */
function proactiveUpdate() {
    var now = Math.floor(Date.now() / 1000);
    var remaining = configExpiry - now;

    // 如果剩余时间小于阈值，尝试获取新配置
    if (remaining < UPDATE_MARGIN_SECONDS) {
        ngx.log(ngx.INFO, 'Iris: Proactive update triggered. Remaining: ' + remaining + 's');

        // 加载新配置
        if (loadConfig()) {
            ngx.log(ngx.INFO, 'Iris: Proactive update successful. New version: ' + configVersion);

            // 预拉取新 WASM
            proactiveFetchWasm();
        } else {
            ngx.log(ngx.WARN, 'Iris: Proactive update failed');
        }
    }
}

/**
 * 预拉取新版本 WASM
 */
function proactiveFetchWasm() {
    var response = ngx.fetch('http://127.0.0.1:8080/iris.wasm', {
        method: 'GET',
        headers: {
            'Accept': 'application/wasm'
        }
    });

    if (response.status == 200) {
        // 注意: njs 中无法直接处理二进制响应
        // 这里记录更新事件，实际 WASM 拉取在请求时进行
        wasmExpiry = configExpiry;
        ngx.log(ngx.INFO, 'Iris: WASM pre-fetch initiated, expires=' + wasmExpiry);
    }
}

/**
 * 获取配置 JSON
 * 用于 nginx 变量注入
 */
function getConfig() {
    if (!config) {
        loadConfig();
    }
    return JSON.stringify(config || {});
}

/**
 * 检查配置是否过期
 */
function isConfigExpired() {
    if (!configExpiry) return true;
    var now = Math.floor(Date.now() / 1000);
    return now > configExpiry;
}

/**
 * 检查配置是否即将过期 (在阈值内)
 */
function isExpiringSoon() {
    if (!configExpiry) return true;
    var now = Math.floor(Date.now() / 1000);
    return (configExpiry - now) < UPDATE_MARGIN_SECONDS;
}

/**
 * 获取剩余有效期（秒）
 */
function getRemainingSeconds() {
    if (!configExpiry) return 0;
    var remaining = configExpiry - Math.floor(Date.now() / 1000);
    return remaining > 0 ? remaining : 0;
}

/**
 * 代理加密请求
 * @param {object} r - nginx request object
 */
function proxyRequest(r) {
    var path = r.uri;

    // 检查是否需要主动更新
    if (isExpiringSoon()) {
        proactiveUpdate();
    }

    // 检查配置是否已加载
    if (!config) {
        if (!loadConfig()) {
            r.return(503, '{"error": "Configuration unavailable"}');
            return;
        }
    }

    // 检查过期
    if (isConfigExpired()) {
        r.return(401, JSON.stringify({
            error: 'Configuration expired',
            expired_at: configExpiry,
            current_time: Math.floor(Date.now() / 1000)
        }));
        return;
    }

    // 查找路由映射
    var encryptedPath = routes[path];
    if (!encryptedPath) {
        r.return(404, JSON.stringify({
            error: 'Route not found',
            path: path
        }));
        return;
    }

    // 转发请求到后端
    r.subrequest('/internal/proxy' + encryptedPath, {
        method: r.method,
        body: r.request_body
    }, function(reply) {
        if (reply.status >= 200 && reply.status < 300) {
            r.return(reply.status, reply.response_body);
        } else {
            r.return(reply.status, reply.response_body || JSON.stringify({
                error: 'Backend error',
                status: reply.status
            }));
        }
    });
}

/**
 * 处理 WASM 配置请求
 * 返回包含密钥和路由的 WASM 配置
 */
function handleWasmConfig(r) {
    // 检查是否需要主动更新
    if (isExpiringSoon()) {
        proactiveUpdate();
    }

    // 设置 CORS 头
    r.headersOut['Content-Type'] = 'application/json';
    r.headersOut['Cache-Control'] = 'no-cache';

    if (!config) {
        loadConfig();
    }

    if (!config) {
        r.return(503, '{"error": "Configuration unavailable"}');
        return;
    }

    if (isConfigExpired()) {
        r.return(401, JSON.stringify({
            error: 'Configuration expired',
            expired_at: configExpiry
        }));
        return;
    }

    // 返回完整配置 (包含密钥)
    r.return(200, JSON.stringify({
        wasm_url: '/iris.wasm',
        key: config.key,
        routes: routes,
        expires_at: configExpiry,
        version: configVersion
    }));
}

/**
 * 处理 WASM 请求
 * 支持预拉取和缓存
 */
function handleWasmRequest(r) {
    // 检查本地缓存
    if (wasmCache && !isConfigExpired()) {
        r.return(200, wasmCache);
        return;
    }

    // 转发到后端获取新 WASM
    r.subrequest('/internal/wasm', function(reply) {
        if (reply.status == 200) {
            wasmCache = reply.response_body;
            wasmExpiry = configExpiry;
            r.return(200, reply.response_body);
        } else {
            r.return(reply.status, reply.response_body || '{"error": "WASM unavailable"}');
        }
    });
}

/**
 * 处理健康检查
 */
function handleHealth(r) {
    var status = isConfigExpired() ? 'expired' : (isExpiringSoon() ? 'expiring_soon' : 'active');
    var remaining = getRemainingSeconds();

    r.return(200, JSON.stringify({
        status: status,
        expires_at: configExpiry,
        remaining_seconds: remaining,
        version: configVersion,
        update_needed: isExpiringSoon()
    }));
}

/**
 * 刷新配置
 */
function refreshConfig(r) {
    loadConfig();
    proactiveFetchWasm();
    r.return(200, JSON.stringify({
        success: true,
        version: configVersion,
        expires_at: configExpiry,
        remaining_seconds: getRemainingSeconds()
    }));
}

/**
 * 获取配置状态
 */
function getConfigStatus(r) {
    var status = {
        loaded: config !== null,
        version: configVersion,
        expires_at: configExpiry,
        remaining_seconds: getRemainingSeconds(),
        is_expired: isConfigExpired(),
        is_expiring_soon: isExpiringSoon(),
        wasm_cached: wasmCache !== null,
        wasm_expires: wasmExpiry,
        routes_count: Object.keys(routes).length
    };

    r.return(200, JSON.stringify(status));
}

/**
 * 初始化模块
 * 主动加载一次配置
 */
function init() {
    loadConfig();
    // 启动定时检查 (在生产环境中应该使用 nginx 定时器)
    // 这里只是记录初始化
    ngx.log(ngx.INFO, 'Iris NJS module initialized, update margin: ' + UPDATE_MARGIN_SECONDS + 's');
}

/**
 * 定时检查函数
 * 由 nginx js_periodic 调用
 */
function periodicHandler() {
    proactiveUpdate();
    ngx.log(ngx.INFO, 'Iris: Periodic check completed, remaining: ' + getRemainingSeconds() + 's');
}

// 导出模块
export default {
    getConfig,
    isConfigExpired,
    isExpiringSoon,
    getRemainingSeconds,
    proxyRequest,
    handleWasmConfig,
    handleWasmRequest,
    handleHealth,
    refreshConfig,
    getConfigStatus,
    loadConfig,
    proactiveUpdate,
    proactiveFetchWasm,
    init,
    periodicHandler
};