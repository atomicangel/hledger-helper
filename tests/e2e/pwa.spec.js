import { test, expect } from '@playwright/test';

test.describe('PWA Manifest', () => {
  test('should have valid manifest.json', async ({ request }) => {
    const response = await request.get('/manifest.json');
    expect(response.ok()).toBe(true);
    
    const manifest = await response.json();
    expect(manifest.name).toBe('hledger Helper');
    expect(manifest.short_name).toBe('hledger');
    expect(manifest.display).toBe('standalone');
    expect(manifest.icons).toBeDefined();
    expect(Array.isArray(manifest.icons)).toBe(true);
  });
});

test.describe('Static Assets', () => {
  test('should load index.html', async ({ request }) => {
    const response = await request.get('/');
    expect(response.ok()).toBe(true);
    const text = await response.text();
    expect(text).toContain('<title>hledger Helper</title>');
    expect(text).toContain('Quick Entry');
    expect(text).toContain('Chat Assistant');
  });

  test('should load CSS', async ({ request }) => {
    const response = await request.get('/css/style.css');
    expect(response.ok()).toBe(true);
    const text = await response.text();
    expect(text.toLowerCase()).toContain('mobile-first');
    expect(text).toContain('--primary');
  });

  test('should load JS', async ({ request }) => {
    const response = await request.get('/js/app.js');
    expect(response.ok()).toBe(true);
    const text = await response.text();
    expect(text).toContain('hledger Helper');
    expect(text).toContain('IndexedDB');
  });

  test('should load service worker', async ({ request }) => {
    const response = await request.get('/sw.js');
    expect(response.ok()).toBe(true);
    const text = await response.text();
    expect(text).toContain('Service Worker');
    expect(text).toContain('hledger-v1');
  });
});

test.describe('API Endpoints', () => {
  test('should have config.json endpoint', async ({ request }) => {
    const response = await request.get('/config.json');
    expect(response.ok()).toBe(true);
    
    const config = await response.json();
    expect(config.nas_path).toBe('./demo_journals');
    expect(config.current_year).toBe(2026);
    expect(config.server).toBeDefined();
    expect(config.server.port).toBe(8080);
  });

  test('should have transfer-accounts endpoint', async ({ request }) => {
    const response = await request.get('/api/transfer-accounts');
    expect(response.ok()).toBe(true);
    const accounts = await response.json();
    expect(Array.isArray(accounts)).toBe(true);
  });
});

test.describe('Mobile Responsiveness', () => {
  test('should have viewport meta tag', async ({ request }) => {
    const response = await request.get('/');
    const text = await response.text();
    expect(text).toContain('<meta name="viewport"');
    expect(text).toContain('width=device-width');
    expect(text).toContain('initial-scale=1.0');
  });

  test('should have responsive CSS', async ({ request }) => {
    const response = await request.get('/css/style.css');
    const text = await response.text();
    expect(text).toContain('@media (max-width: 768px)');
    expect(text).toContain('@media (min-width: 1200px)');
  });
});

test.describe('Offline Support', () => {
  test('should have offline indicator in JS', async ({ request }) => {
    const response = await request.get('/js/app.js');
    const text = await response.text();
    expect(text).toContain('offlineIndicator');
    expect(text).toContain('addToQueue');
  });
});

test.describe('UI Components', () => {
  test('should have Quick Entry tab', async ({ request }) => {
    const response = await request.get('/');
    const text = await response.text();
    expect(text).toContain('Quick Entry');
    expect(text).toContain('payee');
    expect(text).toContain('split-rows');
    expect(text).toContain('amount');
  });

  test('should have Chat Assistant tab', async ({ request }) => {
    const response = await request.get('/');
    const text = await response.text();
    expect(text).toContain('Chat Assistant');
    expect(text).toContain('chat-container');
  });

  test('should have payment source chips container', async ({ request }) => {
    const response = await request.get('/');
    const text = await response.text();
    expect(text).toContain('payment-chips');
  });

  test('should have Transfers tab', async ({ request }) => {
    const response = await request.get('/');
    const text = await response.text();
    expect(text).toContain('Transfers');
    expect(text).toContain('id="transfers"');
    expect(text).toContain('id="transfer-payee"');
    expect(text).toContain('id="transfer-source"');
    expect(text).toContain('id="transfer-dest"');
    expect(text).toContain('id="transfer-amount"');
    expect(text).toContain('id="submit-transfer"');
  });
});

test.describe('Form Functionality', () => {
  test('should have transaction form fields', async ({ request }) => {
    const response = await request.get('/');
    const text = await response.text();
    expect(text).toContain('id="payee"');
    expect(text).toContain('id="amount"');
    expect(text).toContain('id="category"');
    expect(text).toContain('id="submit-transaction"');
  });

  test('should have running total display', async ({ request }) => {
    const response = await request.get('/');
    const text = await response.text();
    expect(text).toContain('total-amount');
    expect(text).toContain('running-total');
  });
});

test.describe('PWA Installation', () => {
  test('should have installable manifest', async ({ request }) => {
    const response = await request.get('/manifest.json');
    const manifest = await response.json();
    
    expect(manifest.display).toBe('standalone');
    expect(Array.isArray(manifest.icons)).toBe(true);
    expect(manifest.icons.length).toBeGreaterThan(0);
  });

  test('should have apple touch icon', async ({ request }) => {
    const response = await request.get('/');
    const text = await response.text();
    expect(text).toContain('rel="apple-touch-icon"');
  });
});
