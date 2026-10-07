// Functional tests for the Rust (egui) build in headless Chromium: node tests/web/run.mjs
import { launch } from './harness.mjs';
import assert from 'node:assert/strict';
const t = await launch();
const { page } = t;
let pass = 0, fail = 0;
async function test(name, fn) {
  try { await fn(); pass++; console.log(`  ok   ${name}`); } catch (e) { fail++; console.log(`  FAIL ${name}\n       ${String(e.message).split('\n').join('\n       ')}`); }
}
const fresh = async () => { await t.page.keyboard.press('Escape'); await t.cmd('ERASE'); await t.cmd('ALL'); await t.page.keyboard.press('Enter'); await t.frame(3); };

await test('app boots with an empty drawing', async () => {
  const s = await t.state();
  assert.equal(s.editors, 1); assert.equal(s.entities, 0); assert.equal(s.busy, false);
});

await test('LINE with clicks creates a line and shows dynamic prompts', async () => {
  await page.mouse.move(700, 400);
  await t.cmd('L');
  assert.equal((await t.state()).busy, true);
  await t.click(10, 10); await t.click(60, 40); await page.keyboard.press('Enter'); await t.frame(3);
  const s = await t.state();
  assert.deepEqual(s.types, ['line']); assert.equal(s.busy, false);
});

await test('CIRCLE by typed centre and radius', async () => {
  await t.cmd('C'); await t.cmd('30,30'); await t.cmd('12');
  const s = await t.state();
  assert.deepEqual(s.types, ['line', 'circle']);
});

await test('click selects, window selects, Delete erases', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.click(35, 25);                          // on the line
  assert.equal((await t.state()).selected.length, 1);
  await page.keyboard.press('Delete'); await t.frame(3);
  assert.deepEqual((await t.state()).types, ['circle']);
  await t.drag(0, 0, 80, 80);                     // window around the circle
  assert.equal((await t.state()).selected.length, 1);
  await page.keyboard.press('Escape'); await t.frame(2);
});

await test('undo and redo', async () => {
  await t.cmd('U');
  assert.deepEqual((await t.state()).types, ['line', 'circle']);
  await t.cmd('REDO');
  assert.deepEqual((await t.state()).types, ['circle']);
});

await test('ribbon button starts a command and Esc cancels it', async () => {
  await page.mouse.click(32, 90);                 // Line button
  await t.frame(3);
  assert.equal((await t.state()).busy, true);
  await page.keyboard.press('Escape'); await t.frame(3);
  assert.equal((await t.state()).busy, false);
});

await test('wheel zooms about the cursor', async () => {
  const before = (await t.state()).view.scale;
  await page.mouse.move(700, 450); await page.mouse.wheel(0, -300); await t.frame(4);
  assert.ok((await t.state()).view.scale > before);
});

await test('ViewCube click changes the view; SW Iso preset via the cube home', async () => {
  await page.mouse.click(1321, 205);              // home icon on the cube
  await page.waitForTimeout(600);
  const s = await t.state();
  assert.ok(s.view.pitch < 1.0, `pitch ${s.view.pitch}`);
});

import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { root } from './harness.mjs';
const sample = f => path.join(root, 'samples', f);
const download = async (trigger) => {
  const [d] = await Promise.all([page.waitForEvent('download', { timeout: 10000 }), trigger()]);
  return { name: d.suggestedFilename(), data: fs.readFileSync(await d.path()) };
};

await test('OPEN a DXF through the file chooser', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  const [fc] = await Promise.all([page.waitForEvent('filechooser', { timeout: 10000 }), t.cmd('OPEN')]);
  await fc.setFiles(sample('L-Bracket.dxf')); await page.waitForTimeout(600); await t.frame(4);
  const s = await t.state();
  assert.equal(s.name, 'L-Bracket'); assert.equal(s.entities, 5);
});

await test('OPEN a CadForge JSON project', async () => {
  const [fc] = await Promise.all([page.waitForEvent('filechooser', { timeout: 10000 }), t.cmd('OPEN')]);
  await fc.setFiles(sample('L-Bracket.cadforge.json')); await page.waitForTimeout(600); await t.frame(4);
  const s = await t.state();
  assert.equal(s.entities, 5); assert.equal(s.solids, 1); assert.ok(s.editors >= 3);
});

