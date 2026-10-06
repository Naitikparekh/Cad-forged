import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { writeDXF } from '../../src/io/dxf-write.js';
import { readDXF, decodeMText, decodeText } from '../../src/io/dxf-read.js';
import { sampleDrawing } from '../../src/io/sample.js';
import { E, splineBeziers, splinePoints } from '../../src/core/entities.js';

const near = (a, b, tol = 1e-6) => assert.ok(Math.abs(a - b) <= tol, `expected ${b}, got ${a}`);

function fullDoc() {
  const d = sampleDrawing();
  d.add(E.arc({ x: 200, y: 20 }, 15, 0.3, 2.2, { layer: 'Outline' }));
  d.add(E.ellipse({ x: 200, y: 70 }, { x: 25, y: 10 }, 0.5, 0, Math.PI * 2, { layer: 'Outline' }));
  d.add(E.ellipse({ x: 200, y: 110 }, { x: 25, y: 10 }, 0.5, 0.5, 2.5, { layer: 'Outline', color: 3 }));
  d.add(E.spline([{ x: 150, y: 0 }, { x: 160, y: 20 }, { x: 175, y: 5 }, { x: 190, y: 25 }], false, { layer: 'Outline' }));
  d.add(E.point({ x: 120, y: 120 }, { layer: 'Notes' }));
  d.add(E.pline([{ x: 220, y: 0, b: 0.5 }, { x: 260, y: 0, b: 0 }, { x: 260, y: 30, b: -0.3 }, { x: 220, y: 30 }], true, { layer: 'Outline', color: '#ff8800', lw: 70, ltype: 'DASHED' }));
  d.add(E.text({ x: 220, y: 60 }, 5, 'Hello \u00D8 DXF', { rot: 0.4, jh: 'center', jv: 'middle', layer: 'Notes' }));
  d.add(E.xline({ x: 0, y: 0 }, { x: 1, y: 1 }, { layer: 'Centre' }));
  d.add(E.dim('rad', { c: { x: 200, y: 20 }, pt: { x: 215, y: 20 }, dp: { x: 230, y: 40 }, layer: 'Dimensions' }));
  d.add(E.dim('ang', { c: { x: 0, y: 0 }, p1: { x: 60, y: 0 }, p2: { x: 40, y: 40 }, dp: { x: 50, y: 20 }, layer: 'Dimensions' }));
  d.add(E.hatch([[{ x: 220, y: 0, b: 0 }, { x: 260, y: 0, b: 0 }, { x: 260, y: 30, b: 0 }, { x: 220, y: 30, b: 0 }]], { pattern: 'SOLID', layer: 'Hatch', color: 5 }));
  d.defineBlock('BOLT', { x: 1, y: 2 }, [E.circle({ x: 0, y: 0 }, 3, {}), E.line({ x: -4, y: 0 }, { x: 4, y: 0 }, {})].map(e => ({ ...e, layer: '0' })));
  d.add(E.insert('BOLT', { x: 40, y: 60 }, { sx: 2, sy: 2, rot: 0.5, layer: 'Holes' }));
  return d;
}

test('spline Bezier form reproduces the Catmull-Rom curve exactly', () => {
  const fit = [{ x: 0, y: 0 }, { x: 10, y: 8 }, { x: 25, y: 3 }, { x: 40, y: 15 }, { x: 50, y: 0 }];
  for (const closed of [false, true]) {
    const e = { fit, closed }, dense = splinePoints(e, 800);
    let worst = 0;
    for (const b of splineBeziers(e)) for (let k = 0; k <= 10; k++) {
      const t = k / 10, u = 1 - t, x = u ** 3 * b.p0.x + 3 * u * u * t * b.c1.x + 3 * u * t * t * b.c2.x + t ** 3 * b.p1.x, y = u ** 3 * b.p0.y + 3 * u * u * t * b.c1.y + 3 * u * t * t * b.c2.y + t ** 3 * b.p1.y;
      let best = Infinity;
      for (let i = 0; i + 1 < dense.length; i++) { const a = dense[i], c = dense[i + 1], dx = c.x - a.x, dy = c.y - a.y, L = dx * dx + dy * dy || 1, s = Math.max(0, Math.min(1, ((x - a.x) * dx + (y - a.y) * dy) / L)); best = Math.min(best, Math.hypot(x - a.x - dx * s, y - a.y - dy * s)); }
      worst = Math.max(worst, best);
    }
    assert.ok(worst < 1e-3, `deviation ${worst}`);
  }
});

