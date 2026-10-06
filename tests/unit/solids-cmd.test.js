import test from 'node:test';
import assert from 'node:assert/strict';
import { Sim, near } from './sim.js';
import { E } from '../../src/core/entities.js';
import * as S from '../../src/solid/mesh.js';

const P = (x, y) => ({ x, y });
const vol = s => S.meshVolume(s);

test('BOX by two corners and height, by Cube, and by Length', async () => {
  const s = new Sim();
  await s.play('BOX', [['click', 0, 0], ['click', 10, 20], ['type', '30']]);
  assert.equal(s.doc.solids.length, 1); near(vol(s.doc.solids[0]), 6000, 1e-6);
  assert.equal(s.doc.solids[0].layer, '0'); assert.ok(s.doc.solids[0].id > 0);
  await s.play('BOX', [['click', 50, 0], ['type', 'C'], ['type', '10']]);
  near(vol(s.doc.solids[1]), 1000);
  await s.play('BOX', [['click', 80, 0], ['type', 'L'], ['type', '4'], ['type', '5'], ['type', '6']]);
  near(vol(s.doc.solids[2]), 120);
  await s.play('BOX', [['type', 'Center'], ['click', 100, 0], ['click', 105, 4], ['type', '2']]);
  near(vol(s.doc.solids[3]), 10 * 8 * 2);
  await s.cmd('U'); await s.finish(); assert.equal(s.doc.solids.length, 3);          // undo covers solids
});

test('CYLINDER, CONE, SPHERE and WEDGE', async () => {
  const s = new Sim();
  await s.play('CYLINDER', [['click', 0, 0], ['type', '5'], ['type', '10']]);
  near(vol(s.doc.solids[0]), Math.PI * 25 * 10, 4);
  await s.play('CYLINDER', [['click', 20, 0], ['type', 'D'], ['type', '10'], ['type', '-4']]);
  assert.ok(S.meshBounds(s.doc.solids[1]).max.z <= 1e-9 && S.meshBounds(s.doc.solids[1]).min.z < -3.9);
  await s.play('CONE', [['click', 40, 0], ['type', '6'], ['type', '12']]);
  near(vol(s.doc.solids[2]), Math.PI * 36 * 12 / 3, 5);
  await s.play('CONE', [['click', 60, 0], ['type', '6'], ['type', 'T'], ['type', '3'], ['type', '12']]);
  near(vol(s.doc.solids[3]), Math.PI * 12 / 3 * (36 + 18 + 9), 6);
  await s.play('SPHERE', [['click', 80, 0], ['type', '10']]);
  near(vol(s.doc.solids[4]) / (4 / 3 * Math.PI * 1000), 1, 0.02);
  await s.play('WEDGE', [['click', 100, 0], ['click', 110, 20], ['type', '30']]);
  near(vol(s.doc.solids[5]), 3000);
});

test('EXTRUDE a closed polyline, a circle and a chain of lines; taper; errors', async () => {
  const s = new Sim();
  s.doc.add(E.pline([P(0, 0), P(10, 0), P(10, 20), P(0, 20)], true));
  s.doc.add(E.circle(P(50, 10), 5));
  await s.cmd('EXTRUDE'); await s.click(5, 0.0); await s.enter();               // pick the polyline edge, then Enter
  await s.type('30'); await s.finish();
  assert.equal(s.doc.solids.length, 1); near(vol(s.doc.solids[0]), 6000);
  assert.deepEqual(s.doc.solids[0].program.features[0].profile, [1]);
  await s.cmd('EXTRUDE'); await s.click(55, 10); await s.enter(); await s.type('10'); await s.finish();
  near(vol(s.doc.solids[1]), Math.PI * 25 * 10, 5);
  // lines forming a closed outline
  const pts = [P(100, 0), P(110, 0), P(110, 10), P(100, 10)];
  pts.forEach((p, i) => s.doc.add(E.line(p, pts[(i + 1) % 4])));
  await s.cmd('EXTRUDE'); await s.type('ALL'); await s.type(''); await s.type('1'); await s.finish();
  assert.equal(s.doc.solids.length, 3);
  // open outline reports an error and creates nothing
  const t = new Sim(); t.doc.add(E.line(P(0, 0), P(10, 0)));
  await t.cmd('EXTRUDE'); await t.click(5, 0); await t.enter(); await t.finish();
  assert.equal(t.doc.solids.length, 0); assert.ok(t.app.history.some(l => /closed/.test(l.text)));
  // taper
  const u = new Sim(); u.doc.add(E.pline([P(0, 0), P(20, 0), P(20, 20), P(0, 20)], true));
  await u.cmd('EXTRUDE'); await u.click(10, 0); await u.enter(); await u.type('T'); await u.type('10'); await u.type('10'); await u.finish();
  const b = S.meshBounds(u.doc.solids[0]); assert.equal(u.doc.solids.length, 1);
  assert.ok(vol(u.doc.solids[0]) < 4000 && vol(u.doc.solids[0]) > 2500, `tapered volume ${vol(u.doc.solids[0])}`); near(b.max.z, 10);
});

