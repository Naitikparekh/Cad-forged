import test from 'node:test';
import assert from 'node:assert/strict';
import * as g from '../../src/core/geom.js';
import { Drawing } from '../../src/core/drawing.js';
import { E, segsOf } from '../../src/core/entities.js';
import { trim, extend, breakAt, splitAt, join, lengthen } from '../../src/ops/trim.js';
import { offsetEntity } from '../../src/ops/offset.js';
import { filletSegs, chamferSegs, filletPline, chamferPline } from '../../src/ops/fillet.js';
import { findRegion } from '../../src/ops/boundary.js';

const near = (a, b, tol = 1e-6) => assert.ok(Math.abs(a - b) <= tol, `${a} != ${b}`);
const P = (x, y) => ({ x, y });
const doc = () => new Drawing();

test('trim a line between two cutters keeps both outer pieces', () => {
  const d = doc();
  const t = d.add(E.line(P(0, 0), P(10, 0)));
  const c1 = d.add(E.line(P(3, -2), P(3, 2))), c2 = d.add(E.line(P(7, -2), P(7, 2)));
  const r = trim(t, P(5, 0), [c1, c2], d);
  assert.equal(r.replace.length, 2);
  near(r.replace[0].b.x, 3); near(r.replace[1].a.x, 7);
});

test('trim the end of a line past the last cutter', () => {
  const d = doc();
  const t = d.add(E.line(P(0, 0), P(10, 0)));
  const c = d.add(E.line(P(4, -2), P(4, 2)));
  const r = trim(t, P(8, 0), [c], d);
  assert.equal(r.replace.length, 1); near(r.replace[0].a.x, 0); near(r.replace[0].b.x, 4);
});

test('trim errors with no intersecting edge', () => {
  const d = doc();
  const t = d.add(E.line(P(0, 0), P(10, 0))), c = d.add(E.line(P(20, -2), P(20, 2)));
  assert.ok(trim(t, P(5, 0), [c], d).error);
});

test('trim a circle between two lines leaves an arc', () => {
  const d = doc();
  const c = d.add(E.circle(P(0, 0), 5));
  const l1 = d.add(E.line(P(-10, 0), P(10, 0))), l2 = d.add(E.line(P(0, -10), P(0, 10)));
  const r = trim(c, P(3.5, 3.5), [l1, l2], d);          // remove the first-quadrant quarter
  assert.equal(r.replace.length, 1);
  const a = r.replace[0];
  assert.equal(a.type, 'arc'); near(g.sweepCCW(a.a0, a.a1), 1.5 * Math.PI);
});

test('trim a polyline segment splits it', () => {
  const d = doc();
  const pl = d.add(E.pline([P(0, 0), P(10, 0), P(10, 10)]));
  const cut = d.add(E.line(P(5, -1), P(5, 1)));
  const r = trim(pl, P(2, 0), [cut], d);
  assert.equal(r.replace.length, 1);
  near(r.replace[0].v[0].x, 5); assert.equal(r.replace[0].v.length, 3);
});

test('extend a line to a boundary', () => {
  const d = doc();
  const t = d.add(E.line(P(0, 0), P(4, 0))), b = d.add(E.line(P(10, -5), P(10, 5)));
  const r = extend(t, P(3.5, 0), [b], d);
  near(r.replace[0].b.x, 10); near(r.replace[0].a.x, 0);
  const r2 = extend(t, P(0.2, 0), [d.add(E.line(P(-6, -5), P(-6, 5)))], d);       // extend the start
  near(r2.replace[0].a.x, -6); near(r2.replace[0].b.x, 4);
});

test('extend an arc to a line', () => {
  const d = doc();
  const arc = d.add(E.arc(P(0, 0), 5, 0, Math.PI / 2));
  const b = d.add(E.line(P(-10, 3), P(10, 3)));              // arc ends at (0,5): extending past 90 deg reaches y=3 at x=-4
  const r = extend(arc, P(0, 5), [b], d);
  const e = r.replace[0];
  const end = g.polar(e.c, e.a1, e.r);
  near(end.x, -4, 1e-6); near(end.y, 3, 1e-6);
});