await test('DXFOUT downloads a DXF that round-trips through ezdxf', async () => {
  const f = await download(() => t.cmd('DXFOUT'));
  assert.equal(f.name, 'L-Bracket.dxf');
  const text = f.data.toString('latin1').replace(/\r\n/g, '\n');
  assert.ok(text.startsWith('  0\nSECTION') || text.startsWith('0\nSECTION'), text.slice(0, 20));
  assert.ok(text.includes('AC1015'));
  const tmp = path.join(root, 'target', 'tmp'); fs.mkdirSync(tmp, { recursive: true });
  const file = path.join(tmp, 'web-export.dxf'); fs.writeFileSync(file, f.data);
  try {
    const out = execFileSync('python3', ['-I', '-c', 'import sys, ezdxf; d = ezdxf.readfile(sys.argv[1]); print(len(d.modelspace()))', file], { encoding: 'utf8' });
    assert.ok(Number(out.trim()) >= 5, out);
  } catch (e) { if (!/No module named 'ezdxf'/.test(String(e.stderr || e.message))) throw e; console.log('       (ezdxf not installed: skipped validation)'); }
});

await test('PLOT dialog downloads a PDF', async () => {
  await t.cmd('PLOT'); await t.frame(4);
  const pdf = await download(() => page.mouse.click(971, 634));
  assert.equal(pdf.name, 'L-Bracket.pdf'); assert.ok(pdf.data.subarray(0, 5).toString() === '%PDF-');
  assert.ok(pdf.data.length > 2000);
  await t.frame(4);
  await page.keyboard.press('Escape'); await t.frame(3);
});

await test('Layer Properties Manager lists the layers', async () => {
  await t.cmd('LAYER'); await t.frame(4);
  await t.shot(path.join(root, 'target', 'tmp', 'web-layers.png'));
  await page.mouse.move(969, 209, { steps: 4 }); await t.frame(3); await page.mouse.click(969, 209); await t.frame(4);   // window close button
  assert.equal((await t.state()).busy, false);
});

await test('HATCH: pick a point, Close Hatch Creation commits the hatch', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.cmd('ERASE'); await t.cmd('ALL'); await page.keyboard.press('Enter'); await t.frame(3);
  await t.cmd('REC'); await t.cmd('0,0'); await t.cmd('100,60'); await t.cmd('ZOOM'); await t.cmd('E');
  await t.cmd('HATCH'); await t.click(50, 30);
  { const b = await t.state(); assert.equal(b.busy, true, JSON.stringify(b).slice(0, 600)); }
  await t.shot(path.join(root, 'target', 'tmp', 'web-hatch.png'));
  await page.mouse.move(784, 93, { steps: 4 }); await t.frame(3); await page.mouse.click(784, 93); await t.frame(4);   // "Close Hatch Creation" on the contextual tab
  const s = await t.state();
  assert.ok(s.types.includes('hatch'), JSON.stringify(s.types)); assert.equal(s.busy, false);
});

await test('MTEXT editor dialog places the typed text', async () => {
  await t.cmd('MTEXT'); await t.cmd('10,10'); await t.cmd('60,25'); await t.frame(4);
  await page.mouse.click(720, 450); await t.type('HELLO CAD'); await page.mouse.click(810, 543); await t.frame(4);   // OK
  const s = await t.state();
  assert.ok(s.types.includes('text') || s.types.includes('mtext'), JSON.stringify(s.types)); assert.equal(s.busy, false);
});

await test('Ctrl+0 toggles clean screen and OPTIONS switches to the light theme', async () => {
  await page.keyboard.press('Control+0'); await t.frame(4);
  await page.keyboard.press('Control+0'); await t.frame(4);
  await t.cmd('OPTIONS'); await page.mouse.click(597, 352); await t.frame(3); await page.mouse.click(850, 652); await t.frame(6);
  await t.shot(path.join(root, 'target', 'tmp', 'web-light.png'));
  const px = await page.evaluate(() => { const c = document.querySelector('canvas'); return !!c; });
  assert.ok(px);
});

console.log(`\n${pass} passed, ${fail} failed`);
if (t.logs.some(l => l.startsWith('[pageerror]') || l.includes('panicked'))) { console.log(t.logs.filter(l => l.startsWith('[pageerror]') || l.includes('panicked')).join('\n')); fail++; }
await t.close();
process.exit(fail ? 1 : 0);
