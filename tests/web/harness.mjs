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

export async function launch({ width = 1440, height = 900 } = {}) {
  const { server, url } = await serve();
  const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--use-gl=angle', '--enable-unsafe-swiftshader', '--ignore-gpu-blocklist', '--enable-webgl'] });
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
  const logs = [];
  page.on('console', m => logs.push(`[${m.type()}] ${m.text()}`));
  page.on('pageerror', e => logs.push(`[pageerror] ${e.message}`));
  await page.goto(url);
  // wait until the loading overlay is gone (the app started) or the failure message shows up
  await page.waitForFunction(() => !document.getElementById('loading') || document.getElementById('loading').textContent.startsWith('Could not'), null, { timeout: 60000 });
  await page.waitForTimeout(800);
  const t = {
    page, logs, server, browser,
    async shot(file) { await page.screenshot({ path: file }); },
    async close() { await browser.close(); server.close(); },
    async frame(n = 2) { await page.waitForTimeout(n * 60); },
    async type(text) { await page.keyboard.type(text, { delay: 5 }); await t.frame(); },
    async cmd(text) { await page.keyboard.type(text, { delay: 5 }); await page.keyboard.press('Enter'); await t.frame(3); },
  };
  return t;
}