test('UNION, SUBTRACT and INTERSECT pick solids on screen', async () => {
  const s = new Sim(); s.ed.visualStyle = 'Shaded';
  await s.play('BOX', [['click', 0, 0], ['click', 10, 10], ['type', '10']]);
  await s.play('BOX', [['click', 5, 0], ['click', 15, 10], ['type', '10']]);
  const [a, b] = s.doc.solids.map(x => x.id);
  await s.cmd('UNION'); await s.click(2, 5); await s.click(13, 5); await s.enter(); await s.finish();
  assert.equal(s.doc.solids.length, 1); near(vol(s.doc.solids[0]), 1500, 1e-4); assert.ok(s.doc.solids[0].id !== a && s.doc.solids[0].id !== b);
  await s.cmd('U'); await s.finish(); assert.equal(s.doc.solids.length, 2);
  await s.cmd('SUBTRACT'); await s.click(2, 5); await s.enter(); await s.click(13, 5); await s.enter(); await s.finish();
  assert.equal(s.doc.solids.length, 1); near(vol(s.doc.solids[0]), 500, 1e-4);
  await s.cmd('U'); await s.finish();
  await s.cmd('INTERSECT'); await s.click(2, 5); await s.click(13, 5); await s.enter(); await s.finish();
  near(vol(s.doc.solids[0]), 500, 1e-4);
});

test('window selecting solids with the pick-first set, ERASE and 3DMOVE', async () => {
  const s = new Sim(); s.ed.visualStyle = 'Shaded';
  await s.play('BOX', [['click', 0, 0], ['click', 10, 10], ['type', '10']]);
  await s.play('BOX', [['click', 30, 0], ['click', 40, 10], ['type', '10']]);
  await s.rt.drag({ x: -2, y: -2 }, { x: 12, y: 12 }); assert.equal(s.ed.selectedSolids.length, 1);
  await s.cmd('3DMOVE'); await s.click(0, 0); await s.click(5, 5); await s.type('7'); await s.finish();
  const bb = S.meshBounds(s.doc.solids[0]); near(bb.min.x, 5); near(bb.min.z, 7);
  await s.rt.drag({ x: -2, y: -2 }, { x: 60, y: 30 }); assert.equal(s.ed.selectedSolids.length, 2);
  await s.cmd('ERASE'); await s.finish(); assert.equal(s.doc.solids.length, 0);
});

