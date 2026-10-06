import test from 'node:test';
import assert from 'node:assert/strict';
import * as g from '../../src/core/geom.js';
import { Drawing } from '../../src/core/drawing.js';
import { E } from '../../src/core/entities.js';
import { Snapper, activeMask, snapToGrid, polarAngles } from '../../src/engine/snap.js';
import { OS_BITS } from '../../src/engine/parse.js';
import { pickAt, selectRect, selectPolygon, selectFence, rectOf } from '../../src/engine/select.js';

const near = (a, b, t = 1e-6) => assert.ok(Math.abs(a - b) <= t, `${a} != ${b}`);
const P = (x, y) => ({ x, y });
const opts = (extra = {}) => ({ scale: 10, aperturePx: 12, osmask: 0, ...extra });
const ALL = Object.values(OS_BITS).reduce((a, b) => a | b, 0) & ~(OS_BITS.QUI | OS_BITS.APP | OS_BITS.EXT | OS_BITS.PAR);

function scene() {
  const d = new Drawing();
  d.add(E.line(P(0, 0), P(10, 0)));
  d.add(E.line(P(5, -5), P(5, 5)));
  d.add(E.circle(P(20, 0), 3));
  return d;
}

test('endpoint, midpoint, intersection, centre and quadrant snaps', () => {
  const d = scene(), s = new Snapper();
  let r = s.resolve(d, P(10.05, 0.05), opts({ osmask: OS_BITS.END }));
  assert.equal(r.snap.type, 'END'); near(r.pt.x, 10); near(r.pt.y, 0);
  r = s.resolve(d, P(5.4, 2.5), opts({ osmask: OS_BITS.MID }));            // the vertical line's midpoint is (5,0): too far
  assert.equal(r.snap, null);
  r = s.resolve(d, P(5.05, 0.08), opts({ osmask: OS_BITS.MID | OS_BITS.INT }));
  assert.ok(['INT', 'MID'].includes(r.snap.type)); near(r.pt.x, 5); near(r.pt.y, 0);
  r = s.resolve(d, P(5.05, 0.05), opts({ osmask: OS_BITS.INT }));
  assert.equal(r.snap.type, 'INT');
  r = s.resolve(d, P(20.1, 0.1), opts({ osmask: OS_BITS.CEN })); assert.equal(r.snap.type, 'CEN');
  r = s.resolve(d, P(23.05, 0.05), opts({ osmask: OS_BITS.QUA })); assert.equal(r.snap.type, 'QUA'); near(r.pt.x, 23);
  r = s.resolve(d, P(7, 1), opts({ osmask: OS_BITS.END }));
  assert.equal(r.snap, null); near(r.pt.x, 7);
});

test('perpendicular, tangent and nearest snaps use the base point', () => {
  const d = scene(), s = new Snapper();
  let r = s.resolve(d, P(8, 0.05), opts({ osmask: OS_BITS.PER, base: P(8, 6) }));
  assert.equal(r.snap.type, 'PER'); near(r.pt.x, 8); near(r.pt.y, 0);
  r = s.resolve(d, P(23, 0.3), opts({ osmask: OS_BITS.TAN, base: P(30, 0) }));
  assert.equal(r.snap.type, 'TAN');
  near(g.dot(g.sub(r.pt, P(20, 0)), g.sub(r.pt, P(30, 0))), 0, 1e-6);
  r = s.resolve(d, P(2.5, 0.05), opts({ osmask: OS_BITS.NEA }));
  assert.equal(r.snap.type, 'NEA'); near(r.pt.x, 2.5); near(r.pt.y, 0);
  // NEA yields to a real snap
  r = s.resolve(d, P(10.02, 0.02), opts({ osmask: OS_BITS.NEA | OS_BITS.END })); assert.equal(r.snap.type, 'END');
});

test('hidden or frozen layers are not snapped', () => {
  const d = scene(); d.layer('0').frozen = true;
  assert.equal(new Snapper().resolve(d, P(10, 0), opts({ osmask: OS_BITS.END })).snap, null);
});

