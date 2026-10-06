// Generates the JSON fixtures the Rust test-suite uses as oracles: drawings saved by the web app and the geometry the
// web app computes for them.   Run:  node tests/fixtures/make-fixtures.mjs
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Drawing } from '../../src/core/drawing.js';
import { E, segsOf, boundsOf, pickDistance, snapPointsOf, gripsOf, explodeEntity, textsOf, hatchSegments } from '../../src/core/entities.js';
import { newLayer } from '../../src/core/style.js';
import { sampleDrawing } from '../../src/io/sample.js';
import { boxMesh, cylinderMesh } from '../../src/solid/mesh.js';

const here = dirname(fileURLToPath(import.meta.url));
const out = (name, obj) => writeFileSync(join(here, name), JSON.stringify(obj, null, 1) + '\n');
const P = (x, y) => ({ x, y });

function kitchenSink() {
  const d = new Drawing('Kitchen sink');
  d.layers.push(newLayer('Walls', { color: 1, lw: 50 }), newLayer('Hid', { color: 3, on: false }), newLayer('Rgb', { color: '#336699', ltype: 'DASHED', lw: 25 }));
  d.add(E.line(P(0, 0), P(10, 5)));
  d.add(E.circle(P(5, 5), 3, { layer: 'Walls' }));
  d.add(E.arc(P(0, 0), 4, 0.3, 2.2, { color: 5 }));
  d.add(E.arc(P(2, 2), 4, 5, 1));
  d.add(E.pline([P(0, 0), P(4, 0), { x: 6, y: 2, b: 0.5 }, P(6, 6), P(0, 6)], true, { layer: 'Rgb' }));
  d.add(E.pline([P(20, 0), { x: 24, y: 0, b: -0.4 }, P(24, 4)], false, { color: '#ff8000' }));
  d.add(E.ellipse(P(30, 5), P(6, 2), 0.5));
  d.add(E.ellipse(P(30, 15), P(4, 4), 0.6, 0.5, 4));
  d.add(E.spline([P(0, 20), P(3, 25), P(6, 19), P(9, 24), P(12, 20)]));
  d.add(E.spline([P(40, 20), P(43, 25), P(46, 19), P(43, 15)], true));
  d.add(E.point(P(7, 7)));
  d.add(E.text(P(1, 30), 2.5, 'Hello CAD'));
  d.add(E.text(P(10, 30), 3, 'Centred', { jh: 'center', jv: 'middle', rot: 0.4 }));
  d.add(E.mtext(P(0, 50), 2.5, 'Multi line text that wraps around a narrow box\nsecond paragraph', 30));
  d.add(E.dim('lin', { p1: P(0, 0), p2: P(40, 0), dp: P(20, -8), ang: 0 }));
  d.add(E.dim('ali', { p1: P(0, 0), p2: P(30, 40), dp: P(-10, 25) }));
  d.add(E.dim('rad', { c: P(5, 5), pt: P(5 + 3 * Math.SQRT1_2, 5 + 3 * Math.SQRT1_2), dp: P(12, 12) }));
  d.add(E.dim('dia', { c: P(5, 5), pt: P(8, 5), dp: P(14, 5) }));
  d.add(E.dim('ang', { c: P(0, 0), p1: P(10, 0), p2: P(0, 10), dp: P(7, 7) }));
  d.add(E.leader([P(0, 0), P(8, 8), P(14, 8)], 'NOTE', 2.5));
  d.add(E.hatch([[{ x: 0, y: 0, b: 0 }, { x: 10, y: 0, b: 0 }, { x: 10, y: 10, b: 0 }, { x: 0, y: 10, b: 0 }], [{ x: 3, y: 3, b: 0 }, { x: 7, y: 3, b: 0 }, { x: 7, y: 7, b: 0 }, { x: 3, y: 7, b: 0 }]], { pattern: 'ANSI31', ps: 1, pa: 0.3 }));
  d.add(E.hatch([[{ x: 50, y: 0, b: 0 }, { x: 60, y: 0, b: 0 }, { x: 55, y: 8, b: 0 }]], { pattern: 'SOLID' }));
  d.add(E.xline(P(0, 0), P(1, 1)));
  d.add(E.ray(P(2, 2), P(-1, 0.5)));
  d.defineBlock('BOLT', P(1, 1), [E.circle(P(1, 1), 1), E.line(P(0, 1), P(2, 1)), E.arc(P(1, 1), 0.5, 0, Math.PI), E.text(P(0, 3), 1, 'B')]);
  d.add(E.insert('BOLT', P(70, 10)));
  d.add(E.insert('BOLT', P(80, 10), { sx: 2, sy: 3, rot: 0.7 }));
  d.add(E.insert('BOLT', P(90, 10), { sx: -1.5, sy: 1.5, rot: 0.2 }));
  d.layouts.push({ name: 'Layout1', paper: { w: 297, h: 210, name: 'A4', landscape: true }, entities: [] });
  d.add({ type: 'viewport', c: P(100, 100), w: 80, h: 60, scale: 0.5 }, 'Layout1');
  d.addSolid({ ...boxMesh(0, 0, 0, 5, 5, 5, 'Box'), layer: 'Walls' });
  d.addSolid(cylinderMesh(10, 0, 0, 2, 6, 16, 'Cyl'));
  d.views.Plan = { cx: 5, cy: 6, scale: 3 };
  d.view = { cx: 1, cy: 2, cz: 0, scale: 7, yaw: 0, pitch: Math.PI / 2 };
  d.cur = { layer: 'Walls', color: 2, ltype: 'DASHED', lw: 25 };
  d.dirty = false;
  return d;
}

const round = v => JSON.parse(JSON.stringify(v, (k, x) => typeof x === 'number' ? Math.round(x * 1e9) / 1e9 : x));

function oracle(doc) {
  const probes = [P(0, 0), P(5, 5), P(12.5, 3.25), P(-4, 20), P(31, 6), P(73, 11)];
  const res = [];
  for (const e of doc.allEntities()) {
    if (e.type === 'viewport') { res.push({ id: e.id, type: e.type, bounds: round(boundsOf(e, doc)) }); continue; }
    const segs = segsOf(e, doc);
    res.push(round({
      id: e.id, type: e.type,
      nsegs: segs.length,
      segs: segs.slice(0, 200).map(s => s.t === 'l' ? ['l', s.a.x, s.a.y, s.b.x, s.b.y] : ['a', s.c.x, s.c.y, s.r, s.a0, s.sw]),
      bounds: boundsOf(e, doc),
      pick: probes.map(p => { const v = pickDistance(e, p, doc); return Number.isFinite(v) ? v : 'inf'; }),
      snaps: snapPointsOf(e, doc).map(s => [s.type, s.pt.x, s.pt.y]),
      grips: gripsOf(e, doc).map(g => [g.id, g.pt.x, g.pt.y]),
      explode: (explodeEntity(e, doc) || []).map(x => x.type),
      texts: textsOf(e, doc).map(t => [t.s, t.p.x, t.p.y, t.h, t.rot || 0]),
      hatch: e.type === 'hatch' ? (hatchSegments(e, doc) || []).length : undefined,
    }));
  }
  return res;
}

const ks = kitchenSink();
out('kitchen-sink.cadforge.json', ks.serialize());
out('kitchen-sink.oracle.json', oracle(ks));
const sample = sampleDrawing();
out('sample.cadforge.json', sample.serialize());
out('sample.oracle.json', oracle(sample));
console.log('fixtures written to', here);
