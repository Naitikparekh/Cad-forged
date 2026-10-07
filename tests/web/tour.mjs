// Takes a tour through the Rust UI and saves screenshots (node tests/web/tour.mjs outdir)
import { launch } from './harness.mjs';
import fs from 'node:fs';
const out = process.argv[2]; fs.mkdirSync(out, { recursive: true });
const t = await launch();
const { page } = t;
const shot = async n => { await t.frame(4); await t.shot(`${out}/${n}.png`); };
const click = async (x, y) => { await page.mouse.move(x, y); await page.mouse.down(); await page.mouse.up(); await t.frame(3); };
// sample drawing through the start page
await click(18, 166);          // Start tab
await shot('01-start');
await click(694, 320);   // Sample tile
await shot('02-sample');
// select a hole circle by clicking its edge, then a window selection
await page.mouse.move(700, 500); await t.frame();
await shot('03-hover');
// properties palette
await t.cmd('PROPERTIES');
await shot('04-properties-none');
await click(700, 560);
await shot('05-properties-sel');
await t.cmd('LAYER');
await shot('06-layers');
console.log(t.logs.slice(-10).join('\n'));
await t.close();
