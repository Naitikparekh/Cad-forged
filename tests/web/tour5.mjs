import { launch } from './harness.mjs';
import fs from 'node:fs';
const out = process.argv[2]; fs.mkdirSync(out, { recursive: true });
const t = await launch();
const { page } = t;
const shot = async n => { await t.frame(4); await t.shot(`${out}/${n}.png`); };
const esc = async () => { await page.keyboard.press('Escape'); await t.frame(3); };
await t.cmd('REC'); await t.cmd('0,0'); await t.cmd('100,60');
await t.cmd('ZOOM'); await t.cmd('E');
// hatch: contextual tab
await t.cmd('HATCH'); await t.frame(3); await shot('40-hatch-prompt');
const p = await t.pt(50, 30); await page.mouse.click(p.x, p.y); await t.frame(4);
await shot('41-hatch-tab');
await esc();
// multiline text -> editor
await t.cmd('MTEXT'); await t.cmd('10,10'); await t.cmd('60,25'); await t.frame(4);
await shot('42-mtext-editor');
await esc(); await esc();
// context menu
await page.mouse.click(300, 400, { button: 'right' }); await t.frame(4); await shot('43-context');
await esc();
// workspace / classic menu bar
await page.mouse.click(331, 17); await t.frame(4); await shot('44-workspace');
await esc();
// light theme through OPTIONS
await t.cmd('OPTIONS'); await page.mouse.click(597, 352); await t.frame(3); await page.mouse.click(850, 652); await t.frame(6);
await shot('45-light');
// clean screen
await page.keyboard.press('Control+0'); await t.frame(4); await shot('46-clean');
await page.keyboard.press('Control+0'); await t.frame(4);
console.log(JSON.stringify(await t.state()).slice(0, 300));
console.log(t.logs.filter(l => /error|panic/i.test(l)).slice(-8).join('\n'));
await t.close();
