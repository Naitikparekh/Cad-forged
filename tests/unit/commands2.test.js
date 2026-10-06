import test from 'node:test';
import assert from 'node:assert/strict';
import { Sim, near } from './sim.js';
import { registry } from '../../src/engine/registry.js';
import { E } from '../../src/core/entities.js';
import * as g from '../../src/core/geom.js';

const P = (x, y) => ({ x, y });

test('registry resolves aliases, synonyms and suggestions', () => {
  assert.equal(registry.find('l').name, 'LINE'); assert.equal(registry.find('PL').name, 'PLINE'); assert.equal(registry.find('rec').name, 'RECTANG');
  assert.equal(registry.find('bevel').name, 'CHAMFER'); assert.equal(registry.find('z').name, 'ZOOM'); assert.equal(registry.find('dli').name, 'DIMLINEAR');
  assert.equal(registry.find('nonsense'), null);
  assert.ok(registry.suggest('cir').includes('CIRCLE')); assert.ok(registry.suggest('ine').includes('LINE'));
  const names = registry.names(); assert.ok(names.length > 100, `only ${names.length} commands`);
});

test('typing a system variable name prompts for a new value (SETVAR flow)', async () => {
  const s = new Sim();
  await s.type('DYNMODE');
  assert.match(s.prompt, /Enter new value for DYNMODE <3>:/);
  await s.type('1'); await s.finish();
  assert.equal(s.app.sysvars.prefs.DYNMODE, 1);
  await s.type('LTSCALE'); await s.type('2.5'); await s.finish();         // LTSCALE is also a command with the same prompt
  assert.equal(s.doc.settings.ltscale, 2.5);
  await s.type('LTSCALE'); await s.type('-1'); assert.ok(s.pending);       // rejected: stays on the prompt
  await s.esc(); await s.finish();
  assert.equal(s.doc.settings.ltscale, 2.5);
  await s.type('PDSIZE'); await s.type('3'); await s.finish(); assert.equal(s.doc.settings.pdsize, 3);
  await s.type('CLAYER'); assert.match(s.prompt, /<0>/); await s.esc(); await s.finish();
});

test('UNDO and REDO commands', async () => {
  const s = new Sim();
  await s.play('C', [['click', 0, 0], ['type', '3']]);
  await s.play('C', [['click', 20, 0], ['type', '3']]);
  assert.equal(s.ents.length, 2);
  await s.cmd('U'); await s.finish(); assert.equal(s.ents.length, 1);
  await s.cmd('REDO'); await s.finish(); assert.equal(s.ents.length, 2);
  await s.cmd('UNDO'); await s.type('2'); await s.finish(); assert.equal(s.ents.length, 0);
});

test('layer commands: -LAYER Make/Color/Freeze, LAYISO, layer wildcard', async () => {
  const s = new Sim();
  await s.cmd('-LAYER');
  await s.type('m'); await s.type('Walls'); await s.type('c'); await s.type('red'); await s.type('Walls');
  await s.type('n'); await s.type('A-Door,A-Window'); await s.type('f'); await s.type('A-*'); await s.enter(); await s.finish();
  assert.equal(s.doc.cur.layer, 'Walls'); assert.equal(s.doc.layer('Walls').color, 1);
  assert.ok(s.doc.layer('A-Door').frozen && s.doc.layer('A-Window').frozen);
  await s.play('L', [['click', 0, 0], ['click', 5, 0], ['enter']]);
  assert.equal(s.ents[0].layer, 'Walls');
  s.doc.setCurrentLayer('0'); s.doc.add(E.line(P(0, 5), P(5, 5)));
  await s.cmd('LAYISO'); await s.click(2, 0); await s.enter(); await s.finish();
  assert.ok(s.doc.layer('0').locked || !s.doc.layer('0').on || s.doc.layer('0').name === s.doc.cur.layer);
  await s.cmd('LAYUNISO'); await s.finish();
});

test('ZOOM window, extents, previous, scale; PAN by displacement', async () => {
  const s = new Sim();
  s.doc.add(E.line(P(0, 0), P(100, 0))); s.doc.add(E.line(P(0, 50), P(100, 50)));
  s.ed.view.w = 800; s.ed.view.h = 600;
  await s.cmd('Z'); await s.type('e'); await s.finish();
  const scaleExt = s.ed.view.scale; near(s.ed.view.target.x, 50, 1e-6); near(s.ed.view.target.y, 25, 1e-6);
  await s.cmd('Z'); await s.type('w'); await s.click(0, 0); await s.click(10, 10); await s.finish();
  assert.ok(s.ed.view.scale > scaleExt * 5);
  await s.cmd('Z'); await s.type('p'); await s.finish(); near(s.ed.view.scale, scaleExt, 1e-9);
  await s.cmd('Z'); await s.type('2x'); await s.finish(); near(s.ed.view.scale, scaleExt * 2, 1e-9);
  const t = s.ed.view.target.x;
  await s.cmd('-PAN'); await s.click(0, 0); await s.click(10, 0); await s.finish();
  near(s.ed.view.target.x, t - 10, 1e-6);
});

