// Browser harness for the Rust (egui) build: serves dist-web/ and drives it in headless Chromium.
import { createRequire } from 'node:module';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const require = createRequire('/opt/node-tools/');
export const { chromium } = require('playwright');
export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json' };

export function serve(dir = path.join(root, 'dist-web')) {
  const server = http.createServer((req, res) => {
    const f = path.join(dir, req.url === '/' ? 'index.html' : decodeURIComponent(req.url.split('?')[0]));
    if (!f.startsWith(dir) || !fs.existsSync(f)) { res.writeHead(404); res.end('not found'); return; }
    res.writeHead(200, { 'content-type': TYPES[path.extname(f)] || 'application/octet-stream' });
    fs.createReadStream(f).pipe(res);
  });
  return new Promise(resolve => server.listen(0, '127.0.0.1', () => resolve({ server, url: `http://127.0.0.1:${server.address().port}/` })));
}

/** `scale` > 1 starts Chromium with a real device scale factor (a HiDPI screen), not Playwright's emulation. */
export async function launch({ width = 1440, height = 900, scale = 1 } = {}) {
  const { server, url } = await serve();
  const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--use-gl=angle', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl', ...(scale > 1 ? [`--force-device-scale-factor=${scale}`] : [])] });
  const page = await browser.newPage({ viewport: { width, height } });
  const logs = [];
  page.on('console', m => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', e => logs.push(`[pageerror] ${e.message}`));
  await page.goto(url);
  // wait until the loading overlay is gone (the app started) or the failure message shows up
  await page.waitForFunction(() => !document.getElementById('loading') || document.getElementById('loading').textContent.startsWith('Could not'), null, { timeout: 60000 });
  await page.waitForTimeout(800);
  // keys typed right after start-up are sometimes lost: prove the input path works (start LINE, cancel it) before any test runs
  for (let i = 0; i < 20; i++) {
    await page.keyboard.type('L', { delay: 5 }); await page.keyboard.press('Enter'); await page.waitForTimeout(150);
    const busy = await page.evaluate(() => { try { return JSON.parse(window.cf.state_json()).busy === true; } catch { return false; } });
    await page.keyboard.press('Escape'); await page.waitForTimeout(150);
    if (busy) break;
  }
  const t = {
    page, logs, server, browser,
    async shot(file) { await page.screenshot({ path: file }); },
    async close() { await browser.close(); server.close(); },
    state: () => page.evaluate(() => JSON.parse(window.cf.state_json())),
    /** Page coordinates of a world point. */
    async pt(x, y) { const [px, py] = await page.evaluate(([x, y]) => [...window.cf.world_to_page(x, y)], [x, y]); return { x: px, y: py }; },
    async click(x, y, o) { const p = await t.pt(x, y); await page.mouse.move(p.x, p.y); await page.mouse.down(o); await page.mouse.up(o); await t.frame(3); },
    async move(x, y, steps = 3) { const p = await t.pt(x, y); await page.mouse.move(p.x, p.y, { steps }); await t.frame(3); },
    async drag(x0, y0, x1, y1) { const a = await t.pt(x0, y0), b = await t.pt(x1, y1); await page.mouse.move(a.x, a.y); await page.mouse.down(); await page.mouse.move((a.x + b.x) / 2, (a.y + b.y) / 2, { steps: 4 }); await page.mouse.move(b.x, b.y, { steps: 4 }); await page.mouse.up(); await t.frame(4); },
    async frame(n = 2) { await page.waitForTimeout(n * 60); },
    async type(text) { await page.keyboard.type(text, { delay: 5 }); await t.frame(); },
    async cmd(text) { await page.keyboard.type(text, { delay: 5 }); await page.keyboard.press('Enter'); await t.frame(3); },
  };
  return t;
}
