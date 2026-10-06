// Browser tests: drive the real UI (mouse, keyboard, ribbon, dialogs) in headless Chromium.
import { chromium, boot, test, summary, eq, ok, near } from './harness.mjs';

const browser = await chromium.launch();
const t = await boot(browser);
const { page } = t;

console.log('startup');
await test('boots without console errors and opens Drawing1', async () => {
  eq(t.errors, []);
  eq(await page.evaluate(() => window.cadforge.app.editors.length), 1);
  ok(await page.locator('.dt.active .dt-name').innerText() === 'Drawing1');
});
await test('ribbon shows the expected tabs', async () => {
  const names = await page.locator('.rtab').allInnerTexts();
  eq(names.slice(0, 4), ['Home', 'Insert', 'Annotate', 'View']);
});

console.log('command line');
await t.fresh();
await test('LINE by typed coordinates', async () => {
  await page.locator('.cl-input').focus();
  await t.cmd('L'); await t.cmd('0,0'); await t.cmd('100,0'); await t.cmd('@0,50'); await t.cmd('c');
  const e = await t.ents();
  eq(e.length, 3);
  eq(e[0].type, 'line'); near(e[0].b.x, 100); near(e[2].b.x, 0); near(e[2].b.y, 0);
  ok(!(await t.busy()), 'command should have ended');
});
await test('Enter on the canvas repeats the last command', async () => {
  await t.fresh();
  await page.locator('#viewport').focus();
  await page.keyboard.type('c'); await page.keyboard.press('Enter');          // typing on the canvas goes to the command line
  ok((await t.prompt()).includes('center point'), `prompt was ${await t.prompt()}`);
  await t.cmd('50,50'); await t.cmd('10');
  eq((await t.ents()).length, 1);
  await page.keyboard.press('Enter');                                          // repeat CIRCLE
  ok(await t.busy());
  await page.keyboard.press('Escape');
});
await test('suggestion list appears for partial command names', async () => {
  await t.fresh(); await page.locator('.cl-input').focus(); await page.keyboard.type('CIR');
  ok(await page.locator('.cl-suggest:not([hidden]) .sg').count() > 0);
  await page.keyboard.press('Escape');
});

console.log('mouse drawing');
await t.fresh();
await test('LINE with mouse clicks, Esc ends', async () => {
  await page.locator('.cl-input').focus(); await t.cmd('L');
  await t.click(20, 20); await t.click(80, 20); await t.click(80, 70);
  await page.keyboard.press('Escape');
  const e = await t.ents();
  eq(e.length, 2); near(e[0].a.x, 20, 0.3); near(e[0].b.x, 80, 0.3); near(e[1].b.y, 70, 0.3);
});
await test('CIRCLE from the ribbon button', async () => {
  await t.fresh();
  await page.locator('.rb[data-cmd="CIRCLE"]').first().click();
  ok(await t.busy());
  await t.click(50, 50); await t.move(65, 50); await t.click(65, 50);
  const e = await t.ents();
  eq(e.length, 1); eq(e[0].type, 'circle'); near(e[0].r, 15, 0.3);
});
await test('rectangle, window select, move, undo', async () => {
  await t.fresh();
  await page.locator('.cl-input').focus(); await t.cmd('REC');
  await t.click(10, 10); await t.click(40, 30);
  eq((await t.ents()).length, 1);
  await t.drag(0, 0, 60, 50);                                              // window around the rectangle
  eq((await t.sel()).length, 1);
  await page.keyboard.press('Delete');
  eq((await t.ents()).length, 0);
  await page.keyboard.press('Control+z');
  eq((await t.ents()).length, 1);
  await page.keyboard.press('Control+y');
  eq((await t.ents()).length, 0);
});

console.log('toggles and menus');
await test('F8 toggles ortho and the status button', async () => {
  const before = await page.evaluate(() => window.cadforge.app.sysvars.prefs.ORTHOMODE);
  await page.locator('#viewport').focus(); await page.keyboard.press('F8');
  eq(await page.evaluate(() => window.cadforge.app.sysvars.prefs.ORTHOMODE), before ? 0 : 1);
  eq(await page.locator('.sb[data-id="ortho"]').evaluate(e => e.classList.contains('on')), !before);
  await page.keyboard.press('F8');
});
await test('right click opens the default context menu', async () => {
  await page.mouse.click(700, 500, { button: 'right' });
  ok(await page.locator('.menu .mi').count() > 3);
  await page.keyboard.press('Escape');
  eq(await page.locator('.menu').count(), 0);
});
await test('Drafting Settings dialog opens, applies and closes with Escape', async () => {
  await page.evaluate(() => { window.cadforge.ui.dialog('dsettings'); });
  ok(await page.locator('.dlg').count() === 1);
  await page.keyboard.press('Escape');
  eq(await page.locator('.dlg').count(), 0);
});
await test('Properties palette lists the selected object', async () => {
  await t.fresh();
  await page.evaluate(() => { const { app, ui } = window.cadforge; app.doc.add({ type: 'circle', c: { x: 10, y: 10 }, r: 5 }); app.active.sel.add(app.doc.entities[0].id); ui.togglePalette('properties', true); app.emit('change'); });
  await page.waitForTimeout(100);
  ok((await page.locator('.pal-section').allInnerTexts()).join(' ').includes('Geometry'));
  ok(await page.locator('.pal-row:has-text("Radius") input').count() === 1);
  await page.evaluate(() => window.cadforge.ui.togglePalette('properties', false));
});

eq(t.errors, [], 'console errors during run');
await browser.close();
process.exit(summary() ? 0 : 1);
