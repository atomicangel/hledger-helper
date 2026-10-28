// Service Worker for hledger Helper
// Handles offline transactions and caching

const CACHE_NAME = 'hledger-v1';
const ASSETS = [
  '/',
  '/css/style.css',
  '/js/app.js'
];

// Install event - cache assets
self.addEventListener('install', (e) => {
  e.waitUntil(
    caches.open(CACHE_NAME)
      .then((cache) => cache.addAll(ASSETS))
      .then(() => self.skipWaiting())
  );
});

// Activate event - clean old caches
self.addEventListener('activate', (e) => {
  e.waitUntil(
    caches.keys().then((keys) => {
      return Promise.all(keys.map((key) => {
        if (key !== CACHE_NAME) {
          return caches.delete(key);
        }
      }));
    }).then(() => self.clients.claim())
  );
});

// Fetch event - serve from cache, fallback to network
self.addEventListener('fetch', (e) => {
  e.respondWith(
    caches.match(e.request)
      .then((response) => {
        return response || fetch(e.request);
      })
  );
});

// Message handling for offline sync
self.addEventListener('message', (e) => {
  if (e.data.type === 'SYNC') {
    e.waitUntil(syncOfflineTransactions());
    e.source.postMessage({ type: 'SYNC_COMPLETE' });
  }
});

async function syncOfflineTransactions() {
  // Sync logic will be implemented in Phase 2b
  console.log('Syncing offline transactions...');
}