test('break a line removes the middle', () => {
  const d = doc();
  const l = d.add(E.line(P(0, 0), P(10, 0)));
  const r = breakAt(l, P(3, 0), P(6, 0), d);
  assert.equal(r.replace.length, 2); near(r.replace[0].b.x, 3); near(r.replace[1].a.x, 6);
  const s = splitAt(l, P(4, 0), d);
  assert.equal(s.replace.length, 2); near(s.replace[0].b.x, 4); near(s.replace[1].a.x, 4);
});

test('join merges connected lines into a polyline and collinear lines into one line', () => {
  const d = doc();
  const a = E.line(P(0, 0), P(5, 0)), b = E.line(P(5, 0), P(5, 5)), c = E.line(P(5, 5), P(0, 5));
  const r = join([a, b, c], d);
  assert.equal(r.results.length, 1); assert.equal(r.results[0].type, 'pline'); assert.equal(r.results[0].v.length, 4);
  const r2 = join([E.line(P(0, 0), P(5, 0)), E.line(P(5, 0), P(9, 0))], d);
  assert.equal(r2.results[0].type, 'line'); near(r2.results[0].b.x, 9);
  const r3 = join([E.line(P(0, 0), P(1, 0)), E.line(P(50, 50), P(51, 50))], d);
  assert.equal(r3.results.length, 2);
});

test('lengthen a line by delta from the picked end', () => {
  const d = doc();
  const l = E.line(P(0, 0), P(10, 0));
  const r = lengthen(l, P(9, 0), 'delta', 5, d);
  near(r.replace[0].b.x, 15);
  const r2 = lengthen(l, P(1, 0), 'total', 4, d);
  near(r2.replace[0].a.x, 6); near(r2.replace[0].b.x, 10);
});

test('offset line, circle, arc and rectangle', () => {
  const d = doc();
  const l = E.line(P(0, 0), P(10, 0));
  const o = offsetEntity(l, 2, P(5, 5), d).result;
  near(o.a.y, 2); near(o.b.y, 2);
  const o2 = offsetEntity(l, 2, P(5, -5), d).result; near(o2.a.y, -2);
  const c = E.circle(P(0, 0), 5);
  near(offsetEntity(c, 1, P(20, 0), d).result.r, 6); near(offsetEntity(c, 1, P(1, 0), d).result.r, 4);
  assert.ok(offsetEntity(c, 6, P(1, 0), d).error);
  const rect = E.pline([P(0, 0), P(10, 0), P(10, 6), P(0, 6)], true);
  const inner = offsetEntity(rect, 1, P(5, 3), d).result;
  assert.equal(inner.v.length, 4); near(inner.v[0].x, 1); near(inner.v[0].y, 1); near(inner.v[2].x, 9); near(inner.v[2].y, 5);
  const outer = offsetEntity(rect, 1, P(50, 50), d).result;
  near(outer.v[0].x, -1); near(outer.v[0].y, -1); near(outer.v[2].x, 11); near(outer.v[2].y, 7);
  assert.ok(offsetEntity(rect, 4, P(5, 3), d).error || Math.abs(g.plineArea(offsetEntity(rect, 4, P(5, 3), d).result.v)) < 1);
});

test('offset a polyline containing an arc', () => {
  const d = doc();
  const pl = E.pline([{ x: 0, y: 0, b: 0 }, { x: 10, y: 0, b: 1 }, { x: 10, y: 10, b: 0 }]);   // semicircle bulging +x
  const r = offsetEntity(pl, 1, P(20, 5), d).result;       // outside of the bulge
  const segs = segsOf(r, d);
  assert.equal(segs.length, 2); assert.equal(segs[1].t, 'a'); near(segs[1].r, 6);
});

