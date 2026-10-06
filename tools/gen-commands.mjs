// Generates docs/COMMANDS.md from the command registry: node tools/gen-commands.mjs
import fs from 'node:fs';
import '../src/commands/index.js';
import { registry } from '../src/engine/registry.js';
import { PREF_DEFAULTS } from '../src/engine/sysvars.js';

const aliasOf = new Map();
for (const [a, n] of registry.aliases) aliasOf.set(n, [...(aliasOf.get(n) || []), a]);
const groups = new Map();
for (const d of registry.commands.values()) {
  if (d.name.startsWith('_') || d.name.startsWith('OSNAP_') || !d.title) continue;
  const g = d.group || 'General';
  groups.set(g, [...(groups.get(g) || []), d]);
}
const out = ['# CadForge command reference', '', `Generated from the command registry (${[...groups.values()].flat().length} commands). Type a command or alias at the command line, or use the ribbon.`, ''];
for (const [g, list] of [...groups].sort()) {
  out.push(`## ${g}`, '', '| Command | Aliases | Description |', '|---|---|---|');
  for (const d of list.sort((a, b) => a.name.localeCompare(b.name))) out.push(`| \`${d.name}\` | ${(aliasOf.get(d.name) || []).sort().map(a => `\`${a}\``).join(' ')} | ${(d.desc || d.title).replace(/\|/g, '\\|')} |`);
  out.push('');
}
out.push('## System variables', '', 'Type a variable name at the command line to view or change it.', '', '| Variable | Default |', '|---|---|');
for (const [k, v] of Object.entries(PREF_DEFAULTS).sort()) out.push(`| \`${k}\` | \`${v === '' ? '""' : v}\` |`);
fs.writeFileSync(new URL('../docs/COMMANDS.md', import.meta.url), out.join('\n') + '\n');
console.log('wrote docs/COMMANDS.md');
