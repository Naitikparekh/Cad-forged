import test from 'node:test';
import assert from 'node:assert/strict';
import { Sim, near } from './sim.js';
import { E, segsOf } from '../../src/core/entities.js';
import * as g from '../../src/core/geom.js';

const P = (x, y) => ({ x, y });
const addLine = (s, a, b) => s.doc.add(E.line(a, b));

test('ERASE with pick-first selection, and OOPS restores', async () => {
  const s = new Sim();
  addLine(s, P(0, 0), P(10, 0)); addLine(s, P(0, 5), P(10, 5));
  await s.click(5, 0);                     // idle click selects the first line
  assert.equal(s.ed.sel.size, 1);
  await s.cmd('E'); await s.finish();      // pick-first: erases without prompting
  assert.equal(s.ents.length, 1);
  await s.cmd('OOPS'); await s.finish();
  assert.equal(s.ents.length, 2);
});

test('MOVE with preview and second point; COPY makes multiple copies; selection is cleared', async () => {
  const s = new Sim();
  addLine(s, P(0, 0), P(10, 0));
  await s.cmd('M'); await s.click(5, 0); await s.enter();
  await s.click(0, 0); await s.move(3, 4);
  assert.equal(s.rt.preview.length, 1); near(s.rt.preview[0].a.x, 3);
  await s.click(3, 4); await s.finish();
  near(s.ents[0].a.x, 3); near(s.ents[0].a.y, 4);
  assert.equal(s.ed.sel.size, 0);
  await s.cmd('CO'); await s.click(8, 4); await s.enter();
  await s.click(3, 4); await s.click(3, 14); await s.click(3, 24); await s.enter(); await s.finish();
  assert.equal(s.ents.length, 3);
  near(s.ents[2].a.y, 24);
});

test('ROTATE (typed angle and Copy), SCALE and MIRROR', async () => {
  const s = new Sim();
  addLine(s, P(0, 0), P(10, 0));
  await s.cmd('RO'); await s.click(5, 0); await s.enter(); await s.click(0, 0); await s.type('90'); await s.finish();
  near(s.ents[0].b.x, 0); near(s.ents[0].b.y, 10);
  await s.cmd('SC'); await s.click(0, 5); await s.enter(); await s.click(0, 0); await s.type('2.5'); await s.finish();
  near(s.ents[0].b.y, 25);
  await s.cmd('MI'); await s.click(0, 12); await s.enter(); await s.click(0, 0); await s.click(10, 0); await s.type('n'); await s.finish();
  assert.equal(s.ents.length, 2); near(s.ents[1].b.y, -25);
  await s.cmd('MI'); await s.click(0, 12); await s.enter(); await s.click(0, 0); await s.click(10, 0); await s.type('y'); await s.finish();
  assert.equal(s.ents.length, 2);
  await s.cmd('RO'); await s.click(0, -12); await s.enter(); await s.click(0, 0); await s.type('c'); await s.type('90'); await s.finish();
  assert.equal(s.ents.length, 3);
});

test('OFFSET distance then side picks', async () => {
  const s = new Sim();
  addLine(s, P(0, 0), P(10, 0));
  await s.cmd('O'); await s.type('3');
  await s.click(5, 0); await s.click(5, 8);            // above
  await s.click(5, 0); await s.click(5, -8);           // below
  await s.enter(); await s.finish();
  assert.equal(s.ents.length, 3);
  near(s.ents[1].a.y, 3); near(s.ents[2].a.y, -3);
  assert.equal(s.doc.settings.offsetGap, 3);
});

test('TRIM and EXTEND with cutting edges', async () => {
  const s = new Sim();
  addLine(s, P(0, 0), P(10, 0)); addLine(s, P(4, -3), P(4, 3)); addLine(s, P(8, -3), P(8, 3));
  await s.cmd('TR'); await s.enter();                  // all objects are cutting edges
  await s.click(6, 0); await s.enter(); await s.finish();
  assert.equal(s.ents.length, 4);                      // middle piece removed: line splits in two
  const horiz = s.ents.filter(e => e.a.y === 0 && e.b.y === 0);
  assert.equal(horiz.length, 2);
  await s.cmd('EX'); await s.enter();
  await s.click(3.5, 0); await s.enter(); await s.finish();
  const longer = s.ents.filter(e => e.a.y === 0).sort((a, b) => a.a.x - b.a.x);
  near(longer[0].b.x, 8);                              // the piece ends on the x=4 edge; EXTEND runs on to the next edge beyond its end
});