test('write -> read round trip keeps geometry, layers, blocks and properties', () => {
  const src = fullDoc();
  const back = readDXF(writeDXF(src), 'rt');
  const byType = d => { const m = {}; for (const e of d.model.entities) m[e.type] = (m[e.type] || 0) + 1; return m; };
  const a = byType(src), b = byType(back);
  for (const t of ['line', 'circle', 'arc', 'pline', 'ellipse', 'spline', 'point', 'xline', 'insert', 'hatch']) assert.ok((b[t] || 0) >= (a[t] || 0), `${t}: ${a[t]} -> ${b[t]}`);
  assert.ok(src.layers.every(l => back.layer(l.name)), 'layers survive');
  const pl = back.model.entities.find(e => e.type === 'pline' && e.v.length === 4 && e.closed);
  assert.ok(pl, 'bulged polyline');
  near(pl.v[0].b, 0.5); near(pl.v[2].b, -0.3); assert.equal(pl.color, '#ff8800'); assert.equal(pl.lw, 70); assert.equal(pl.ltype, 'DASHED');
  const circ = back.model.entities.find(e => e.type === 'circle' && Math.abs(e.c.x - 75) < 1e-9);
  near(circ.r, 5); assert.equal(circ.layer, 'Holes');
  const ell = back.model.entities.filter(e => e.type === 'ellipse');
  assert.equal(ell.length, 2); near(ell[0].ratio, 0.5);
  const arc = back.model.entities.find(e => e.type === 'arc' && Math.abs(e.c.x - 200) < 1e-9);
  near(arc.a0, 0.3, 1e-9); near(arc.a1, 2.2, 1e-9);
  assert.ok(back.blocks.BOLT && back.blocks.BOLT.entities.length === 2); near(back.blocks.BOLT.base.x, 1); near(back.blocks.BOLT.base.y, 2);
  const ins = back.model.entities.find(e => e.type === 'insert'); near(ins.sx, 2); near(ins.rot, 0.5, 1e-9);
  const txt = back.model.entities.find(e => e.type === 'text' && e.s.includes('Hello'));
  assert.equal(txt.s, 'Hello \u00D8 DXF'); assert.equal(txt.jh, 'center'); assert.equal(txt.jv, 'middle'); near(txt.rot, 0.4, 1e-9);
  const mt = back.model.entities.find(e => e.type === 'text' && e.multi); assert.ok(mt.s.includes('Material'));
  assert.ok(back.model.entities.filter(e => e.type === 'hatch').length >= 2);
  assert.equal(back.settings.units.insUnits, 4);
});

test('dimensions come back as their block geometry', () => {
  const back = readDXF(writeDXF(sampleDrawing()), 'x');
  assert.ok(back.model.entities.some(e => e.type === 'text' && /^120(\.0+)?$/.test(e.s)), 'linear dimension text');
  assert.ok(back.model.entities.filter(e => e.layer === 'Dimensions').length > 10);
});

test('spline control-point import (clamped cubic)', () => {
  const src = fullDoc();
  const back = readDXF(writeDXF(src), 'x');
  const sp = back.model.entities.find(e => e.type === 'spline');
  assert.ok(sp && sp.fit.length > 10);
  near(sp.fit[0].x, 150); near(sp.fit[sp.fit.length - 1].x, 190); near(sp.fit[sp.fit.length - 1].y, 25);
});

test('text decoding helpers', () => {
  assert.equal(decodeText('%%c10 %%d \\U+00D8'), '\u230010 \u00B0 \u00D8');
  assert.equal(decodeMText('{\\fArial|b0;Hello}\\Pworld \\S1/2; \\C1;red'), 'Hello\nworld 1/2 red');
});

