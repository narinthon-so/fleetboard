import { chromium } from 'playwright';
import { spawn, execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';

async function main() {
  const tmpHome = fs.mkdtempSync(path.join(os.tmpdir(), 'fb-test-home-'));
  const tmpRepo = fs.mkdtempSync(path.join(os.tmpdir(), 'fb-test-repo-'));
  const screenshotsDir = path.resolve('../docs/screenshots');
  fs.mkdirSync(screenshotsDir, { recursive: true });

  const fbBin = path.resolve('../target/debug/fb');

  console.log('Setting up synthetic git repo...');
  execSync(`git init -b main "${tmpRepo}"`);
  execSync(`git -C "${tmpRepo}" config user.email "bot@example.com"`);
  execSync(`git -C "${tmpRepo}" config user.name "Bot"`);
  execSync(`git -C "${tmpRepo}" commit --allow-empty -m "Initial commit"`);
  execSync(`git -C "${tmpRepo}" remote add origin https://github.com/example/api.git`);

  console.log('Spawning fb serve...');
  const server = spawn(fbBin, ['--json', 'serve', '--port', '7988'], {
    env: {
      ...process.env,
      FB_HOME: tmpHome,
      PATH: process.env.PATH,
    },
  });

  let token = null;
  let port = 7988;

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

  console.log(`fb serve running on port ${port}`);

  function agentEnv(name, extId) {
    const cleanEnv = { ...process.env };
    for (const key of Object.keys(cleanEnv)) {
      if (key.startsWith('PASEO_') || key.startsWith('CLAUDE') || key.startsWith('CODEX')) {
        delete cleanEnv[key];
      }
    }
    return {
      ...cleanEnv,
      FB_HOME: tmpHome,
      FB_TEST_CHAIN: 'bash',
      PASEO_AGENT_ID: `test-agent-${extId}`,
      PASEO_AI_AGENT: name,
      PASEO_AGENT_EXTERNAL_ID: extId,
      PATH: process.env.PATH,
    };
  }

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

  // 1. Seed tasks
  console.log('Seeding initial tasks...');
  // Task 1: Draft
  const t1Res = JSON.parse(
    execSync(`"${fbBin}" --json add "Draft task for promotion test" --prio 2`, {
      cwd: tmpRepo,
      env: agentEnv('codex', '403254b'),
    })
  );
  const t1Id = t1Res.data.task.id;

  // Task 2: Ready
  const t2Res = JSON.parse(runCli('--as-human add "Ready task to test invalid drag" --prio 2'));
  const t2Id = t2Res.data.task.id;

  // Task 3: In progress with commits
  const t3Res = JSON.parse(
    runCli(
      '--as-human add "In progress task with commits" --prio 1 --goal "Complete feature" --ac "step 1" --ac "step 2"'
    )
  );
  const t3Id = t3Res.data.task.id;
  execSync(`"${fbBin}" claim ${t3Id}`, {
    cwd: tmpRepo,
    env: agentEnv('codex', '403254b'),
  });
  execSync(`git -C "${tmpRepo}" checkout -b task/${t3Id}`);
  execSync(
    `echo "code" > "${tmpRepo}/code.txt" && git -C "${tmpRepo}" add . && git -C "${tmpRepo}" commit -m "Commit for ${t3Id}"`
  );
  execSync(`"${fbBin}" show ${t3Id}`, {
    cwd: tmpRepo,
    env: agentEnv('codex', '403254b'),
  });

  // Task 4: Review without commits (no evidence)
  const t4Res = JSON.parse(runCli('--as-human add "Review task without evidence" --prio 0'));
  const t4Id = t4Res.data.task.id;
  execSync(`git -C "${tmpRepo}" checkout main`);
  execSync(`git -C "${tmpRepo}" checkout -b task/${t4Id}`);
  execSync(`"${fbBin}" claim ${t4Id}`, {
    cwd: tmpRepo,
    env: agentEnv('antigravity', '20aabd3'),
  });
  execSync(`"${fbBin}" note ${t4Id} "Handoff note for investigation"`, {
    cwd: tmpRepo,
    env: agentEnv('antigravity', '20aabd3'),
  });
  execSync(`"${fbBin}" done ${t4Id}`, {
    cwd: tmpRepo,
    env: agentEnv('antigravity', '20aabd3'),
  });

  // Task 5: Review WITH commits in tmpRepo2
  const tmpRepo2 = fs.mkdtempSync(path.join(os.tmpdir(), 'fb-test-repo2-'));
  execSync(`git clone "${tmpRepo}" "${tmpRepo2}"`);
  execSync(`git -C "${tmpRepo2}" config user.email "bot@example.com"`);
  execSync(`git -C "${tmpRepo2}" config user.name "Bot"`);

  const t5Res = JSON.parse(runCli('--as-human add "Review task with commits" --prio 1'));
  const t5Id = t5Res.data.task.id;
  execSync(`"${fbBin}" claim ${t5Id}`, {
    cwd: tmpRepo2,
    env: agentEnv('claude', '60a1168'),
  });
  execSync(`git -C "${tmpRepo2}" checkout -b task/${t5Id}`);
  execSync(
    `echo "more" > "${tmpRepo2}/more.txt" && git -C "${tmpRepo2}" add . && git -C "${tmpRepo2}" commit -m "Commit for ${t5Id}"`
  );
  execSync(`"${fbBin}" done ${t5Id}`, {
    cwd: tmpRepo2,
    env: agentEnv('claude', '60a1168'),
  });

  // Launch Playwright
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
  console.log('✓ Successfully logged in and board rendered');

  async function dragAndDrop(sourceLocator, targetLocator) {
    const sourceBox = await sourceLocator.boundingBox();
    const targetBox = await targetLocator.boundingBox();
    if (!sourceBox || !targetBox) throw new Error('Bounding box not found');

    await page.mouse.move(sourceBox.x + sourceBox.width / 2, sourceBox.y + sourceBox.height / 2);
    await page.mouse.down();
    await page.mouse.move(sourceBox.x + sourceBox.width / 2 + 15, sourceBox.y + sourceBox.height / 2 + 15, { steps: 5 });
    await page.waitForTimeout(100);
    await page.mouse.move(targetBox.x + targetBox.width / 2, targetBox.y + targetBox.height / 2, { steps: 10 });
    await page.waitForTimeout(100);
    await page.mouse.up();
  }

  // TEST 1: Invalid Drag (Ready -> In progress) gives reason toast
  console.log('TEST 1: Testing invalid drag (Ready -> In progress)...');
  const readyCard = page.locator(`[data-card-id="${t2Id}"]`);
  const inProgressCol = page.locator('[data-column-id="in_progress"]');
  await dragAndDrop(readyCard, inProgressCol);
  // Expect toast
  await page.waitForSelector('text=has no claim — agents move cards by claiming', { timeout: 3000 });
  console.log('✓ Invalid drag produced reason toast: "has no claim — agents move cards by claiming"');

  // TEST 2: Valid Drag (Draft -> Ready promotes)
  console.log('TEST 2: Testing valid drag (Draft -> Ready)...');
  const draftCard = page.locator(`[data-card-id="${t1Id}"]`);
  const readyCol = page.locator('[data-column-id="ready"]');
  await dragAndDrop(draftCard, readyCol);
  // Wait for card to appear in ready column
  await page.waitForSelector(`[data-column-id="ready"] [data-card-id="${t1Id}"]`, { timeout: 4000 });
  console.log(`✓ ${t1Id} successfully promoted to Ready via drag and drop`);

  // TEST 3: Drag Review without evidence -> Done opens confirmation dialog
  console.log('TEST 3: Testing Review without evidence -> Done (confirm dialog)...');
  const reviewNoEvidenceCard = page.locator(`[data-card-id="${t4Id}"]`);
  const doneCol = page.locator('[data-column-id="done"]');
  await dragAndDrop(reviewNoEvidenceCard, doneCol);

  // Confirmation dialog should be visible
  await page.waitForSelector('text=Verify without evidence', { timeout: 3000 });
  console.log('✓ Confirmation dialog displayed for verifying without evidence');

  // Cancel it first to verify cancel works
  await page.click('button:has-text("Cancel")');
  await page.waitForTimeout(300);

  // Now open card detail for t4Id to test verify confirmation from CardDetail
  console.log('TEST 4: Card detail Verify button with no evidence...');
  await page.click(`[data-card-id="${t4Id}"]`);
  await page.waitForSelector(`role=dialog[name="Task detail for ${t4Id}"]`);

  // Click Verify in CardDetail header
  await page.click('button:has-text("Verify")');
  await page.waitForSelector('text=Verify without evidence', { timeout: 3000 });
  console.log('✓ CardDetail Verify button also asked for confirmation');

  // Confirm with force
  await page.click('button:has-text("Verify with force")');
  await page.waitForSelector('text=verified and marked done', { timeout: 4000 });
  console.log(`✓ Task ${t4Id} verified with force`);

  // Close sheet via close button
  await page.click('button[aria-label="Close task detail"]');
  await page.waitForTimeout(300);

  // TEST 5: Drag Review WITH evidence -> Done verifies directly without dialog
  console.log('TEST 5: Testing Review with evidence -> Done (verifies directly)...');
  const reviewWithEvidenceCard = page.locator(`[data-card-id="${t5Id}"]`);
  await dragAndDrop(reviewWithEvidenceCard, doneCol);
  await page.waitForSelector('text=verified and marked done', { timeout: 4000 });
  console.log(`✓ Task ${t5Id} verified directly via drag and drop`);

  // TEST 6: Card detail tabs and Add comment
  console.log('TEST 6: Testing Card detail tabs, Evidence, and Notes comment...');
  await page.waitForTimeout(600);
  await page.click(`[data-card-id="${t3Id}"]`);
  await page.waitForSelector(`role=dialog[name="Task detail for ${t3Id}"]`, { timeout: 5000 });

  // Overview tab checks
  await page.waitForSelector('text=Goal');
  await page.waitForSelector('text=Acceptance Criteria');

  // Evidence tab
  await page.click('button:has-text("Evidence")');
  await page.waitForSelector('text=Evidence Table');
  await page.waitForSelector('text=1 commits');
  await page.click('button:has-text("Refresh")');
  console.log('✓ Evidence tab and Refresh button verified');

  // Notes tab & Add Comment
  await page.click('button:has-text("Notes")');
  await page.fill('textarea[placeholder="Write a comment or instruction..."]', 'Operator reviewed progress, looks solid.');
  await page.click('button:has-text("Add Comment")');
  await page.waitForSelector('text=Operator reviewed progress, looks solid.', { timeout: 4000 });
  console.log('✓ Notes tab: human comment added and rendered in audit note list');

  // Switch back to overview tab to capture card-detail screenshot
  await page.click('button:has-text("Overview")');
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(screenshotsDir, 'card-detail.png') });
  console.log('✓ Captured card-detail.png');

  // Release claim from CardDetail
  console.log('TEST 7: Testing Release claim from CardDetail...');
  await page.click('button:has-text("Release")');
  await page.waitForSelector('text=Claim released', { timeout: 4000 });
  console.log(`✓ Claim on ${t3Id} released to Ready`);

  // Close card detail via close button
  await page.click('button[aria-label="Close task detail"]');
  await page.waitForTimeout(300);

  // TEST 8: Keyboard shortcuts: '?' opens cheatsheet, 'Esc' closes
  console.log('TEST 8: Testing keyboard shortcuts...');
  await page.keyboard.press('?');
  await page.waitForSelector('text=Keyboard Shortcuts');
  await page.waitForSelector('text=Verify open card (confirm first)');
  await page.waitForSelector('text=Reject open card (with comment)');
  await page.keyboard.press('Escape');
  await page.waitForTimeout(200);
  console.log('✓ Keyboard shortcuts dialog and listing verified');

  // Keyboard navigation: j / k
  await page.keyboard.press('j');
  await page.waitForTimeout(200);

  // Take fresh polished screenshots
  console.log('Capturing final polished screenshots...');
  // 1. Board screenshot
  await page.screenshot({ path: path.join(screenshotsDir, 'board.png') });
  console.log('✓ Captured board.png');

  await browser.close();
  server.kill('SIGTERM');

  fs.rmSync(tmpHome, { recursive: true, force: true });
  fs.rmSync(tmpRepo, { recursive: true, force: true });
  try {
    fs.rmSync(tmpRepo2, { recursive: true, force: true });
  } catch {}

  console.log('\n=========================================');
  console.log('ALL PLAYWRIGHT TESTS PASSED SUCCESSFULLY!');
  console.log('=========================================\n');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
