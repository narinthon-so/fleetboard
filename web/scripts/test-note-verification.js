import assert from 'node:assert/strict';
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';

const server = spawn('pnpm', ['exec', 'vite', '--host', '127.0.0.1', '--port', '5193', '--strictPort'], { cwd: process.cwd(), stdio: 'pipe' });
let browser;
try {
  await new Promise((resolve, reject) => {
    server.stdout.on('data', data => { if (data.toString().includes('5193')) resolve(); });
    server.on('error', reject);
    server.on('exit', code => reject(new Error(`Vite exited ${code}`)));
  });
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  page.setDefaultTimeout(5000);
  await page.addInitScript(() => {
    window.EventSource = class extends EventTarget {
      constructor() { super(); setTimeout(() => this.onopen?.(new Event('open')), 0); }
      close() {}
    };
  });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const derived = { ready: false, blocked: false, orphaned: false, plan_pending: false, stale: false, no_evidence: false, needs_me: false };
  const base = { kind: 'code', status: 'todo', priority: 2, project: null, ticket: null, goal: null, acceptance: null, plan: null, result: null, needs_plan_approval: false, plan_approved_at: null, claim: null, evidence: null, derived, blocked_by: [], created_by: { id: 1, kind: 'human', label: 'Human' }, created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z', claims: [], notes: [], events: [] };
  let task = { ...base, id: 'fb-1', title: 'Review task', kind: 'note', status: 'review', result: 'Completed audit' };
  const requests = [];
  await page.route('**/api/**', route => {
    const url = new URL(route.request().url());
    if (url.pathname === '/api/events') return route.fulfill({ contentType: 'text/event-stream', body: ': connected\n\n' });
    if (url.pathname.endsWith('/verify')) requests.push(route.request().postDataJSON());
    const data = url.pathname === '/api/board' ? { tasks: [task], projects: [], instances: [], actors: [] } : { task };
    return route.fulfill({ json: { v: 1, ok: true, data } });
  });
  async function open(kind, evidence, detail = true) {
    requests.length = 0;
    task = { ...task, kind, evidence, derived: { ...derived, needs_me: true, no_evidence: kind === 'code' } };
    await page.goto(`http://127.0.0.1:5193/${detail ? 'task/fb-1' : ''}`);
    await page.locator('[data-column-id="review"] [data-card-id="fb-1"]').waitFor();
    if (detail) await page.getByRole('dialog', { name: 'Task detail for fb-1' }).waitFor();
  }
  async function verified() {
    await page.getByText('fb-1 verified and marked done', { exact: true }).waitFor();
    assert.deepEqual(requests, [{ force: false }]);
    assert.equal(await page.getByText('Verify without evidence', { exact: true }).count(), 0);
  }
  for (const evidence of [null, { commits: 0, reachable: true, dirty: false }]) {
    await open('note', evidence);
    await page.getByRole('button', { name: 'Verify', exact: true }).click();
    await verified();
    await open('code', evidence);
    await page.getByRole('button', { name: 'Verify', exact: true }).click();
    await page.getByText('Verify without evidence', { exact: true }).waitFor();
    assert.equal(requests.length, 0);
  }
  await open('note', null);
  await page.keyboard.press('v');
  await page.getByRole('button', { name: 'Verify and Complete', exact: true }).click();
  await verified();
  await open('code', null);
  await page.keyboard.press('v');
  await page.getByRole('button', { name: 'Verify with force', exact: true }).waitFor();
  assert.equal(requests.length, 0);
  for (const kind of ['note', 'code']) {
    await open(kind, null, false);
    const source = await page.locator('[data-column-id="review"] [data-card-id="fb-1"]').boundingBox();
    const target = await page.locator('[data-column-id="done"]').boundingBox();
    await page.mouse.move(source.x + source.width / 2, source.y + source.height / 2);
    await page.mouse.down();
    await page.mouse.move(source.x + source.width / 2 + 15, source.y + source.height / 2, { steps: 5 });
    await page.mouse.move(target.x + target.width / 2, target.y + target.height / 2, { steps: 15 });
    await page.mouse.up();
    if (kind === 'note') await verified();
    else {
      await page.getByText('Verify without evidence', { exact: true }).waitFor();
      assert.equal(requests.length, 0);
    }
  }
  assert.deepEqual(errors, []);
  console.log('PASS: 8 verification cases (detail null/zero evidence, keyboard, drag; note and code)');
} finally {
  await browser?.close();
  server.kill('SIGTERM');
}
