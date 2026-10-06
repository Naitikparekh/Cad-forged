import test from 'node:test';
import assert from 'node:assert/strict';
import { earcut } from '../../src/solid/earcut.js';
import * as S from '../../src/solid/mesh.js';
import { Drawing } from '../../src/core/drawing.js';
import { E } from '../../src/core/entities.js';

const near = (a, b, tol = 1e-6) => assert.ok(Math.abs(a - b) <= tol, `expected ${b}, got ${a}`);
const triArea = (flat, t) => { let s = 0; for (let i = 0; i < t.length; i += 3) { const [a, b, c] = [t[i], t[i + 1], t[i + 2]]; s += Math.abs((flat[b * 2] - flat[a * 2]) * (flat[c * 2 + 1] - flat[a * 2 + 1]) - (flat[b * 2 + 1] - flat[a * 2 + 1]) * (flat[c * 2] - flat[a * 2])) / 2; } return s; };
const rect = (x, y, w, h) => [{ x, y }, { x: x + w, y }, { x: x + w, y: y + h }, { x, y: y + h }];

test('earcut triangulates convex, concave and holed polygons with exact area', () => {
  const L = [0, 0, 10, 0, 10, 5, 5, 5, 5, 10, 0, 10];
  const t = earcut(L); assert.equal(t.length / 3, 4); near(triArea(L, t), 75);
  const plate = [0, 0, 100, 0, 100, 50, 0, 50, 20, 20, 30, 20, 30, 30, 20, 30, 60, 10, 80, 10, 80, 40, 60, 40];
  const t2 = earcut(plate, [4, 8]); near(triArea(plate, t2), 5000 - 100 - 600);
  const star = []; for (let i = 0; i < 20; i++) { const r = i % 2 ? 4 : 10, a = Math.PI * i / 10; star.push(r * Math.cos(a), r * Math.sin(a)); }
  const ts = earcut(star); assert.equal(ts.length / 3, 18);
});

test('extrusion of a rectangle: 8 vertices, 12 faces, watertight, exact volume', () => {
  const m = S.extrudeLoops([rect(0, 0, 10, 20)], 30);
  assert.equal(m.vertices.length, 8); assert.equal(m.faces.length, 12);
  assert.ok(S.isWatertight(m)); near(S.meshVolume(m), 6000);
  const down = S.extrudeLoops([rect(0, 0, 10, 20)], -30, { startZ: 5 });
  near(S.meshBounds(down).min.z, -25); near(S.meshBounds(down).max.z, 5); near(S.meshVolume(down), 6000);
});

test('extrusion handles concave outlines and holes (even-odd), watertight and signed volume positive', () => {
  const concave = [{ x: 0, y: 0 }, { x: 10, y: 0 }, { x: 10, y: 5 }, { x: 5, y: 5 }, { x: 5, y: 10 }, { x: 0, y: 10 }];
  const c = S.extrudeLoops([concave], 3); assert.ok(S.isWatertight(c)); near(S.meshVolume(c), 225);
  const hole = S.extrudeLoops([rect(0, 0, 100, 50), rect(20, 20, 10, 10).reverse(), rect(60, 10, 20, 30)], 4);
  assert.ok(S.isWatertight(hole)); near(S.meshVolume(hole), (5000 - 100 - 600) * 4);
  const nested = S.extrudeLoops([rect(0, 0, 100, 100), rect(10, 10, 80, 80), rect(40, 40, 20, 20)], 1);     // plate, hole, island inside the hole
  assert.ok(S.isWatertight(nested)); near(S.meshVolume(nested), (10000 - 6400 + 400));
});

test('profileLoops: closed entities, circles and chains of lines and arcs', () => {
  const d = new Drawing('t');
  const pl = d.add(E.pline(rect(0, 0, 10, 10), true)), c = d.add(E.circle({ x: 50, y: 50 }, 5));
  const r1 = S.profileLoops([pl, c], d); assert.equal(r1.loops.length, 2);
  const lines = rect(100, 0, 20, 10).map((p, i, a) => d.add(E.line(p, a[(i + 1) % 4])));
  const r2 = S.profileLoops(lines.reverse(), d); assert.equal(r2.error, undefined); assert.equal(r2.loops.length, 1); near(Math.abs(S.meshVolume(S.extrudeLoops(r2.loops, 1))), 200);
  const open = S.profileLoops(lines.slice(0, 3), d); assert.match(open.error, /closed/);
  const bad = S.profileLoops([d.add(E.line({ x: 0, y: 0 }, { x: 1, y: 1 }))], d); assert.ok(bad.error);
  const circle = S.extrudeLoops(S.profileLoops([c], d).loops, 10); near(S.meshVolume(circle), Math.PI * 25 * 10, 5);
});

