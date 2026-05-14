/**
 * Iris Service Worker - Zero-config transparent request hooking
 *
 * Flow:
 * 1. On install, fetch /iris-sw-config to check if gateway is configured
 * 2. Check X-Iris-Configured header on every response
 * 3. If NOT configured (X-Iris-Configured: false) → pass through, do nothing
 * 4. If configured (X-Iris-Configured: true) → auto-decrypt encrypted responses
 *
 * The init JS (iris-bootstrap.js) checks headers the same way.
 * If no SW support, init_iris.js falls back to manual mode.
 */

// ── State ──────────────────────────────────────────────────────────────────
let IRIS_CONFIG = null;
let CONFIG_LOADED = false;
let IS_CONFIGURED = false;  // Cached from X-Iris-Configured header

// ── Config fetch ───────────────────────────────────────────────────────────
async function loadConfig() {
  if (CONFIG_LOADED) return IRIS_CONFIG;
  try {
    const resp = await fetch('/iris-sw-config', { cache: 'no-store' });
    if (resp.ok) {
      IRIS_CONFIG = await resp.json();
      IS_CONFIGURED = IRIS_CONFIG.configured === true;
      CONFIG_LOADED = true;
      console.log('[Iris SW] Config loaded, configured:', IS_CONFIGURED);
    } else {
      console.warn('[Iris SW] Config fetch failed:', resp.status);
    }
  } catch (e) {
    console.warn('[Iris SW] Config fetch error:', e);
  }
  return IRIS_CONFIG;
}

/** Read X-Iris-Configured from a response header */
function checkConfiguredFromHeader(response) {
  var val = response.headers.get('X-Iris-Configured');
  if (val === 'true') {
    IS_CONFIGURED = true;
  } else if (val === 'false') {
    IS_CONFIGURED = false;
  }
  return IS_CONFIGURED;
}

// ── Helpers ────────────────────────────────────────────────────────────────

/** Check if a URL path matches a known encrypted file mapping */
function resolveEncryptedPath(urlPath, config) {
  if (!config || !config.fileMappings) return null;
  var path = urlPath.charAt(0) === '/' ? urlPath : new URL(urlPath, self.location.origin).pathname;
  var clean = path.replace(/^\//, '');
  return config.fileMappings[clean] || null;
}

/** Check if a URL path matches an API proxy pattern */
function matchApiProxy(urlPath, method, config) {
  if (!config || !config.apiPatterns || config.apiPatterns.length === 0) return false;
  var path = urlPath.charAt(0) === '/' ? urlPath : new URL(urlPath, self.location.origin).pathname;
  for (var i = 0; i < config.apiPatterns.length; i++) {
    var api = config.apiPatterns[i];
    try {
      var re = new RegExp(api.pattern);
      if (re.test(path) && api.methods.map(function(m) { return m.toUpperCase(); }).indexOf(method.toUpperCase()) >= 0) {
        return true;
      }
    } catch (_) { /* skip invalid regex */ }
  }
  return false;
}

/** Check if response might be encrypted (based on Content-Type) */
function looksEncryptable(contentType) {
  if (!contentType) return false;
  // Skip known clear-text types that are never encrypted
  if (contentType.indexOf('text/html') >= 0) return false;
  if (contentType.indexOf('application/javascript') >= 0) return false;
  if (contentType.indexOf('text/css') >= 0) return false;
  if (contentType.indexOf('application/json') >= 0) return false;
  if (contentType.indexOf('application/wasm') >= 0) return false;
  // Images might be encrypted
  if (contentType.indexOf('application/octet-stream') >= 0) return true;
  return false;
}

/** Decrypt data via the gateway's /internal/decrypt endpoint */
async function decryptViaGateway(encryptedBytes) {
  if (!IRIS_CONFIG || !IRIS_CONFIG.decryptUrl) {
    throw new Error('No decrypt URL configured');
  }
  var base64 = arrayBufferToBase64(encryptedBytes);
  var resp = await fetch(IRIS_CONFIG.decryptUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ data: base64 }),
  });
  if (!resp.ok) {
    throw new Error('Decrypt failed: ' + resp.status);
  }
  var result = await resp.json();
  return base64ToArrayBuffer(result.data);
}

