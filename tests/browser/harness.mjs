// Minimal browser-test harness: boots CadForge.html in headless Chromium and exposes helpers.
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
const require = createRequire('/opt/node-tools/');
export const { chromium } = require('playwright');
export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

export async function boot(browser, { width = 1500, height = 900 } = {}) {
  const page = await browser.newPage({ viewport: { width, height } });
  const errors = [];
  page.on('console', m => { if (m.type() === 'error') errors.push(m.text()); });
  page.on('pageerror', e => errors.push(`${e.message}\n${e.stack?.split('\n').slice(0, 4).join('\n')}`));
  await page.goto(pathToFileURL(path.join(root, 'CadForge.html')).href);
  await page.waitForFunction(() => window.cadforge?.app?.active);
  await page.waitForTimeout(150);
  const t = {
    page, errors,
    ents: () => page.evaluate(() => window.cadforge.app.doc.entities.map(e => JSON.parse(JSON.stringify(e)))),
    sel: () => page.evaluate(() => [...window.cadforge.app.active.sel]),
    log: () => page.evaluate(() => window.cadforge.app.history.slice(-12).map(l => l.text)),
    busy: () => page.evaluate(() => window.cadforge.app.runtime.busy),
    prompt: () => page.evaluate(() => window.cadforge.app.runtime.promptText()),
    /** Type text into the command line and press Enter. */
    async cmd(text) { await page.keyboard.type(text); await page.keyboard.press('Enter'); await page.waitForTimeout(40); },
    /** Screen position (page coordinates) of a world point. */
    async screen(x, y) { return page.evaluate(([x, y]) => { const { ui } = window.cadforge; const r = ui.viewport.el.getBoundingClientRect(), s = ui.editor.view.toScreen({ x, y, z: 0 }); return { x: r.left + s.x, y: r.top + s.y }; }, [x, y]); },
    async click(x, y, opts) { const p = await t.screen(x, y); await page.mouse.move(p.x, p.y); await page.mouse.down(opts); await page.mouse.up(opts); await page.waitForTimeout(40); },
    async move(x, y) { const p = await t.screen(x, y); await page.mouse.move(p.x, p.y); await page.waitForTimeout(20); },
    async drag(x0, y0, x1, y1) { const a = await t.screen(x0, y0), b = await t.screen(x1, y1); await page.mouse.move(a.x, a.y); await page.mouse.down(); await page.mouse.move((a.x + b.x) / 2, (a.y + b.y) / 2, { steps: 4 }); await page.mouse.move(b.x, b.y, { steps: 4 }); await page.mouse.up(); await page.waitForTimeout(60); },
    async fresh() { await page.evaluate(() => { const { app } = window.cadforge; app.runtime.escape(); app.doc.entities.length = 0; app.doc.undoStack.length = 0; app.doc.redoStack.length = 0; app.active.view.setState({ cx: 50, cy: 50, scale: 6 }); app.active.sel.clear(); app.sysvars.set('OSMODE', 0); app.sysvars.set('GRIDMODE', 0); app.emit('change'); window.cadforge.ui.viewport.invalidate(); }); await page.mouse.move(700, 500); await page.waitForTimeout(60); },
  };
  return t;
}

let pass = 0, fail = 0; const failures = [];
export async function test(name, fn) {
  try { await fn(); pass++; console.log(`  ok   ${name}`); }
  catch (e) { fail++; failures.push(name); console.log(`  FAIL ${name}\n       ${String(e.message).split('\n').join('\n       ')}`); }
}
export function summary() { console.log(`\n${pass} passed, ${fail} failed`); return fail === 0; }
export const eq = (a, b, msg = '') => { if (JSON.stringify(a) !== JSON.stringify(b)) throw Error(`${msg} expected ${JSON.stringify(b)}, got ${JSON.stringify(a)}`); };
export const ok = (v, msg = 'assertion failed') => { if (!v) throw Error(msg); };
export const near = (a, b, tol = 1e-6, msg = '') => { if (Math.abs(a - b) > tol) throw Error(`${msg} expected ${b}, got ${a}`); };
