import test from 'node:test';
import assert from 'node:assert/strict';
import * as g from '../../src/core/geom.js';

const close = (a, b, tol = 1e-9) => assert.ok(Math.abs(a - b) <= tol, `${a} != ${b}`);
const closeP = (p, q, tol = 1e-9) => { close(p.x, q.x, tol); close(p.y, q.y, tol); };

test('bulge arc: semicircle both directions', () => {
  const ccw = g.bulgeSeg({ x: 0, y: 0 }, { x: 2, y: 0 }, 1);
  assert.equal(ccw.t, 'a'); closeP(ccw.c, { x: 1, y: 0 }); close(ccw.r, 1); close(ccw.sw, Math.PI);
  closeP(g.segAt(ccw, .5), { x: 1, y: -1 });                 // CCW from (0,0) to (2,0) passes below the chord
  const cw = g.bulgeSeg({ x: 0, y: 0 }, { x: 2, y: 0 }, -1);
  closeP(g.segAt(cw, .5), { x: 1, y: 1 }); close(cw.sw, -Math.PI);
  close(g.bulgeOf(ccw), 1); close(g.bulgeOf(cw), -1);
});

test('arcThrough three points', () => {
  const a = g.arcThrough({ x: 1, y: 0 }, { x: 0, y: 1 }, { x: -1, y: 0 });
  closeP(a.c, { x: 0, y: 0 }); close(a.r, 1); close(a.sw, Math.PI);
  const b = g.arcThrough({ x: 1, y: 0 }, { x: 0, y: -1 }, { x: -1, y: 0 });
  close(b.sw, -Math.PI);
  assert.equal(g.arcThrough({ x: 0, y: 0 }, { x: 1, y: 1 }, { x: 2, y: 2 }), null);
});

test('line-line intersection with extension flags', () => {
  const a = { t: 'l', a: { x: 0, y: 0 }, b: { x: 10, y: 0 } }, b = { t: 'l', a: { x: 15, y: -5 }, b: { x: 15, y: 5 } };
  assert.equal(g.segIntersect(a, b).length, 0);
  const h = g.segIntersect(a, b, true, false);
  assert.equal(h.length, 1); closeP(h[0].pt, { x: 15, y: 0 }); close(h[0].t1, 1.5); close(h[0].t2, .5);
});

test('line-arc intersections respect the sweep', () => {
  const arc = { t: 'a', c: { x: 0, y: 0 }, r: 5, a0: 0, sw: Math.PI };           // upper half circle
  const l = { t: 'l', a: { x: -10, y: 3 }, b: { x: 10, y: 3 } };
  const hits = g.segIntersect(l, arc);
  assert.equal(hits.length, 2);
  const lower = { t: 'l', a: { x: -10, y: -3 }, b: { x: 10, y: -3 } };
  assert.equal(g.segIntersect(lower, arc).length, 0);
  assert.equal(g.segIntersect(lower, arc, false, true).length, 2);                // extended arc = full circle
  const tangent = { t: 'l', a: { x: -10, y: 5 }, b: { x: 10, y: 5 } };
  assert.equal(g.segIntersect(tangent, arc).length, 1);
});

test('arc-arc intersections', () => {
  const a = { t: 'a', c: { x: 0, y: 0 }, r: 5, a0: 0, sw: g.TAU }, b = { t: 'a', c: { x: 6, y: 0 }, r: 5, a0: 0, sw: g.TAU };
  const hits = g.segIntersect(a, b);
  assert.equal(hits.length, 2); close(hits[0].pt.x, 3); close(Math.abs(hits[0].pt.y), 4);
  assert.equal(g.segIntersect(a, { ...b, c: { x: 20, y: 0 } }).length, 0);
});

test('segment offset sides', () => {
  const l = { t: 'l', a: { x: 0, y: 0 }, b: { x: 10, y: 0 } };
  closeP(g.segOffset(l, 2).a, { x: 0, y: 2 }); closeP(g.segOffset(l, -2).b, { x: 10, y: -2 });
  const ccw = { t: 'a', c: { x: 0, y: 0 }, r: 5, a0: 0, sw: Math.PI };
  close(g.segOffset(ccw, 1).r, 4); close(g.segOffset(ccw, -1).r, 6); assert.equal(g.segOffset(ccw, 6), null);
  const cw = g.segReverse(ccw);
  close(g.segOffset(cw, 1).r, 6);
});

