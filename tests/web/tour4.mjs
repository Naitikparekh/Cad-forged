import { launch } from './harness.mjs';
import fs from 'node:fs';
const out = process.argv[2]; fs.mkdirSync(out, { recursive: true });
const t = await launch();
const { page } = t;
const shot = async n => { await t.frame(4); await t.shot(`${out}/${n}.png`); };
const esc = async () => { await page.keyboard.press('Escape'); await t.frame(3); };
await page.mouse.click(18, 166); await t.frame(3); await page.mouse.click(694, 320); await t.frame(4);   // sample
for (const c of ['PLOT', 'DIMSTYLE', 'OPTIONS', 'LINETYPE', 'COLOR', 'LWEIGHT', 'UNITS', 'TOOLPALETTES', 'QUICKPROPERTIES']) {
  await t.cmd(c); await shot(`30-${c.toLowerCase()}`); await esc(); await esc();
}
console.log(JSON.stringify(await t.state()).slice(0, 200));
console.log(t.logs.filter(l => !l.includes('[log]')).slice(-8).join('\n'));
await t.close();
