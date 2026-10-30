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
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const derived = { ready: false, blocked: false, orphaned: false, plan_pending: false, stale: false, no_evidence: false, needs_me: false };
  const base = { kind: 'code', status: 'todo', priority: 2, project: null, ticket: null, goal: null, acceptance: null, plan: null, result: null, needs_plan_approval: false, plan_approved_at: null, claim: null, evidence: null, derived, blocked_by: [], created_by: { id: 1, kind: 'human', label: 'Human' }, created_at: '2026-01-01T00:00:00Z', updated_at: '2026-01-01T00:00:00Z', claims: [], notes: [], events: [] };
  const tasks = [ { ...base, id: 'fb-1', title: 'Blocker' }, { ...base, id: 'fb-2', title: 'Dependent', blocked_by: [{ id: 'fb-1', title: 'Blocker', status: 'todo' }], derived: { ...derived, blocked: true } } ];
  await page.route('**/api/**', route => {
    const url = new URL(route.request().url());
    if (url.pathname === '/api/events') return route.fulfill({ contentType: 'text/event-stream', body: ': connected\n\n' });
    const data = url.pathname === '/api/board' ? { tasks, projects: [], instances: [], actors: [] } : { task: tasks.find(task => url.pathname.endsWith(task.id)) };
    return route.fulfill({ json: { v: 1, ok: true, data } });
  });
  await page.goto('http://127.0.0.1:5193/task/fb-2');
  const blocker = page.getByRole('button', { name: 'fb-1', exact: true });
  await blocker.waitFor({ timeout: 5000 });
  await blocker.click();
  await page.getByRole('dialog', { name: 'Task detail for fb-1' }).waitFor();
  await page.getByRole('button', { name: 'fb-2 (todo)', exact: true }).waitFor();
  assert.deepEqual(errors, []);
  console.log('PASS: dependency detail renders, navigates, and shows reverse dependencies');
} finally {
  await browser?.close();
  server.kill('SIGTERM');
}