test('fillet two perpendicular lines with a radius', () => {
  const sA = { t: 'l', a: P(0, 0), b: P(10, 0) }, sB = { t: 'l', a: P(0, 0), b: P(0, 10) };
  const r = filletSegs(sA, P(8, 0), sB, P(0, 8), 2);
  near(r.a.a.x, 2); near(r.a.b.x, 10); near(r.b.a.y, 2);
  near(r.arc.r, 2); near(r.arc.c.x, 2); near(r.arc.c.y, 2); near(Math.abs(r.arc.sw), Math.PI / 2);
  const z = filletSegs({ t: 'l', a: P(0, 0), b: P(4, 0) }, P(3, 0), { t: 'l', a: P(10, -5), b: P(10, 5) }, P(10, 4), 0);
  near(z.a.b.x, 10); near(z.b.a.y, 0);                       // radius 0: extend both to the corner
});

test('fillet and chamfer on polylines (all vertices)', () => {
  const d = doc();
  const rect = E.pline([P(0, 0), P(10, 0), P(10, 10), P(0, 10)], true);
  const f = filletPline(rect, 2, d);
  assert.equal(f.applied, 4); assert.equal(f.entity.v.length, 8);
  near(g.plineArea(f.entity.v), 100 - (4 - Math.PI) * 4 * 1, 1e-6 + 0.01);
  const c = chamferPline(rect, 2, 2, d);
  assert.equal(c.applied, 4); near(g.plineArea(c.entity.v), 100 - 4 * 2, 1e-6);
});

test('chamfer two lines', () => {
  const r = chamferSegs({ t: 'l', a: P(0, 0), b: P(10, 0) }, P(8, 0), { t: 'l', a: P(0, 0), b: P(0, 10) }, P(0, 8), 2, 3);
  near(r.a.a.x, 2); near(r.b.a.y, 3); near(r.line.a.x, 2); near(r.line.b.y, 3);
});

test('boundary detection: rectangle with an island circle and a dangling line', () => {
  const d = doc();
  d.add(E.pline([P(0, 0), P(20, 0), P(20, 20), P(0, 20)], true));
  d.add(E.circle(P(10, 10), 4));
  d.add(E.line(P(20, 10), P(30, 10)));                       // dangling spur
  const r = findRegion(d, P(2, 2));
  assert.ok(!r.error, r.error);
  assert.equal(r.loops.length, 2);
  const areas = r.loops.map(l => Math.abs(g.plineArea(l)));
  near(Math.max(...areas), 400, 1e-3); near(Math.min(...areas), Math.PI * 16, 1e-3);
  const inside = findRegion(d, P(10, 10));                    // inside the circle
  assert.equal(inside.loops.length, 1); near(Math.abs(g.plineArea(inside.loops[0])), Math.PI * 16, 1e-3);
  assert.ok(findRegion(d, P(50, 50)).error);
});

test('boundary detection: two adjacent regions split by a line', () => {
  const d = doc();
  d.add(E.pline([P(0, 0), P(20, 0), P(20, 10), P(0, 10)], true));
  d.add(E.line(P(10, -2), P(10, 12)));                         // crosses both edges
  const left = findRegion(d, P(3, 5)), right = findRegion(d, P(17, 5));
  near(Math.abs(g.plineArea(left.loops[0])), 100, 1e-6); near(Math.abs(g.plineArea(right.loops[0])), 100, 1e-6);
});

test('boundary detection with bulged polyline and overlapping circles', () => {
  const d = doc();
  d.add(E.circle(P(0, 0), 5)); d.add(E.circle(P(6, 0), 5));
  const lens = findRegion(d, P(3, 0));
  assert.ok(!lens.error, lens.error);
  const a = Math.abs(g.plineArea(lens.loops[0]));
  near(a, 2 * 25 * Math.acos(3 / 5) - 6 * 4, 1e-3);          // lens area of two r=5 circles 6 apart
});

test('offset a full ellipse produces a closed outline around it', () => {
  const d = doc();
  const r = offsetEntity(E.ellipse(P(0, 0), P(10, 0), 0.5), 1, P(30, 0), d).result;
  assert.equal(r.type, 'pline'); assert.ok(r.closed);
  const b = g.ptsBounds(r.v);
  near(b.maxX, 11, 0.05); near(b.minX, -11, 0.05); near(b.maxY, 6, 0.05);
});
