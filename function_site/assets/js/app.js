// Iris Demo App

(function() {
    window.addEventListener('load', function() {
        refreshStatus();
        checkServiceWorker();
        showConfig();
        checkRendererInfo();
    });

    window.refreshStatus = async function() {
        var el = document.getElementById('status-info');
        try {
            var resp = await fetch('/status');
            var data = await resp.json();
            var configured = resp.headers.get('X-Iris-Configured');
            var version = resp.headers.get('X-Iris-Version');
            el.textContent = JSON.stringify({
                status: data.status,
                configured: data.configured,
                header_X_Iris_Configured: configured,
                header_X_Iris_Version: version,
                key_id: data.key_id ? data.key_id.substring(0, 8) + '...' : 'none',
                algorithm: data.algorithm,
                expires_at: data.expires_at
            }, null, 2);
        } catch (e) {
            el.textContent = 'Error: ' + e.message;
        }
    };

    window.checkServiceWorker = async function() {
        var el = document.getElementById('sw-status');
        if (!('serviceWorker' in navigator)) {
            el.textContent = 'Service Worker: NOT SUPPORTED';
            return;
        }
        var reg = await navigator.serviceWorker.getRegistration('/');
        if (reg) {
            el.textContent = 'Service Worker: ACTIVE\nScope: ' + reg.scope + '\nState: ' + (reg.active ? reg.active.state : 'unknown');
        } else {
            el.textContent = 'Service Worker: NOT REGISTERED (iris-bootstrap.js should register it)';
        }
    };

    window.showConfig = async function() {
        var el = document.getElementById('config-info');
        try {
            var resp = await fetch('/iris-sw-config');
            var data = await resp.json();
            el.textContent = JSON.stringify(data, null, 2);
        } catch (e) {
            el.textContent = 'Error: ' + e.message;
        }
    };

    window.checkRendererInfo = function() {
        var el = document.getElementById('renderer-status');
        var gpu = !!navigator.gpu;
        var info = 'WebGPU: ' + (gpu ? 'AVAILABLE' : 'NOT AVAILABLE (will use Canvas 2D fallback)');
        
        // Check rendered canvases after a delay
        setTimeout(function() {
            var canvases = document.querySelectorAll('.iris-secure-wrapper canvas');
            if (canvases.length > 0) {
                info += '\nSecure canvases: ' + canvases.length;
                canvases.forEach(function(c, i) {
                    var renderer = c.dataset.irisRenderer || 'unknown';
                    info += '\n  Canvas ' + (i+1) + ': ' + renderer + ' (' + c.width + 'x' + c.height + ')';
                });
            } else {
                info += '\nNo secure canvases rendered yet (images may still be loading)';
            }
            el.textContent = info;
        }, 3000);
    };

    window.checkRenderer = function() {
        checkRendererInfo();
        var output = document.getElementById('test-output');
        output.textContent = 'Checking renderer status... (see Renderer section above)';
    };

    window.testDecrypt = async function() {
        var output = document.getElementById('test-output');
        output.textContent = 'Testing decrypt API...\n';

        try {
            var testData = 'Hello from Iris Gateway!';
            var testBase64 = btoa(testData);

            var resp = await fetch('/internal/decrypt', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'X-Internal-Token': 'demo-token-2024'
                },
                body: JSON.stringify({ data: testBase64 })
            });

            if (resp.ok) {
                var result = await resp.json();
                output.textContent += 'Status: OK\n';
                output.textContent += 'Note: raw data is not encrypted, decrypt returns as-is or error\n';
                output.textContent += 'Response length: ' + result.data.length + '\n';
            } else {
                output.textContent += 'Status: ' + resp.status + ' ' + resp.statusText + '\n';
                output.textContent += '(Expected: decrypt only works on AES-256-GCM encrypted data)\n';
            }
        } catch (e) {
            output.textContent += 'Error: ' + e.message + '\n';
        }
    };

    window.testVisibility = function() {
        var output = document.getElementById('test-output');
        output.textContent = 'Visibility test: Switch to another tab/window now.\n';
        output.textContent += 'All secure canvases should blank out (GPU renders noise or clears).\n';
        output.textContent += 'When you come back, they should restore.\n\n';
        output.textContent += 'WebGPU canvases: GPU renders visual noise instead of blank.\n';
        output.textContent += 'Canvas 2D fallback: canvas is cleared to transparent.';
    };

    window.testPrint = function() {
        var output = document.getElementById('test-output');
        output.textContent = 'Print test: Press Ctrl+P (or Cmd+P)\n';
        output.textContent += 'Secure images should be hidden in print preview.\n\n';
        output.textContent += 'Also try: PrintScreen key to test screenshot protection.\n';
        output.textContent += 'WebGPU canvases will render visual noise on screenshot detection.';
    };
})();
