/**
 * Iris Service Worker — Local AES-256-GCM decryption via Web Crypto API
 *
 * No HTTP round-trips for decryption. The key is in IRIS_CONFIG.k (hex).
 * The gateway rewrites HTML URIs to encrypted paths, so the browser
 * requests encrypted paths directly. This SW intercepts and decrypts locally.
 *
 * Supports:
 * - Full file decryption for images, documents
 * - Range request handling for audio/video (206 Partial Content)
 * - Key rotation notification via BroadcastChannel
 *
 * Flow:
 * 1. Page loads → gateway injects IRIS_CONFIG (with im, k) into HTML
 * 2. Bootstrap JS sends IRIS_CONFIG to SW via postMessage
 * 3. SW imports AES key via crypto.subtle.importKey
 * 4. On fetch: if path is in im → fetch encrypted bytes → decrypt locally → return
 */

// ── State ──────────────────────────────────────────────────────────────────
let IRIS_CONFIG = null;
let IS_CONFIGURED = false;
let AES_KEY = null;  // CryptoKey object
let DECRYPT_CACHE = new Map();  // path → { data: ArrayBuffer, timestamp: number }
const CACHE_MAX_ENTRIES = 50;
const CACHE_TTL_MS = 5 * 60 * 1000;  // 5 minutes

// BroadcastChannel for key rotation notifications
let bc = null;
try {
  bc = new BroadcastChannel('iris-key-rotation');
  bc.onmessage = function(event) {
    if (event.data && event.data.type === 'KEY_ROTATION' && event.data.config) {
      setConfig(event.data.config);
    }
  };
} catch (_) { /* BroadcastChannel not supported */ }

// Receive config from page (injected by gateway into HTML, sent via postMessage)
self.addEventListener('message', function(event) {
  if (event.data && event.data.type === 'IRIS_CONFIG' && event.data.config) {
    setConfig(event.data.config);
  }
});

async function setConfig(config) {
  IRIS_CONFIG = config;
  IS_CONFIGURED = true;

  // Import AES key from hex string
  if (config.k) {
    try {
      AES_KEY = await importAesKey(config.k);
      // Clear cache on key change (old cached data is invalid)
      DECRYPT_CACHE.clear();
      console.log('[Iris SW] AES key imported');
    } catch (e) {
      console.error('[Iris SW] Failed to import AES key:', e);
    }
  }
  console.log('[Iris SW] Config received, configured:', IS_CONFIGURED);
}

// ── Crypto ─────────────────────────────────────────────────────────────────

/** Import AES-256-GCM key from hex string into Web Crypto API */
async function importAesKey(hexStr) {
  var keyBytes = hexToBytes(hexStr);
  return crypto.subtle.importKey(
    'raw',
    keyBytes,
    { name: 'AES-GCM' },
    false,  // not extractable
    ['decrypt']
  );
}

/** Hex string → Uint8Array */
function hexToBytes(hex) {
  var bytes = new Uint8Array(hex.length / 2);
  for (var i = 0; i < hex.length; i += 2) {
    bytes[i / 2] = parseInt(hex.substr(i, 2), 16);
  }
  return bytes;
}

/** Decrypt AES-256-GCM data. Input: [12-byte nonce][ciphertext with 16-byte auth tag] */
async function decryptLocal(encryptedBytes) {
  if (!AES_KEY) throw new Error('AES key not imported');

  var data = new Uint8Array(encryptedBytes);
  if (data.length < 12) throw new Error('Data too short');

  // Use slice() on the Uint8Array (not .buffer) to handle offset views correctly
  var nonce = data.slice(0, 12).buffer;
  var ciphertext = data.slice(12).buffer;

  var decrypted = await crypto.subtle.decrypt(
    { name: 'AES-GCM', iv: nonce },
    AES_KEY,
    ciphertext
  );
  return decrypted;
}

// ── Cache ──────────────────────────────────────────────────────────────────

function getCached(path) {
  var entry = DECRYPT_CACHE.get(path);
  if (!entry) return null;
  if (Date.now() - entry.timestamp > CACHE_TTL_MS) {
    DECRYPT_CACHE.delete(path);
    return null;
  }
  return entry.data;
}

function setCache(path, data) {
  if (DECRYPT_CACHE.size >= CACHE_MAX_ENTRIES) {
    // Evict oldest entry
    var oldest = null;
    DECRYPT_CACHE.forEach(function(v, k) {
      if (!oldest || v.timestamp < oldest.timestamp) oldest = { key: k, timestamp: v.timestamp };
    });
    if (oldest) DECRYPT_CACHE.delete(oldest.key);
  }
  DECRYPT_CACHE.set(path, { data: data, timestamp: Date.now() });
}

// ── Helpers ────────────────────────────────────────────────────────────────

