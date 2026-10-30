import { chromium } from 'playwright';
import { spawn, execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';

async function main() {
  const tmpHome = fs.mkdtempSync(path.join(os.tmpdir(), 'fb-screenshots-home-'));
  const tmpRepo = fs.mkdtempSync(path.join(os.tmpdir(), 'fb-screenshots-repo-'));
  const screenshotsDir = path.resolve('../docs/screenshots');
  fs.mkdirSync(screenshotsDir, { recursive: true });

  const fbBin = path.resolve('../target/debug/fb');

  // Initialize a fake git repo
  execSync(`git init -b main "${tmpRepo}"`);
  execSync(`git -C "${tmpRepo}" config user.email "bot@example.com"`);
  execSync(`git -C "${tmpRepo}" config user.name "Bot"`);
  execSync(`git -C "${tmpRepo}" commit --allow-empty -m "Initial commit"`);
  execSync(`git -C "${tmpRepo}" remote add origin https://github.com/example/api.git`);

  console.log('Spawning fb serve...');
  const server = spawn(fbBin, ['--json', 'serve', '--port', '7979'], {
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PATH: process.env.PATH,
    },
  });

  let token = null;
  let port = 7979;

  await new Promise((resolve, reject) => {
    server.stdout.on('data', (data) => {
      const text = data.toString();
      try {
        const json = JSON.parse(text);
        if (json.ok && json.data) {
          token = json.data.token;
          port = json.data.port;
          resolve();
        }
      } catch {
        // ignore
      }
    });

    server.stderr.on('data', (d) => process.stderr.write(d));
    server.on('error', reject);
    setTimeout(() => reject(new Error('Timeout waiting for fb serve output')), 10000);
  });

  console.log(`fb serve running on port ${port} with token: ${token.slice(0, 10)}...`);

  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
  });
  const page = await context.newPage();

  // 1. Load root (should show login screen)
  console.log('Navigating to login screen...');
  await page.goto(`http://127.0.0.1:${port}/`);
  await page.waitForSelector('input#token-input');

  // Fill in token and submit
  console.log('Logging in...');
  await page.fill('input#token-input', token);
  await page.click('button[type="submit"]');

  // 2. Empty state
  console.log('Waiting for empty board state...');
  await page.waitForSelector('text=Board is empty');
  await page.waitForTimeout(500);

  const emptyStatePath = path.join(screenshotsDir, 'empty-state.png');
  await page.screenshot({ path: emptyStatePath });
  console.log(`Saved empty state screenshot to: ${emptyStatePath}`);

  // 3. Seed some tasks using CLI
  console.log('Seeding tasks via CLI...');
  function runCli(args, extraEnv = {}) {
    const cleanEnv = { ...process.env };
    for (const key of Object.keys(cleanEnv)) {
      if (key.startsWith('PASEO_') || key.startsWith('CLAUDE') || key.startsWith('CODEX')) {
        delete cleanEnv[key];
      }
    }
    return execSync(`"${fbBin}" --json ${args}`, {
      cwd: tmpRepo,
      env: {
        ...cleanEnv,
        FB_HOME: tmpHome,
        FB_TEST_CHAIN: 'bash',
        PATH: process.env.PATH,
        ...extraEnv,
      },
      encoding: 'utf-8',
    });
  }

  // Create tasks
  // Task 1: Draft (added by agent)
  execSync(`"${fbBin}" --json add "GraphQL schema introspection caching" --prio 3`, {
    cwd: tmpRepo,
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PASEO_AI_AGENT: 'codex',
      PASEO_AGENT_EXTERNAL_ID: '403254b',
    },
  });

  // Task 2: Ready
  runCli('--as-human add "Audit log retention and cleanup" --prio 2');

  // Task 3: In progress with commits
  const t3Json = JSON.parse(runCli('--as-human add "Rate-limit public endpoints" --prio 1 --goal "Rate limit public API" --ac "429 after 100 req/min" --ac "Metric counters incremented"'));
  const t3Id = t3Json.data.task.id;
  // Claim as codex agent
  execSync(`"${fbBin}" claim ${t3Id}`, {
    cwd: tmpRepo,
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PASEO_AI_AGENT: 'codex',
      PASEO_AGENT_EXTERNAL_ID: '403254b',
    },
  });
  // Make 3 commits on task/t3Id branch
  execSync(`git -C "${tmpRepo}" checkout -b task/${t3Id}`);
  execSync(`echo "test" > "${tmpRepo}/rate.txt" && git -C "${tmpRepo}" add . && git -C "${tmpRepo}" commit -m "Add rate limit"`);
  execSync(`echo "test2" >> "${tmpRepo}/rate.txt" && git -C "${tmpRepo}" add . && git -C "${tmpRepo}" commit -m "Tests for rate limit"`);
  execSync(`echo "test3" >> "${tmpRepo}/rate.txt" && git -C "${tmpRepo}" add . && git -C "${tmpRepo}" commit -m "Refactor rate limit"`);
  // Note task so fb sees latest activity
  execSync(`"${fbBin}" note ${t3Id} "Implemented token bucket filter"`, {
    cwd: tmpRepo,
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PASEO_AI_AGENT: 'codex',
      PASEO_AGENT_EXTERNAL_ID: '403254b',
    },
  });

  // Task 4: Review with no commits (needs me)
  const t4Json = JSON.parse(runCli('--as-human add "Investigate memory leak under high concurrency" --prio 1 --goal "Find memory leak root cause"'));
  const t4Id = t4Json.data.task.id;
  execSync(`"${fbBin}" claim ${t4Id}`, {
    cwd: tmpRepo,
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PASEO_AI_AGENT: 'antigravity',
      PASEO_AGENT_EXTERNAL_ID: '20aabd3',
    },
  });
  execSync(`"${fbBin}" note ${t4Id} "Profiling shows jemalloc fragmentation"`, {
    cwd: tmpRepo,
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PASEO_AI_AGENT: 'antigravity',
      PASEO_AGENT_EXTERNAL_ID: '20aabd3',
    },
  });
  execSync(`"${fbBin}" done ${t4Id}`, {
    cwd: tmpRepo,
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PASEO_AI_AGENT: 'antigravity',
      PASEO_AGENT_EXTERNAL_ID: '20aabd3',
    },
  });

  // Task 5: Done task
  const t5Json = JSON.parse(runCli('--as-human add "Semantic CSS token migration" --prio 2'));
  const t5Id = t5Json.data.task.id;
  execSync(`"${fbBin}" claim ${t5Id}`, {
    cwd: tmpRepo,
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PASEO_AI_AGENT: 'claude',
      PASEO_AGENT_EXTERNAL_ID: '8872f10',
    },
  });
  execSync(`"${fbBin}" done ${t5Id}`, {
    cwd: tmpRepo,
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PASEO_AI_AGENT: 'claude',
      PASEO_AGENT_EXTERNAL_ID: '8872f10',
    },
  });
  runCli(`--as-human verify ${t5Id} --force`);

  // Task 6: Blocked task
  const t6Json = JSON.parse(runCli(`--as-human add "Zero-downtime database rollout" --prio 0 --blocked-by ${t3Id}`));
  const _t6Id = t6Json.data.task.id;

  // Wait for board to update via SSE or refresh
  console.log('Reloading board to display all tasks...');
  await page.reload();
  await page.waitForSelector(`[data-card-id="${t3Id}"]`);
  await page.waitForTimeout(1000);

  // Check column layout bounds
  const columnRects = await page.evaluate(() => {
    return Array.from(document.querySelectorAll('[data-column-id]')).map((el) => {
      const r = el.getBoundingClientRect();
      return { id: el.getAttribute('data-column-id'), left: Math.round(r.left), right: Math.round(r.right), width: Math.round(r.width) };
    });
  });
  console.log('Columns layout bounds at 1440px:', columnRects);

  const boardPath = path.join(screenshotsDir, 'board.png');
  await page.screenshot({ path: boardPath });
  console.log(`Saved board screenshot to: ${boardPath}`);

  // 4. Open Card Detail Sheet
  console.log('Opening card detail sheet...');
  await page.click(`[data-card-id="${t3Id}"]`);
  await page.waitForSelector(`role=dialog[name="Task detail for ${t3Id}"]`);
  await page.waitForTimeout(600);

  const detailPath = path.join(screenshotsDir, 'card-detail.png');
  await page.screenshot({ path: detailPath });
  console.log(`Saved card detail screenshot to: ${detailPath}`);

  await browser.close();
  server.kill('SIGTERM');

  // Clean up temp dirs
  fs.rmSync(tmpHome, { recursive: true, force: true });
  fs.rmSync(tmpRepo, { recursive: true, force: true });
  console.log('All screenshots captured successfully!');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