function arrayBufferToBase64(buffer) {
  var bytes = new Uint8Array(buffer);
  var binary = '';
  for (var i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return self.btoa(binary);
}

function base64ToArrayBuffer(base64) {
  var binary = self.atob(base64);
  var bytes = new Uint8Array(binary.length);
  for (var i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes.buffer;
}

// ── Content type detection ─────────────────────────────────────────────────

function contentTypeFromPath(path) {
  var lower = path.toLowerCase();
  if (lower.endsWith('.html') || lower.endsWith('.htm')) return 'text/html; charset=utf-8';
  if (lower.endsWith('.js') || lower.endsWith('.mjs')) return 'application/javascript; charset=utf-8';
  if (lower.endsWith('.css')) return 'text/css; charset=utf-8';
  if (lower.endsWith('.wasm')) return 'application/wasm';
  if (lower.endsWith('.json')) return 'application/json; charset=utf-8';
  if (lower.endsWith('.png')) return 'image/png';
  if (lower.endsWith('.jpg') || lower.endsWith('.jpeg')) return 'image/jpeg';
  if (lower.endsWith('.gif')) return 'image/gif';
  if (lower.endsWith('.svg')) return 'image/svg+xml';
  if (lower.endsWith('.webp')) return 'image/webp';
  if (lower.endsWith('.woff2')) return 'font/woff2';
  if (lower.endsWith('.woff')) return 'font/woff';
  if (lower.endsWith('.ttf')) return 'font/ttf';
  if (lower.endsWith('.pdf')) return 'application/pdf';
  if (lower.endsWith('.txt')) return 'text/plain; charset=utf-8';
  return 'application/octet-stream';
}

// ── Service Worker Events ──────────────────────────────────────────────────

self.addEventListener('install', function(event) {
  console.log('[Iris SW] Installing...');
  event.waitUntil(loadConfig().then(function() { return self.skipWaiting(); }));
});

self.addEventListener('activate', function(event) {
  console.log('[Iris SW] Activating, configured:', IS_CONFIGURED);
  event.waitUntil(self.clients.claim());
});

self.addEventListener('fetch', function(event) {
  var url = new URL(event.request.url);

  // Only intercept same-origin requests
  if (url.origin !== self.location.origin) return;

  // Skip our own gateway endpoints to avoid infinite loops
  var path = url.pathname;
  if (path === '/iris-sw-config') return;
  if (path === '/iris-sw.js') return;
  if (path === '/iris-bootstrap.js') return;
  if (path === '/iris-canvas.js') return;
  if (path === '/init_iris.js') return;
  if (path === '/health') return;
  if (path === '/status') return;
  if (path.indexOf('/internal/') === 0) return;
  if (path === '/iris.wasm') return;

  // If not configured, just pass through - the gateway is transparent
  if (!IS_CONFIGURED) {
    // Still watch for X-Iris-Configured header on responses to detect
    // when the gateway gets configured (e.g. via API push)
    event.respondWith(
      fetch(event.request).then(function(response) {
        checkConfiguredFromHeader(response);
        return response;
      })
    );
    return;
  }

  // ── Configured mode: intercept and auto-decrypt ──

  var isEncryptedFile = resolveEncryptedPath(path, IRIS_CONFIG);
  var isApiProxy = matchApiProxy(path, event.request.method, IRIS_CONFIG);

  if (isApiProxy) {
    // API proxy: just pass through, gateway handles routing
    console.log('[Iris SW] API proxy:', path);
    event.respondWith(fetch(event.request));
    return;
  }

  if (isEncryptedFile) {
    // Known encrypted file: fetch from gateway, then decrypt
    console.log('[Iris SW] Decrypting:', path, '->', isEncryptedFile);
    event.respondWith(
      fetch(event.request)
        .then(function(response) {
          checkConfiguredFromHeader(response);
          if (!response.ok) return response;
          return response.arrayBuffer().then(function(encryptedBytes) {
            if (encryptedBytes.byteLength === 0) return response;
            return decryptViaGateway(encryptedBytes).then(function(decryptedBytes) {
              var ct = contentTypeFromPath(isEncryptedFile);
              var headers = new Headers(response.headers);
              headers.set('Content-Type', ct);
              headers.delete('Content-Length');
              return new Response(decryptedBytes, {
                status: response.status,
                statusText: response.statusText,
                headers: headers,
              });
            });
          });
        })
        .catch(function(e) {
          console.error('[Iris SW] Decrypt error:', e);
          return fetch(event.request);
        })
    );
    return;
  }

  // For other requests: pass through normally.
  // No automatic decryption for unknown paths - only mapped files are decrypted.
  event.respondWith(
    fetch(event.request).then(function(response) {
      checkConfiguredFromHeader(response);
      return response;
    })
  );
});
