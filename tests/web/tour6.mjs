import { launch } from './harness.mjs';
import fs from 'node:fs';
const out = process.argv[2]; fs.mkdirSync(out, { recursive: true });
const t = await launch();
const { page } = t;
const shot = async n => { await t.frame(5); await t.shot(`${out}/${n}.png`); };
await page.mouse.click(18, 166); await t.frame(3); await page.mouse.click(694, 320); await t.frame(4);   // sample
await shot('50-model');
await t.cmd('LAYOUT'); await t.cmd('Set'); await t.cmd('Layout1');
await shot('51-layout1');
await t.cmd('MSPACE'); await shot('52-mspace');
console.log(JSON.stringify(await t.state()).slice(0, 300));
console.log(t.logs.filter(l => /error|panic/i.test(l)).slice(-8).join('\n'));
await t.close();
