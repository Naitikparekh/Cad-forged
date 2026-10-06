// Loads CadForge.html in headless Chromium, prints console errors and saves a screenshot.
//   node tests/browser/shot.mjs out.png [width height] [--script "js to run before the shot"]
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
const require = createRequire('/opt/node-tools/');
const { chromium } = require('playwright');
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const out = args[0] || 'shot.png';
const w = +args[1] || 1440, hgt = +args[2] || 900;
const si = args.indexOf('--script');
const script = si >= 0 ? args[si + 1] : '';
const browser = await chromium.launch({ executablePath: process.env.PLAYWRIGHT_BROWSERS_PATH ? undefined : undefined });
const page = await browser.newPage({ viewport: { width: w, height: hgt } });
const errors = [];
page.on('console', m => { if (m.type() === 'error' || m.type() === 'warning') errors.push(`[${m.type()}] ${m.text()}`); });
page.on('pageerror', e => errors.push(`[pageerror] ${e.message}\n${e.stack?.split('\n').slice(0, 4).join('\n')}`));
await page.goto(pathToFileURL(path.join(root, 'CadForge.html')).href);
await page.waitForTimeout(400);
if (script) { await page.evaluate(script); await page.waitForTimeout(300); }
await page.screenshot({ path: out });
console.log(errors.length ? errors.join('\n') : 'no console errors');
await browser.close();