test('primitives have the right volume and are watertight', () => {
  const box = S.boxMesh(0, 0, 0, 10, 20, 30); near(S.meshVolume(box), 6000); assert.ok(S.isWatertight(box));
  const wedge = S.wedgeMesh(0, 0, 0, 10, 20, 30); near(S.meshVolume(wedge), 3000); assert.ok(S.isWatertight(wedge));
  const cyl = S.cylinderMesh(0, 0, 0, 5, 10, 96); near(S.meshVolume(cyl), Math.PI * 25 * 10, 2.5); assert.ok(S.isWatertight(cyl));
  const cone = S.coneMesh(0, 0, 0, 6, 0, 12, 96); near(S.meshVolume(cone), Math.PI * 36 * 12 / 3, 4); assert.ok(S.isWatertight(cone));
  const frustum = S.coneMesh(0, 0, 0, 6, 3, 12, 96); near(S.meshVolume(frustum), Math.PI * 12 / 3 * (36 + 18 + 9), 5); assert.ok(S.isWatertight(frustum));
  const sph = S.sphereMesh(1, 2, 3, 10, 64); near(S.meshVolume(sph) / (4 / 3 * Math.PI * 1000), 1, 0.012); assert.ok(S.isWatertight(sph));
  const c = S.meshCentroid(box); near(c.x, 5); near(c.y, 10); near(c.z, 15);
  near(S.meshArea(box), 2 * (200 + 300 + 600));
});

test('transforms keep solids outward-facing, including mirrors', () => {
  const box = S.boxMesh(0, 0, 0, 10, 20, 30);
  const mv = S.transformMesh(box, S.M3.translate(5, 5, 5)); near(S.meshBounds(mv).min.z, 5); near(S.meshVolume(mv), 6000);
  const rot = S.transformMesh(box, S.M3.rotateZ(Math.PI / 2)); near(S.meshBounds(rot).max.x, 0); near(S.meshBounds(rot).min.x, -20); near(S.meshVolume(rot), 6000);
  const mir = S.transformMesh(box, S.M3.mirrorXY({ x: 0, y: 0 }, { x: 0, y: 1 })); near(S.meshBounds(mir).max.x, 0); near(S.meshVolume(mir), 6000); assert.ok(S.isWatertight(mir));
  const sc = S.transformMesh(box, S.M3.scale(2)); near(S.meshVolume(sc), 48000);
});

test('OBJ and STL export', () => {
  const obj = S.toOBJ([S.boxMesh(0, 0, 0, 1, 1, 1, 'My box')]);
  assert.match(obj, /^o My_box$/m); assert.equal(obj.match(/^v /gm).length, 8); assert.equal(obj.match(/^f /gm).length, 12);
  const stl = S.toSTL([S.boxMesh(0, 0, 0, 1, 1, 1)]); assert.equal(stl.match(/facet normal/g).length, 12); assert.match(stl, /endsolid/);
});

import { booleanSync, booleanAsync } from '../../src/solid/boolean.js';

test('boolean union / subtract / intersect volumes (overlapping boxes)', () => {
  const A = S.boxMesh(0, 0, 0, 10, 10, 10), B = S.boxMesh(5, 0, 0, 10, 10, 10);
  for (const [op, v] of [['union', 1500], ['subtract', 500], ['intersect', 500]]) { const r = booleanSync(A, B, op); near(S.meshVolume(r), v, 1e-5); }
  assert.equal(booleanSync(A, A, 'subtract').faces.length, 0);
  const far = S.transformMesh(B, S.M3.translate(100, 0, 0));
  assert.equal(booleanSync(A, far, 'intersect').faces.length, 0); near(S.meshVolume(booleanSync(A, far, 'union')), 2000, 1e-5);
});

test('drilling a cylindrical hole through a plate', () => {
  const plate = S.boxMesh(-20, -20, 0, 40, 40, 10), drill = S.cylinderMesh(0, 0, -5, 6, 20, 48);
  const r = booleanSync(plate, drill, 'subtract');
  const cyl = S.meshVolume(S.cylinderMesh(0, 0, 0, 6, 10, 48));
  near(S.meshVolume(r), 16000 - cyl, 1e-3);     // BSP output may contain T-junctions, so only the volume is checked
});

test('subtracting a sphere from a box and an async fallback without a worker', async () => {
  const box = S.boxMesh(-10, -10, -10, 20, 20, 20), sph = S.sphereMesh(0, 0, 0, 12, 32);
  const r = await booleanAsync(box, sph, 'intersect');
  assert.ok(S.meshVolume(r) > 0 && S.meshVolume(r) < 8000);
  await assert.rejects(async () => booleanSync(box, sph, 'bogus'), /Unknown/);
});