test('MOVE, COPY, ROTATE, SCALE and MIRROR act on solids', async () => {
  const s = new Sim(); s.ed.visualStyle = 'Shaded';
  await s.play('BOX', [['click', 0, 0], ['click', 10, 10], ['type', '10']]);
  await s.rt.drag({ x: -2, y: -2 }, { x: 12, y: 12 });
  await s.cmd('MOVE'); await s.click(0, 0); await s.click(20, 5); await s.finish();
  near(S.meshBounds(s.doc.solids[0]).min.x, 20); near(S.meshBounds(s.doc.solids[0]).min.y, 5);
  await s.rt.drag({ x: 18, y: 3 }, { x: 32, y: 17 });
  await s.cmd('COPY'); await s.click(20, 5); await s.click(40, 5); await s.enter(); await s.finish();
  assert.equal(s.doc.solids.length, 2); near(S.meshBounds(s.doc.solids[1]).min.x, 40);
  assert.ok(s.doc.solids[0].id !== s.doc.solids[1].id);
  s.ed.sel = new Set([s.doc.solids[0].id]);
  await s.cmd('SCALE'); await s.click(20, 5); await s.type('2'); await s.finish();
  near(vol(s.doc.solids[0]), 8000, 1e-6); near(S.meshBounds(s.doc.solids[0]).max.z, 20);
  s.ed.sel = new Set([s.doc.solids[1].id]);
  await s.cmd('MIRROR'); await s.click(0, 0); await s.click(0, 10); await s.enter(); await s.finish();     // mirror about the Y axis, keep the original
  assert.equal(s.doc.solids.length, 3); near(vol(s.doc.solids[2]), 1000); assert.ok(S.meshBounds(s.doc.solids[2]).max.x <= -39.99);
  s.ed.sel = new Set([s.doc.solids[1].id]);
  await s.cmd('ROTATE'); await s.click(40, 5); await s.type('90'); await s.finish();
  near(vol(s.doc.solids[1]), 1000, 1e-6);
});

test('PRESSPULL, MASSPROP, UPDATEEXTRUSION and subtractive extrusions', async () => {
  const s = new Sim();
  s.doc.add(E.pline([P(0, 0), P(40, 0), P(40, 40), P(0, 40)], true));
  s.doc.add(E.circle(P(20, 20), 5));
  await s.cmd('PRESSPULL'); await s.click(5, 5); await s.type('8'); await s.finish();             // area with a circular hole
  near(vol(s.doc.solids[0]), (1600 - Math.PI * 25) * 8, 4);
  s.ed.visualStyle = 'Shaded';
  await s.cmd('MASSPROP'); await s.click(5, 5); await s.enter(); await s.finish();
  assert.ok(s.app.history.some(l => /Volume:/.test(l.text)));
  // extrude a circle as a subtractive feature through the plate, then edit the circle and update
  const hole = s.doc.add(E.circle(P(10, 10), 3));
  await s.cmd('EXTRUDE', { args: { op: 'subtract' } });
  await s.click(13, 10); await s.enter(); await s.type('20'); await s.click(5, 5); await s.enter(); await s.finish();
  const v1 = vol(s.doc.solids[0]);
  assert.ok(v1 < (1600 - Math.PI * 25) * 8 - 150, `drilled ${v1}`);
  assert.equal(s.doc.solids[0].program.features.length, 1); assert.ok(s.doc.solids[0].program.base, 'the plate before the cut is kept as the base');
  s.doc.change(() => { hole.r = 5; });
  s.ed.sel = new Set([s.doc.solids[0].id]);
  await s.cmd('UPDATEEXTRUSION'); await s.finish();
  assert.ok(vol(s.doc.solids[0]) < v1 - 100, 'a larger hole removes more material');
  s.doc.remove(hole);
  s.ed.sel = new Set([s.doc.solids[0].id]);
  await s.cmd('UPDATEEXTRUSION'); await s.finish();
  assert.ok(s.app.history.some(l => /deleted/.test(l.text)), 'missing source outline is reported');
});

test('VSCURRENT sets the visual style; solids on frozen layers cannot be picked', async () => {
  const s = new Sim();
  await s.cmd('VSCURRENT'); await s.type('Shaded'); await s.finish(); assert.equal(s.ed.visualStyle, 'Shaded');
  await s.play('BOX', [['click', 0, 0], ['click', 10, 10], ['type', '10']]);
  assert.ok(s.rt.solidAt(s.ed, { x: 5, y: 5 }));
  s.doc.layers[0].frozen = true; assert.equal(s.rt.solidAt(s.ed, { x: 5, y: 5 }), null);
});
