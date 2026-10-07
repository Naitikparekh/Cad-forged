// Exports the web UI's icon set and ribbon layout as JSON for the Rust application (crates/cadforge-app/assets).
//   node tools/gen-app-data.mjs
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { ICONS } from '../src/ui/icons.js';
import { RIBBON, HATCH_TAB, HATCHEDIT_TAB } from '../src/ui/ribbon-data.js';

const out = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../crates/cadforge-app/assets');
fs.mkdirSync(out, { recursive: true });
fs.writeFileSync(path.join(out, 'icons.json'), JSON.stringify(ICONS));
fs.writeFileSync(path.join(out, 'ribbon.json'), JSON.stringify({ tabs: RIBBON, hatch: HATCH_TAB, hatchedit: HATCHEDIT_TAB }));
console.log(`icons: ${Object.keys(ICONS).length}, ribbon tabs: ${RIBBON.length}`);
