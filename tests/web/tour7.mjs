import { launch } from './harness.mjs';
import fs from 'node:fs';
const out = process.argv[2]; fs.mkdirSync(out, { recursive: true });
const t = await launch();
const { page } = t;
const shot = async n => { await t.frame(5); await t.shot(`${out}/${n}.png`); };
await page.mouse.click(18, 166); await t.frame(3); await page.mouse.click(694, 320); await t.frame(4);   // sample
const tabs = [['insert', 100], ['annotate', 173], ['layout', 248], ['view', 312], ['manage', 360], ['output', 420], ['3d', 527]];
for (const [n, x] of tabs) { await page.mouse.move(x, 49, { steps: 3 }); await t.frame(2); await page.mouse.click(x, 49); await t.frame(4); await shot(`60-tab-${n}`); }
console.log(t.logs.filter(l => /error|panic/i.test(l)).slice(-8).join('\n'));
await t.close();
