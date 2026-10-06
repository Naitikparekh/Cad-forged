import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { exportVector, defaultPlotOptions, plotLayout, pageSize } from '../../src/io/plot.js';
import { sampleDrawing } from '../../src/io/sample.js';
import { RecordingContext } from '../../src/io/recorder.js';
import { Drawing } from '../../src/core/drawing.js';
import { E } from '../../src/core/entities.js';

test('layout fits the drawing on the sheet and keeps it centred', () => {
  const d = sampleDrawing(), o = defaultPlotOptions(d);
  const L = plotLayout(d, o);
  assert.deepEqual(L.page, { w: 297, h: 210 });
  const m = L.worldM, s = L.mmPerUnit;
  const x0 = m[4] / 10 + s * L.bounds.minX, x1 = m[4] / 10 + s * L.bounds.maxX;     // mm on the sheet
  assert.ok(x0 >= 4.99 && x1 <= 297 - 4.99, `x range ${x0}..${x1}`);
  assert.ok(Math.abs(x0 - (297 - x1)) < 1e-6, 'centred horizontally');
  assert.ok(L.fits);
  assert.deepEqual(pageSize({ ...o, landscape: false }), { w: 210, h: 297 });
});

test('fixed scale maps units to millimetres using the drawing units', () => {
  const d = sampleDrawing(); d.settings.units.insUnits = 4;
  const L = plotLayout(d, { ...defaultPlotOptions(d), scale: 1 / 2, paper: 'A2' });
  assert.equal(L.mmPerUnit, 0.5);
  d.settings.units.insUnits = 1;
  assert.equal(plotLayout(d, { ...defaultPlotOptions(d), scale: 1, paper: 'A0' }).mmPerUnit, 25.4);
});

test('empty drawings cannot be plotted', () => { assert.throws(() => plotLayout(new Drawing('x'), defaultPlotOptions(new Drawing('x'))), /nothing to plot/); });

test('recording context flattens arcs and keeps device-space geometry', () => {
  const c = new RecordingContext(0.05);
  c.setTransform(2, 0, 0, 2, 10, 20); c.beginPath(); c.arc(0, 0, 50, 0, Math.PI * 2); c.stroke();
  const p = c.ops[0].paths[0].pts;
  assert.ok(p.length > 40);
  for (const [x, y] of p) assert.ok(Math.abs(Math.hypot(x - 10, y - 20) - 100) < 0.06);
  assert.equal(c.ops[0].t, 'stroke');
});

test('PDF export is well formed and has the expected content', () => {
  const d = sampleDrawing(), r = exportVector(d, { ...defaultPlotOptions(d), format: 'pdf' });
  const s = Buffer.from(r.data).toString('latin1');
  assert.ok(s.startsWith('%PDF-1.4'));
  assert.ok(s.trimEnd().endsWith('%%EOF'));
  const xref = Number(/startxref\n(\d+)/.exec(s)[1]);
  assert.ok(s.slice(xref).startsWith('xref'), 'startxref points at the xref table');
  for (const m of s.matchAll(/(\d{10}) 00000 n/g)) assert.match(s.slice(Number(m[1]), Number(m[1]) + 12), /^\d+ 0 obj/);
  assert.match(s, /MediaBox \[0 0 841\.89/);
  assert.match(s, /\(L-BRACKET\)/);
  assert.equal(r.mime, 'application/pdf');
});

test('SVG export has paths and text', () => {
  const d = sampleDrawing(), r = exportVector(d, { ...defaultPlotOptions(d), format: 'svg' });
  assert.match(r.data, /<svg [^>]*width="297mm"/);
  assert.ok((r.data.match(/<path /g) || []).length > 20);
  assert.match(r.data, />L-BRACKET</);
});

test('monochrome plotting forces black; colour keeps layer colours', () => {
  const d = sampleDrawing();
  const mono = exportVector(d, { ...defaultPlotOptions(d), format: 'svg', mono: true }).data;
  const col = exportVector(d, { ...defaultPlotOptions(d), format: 'svg', mono: false }).data;
  assert.ok(!/stroke="#00ffff"/.test(mono) && /stroke="#00ffff"/.test(col));
});

test('non-plot and frozen layers are skipped; lineweights follow the option', () => {
  const d = new Drawing('t');
  d.layers.push({ name: 'Hidden', color: 1, ltype: 'Continuous', lw: 25, on: true, frozen: false, locked: false, plot: false });
  d.add(E.line({ x: 0, y: 0 }, { x: 100, y: 0 }, { layer: '0', lw: 50 })); d.add(E.line({ x: 0, y: 10 }, { x: 100, y: 10 }, { layer: 'Hidden' }));
  const svg = exportVector(d, { ...defaultPlotOptions(d), format: 'svg' }).data;
  assert.equal((svg.match(/<path /g) || []).length, 1);
  assert.match(svg, /stroke-width="5"/);                      // 0.50 mm = 5 device units
  const thin = exportVector(d, { ...defaultPlotOptions(d), format: 'svg', lineweights: false }).data;
  assert.match(thin, /stroke-width="2"/);
});

test('PDF renders with pdftoppm/pymupdf when available (page count and non-blank output)', t => {
  const probe = spawnSync('python3', ['-I', '-c', 'import pymupdf'], { encoding: 'utf8' });
  if (probe.status !== 0) { t.skip('pymupdf not installed'); return; }
  const d = sampleDrawing(), r = exportVector(d, defaultPlotOptions(d));
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'cf-pdf-')), f = path.join(dir, 'a.pdf');
  fs.writeFileSync(f, r.data);
  const py = `import pymupdf,sys\ndoc=pymupdf.open(sys.argv[1]);p=doc[0];pix=p.get_pixmap(dpi=50);dark=sum(1 for i in range(0,len(pix.samples),pix.n) if pix.samples[i]<128)\nprint(len(doc),round(p.rect.width),round(p.rect.height),dark>200)`;
  const out = spawnSync('python3', ['-I', '-c', py, f], { encoding: 'utf8' });
  assert.equal(out.status, 0, out.stderr);
  assert.equal(out.stdout.trim(), '1 842 595 True');
});