/** Check if a URL path is a known encrypted resource (key in im) */
function isEncryptedPath(urlPath, config) {
  if (!config || !config.im) return false;
  var clean = urlPath.replace(/^\//, '');
  return config.im.hasOwnProperty(clean);
}

/** Get content type for an encrypted path from im */
function contentTypeForEncrypted(encryptedPath, config) {
  if (!config || !config.im) return 'application/octet-stream';
  var clean = encryptedPath.replace(/^\//, '');
  return config.im[clean] || 'application/octet-stream';
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

/** Parse a Range header, returns {start, end} or null */
function parseRangeHeader(rangeHeader, totalLength) {
  if (!rangeHeader) return null;
  // Format: bytes=start-end or bytes=start-
  var match = rangeHeader.match(/^bytes=(\d+)-(\d*)$/);
  if (!match) return null;
  var start = parseInt(match[1], 10);
  var end = match[2] ? parseInt(match[2], 10) : totalLength - 1;
  if (start > end || start >= totalLength) return null;
  end = Math.min(end, totalLength - 1);
  return { start: start, end: end };
}

// ── Service Worker Events ──────────────────────────────────────────────────

self.addEventListener('install', function(event) {
  console.log('[Iris SW] Installing...');
  event.waitUntil(self.skipWaiting());
});

self.addEventListener('activate', function(event) {
  console.log('[Iris SW] Activating, configured:', IS_CONFIGURED);
  event.waitUntil(self.clients.claim());
  // Start periodic key version check
  startKeyVersionPoll();
});

// ── Key rotation polling ───────────────────────────────────────────────────
var lastKeyId = null;
var KEY_POLL_INTERVAL = 60 * 1000; // Check every 60 seconds

function startKeyVersionPoll() {
  setInterval(function() {
    if (!IS_CONFIGURED) return;
    fetch('/iris-key-version')
      .then(function(r) { return r.json(); })
      .then(function(data) {
        if (data && data.k && data.key_id && data.key_id !== lastKeyId) {
          if (lastKeyId !== null) {
            // Key has rotated — re-import
            console.log('[Iris SW] Key rotation detected, new key_id:', data.key_id);
            importAesKey(data.k).then(function(key) {
              AES_KEY = key;
              DECRYPT_CACHE.clear();
              if (IRIS_CONFIG) {
                IRIS_CONFIG.k = data.k;
              }
              // Notify other tabs via BroadcastChannel
              if (bc) {
                bc.postMessage({ type: 'KEY_ROTATION', config: IRIS_CONFIG });
              }
            }).catch(function(e) {
              console.error('[Iris SW] Failed to re-import rotated key:', e);
            });
          }
          lastKeyId = data.key_id;
        }
      })
      .catch(function() { /* ignore poll errors */ });
  }, KEY_POLL_INTERVAL);
}

self.addEventListener('fetch', function(event) {
  var url = new URL(event.request.url);

  // Only intercept same-origin requests
  if (url.origin !== self.location.origin) return;

  // Skip our own gateway endpoints
  var path = url.pathname;
  if (path === '/iris-sw.js') return;
  if (path === '/iris-bootstrap.js') return;
  if (path === '/iris-canvas.js') return;
  if (path === '/init_iris.js') return;
  if (path === '/health') return;
  if (path === '/status') return;
  if (path === '/iris-sw-config') return;
  if (path === '/iris-key-version') return;
  if (path.indexOf('/internal/') === 0) return;
  if (path === '/iris.wasm') return;

  // If not configured, pass through
  if (!IS_CONFIGURED) {
    event.respondWith(fetch(event.request));
    return;
  }

  // API proxy: pass through
  if (matchApiProxy(path, event.request.method, IRIS_CONFIG)) {
    event.respondWith(fetch(event.request));
    return;
  }

  // Encrypted resource: fetch → decrypt locally → return with correct Content-Type
  if (isEncryptedPath(path, IRIS_CONFIG) && AES_KEY) {
    event.respondWith(handleEncryptedRequest(event.request, path));
    return;
  }

  // All other requests: pass through
  event.respondWith(fetch(event.request));
});

/**
 * Handle an encrypted resource request.
 * Supports Range requests for audio/video seeking (206 Partial Content).
 */
async function handleEncryptedRequest(request, path) {
  var ct = contentTypeForEncrypted(path, IRIS_CONFIG);
  var isMedia = ct.startsWith('audio/') || ct.startsWith('video/');

  try {
    // Check cache first
    var cachedData = getCached(path);

    if (!cachedData) {
      // Fetch encrypted bytes from server
      var response = await fetch(request);
      if (!response.ok) return response;

      var encryptedBytes = await response.arrayBuffer();
      if (encryptedBytes.byteLength === 0) return response;

      // Decrypt locally
      var decryptedBytes = await decryptLocal(encryptedBytes);
      setCache(path, decryptedBytes);
      cachedData = decryptedBytes;
    }

    // Handle Range requests for media
    var rangeHeader = request.headers.get('Range');
    if (isMedia && rangeHeader) {
      var range = parseRangeHeader(rangeHeader, cachedData.byteLength);
      if (range) {
        var slice = cachedData.slice(range.start, range.end + 1);
        return new Response(slice, {
          status: 206,
          statusText: 'Partial Content',
          headers: {
            'Content-Type': ct,
            'Content-Range': 'bytes ' + range.start + '-' + range.end + '/' + cachedData.byteLength,
            'Content-Length': String(slice.byteLength),
            'Accept-Ranges': 'bytes',
            'Cache-Control': 'no-cache',
          },
        });
      }
    }

    // Full response
    var headers = {
      'Content-Type': ct,
      'Content-Length': String(cachedData.byteLength),
      'Cache-Control': 'no-cache',
    };
    if (isMedia) {
      headers['Accept-Ranges'] = 'bytes';
    }

    return new Response(cachedData, {
      status: 200,
      statusText: 'OK',
      headers: headers,
    });
  } catch (e) {
    console.error('[Iris SW] Decrypt error:', e);
    return fetch(request);
  }
}
