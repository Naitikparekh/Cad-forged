import test from 'node:test';
import assert from 'node:assert/strict';
import { Sim, near } from './sim.js';
import * as g from '../../src/core/geom.js';

test('LINE draws segments, supports Undo and Close, and the prompt lists options', async () => {
  const s = new Sim();
  await s.cmd('LINE');
  assert.equal(s.prompt, 'Specify first point:');
  await s.click(0, 0);
  assert.equal(s.prompt, 'Specify next point:');
  await s.click(10, 0); await s.click(10, 10);
  assert.equal(s.prompt, 'Specify next point or [Close/Undo]:');
  await s.click(0, 10);
  await s.type('u');
  assert.equal(s.ents.length, 2);
  await s.click(5, 12);
  await s.type('c');
  await s.finish();
  assert.equal(s.ents.length, 4);
  assert.deepEqual(s.ents.at(-1).b, { x: 0, y: 0 });
  assert.equal(s.doc.undoStack.length, 1);
  s.doc.undo(); assert.equal(s.ents.length, 0);
});

test('LINE accepts typed absolute, relative and polar coordinates and direct distance entry', async () => {
  const s = new Sim();
  await s.cmd('L');
  await s.type('1,1');
  await s.type('@5,0');
  await s.type('@5<90');
  await s.move(11, 8);           // cursor points up-right-ish: direction (6,1)... DDE uses the cursor direction from the last point
  await s.type('2');
  await s.enter();
  await s.finish();
  assert.equal(s.ents.length, 3);
  near(s.ents[0].b.x, 6); near(s.ents[0].b.y, 1);
  near(s.ents[1].b.x, 6); near(s.ents[1].b.y, 6);
  const d = g.sub(s.ents[2].b, s.ents[2].a); near(g.len(d), 2);
});

test('Escape cancels but keeps entities already created; Enter repeats the last command', async () => {
  const s = new Sim();
  await s.cmd('LINE'); await s.click(0, 0); await s.click(5, 0); await s.esc(); await s.finish();
  assert.equal(s.ents.length, 1);
  await s.type('');                // Enter at idle repeats LINE
  assert.equal(s.rt.pending.message, 'Specify first point');
  await s.esc(); await s.finish();
});

test('CIRCLE: centre+radius by click, typed radius, diameter, 2P, 3P', async () => {
  const s = new Sim();
  await s.play('C', [['click', 0, 0], ['click', 3, 4]]);
  near(s.ents[0].r, 5);
  await s.play('CIRCLE', [['click', 20, 0], ['type', '2.5']]);
  near(s.ents[1].r, 2.5);
  await s.play('CIRCLE', [['click', 40, 0], ['type', 'd'], ['type', '8']]);
  near(s.ents[2].r, 4);
  await s.play('CIRCLE', [['type', '2p'], ['click', 0, 20], ['click', 10, 20]]);
  near(s.ents[3].r, 5); near(s.ents[3].c.x, 5);
  await s.play('CIRCLE', [['type', '3p'], ['click', 10, 0 + 40], ['click', 0, 50], ['click', -10, 40]]);
  near(s.ents[4].r, 10); near(s.ents[4].c.y, 40);
  // default radius is remembered
  await s.cmd('C'); await s.click(100, 100); assert.match(s.prompt, /<10>/);
  await s.enter(); await s.finish(); near(s.ents[5].r, 10);
});

test('RECTANG with fillet option produces a rounded closed polyline', async () => {
  const s = new Sim();
  await s.play('REC', [['type', 'f'], ['type', '2'], ['click', 0, 0], ['click', 20, 10]]);
  const e = s.ents[0];
  assert.equal(e.type, 'pline'); assert.ok(e.closed); assert.equal(e.v.length, 8);
  near(Math.abs(g.plineArea(e.v)), 200 - (4 - Math.PI) * 4, 1e-6);
  await s.play('REC', [['type', 'f'], ['type', '0'], ['click', 50, 0], ['type', 'd'], ['type', '10'], ['type', '6'], ['click', 60, 10]]);
  near(Math.abs(g.plineArea(s.ents[1].v)), 60);
});

test('POLYGON, ELLIPSE and ARC (three-point, start-center-end)', async () => {
  const s = new Sim();
  await s.play('POL', [['type', '6'], ['click', 0, 0], ['type', 'c'], ['click', 5, 0]]);          // circumscribed about a circle radius 5
  assert.equal(s.ents[0].v.length, 6);
  const apothem = g.dist({ x: 0, y: 0 }, g.mid(s.ents[0].v[0], s.ents[0].v[1])); near(apothem, 5, 1e-6);
  await s.play('EL', [['click', 100, 0], ['click', 110, 0], ['click', 100, 3]]);
  near(s.ents[1].ratio, .6); near(s.ents[1].c.x, 105);
  await s.play('ARC', [['click', 10, 0], ['click', 0, 10], ['click', -10, 0]]);
  near(s.ents[2].r, 10); near(g.sweepCCW(s.ents[2].a0, s.ents[2].a1), Math.PI);
  await s.play('ARC', [['type', 'c'], ['click', 0, 0], ['click', 5, 0], ['click', 0, 9]]);
  near(s.ents[3].r, 5); near(s.ents[3].a0, 0); near(s.ents[3].a1, Math.PI / 2);
});

test('PLINE with an arc segment, undo and close', async () => {
  const s = new Sim();
  await s.cmd('PL'); await s.click(0, 0); await s.click(10, 0);
  await s.type('a'); await s.click(10, 10);          // tangent arc up to (10,10)
  await s.type('l'); await s.click(0, 10);
  await s.type('c'); await s.finish();
  const e = s.ents[0];
  assert.equal(e.type, 'pline'); assert.ok(e.closed); assert.equal(e.v.length, 4);
  assert.ok(Math.abs(e.v[1].b) > 0.1);
  near(e.v[1].x, 10); near(e.v[1].y, 0);
});

test('SPLINE, POINT, XLINE and DONUT create objects', async () => {
  const s = new Sim();
  await s.play('SPL', [['click', 0, 0], ['click', 5, 5], ['click', 10, 0], ['enter']]);
  assert.equal(s.ents[0].type, 'spline'); assert.equal(s.ents[0].fit.length, 3);
  await s.play('PO', [['click', 3, 3]]); assert.equal(s.ents[1].type, 'point');
  await s.play('XL', [['type', 'h'], ['click', 0, 7], ['enter']]); assert.equal(s.ents[2].type, 'xline'); near(s.ents[2].d.y, 0);
  await s.play('DONUT', [['type', '2'], ['type', '6'], ['click', 0, 0], ['enter']]);
  const d = s.ents[3]; assert.equal(d.type, 'hatch'); assert.equal(d.loops.length, 2);
});
