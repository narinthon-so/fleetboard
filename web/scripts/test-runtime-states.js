import { chromium } from 'playwright';
import { spawn, execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';

async function main() {
  const tmpHome = fs.mkdtempSync(path.join(os.tmpdir(), 'fb-runtime-home-'));
  const tmpRepo = fs.mkdtempSync(path.join(os.tmpdir(), 'fb-runtime-repo-'));
  const tmpBin = fs.mkdtempSync(path.join(os.tmpdir(), 'fb-runtime-bin-'));
  const screenshotsDir = path.resolve('../docs/screenshots');
  fs.mkdirSync(screenshotsDir, { recursive: true });

  const fbBin = path.resolve('../target/debug/fb');

  // 1. Initialize git repo
  execSync(`git init -b main "${tmpRepo}"`);
  execSync(`git -C "${tmpRepo}" config user.email "bot@example.com"`);
  execSync(`git -C "${tmpRepo}" config user.name "Bot"`);
  execSync(`git -C "${tmpRepo}" commit --allow-empty -m "Initial commit"`);
  execSync(`git -C "${tmpRepo}" remote add origin https://github.com/example/api.git`);

  // 2. Configure adapter = "paseo" in $FB_HOME/config.toml
  fs.writeFileSync(
    path.join(tmpHome, 'config.toml'),
    `adapter = "paseo"\nstale_after = "2h"\n`
  );

  // 3. Create fake paseo binary
  const paseoScript = `#!/bin/bash
case "$1" in
  workspace)
    echo '[{"workspaceId": "wks_001", "cwd": "${tmpRepo}", "project": "api"}]'
    exit 0
    ;;
  ls)
    echo '[
      {"id": "agent-running", "shortId": "run1", "status": "running", "cwd": "${tmpRepo}"},
      {"id": "agent-idle", "shortId": "idl2", "status": "idle", "cwd": "${tmpRepo}"},
      {"id": "agent-perm", "shortId": "prm3", "status": "permission", "cwd": "${tmpRepo}"},
      {"id": "agent-gone", "shortId": "gon4", "status": "archived", "cwd": "${tmpRepo}"}
    ]'
    exit 0
    ;;
  inspect)
    ID="$2"
    if [ "$ID" = "agent-running" ] || [ "$ID" = "run1" ]; then
      echo '{"Id": "agent-running", "Status": "running", "Archived": false, "Cwd": "${tmpRepo}", "PendingPermissions": []}'
      exit 0
    elif [ "$ID" = "agent-idle" ] || [ "$ID" = "idl2" ]; then
      echo '{"Id": "agent-idle", "Status": "idle", "Archived": false, "Cwd": "${tmpRepo}", "PendingPermissions": []}'
      exit 0
    elif [ "$ID" = "agent-perm" ] || [ "$ID" = "prm3" ]; then
      echo '{"Id": "agent-perm", "Status": "permission", "Archived": false, "Cwd": "${tmpRepo}", "PendingPermissions": [{"id": "perm_001", "tool": "bash"}]}'
      exit 0
    elif [ "$ID" = "agent-gone" ] || [ "$ID" = "gon4" ]; then
      echo '{"Id": "agent-gone", "Status": "archived", "Archived": true, "Cwd": "${tmpRepo}", "PendingPermissions": []}'
      exit 0
    else
      echo '{"error": {"code": "NOT_FOUND", "message": "agent not found"}}' >&2
      exit 1
    fi
    ;;
  *)
    exit 1
    ;;
esac
`;
  const paseoBinPath = path.join(tmpBin, 'paseo');
  fs.writeFileSync(paseoBinPath, paseoScript, { mode: 0o755 });

  const customPath = `${tmpBin}:${process.env.PATH}`;

  // 4. Spawn fb serve
  console.log('Spawning fb serve with paseo adapter...');
  const server = spawn(fbBin, ['--json', 'serve', '--port', '7999'], {
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PATH: customPath,
    },
  });

  let token = null;
  let port = 7999;

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

  console.log(`fb serve running on port ${port} with paseo adapter`);

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
        PATH: customPath,
        ...extraEnv,
      },
      encoding: 'utf-8',
    });
  }

  // 5. Seed 4 tasks for each runtime state
  console.log('Seeding tasks for 4 runtime states...');

  // Task 1: Running
  const t1Res = JSON.parse(runCli('--as-human add "Task with Running agent" --prio 1 --goal "Execute background job"'));
  const t1Id = t1Res.data.task.id;
  execSync(`"${fbBin}" claim ${t1Id}`, {
    cwd: tmpRepo,
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PATH: customPath,
      PASEO_AGENT_ID: 'agent-running',
      PASEO_AI_AGENT: 'codex',
      PASEO_AGENT_EXTERNAL_ID: 'run1',
    },
  });

  // Task 2: Idle
  const t2Res = JSON.parse(runCli('--as-human add "Task with Idle agent" --prio 2 --goal "Wait for input"'));
  const t2Id = t2Res.data.task.id;
  execSync(`"${fbBin}" claim ${t2Id}`, {
    cwd: tmpRepo,
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PATH: customPath,
      PASEO_AGENT_ID: 'agent-idle',
      PASEO_AI_AGENT: 'claude',
      PASEO_AGENT_EXTERNAL_ID: 'idl2',
    },
  });

  // Task 3: Waiting for Permission
  const t3Res = JSON.parse(runCli('--as-human add "Task Waiting for Permission" --prio 1 --goal "Execute sensitive tool"'));
  const t3Id = t3Res.data.task.id;
  execSync(`"${fbBin}" claim ${t3Id}`, {
    cwd: tmpRepo,
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PATH: customPath,
      PASEO_AGENT_ID: 'agent-perm',
      PASEO_AI_AGENT: 'antigravity',
      PASEO_AGENT_EXTERNAL_ID: 'prm3',
    },
  });

  // Task 4: Gone / Archived (stale from runtime)
  const t4Res = JSON.parse(runCli('--as-human add "Task with Gone agent" --prio 2 --goal "Agent closed session"'));
  const t4Id = t4Res.data.task.id;
  execSync(`"${fbBin}" claim ${t4Id}`, {
    cwd: tmpRepo,
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PATH: customPath,
      PASEO_AGENT_ID: 'agent-gone',
      PASEO_AI_AGENT: 'codex',
      PASEO_AGENT_EXTERNAL_ID: 'gon4',
    },
  });

  // 6. Launch browser with Playwright
  console.log('Launching Playwright browser...');
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
  });
  const page = await context.newPage();
  page.on('console', (msg) => console.log('PAGE LOG:', msg.text()));

  // Login
  await page.goto(`http://127.0.0.1:${port}/`);
  await page.waitForSelector('input#token-input');
  await page.fill('input#token-input', token);
  await page.click('button[type="submit"]');

  await page.waitForSelector(`[data-card-id="${t1Id}"]`);
  await page.waitForTimeout(600);
  console.log('✓ Logged in and board rendered');

  // Verify Task 1 (Running)
  const t1Badge = page.locator(`[data-card-id="${t1Id}"] [data-runtime-status="running"]`);
  await t1Badge.waitFor({ state: 'visible' });
  const t1Text = await t1Badge.innerText();
  console.log(`✓ Running badge text: "${t1Text}"`);
  if (!t1Text.includes('running')) throw new Error(`Expected "running", got "${t1Text}"`);

  // Verify Task 2 (Idle)
  const t2Badge = page.locator(`[data-card-id="${t2Id}"] [data-runtime-status="idle"]`);
  await t2Badge.waitFor({ state: 'visible' });
  const t2Text = await t2Badge.innerText();
  console.log(`✓ Idle badge text: "${t2Text}"`);
  if (!t2Text.includes('idle')) throw new Error(`Expected "idle", got "${t2Text}"`);

  // Verify Task 3 (Waiting for permission)
  const t3Badge = page.locator(`[data-card-id="${t3Id}"] [data-runtime-status="permission"]`);
  await t3Badge.waitFor({ state: 'visible' });
  const t3Text = await t3Badge.innerText();
  console.log(`✓ Permission badge text: "${t3Text}"`);
  if (!t3Text.includes('waiting-for-permission (1)')) {
    throw new Error(`Expected "waiting-for-permission (1)", got "${t3Text}"`);
  }

  // Verify Needs-me chip for Task 3
  const t3Chip = page.locator('[aria-label="Needs me strip"] button', { hasText: t3Id });
  await t3Chip.waitFor({ state: 'visible' });
  const t3ChipText = await t3Chip.innerText();
  console.log(`✓ Needs-me chip for permission: "${t3ChipText}"`);
  if (!t3ChipText.includes('1 permission pending')) {
    throw new Error(`Expected "1 permission pending" chip, got "${t3ChipText}"`);
  }

  // Verify Task 4 (Gone - Stale from runtime)
  const t4Badge = page.locator(`[data-card-id="${t4Id}"] [data-runtime-status="gone"]`);
  await t4Badge.waitFor({ state: 'visible' });
  const t4Text = await t4Badge.innerText();
  console.log(`✓ Gone badge text: "${t4Text}"`);
  if (!t4Text.includes('gone')) throw new Error(`Expected "gone", got "${t4Text}"`);

  // Verify stale flag on Task 4 card
  const t4StaleFlag = page.locator(`[data-card-id="${t4Id}"] [title*="Stale: runtime agent gone"]`);
  await t4StaleFlag.waitFor({ state: 'visible' });
  const t4StaleText = await t4StaleFlag.innerText();
  console.log(`✓ Stale flag text: "${t4StaleText}"`);
  if (!t4StaleText.includes('gone')) throw new Error(`Expected stale flag to show "gone", got "${t4StaleText}"`);

  // Verify Needs-me chip for Task 4
  const t4Chip = page.locator('[aria-label="Needs me strip"] button', { hasText: t4Id });
  await t4Chip.waitFor({ state: 'visible' });
  const t4ChipText = await t4Chip.innerText();
  console.log(`✓ Needs-me chip for stale gone: "${t4ChipText}"`);
  if (!t4ChipText.includes('stale · agent gone')) {
    throw new Error(`Expected "stale · agent gone" chip, got "${t4ChipText}"`);
  }

  // 7. Capture comprehensive screenshots
  console.log('Capturing screenshots...');

  // Board overview showing all 4 runtime states
  const boardScreenshotPath = path.join(screenshotsDir, 'runtime-states.png');
  await page.screenshot({ path: boardScreenshotPath });
  console.log(`✓ Saved board screenshot with all 4 runtime states: ${boardScreenshotPath}`);

  // Individual card screenshots
  await page.locator(`[data-card-id="${t1Id}"]`).screenshot({
    path: path.join(screenshotsDir, 'runtime-card-running.png'),
  });
  console.log('✓ Saved runtime-card-running.png');

  await page.locator(`[data-card-id="${t2Id}"]`).screenshot({
    path: path.join(screenshotsDir, 'runtime-card-idle.png'),
  });
  console.log('✓ Saved runtime-card-idle.png');

  await page.locator(`[data-card-id="${t3Id}"]`).screenshot({
    path: path.join(screenshotsDir, 'runtime-card-permission.png'),
  });
  console.log('✓ Saved runtime-card-permission.png');

  await page.locator(`[data-card-id="${t4Id}"]`).screenshot({
    path: path.join(screenshotsDir, 'runtime-card-gone.png'),
  });
  console.log('✓ Saved runtime-card-gone.png');

  // Open CardDetail for Task 3 (Waiting for permission)
  console.log('Opening CardDetail for Task 3...');
  await page.click(`[data-card-id="${t3Id}"]`);
  const dialog = page.locator('[role="dialog"]');
  await dialog.waitFor({ state: 'visible' });
  await page.waitForTimeout(500);

  // Verify CardDetail displays runtime status and permissions
  const detailBadge = dialog.locator('[data-runtime-status="permission"]');
  await detailBadge.first().waitFor({ state: 'visible' });
  console.log('✓ CardDetail displays runtime status badge');

  const detailScreenshotPath = path.join(screenshotsDir, 'runtime-detail-permission.png');
  await page.screenshot({ path: detailScreenshotPath });
  console.log(`✓ Saved card detail screenshot: ${detailScreenshotPath}`);

  await browser.close();
  server.kill('SIGTERM');

  // Clean up temp dirs
  fs.rmSync(tmpHome, { recursive: true, force: true });
  fs.rmSync(tmpRepo, { recursive: true, force: true });
  fs.rmSync(tmpBin, { recursive: true, force: true });

  console.log('============================================');
  console.log('ALL RUNTIME STATUS TESTS PASSED AND VERIFIED');
  console.log('============================================');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
