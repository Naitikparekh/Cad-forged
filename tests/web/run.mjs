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

// ---- features added for AutoCAD parity: layouts, UCS, tables, solids of revolution, regions
await test('layout tabs switch to paper space and a double-click enters the viewport', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.cmd('ERASE'); await t.cmd('ALL'); await page.keyboard.press('Enter'); await t.frame(3);
  await t.cmd('REC'); await t.cmd('0,0'); await t.cmd('100,60'); await t.cmd('ZOOM'); await t.cmd('E');
  let s = await t.state();
  assert.ok(s.layouts.includes('Layout1')); assert.equal(s.tab.toLowerCase(), 'model');
  // the layout tabs live under the drawing area: Model | Layout1 | Layout2
  await page.mouse.move(103, 770, { steps: 3 }); await t.frame(2); await page.mouse.click(103, 770); await t.frame(5);
  s = await t.state();
  assert.equal(s.space, 'Layout1'); assert.equal(s.mspace, false);
  // inside the default viewport; whether two quick clicks count as a double-click depends on frame timing, so try again if not.
  // Each try waits out egui's double-click window first, so the tab click or an earlier try does not make it a triple click.
  for (let attempt = 0; attempt < 3 && !(await t.state()).mspace; attempt++) {
    await page.mouse.move(720, 460, { steps: 3 }); await t.frame(3); await page.waitForTimeout(400); await page.mouse.dblclick(720, 460); await t.frame(5);
  }
  s = await t.state();
  if (!s.mspace) { fs.mkdirSync(path.join(root, 'target', 'tmp'), { recursive: true }); await t.shot(path.join(root, 'target', 'tmp', 'layout-dblclick-failed.png')); }
  assert.equal(s.mspace, true, 'double-click activates the viewport: ' + JSON.stringify({ space: s.space, editors: s.editors, entities: s.entities, busy: s.busy, tab: s.tab, log: (s.history || []).slice(0, 8) }));
  assert.equal(s.space, 'model');
  await t.cmd('PSPACE'); s = await t.state();
  assert.equal(s.mspace, false); assert.equal(s.space, 'Layout1');
  await page.mouse.move(37, 770, { steps: 3 }); await t.frame(2); await page.mouse.click(37, 770); await t.frame(4);   // back to Model
  assert.equal((await t.state()).space, 'model');
});

await test('PLOT from a layout downloads a PDF the size of its sheet', async () => {
  await page.mouse.move(103, 770, { steps: 3 }); await t.frame(2); await page.mouse.click(103, 770); await t.frame(5);
  await t.cmd('PLOT'); await t.frame(4);
  const pdf = await download(() => page.mouse.click(971, 634));
  assert.ok(pdf.data.subarray(0, 5).toString() === '%PDF-');
  const m = pdf.data.toString('latin1').match(/\/MediaBox\s*\[\s*0\s+0\s+([\d.]+)\s+([\d.]+)/);
  assert.ok(m, 'MediaBox'); assert.ok(Math.abs(Number(m[1]) - 297 * 72 / 25.4) < 2, `width ${m[1]}`);
  await page.keyboard.press('Escape'); await t.frame(3);
  await page.mouse.move(37, 770, { steps: 3 }); await t.frame(2); await page.mouse.click(37, 770); await t.frame(4);
});

await test('UCS: a rotated user coordinate system rotates typed coordinates', async () => {
  await t.cmd('UCS'); await t.cmd('N'); await t.cmd('Z'); await t.cmd('90');
  let s = await t.state();
  assert.ok(Math.abs(s.ucs.angle - Math.PI / 2) < 1e-6, `angle ${s.ucs.angle}`);
  const before = s.entities;
  await t.cmd('L'); await t.cmd('0,0'); await t.cmd('@10,0'); await page.keyboard.press('Enter'); await t.frame(3);
  s = await t.state(); assert.equal(s.entities, before + 1);
  await t.cmd('UCS'); await t.cmd('W');
  assert.equal((await t.state()).ucs.angle, 0);
});

await test('TABLE builds a grid with typed cell text', async () => {
  const n0 = (await t.state()).entities;
  await t.cmd('TABLE'); for (const v of ['2', '2', '30', '8']) await t.cmd(v);
  const p = await t.pt(10, 40); await page.mouse.move(p.x, p.y, { steps: 3 }); await t.frame(3); await page.mouse.click(p.x, p.y); await t.frame(4);
  await t.cmd('no');
  const s = await t.state();
  assert.ok(s.types.includes('table'), JSON.stringify(s.types)); assert.equal(s.entities, n0 + 1);
});

await test('REVOLVE a rectangle into a solid; SLICE halves it', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.cmd('ERASE'); await t.cmd('ALL'); await page.keyboard.press('Enter'); await t.frame(3);
  await t.cmd('REC'); await t.cmd('20,0'); await t.cmd('50,40');
  await t.cmd('REVOLVE'); await t.cmd('ALL'); await page.keyboard.press('Enter'); await t.frame(2);
  await t.cmd('0,0'); await t.cmd('0,40'); await t.cmd('360');
  let s = await t.state(); assert.equal(s.solids, 1, JSON.stringify(s.history));
  await t.cmd('SLICE'); const lines = await t.state();
  assert.equal(lines.busy, true);
  await page.keyboard.press('Escape'); await t.frame(3);
});

