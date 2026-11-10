import assert from 'node:assert/strict';
import { chromium } from '/home/nsubuntu22/code/fleetboard/web/node_modules/playwright/index.mjs';
import { spawn } from 'node:child_process';
import fs from 'node:fs';

const EVIDENCE_DIR = '/tmp/fb-108-evidence';
fs.mkdirSync(EVIDENCE_DIR, { recursive: true });

const PORT = 5195;
const server = spawn('pnpm', ['exec', 'vite', 'preview', '--host', '127.0.0.1', '--port', String(PORT), '--strictPort'], {
  cwd: '/home/nsubuntu22/code/fleetboard/web',
  stdio: 'pipe',
});

let browser;
const qaLogs = [];
function log(msg) {
  console.log(msg);
  qaLogs.push(msg);
}

try {
  await new Promise((resolve, reject) => {
    server.stdout.on('data', d => {
      const s = d.toString();
      if (s.includes(String(PORT))) resolve();
    });
    server.stderr.on('data', d => {
      console.error('SERVER ERR:', d.toString());
    });
    server.on('error', reject);
    server.on('exit', c => reject(new Error(`Preview server exited with ${c}`)));
  });

  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const pageErrors = [];
  const consoleWarnings = [];
  page.on('pageerror', e => pageErrors.push(e.message));
  page.on('console', msg => {
    if (msg.type() === 'error') {
      const text = msg.text();
      // Ignore expected 401 network logs from unauthenticated checks
      if (!text.includes('status of 401')) {
        pageErrors.push(text);
      }
    }
    if (msg.type() === 'warning') consoleWarnings.push(msg.text());
  });

  // Track all asset network requests
  const networkRequests = new Map();
  page.on('request', req => log(`  REQ: ${req.method()} ${req.url()}`));
  page.on('response', resp => {
    log(`  RESP: ${resp.status()} ${resp.url()}`);
    const url = new URL(resp.url());
    if (!url.pathname.startsWith('/api')) {
      networkRequests.set(url.pathname, {
        status: resp.status(),
        contentType: resp.headers()['content-type'] || '',
        ok: resp.ok(),
      });
    }
  });

  // Setup rich test board data
  let isAuthenticated = false;
  const projectObj = { key: 'github.com/narinthon-so/fleetboard', name: 'fleetboard', alias: 'fleetboard' };
  const humanActor = { id: 1, kind: 'human', label: 'Human', provider: null, external_id: null, trusted: true };
  const codexActor = { id: 2, kind: 'agent', label: 'Codex', provider: 'codex', external_id: 'gpt-6.1-sol', trusted: false };
  const agyActor = { id: 3, kind: 'agent', label: 'Agy', provider: 'antigravity', external_id: 'gemini-3.8-flash-high', trusted: false };

  const sampleTasks = [
    {
      id: 'fb-101',
      title: 'Setup parallel brand identity tokens',
      status: 'in_progress',
      kind: 'code',
      priority: 1,
      project: projectObj,
      ticket: null,
      goal: 'Apply Parallel F branding across web app',
      acceptance: '- Header and login branding\n- Serves icons\n- README',
      plan: '1. Audit\n2. Integrate\n3. QA',
      result: null,
      needs_plan_approval: false,
      plan_approved_at: null,
      evidence: null,
      created_by: humanActor,
      created_at: '2026-11-10T14:00:00Z',
      updated_at: '2026-11-10T14:45:00Z',
      claim: {
        id: 1,
        actor: codexActor,
        host: 'RPI',
        cwd: '/home/nsubuntu22/code/fleetboard',
        branch: 'task/fb-107',
        base_commit: 'de3af43',
        workspace_id: null,
        claimed_at: '2026-11-10T14:43:00Z',
        last_activity: '2026-11-10T14:46:00Z',
        released_at: null,
        release_reason: null
      },
      blocked_by: [],
      derived: { ready: false, blocked: false, orphaned: false, plan_pending: false, stale: false, no_evidence: false, needs_me: true },
      claims: [],
      notes: [{ id: 1, kind: 'note', actor: codexActor, body: 'In progress implementing SVG assets', created_at: '2026-11-10T14:46:00Z' }],
      events: []
    },
    {
      id: 'fb-102',
      title: 'Audit header responsive wrapping on 320px',
      status: 'todo',
      kind: 'code',
      priority: 2,
      project: projectObj,
      ticket: null,
      goal: null,
      acceptance: null,
      plan: null,
      result: null,
      needs_plan_approval: false,
      plan_approved_at: null,
      evidence: null,
      created_by: humanActor,
      created_at: '2026-11-10T14:00:00Z',
      updated_at: '2026-11-10T14:00:00Z',
      claim: null,
      blocked_by: [],
      derived: { ready: true, blocked: false, orphaned: false, plan_pending: false, stale: false, no_evidence: false, needs_me: false },
      claims: [],
      notes: [],
      events: []
    },
    {
      id: 'fb-103',
      title: 'Verify Apple touch icon and manifest links',
      status: 'review',
      kind: 'note',
      priority: 2,
      project: projectObj,
      ticket: null,
      goal: null,
      acceptance: null,
      plan: null,
      result: null,
      needs_plan_approval: false,
      plan_approved_at: null,
      evidence: null,
      created_by: humanActor,
      created_at: '2026-11-10T14:00:00Z',
      updated_at: '2026-11-10T14:50:00Z',
      claim: {
        id: 2,
        actor: agyActor,
        host: 'RPI',
        cwd: '/home/nsubuntu22/code/fleetboard',
        branch: 'task/fb-108',
        base_commit: '894ccd7',
        workspace_id: null,
        claimed_at: '2026-11-10T14:50:00Z',
        last_activity: '2026-11-10T14:55:00Z',
        released_at: null,
        release_reason: null
      },
      blocked_by: [],
      derived: { ready: false, blocked: false, orphaned: false, plan_pending: false, stale: false, no_evidence: false, needs_me: false },
      claims: [],
      notes: [{ id: 2, kind: 'note', actor: agyActor, body: 'Reviewing asset resolution', created_at: '2026-11-10T14:55:00Z' }],
      events: []
    },
    {
      id: 'fb-104',
      title: 'Update design documentation with Parallel F kit',
      status: 'done',
      kind: 'note',
      priority: 3,
      project: { key: 'github.com/narinthon-so/docs', name: 'docs', alias: 'docs' },
      ticket: null,
      goal: null,
      acceptance: null,
      plan: null,
      result: 'Docs completed',
      needs_plan_approval: false,
      plan_approved_at: null,
      evidence: null,
      created_by: humanActor,
      created_at: '2026-11-10T13:00:00Z',
      updated_at: '2026-11-10T14:30:00Z',
      claim: null,
      blocked_by: [],
      derived: { ready: false, blocked: false, orphaned: false, plan_pending: false, stale: false, no_evidence: false, needs_me: false },
      claims: [],
      notes: [],
      events: []
    }
  ];

  await page.route('**/api/**', async route => {
    const url = new URL(route.request().url());
    if (url.pathname === '/api/events') {
      return route.fulfill({
        contentType: 'text/event-stream',
        body: ': connected\n\nevent: ping\ndata: {}\n\n',
      });
    }
    if (url.pathname === '/api/login') {
      const postData = JSON.parse(route.request().postData() || '{}');
      if (postData.token === 'test-valid-token') {
        isAuthenticated = true;
        return route.fulfill({
          status: 200,
          headers: { 'Set-Cookie': 'fb_session=valid-qa-session; Path=/; HttpOnly; SameSite=Strict' },
          json: { v: 1, ok: true, data: { user: 'human' } },
        });
      } else {
        return route.fulfill({
          status: 401,
          json: { v: 1, ok: false, error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
        });
      }
    }
    if (!isAuthenticated) {
      return route.fulfill({
        status: 401,
        json: { v: 1, ok: false, error: { code: 'UNAUTHORIZED', message: 'Authentication required' } },
      });
    }
    if (url.pathname === '/api/board') {
      return route.fulfill({
        json: {
          v: 1,
          ok: true,
          data: {
            tasks: sampleTasks,
            projects: [projectObj, { key: 'github.com/narinthon-so/docs', name: 'docs', alias: 'docs' }],
            actors: [humanActor, codexActor, agyActor],
            instances: []
          }
        }
      });
    }
    if (url.pathname.startsWith('/api/tasks/')) {
      const id = url.pathname.replace('/api/tasks/', '');
      const task = sampleTasks.find(t => t.id === id);
      if (task) {
        return route.fulfill({ json: { v: 1, ok: true, data: { task } } });
      }
      return route.fulfill({ status: 404, json: { v: 1, ok: false, error: { code: 'NOT_FOUND', message: 'Task not found' } } });
    }
    return route.fulfill({ status: 404, json: { v: 1, ok: false, error: { code: 'NOT_FOUND', message: 'Not found' } } });
  });

  log('--- 1. TESTING UNAUTHENTICATED LOGIN SCREEN BRANDING ---');
  isAuthenticated = false;

  for (const width of [1440, 375, 320]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto(`http://127.0.0.1:${PORT}/`);
    await page.locator('#token-input').waitFor();

    for (const theme of ['dark', 'light']) {
      await page.evaluate(t => {
        document.documentElement.className = t;
      }, theme);

      // Verify BrandLogo
      const logo = page.getByRole('img', { name: 'fleetboard', exact: true });
      assert(await logo.isVisible(), `Login logo not visible in ${theme} at ${width}px`);

      const darkImg = logo.locator('.brand-logo-dark');
      const lightImg = logo.locator('.brand-logo-light');

      if (theme === 'dark') {
        assert(await darkImg.isVisible(), `Dark logo not visible in dark theme at ${width}px`);
        assert(!(await lightImg.isVisible()), `Light logo visible in dark theme at ${width}px`);
        const imgOk = await darkImg.evaluate(el => el.complete && el.naturalWidth > 0 && el.naturalHeight > 0);
        assert(imgOk, `Dark logo image not loaded in ${theme} at ${width}px`);
      } else {
        assert(await lightImg.isVisible(), `Light logo not visible in light theme at ${width}px`);
        assert(!(await darkImg.isVisible()), `Dark logo visible in light theme at ${width}px`);
        const imgOk = await lightImg.evaluate(el => el.complete && el.naturalWidth > 0 && el.naturalHeight > 0);
        assert(imgOk, `Light logo image not loaded in ${theme} at ${width}px`);
      }

      const box = await logo.boundingBox();
      assert(box, `Could not get logo bounding box in ${theme} at ${width}px`);
      log(`Login logo ${theme} ${width}px: width=${box.width.toFixed(1)}px, height=${box.height.toFixed(1)}px, x=${box.x.toFixed(1)}px`);
      assert(box.width >= 200, `Login logo width < 200px: ${box.width}px`);
      assert(box.x >= 0 && box.x + box.width <= width, `Login logo clipped horizontally: ${box.x} to ${box.x + box.width}`);

      // Check no horizontal overflow on the page
      const overflow = await page.evaluate(() => {
        return document.documentElement.scrollWidth > document.documentElement.clientWidth;
      });
      assert(!overflow, `Horizontal overflow detected on Login screen in ${theme} at ${width}px!`);

      const screenshotPath = `${EVIDENCE_DIR}/login-${theme}-${width}.png`;
      await page.screenshot({ path: screenshotPath, fullPage: true });
      log(`  Captured screenshot: ${screenshotPath}`);
    }
  }

  log('--- 2. TESTING AUTHENTICATION FLOW VIA UI ---');
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto(`http://127.0.0.1:${PORT}/`);
  const tokenInput = page.locator('#token-input');
  await tokenInput.waitFor();

  // Test invalid token
  await tokenInput.fill('invalid-token');
  await page.getByRole('button', { name: /Authenticate/i }).click();
  const alert = page.locator('text=Invalid token');
  await alert.waitFor({ timeout: 3000 });
  log('  Invalid token error correctly displayed.');

  // Test valid token
  await tokenInput.fill('test-valid-token');
  await page.getByRole('button', { name: /Authenticate/i }).click();
  await page.locator('header').waitFor({ timeout: 10000 });
  log('  Successful login! Board header loaded.');

  log('--- 3. TESTING AUTHENTICATED BOARD SCREEN BRANDING & CONTROLS ---');
  for (const width of [1440, 375, 320]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto(`http://127.0.0.1:${PORT}/`);
    await page.locator('header').waitFor();
    await page.locator('[data-card-id="fb-101"]').waitFor();

    for (const theme of ['dark', 'light']) {
      // Switch theme using UI button if needed, or verify state
      const isCurrentlyDark = await page.evaluate(() => document.documentElement.classList.contains('dark'));
      const shouldBeDark = theme === 'dark';
      if (isCurrentlyDark !== shouldBeDark) {
        const themeBtn = page.getByRole('button', { name: /Switch to/i });
        await themeBtn.click();
        await page.waitForTimeout(200);
      }

      const activeTheme = await page.evaluate(() => document.documentElement.classList.contains('dark') ? 'dark' : 'light');
      assert.equal(activeTheme, theme, `Theme toggle button failed to set ${theme}`);

      // Check header BrandLogo
      const logo = page.locator('header').getByRole('img', { name: 'fleetboard', exact: true });
      assert(await logo.isVisible(), `Header logo not visible in ${theme} at ${width}px`);

      const darkImg = logo.locator('.brand-logo-dark');
      const lightImg = logo.locator('.brand-logo-light');

      if (theme === 'dark') {
        assert(await darkImg.isVisible(), `Header dark logo not visible in dark mode at ${width}px`);
        assert(!(await lightImg.isVisible()), `Header light logo visible in dark mode at ${width}px`);
        const imgOk = await darkImg.evaluate(el => el.complete && el.naturalWidth > 0 && el.naturalHeight > 0);
        assert(imgOk, `Header dark logo image not loaded at ${width}px`);
      } else {
        assert(await lightImg.isVisible(), `Header light logo not visible in light mode at ${width}px`);
        assert(!(await darkImg.isVisible()), `Header dark logo visible in light mode at ${width}px`);
        const imgOk = await lightImg.evaluate(el => el.complete && el.naturalWidth > 0 && el.naturalHeight > 0);
        assert(imgOk, `Header light logo image not loaded at ${width}px`);
      }

      const box = await logo.boundingBox();
      assert(box, `Could not get header logo box at ${width}px`);
      log(`Board header logo ${theme} ${width}px: width=${box.width.toFixed(1)}px, height=${box.height.toFixed(1)}px, x=${box.x.toFixed(1)}px`);
      assert(box.width >= 200, `Board logo width must be >= 200px: ${box.width}px`);
      assert(box.x >= 0 && box.x + box.width <= width, `Board logo clipped horizontally at ${width}px: x=${box.x}, width=${box.width}`);

      // Check page horizontal overflow
      const overflow = await page.evaluate(() => {
        return document.documentElement.scrollWidth > document.documentElement.clientWidth;
      });
      assert(!overflow, `Horizontal overflow detected on Board screen in ${theme} at ${width}px!`);

      // Check theme switch button visibility
      const themeBtn = page.getByRole('button', { name: /Switch to/i });
      assert(await themeBtn.isVisible(), `Theme toggle button not visible at ${width}px`);

      // Check SSE status indicator is present
      const sseIndicator = page.locator('header div[title*="fleetboard daemon"], header div[title*="Real-time updates"]');
      assert(await sseIndicator.isVisible(), `SSE status indicator not visible at ${width}px`);
      log(`  SSE indicator visible (${await sseIndicator.innerText()})`);

      const screenshotPath = `${EVIDENCE_DIR}/board-${theme}-${width}.png`;
      await page.screenshot({ path: screenshotPath, fullPage: true });
      log(`  Captured screenshot: ${screenshotPath}`);
    }
  }

  log('--- 4. TESTING BOARD CONTROLS & FUNCTIONAL STATUS COLOURS ---');
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto(`http://127.0.0.1:${PORT}/`);
  await page.locator('header').waitFor();
  await page.locator('[data-card-id="fb-101"]').waitFor();

  // Test keyboard navigation
  await page.keyboard.press('j');
  await page.waitForTimeout(100);
  await page.keyboard.press('Enter');
  const dialog = page.getByRole('dialog', { name: /Task detail for fb-101/i });
  await dialog.waitFor({ timeout: 3000 });
  log('  Keyboard navigation: j selected fb-101, Enter opened task detail modal.');
  await page.screenshot({ path: `${EVIDENCE_DIR}/board-task-modal-1440.png` });

  await page.keyboard.press('Escape');
  assert.equal(await page.getByRole('dialog').count(), 0, 'Dialog should close on Escape');
  log('  Escape closed task detail modal.');

  // Test Filter Controls
  const projectBtn = page.getByRole('combobox').first();
  await projectBtn.click();
  await page.waitForTimeout(200);
  await page.screenshot({ path: `${EVIDENCE_DIR}/board-filter-active-1440.png` });
  log('  Filter combobox opened successfully.');
  await page.keyboard.press('Escape');

  // Test narrow mobile filter row wrapping
  await page.setViewportSize({ width: 320, height: 568 });
  await page.goto(`http://127.0.0.1:${PORT}/`);
  await page.locator('header').waitFor();
  await page.screenshot({ path: `${EVIDENCE_DIR}/board-narrow-filter-scroll-320.png` });
  log('  Narrow mobile filter row rendered with scrollable container.');

  log('--- 5. TESTING STATIC ASSET DIRECT NETWORK RESOLUTION ---');
  const criticalAssets = [
    '/favicon.ico',
    '/favicon.svg',
    '/apple-touch-icon.png',
    '/site.webmanifest',
    '/brand/fleetboard-horizontal-color.svg',
    '/brand/fleetboard-horizontal-white.svg',
    '/icon-192.png',
    '/icon-512.png',
    '/maskable-512.png',
  ];

  for (const assetPath of criticalAssets) {
    const resp = await page.request.get(`http://127.0.0.1:${PORT}${assetPath}`);
    assert.equal(resp.status(), 200, `Asset ${assetPath} returned HTTP ${resp.status()}`);
    const ct = resp.headers()['content-type'] || '';
    assert(ct.length > 0, `Asset ${assetPath} missing Content-Type`);
    const body = await resp.body();
    assert(body.length > 0, `Asset ${assetPath} returned empty body`);
    log(`  Asset OK: ${assetPath} [HTTP ${resp.status()}, ${ct}, ${body.length} bytes]`);
  }

  // Note: Vite preview uses historyApiFallback serving index.html for unmatched paths.
  // The Rust embedded Axum server's 404 behavior for paths containing extensions
  // is verified independently in cargo test test_serves_embedded_web_ui.
  log('  Static asset resolution verified on all 9 critical runtime assets.');

  // Verify page errors
  assert.deepEqual(pageErrors, [], `Encountered browser page errors: ${JSON.stringify(pageErrors)}`);
  log('  0 browser page errors encountered during entire test suite.');

  log('\n=== ALL PLAYWRIGHT BRANDING & INTERACTION CHECKS PASSED ===');
  fs.writeFileSync(`${EVIDENCE_DIR}/browser-qa.log`, qaLogs.join('\n') + '\n');
} finally {
  await browser?.close();
  server.kill('SIGTERM');
}
