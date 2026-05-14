/**
 * Iris Bootstrap - Registers the Iris Service Worker
 *
 * This is the ONLY script that needs to be injected into HTML.
 * Once the SW is registered, it handles all encryption/decryption transparently.
 *
 * Usage (injected by gateway before </head>):
 *   <script src="/iris-bootstrap.js"></script>
 *
 * Or manually:
 *   <script src="/iris-bootstrap.js" data-iris-sw="/iris-sw.js"></script>
 */

(function() {
  if (!('serviceWorker' in navigator)) {
    console.warn('[Iris] Service Workers not supported in this browser');
    return;
  }

  // Allow custom SW URL via data attribute, default to /iris-sw.js
  var script = document.currentScript;
  var swUrl = (script && script.dataset.irisSw) || '/iris-sw.js';

  /** Send IRIS_CONFIG to the active Service Worker */
  function sendConfigToSW() {
    if (window.IRIS_CONFIG && navigator.serviceWorker.controller) {
      navigator.serviceWorker.controller.postMessage({
        type: 'IRIS_CONFIG',
        config: window.IRIS_CONFIG,
      });
    }
  }

  navigator.serviceWorker.register(swUrl, { scope: '/' })
    .then(function(reg) {
      console.log('[Iris] Service Worker registered, scope:', reg.scope);

      // If there's an update, apply it immediately
      reg.addEventListener('updatefound', function() {
        var newWorker = reg.installing;
        newWorker.addEventListener('statechange', function() {
          if (newWorker.state === 'activated') {
            console.log('[Iris] Service Worker updated and activated');
            // Send config to newly activated SW
            sendConfigToSW();
          }
        });
      });

      // If the page is already controlled by a SW, send config immediately
      sendConfigToSW();

      // Also listen for controller change (first activation)
      navigator.serviceWorker.addEventListener('controllerchange', function() {
        // Small delay to let the SW finish activating
        setTimeout(sendConfigToSW, 100);
      });
    })
    .catch(function(err) {
      console.error('[Iris] Service Worker registration failed:', err);
    });
})();
