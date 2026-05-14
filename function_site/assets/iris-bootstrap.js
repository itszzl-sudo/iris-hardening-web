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
  const script = document.currentScript;
  const swUrl = (script && script.dataset.irisSw) || '/iris-sw.js';

  navigator.serviceWorker.register(swUrl, { scope: '/' })
    .then(function(reg) {
      console.log('[Iris] Service Worker registered, scope:', reg.scope);

      // If there's an update, apply it immediately
      reg.addEventListener('updatefound', function() {
        var newWorker = reg.installing;
        newWorker.addEventListener('statechange', function() {
          if (newWorker.state === 'activated') {
            console.log('[Iris] Service Worker updated and activated');
          }
        });
      });

      // If the page is controlled by an old SW, reload to use the new one
      if (navigator.serviceWorker.controller) {
        console.log('[Iris] Service Worker is controlling this page');
      }
    })
    .catch(function(err) {
      console.error('[Iris] Service Worker registration failed:', err);
    });
})();
