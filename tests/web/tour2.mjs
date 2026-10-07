import { launch } from './harness.mjs';
import fs from 'node:fs';
const out = process.argv[2]; fs.mkdirSync(out, { recursive: true });
const t = await launch();
const { page } = t;
const shot = async n => { await t.frame(4); await t.shot(`${out}/${n}.png`); };
const click = async (x, y, o) => { await page.mouse.move(x, y); await page.mouse.down(o); await page.mouse.up(o); await t.frame(3); };
await t.cmd('SAMPLE');   // unknown → error line
await click(18, 166); await click(694, 320);              // sample
await click(30, 18);       // app button
await shot('10-appmenu');
await page.keyboard.press('Escape'); await t.frame(3);
await click(458, 49);      // 3D tools tab
await shot('11-3dtab');
await click(100, 49);      // Insert tab
await shot('12-inserttab');
await click(36, 49);       // Home
await click(141, 125);     // circle dropdown arrow
await shot('13-circle-menu');
await page.keyboard.press('Escape'); await t.frame(3);
await click(930, 75);      // layer combo
await shot('14-layer-combo');
await page.keyboard.press('Escape'); await t.frame(3);
await click(700, 400, { button: 'right' });
await shot('15-context');
await page.keyboard.press('Escape'); await t.frame(3);
await t.cmd('DSETTINGS');
await shot('16-dsettings');
await page.keyboard.press('Escape'); await t.frame(3);
await t.cmd('OPTIONS');
await shot('17-options');
await page.keyboard.press('Escape'); await t.frame(3);
await t.cmd('PLOT');
await shot('18-plot');
await page.keyboard.press('Escape'); await t.frame(3);
console.log(t.logs.slice(-10).join('\n'));
await t.close();
