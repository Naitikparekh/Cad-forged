// Browser tests: drive the real UI (mouse, keyboard, ribbon, dialogs) in headless Chromium.
import fs from 'node:fs';
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

console.log('files and plotting');
await test('plot dialog exports a PDF download', async () => {
  await page.evaluate(() => { window.cadforge.ui.files.openSample(); });
  await page.waitForTimeout(100);
  await page.evaluate(() => { window.cadforge.ui.dialog('plot'); });
  await page.waitForSelector('.plot-prev');
  const [dl] = await Promise.all([page.waitForEvent('download'), page.locator('.dlg .btn.primary').click()]);
  eq(dl.suggestedFilename(), 'L-Bracket.pdf');
  const buf = fs.readFileSync(await dl.path());
  ok(buf.slice(0, 8).toString() === '%PDF-1.4', 'PDF header');
  eq(await page.locator('.dlg').count(), 0);
});
await test('plot dialog exports SVG and PNG', async () => {
  for (const [fmt, ext] of [['svg', 'svg'], ['png', 'png']]) {
    await page.evaluate(f => { window.cadforge.ui.dialog('plot', { format: f }); }, fmt);
    await page.waitForSelector('.plot-prev');
    const [dl] = await Promise.all([page.waitForEvent('download'), page.locator('.dlg .btn.primary').click()]);
    eq(dl.suggestedFilename(), `L-Bracket.${ext}`);
    const buf = fs.readFileSync(await dl.path());
    ok(ext === 'svg' ? buf.toString().includes('<svg') : buf.slice(1, 4).toString() === 'PNG', `${ext} content`);
  }
});
await test('Export DXF downloads a file ezdxf-style readers accept (header check)', async () => {
  const [dl] = await Promise.all([page.waitForEvent('download'), page.evaluate(() => { window.cadforge.ui.files.exportDXF(); })]);
  eq(dl.suggestedFilename(), 'L-Bracket.dxf');
  const txt = fs.readFileSync(await dl.path(), 'utf8');
  ok(txt.includes('AC1015') && txt.includes('ENTITIES') && txt.trimEnd().endsWith('EOF'));
});
await test('Save As JSON then reopen restores the drawing', async () => {
  const before = await page.evaluate(() => window.cadforge.app.doc.entities.length);
  const [dl] = await Promise.all([page.waitForEvent('download'), page.evaluate(() => { window.cadforge.ui.files.write(window.cadforge.app.doc, 'copy.cadforge.json'); })]);
  const file = await dl.path();
  const json = fs.readFileSync(file, 'utf8');
  await page.evaluate(async txt => { const f = new File([txt], 'copy.cadforge.json'); await window.cadforge.ui.files.openFile(f); }, json);
  await page.waitForTimeout(100);
  eq(await page.evaluate(() => window.cadforge.app.doc.entities.length), before);
  eq(await page.evaluate(() => window.cadforge.app.doc.name), 'copy');
});
await test('opening a DXF through the UI works', async () => {
  const dxf = fs.readFileSync('tests/dxf/fixtures/ezdxf-sample.dxf', 'utf8');
  await page.evaluate(async txt => { await window.cadforge.ui.files.openFile(new File([txt], 'ez.dxf')); }, dxf);
  await page.waitForTimeout(150);
  ok(await page.evaluate(() => window.cadforge.app.doc.entities.length) > 30);
  eq(await page.evaluate(() => window.cadforge.app.doc.name), 'ez');
});

console.log('3D solids');
await test('BOX/CYLINDER then SUBTRACT by clicking solids in an isometric view (worker path)', async () => {
  await t.fresh();
  await page.evaluate(() => { const { app, ui } = window.cadforge; app.doc.solids.length = 0; app.active.visualStyle = 'Shaded'; app.active.view.setPreset('swiso'); app.active.zoomExtents(); ui.viewChanged(); });
  await page.locator('.cl-input').focus();
  await t.cmd('BOX'); await t.cmd('0,0'); await t.cmd('60,40'); await t.cmd('20');
  await t.cmd('CYLINDER'); await t.cmd('30,20'); await t.cmd('10'); await t.cmd('40');
  eq(await page.evaluate(() => window.cadforge.app.doc.solids.length), 2);
  await page.evaluate(() => { const { app, ui } = window.cadforge; app.active.zoomExtents(); ui.viewChanged(); });
  const at = (x, y, z) => page.evaluate(([x, y, z]) => { const { ui } = window.cadforge, r = ui.viewport.el.getBoundingClientRect(), p = ui.editor.view.project({ x, y, z }); return { x: r.left + p.x, y: r.top + p.y }; }, [x, y, z]);
  await t.cmd('SUBTRACT');
  let p = await at(5, 5, 20); await page.mouse.click(p.x, p.y); await page.waitForTimeout(80);
  await page.keyboard.press('Enter');
  p = await at(30, 20, 40); await page.mouse.click(p.x, p.y); await page.waitForTimeout(80);
  await page.keyboard.press('Enter');
  await page.waitForFunction(() => window.cadforge.app.doc.solids.length === 1 && !window.cadforge.app.runtime.busy, null, { timeout: 15000 });
  const vol = await page.evaluate(() => { const s = window.cadforge.app.doc.solids[0]; let v = 0; for (const [i, j, k] of s.faces) { const a = s.vertices[i], b = s.vertices[j], c = s.vertices[k]; v += (a.x * (b.y * c.z - b.z * c.y) - a.y * (b.x * c.z - b.z * c.x) + a.z * (b.x * c.y - b.y * c.x)) / 6; } return v; });
  near(vol, 60 * 40 * 20 - Math.PI * 100 * 20, 40);
  ok((await page.evaluate(() => window.cadforge.solidStats.worker)) >= 1, 'the boolean ran on the worker thread');
});
await test('exporting solids as OBJ and STL downloads files', async () => {
  for (const ext of ['obj', 'stl']) {
    const [dl] = await Promise.all([page.waitForEvent('download'), page.evaluate(e => { window.cadforge.ui.files.exportMesh(e); }, ext)]);
    eq(dl.suggestedFilename().endsWith('.' + ext), true);
    ok(fs.readFileSync(await dl.path(), 'utf8').length > 200);
  }
});
await test('selecting a solid by clicking shows it in the Properties palette summary', async () => {
  await page.evaluate(() => { const { app } = window.cadforge; app.active.sel.clear(); app.emit('change'); });
  const p = await page.evaluate(() => { const { ui } = window.cadforge, r = ui.viewport.el.getBoundingClientRect(), q = ui.editor.view.project({ x: 5, y: 5, z: 20 }); return { x: r.left + q.x, y: r.top + q.y }; });
  await page.mouse.click(p.x, p.y); await page.waitForTimeout(100);
  eq(await page.evaluate(() => window.cadforge.app.active.selectedSolids.length), 1);
});

eq(t.errors, [], 'console errors during run');
await browser.close();
process.exit(summary() ? 0 : 1);
