// node tests/web/shot.mjs out.png [width height]
import { launch } from './harness.mjs';
const [out = 'web-shot.png', w = '1440', h = '900'] = process.argv.slice(2);
const t = await launch({ width: +w, height: +h });
await t.shot(out);
console.log(t.logs.slice(-20).join('\n') || 'no console output');
await t.close();
