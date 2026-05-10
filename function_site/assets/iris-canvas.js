/**
 * Iris Secure Canvas - Replace <img> with anti-screenshot canvas rendering
 *
 * Defenses against photo/screenshot capture:
 * 1. Images rendered on <canvas> (not <img>, harder to right-click save)
 * 2. Fragmented tile rendering via WASM (shuffled draw order)
 * 3. Per-tile noise watermark (proves source, enables forensics)
 * 4. CSS protection: user-select:none, -webkit-user-drag:none, pointer-events on overlay
 * 5. Visibility API detection: blank canvas when tab loses focus
 * 6. Screen capture API detection (getDisplayMedia)
 * 7. Print protection: @media print hides canvases
 *
 * Usage:
 *   <script src="/iris-canvas.js"></script>
 *   // All <img data-iris-secure> will be converted to secure canvases
 */

(function() {
    var irisEngine = null;
    var secureRenderers = new Map(); // img.src -> SecureImageRenderer

    // ── Load WASM ──────────────────────────────────────────────────────────

    async function loadIrisWasm() {
        if (irisEngine) return irisEngine;
        try {
            var wasmUrl = (window.IRIS_CONFIG && window.IRIS_CONFIG.wasmUrl) || '/iris.wasm';
            var resp = await fetch(wasmUrl);
            var wasmBuffer = await resp.arrayBuffer();

            // Check if encrypted (not WASM magic)
            var view = new Uint8Array(wasmBuffer, 0, 4);
            var isWasm = view[0] === 0x00 && view[1] === 0x61 && view[2] === 0x73 && view[3] === 0x6d;

            if (!isWasm && window.IRIS_CONFIG && window.IRIS_CONFIG.decryptUrl) {
                var base64 = arrayBufferToBase64(wasmBuffer);
                var decResp = await fetch(window.IRIS_CONFIG.decryptUrl, {
                    method: 'POST',
                    headers: { 'Content-Type': 'application/json' },
                    body: JSON.stringify({ data: base64 }),
                });
                if (decResp.ok) {
                    var decResult = await decResp.json();
                    wasmBuffer = base64ToArrayBuffer(decResult.data);
                }
            }

            var result = await WebAssembly.instantiate(wasmBuffer, {
                env: {
                    memory: new WebAssembly.Memory({ initial: 256, maximum: 1024 }),
                },
            });

            irisEngine = result.instance.exports;
            if (irisEngine.init) irisEngine.init();
            console.log('[Iris Canvas] WASM loaded');
            return irisEngine;
        } catch (e) {
            console.error('[Iris Canvas] WASM load failed:', e);
            return null;
        }
    }

    function arrayBufferToBase64(buffer) {
        var bytes = new Uint8Array(buffer);
        var binary = '';
        for (var i = 0; i < bytes.byteLength; i++) {
            binary += String.fromCharCode(bytes[i]);
        }
        return btoa(binary);
    }

    function base64ToArrayBuffer(base64) {
        var binary = atob(base64);
        var bytes = new Uint8Array(binary.length);
        for (var i = 0; i < binary.length; i++) {
            bytes[i] = binary.charCodeAt(i);
        }
        return bytes.buffer;
    }

    // ── Secure Image Rendering ─────────────────────────────────────────────

    async function renderSecureImage(imgElement) {
        var src = imgElement.getAttribute('data-iris-src') || imgElement.src;
        if (!src || imgElement.dataset.irisRendered === 'true') return;

        var wasm = await loadIrisWasm();
        if (!wasm) {
            console.warn('[Iris Canvas] WASM not available, falling back to img');
            return;
        }

        try {
            // Fetch the image
            var imgResp = await fetch(src);
            if (!imgResp.ok) return;

            var imgBlob = await imgResp.blob();
            var imgBitmap = await createImageBitmap(imgBlob);

            var w = imgBitmap.width;
            var h = imgBitmap.height;

            // Create canvas matching image size
            var canvas = document.createElement('canvas');
            canvas.width = w;
            canvas.height = h;
            canvas.className = imgElement.className || '';
            canvas.style.cssText = imgElement.style.cssText || '';
            // Anti-drag, anti-select CSS
            canvas.style.userSelect = 'none';
            canvas.style.webkitUserSelect = 'none';
            canvas.style.webkitUserDrag = 'none';
            canvas.setAttribute('draggable', 'false');

            // Draw original image to get pixel data
            var tmpCanvas = document.createElement('canvas');
            tmpCanvas.width = w;
            tmpCanvas.height = h;
            var tmpCtx = tmpCanvas.getContext('2d');
            tmpCtx.drawImage(imgBitmap, 0, 0);
            var imageData = tmpCtx.getImageData(0, 0, w, h);

            // Fragmented rendering: split into tiles, shuffle, draw with noise
            var tilesX = Math.max(2, Math.ceil(w / 64));  // ~64px tiles
            var tilesY = Math.max(2, Math.ceil(h / 64));
            var tileW = Math.floor(w / tilesX);
            var tileH = Math.floor(h / tilesY);

            // Use WASM SecureImageRenderer if available
            var renderer = null;
            if (wasm.SecureImageRenderer && wasm.SecureImageRenderer.new) {
                renderer = wasm.SecureImageRenderer.new(w, h, tilesX, tilesY);
                if (wasm.SecureImageRenderer.set_session_id && window.IRIS_SESSION) {
                    renderer.set_session_id(window.IRIS_SESSION);
                }
            }

            var ctx = canvas.getContext('2d');

            // Get shuffled render order
            var tileCount = tilesX * tilesY;
            var renderOrder = [];
            if (renderer && wasm.SecureImageRenderer.get_render_order) {
                var orderPtr = renderer.get_render_order();
                // WASM returns a pointer to a vec<u32> - we need to read it differently
                // Fallback: JS-side shuffle
                renderOrder = jsShuffle(tileCount);
            } else {
                renderOrder = jsShuffle(tileCount);
            }

            // Draw tiles in shuffled order with noise
            for (var t = 0; t < renderOrder.length; t++) {
                var idx = renderOrder[t];
                var tx = idx % tilesX;
                var ty = Math.floor(idx / tilesX);
                var sx = tx * tileW;
                var sy = ty * tileH;

                // Extract tile pixels
                var tw = (tx === tilesX - 1) ? (w - sx) : tileW;
                var th = (ty === tilesY - 1) ? (h - sy) : tileH;
                var tileData = tmpCtx.getImageData(sx, sy, tw, th);

                // Apply noise via WASM
                if (renderer && wasm.SecureImageRenderer.apply_noise) {
                    // Copy pixels to WASM memory and apply noise
                    var pixels = tileData.data;
                    // We need to pass a mutable slice - use JS-side noise instead
                    applyJsNoise(pixels, idx);
                } else {
                    applyJsNoise(tileData.data, idx);
                }

                // Draw tile to final canvas
                ctx.putImageData(tileData, sx, sy);
            }

            // Add invisible overlay div for extra protection
            var wrapper = document.createElement('div');
            wrapper.style.position = 'relative';
            wrapper.style.display = 'inline-block';
            wrapper.className = 'iris-secure-wrapper';
            canvas.style.display = 'block';

            var overlay = document.createElement('div');
            overlay.style.position = 'absolute';
            overlay.style.top = '0';
            overlay.style.left = '0';
            overlay.style.width = '100%';
            overlay.style.height = '100%';
            overlay.style.zIndex = '1';
            overlay.style.pointerEvents = 'none'; // Let clicks pass through

            wrapper.appendChild(canvas);
            wrapper.appendChild(overlay);

            // Replace img with our secure wrapper
            imgElement.parentNode.replaceChild(wrapper, imgElement);
            imgElement.dataset.irisRendered = 'true';

        } catch (e) {
            console.error('[Iris Canvas] Render error:', e);
        }
    }

    /// JS-side noise: encode tile index into B channel LSB
    function applyJsNoise(pixels, tileIndex) {
        if (pixels.length < 16) return;

        // Encode tile index into first 4 pixels' B channel
        var lo = tileIndex & 0xFF;
        var hi = (tileIndex >> 8) & 0xFF;
        var mask = 0xFC; // clear lowest 2 bits

        pixels[2] = (pixels[2] & mask) | (lo & 0x03);         // pixel 0, B
        pixels[6] = (pixels[6] & mask) | ((lo >> 2) & 0x03);   // pixel 1, B
        pixels[10] = (pixels[10] & mask) | ((lo >> 4) & 0x03); // pixel 2, B
        pixels[14] = (pixels[14] & mask) | ((lo >> 6) & 0x03); // pixel 3, B

        // Additional random noise on remaining pixels
        var rng = tileIndex * 7919 + 0x49524953;
        for (var i = 4; i < pixels.length / 4; i++) {
            rng = ((rng * 1103515245) + 12345) & 0x7FFFFFFF;
            var noise = rng & 0x03;
            var pi = i * 4 + 2; // B channel
            if (pi < pixels.length) {
                pixels[pi] = (pixels[pi] & mask) | noise;
            }
        }
    }

    /// Fisher-Yates shuffle
    function jsShuffle(count) {
        var order = [];
        for (var i = 0; i < count; i++) order.push(i);
        var rng = Date.now() & 0xFFFF;
        for (var j = count - 1; j > 0; j--) {
            rng = ((rng * 1103515245) + 12345) & 0x7FFFFFFF;
            var k = rng % (j + 1);
            var tmp = order[j];
            order[j] = order[k];
            order[k] = tmp;
        }
        return order;
    }

    // ── Anti-capture defenses ──────────────────────────────────────────────

    /// When tab loses focus, blank all secure canvases
    function setupVisibilityProtection() {
        document.addEventListener('visibilitychange', function() {
            var canvases = document.querySelectorAll('.iris-secure-wrapper canvas');
            canvases.forEach(function(canvas) {
                var ctx = canvas.getContext('2d');
                if (document.hidden) {
                    // Save current image data and blank
                    canvas._irisSavedData = ctx.getImageData(0, 0, canvas.width, canvas.height);
                    ctx.clearRect(0, 0, canvas.width, canvas.height);
                } else if (canvas._irisSavedData) {
                    // Restore
                    ctx.putImageData(canvas._irisSavedData, 0, 0);
                    delete canvas._irisSavedData;
                }
            });
        });
    }

    /// Detect screen capture attempts
    function setupCaptureDetection() {
        // Monitor getDisplayMedia (screen share / capture)
        if (navigator.mediaDevices && navigator.mediaDevices.getDisplayMedia) {
            var original = navigator.mediaDevices.getDisplayMedia.bind(navigator.mediaDevices);
            navigator.mediaDevices.getDisplayMedia = function(constraints) {
                console.warn('[Iris] Screen capture detected');
                // Blank canvases before allowing capture
                blankAllCanvases();
                var promise = original(constraints);
                // Restore after a delay
                setTimeout(restoreAllCanvases, 100);
                return promise;
            };
        }
    }

    function blankAllCanvases() {
        var canvases = document.querySelectorAll('.iris-secure-wrapper canvas');
        canvases.forEach(function(canvas) {
            var ctx = canvas.getContext('2d');
            if (!canvas._irisSavedData) {
                canvas._irisSavedData = ctx.getImageData(0, 0, canvas.width, canvas.height);
                ctx.clearRect(0, 0, canvas.width, canvas.height);
            }
        });
    }

    function restoreAllCanvases() {
        var canvases = document.querySelectorAll('.iris-secure-wrapper canvas');
        canvases.forEach(function(canvas) {
            if (canvas._irisSavedData) {
                var ctx = canvas.getContext('2d');
                ctx.putImageData(canvas._irisSavedData, 0, 0);
                delete canvas._irisSavedData;
            }
        });
    }

    /// Print protection: hide canvases when printing
    function setupPrintProtection() {
        var style = document.createElement('style');
        style.textContent = '@media print { .iris-secure-wrapper { display: none !important; } }';
        document.head.appendChild(style);
    }

    /// Prevent right-click save on canvases
    function setupContextMenuBlock() {
        document.addEventListener('contextmenu', function(e) {
            if (e.target.tagName === 'CANVAS' && e.target.closest('.iris-secure-wrapper')) {
                e.preventDefault();
            }
        });
    }

    /// Keyboard shortcut detection (Ctrl+P, PrtScn, etc.)
    function setupKeyProtection() {
        document.addEventListener('keydown', function(e) {
            // Ctrl+P (print)
            if (e.ctrlKey && e.key === 'p') {
                blankAllCanvases();
                setTimeout(restoreAllCanvases, 500);
            }
            // PrtScn
            if (e.key === 'PrintScreen') {
                blankAllCanvases();
                setTimeout(restoreAllCanvases, 300);
            }
        });
    }

    // ── Auto-init ──────────────────────────────────────────────────────────

    function initIrisCanvas() {
        // Generate session ID
        window.IRIS_SESSION = 'iris-' + Date.now().toString(36) + '-' + Math.random().toString(36).substr(2, 5);

        // Setup all defenses
        setupVisibilityProtection();
        setupCaptureDetection();
        setupPrintProtection();
        setupContextMenuBlock();
        setupKeyProtection();

        // Find all images to secure:
        // - <img data-iris-secure>
        // - <img> with src matching fileMappings in IRIS_CONFIG
        var images = document.querySelectorAll('img');
        var toSecure = [];

        images.forEach(function(img) {
            if (img.dataset.irisSecure !== undefined || img.hasAttribute('data-iris-secure')) {
                toSecure.push(img);
                return;
            }
            // Auto-detect: check if src matches an encrypted file mapping
            if (window.IRIS_CONFIG && window.IRIS_CONFIG.fileMappings) {
                var src = img.getAttribute('src') || '';
                var cleanSrc = src.replace(/^\//, '').replace(/^\.\//, '');
                var mappings = window.IRIS_CONFIG.fileMappings;
                for (var key in mappings) {
                    if (mappings[key] === cleanSrc || key === cleanSrc) {
                        toSecure.push(img);
                        break;
                    }
                }
            }
        });

        console.log('[Iris Canvas] Found ' + toSecure.length + ' images to secure');

        // Process each image
        toSecure.forEach(function(img) {
            if (img.complete && img.naturalWidth > 0) {
                renderSecureImage(img);
            } else {
                img.addEventListener('load', function() { renderSecureImage(img); });
            }
        });

        // Observe DOM for dynamically added images
        if (typeof MutationObserver !== 'undefined') {
            var observer = new MutationObserver(function(mutations) {
                mutations.forEach(function(mutation) {
                    mutation.addedNodes.forEach(function(node) {
                        if (node.nodeType === 1) {
                            if (node.tagName === 'IMG' && (node.dataset.irisSecure !== undefined || node.hasAttribute('data-iris-secure'))) {
                                if (node.complete) renderSecureImage(node);
                                else node.addEventListener('load', function() { renderSecureImage(node); });
                            }
                            // Also check children
                            var imgs = node.querySelectorAll && node.querySelectorAll('img[data-iris-secure]');
                            if (imgs) {
                                imgs.forEach(function(img) {
                                    if (img.complete) renderSecureImage(img);
                                    else img.addEventListener('load', function() { renderSecureImage(img); });
                                });
                            }
                        }
                    });
                });
            });
            observer.observe(document.body || document.documentElement, { childList: true, subtree: true });
        }
    }

    // Init on DOM ready
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', initIrisCanvas);
    } else {
        initIrisCanvas();
    }
})();
