// General-purpose dialogs: confirm/prompt, About, Help, colour, linetype, lineweight, insert, hatch, purge, layer states, QuickCalc.
import { h, svgIcon } from './dom.js';
import { row, num, text, select, check, tabs, swatch, note, group } from './dialogs-kit.js';
import { aciHex, colorToCss, describeColor, LINETYPES, ltypeDisplay, LINEWEIGHTS, lwDisplay } from '../core/style.js';
import { PATTERNS } from '../core/hatch.js';
import { registry } from '../engine/registry.js';

const hexOk = s => /^#[0-9a-f]{6}$/i.test(s);

// ---------------------------------------------------------------- QuickCalc expression evaluator

function evalExpr(src) {
  const t = String(src).toLowerCase().replace(/\s+/g, '').replace(/×/g, '*').replace(/÷/g, '/');
  let i = 0;
  const FN = { sin: x => Math.sin(x * Math.PI / 180), cos: x => Math.cos(x * Math.PI / 180), tan: x => Math.tan(x * Math.PI / 180), asin: x => Math.asin(x) * 180 / Math.PI, acos: x => Math.acos(x) * 180 / Math.PI, atan: x => Math.atan(x) * 180 / Math.PI, sqrt: Math.sqrt, abs: Math.abs, ln: Math.log, log: Math.log10, exp: Math.exp, round: Math.round, floor: Math.floor, ceil: Math.ceil, r2d: x => x * 180 / Math.PI, d2r: x => x * Math.PI / 180 };
  const peek = () => t[i];
  const expr = () => { let v = term(); while (peek() === '+' || peek() === '-') { const op = t[i++]; const r = term(); v = op === '+' ? v + r : v - r; } return v; };
  const term = () => { let v = pow(); while (peek() === '*' || peek() === '/' || peek() === '%') { const op = t[i++]; const r = pow(); v = op === '*' ? v * r : op === '/' ? v / r : v % r; } return v; };
  const pow = () => { const b = unary(); if (peek() === '^') { i++; return Math.pow(b, pow()); } return b; };
  const unary = () => { if (peek() === '-') { i++; return -unary(); } if (peek() === '+') { i++; return unary(); } return atom(); };
  const atom = () => {
    if (peek() === '(') { i++; const v = expr(); if (t[i++] !== ')') throw Error('Missing )'); return v; }
    const m = /^(\d+\.?\d*|\.\d+)(e[+-]?\d+)?/.exec(t.slice(i));
    if (m) { i += m[0].length; return Number(m[0]); }
    const w = /^[a-z0-9]+/.exec(t.slice(i));
    if (w) {
      i += w[0].length;
      if (w[0] === 'pi') return Math.PI;
      if (w[0] === 'e') return Math.E;
      if (FN[w[0]] && peek() === '(') { i++; const v = expr(); if (t[i++] !== ')') throw Error('Missing )'); return FN[w[0]](v); }
      throw Error(`Unknown name "${w[0]}"`);
    }
    throw Error('Syntax error');
  };
  const v = expr();
  if (i < t.length) throw Error('Syntax error');
  if (!Number.isFinite(v)) throw Error('Not a finite number');
  return v;
}
export { evalExpr };

