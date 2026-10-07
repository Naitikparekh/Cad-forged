// Functional tests for the Rust (egui) build in headless Chromium: node tests/web/run.mjs
import { launch } from './harness.mjs';
import assert from 'node:assert/strict';
const t = await launch();
const { page } = t;
let pass = 0, fail = 0;
async function test(name, fn) {
  try { await fn(); pass++; console.log(`  ok   ${name}`); } catch (e) { fail++; console.log(`  FAIL ${name}\n       ${String(e.message).split('\n').join('\n       ')}`); }
}
const fresh = async () => { await t.page.keyboard.press('Escape'); await t.cmd('ERASE'); await t.cmd('ALL'); await t.page.keyboard.press('Enter'); await t.frame(3); };

await test('app boots with an empty drawing', async () => {
  const s = await t.state();
  assert.equal(s.editors, 1); assert.equal(s.entities, 0); assert.equal(s.busy, false);
});

await test('LINE with clicks creates a line and shows dynamic prompts', async () => {
  await page.mouse.move(700, 400);
  await t.cmd('L');
  assert.equal((await t.state()).busy, true);
  await t.click(10, 10); await t.click(60, 40); await page.keyboard.press('Enter'); await t.frame(3);
  const s = await t.state();
  assert.deepEqual(s.types, ['line']); assert.equal(s.busy, false);
});

await test('CIRCLE by typed centre and radius', async () => {
  await t.cmd('C'); await t.cmd('30,30'); await t.cmd('12');
  const s = await t.state();
  assert.deepEqual(s.types, ['line', 'circle']);
});

await test('click selects, window selects, Delete erases', async () => {
  await page.keyboard.press('Escape'); await t.frame(2);
  await t.click(35, 25);                          // on the line
  assert.equal((await t.state()).selected.length, 1);
  await page.keyboard.press('Delete'); await t.frame(3);
  assert.deepEqual((await t.state()).types, ['circle']);
  await t.drag(0, 0, 80, 80);                     // window around the circle
  assert.equal((await t.state()).selected.length, 1);
  await page.keyboard.press('Escape'); await t.frame(2);
});

await test('undo and redo', async () => {
  await t.cmd('U');
  assert.deepEqual((await t.state()).types, ['line', 'circle']);
  await t.cmd('REDO');
  assert.deepEqual((await t.state()).types, ['circle']);
});

await test('ribbon button starts a command and Esc cancels it', async () => {
  await page.mouse.click(32, 90);                 // Line button
  await t.frame(3);
  assert.equal((await t.state()).busy, true);
  await page.keyboard.press('Escape'); await t.frame(3);
  assert.equal((await t.state()).busy, false);
});

await test('wheel zooms about the cursor', async () => {
  const before = (await t.state()).view.scale;
  await page.mouse.move(700, 450); await page.mouse.wheel(0, -300); await t.frame(4);
  assert.ok((await t.state()).view.scale > before);
});

await test('ViewCube click changes the view; SW Iso preset via the cube home', async () => {
  await page.mouse.click(1321, 205);              // home icon on the cube
  await page.waitForTimeout(600);
  const s = await t.state();
  assert.ok(s.view.pitch < 1.0, `pitch ${s.view.pitch}`);
});

console.log(`\n${pass} passed, ${fail} failed`);
if (t.logs.some(l => l.startsWith('[pageerror]') || l.includes('panicked'))) { console.log(t.logs.filter(l => l.startsWith('[pageerror]') || l.includes('panicked')).join('\n')); fail++; }
await t.close();
process.exit(fail ? 1 : 0);