test('FILLET and CHAMFER between two lines, polyline fillet', async () => {
  const s = new Sim();
  addLine(s, P(0, 0), P(10, 0)); addLine(s, P(0, 0), P(0, 10));
  await s.cmd('F'); await s.type('r'); await s.type('2');
  await s.click(8, 0); await s.click(0, 8); await s.finish();
  assert.equal(s.ents.length, 3);
  const arc = s.ents[2]; assert.equal(arc.type, 'arc'); near(arc.r, 2); near(arc.c.x, 2); near(arc.c.y, 2);
  const t = new Sim();
  addLine(t, P(0, 0), P(10, 0)); addLine(t, P(0, 0), P(0, 10));
  await t.cmd('CHA'); await t.type('d'); await t.type('2'); await t.type('3');
  await t.click(8, 0); await t.click(0, 8); await t.finish();
  assert.equal(t.ents.length, 3);
  near(t.ents[2].a.x, 2); near(t.ents[2].b.y, 3);
  const u = new Sim();
  u.doc.add(E.pline([P(0, 0), P(10, 0), P(10, 10), P(0, 10)], true));
  u.doc.settings.filletRadius = 1;
  await u.cmd('F'); await u.type('p'); await u.click(5, 0); await u.finish();
  assert.equal(u.ents[0].v.length, 8);
});

test('ARRAY rectangular and polar', async () => {
  const s = new Sim();
  s.doc.add(E.circle(P(0, 0), 1));
  await s.cmd('ARRAYRECT'); await s.click(1, 0); await s.enter();
  await s.type('2'); await s.type('3'); await s.type('10'); await s.type('20'); await s.finish();
  assert.equal(s.ents.length, 6);
  const t = new Sim();
  t.doc.add(E.circle(P(10, 0), 1));
  await t.cmd('ARRAYPOLAR'); await t.click(11, 0); await t.enter();
  await t.click(0, 0); await t.type('4'); await t.type('360'); await t.type('y'); await t.finish();
  assert.equal(t.ents.length, 4);
  near(t.ents[1].c.x, 0, 1e-6); near(t.ents[1].c.y, 10, 1e-6);
});

test('STRETCH moves only the endpoints inside the crossing window', async () => {
  const t = new Sim();
  addLine(t, P(0, 0), P(10, 0));
  await t.cmd('S'); await t.click(12, 3); await t.click(8, -3);     // right-to-left = crossing
  await t.enter(); await t.click(10, 0); await t.click(15, 5); await t.finish();
  near(t.ents[0].a.x, 0); near(t.ents[0].b.x, 15); near(t.ents[0].b.y, 5);
});

test('BREAK, JOIN and EXPLODE', async () => {
  const s = new Sim();
  addLine(s, P(0, 0), P(10, 0));
  await s.cmd('BR'); await s.click(3, 0); await s.click(6, 0); await s.finish();
  assert.equal(s.ents.length, 2);
  await s.cmd('J'); await s.click(1, 0); await s.click(9, 0); await s.enter(); await s.finish();
  assert.equal(s.ents.length, 2);                     // gap remains: cannot join disconnected lines
  const t = new Sim();
  t.doc.add(E.pline([P(0, 0), P(10, 0), P(10, 10)], false));
  await t.cmd('X'); await t.click(5, 0); await t.enter(); await t.finish();
  assert.equal(t.ents.length, 2); assert.equal(t.ents[0].type, 'line');
});

test('grip editing: stretch an endpoint and move a line from its midpoint grip', async () => {
  const s = new Sim();
  addLine(s, P(0, 0), P(10, 0));
  await s.click(5, 0);                                  // select
  await s.click(10, 0);                                 // hot grip at the endpoint
  assert.equal(s.rt.ctx.name, '_GRIPEDIT');
  await s.click(10, 10); await s.finish();
  near(s.ents[0].b.y, 10); near(s.ents[0].a.y, 0);
  await s.click(5, 5);                                  // midpoint grip (move)
  await s.click(5, 8);
  await s.finish();
  near(s.ents[0].a.y, 3); near(s.ents[0].b.y, 13);
  assert.equal(s.doc.undoStack.length, 2);
});

test('selecting by window and crossing at idle, Esc clears', async () => {
  const s = new Sim();
  addLine(s, P(0, 0), P(4, 0)); addLine(s, P(6, 0), P(14, 0));
  await s.click(-1, -1); await s.click(5, 1);           // left-to-right window: only the first line is inside
  assert.equal(s.ed.sel.size, 1);
  await s.esc(); assert.equal(s.ed.sel.size, 0);
  await s.click(10, 1); await s.click(3, -1);           // right-to-left crossing: both touched
  assert.equal(s.ed.sel.size, 2);
});