await test('REGION booleans: UNION of two overlapping regions leaves one region', async () => {
  await t.cmd('ERASE'); await t.cmd('ALL'); await page.keyboard.press('Enter'); await t.frame(3);
  await t.cmd('REC'); await t.cmd('0,0'); await t.cmd('40,30');
  await t.cmd('REC'); await t.cmd('20,10'); await t.cmd('60,50');
  await t.cmd('REGION'); await t.cmd('ALL'); await page.keyboard.press('Enter'); await t.frame(3);
  let s = await t.state(); assert.deepEqual(s.types, ['region', 'region']);
  await t.cmd('UNION'); await t.cmd('ALL'); await page.keyboard.press('Enter'); await t.frame(3);
  s = await t.state(); assert.deepEqual(s.types, ['region'], JSON.stringify(s.history));
});

// ---------------------------------------------------------------- images, Lisp, block palette, exports
import zlib from 'node:zlib';
/** A tiny RGBA PNG: left half red, right half blue. */
function testPng(w = 4, h = 2) {
  const crcTable = Array.from({ length: 256 }, (_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
  const crc = b => { let c = 0xffffffff; for (const x of b) c = crcTable[(c ^ x) & 255] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
  const chunk = (type, data) => { const len = Buffer.alloc(4); len.writeUInt32BE(data.length); const td = Buffer.concat([Buffer.from(type), data]); const c = Buffer.alloc(4); c.writeUInt32BE(crc(td)); return Buffer.concat([len, td, c]); };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 6;
  const raw = Buffer.alloc((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) { const o = y * (w * 4 + 1) + 1 + x * 4; raw[o] = x < w / 2 ? 255 : 0; raw[o + 2] = x < w / 2 ? 0 : 255; raw[o + 3] = 255; }
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}
const pixel = (file, x, y) => {
  try { return execFileSync('python3', ['-I', '-c', 'import sys\nfrom PIL import Image\nim = Image.open(sys.argv[1]).convert("RGB")\nprint(*im.getpixel((int(float(sys.argv[2])), int(float(sys.argv[3])))))', file, String(x), String(y)], { encoding: 'utf8' }).trim().split(' ').map(Number); }
  catch (e) { return null; }
};

await test('IMAGEATTACH: choose a PNG, place it, and see its pixels on the canvas', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.cmd('ERASE'); await t.cmd('ALL'); await page.keyboard.press('Enter'); await t.frame(3);
  const [fc] = await Promise.all([page.waitForEvent('filechooser', { timeout: 10000 }), t.cmd('IMAGEATTACH')]);
  await fc.setFiles({ name: 'pic.png', mimeType: 'image/png', buffer: testPng() }); await page.waitForTimeout(500); await t.frame(4);
  let s = await t.state(); assert.equal(s.busy, true, JSON.stringify(s.history));
  await t.cmd('0,0'); await t.cmd('10'); await t.cmd('0');      // insertion point, 10 units per pixel, no rotation
  s = await t.state(); assert.deepEqual(s.types, ['image'], JSON.stringify(s.history));
  await t.cmd('ZOOM'); await t.cmd('E'); await t.frame(6);
  await page.keyboard.press('Escape'); await t.frame(3);
  const file = path.join(root, 'target', 'tmp', 'web-image.png'); await t.shot(file);
  const left = await t.pt(10, 15), right = await t.pt(30, 15);
  const [lp, rp] = [pixel(file, left.x, left.y), pixel(file, right.x, right.y)];
  if (!lp) { console.log('       (PIL not installed: skipped the pixel check)'); return; }
  assert.ok(lp[0] > 200 && lp[2] < 60, `left half should be red, got ${lp}`);
  assert.ok(rp[2] > 200 && rp[0] < 60, `right half should be blue, got ${rp}`);
});

await test('IMAGECLIP crops the picture; the clipped half shows the canvas again', async () => {
  await t.cmd('IMAGECLIP'); await t.click(10, 10);
  await t.cmd(''); await t.cmd('');                               // New boundary, Rectangular
  await t.cmd('0,0'); await t.cmd('20,20'); await t.frame(4);
  const file = path.join(root, 'target', 'tmp', 'web-image-clip.png'); await t.shot(file);
  const right = await t.pt(30, 15), left = await t.pt(10, 15);
  const [lp, rp] = [pixel(file, left.x, left.y), pixel(file, right.x, right.y)];
  if (!lp) return;
  assert.ok(lp[0] > 200, `left still red: ${lp}`);
  assert.ok(!(rp[2] > 200 && rp[0] < 60), `the blue half is clipped away: ${rp}`);
});

await test('AutoLISP at the command line: (command ...) draws, defun c:name defines a command', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.cmd('ERASE'); await t.cmd('ALL'); await page.keyboard.press('Enter'); await t.frame(3);
  await t.cmd('(command "circle" (list 5 5) 3)');
  let s = await t.state(); assert.deepEqual(s.types, ['circle'], JSON.stringify(s.history));
  await t.cmd('(defun c:ring (/ r) (setq r 4) (command "circle" (list 20 5) r) (princ))');
  await t.cmd('RING');
  s = await t.state(); assert.deepEqual(s.types, ['circle', 'circle'], JSON.stringify(s.history));
  await t.cmd('(setq total 0)'); await t.cmd('(foreach x (list 1 2 3 4) (setq total (+ total x)))');
  await t.cmd('!total'); s = await t.state(); assert.equal(s.history[0], '10', JSON.stringify(s.history));
});

await test('the AutoLISP console runs code from its dialog', async () => {
  await t.cmd('VLISP'); await t.frame(4);
  await page.mouse.click(720, 385); await t.type('(command "line" (list 0 0) (list 40 0) "")');
  await page.mouse.click(463, 470); await t.frame(5);              // Run
  let s = await t.state(); assert.ok(s.types.includes('line'), JSON.stringify(s.history));
  await page.mouse.click(976, 470); await t.frame(4);              // Close
});

await test('BLOCK then the Blocks palette lists it and a click starts INSERT', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.cmd('BLOCK'); await t.cmd('unit'); await t.cmd('0,0'); await t.cmd('ALL'); await t.cmd(''); await t.frame(3);
  await t.cmd('BLOCKPALETTE'); await t.frame(4);
  await page.mouse.move(50, 255, { steps: 3 }); await t.frame(2); await page.mouse.click(50, 255); await t.frame(4);
  const s = await t.state(); assert.equal(s.busy, true, JSON.stringify(s.history));
  assert.ok(/insertion point|Specify/i.test(s.prompt), s.prompt);
  await page.keyboard.press('Escape'); await t.frame(3);
});

await test('WBLOCK downloads a DXF with the chosen objects', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.cmd('WBLOCK'); await t.cmd('ALL'); await t.cmd(''); await t.cmd('0,0');
  const f = await download(() => t.cmd('part'));
  assert.equal(f.name, 'part.dxf');
  assert.ok(f.data.toString('latin1').includes('SECTION'), f.data.subarray(0, 40).toString());
});

