import assert from 'node:assert/strict';
import { chromium } from '/home/nsubuntu22/code/fleetboard/web/node_modules/playwright/index.mjs';
import { spawn } from 'node:child_process';
const server = spawn('pnpm', ['exec', 'vite', 'preview', '--host', '127.0.0.1', '--port', '5194', '--strictPort'], { cwd: '/home/nsubuntu22/code/fleetboard/web', stdio: 'pipe' });
let browser;
try {
  await new Promise((resolve, reject) => {
    server.stdout.on('data', d => { if (d.toString().includes('5194')) resolve(); });
    server.on('error', reject);
    server.on('exit', c => reject(new Error(`preview exited ${c}`)));
  });
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', e => errors.push(e.message));
  let login = false;
  await page.route('**/api/**', route => {
    if (new URL(route.request().url()).pathname === '/api/events') return route.fulfill({contentType:'text/event-stream',body:': connected\n\n'});
    if (login) return route.fulfill({status:401,json:{v:1,ok:false,error:{code:'UNAUTHORIZED',message:'Authentication required'}}});
    return route.fulfill({json:{v:1,ok:true,data:{tasks:[],projects:[],actors:[],instances:[]}}});
  });
  for (const screen of ['board','login']) {
    login = screen === 'login';
    for (const width of [1440,375,320]) {
      await page.setViewportSize({width,height:900});
      await page.goto('http://127.0.0.1:5194/');
      if (login) await page.getByRole('heading',{name:'fleetboard auth'}).waitFor();
      else await page.locator('header').waitFor();
      for (const theme of ['dark','light']) {
        await page.evaluate(theme => { document.documentElement.className = theme; }, theme);
        const logo = page.getByRole('img',{name:'fleetboard',exact:true});
        await logo.waitFor();
        const active = logo.locator(theme === 'dark' ? '.brand-logo-dark' : '.brand-logo-light');
        assert(await active.isVisible());
        assert(await active.evaluate(el => el.complete && el.naturalWidth > 0));
        const box = await logo.boundingBox();
        assert(box.width >= 200 && box.x >= 0 && box.x + box.width <= width, `logo clipped ${screen} ${width}`);
        if (!login) assert(await page.getByRole('button',{name:/Switch to/}).isVisible());
        await page.screenshot({path:`/tmp/fb-107-evidence/${screen}-${theme}-${width}.png`,fullPage:true});
      }
    }
  }
  assert.deepEqual(errors,[]);
  console.log('PASS: built dashboard/login, dark/light, 1440/375/320px; loaded artwork, accessible names, visible logos and theme control; no page errors');
} finally { await browser?.close(); server.kill('SIGTERM'); }
