use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NginxContext {
    pub domain: String,
    pub nginx_port: u16,
    pub gateway_host: String,
    pub gateway_port: u16,
    pub wasm_validity_hours: i64,
}

pub struct NginxGenerator;

impl NginxGenerator {
    /// Generate nginx.conf for the secure gateway
    pub fn generate_config(ctx: &NginxContext) -> String {
        let wasm_validity = ctx.wasm_validity_hours;

        format!(r#"
# iris-secure-gateway nginx configuration
# Generated automatically - DO NOT EDIT MANUALLY

worker_processes auto;
error_log /var/log/nginx/error.log warn;
pid /var/run/nginx.pid;

events {{
    worker_connections 1024;
}}

http {{
    include /etc/nginx/mime.types;
    default_type application/octet-stream;

    log_format main '$remote_addr - $remote_user [$time_local] "$request" '
                    '$status $body_bytes_sent "$http_referer" '
                    '"$http_user_agent" "$http_x_forwarded_for"';

    access_log /var/log/nginx/access.log main;

    sendfile on;
    tcp_nopush on;
    tcp_nodelay on;
    keepalive_timeout 65;
    types_hash_max_size 2048;

    # Gzip compression
    gzip on;
    gzip_vary on;
    gzip_proxied any;
    gzip_comp_level 6;
    gzip_types text/plain text/css text/xml application/json application/javascript application/rss+xml application/atom+xml image/svg+xml;

    upstream iris_backend {{
        server {gateway_host}:{gateway_port};
    }}

    server {{
        listen {nginx_port};
        server_name {domain};

        # Security headers
        add_header X-Frame-Options "SAMEORIGIN" always;
        add_header X-Content-Type-Options "nosniff" always;
        add_header X-XSS-Protection "1; mode=block" always;

        # Static files location
        location /iris.wasm {{
            proxy_pass http://iris_backend;
            proxy_http_version 1.1;
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection "upgrade";
            proxy_set_header Host $host;
            proxy_cache_bypass $http_upgrade;

            # No caching for WASM (security)
            add_header Cache-Control "no-store, no-cache, must-revalidate";
            proxy_set_header X-Content-Type-Options "nosniff";
        }}

        # Config endpoint (provides route mapping and keys)
        location /api/config {{
            proxy_pass http://iris_backend;
            proxy_http_version 1.1;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            add_header Cache-Control "no-store";
        }}

        # Status endpoint for monitoring
        location /api/status {{
            proxy_pass http://iris_backend;
            proxy_http_version 1.1;
            proxy_set_header Host $host;
            add_header Cache-Control "no-store";
        }}

        # NJS module for dynamic configuration
        js_import iris_main from /etc/nginx/iris_main.js;
        js_set $iris_route iris_main.get_route;

        # Route-based proxy (dynamic)
        location ~ ^/protected/(?<route>.+)$ {{
            proxy_pass http://iris_backend/protected/$route;
            proxy_http_version 1.1;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Original-URI $request_uri;
            add_header X-Protected "true";
        }}

        # Default location
        location / {{
            proxy_pass http://iris_backend;
            proxy_http_version 1.1;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        }}

        # Health check endpoint
        location /health {{
            return 200 'OK';
            add_header Content-Type text/plain;
        }}

        # Periodic handler for proactive updates
        js_periodic iris_main.periodicHandler 10s;
    }}
}}
"#, 
            domain = ctx.domain,
            nginx_port = ctx.nginx_port,
            gateway_host = ctx.gateway_host,
            gateway_port = ctx.gateway_port
        )
    }

    /// Generate minimal njs script
    pub fn generate_njs_script(gateway_url: &str) -> String {
        format!(r#"
// iris_main.js - NJS script for Iris Secure Gateway
// Auto-generated configuration module

var UPDATE_MARGIN_SECONDS = 300;  // 5 minutes before expiry
var configCache = null;
var configExpiry = 0;
var gatewayUrl = "{gateway_url}";

// Load configuration from secure gateway
function loadConfig() {{
    var headers = {{}};
    headers["Accept"] = "application/json";

    var response = ngx.fetch(gatewayUrl + "/api/config", {{
        method: "GET",
        headers: headers,
        timeout: 5000
    }});

    if (response && response.ok) {{
        var data = response.json();
        if (data) {{
            configCache = data;
            // Calculate expiry (validity_hours from config)
            var validitySecs = (data.validity_hours || 24) * 3600;
            configExpiry = Date.now() + validitySecs * 1000;
        }}
    }}
}}

// Get remaining time until config expiry
function getRemainingSeconds() {{
    if (!configExpiry) return Infinity;
    return Math.floor((configExpiry - Date.now()) / 1000);
}}

// Proactive update when approaching expiry
function proactiveUpdate() {{
    var remaining = getRemainingSeconds();
    if (remaining < UPDATE_MARGIN_SECONDS) {{
        // Reload config before it expires
        loadConfig();
    }}
}}

// Periodic handler (called every 10 seconds by nginx)
function periodicHandler() {{
    proactiveUpdate();
}}

// Get route for request
function getRoute(r) {{
    var path = r.uri;
    if (configCache && configCache.routes) {{
        for (var route in configCache.routes) {{
            if (path.startsWith(route)) {{
                return configCache.routes[route];
            }}
        }}
    }}
    return null;
}}

// Initialize on first request
if (!configCache) {{
    loadConfig();
}}
"#, 
            gateway_url = gateway_url
        )
    }

    /// Generate njs script for embedded nginx
    pub fn generate_embedded_njs() -> String {
        r#"
// iris_main.js - Embedded NJS for Iris Secure Gateway
// This is the njs module loaded by nginx

var UPDATE_MARGIN_SECONDS = 300;
var configCache = null;
var configExpiry = 0;
var gatewayUrl = 'http://127.0.0.1:9001';

function loadConfig() {
    try {
        var response = ngx.fetch(gatewayUrl + '/api/config', {
            method: 'GET',
            headers: { 'Accept': 'application/json' },
            timeout: 5000
        });
        
        if (response && response.ok) {
            configCache = response.json();
            var validitySecs = (configCache.validity_hours || 24) * 3600;
            configExpiry = Date.now() + validitySecs * 1000;
            ngx.log(ngx.INFO, 'Iris config loaded, expires in ' + validitySecs + 's');
        }
    } catch (e) {
        ngx.log(ngx.WARN, 'Failed to load config: ' + e);
    }
}

function getRemainingSeconds() {
    if (!configExpiry) return Infinity;
    return Math.floor((configExpiry - Date.now()) / 1000);
}

function proactiveUpdate() {
    var remaining = getRemainingSeconds();
    if (remaining < UPDATE_MARGIN_SECONDS) {
        ngx.log(ngx.INFO, 'Config nearing expiry, reloading...');
        loadConfig();
    }
}

function periodicHandler() {
    proactiveUpdate();
}

function getRoute(r) {
    var path = r.uri;
    if (configCache && configCache.routes) {
        for (var route in configCache.routes) {
            if (path.startsWith(route)) {
                return configCache.routes[route];
            }
        }
    }
    return null;
}

// Auto-initialize
loadConfig();
"#.to_string()
    }
}