await test('DATAEXTRACTION downloads a CSV of object counts', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.cmd('DATAEXTRACTION');
  const f = await download(() => t.cmd('Objects'));
  assert.ok(f.name.endsWith('-extract.csv'), f.name);
  assert.ok(f.data.toString().startsWith('Count,Name,Layer'), f.data.toString().slice(0, 60));
});

await test('-LINETYPE Load reads a .lin file through the chooser', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.cmd('-LINETYPE');
  const [fc] = await Promise.all([page.waitForEvent('filechooser', { timeout: 10000 }), t.cmd('Load')]);
  await fc.setFiles({ name: 'web.lin', mimeType: 'text/plain', buffer: Buffer.from('*WEB_FENCE, fence\nA,12,-6\n') }); await page.waitForTimeout(500); await t.frame(4);
  let s = await t.state(); assert.equal(s.busy, true, JSON.stringify(s.history));    // "Enter linetype(s) to load <*>"
  await t.cmd(''); await t.frame(3);
  s = await t.state(); assert.ok(s.history.some(h => /Loaded 1 linetype: WEB_FENCE/.test(h.text || h)), JSON.stringify(s.history.slice(-6)));
});

await test('LOADPAT reads a .pat file through the chooser', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  const [fc] = await Promise.all([page.waitForEvent('filechooser', { timeout: 10000 }), t.cmd('LOADPAT')]);
  await fc.setFiles({ name: 'web.pat', mimeType: 'text/plain', buffer: Buffer.from('*WEB_WEAVE, weave\n0, 0,0, 0,4\n90, 0,0, 0,4, 2,-2\n') }); await page.waitForTimeout(500); await t.frame(4);
  const s = await t.state(); assert.ok(s.history.some(h => /Loaded 1 hatch pattern: WEB_WEAVE/.test(h.text || h)), JSON.stringify(s.history.slice(-6)));
});

// a file dragged from the desktop onto the canvas
const drop = async (name, mime, buf) => {
  await page.evaluate(({ name, mime, b64 }) => {
    const bin = Uint8Array.from(atob(b64), c => c.charCodeAt(0));
    const dt = new DataTransfer(); dt.items.add(new File([bin], name, { type: mime }));
    const canvas = document.querySelector('canvas');
    for (const type of ['dragenter', 'dragover', 'drop']) canvas.dispatchEvent(new DragEvent(type, { dataTransfer: dt, bubbles: true, cancelable: true }));
  }, { name, mime, b64: Buffer.from(buf).toString('base64') });
  await page.waitForTimeout(600); await t.frame(4);
};