export const BASIC = {
  // ------------------------------------------------------------ simple message boxes
  confirm: ({ data }) => ({
    title: data.title || 'CadForge', width: 380, body: h('div.dlg-msg', svgIcon('about', 28), h('p', data.message)), ok: null, cancel: null,
    buttons: (data.buttons || ['OK']).map((label, i, a) => ({ label, action: () => label })), cancelValue: data.buttons?.includes('Cancel') ? 'Cancel' : null, onOpen: win => { const b = win.querySelectorAll('.dlg-btns .btn'); b[0]?.classList.add('primary'); b[0]?.focus(); }
  }),

  prompt: ({ data }) => {
    const input = text(data.value || '', { width: 320, label: data.label });
    return { title: data.title || 'CadForge', width: 420, body: h('div', h('label.fstack', h('span', data.label || ''), input)), ok: data.ok || 'OK', commit: () => (input.value.trim() ? input.value.trim() : false) };
  },

  about: ({ ui }) => ({
    title: 'About CadForge', width: 420, ok: null, cancel: 'Close',
    body: h('div.about', h('div.about-logo', { html: '<svg viewBox="0 0 28 28" width="64" height="64"><path d="M14 2.5 24 8.2v11.6L14 25.5 4 19.8V8.2z" fill="#2b8fe0"/><path d="M14 2.5 24 8.2 14 14 4 8.2z" fill="#5bb8ff"/><path d="M14 14v11.5L4 19.8V8.2z" fill="#1a6fb5"/><path d="M19.4 11.4a6 6 0 1 0 0 5.2" fill="none" stroke="#fff" stroke-width="2.2" stroke-linecap="round"/></svg>' }),
      h('h2', 'CadForge 2.0'), h('p', 'A 2D drafting and 3D solid modelling application with an AutoCAD-style interface and command set.'),
      h('p.fnote', `${registry.commands.size} commands · ${registry.aliases.size} aliases · runs entirely in your browser.`), h('p.fnote', `Theme: ${ui.theme.dark ? 'Dark' : 'Light'}`)),
  }),

  help: ({ ui }) => {
    const q = text('', { placeholder: 'Search commands', width: 280, label: 'Search commands' });
    const list = h('div.help-list');
    const aliasMap = new Map();
    for (const [a, n] of registry.aliases) aliasMap.set(n, [...(aliasMap.get(n) || []), a]);
    const draw = () => {
      const s = q.value.trim().toLowerCase();
      const defs = [...registry.commands.values()].filter(d => !d.name.startsWith('_') && !d.name.startsWith('OSNAP_') && d.title && (!s || d.name.toLowerCase().includes(s) || d.title.toLowerCase().includes(s) || (d.desc || '').toLowerCase().includes(s) || (aliasMap.get(d.name) || []).some(a => a.toLowerCase() === s)));
      list.replaceChildren(...defs.sort((a, b) => a.name.localeCompare(b.name)).slice(0, 80).map(d => h('div.help-row', { onclick: () => { ui.dialogs.cancelTop(); ui.runCommand(d.name); } }, svgIcon(d.icon || d.name.toLowerCase(), 18), h('b', d.name), h('span.alias', (aliasMap.get(d.name) || []).slice(0, 3).join(', ')), h('span.desc', d.desc || d.title))));
    };
    q.addEventListener('input', draw); draw();
    const keys = [['F1', 'Help'], ['F2', 'Text window'], ['F3', 'Object snap'], ['F7', 'Grid'], ['F8', 'Ortho'], ['F9', 'Snap'], ['F10', 'Polar tracking'], ['F11', 'Object snap tracking'], ['F12', 'Dynamic input'],
      ['Ctrl+N / O / S', 'New / Open / Save'], ['Ctrl+Z / Y', 'Undo / Redo'], ['Ctrl+C / X / V', 'Copy / Cut / Paste'], ['Ctrl+A', 'Select all'], ['Ctrl+1', 'Properties'], ['Ctrl+9', 'Command line'], ['Ctrl+0', 'Clean screen'],
      ['Space / Enter', 'Repeat last command or accept'], ['Esc', 'Cancel'], ['Shift (held)', 'Temporary ortho override'], ['Shift+Right click', 'Object snap menu'], ['Wheel / MMB drag', 'Zoom / pan'], ['Shift+MMB drag', 'Orbit (3D)']];
    return {
      title: 'Help', width: 640, ok: null, cancel: 'Close',
      body: tabs([
        ['Commands', h('div', row('Search', q), list)],
        ['Keyboard', h('table.keys', keys.map(([k, d]) => h('tr', h('td', h('kbd', k)), h('td', d))))],
        ['Command line', h('div.help-text', h('p', 'Type a command name or alias (for example ', h('code', 'L'), ' for LINE or ', h('code', 'TR'), ' for TRIM) and press Enter or Space.'),
          h('p', 'Enter points as ', h('code', '10,20'), ' (absolute), ', h('code', '@10,5'), ' (relative) or ', h('code', '@50<30'), ' (polar). Type a distance with the cursor in the desired direction for direct distance entry.'),
          h('p', 'Prompts show options in square brackets; type the capitalised letters or click an option. Type ', h('code', 'END'), ', ', h('code', 'MID'), ', ', h('code', 'CEN'), ' ... for a one-off object snap, ', h('code', 'FROM'), ' for an offset base point.'),
          h('p', 'Type a system variable such as ', h('code', 'OSMODE'), ' to view or change it.'))],
      ]),
    };
  },

  // ------------------------------------------------------------ colour
  color: ({ data, close }) => {
    let value = data.value;
    const cur = h('div.col-cur', swatch(''), h('span'));
    const show = () => { cur.firstChild.style.background = value === undefined || value === 'BYLAYER' ? 'linear-gradient(135deg,#888 50%,#bbb 50%)' : value === 'BYBLOCK' ? 'repeating-linear-gradient(45deg,#888,#888 3px,#bbb 3px,#bbb 6px)' : colorToCss(value); cur.lastChild.textContent = describeColor(value); };
    const cell = i => h('button.aci', { type: 'button', title: `${i}`, dataset: { aci: i }, style: { background: aciHex(i) }, onclick: () => { value = i; show(); hex.value = aciHex(i); }, ondblclick: () => close(i) });
    const grid = h('div.aci-grid'); for (let hue = 0; hue < 24; hue++) for (let r = 0; r < 10; r++) grid.append(cell(10 + hue * 10 + r));
    const std = h('div.aci-row', [1, 2, 3, 4, 5, 6, 7].map(cell)), grays = h('div.aci-row', [250, 251, 252, 253, 254, 255].map(cell));
    const hex = h('input.fi', { type: 'text', value: hexOk(value) ? value : '#ffffff', maxlength: 7, style: { width: '90px' }, 'aria-label': 'Hex color' });
    const picker = h('input', { type: 'color', value: hexOk(hex.value) ? hex.value : '#ffffff', 'aria-label': 'Pick a color' });
    picker.addEventListener('input', () => { hex.value = picker.value; value = picker.value; show(); });
    hex.addEventListener('input', () => { if (hexOk(hex.value)) { value = hex.value.toLowerCase(); picker.value = value; show(); } });
    const lay = h('div.col-bylayer', data.allowByLayer !== false ? [h('button.btn', { type: 'button', onclick: () => { value = 'BYLAYER'; show(); } }, 'ByLayer'), h('button.btn', { type: 'button', onclick: () => { value = 'BYBLOCK'; show(); } }, 'ByBlock')] : []);
    show();
    return {
      title: 'Select Color', width: 520,
      body: tabs([['Index Color', h('div', note('AutoCAD Color Index (ACI)'), grid, h('div.aci-rows', std, grays), lay)], ['True Color', h('div', row('Hex', hex, picker), note('Any RGB color. It is written to DXF as the nearest color index plus a true-color value.'), lay.cloneNode(false))]]),
      commit: () => (value === 'BYLAYER' && data.allowByLayer === false ? 7 : value), cancelValue: null,
      onOpen: win => { win.querySelector('.dlg-body').append(h('div.col-foot', h('span', 'Color: '), cur)); },
    };
  },

  // ------------------------------------------------------------ linetype / lineweight
  linetype: ({ ui, data }) => {
    const doc = ui.app.doc, cur = data.value ?? doc?.cur.ltype ?? 'Continuous';
    let sel = cur === undefined || cur === 'BYLAYER' ? 'Continuous' : cur;
    const list = h('div.lt-list');
    const draw = () => list.replaceChildren(...Object.keys(LINETYPES).map(n => {
      const svg = ui.ribbon.ltSample(n);
      return h('div.lt-row' + (n === sel ? '.sel' : ''), { onclick: () => { sel = n; draw(); }, ondblclick: () => ui.dialogs.top().commit() }, h('span.lt-name', ltypeDisplay(n)), h('span.lt-desc', LINETYPES[n].desc || ''), svg);
    }));
    draw();
    return { title: 'Linetype Manager', width: 520, body: h('div', note(data.pick ? 'Choose a linetype.' : 'Choose the current linetype for new objects.'), list), commit: () => { if (!data.pick) ui.setProperty('ltype', sel); return sel; } };
  },

  lweight: ({ ui, data }) => {
    const doc = ui.app.doc;
    let sel = data.value ?? doc?.cur.lw;
    const disp = check('Display lineweight', ui.app.sysvars.prefs.LWDISPLAY);
    const list = h('div.lt-list');
    const opts = [[undefined, 'ByLayer'], [-2, 'ByBlock'], [-3, 'Default'], ...LINEWEIGHTS.map(w => [w, lwDisplay(w)])];
    const draw = () => list.replaceChildren(...opts.map(([v, label]) => h('div.lt-row' + (v === sel ? '.sel' : ''), { onclick: () => { sel = v; draw(); }, ondblclick: () => ui.dialogs.top().commit() }, h('span.lt-name', label), h('span.lwsample', { style: { height: Math.max(1, (typeof v === 'number' && v > 0 ? v : 25) / 100 * 1.6) + 'px' } }))));
    draw();
    return { title: 'Lineweight Settings', width: 380, body: h('div', list, h('div.frow', disp)), commit: () => { ui.app.sysvars.set('LWDISPLAY', disp.input.checked ? 1 : 0); if (!data.pick) ui.setProperty('lw', sel); ui.statusbar.refresh(); ui.viewport.invalidate(); return sel === undefined ? 'BYLAYER' : sel; } };
  },

  // ------------------------------------------------------------ blocks
  insert: ({ data }) => {
    const names = data.names || [];
    let sel = names[0];
    const list = h('div.lt-list');
    const draw = () => list.replaceChildren(...names.map(n => h('div.lt-row' + (n === sel ? '.sel' : ''), { onclick: () => { sel = n; draw(); } }, h('span.lt-name', n))));
    draw();
    return { title: 'Insert', width: 360, body: names.length ? h('div', note('Choose a block definition to insert.'), list) : note('No blocks are defined in this drawing. Use BLOCK to create one.'), ok: names.length ? 'OK' : null, commit: () => sel, cancel: names.length ? 'Cancel' : 'Close' };
  },

  // ------------------------------------------------------------ hatch settings (mutates the live settings object)
  hatch: ({ ui, data }) => {
    const pat = select(Object.keys(PATTERNS), data.pattern), scale = num(data.scale, { min: 0.0001, width: 90 }), angle = num(data.angle, { width: 90 });
    let color = data.color;
    const cbtn = h('button.btn', { type: 'button', onclick: async () => { const v = await ui.dialog('color', { value: color, allowByLayer: true }); if (v !== null && v !== undefined) { color = v === 'BYLAYER' ? undefined : v; paint(); } } });
    const paint = () => cbtn.replaceChildren(swatch(color === undefined ? 'linear-gradient(135deg,#888 50%,#bbb 50%)' : colorToCss(color)), ' ' + describeColor(color));
    paint();
    return {
      title: 'Hatch Settings', width: 380,
      body: h('div', row('Pattern', pat), row('Scale', scale), row('Angle', angle), row('Color', cbtn), note(PATTERNS[data.pattern]?.desc || '')),
      commit: () => { const s = Number(scale.value); if (!(s > 0)) return false; data.pattern = pat.value; data.scale = s; data.angle = Number(angle.value) || 0; data.color = color; return true; },
    };
  },

  // ------------------------------------------------------------ purge
  purge: ({ ui, app, doc }) => {
    const used = new Set();
    const walk = list => { for (const e of list) if (e.type === 'insert' && !used.has(e.name)) { used.add(e.name); walk(doc.blocks[e.name]?.entities || []); } };
    walk([...doc.model.entities, ...doc.layouts.flatMap(l => l.entities)]);
    const blocks = Object.keys(doc.blocks).filter(n => !used.has(n));
    const layers = doc.layers.filter(l => l.name !== '0' && l.name !== doc.cur.layer && !doc.layerUsage(l.name));
    const mk = (title, names) => group(`${title} (${names.length})`, names.length ? h('div.purge-list', names.map(n => h('div', n))) : note('Nothing to purge.'));
    return {
      title: 'Purge', width: 380, body: h('div', note('Unreferenced named objects that can be removed:'), mk('Blocks', blocks), mk('Layers', layers.map(l => l.name))), ok: blocks.length || layers.length ? 'Purge All' : null, cancel: 'Close',
      commit: () => { ui.runCommand('PURGE'); void app; return true; },
    };
  },

  // ------------------------------------------------------------ layer states
  layerstate: ({ ui, doc }) => {
    const states = (doc.settings.layerStates ??= {});
    let sel = Object.keys(states)[0] || null;
    const nameIn = text('', { placeholder: 'New layer state name', width: 200, label: 'Layer state name' });
    const list = h('div.lt-list');
    const draw = () => list.replaceChildren(...Object.keys(states).map(n => h('div.lt-row' + (n === sel ? '.sel' : ''), { onclick: () => { sel = n; draw(); } }, h('span.lt-name', n), h('span.lt-desc', `${states[n].length} layers`))));
    draw();
    const save = () => {
      const n = nameIn.value.trim(); if (!n) return false;
      ui.edit('Layer state', d => { d.willChange(); d.settings.layerStates ??= {}; d.settings.layerStates[n] = d.layers.map(l => structuredClone(l)); d.touch(); });
      sel = n; nameIn.value = ''; draw(); return false;
    };
    const restore = () => {
      if (!sel) return false;
      ui.edit('Restore layer state', d => { d.willChange(); const st = d.settings.layerStates[sel]; for (const s of st) { const l = d.layer(s.name); if (l) Object.assign(l, structuredClone(s)); } d.touch(); });
      ui.viewport.invalidate(); return true;
    };
    const del = () => { if (!sel) return false; ui.edit('Delete layer state', d => { d.willChange(); delete d.settings.layerStates[sel]; d.touch(); }); sel = Object.keys(doc.settings.layerStates)[0] || null; draw(); return false; };
    return { title: 'Layer States Manager', width: 420, body: h('div', list, h('div.frow', nameIn, h('button.btn', { type: 'button', onclick: save }, 'Save'))), ok: 'Restore', commit: restore, buttons: [{ label: 'Delete', action: del, close: false }] };
  },

  // ------------------------------------------------------------ QuickCalc
  calc: ({ ui }) => {
    const out = h('div.calc-out', { role: 'log' }), input = text('', { width: 360, label: 'Expression', placeholder: 'e.g. 2*(3+4)/sqrt(2)' });
    let last = null;
    const run = () => {
      const s = input.value.trim(); if (!s) return;
      try { const v = evalExpr(s); last = v; out.append(h('div.calc-line', h('span.calc-in', s), h('span.calc-eq', '= '), h('b', String(+v.toPrecision(12))))); input.value = ''; }
      catch (e) { out.append(h('div.calc-line.err', `${s}  → ${e.message}`)); }
      out.scrollTop = out.scrollHeight;
    };
    input.addEventListener('keydown', e => { if (e.key === 'Enter') { e.stopPropagation(); e.preventDefault(); run(); } });
    return { title: 'QuickCalc', width: 440, body: h('div', out, h('div.frow', input, h('button.btn', { type: 'button', onclick: run }, '=')), note('Operators + − × ÷ ^ %, functions sin cos tan (degrees), sqrt, abs, ln, log, exp, and the constant pi.')), ok: 'Copy result', commit: () => { if (last !== null) navigator.clipboard?.writeText(String(+last.toPrecision(12))); void ui; return true; }, cancel: 'Close' };
  },
};