test('reader tolerates a hand-written minimal DXF (R12 style, POLYLINE/VERTEX, CRLF)', () => {
  const dxf = ['0','SECTION','2','ENTITIES','0','LINE','8','A','10','0','20','0','30','0','11','10','21','5','31','0','0','POLYLINE','8','B','66','1','70','1','0','VERTEX','8','B','10','0','20','0','42','0.5','0','VERTEX','8','B','10','4','20','0','0','VERTEX','8','B','10','4','20','3','0','SEQEND','0','CIRCLE','8','A','10','2','20','2','40','1','0','ENDSEC','0','EOF'].join('\r\n');
  const d = readDXF(dxf, 'min');
  assert.deepEqual(d.model.entities.map(e => e.type), ['line', 'pline', 'circle']);
  assert.equal(d.model.entities[1].closed, true); near(d.model.entities[1].v[0].b, 0.5);
  assert.ok(d.layer('A') && d.layer('B'));
});

test('rejects non-DXF input', () => { assert.throws(() => readDXF('hello world'), /DXF/); });

test('output validates with ezdxf (skipped when python/ezdxf is unavailable)', t => {
  const probe = spawnSync('python3', ['-I', '-c', 'import ezdxf'], { encoding: 'utf8' });
  if (probe.status !== 0) { t.skip('ezdxf not installed'); return; }
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'cf-dxf-')), file = path.join(dir, 'out.dxf');
  fs.writeFileSync(file, writeDXF(fullDoc()));
  const r = spawnSync('python3', ['-I', path.resolve('tests/dxf/validate.py'), file], { encoding: 'utf8' });
  assert.equal(r.status, 0, r.stdout + r.stderr);
  assert.match(r.stdout, /audit errors: \[\]/);
});


test('reads a DXF produced by ezdxf (independent writer)', () => {
  const file = path.resolve('tests/dxf/fixtures/ezdxf-sample.dxf');
  const d = readDXF(fs.readFileSync(file, 'utf8'), 'ezdxf');
  const types = {}; for (const e of d.model.entities) if (e.layer !== 'DIMS') types[e.type] = (types[e.type] || 0) + 1;
  assert.equal(types.line >= 1, true); assert.equal(types.circle >= 1, true);
  assert.equal(types.pline, 1); assert.equal(types.ellipse, 1); assert.equal(types.spline, 2);
  assert.equal(types.point, 1); assert.equal(types.insert, 1);
  assert.ok((types.hatch || 0) >= 3, `hatches ${types.hatch}`);          // 2 hatches + SOLID
  assert.equal(types.text, 2, 'text and mtext');
  const pl = d.model.entities.find(e => e.type === 'pline'); near(pl.v[0].b, 0.5); assert.equal(pl.closed, true);
  const t = d.model.entities.find(e => e.type === 'text' && e.s.startsWith('Hello')); assert.equal(t.s, 'Hello \u230025'); assert.equal(t.jh, 'center'); near(t.rot, Math.PI / 6, 1e-9);
  const mt = d.model.entities.find(e => e.type === 'text' && e.multi); assert.equal(mt.s, 'Bold line\nsecond 1/2');
  const walls = d.model.entities.find(e => e.layer === 'WALLS' && e.type === 'line'); assert.equal(d.layer('WALLS').color, 5); assert.equal(d.layer('WALLS').ltype, 'DASHED'); assert.ok(walls);
  const edgeHatch = d.model.entities.filter(e => e.type === 'hatch').find(h => h.loops[0].some(v => Math.abs(v.b) > 1e-6));
  assert.ok(edgeHatch, 'edge-path hatch with an arc edge becomes a bulged loop');
  assert.ok(d.model.entities.filter(e => e.layer === 'DIMS').length > 15, 'dimension geometry imported from blocks');
  const mirrored = d.model.entities.filter(e => e.type === 'arc').find(a => Math.abs(a.c.x + 200) < 1e-9);
  assert.ok(mirrored, 'arc with extrusion (0,0,-1) is mirrored to x = -200');
  assert.equal(d.settings.units.insUnits, 4);
});