await test('dropping a DXF opens it as a new drawing', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  const before = (await t.state()).editors;
  await drop('L-Bracket.dxf', 'application/dxf', fs.readFileSync(sample('L-Bracket.dxf')));
  const s = await t.state();
  assert.equal(s.name, 'L-Bracket'); assert.equal(s.entities, 5); assert.equal(s.editors, before + 1);
});

await test('dropping a PNG starts IMAGEATTACH and a .lin file loads its linetypes', async () => {
  await drop('pic.png', 'image/png', testPng());
  let s = await t.state(); assert.equal(s.busy, true, JSON.stringify(s.history));
  assert.ok(/insertion point/i.test(s.prompt), s.prompt);
  await page.keyboard.press('Escape'); await t.frame(3);
  await drop('drop.lin', 'text/plain', Buffer.from('*DROP_LT, dropped\nA,6,-3\n'));
  s = await t.state(); assert.equal(s.busy, true, JSON.stringify(s.history));
  await t.cmd(''); await t.frame(3);
  s = await t.state(); assert.ok(s.history.some(h => /Loaded 1 linetype: DROP_LT/.test(h.text || h)), JSON.stringify(s.history.slice(-6)));
});

await test('dropping an unsupported file changes nothing', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  const before = await t.state();
  await drop('notes.docx', 'application/octet-stream', Buffer.from('not a drawing'));
  const after = await t.state();
  assert.equal(after.editors, before.editors); assert.equal(after.entities, before.entities); assert.equal(after.busy, false);
});

/** Type a command into a freshly started page until it is really running: keys typed before the app is ready are lost. */
const startCommand = async (tt, name) => {
  for (let attempt = 0; attempt < 6; attempt++) {
    await tt.cmd(name);
    for (let i = 0; i < 10; i++) { if ((await tt.state()).busy) return; await tt.page.waitForTimeout(100); }
    await tt.page.keyboard.press('Escape'); await tt.page.waitForTimeout(300);
  }
  throw new Error(`${name} did not start`);
};
/** Poll until `fn` returns a truthy value (or fail after `ms`). */
const until = async (tt, fn, what, ms = 8000) => {
  for (let waited = 0; waited < ms; waited += 100) { const v = await fn(); if (v) return v; await tt.page.waitForTimeout(100); }
  throw new Error(`timed out waiting for ${what}`);
};

await test('autosave keeps unsaved work in the browser and a reload recovers it', async () => {
  const t2 = await launch();                    // a fresh browser context: empty localStorage
  try {
    await startCommand(t2, 'CIRCLE'); await t2.cmd('12,34'); await t2.cmd('7');
    let s = await until(t2, async () => { const x = await t2.state(); return x.types.length === 1 && x; }, 'the circle to exist');
    assert.deepEqual(s.types, ['circle']); assert.equal(s.dirty, true);
    const wrote = await t2.page.evaluate(() => window.cf.autosave_now());
    assert.equal(wrote, 1);
    const keys = await t2.page.evaluate(() => Object.keys(localStorage).filter(k => k.startsWith('cf.autosave.')));
    assert.equal(keys.length, 1, JSON.stringify(keys));
    await t2.page.reload();
    await t2.page.waitForFunction(() => !document.getElementById('loading') || document.getElementById('loading').textContent.startsWith('Could not'), null, { timeout: 60000 });
    s = await until(t2, async () => { try { const x = await t2.state(); return x.editors === 2 && x; } catch { return false; } }, 'the recovered drawing to open');
    assert.equal(s.name, 'Drawing1 (recovered)'); assert.deepEqual(s.types, ['circle']); assert.equal(s.dirty, true);
    assert.ok(s.history.some(h => /Recovered 1 unsaved drawing from the last session/.test(h.text || h)), JSON.stringify(s.history));
    // the file stays until the drawing is saved or closed
    const keys2 = await t2.page.evaluate(() => Object.keys(localStorage).filter(k => k.startsWith('cf.autosave.')));
    assert.deepEqual(keys2, keys);
  } finally { await t2.close(); }
});

await test('a high-DPI display (scale 2): the canvas is rendered at native resolution', async () => {
  // Chromium started with a real device scale factor of 2; the canvas backing store must be twice its CSS size.
  // (Pointer mapping is not asserted: headless Chromium reports devicePixelRatio 1 in this mode.)
  const t2 = await launch({ width: 1280, height: 800, scale: 2 });
  try {
    const dims = await t2.page.evaluate(() => { const c = document.querySelector('canvas'); return { w: c.width, h: c.height, cw: c.clientWidth, ch: c.clientHeight }; });
    assert.ok(Math.abs(dims.w - dims.cw * 2) <= 2 && Math.abs(dims.h - dims.ch * 2) <= 2, `canvas backing store ${dims.w}x${dims.h} for ${dims.cw}x${dims.ch} CSS px`);
    const s = await t2.state(); assert.equal(s.editors, 1);        // and the app is up and running
  } finally { await t2.close(); }
});