test('ortho and polar tracking', () => {
  const d = new Drawing(), s = new Snapper();
  let r = s.resolve(d, P(10, 1.5), opts({ base: P(0, 0), ortho: true }));
  near(r.pt.y, 0); near(r.pt.x, 10);
  r = s.resolve(d, P(1, 9), opts({ base: P(0, 0), ortho: true })); near(r.pt.x, 0); near(r.pt.y, 9);
  r = s.resolve(d, P(10, 10.4), opts({ base: P(0, 0), polar: true, polarInc: Math.PI / 4 }));
  assert.equal(r.track.kind, 'polar'); near(r.pt.x, r.pt.y, 1e-6); assert.ok(/45/.test(r.track.label));
  r = s.resolve(d, P(10, 5), opts({ base: P(0, 0), polar: true, polarInc: Math.PI / 4 }));
  assert.equal(r.track, null); near(r.pt.y, 5);
  assert.equal(polarAngles(Math.PI / 2).length, 4); assert.equal(polarAngles(Math.PI / 6).length, 12);
});

test('object snap tracking intersects two alignments', () => {
  const d = new Drawing(); d.add(E.line(P(0, 0), P(10, 0))); d.add(E.line(P(20, 10), P(30, 10)));
  const s = new Snapper();
  s.noteHover({ pt: P(10, 0) }, 0); s.noteHover({ pt: P(10, 0) }, 400);
  s.noteHover({ pt: P(20, 10) }, 500); s.noteHover({ pt: P(20, 10) }, 900);
  assert.equal(s.acquired.length, 2);
  const r = s.resolve(d, P(10.1, 10.1), opts({ otrack: true }));
  assert.equal(r.track.kind, 'intersection'); near(r.pt.x, 10); near(r.pt.y, 10);
  const single = s.resolve(d, P(10.05, 4), opts({ otrack: true }));
  assert.equal(single.track.kind, 'otrack'); near(single.pt.x, 10); near(single.pt.y, 4);
  s.noteHover({ pt: P(10, 0) }, 2000); s.noteHover({ pt: P(10, 0) }, 2400);       // hovering again removes it
  assert.equal(s.acquired.length, 1);
});

test('grid snap with spacing, base and angle', () => {
  const p = snapToGrid(P(12.4, 7.6), { sx: 5, sy: 5, base: P(0, 0), angle: 0 }); near(p.x, 10); near(p.y, 10);
  const q = snapToGrid(P(12.4, 7.6), { sx: 5, sy: 2, base: P(1, 1), angle: 0 }); near(q.x, 11); near(q.y, 7);
  const s = new Snapper(), d = new Drawing();
  const r = s.resolve(d, P(3.2, 4.9), opts({ grid: { on: true, sx: 2.5, sy: 2.5, base: P(0, 0), angle: 0 } })); near(r.pt.x, 2.5); near(r.pt.y, 5);
  assert.equal(activeMask(4133, null), 4133 & ~(16384 | 1024)); assert.equal(activeMask(4133 | 16384, null), 0);
  assert.equal(activeMask(4133, 'none'), 0); assert.equal(activeMask(4133, OS_BITS.MID), OS_BITS.MID);
  void ALL;
});

test('picking: nearest wins; hatches yield to lines; locked layers are skipped', () => {
  const d = new Drawing();
  const l1 = d.add(E.line(P(0, 0), P(10, 0)));
  const hatch = d.add(E.hatch([[P(0, 0), P(10, 0), P(10, 10), P(0, 10)].map(q => ({ ...q, b: 0 }))], { pattern: 'SOLID' }));
  assert.equal(pickAt(d, P(5, 0.1), .5), l1);
  assert.equal(pickAt(d, P(5, 5), .5), hatch);
  d.addLayer('Locked', { locked: true });
  const l2 = d.add(E.line(P(0, 20), P(10, 20)), 'model'); l2.layer = 'Locked';
  assert.equal(pickAt(d, P(5, 20), .5), null);
});

test('window vs crossing vs polygon vs fence selection', () => {
  const d = new Drawing();
  const inside = d.add(E.line(P(2, 2), P(4, 4))), crossing = d.add(E.line(P(5, 5), P(15, 5))), outside = d.add(E.line(P(20, 20), P(30, 20)));
  const r = rectOf(P(0, 0), P(10, 10));
  assert.deepEqual(selectRect(d, r, false), [inside]);
  assert.deepEqual(selectRect(d, r, true).sort((a, b) => a.id - b.id), [inside, crossing]);
  const tri = [P(0, 0), P(12, 0), P(0, 12)];
  assert.deepEqual(selectPolygon(d, tri, false), [inside]);
  assert.ok(selectPolygon(d, tri, true).includes(crossing)); assert.ok(!selectPolygon(d, tri, true).includes(outside));
  assert.deepEqual(selectFence(d, [P(8, 0), P(8, 10)]), [crossing]);
});
