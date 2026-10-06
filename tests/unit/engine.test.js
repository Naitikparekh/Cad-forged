import test from 'node:test';
import assert from 'node:assert/strict';
import { parseInput, parseOverride, matchOption, optionKey, parseNumber } from '../../src/engine/parse.js';
import { View } from '../../src/engine/view.js';

const near = (a, b, t = 1e-9) => assert.ok(Math.abs(a - b) <= t, `${a} != ${b}`);

test('coordinate input: absolute, relative, polar, dynamic-input rules', () => {
  assert.deepEqual(parseInput('10,20').pt, { x: 10, y: 20 });
  assert.deepEqual(parseInput('@5,-3', { base: { x: 10, y: 10 } }).pt, { x: 15, y: 7 });
  const p = parseInput('@10<90', { base: { x: 1, y: 1 } }).pt; near(p.x, 1); near(p.y, 11);
  const q = parseInput('5<180').pt; near(q.x, -5); near(q.y, 0, 1e-12);
  // dynamic input: second point typed without @ is relative; # forces absolute
  assert.deepEqual(parseInput('3,4', { base: { x: 10, y: 10 }, relativeDefault: true }).pt, { x: 13, y: 14 });
  assert.deepEqual(parseInput('#3,4', { base: { x: 10, y: 10 }, relativeDefault: true }).pt, { x: 3, y: 4 });
  assert.deepEqual(parseInput('3,4', { relativeDefault: true }).pt, { x: 3, y: 4 });       // no base point: absolute
  assert.deepEqual(parseInput(' 1 , 2 ').pt, { x: 1, y: 2 });
  assert.equal(parseInput('12.5').kind, 'number'); assert.equal(parseInput('12.5').value, 12.5);
  assert.equal(parseInput('abc'), null); assert.equal(parseInput(''), null);
  assert.equal(parseInput('1,2,3').z, 3);
});

test('numbers accept fractions and exponents', () => {
  assert.equal(parseNumber('1/2'), .5); assert.equal(parseNumber('3-1/4'), 3.25); assert.equal(parseNumber('-2 1/8'), -2.125);
  assert.equal(parseNumber('1e3'), 1000); assert.equal(parseNumber('x'), null);
});

test('object snap overrides', () => {
  assert.deepEqual(parseOverride('END'), { kind: 'osnap', mode: 'END' });
  assert.deepEqual(parseOverride('midpoint'), { kind: 'osnap', mode: 'MID' });
  assert.deepEqual(parseOverride('cen'), { kind: 'osnap', mode: 'CEN' });
  assert.equal(parseOverride('from').kind, 'from'); assert.equal(parseOverride('none').kind, 'none');
  assert.equal(parseOverride('foo'), null);
});

test('keyword options use capital-letter abbreviations and unique prefixes', () => {
  const opts = ['Close', 'Undo', 'Ttr (tan tan radius)', '3P', '2P', 'Diameter', 'eXit'];
  assert.equal(optionKey('Ttr (tan tan radius)'), 'T'); assert.equal(optionKey('3P'), '3P'); assert.equal(optionKey('eXit'), 'X');
  assert.equal(matchOption('c', opts), 'Close'); assert.equal(matchOption('CLOSE', opts), 'Close');
  assert.equal(matchOption('t', opts), 'Ttr (tan tan radius)'); assert.equal(matchOption('3p', opts), '3P');
  assert.equal(matchOption('x', opts), 'eXit'); assert.equal(matchOption('dia', opts), 'Diameter');
  assert.equal(matchOption('zzz', opts), null);
});

test('view: plan transform round-trips and zoom keeps the anchor fixed', () => {
  const v = new View(); v.w = 1000; v.h = 600; v.scale = 4; v.target = { x: 50, y: 20, z: 0 };
  const p = { x: 73.5, y: -12 };
  const s = v.toScreen(p), w = v.toWorld(s.x, s.y);
  near(w.x, p.x, 1e-9); near(w.y, p.y, 1e-9);
  near(v.toScreen({ x: 50, y: 20 }).x, 500); near(v.toScreen({ x: 50, y: 20 }).y, 300);
  assert.ok(v.toScreen({ x: 51, y: 21 }).y < 300);          // +y goes up the screen
  const anchor = v.toWorld(200, 100); v.zoomAt(200, 100, 2.5);
  const after = v.toWorld(200, 100); near(after.x, anchor.x, 1e-9); near(after.y, anchor.y, 1e-9);
  v.yaw = .7; v.pitch = 1.2;
  const s2 = v.toScreen(p), w2 = v.toWorld(s2.x, s2.y); near(w2.x, p.x, 1e-8); near(w2.y, p.y, 1e-8);
  const pr = v.project({ x: p.x, y: p.y, z: 0 }); near(pr.x, s2.x, 1e-8); near(pr.y, s2.y, 1e-8);
});

test('view: fit and standard presets', () => {
  const v = new View(); v.w = 1000; v.h = 500;
  v.fit({ minX: 0, minY: 0, maxX: 100, maxY: 50 });
  near(v.target.x, 50); near(v.target.y, 25);
  const a = v.toScreen({ x: 0, y: 0 }), b = v.toScreen({ x: 100, y: 50 });
  assert.ok(a.x > 0 && b.x < 1000 && b.y > 0 && a.y < 500);
  v.setPreset('front'); assert.equal(v.presetName(), 'Front');
  v.setPreset('swiso'); assert.equal(v.presetName(), 'SW Isometric');
  const ne = v.project({ x: 1, y: 0, z: 0 }), up = v.project({ x: 0, y: 0, z: 1 });
  assert.ok(up.y < v.project({ x: 0, y: 0, z: 0 }).y);        // +z is up the screen
  assert.ok(ne.x > v.project({ x: 0, y: 0, z: 0 }).x);        // from the SW, +x (east) points right
  v.setPreset('top'); assert.equal(v.presetName(), 'Top'); assert.ok(v.isPlan);
  v.setPreset('right'); const r = v.project({ x: 0, y: 1, z: 0 }); assert.ok(r.x > v.project({ x: 0, y: 0, z: 0 }).x);   // from +x looking west, +y is right
});