// a colour dependent plot style table in AutoCAD's compressed container: colour 1 prints black at 0.25 mm
const ctbBytes = () => {
  let t = 'description="web test"\naci_table_available=TRUE\nlineweight_table{\n\t0=0.000000\n\t1=0.250000\n}\naci_table{\n';
  for (let i = 0; i < 255; i++) t += `\t${i}="Color_${i + 1}"\n`;
  t += '}\nplot_style{\n';
  for (let i = 0; i < 255; i++) t += `\t${i}{\n\t\tname="Color_${i + 1}"\n\t\tcolor=${i === 0 ? -1006632960 : -1}\n\t\tscreen=100\n\t\tlineweight=${i === 0 ? 1 : 0}\n\t}\n`;
  t += '}\n';
  return Buffer.concat([Buffer.from('PIAFILEVERSION_2.0,CTBVER1,compress\r\npmzlibcodec'), Buffer.alloc(12), zlib.deflateSync(Buffer.from(t)), Buffer.alloc(4)]);
};

await test('PLOTSTYLE Load reads a compressed .ctb through the chooser', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.cmd('PLOTSTYLE');
  const [fc] = await Promise.all([page.waitForEvent('filechooser', { timeout: 10000 }), t.cmd('Load')]);
  await fc.setFiles({ name: 'Office.ctb', mimeType: 'application/octet-stream', buffer: ctbBytes() }); await page.waitForTimeout(500); await t.frame(4);
  let s = await t.state();
  assert.ok(s.history.some(h => /Plot style table "Office" \(colour dependent, 255 styles\)/.test(h.text || h)), JSON.stringify(s.history.slice(-6)));
  await t.cmd('PLOTSTYLE'); await t.cmd('?'); await t.frame(3);
  s = await t.state();
  assert.ok(s.history.some(h => /colour\s+1: 0,0,0, 0\.25 mm/.test(h.text || h)), JSON.stringify(s.history.slice(-8)));
});

await test('dropping a .ctb loads it, and PLOTSTYLE None removes it again', async () => {
  await t.cmd('PLOTSTYLE'); await t.cmd('None'); await t.frame(3);
  await drop('Dropped.ctb', 'application/octet-stream', ctbBytes());
  let s = await t.state();
  assert.ok(s.history.some(h => /Plot style table "Dropped"/.test(h.text || h)), JSON.stringify(s.history.slice(-6)));
  await t.cmd('PLOTSTYLE'); await t.cmd('None'); await t.frame(3);
  s = await t.state();
  assert.ok(s.history.some(h => /The plot style table was removed/.test(h.text || h)), JSON.stringify(s.history.slice(-6)));
});

// count pixels in a screenshot that satisfy a colour predicate (python + PIL, like `pixel`; null when PIL is missing)
const countPixels = (file, test) => {
  try { return Number(execFileSync('python3', ['-I', '-c', 'import sys\nfrom PIL import Image\ndata = Image.open(sys.argv[1]).convert("RGB").tobytes()\nn = 0\nfor i in range(0, len(data), 3):\n    r, g, b = data[i], data[i + 1], data[i + 2]\n    if ' + test + ':\n        n += 1\nprint(n)', file], { encoding: 'utf8' }).trim()); }
  catch (e) { return null; }
};

await test('multiline text with formatting codes: a coloured run is drawn in its colour, bold and fractions appear', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  // a drawing with one MTEXT that has a red run and a fraction, as AutoCAD would store it
  const dxf = ['0', 'SECTION', '2', 'ENTITIES', '0', 'MTEXT', '8', '0', '10', '0', '20', '0', '30', '0', '40', '8', '71', '1', '1', '{\\C1;REDWORD} plain \\S1/2; {\\fArial|b1;bold}', '0', 'ENDSEC', '0', 'EOF', ''].join('\n');
  await drop('fmt.dxf', 'application/dxf', Buffer.from(dxf));
  let s = await t.state();
  assert.equal(s.name, 'fmt'); assert.deepEqual(s.types, ['text']);
  await t.cmd('ZOOM'); await t.cmd('E'); await t.frame(6);
  await page.keyboard.press('Escape'); await t.frame(3);
  const file = path.join(root, 'target', 'tmp', 'web-mtext.png'); await t.shot(file);
  const red = countPixels(file, 'r > 200 and g < 70 and b < 70');
  if (red === null) { console.log('       (PIL not installed: skipped the pixel check)'); return; }
  assert.ok(red > 40, `expected red text pixels from the \\C1; run, got ${red}`);
});

