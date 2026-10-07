import { launch } from './harness.mjs';
import fs from 'node:fs';
const out = process.argv[2]; fs.mkdirSync(out, { recursive: true });
const t = await launch();
const { page } = t;
const shot = async n => { await t.frame(4); await t.shot(`${out}/${n}.png`); };
await page.mouse.click(18, 166); await t.frame(3); await page.mouse.click(694, 320); await t.frame(4);   // sample
await t.click(60, 0);        // click on the bottom edge of the bracket (selects the pline)
await t.cmd('PROPERTIES');
await shot('20-props-pline');
await t.cmd('LAYER');
await shot('21-layers');
await page.keyboard.press('Escape'); await t.frame(3);
// grips
await t.drag(-20, -20, 140, 100);
await shot('22-grips');
await page.keyboard.press('Escape'); await t.frame(3);
// 3D: box + iso view
await t.cmd('BOX'); await t.cmd('150,0'); await t.cmd('180,40'); await t.cmd('30');
await t.cmd('3DORBIT');
await page.keyboard.press('Escape'); await t.frame(2);
await page.mouse.click(1321, 205); await page.waitForTimeout(700); await t.frame(3);
await t.cmd('ZOOM'); await t.cmd('E');
await shot('23-3d');
console.log(JSON.stringify(await t.state()).slice(0, 400));
console.log(t.logs.slice(-8).join('\n'));
await t.close();