test('nearest point on arcs clamps to endpoints outside the sweep', () => {
  const arc = { t: 'a', c: { x: 0, y: 0 }, r: 5, a0: 0, sw: Math.PI / 2 };
  const r = g.segNearest(arc, { x: -10, y: -1 });
  closeP(r.pt, { x: 0, y: 5 }); close(r.t, 1);
  const r2 = g.segNearest(arc, { x: 10, y: 10 });
  closeP(r2.pt, { x: 5 * Math.SQRT1_2, y: 5 * Math.SQRT1_2 });
});

test('polyline area including bulges', () => {
  // CCW square; positive bulge bulges to the right of travel (outward), negative inward
  const v = [{ x: 0, y: 0, b: 0 }, { x: 2, y: 0, b: 0 }, { x: 2, y: 2, b: 1 }, { x: 0, y: 2, b: 0 }];
  close(g.plineArea(v), 4 + Math.PI / 2, 1e-9);
  const v2 = [{ x: 0, y: 0, b: 0 }, { x: 2, y: 0, b: 0 }, { x: 2, y: 2, b: -1 }, { x: 0, y: 2, b: 0 }];
  close(g.plineArea(v2), 4 - Math.PI / 2, 1e-9);
  close(g.plineLength([{ x: 0, y: 0, b: 1 }, { x: 2, y: 0, b: 0 }], false), Math.PI, 1e-9);
});

test('matrices: rotate about a point, mirror, similarity', () => {
  closeP(g.M.apply(g.M.rotate(Math.PI / 2, { x: 1, y: 1 }), { x: 2, y: 1 }), { x: 1, y: 2 });
  closeP(g.M.apply(g.M.mirror({ x: 0, y: 0 }, { x: 1, y: 1 }), { x: 3, y: 0 }), { x: 0, y: 3 });
  assert.ok(g.M.det(g.M.mirror({ x: 0, y: 0 }, { x: 1, y: 0 })) < 0);
  assert.ok(g.M.isSimilarity(g.M.mul(g.M.rotate(.3), g.M.scale(2))));
  assert.ok(!g.M.isSimilarity(g.M.scale(2, 3)));
  const inv = g.M.invert(g.M.mul(g.M.rotate(.4, { x: 3, y: 4 }), g.M.scale(2, 2, { x: 1, y: 1 })));
  const p = { x: 7, y: -2 };
  closeP(g.M.apply(inv, g.M.apply(g.M.mul(g.M.rotate(.4, { x: 3, y: 4 }), g.M.scale(2, 2, { x: 1, y: 1 })), p)), p);
});

test('mirrored arcs reverse direction', () => {
  const arc = { t: 'a', c: { x: 0, y: 0 }, r: 5, a0: 0, sw: Math.PI / 2 };
  const m = g.transformSeg(arc, g.M.mirror({ x: 0, y: 0 }, { x: 1, y: 0 }));
  closeP(g.segStart(m), { x: 5, y: 0 }); closeP(g.segEnd(m), { x: 0, y: -5 }); assert.ok(m.sw < 0);
});

test('tangent points and tangent circle centres', () => {
  const c = { t: 'a', c: { x: 0, y: 0 }, r: 5, a0: 0, sw: g.TAU };
  const t = g.tangentPoints(c, { x: 13, y: 0 });
  assert.equal(t.length, 2);
  for (const h of t) close(g.dot(g.sub(h.pt, c.c), g.sub(h.pt, { x: 13, y: 0 })), 0, 1e-9);
  const l1 = { t: 'l', a: { x: 0, y: 0 }, b: { x: 10, y: 0 } }, l2 = { t: 'l', a: { x: 0, y: 0 }, b: { x: 0, y: 10 } };
  const cs = g.tangentCentres(l1, l2, 2);
  assert.ok(cs.some(p => Math.abs(p.x - 2) < 1e-9 && Math.abs(p.y - 2) < 1e-9));
});

test('clip line to rectangle and crossing test', () => {
  const r = { minX: 0, minY: 0, maxX: 10, maxY: 10 };
  const c = g.clipLineToRect({ x: 5, y: 5 }, { x: 1, y: 0 }, r);
  closeP(c[0], { x: 0, y: 5 }); closeP(c[1], { x: 10, y: 5 });
  assert.equal(g.clipLineToRect({ x: 5, y: 20 }, { x: 1, y: 0 }, r), null);
  assert.ok(g.segTouchesRect({ t: 'l', a: { x: -5, y: 5 }, b: { x: 15, y: 5 } }, r));
  assert.ok(!g.segTouchesRect({ t: 'l', a: { x: -5, y: 15 }, b: { x: 15, y: 15 } }, r));
});