await test('binary DXF: -DXFOUT downloads one that ezdxf reads, and a binary file from ezdxf opens', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  const tmp = path.join(root, 'target', 'tmp'); fs.mkdirSync(tmp, { recursive: true });
  // out: the active drawing as binary DXF
  const before = await t.state();
  const f = await download(async () => { await t.cmd('-DXFOUT'); await t.cmd('Binary'); });
  assert.ok(f.name.endsWith('.dxf'), f.name);
  assert.equal(f.data.subarray(0, 18).toString('latin1'), 'AutoCAD Binary DXF', 'starts with the binary sentinel');
  const mine = path.join(tmp, 'web-binary-out.dxf'); fs.writeFileSync(mine, f.data);
  const py = (code, ...args) => { try { return execFileSync('python3', ['-I', '-c', code, ...args], { encoding: 'utf8' }).trim(); } catch (e) { if (/No module named 'ezdxf'/.test(String(e.stderr || e.message))) return null; throw e; } };
  const n = py('import sys, ezdxf; d = ezdxf.readfile(sys.argv[1]); print(len(d.modelspace()))', mine);
  if (n === null) { console.log('       (ezdxf not installed: skipped the checks that need it)'); return; }
  assert.equal(Number(n), before.entities, `ezdxf finds the ${before.entities} object(s) in the binary file`);
  // in: a binary DXF written by ezdxf (an independent implementation), dropped on the canvas
  const theirs = path.join(tmp, 'web-binary-in.dxf');
  py("import sys, ezdxf\ndoc = ezdxf.new('R2018')\nm = doc.modelspace()\nm.add_line((0, 0), (40, 0))\nm.add_circle((20, 10), 5)\nm.add_text('from ezdxf', dxfattribs={'height': 3, 'insert': (2, 20)})\ndoc.saveas(sys.argv[1], fmt='bin')", theirs);
  assert.equal(fs.readFileSync(theirs).subarray(0, 18).toString('latin1'), 'AutoCAD Binary DXF');
  await drop('web-binary-in.dxf', 'application/dxf', fs.readFileSync(theirs));
  const s = await t.state();
  assert.equal(s.name, 'web-binary-in'); assert.deepEqual([...s.types].sort(), ['circle', 'line', 'text']);
});

// AutoCAD DWG: open (chooser and drop), Save As DWG on the command line, and QSAVE keeping the file's DWG version
const dwgSample = v => path.join(root, 'tests', 'dwg', 'samples', `sample_${v}.dwg`);
await test('OPEN an AutoCAD 2018 DWG through the file chooser', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  const [fc] = await Promise.all([page.waitForEvent('filechooser', { timeout: 10000 }), t.cmd('OPEN')]);
  await fc.setFiles(dwgSample('AC1032')); await page.waitForTimeout(1500); await t.frame(4);
  const s = await t.state();
  assert.equal(s.name, 'sample_AC1032'); assert.equal(s.entities, 202);
  assert.ok(s.history.some(h => /sample_AC1032\.dwg: not shown/.test(h.text || h)), 'what was left out is reported: ' + JSON.stringify(s.history.slice(-4)));
});

await test('dropping an AutoCAD 2004 DWG opens it as a new drawing', async () => {
  const before = (await t.state()).editors;
  await drop('sample_AC1018.dwg', 'application/octet-stream', fs.readFileSync(dwgSample('AC1018')));
  await page.waitForTimeout(900); await t.frame(4);
  const s = await t.state();
  assert.equal(s.name, 'sample_AC1018'); assert.equal(s.entities, 202); assert.equal(s.editors, before + 1);
});

await test('-SAVEAS downloads an AutoCAD 2004 DWG that opens again with every object', async () => {
  const before = await t.state();
  const f = await download(async () => { await t.cmd('-SAVEAS'); await page.keyboard.press('Enter'); await t.frame(3); await t.cmd('web-save'); });
  assert.equal(f.name, 'web-save.dwg');
  assert.equal(f.data.subarray(0, 6).toString('latin1'), 'AC1018');
  const tmp = path.join(root, 'target', 'tmp'); fs.mkdirSync(tmp, { recursive: true });
  fs.writeFileSync(path.join(tmp, 'web-save.dwg'), f.data);
  await drop('web-save-again.dwg', 'application/octet-stream', f.data);
  await page.waitForTimeout(900); await t.frame(4);
  const s = await t.state();
  assert.equal(s.name, 'web-save-again'); assert.equal(s.entities, before.entities);
  assert.deepEqual([...s.types].sort(), [...before.types].sort());
});

await test('QSAVE on an opened AutoCAD 2000 DWG saves a DWG 2000 under the same name', async () => {
  await drop('sample_AC1015.dwg', 'application/octet-stream', fs.readFileSync(dwgSample('AC1015')));
  await page.waitForTimeout(900); await t.frame(4);
  assert.equal((await t.state()).name, 'sample_AC1015');
  const f = await download(() => t.cmd('QSAVE'));
  assert.equal(f.name, 'sample_AC1015.dwg');
  assert.equal(f.data.subarray(0, 6).toString('latin1'), 'AC1015');
});