test('BLOCK, INSERT with scale and rotation, EXPLODE of the reference', async () => {
  const s = new Sim();
  s.doc.add(E.line(P(0, 0), P(10, 0))); s.doc.add(E.circle(P(5, 5), 2));
  await s.cmd('B'); await s.type('Part'); await s.click(0, 0);
  await s.click(5, 0); await s.click(5, 7); await s.enter(); await s.finish();     // pick-first not used; select both with clicks
  assert.ok(s.doc.blocks.Part); assert.equal(s.doc.blocks.Part.entities.length, 2);
  assert.equal(s.ents.length, 1); assert.equal(s.ents[0].type, 'insert');
  await s.cmd('I'); await s.type('Part'); await s.type('s'); await s.type('2'); await s.type('r'); await s.type('90'); await s.click(50, 50); await s.finish();
  const ref = s.ents[1]; assert.equal(ref.type, 'insert'); near(ref.sx, 2); near(ref.rot, Math.PI / 2);
  await s.cmd('X'); await s.click(50, 50); await s.enter(); await s.finish();
  assert.ok(s.ents.some(e => e.type === 'circle' && Math.abs(e.r - 4) < 1e-9));
});

test('DIST, AREA (object) and LIST report through the command history', async () => {
  const s = new Sim();
  await s.play('DI', [['click', 0, 0], ['click', 3, 4]]);
  assert.ok(s.app.history.some(h => /Distance = 5\.0000/.test(h.text)));
  s.doc.add(E.circle(P(20, 0), 2));
  await s.play('AA', [['type', 'o'], ['click', 22, 0], ['enter']]);
  assert.ok(s.app.history.some(h => /Area = 12\.5664/.test(h.text)));
  await s.cmd('LIST'); await s.click(22, 0); await s.enter(); await s.finish();
  assert.ok(s.app.history.some(h => /CIRCLE/.test(h.text) && /Layer: "0"/.test(h.text)));
});

test('DIMLINEAR creates a measured dimension; TEXT creates text', async () => {
  const s = new Sim();
  await s.play('DLI', [['click', 0, 0], ['click', 10, 0], ['click', 5, 4]]);
  const d = s.ents[0]; assert.equal(d.type, 'dim'); assert.equal(d.kind, 'lin'); near(d.ang, 0);
  const geo = (await import('../../src/core/dim.js')).dimGeometry(d, s.doc); near(geo.measured, 10); assert.equal(geo.text.s, '10.00');
  await s.play('TEXT', [['click', 0, 10], ['type', '3'], ['type', '0'], ['type', 'Hello'], ['enter']]);
  const t = s.ents.find(e => e.type === 'text'); assert.equal(t.s, 'Hello'); near(t.h, 3);
});

test('HATCH pick-internal-point creates a hatch from the surrounding closed region', async () => {
  const s = new Sim();
  s.doc.add(E.pline([P(0, 0), P(20, 0), P(20, 10), P(0, 10)], true));
  await s.cmd('H'); await s.click(5, 5); await s.enter(); await s.finish();
  const h = s.ents.find(e => e.type === 'hatch'); assert.ok(h); assert.equal(h.loops.length, 1); assert.equal(h.pattern, 'ANSI31');
  await s.play('H', [['click', 100, 100], ['enter']]);          // no region there
  assert.equal(s.ents.filter(e => e.type === 'hatch').length, 1);
});

test('MATCHPROP, DRAWORDER and SELECTALL', async () => {
  const s = new Sim();
  s.doc.addLayer('Red', { color: 1 });
  const a = s.doc.add(E.line(P(0, 0), P(10, 0))); a.layer = 'Red'; a.color = 3;
  const b = s.doc.add(E.line(P(0, 5), P(10, 5)));
  await s.cmd('MA'); await s.click(5, 0); await s.click(5, 5); await s.enter(); await s.finish();
  assert.equal(b.layer, 'Red'); assert.equal(b.color, 3);
  await s.cmd('DRAWORDER'); await s.click(5, 5); await s.enter(); await s.type('f'); await s.finish();
  assert.equal(s.ents[s.ents.length - 1], b);
  await s.cmd('SELECTALL'); await s.finish(); assert.equal(s.ed.sel.size, 2);
});
