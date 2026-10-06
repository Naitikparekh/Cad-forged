// Settings dialogs: Drafting Settings, Options, Units, Dimension Style, Text Style, Aliases and Quick Select.
import { h } from './dom.js';
import { row, num, text, select, check, tabs, note, group } from './dialogs-kit.js';
import { OS_BITS, OS_FULL } from '../engine/parse.js';
import { registry } from '../engine/registry.js';
import { describeColor, lwDisplay } from '../core/style.js';
import { labelOf } from '../core/entities.js';
import { PREF_DEFAULTS } from '../engine/sysvars.js';

const OSNAP_MODES = ['END', 'MID', 'CEN', 'NOD', 'QUA', 'INT', 'INS', 'PER', 'TAN', 'NEA'];
const TABS = { snap: 0, polar: 1, osnap: 2, dyn: 3 };

export function loadUserAliases() {
  try { for (const [a, n] of JSON.parse(localStorage.getItem('cf.aliases') || '[]')) if (registry.commands.has(n)) registry.userAliases.set(a.toUpperCase(), n); } catch { /* ignore */ }
}
export function saveUserAliases() {
  try { localStorage.setItem('cf.aliases', JSON.stringify([...registry.userAliases])); } catch { /* ignore */ }
}

export const SETTINGS = {
  // ------------------------------------------------------------ drafting settings
  dsettings: ({ ui, data }) => {
    const sv = ui.app.sysvars, p = sv.prefs;
    const snapOn = check('Snap On (F9)', p.SNAPMODE), snapX = num(p.SNAPUNIT_X, { min: 0.0001, width: 90 }), snapY = num(p.SNAPUNIT_Y, { min: 0.0001, width: 90 });
    const gridOn = check('Grid On (F7)', p.GRIDMODE), gridX = num(p.GRIDUNIT_X, { min: 0.0001, width: 90 }), gridY = num(p.GRIDUNIT_Y, { min: 0.0001, width: 90 }), major = num(p.GRIDMAJOR, { min: 1, step: 1, width: 90 });
    const polarOn = check('Polar Tracking On (F10)', p.POLARMODE), inc = select([90, 45, 30, 22.5, 18, 15, 10, 5], p.POLARANG, { width: 90 }), otrack = check('Object Snap Tracking On (F11)', p.OTRACKMODE);
    const osOn = check('Object Snap On (F3)', !(p.OSMODE & 16384));
    const modes = Object.fromEntries(OSNAP_MODES.map(k => [k, check(OS_FULL[k], p.OSMODE & OS_BITS[k])]));
    const dynOn = check('Enable Dynamic Input (F12)', p.DYNMODE > 0), dynPtr = check('Enable Pointer Input', (Math.abs(p.DYNMODE) & 1) || !p.DYNMODE), dynDim = check('Enable Dimension Input where possible', (Math.abs(p.DYNMODE) & 2) || !p.DYNMODE);
    const body = tabs([
      ['Snap and Grid', h('div.two', group('Snap spacing', h('div.frow', snapOn), row('Snap X spacing', snapX), row('Snap Y spacing', snapY)), group('Grid', h('div.frow', gridOn), row('Grid X spacing', gridX), row('Grid Y spacing', gridY), row('Major line every', major)))],
      ['Polar Tracking', h('div', group('Polar angle settings', h('div.frow', polarOn), row('Increment angle', inc)), group('Object snap tracking', h('div.frow', otrack)))],
      ['Object Snap', h('div', h('div.frow', osOn), group('Object Snap modes', h('div.osgrid', OSNAP_MODES.map(k => modes[k]))), h('div.frow', h('button.btn', { type: 'button', onclick: () => OSNAP_MODES.forEach(k => { modes[k].input.checked = true; }) }, 'Select All'), h('button.btn', { type: 'button', onclick: () => OSNAP_MODES.forEach(k => { modes[k].input.checked = false; }) }, 'Clear All')))],
      ['Dynamic Input', h('div', dynOn, group('Pointer input', h('div.frow', dynPtr)), group('Dimension input', h('div.frow', dynDim)))],
    ], TABS[data.tab] ?? 0);
    return {
      title: 'Drafting Settings', width: 520, body, ok: 'OK',
      commit: () => {
        const pos = [snapX, snapY, gridX, gridY].every(i => Number(i.value) > 0);
        if (!pos) return false;
        sv.set('SNAPMODE', snapOn.input.checked ? 1 : 0); sv.set('SNAPUNIT_X', Number(snapX.value)); sv.set('SNAPUNIT_Y', Number(snapY.value));
        sv.set('GRIDMODE', gridOn.input.checked ? 1 : 0); sv.set('GRIDUNIT_X', Number(gridX.value)); sv.set('GRIDUNIT_Y', Number(gridY.value)); sv.set('GRIDMAJOR', Math.max(1, Math.round(Number(major.value)) || 5));
        sv.set('POLARANG', Number(inc.value)); sv.set('POLARMODE', polarOn.input.checked ? 1 : 0); sv.set('OTRACKMODE', otrack.input.checked ? 1 : 0);
        if (polarOn.input.checked) sv.set('ORTHOMODE', 0);
        let m = osOn.input.checked ? 0 : 16384; for (const k of OSNAP_MODES) if (modes[k].input.checked) m |= OS_BITS[k];
        sv.set('OSMODE', m);
        const bits = (dynPtr.input.checked ? 1 : 0) | (dynDim.input.checked ? 2 : 0) || 3;
        sv.set('DYNMODE', dynOn.input.checked ? bits : -bits);
        ui.statusbar.refresh(); ui.viewport.invalidate();
        return true;
      },
    };
  },

  // ------------------------------------------------------------ options
  options: ({ ui }) => {
    const sv = ui.app.sysvars, p = sv.prefs;
    const theme = select([{ value: 'dark', label: 'Dark' }, { value: 'light', label: 'Light' }], ui.theme.dark ? 'dark' : 'light', { width: 120 });
    const cursor = num(p.CURSORSIZE, { min: 1, max: 100, step: 1, width: 80 }), lw = check('Display lineweights in model space', p.LWDISPLAY), cube = check('Show ViewCube', p.NAVVCUBE), nav = check('Show navigation bar', p.NAVBAR), vpc = check('Show viewport controls', p.VPCONTROL), ucs = check('Show UCS icon', p.UCSICON), tips = check('Show rich tooltips', p.TOOLTIPS);
    const pick = num(p.PICKBOX, { min: 0, max: 50, step: 1, width: 80 }), grip = num(p.GRIPSIZE, { min: 1, max: 20, step: 1, width: 80 }), grips = check('Enable grips', p.GRIPS), pickfirst = check('Allow selection before the command (noun/verb)', p.PICKFIRST);
    const apt = num(p.APERTURE, { min: 1, max: 50, step: 1, width: 80 }), auto = check('AutoComplete command names', p.AUTOCOMPLETE);
    const sc = [(p.SHORTCUTMENU & 1) > 0, (p.SHORTCUTMENU & 4) > 0, (p.SHORTCUTMENU & 2) > 0].map(v => v);
    const scDef = check('Default mode menu (no objects selected)', sc[0]), scEdit = check('Edit mode menu (objects selected)', sc[2]), scCmd = check('Command mode menu (right-click shows a menu; otherwise it acts as Enter)', sc[1]);
    return {
      title: 'Options', width: 540,
      body: tabs([
        ['Display', h('div', group('Colour scheme', row('Theme', theme)), group('Crosshair', row('Size (% of screen)', cursor)), group('Display elements', h('div.fcol', lw, cube, nav, vpc, ucs, tips)))],
        ['Selection', h('div', group('Pickbox and grips', row('Pickbox size (px)', pick), row('Grip size (px)', grip), h('div.fcol', grips)), group('Selection modes', h('div.fcol', pickfirst)))],
        ['Drafting', h('div', group('AutoSnap', row('Aperture size (px)', apt)), group('Command line', h('div.fcol', auto)))],
        ['User Preferences', h('div', group('Right-click customization', h('div.fcol', scDef, scEdit, scCmd)), h('div.frow', h('button.btn', { type: 'button', onclick: () => ui.runCommand('ALIASEDIT') }, 'Edit Aliases...')))],
      ]),
      buttons: [{ label: 'Reset', close: false, action: () => { for (const k of ['CURSORSIZE', 'PICKBOX', 'GRIPSIZE', 'APERTURE']) sv.set(k, PREF_DEFAULTS[k]); cursor.value = PREF_DEFAULTS.CURSORSIZE; pick.value = PREF_DEFAULTS.PICKBOX; grip.value = PREF_DEFAULTS.GRIPSIZE; apt.value = PREF_DEFAULTS.APERTURE; return false; } }],
      commit: () => {
        const n = (el, lo, hi) => Math.max(lo, Math.min(hi, Math.round(Number(el.value)) || lo));
        ui.setTheme(theme.value);
        sv.set('CURSORSIZE', n(cursor, 1, 100)); sv.set('LWDISPLAY', lw.input.checked ? 1 : 0); sv.set('NAVVCUBE', cube.input.checked ? 1 : 0); sv.set('NAVBAR', nav.input.checked ? 1 : 0); sv.set('VPCONTROL', vpc.input.checked ? 1 : 0); sv.set('UCSICON', ucs.input.checked ? 1 : 0); sv.set('TOOLTIPS', tips.input.checked ? 1 : 0);
        sv.set('PICKBOX', n(pick, 0, 50)); sv.set('GRIPSIZE', n(grip, 1, 20)); sv.set('GRIPS', grips.input.checked ? 2 : 0); sv.set('PICKFIRST', pickfirst.input.checked ? 1 : 0);
        sv.set('APERTURE', n(apt, 1, 50)); sv.set('AUTOCOMPLETE', auto.input.checked ? 1 : 0);
        sv.set('SHORTCUTMENU', (scDef.input.checked ? 1 : 0) | (scEdit.input.checked ? 2 : 0) | (scCmd.input.checked ? 4 : 0) | 8);
        ui.viewport.buildControls(); ui.viewport.invalidate(); ui.statusbar.refresh();
        return true;
      },
    };
  },

  // ------------------------------------------------------------ units
  units: ({ ui, doc }) => {
    const u = doc.settings.units;
    const lp = select([0, 1, 2, 3, 4, 5, 6, 7, 8].map(v => ({ value: v, label: v ? '0.' + '0'.repeat(v) : '0' })), u.precision, { width: 120 }), ap = select([0, 1, 2, 3, 4].map(v => ({ value: v, label: v ? '0.' + '0'.repeat(v) : '0' })), u.angPrecision, { width: 120 });
    const ins = select([[0, 'Unitless'], [1, 'Inches'], [2, 'Feet'], [4, 'Millimeters'], [5, 'Centimeters'], [6, 'Meters']].map(([value, label]) => ({ value, label })), u.insUnits, { width: 140 });
    return {
      title: 'Drawing Units', width: 420,
      body: h('div.two', group('Length', row('Type', select(['Decimal'], 'Decimal', { width: 120 })), row('Precision', lp)), group('Angle', row('Type', select(['Decimal Degrees'], 'Decimal Degrees', { width: 120 })), row('Precision', ap), note('Angles measure counter-clockwise from the +X axis.')), group('Insertion scale', row('Units to scale inserted content', ins))),
      commit: () => { ui.edit('Units', d => { d.willChange(); d.settings.units.precision = Number(lp.value); d.settings.units.angPrecision = Number(ap.value); d.settings.units.insUnits = Number(ins.value); d.touch(); }); ui.statusbar.refresh(); return true; },
    };
  },

  // ------------------------------------------------------------ dimension style
  dimstyle: ({ ui, doc }) => {
    const styles = doc.settings.dimStyles;
    let sel = doc.settings.curDimStyle;
    const list = h('div.lt-list.short'), preview = h('div.ds-prev');
    const draw = () => {
      list.replaceChildren(...Object.keys(styles).map(n => h('div.lt-row' + (n === sel ? '.sel' : ''), { onclick: () => { sel = n; draw(); }, ondblclick: () => modify() }, h('span.lt-name', n), n === doc.settings.curDimStyle ? h('span.lt-desc', '(current)') : null)));
      const s = styles[sel];
      preview.replaceChildren(h('b', sel), h('div', `Text height ${s.txt} · Arrow size ${s.asz} · ${s.dec} decimals`), h('div', `Extension offset ${s.exo} · beyond ${s.exe} · Scale ${s.scale}`));
    };
    const modify = async () => {
      const s = styles[sel], f = (k, o = {}) => num(s[k], { width: 90, min: 0, ...o });
      const txt = f('txt', { min: 0.0001 }), gap = f('gap'), asz = f('asz'), exo = f('exo'), exe = f('exe'), cen = f('cen'), scale = f('scale', { min: 0.0001 }), dec = f('dec', { step: 1, max: 8 }), lfac = f('lfac', { min: 0.0001 });
      const pre = text(s.pre, { width: 90 }), suf = text(s.suf, { width: 90 }), tad = select([{ value: 1, label: 'Above dimension line' }, { value: 0, label: 'Centered' }], s.tad ? 1 : 0, { width: 180 });
      const blk = select([['closed', 'Closed filled'], ['open', 'Open'], ['tick', 'Architectural tick'], ['dot', 'Dot'], ['none', 'None']].map(([value, label]) => ({ value, label })), s.blk, { width: 180 }), trail = check('Suppress trailing zeros', s.zin & 8);
      const r = await ui.dialog('dimstyle-edit', { title: `Modify Dimension Style: ${sel}`, pages: [
        ['Lines', h('div', row('Extend beyond dim lines', exe), row('Offset from origin', exo))],
        ['Symbols and Arrows', h('div', row('Arrowhead', blk), row('Arrow size', asz), row('Center mark size', cen))],
        ['Text', h('div', row('Text height', txt), row('Vertical placement', tad), row('Offset from dim line', gap))],
        ['Fit', h('div', row('Overall scale', scale))],
        ['Primary Units', h('div', row('Precision (decimal places)', dec), row('Linear scale factor', lfac), row('Prefix', pre), row('Suffix', suf), h('div.frow', trail))],
      ] });
      if (!r) return;
      const ok = [txt, asz, scale, lfac].every(i => Number(i.value) > 0);
      if (!ok) { ui.toast('Text height, arrow size, scale and scale factor must be positive.'); return; }
      ui.edit('Dimension style', d => {
        d.willChange(); Object.assign(d.settings.dimStyles[sel], { txt: +txt.value, gap: +gap.value || 0, asz: +asz.value, exo: +exo.value || 0, exe: +exe.value || 0, cen: +cen.value || 0, scale: +scale.value, dec: Math.max(0, Math.min(8, Math.round(+dec.value) || 0)), lfac: +lfac.value, pre: pre.value, suf: suf.value, tad: +tad.value, blk: blk.value, zin: trail.input.checked ? (s.zin | 8) : (s.zin & ~8) });
        d.touch();
      });
      draw();
    };
    draw();
    return {
      title: 'Dimension Style Manager', width: 480, ok: 'Close', cancel: null,
      body: h('div.two', h('div', note('Styles'), list), h('div', note('Preview'), preview, h('div.btncol',
        h('button.btn', { type: 'button', onclick: () => { ui.edit('Dimension style', d => { d.willChange(); d.settings.curDimStyle = sel; d.touch(); }); draw(); } }, 'Set Current'),
        h('button.btn', { type: 'button', onclick: async () => { const n = await ui.dialog('prompt', { title: 'New Dimension Style', label: 'New style name', ok: 'Create' }); if (!n || styles[n]) return; ui.edit('Dimension style', d => { d.willChange(); d.settings.dimStyles[n] = { ...structuredClone(d.settings.dimStyles[sel]), name: n }; d.touch(); }); sel = n; draw(); } }, 'New...'),
        h('button.btn', { type: 'button', onclick: modify }, 'Modify...'),
        h('button.btn', { type: 'button', onclick: () => { if (Object.keys(styles).length < 2 || sel === doc.settings.curDimStyle) { ui.toast('The current style and the last style cannot be deleted.'); return; } ui.edit('Dimension style', d => { d.willChange(); delete d.settings.dimStyles[sel]; d.touch(); }); sel = doc.settings.curDimStyle; draw(); } }, 'Delete')))),
      commit: () => true,
    };
  },

  'dimstyle-edit': ({ data }) => ({ title: data.title, width: 440, body: tabs(data.pages), commit: () => true }),

  // ------------------------------------------------------------ text style
  textstyle: ({ ui, doc }) => {
    const hgt = num(doc.settings.textHeight, { min: 0.0001, width: 100 });
    return {
      title: 'Text Style', width: 380,
      body: h('div', row('Style name', text('Standard', { width: 140 })), row('Font', select(['Sans-serif (system)'], 'Sans-serif (system)', { width: 180 })), row('Default height', hgt), note('Text is drawn with the system sans-serif font. The default height is used by TEXT and MTEXT.')),
      commit: () => { const v = Number(hgt.value); if (!(v > 0)) return false; ui.edit('Text style', d => { d.willChange(); d.settings.textHeight = v; d.touch(); }); return true; },
    };
  },

  // ------------------------------------------------------------ aliases
  aliasedit: ({ ui }) => {
    const list = h('div.alias-list');
    const q = text('', { placeholder: 'Filter', width: 160, label: 'Filter aliases' });
    const all = () => {
      const map = new Map();
      for (const [a, n] of registry.aliases) map.set(a, { alias: a, cmd: n, user: false });
      for (const [a, n] of registry.userAliases) map.set(a, { alias: a, cmd: n, user: true });
      return [...map.values()].sort((x, y) => x.alias.localeCompare(y.alias));
    };
    const draw = () => {
      const s = q.value.trim().toUpperCase();
      list.replaceChildren(h('div.alias-row.head', h('span', 'Alias'), h('span', 'Command'), h('span')),
        ...all().filter(r => !s || r.alias.includes(s) || r.cmd.includes(s)).map(r => h('div.alias-row' + (r.user ? '.user' : ''), h('span.a', r.alias), h('span', r.cmd), r.user ? h('button.btn.sm', { type: 'button', onclick: () => { registry.userAliases.delete(r.alias); saveUserAliases(); draw(); } }, 'Remove') : h('span'))));
    };
    q.addEventListener('input', draw);
    const na = text('', { width: 70, label: 'New alias', placeholder: 'Alias' }), nc = select([...registry.commands.keys()].filter(n => !n.startsWith('_') && !n.startsWith('OSNAP_')).sort(), 'LINE', { width: 160 });
    const add = () => {
      const a = na.value.trim().toUpperCase();
      if (!/^[A-Z0-9_-]{1,12}$/.test(a)) { ui.toast('An alias is 1 to 12 letters or digits.'); return; }
      if (registry.commands.has(a)) { ui.toast(`${a} is already a command name.`); return; }
      registry.userAliases.set(a, nc.value); saveUserAliases(); na.value = ''; draw();
    };
    draw();
    return { title: 'Edit Aliases', width: 480, ok: null, cancel: 'Close', body: h('div', note('Aliases you add override the defaults and are remembered in this browser.'), row('Find', q), list, h('div.frow', na, nc, h('button.btn', { type: 'button', onclick: add }, 'Add'))) };
  },

  // ------------------------------------------------------------ quick select
  qselect: ({ app, doc }) => {
    const ed = app.active;
    const types = [...new Set(doc.entities.map(e => labelOf(e)))].sort();
    const prop = select(['Type', 'Layer', 'Color', 'Linetype', 'Lineweight'], 'Type', { width: 140 }), op = select([{ value: '=', label: '= Equals' }, { value: '!=', label: '<> Not equal to' }], '=', { width: 140 });
    const value = select(types, types[0] || '', { width: 180 });
    const include = select([{ value: 'in', label: 'Include in new selection set' }, { value: 'out', label: 'Exclude from new selection set' }], 'in', { width: 240 }), append = check('Append to current selection set', false);
    const valueOf = (e, k) => k === 'Type' ? labelOf(e) : k === 'Layer' ? e.layer : k === 'Color' ? describeColor(e.color) : k === 'Linetype' ? (e.ltype ?? 'ByLayer') : lwDisplay(e.lw);
    const refill = () => {
      const vals = [...new Set(doc.entities.map(e => valueOf(e, prop.value)))].sort();
      value.replaceChildren(...vals.map(v => h('option', { value: v }, v)));
    };
    prop.addEventListener('change', refill); refill();
    return {
      title: 'Quick Select', width: 420,
      body: h('div', row('Apply to', select(['Entire drawing'], 'Entire drawing', { width: 180 })), row('Property', prop), row('Operator', op), row('Value', value), row('How to apply', include), h('div.frow', append)),
      commit: () => {
        const base = new Set(doc.entities.filter(e => doc.isVisible(e) && !doc.isLocked(e)).map(e => e.id));
        const hit = new Set([...base].filter(id => { const e = doc.entities.find(x => x.id === id); const match = (valueOf(e, prop.value) === value.value) === (op.value === '='); return include.value === 'in' ? match : !match; }));
        if (!append.input.checked) ed.sel.clear();
        for (const id of hit) ed.sel.add(id);
        app.log(`${hit.size} found`); app.emit('change');
        return true;
      },
    };
  },
};