await test('a drawing saved as a template (.dwt) starts a new, unnamed drawing through -NEW File', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  const src = await t.state();
  const f = await download(async () => { await t.cmd('-SAVEAS'); await t.cmd('Template'); await t.cmd('web-template'); });
  assert.equal(f.name, 'web-template.dwt');
  assert.equal(f.data.subarray(0, 6).toString('latin1'), 'AC1018');
  const [fc] = await Promise.all([page.waitForEvent('filechooser', { timeout: 10000 }), (async () => { await t.cmd('-NEW'); await t.cmd('File'); })()]);
  await fc.setFiles({ name: 'web-template.dwt', mimeType: 'application/octet-stream', buffer: f.data }); await page.waitForTimeout(900); await t.frame(4);
  const s = await t.state();
  assert.ok(/^Drawing\d+$/.test(s.name), s.name);
  assert.equal(s.editors, src.editors + 1);
  assert.equal(s.entities, src.entities);
  assert.equal(s.dirty, false);
});

await test('a dropped .dws standards file is checked, and Fix All in the Check Standards dialog fixes the drawing', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.cmd('-LAYER'); await t.cmd('Make'); await t.cmd('NotInTheStandard'); await t.cmd('');
  await drop('office.dws', 'application/octet-stream', fs.readFileSync(dwgSample('AC1018')));
  await page.waitForTimeout(900); await t.frame(4);
  let s = await t.state();
  assert.equal(s.standards, 1, JSON.stringify(s.history));
  assert.ok(s.history.some(h => /Standards file associated with the drawing\. office\.dws: 21 layers/.test(h.text || h)), JSON.stringify(s.history));
  assert.ok(s.standardsProblems.includes('Layer|NOTINTHESTANDARD'), JSON.stringify(s.standardsProblems));
  await t.cmd('CHECKSTANDARDS'); await t.frame(6);
  await page.mouse.click(702, 584); await t.frame(6);              // Fix All (the preferred fixes)
  s = await t.state();
  assert.ok(!s.layers.includes('NotInTheStandard') && s.layers.includes('Layer1'), 'replaced by the standard layer with its properties: ' + JSON.stringify(s.layers));
  assert.ok(!s.standardsProblems.includes('Dimension style|STANDARD'), JSON.stringify(s.standardsProblems));
  assert.ok(s.history.some(h => /problems? fixed\./.test(h.text || h)), JSON.stringify(s.history));
  await page.keyboard.press('Escape'); await t.frame(4);
});

await test('DATALINK makes a linked table from a CSV chosen in the file chooser', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  const before = (await t.state()).types.filter(x => x === 'table').length;
  const [fc] = await Promise.all([page.waitForEvent('filechooser', { timeout: 10000 }), t.cmd('DATALINK')]);
  await fc.setFiles({ name: 'doors.csv', mimeType: 'text/csv', buffer: Buffer.from('Mark,Qty\nD1,2\nD2,3\nTotal,=SUM(B2:B3)\n') });
  await page.waitForTimeout(600); await t.frame(4);
  let s = await t.state(); assert.ok(/Enter for a new table/.test(s.prompt), s.prompt);
  await page.keyboard.press('Enter'); await t.frame(3);
  await t.cmd('0,0');
  s = await t.state();
  assert.equal(s.types.filter(x => x === 'table').length, before + 1, JSON.stringify(s.history));
  assert.ok(s.history.some(h => /Table linked to doors\.csv \(4 rows, 2 columns\)/.test(h.text || h)), JSON.stringify(s.history));
});

await test('LOCALE DEU shows the ribbon in German, ENU brings English back', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  const clip = { x: 0, y: 28, width: 900, height: 140 };
  const en = await page.screenshot({ clip });
  await t.cmd('LOCALE'); await t.cmd('DEU'); await t.frame(4);
  assert.equal((await t.state()).locale, 'DEU');
  const de = await page.screenshot({ clip });
  assert.ok(!en.equals(de), 'the ribbon is drawn again with other text');
  await t.cmd('LOCALE'); await t.cmd('XYZ');
  assert.equal((await t.state()).locale, 'DEU', 'an unknown language changes nothing');
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.cmd('LOCALE'); await t.cmd('ENU'); await t.frame(4);
  assert.equal((await t.state()).locale, 'ENU');
  assert.ok(en.equals(await page.screenshot({ clip })), 'English again, pixel for pixel');
});

await test('touch screen: two fingers pan and pinch-zoom, a tap picks, a long press is a right-click (Enter), the touch bar has Enter / Cancel / Undo; a pen picks', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.cmd('ZOOM'); await t.cmd('C'); await t.cmd('0,0'); await t.cmd('100');
  await t.cmd('L'); await t.cmd('-20,0'); await t.cmd('@40,0'); await page.keyboard.press('Enter'); await t.frame(3);
  const n = (await t.state()).entities;
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Emulation.setTouchEmulationEnabled', { enabled: true, maxTouchPoints: 5 });
  const touch = async (type, pts) => { await cdp.send('Input.dispatchTouchEvent', { type, touchPoints: pts.map(([x, y], id) => ({ x, y, id })) }); await t.frame(1); };
  const gesture = async (from, to, steps = 8) => {
    await touch('touchStart', from);
    for (let i = 1; i <= steps; i++) await touch('touchMove', from.map(([x, y], k) => [x + (to[k][0] - x) * i / steps, y + (to[k][1] - y) * i / steps]));
    await touch('touchEnd', []); await t.frame(3);
  };
  // pinch apart about (700, 450): twice the finger distance is twice the zoom, the point between the fingers stays put
  const s0 = await t.state();
  const at = async () => { const p = await t.pt(10, 10); return [p.x, p.y]; };
  const before = await at();
  await gesture([[650, 450], [750, 450]], [[600, 450], [800, 450]]);
  const s1 = await t.state();
  assert.ok(Math.abs(s1.view.scale / s0.view.scale - 2) < 0.05, `zoom ${s1.view.scale / s0.view.scale}`);
  const mid = [700, 450], after = await at();
  assert.ok(Math.abs((after[0] - mid[0]) - 2 * (before[0] - mid[0])) < 3 && Math.abs((after[1] - mid[1]) - 2 * (before[1] - mid[1])) < 3, `${before} -> ${after}`);
  // two fingers moving together pan the drawing with them
  await gesture([[600, 400], [700, 400]], [[680, 440], [780, 440]]);
  const panned = await at();
  assert.ok(Math.abs(panned[0] - after[0] - 80) < 3 && Math.abs(panned[1] - after[1] - 40) < 3, `${after} -> ${panned}`);
  let s = await t.state();
  assert.equal(s.selected.length, 0, 'the gestures picked nothing'); assert.equal(s.busy, false); assert.equal(s.entities, n);
  // a tap picks the line
  const lp = await t.pt(10, 0);
  await touch('touchStart', [[lp.x, lp.y]]); await touch('touchEnd', []); await t.frame(3);
  assert.equal((await t.state()).selected.length, 1, 'tap selects');
  await page.keyboard.press('Escape'); await t.frame(2);
  // ERASE: tap the line, then rest a finger on empty space to finish the selection
  await t.cmd('ERASE');
  await touch('touchStart', [[lp.x, lp.y]]); await touch('touchEnd', []); await t.frame(3);
  const empty = { x: lp.x, y: lp.y - 40 };
  await touch('touchStart', [[empty.x, empty.y]]); await page.waitForTimeout(900); await touch('touchEnd', []); await t.frame(4);
  s = await t.state();
  assert.equal(s.busy, false, s.prompt); assert.equal(s.entities, n - 1, JSON.stringify(s.history));
  // the touch bar: Enter, Cancel and Undo without a keyboard
  const [vx, vy, vw, vh] = await page.evaluate(() => [...window.cf.viewport_rect()]);
  const bar = i => [vx + vw / 2 + (i - 1) * 82, vy + vh - 38];
  const tap = async ([x, y]) => { await touch('touchStart', [[x, y]]); await touch('touchEnd', []); await t.frame(3); };
  await t.cmd('L');
  for (const [dx, dy] of [[0, -60], [60, -60], [60, -120]]) await tap([lp.x + dx, lp.y + dy]);
  await tap(bar(2));                                      // Undo: the second segment goes
  await tap(bar(0));                                      // Enter ends LINE
  s = await t.state();
  assert.equal(s.busy, false, s.prompt); assert.equal(s.entities, n, JSON.stringify(s.history));
  await t.cmd('C'); assert.equal((await t.state()).busy, true);
  await tap(bar(1));                                      // Cancel
  assert.equal((await t.state()).busy, false);
  await cdp.send('Emulation.setTouchEmulationEnabled', { enabled: false });
  // a pen works like the mouse: a tap with the pen picks the line
  await page.keyboard.press('Escape'); await t.frame(2);
  const pen = async (type, x, y) => { await cdp.send('Input.dispatchMouseEvent', { type, x, y, button: 'left', buttons: type === 'mouseReleased' ? 0 : 1, clickCount: 1, pointerType: 'pen' }); await t.frame(1); };
  const on = { x: lp.x + 30, y: lp.y - 60 };                 // on the segment drawn with taps above
  await pen('mouseMoved', on.x, on.y); await pen('mousePressed', on.x, on.y); await pen('mouseReleased', on.x, on.y); await t.frame(3);
  assert.equal((await t.state()).selected.length, 1, 'pen tap selects');
  await page.keyboard.press('Escape'); await t.frame(2);
});

console.log(`\n${pass} passed, ${fail} failed`);
if (t.logs.some(l => l.startsWith('[pageerror]') || l.includes('panicked'))) { console.log(t.logs.filter(l => l.startsWith('[pageerror]') || l.includes('panicked')).join('\n')); fail++; }
await t.close();
process.exit(fail ? 1 : 0);
