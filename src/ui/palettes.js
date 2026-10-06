// Dockable / floating palettes: Properties, Layer Properties Manager, Tool Palettes and Quick Properties.
import { h, svgIcon, clear, raf } from './dom.js';
import { showMenu, attachTip } from './menu.js';
import { labelOf, segsOf } from '../core/entities.js';
import { entityArea } from '../commands/inquiry.js';
import * as g from '../core/geom.js';
import { colorToCss, describeColor, LINETYPES, LINEWEIGHTS, lwDisplay, ltypeDisplay, fmtNum } from '../core/style.js';
import { PATTERNS } from '../core/hatch.js';
import { dimGeometry } from '../core/dim.js';
import { registry } from '../engine/registry.js';

const lengthOf = (e, doc) => segsOf(e, doc).reduce((s, q) => s + g.segLength(q), 0);
const JH = ['left', 'center', 'right'];

// ---------------------------------------------------------------- property schema (Geometry section)

const X = (label, get, set) => ({ label, kind: 'num', get, set });
const RO = (label, get) => ({ label, kind: 'ro', get });
const ANG = (label, get, set) => ({ label, kind: 'num', get: e => g.deg(get(e)), set: (e, v) => set(e, g.rad(v)), unit: '°' });

const GEOM = {
  line: [
    X('Start X', e => e.a.x, (e, v) => { e.a.x = v; }), X('Start Y', e => e.a.y, (e, v) => { e.a.y = v; }), X('End X', e => e.b.x, (e, v) => { e.b.x = v; }), X('End Y', e => e.b.y, (e, v) => { e.b.y = v; }),
    RO('Delta X', e => e.b.x - e.a.x), RO('Delta Y', e => e.b.y - e.a.y), RO('Length', e => g.dist(e.a, e.b)), RO('Angle', e => g.deg(g.normAngle(g.angleOf(e.a, e.b))) + '°'),
  ],
  circle: [
    X('Center X', e => e.c.x, (e, v) => { e.c.x = v; }), X('Center Y', e => e.c.y, (e, v) => { e.c.y = v; }), { label: 'Radius', kind: 'num', min: 1e-9, get: e => e.r, set: (e, v) => { e.r = v; } },
    { label: 'Diameter', kind: 'num', min: 1e-9, get: e => e.r * 2, set: (e, v) => { e.r = v / 2; } }, RO('Circumference', e => 2 * Math.PI * e.r), RO('Area', e => Math.PI * e.r * e.r),
  ],
  arc: [
    X('Center X', e => e.c.x, (e, v) => { e.c.x = v; }), X('Center Y', e => e.c.y, (e, v) => { e.c.y = v; }), { label: 'Radius', kind: 'num', min: 1e-9, get: e => e.r, set: (e, v) => { e.r = v; } },
    ANG('Start angle', e => e.a0, (e, v) => { e.a0 = g.normAngle(v); }), ANG('End angle', e => e.a1, (e, v) => { e.a1 = g.normAngle(v); }),
    RO('Total angle', e => g.deg(g.sweepCCW(e.a0, e.a1)) + '°'), RO('Arc length', e => e.r * g.sweepCCW(e.a0, e.a1)),
  ],
  pline: [
    { label: 'Closed', kind: 'bool', get: e => !!e.closed, set: (e, v) => { e.closed = v; } }, RO('Vertices', e => e.v.length), RO('Length', (e, d) => lengthOf(e, d)), RO('Area', e => (e.closed ? Math.abs(g.plineArea(e.v)) : 0)),
  ],
  ellipse: [
    X('Center X', e => e.c.x, (e, v) => { e.c.x = v; }), X('Center Y', e => e.c.y, (e, v) => { e.c.y = v; }), RO('Major radius', e => g.len(e.m)), RO('Minor radius', e => g.len(e.m) * e.ratio), { label: 'Radius ratio', kind: 'num', min: 1e-6, max: 1, get: e => e.ratio, set: (e, v) => { e.ratio = v; } },
  ],
  spline: [{ label: 'Closed', kind: 'bool', get: e => !!e.closed, set: (e, v) => { e.closed = v; } }, RO('Fit points', e => e.fit.length)],
  point: [X('Position X', e => e.p.x, (e, v) => { e.p.x = v; }), X('Position Y', e => e.p.y, (e, v) => { e.p.y = v; })],
  text: [
    { label: 'Contents', kind: 'text', get: e => e.s, set: (e, v) => { e.s = v; } }, { label: 'Height', kind: 'num', min: 1e-9, get: e => e.h, set: (e, v) => { e.h = v; } }, ANG('Rotation', e => e.rot || 0, (e, v) => { e.rot = v; }),
    X('Position X', e => e.p.x, (e, v) => { e.p.x = v; }), X('Position Y', e => e.p.y, (e, v) => { e.p.y = v; }), { label: 'Justify', kind: 'select', options: JH, get: e => e.jh || 'left', set: (e, v) => { e.jh = v; } },
  ],
  dim: [RO('Measurement', (e, d) => { try { return fmtNum(dimMeasured(e, d), 4); } catch { return ''; } }), { label: 'Text override', kind: 'text', get: e => e.txt || '', set: (e, v) => { e.txt = v; } }],
  leader: [{ label: 'Contents', kind: 'text', get: e => e.s, set: (e, v) => { e.s = v; } }, { label: 'Text height', kind: 'num', min: 1e-9, get: e => e.h, set: (e, v) => { e.h = v; } }],
  hatch: [
    { label: 'Pattern', kind: 'select', options: Object.keys(PATTERNS), get: e => e.pattern, set: (e, v) => { e.pattern = v; } }, { label: 'Scale', kind: 'num', min: 1e-6, get: e => e.ps, set: (e, v) => { e.ps = v; } },
    ANG('Angle', e => e.pa || 0, (e, v) => { e.pa = v; }), RO('Area', (e, d) => entityArea(e, d)?.area ?? 0),
  ],
  insert: [RO('Block name', e => e.name), X('Position X', e => e.p.x, (e, v) => { e.p.x = v; }), X('Position Y', e => e.p.y, (e, v) => { e.p.y = v; }), { label: 'Scale X', kind: 'num', get: e => e.sx, set: (e, v) => { if (v) e.sx = v; } }, { label: 'Scale Y', kind: 'num', get: e => e.sy, set: (e, v) => { if (v) e.sy = v; } }, ANG('Rotation', e => e.rot || 0, (e, v) => { e.rot = v; })],
  xline: [X('Base X', e => e.p.x, (e, v) => { e.p.x = v; }), X('Base Y', e => e.p.y, (e, v) => { e.p.y = v; }), RO('Angle', e => g.deg(g.normAngle(Math.atan2(e.d.y, e.d.x))) + '°')],
  ray: [X('Start X', e => e.p.x, (e, v) => { e.p.x = v; }), X('Start Y', e => e.p.y, (e, v) => { e.p.y = v; }), RO('Angle', e => g.deg(g.normAngle(Math.atan2(e.d.y, e.d.x))) + '°')],
};
const dimMeasured = (e, d) => dimGeometry(e, d).measured;

// ---------------------------------------------------------------- the palette manager

export class Palettes {
  constructor(ui, { left, right, root }) {
    this.ui = ui; this.app = ui.app; this.left = left; this.right = right; this.root = root;
    this.map = {};
    this.map.properties = new PropertiesPalette(ui, left);
    this.map.tools = new ToolPalette(ui, left);
    this.map.layers = new LayerManager(ui, root);
    this.map.quick = new QuickProps(ui, ui.els.viewport);
    this.map.command = { toggle: show => { const host = ui.els.cmdline; const on = show ?? host.hidden; host.hidden = !on; ui.layoutChanged(); if (on) ui.cmdline.focus(); }, get visible() { return !ui.els.cmdline.hidden; } };
    this.refresh = raf(() => { for (const p of Object.values(this.map)) p.refresh?.(); });
    this.app.on('change', () => this.refresh());
    this.app.on('active', () => this.refresh());
  }

  show(name, on) { this.toggle(name, on); }

  toggle(name, show) {
    const p = this.map[name]; if (!p) return;
    if (name === 'properties' || name === 'tools') {
      const other = name === 'properties' ? this.map.tools : this.map.properties;
      const want = show ?? !p.visible;
      if (want) other.setVisible(false);
      p.setVisible(want);
      this.left.hidden = !(this.map.properties.visible || this.map.tools.visible);
      this.ui.layoutChanged();
      return;
    }
    p.toggle(show);
  }
}

// ---------------------------------------------------------------- base class for docked palettes

class Docked {
  constructor(ui, host, title, id) {
    this.ui = ui; this.app = ui.app; this.host = host; this.visible = false;
    this.body = h('div.pal-body');
    this.el = h('section.palette', { dataset: { id }, hidden: true },
      h('header.pal-head', h('span.pal-title', title), h('button.pal-x', { title: 'Close', 'aria-label': `Close ${title}`, onclick: () => ui.togglePalette(id, false) }, svgIcon('x', 10))), this.body);
    this.resizer = h('div.pal-resize', { onpointerdown: e => this.resize(e) });
    host.append(this.el);
    if (!host.querySelector('.pal-resize')) host.append(this.resizer);
  }
  setVisible(v) { this.visible = v; this.el.hidden = !v; if (v) this.refresh(); }
  resize(e) {
    e.preventDefault();
    const x0 = e.clientX, w0 = this.host.getBoundingClientRect().width;
    const mv = ev => { this.host.style.width = Math.max(220, Math.min(560, w0 + ev.clientX - x0)) + 'px'; this.ui.layoutChanged(); };
    const up = () => { document.removeEventListener('pointermove', mv); document.removeEventListener('pointerup', up); };
    document.addEventListener('pointermove', mv); document.addEventListener('pointerup', up);
  }
}

// ---------------------------------------------------------------- Properties palette

class PropertiesPalette extends Docked {
  constructor(ui, host) { super(ui, host, 'Properties', 'properties'); this.filter = 'all'; this.key = ''; this.collapsed = new Set(); }

  refresh() {
    if (!this.visible) return;
    const ed = this.app.active; if (!ed) { clear(this.body); return; }
    const sel = ed.selected, key = `${ed.doc.rev}|${[...ed.sel].join(',')}|${this.filter}|${ed.doc.cur.layer}`;
    if (key === this.key && this.body.childElementCount) return;
    if (this.body.contains(document.activeElement) && document.activeElement.matches('input,select') && this.key.split('|')[1] === key.split('|')[1]) return;   // do not rebuild under the user's caret
    this.key = key;
    this.render(ed, sel);
  }

  render(ed, sel) {
    const doc = ed.doc, ui = this.ui;
    clear(this.body);
    const types = [...new Set(sel.map(e => labelOf(e)))];
    if (this.filter !== 'all' && !types.includes(this.filter)) this.filter = 'all';
    const targets = this.filter === 'all' ? sel : sel.filter(e => labelOf(e) === this.filter);
    const head = h('select.pal-sel', { 'aria-label': 'Selected objects', onchange: e => { this.filter = e.target.value; this.key = ''; this.refresh(); } },
      h('option', { value: 'all' }, sel.length ? `All (${sel.length})` : 'No selection'), ...types.map(t => h('option', { value: t }, `${t} (${sel.filter(e => labelOf(e) === t).length})`)));
    head.value = this.filter;
    this.body.append(h('div.pal-top', head, h('button.pal-btn', { title: 'Quick Select', onclick: () => ui.runCommand('QSELECT') }, svgIcon('qselect', 16))));

    const apply = (fn, label = 'Properties') => { ui.edit(label, d => { d.willChange(); for (const e of targets) if (!d.isLocked(e)) fn(e, d); d.touch(); }); ui.viewport.invalidate(); };
    const same = get => { const set = new Set(targets.map(e => JSON.stringify(get(e) ?? null))); return set.size === 1 ? get(targets[0]) : '*VARIES*'; };

    // ---- General
    const general = [];
    if (targets.length) {
      const colorVal = same(e => e.color);
      general.push(this.field('Color', this.colorButton(colorVal, async () => { const v = await ui.dialog('color', { value: colorVal === '*VARIES*' ? undefined : colorVal, allowByLayer: true }); if (v !== null && v !== undefined) apply(e => { if (v === 'BYLAYER') delete e.color; else e.color = v; }); })));
      const layerVal = same(e => e.layer);
      general.push(this.field('Layer', this.selectEl(doc.layers.map(l => l.name), layerVal, v => apply(e => { e.layer = v; }))));
      const ltVal = same(e => e.ltype);
      general.push(this.field('Linetype', this.selectEl(['ByLayer', 'ByBlock', ...Object.keys(LINETYPES).map(ltypeDisplay)], ltVal === '*VARIES*' ? ltVal : ltVal === undefined ? 'ByLayer' : ltVal === 'BYBLOCK' ? 'ByBlock' : ltypeDisplay(ltVal), v => apply(e => { if (v === 'ByLayer') delete e.ltype; else if (v === 'ByBlock') e.ltype = 'BYBLOCK'; else e.ltype = Object.keys(LINETYPES).find(k => ltypeDisplay(k) === v); }))));
      const lwVal = same(e => e.lw);
      general.push(this.field('Lineweight', this.selectEl(['ByLayer', 'ByBlock', 'Default', ...LINEWEIGHTS.map(lwDisplay)], lwVal === '*VARIES*' ? lwVal : lwVal === undefined ? 'ByLayer' : lwDisplay(lwVal), v => apply(e => { if (v === 'ByLayer') delete e.lw; else if (v === 'ByBlock') e.lw = -2; else if (v === 'Default') e.lw = -3; else e.lw = Math.round(parseFloat(v) * 100); }))));
    } else {
      const c = doc.cur;
      general.push(this.field('Color', this.colorButton(c.color, async () => { const v = await ui.dialog('color', { value: c.color, allowByLayer: true }); if (v !== null && v !== undefined) ui.setProperty('color', v === 'BYLAYER' ? undefined : v); })));
      general.push(this.field('Layer', this.selectEl(doc.layers.map(l => l.name), c.layer, v => ui.chooseLayer(v))));
      general.push(this.field('Linetype', h('span.pal-ro', c.ltype === undefined ? 'ByLayer' : ltypeDisplay(c.ltype))), this.field('Lineweight', h('span.pal-ro', lwDisplay(c.lw))), this.field('Linetype scale', h('span.pal-ro', String(doc.settings.ltscale))));
    }
    this.section('General', general);

    // ---- Geometry (single type only)
    const ftype = this.filter === 'all' ? (types.length === 1 ? sel[0].type : null) : targets[0]?.type;
    if (targets.length && ftype && GEOM[ftype]) {
      const rows = GEOM[ftype].map(f => this.geomField(f, targets, apply, doc)).filter(Boolean);
      this.section('Geometry', rows);
    } else if (targets.length && !ftype) this.section('Geometry', [h('div.pal-note', 'Select a single object type to edit its geometry.')]);

    // ---- View / drawing info when nothing is selected
    if (!sel.length) {
      const v = ed.view, p = doc.settings.units.precision;
      this.section('View', [this.field('Center X', h('span.pal-ro', fmtNum(v.target.x, p))), this.field('Center Y', h('span.pal-ro', fmtNum(v.target.y, p))), this.field('Height', h('span.pal-ro', fmtNum(v.h / v.scale, p))), this.field('Width', h('span.pal-ro', fmtNum(v.w / v.scale, p)))]);
      this.section('Misc', [this.field('Drawing', h('span.pal-ro', doc.name)), this.field('Objects', h('span.pal-ro', String(doc.entities.length))), this.field('Layers', h('span.pal-ro', String(doc.layers.length))), this.field('Units', h('span.pal-ro', ['Unitless', 'Inches', 'Feet', '', 'Millimeters', 'Centimeters', 'Meters'][doc.settings.units.insUnits] || 'Unitless'))]);
    }
  }

  section(title, rows) {
    const open = !this.collapsed.has(title);
    const body = h('div.pal-rows', rows);
    const head = h('button.pal-sec' + (open ? '.open' : ''), { onclick: () => { if (this.collapsed.has(title)) this.collapsed.delete(title); else this.collapsed.add(title); head.classList.toggle('open'); body.hidden = !body.hidden; } }, svgIcon('chevdown', 9), h('span', title));
    body.hidden = !open;
    this.body.append(h('div.pal-section', head, body));
  }

  field(label, control) { return h('div.pal-row', h('span.pal-label', label), h('span.pal-ctl', control)); }

  colorButton(v, onclick) {
    const css = v === '*VARIES*' ? '#888' : v === undefined ? 'linear-gradient(135deg,#888 50%,#bbb 50%)' : v === 'BYBLOCK' ? 'repeating-linear-gradient(45deg,#888,#888 3px,#bbb 3px,#bbb 6px)' : colorToCss(v);
    return h('button.pal-color', { type: 'button', onclick }, h('span.cswatch', { style: { background: css } }), h('span', v === '*VARIES*' ? '*VARIES*' : describeColor(v)));
  }

  selectEl(options, value, onchange) {
    const el = h('select.pal-in', { onchange: e => onchange(e.target.value) }, options.map(o => h('option', { value: o }, o)));
    if (value === '*VARIES*') { el.prepend(h('option', { value: '*VARIES*' }, '*VARIES*')); el.value = '*VARIES*'; } else el.value = value;
    return el;
  }

  geomField(f, targets, apply, doc) {
    const vals = targets.map(e => f.get(e, doc));
    const same = vals.every(v => v === vals[0] || (typeof v === 'number' && Math.abs(v - vals[0]) < 1e-9));
    const shown = same ? vals[0] : '*VARIES*';
    const prec = doc.settings.units.precision;
    if (f.kind === 'ro') return this.field(f.label, h('span.pal-ro', typeof shown === 'number' ? fmtNum(shown, prec) : String(shown)));
    if (f.kind === 'bool') return this.field(f.label, this.selectEl(['Yes', 'No'], shown === '*VARIES*' ? shown : shown ? 'Yes' : 'No', v => apply(e => f.set(e, v === 'Yes', doc))));
    if (f.kind === 'select') return this.field(f.label, this.selectEl(f.options, shown, v => apply(e => f.set(e, v, doc))));
    if (f.kind === 'text') {
      const inp = h('input.pal-in', { type: 'text', value: shown === '*VARIES*' ? '' : String(shown).replace(/\n/g, ' ⏎ '), placeholder: shown === '*VARIES*' ? '*VARIES*' : '', onchange: e => apply(en => f.set(en, e.target.value.replace(/ ⏎ /g, '\n'), doc)) });
      inp.addEventListener('keydown', e => { e.stopPropagation(); if (e.key === 'Enter') inp.blur(); });
      return this.field(f.label, inp);
    }
    const inp = h('input.pal-in', { type: 'text', inputmode: 'decimal', value: shown === '*VARIES*' ? '' : fmtNum(shown, prec, true), placeholder: shown === '*VARIES*' ? '*VARIES*' : '' });
    const commit = () => {
      const v = Number(inp.value);
      if (!Number.isFinite(v) || inp.value.trim() === '' || (f.min !== undefined && v < f.min) || (f.max !== undefined && v > f.max)) { this.key = ''; this.refresh(); return; }
      apply(e => f.set(e, v, doc));
    };
    inp.addEventListener('change', commit);
    inp.addEventListener('keydown', e => { e.stopPropagation(); if (e.key === 'Enter') inp.blur(); if (e.key === 'Escape') { this.key = ''; this.refresh(); } });
    return this.field(f.label + (f.unit ? ` (${f.unit})` : ''), inp);
  }
}

// ---------------------------------------------------------------- Tool palettes (draw / modify / annotate command buttons)

const TOOL_SETS = [
  ['Draw', ['LINE', 'PLINE', 'CIRCLE', 'ARC', 'RECTANG', 'POLYGON', 'ELLIPSE', 'SPLINE', 'POINT', 'XLINE', 'RAY', 'DONUT', 'HATCH', 'BOUNDARY']],
  ['Modify', ['MOVE', 'COPY', 'ROTATE', 'SCALE', 'MIRROR', 'OFFSET', 'TRIM', 'EXTEND', 'FILLET', 'CHAMFER', 'STRETCH', 'EXPLODE', 'ERASE', 'JOIN', 'BREAK', 'LENGTHEN', 'PEDIT', 'ARRAYRECT', 'ARRAYPOLAR']],
  ['Annotation', ['MTEXT', 'TEXT', 'DIMLINEAR', 'DIMALIGNED', 'DIMANGULAR', 'DIMRADIUS', 'DIMDIAMETER', 'DIMBASELINE', 'DIMCONTINUE', 'MLEADER']],
  ['3D', ['BOX', 'CYLINDER', 'SPHERE', 'CONE', 'WEDGE', 'EXTRUDE', 'PRESSPULL', 'UNION', 'SUBTRACT', 'INTERSECT']],
];

class ToolPalette extends Docked {
  constructor(ui, host) { super(ui, host, 'Tool Palettes', 'tools'); this.tab = 0; }
  refresh() {
    if (!this.visible) return;
    clear(this.body);
    const tabs = h('div.tp-tabs', TOOL_SETS.map(([t], i) => h('button.tp-tab' + (i === this.tab ? '.active' : ''), { onclick: () => { this.tab = i; this.refresh(); } }, t)));
    const grid = h('div.tp-grid');
    for (const name of TOOL_SETS[this.tab][1]) {
      const def = registry.commands.get(name); if (!def) continue;
      const b = h('button.tp-tool', { onclick: () => this.ui.runCommand(name) }, svgIcon(def.icon || name.toLowerCase(), 24), h('span', def.title || name));
      attachTip(b, { title: def.title, desc: def.desc, cmd: def.name });
      grid.append(b);
    }
    this.body.append(tabs, grid);
  }
}

// ---------------------------------------------------------------- Layer Properties Manager (floating)

class LayerManager {
  constructor(ui, root) {
    this.ui = ui; this.app = ui.app; this.root = root; this.visible = false;
    this.sel = new Set(); this.filter = 'all'; this.search = ''; this.editing = null;
    this.table = h('div.lm-table', { role: 'grid' });
    this.status = h('div.lm-status');
    this.tree = h('div.lm-tree');
    const q = h('input.lm-search', { type: 'search', placeholder: 'Search for layer', 'aria-label': 'Search for layer', oninput: e => { this.search = e.target.value.toLowerCase(); this.refresh(); } });
    this.tool = (icon, title, fn) => { const b = h('button.lm-tool', { title, 'aria-label': title, onclick: fn }, svgIcon(icon, 18)); return b; };
    this.el = h('section.layerman', { hidden: true, role: 'dialog', 'aria-label': 'Layer Properties Manager' },
      h('header.pal-head', { onpointerdown: e => this.drag(e) }, h('span.pal-title', 'Layer Properties Manager'), h('button.pal-x', { title: 'Close', 'aria-label': 'Close Layer Properties Manager', onclick: () => this.toggle(false) }, svgIcon('x', 10))),
      h('div.lm-bar', this.tool('layers', 'New Layer (Alt+N)', () => this.newLayer()), this.tool('trash', 'Delete Layer (Alt+D)', () => this.deleteSel()), this.tool('laycur', 'Set Current (Alt+C)', () => this.setCurrent()), h('div.lm-fill'), q),
      h('div.lm-main', this.tree, this.table), this.status, h('div.lm-grip'));
    root.append(this.el);
    this.el.addEventListener('keydown', e => { e.stopPropagation(); if (e.key === 'Delete' && !this.editing) this.deleteSel(); if (e.altKey && e.key.toLowerCase() === 'n') this.newLayer(); });
  }

  toggle(show = !this.visible) { this.visible = show; this.el.hidden = !show; if (show) { this.refresh(); this.el.querySelector('.lm-search')?.focus(); } }
  drag(e) {
    if (e.target.closest('button')) return;
    const r = this.el.getBoundingClientRect(), dx = e.clientX - r.left, dy = e.clientY - r.top;
    const mv = ev => { this.el.style.left = Math.max(0, ev.clientX - dx) + 'px'; this.el.style.top = Math.max(0, ev.clientY - dy) + 'px'; this.el.style.right = 'auto'; };
    const up = () => { document.removeEventListener('pointermove', mv); document.removeEventListener('pointerup', up); };
    document.addEventListener('pointermove', mv); document.addEventListener('pointerup', up);
  }

  get doc() { return this.app.doc; }

  newLayer() {
    const doc = this.doc; let i = 1; while (doc.layer(`Layer${i}`)) i++;
    const base = [...this.sel][0] && doc.layer([...this.sel][0]);
    this.ui.edit('New layer', d => { d.addLayer(`Layer${i}`, base ? { color: base.color, ltype: base.ltype, lw: base.lw } : {}); });
    this.sel = new Set([`Layer${i}`]); this.refresh(); this.startRename(`Layer${i}`);
  }

  deleteSel() {
    const names = [...this.sel]; let n = 0;
    for (const nm of names) { try { this.ui.edit('Delete layer', d => { d.deleteLayer(nm); }); n++; } catch (e) { this.ui.toast(e.message); } }
    this.sel.clear(); this.refresh();
    void n;
  }

  setCurrent() { const n = [...this.sel][0]; if (!n) return; try { this.ui.chooseLayer(n); } catch (e) { this.ui.toast(e.message); } this.refresh(); }

  startRename(name) {
    const row = [...this.table.querySelectorAll('.lm-row')].find(r => r.dataset.name === name); if (!row) return;
    const cell = row.querySelector('.c-name'), inp = h('input.lm-edit', { type: 'text', value: name });
    this.editing = name; cell.replaceChildren(inp); inp.focus(); inp.select();
    const done = ok => {
      if (this.editing !== name) return; this.editing = null;
      const v = inp.value.trim();
      if (ok && v && v !== name) { try { this.ui.edit('Rename layer', d => { d.renameLayer(name, v); }); this.sel = new Set([v]); } catch (e) { this.ui.toast(e.message); } }
      this.refresh();
    };
    inp.addEventListener('keydown', e => { e.stopPropagation(); if (e.key === 'Enter') done(true); if (e.key === 'Escape') done(false); });
    inp.addEventListener('blur', () => done(true));
  }

  refresh() {
    if (!this.visible || !this.doc || this.editing) return;
    const doc = this.doc, ui = this.ui;
    clear(this.tree);
    for (const [id, label] of [['all', 'All'], ['used', 'All Used Layers']]) this.tree.append(h('button.lm-node' + (this.filter === id ? '.sel' : ''), { onclick: () => { this.filter = id; this.refresh(); } }, svgIcon('layers', 14), label));
    const list = [...doc.layers].filter(l => (this.filter === 'all' || doc.layerUsage(l.name) || l.name === doc.cur.layer) && l.name.toLowerCase().includes(this.search)).sort((a, b) => (a.name === '0' ? -1 : b.name === '0' ? 1 : a.name.localeCompare(b.name, undefined, { numeric: true })));
    clear(this.table);
    const cols = ['S', 'Name', 'On', 'Freeze', 'Lock', 'Color', 'Linetype', 'Lineweight', 'Plot', 'Description'];
    this.table.append(h('div.lm-row.head', cols.map((c, i) => h('div.c' + ['-s', '-name', '-on', '-fr', '-lk', '-col', '-lt', '-lw', '-pl', '-desc'][i], c))));
    const toggle = (l, key, icons) => h('button.lm-ico', { title: key, onclick: e => { e.stopPropagation(); ui.layerToggle(l, key); this.refresh(); } }, svgIcon(icons[l[key] ? 1 : 0], 15));
    for (const l of list) {
      const row = h('div.lm-row' + (this.sel.has(l.name) ? '.sel' : ''), { dataset: { name: l.name }, role: 'row', onclick: e => { if (e.ctrlKey || e.metaKey) { if (this.sel.has(l.name)) this.sel.delete(l.name); else this.sel.add(l.name); } else if (e.shiftKey && this.sel.size) { const names = list.map(x => x.name), a = names.indexOf([...this.sel].pop()), b = names.indexOf(l.name); this.sel = new Set(names.slice(Math.min(a, b), Math.max(a, b) + 1)); } else this.sel = new Set([l.name]); this.refresh(); }, ondblclick: e => { if (e.target.closest('.c-name') && l.name !== '0') this.startRename(l.name); else if (!e.target.closest('button')) { ui.chooseLayer(l.name); this.refresh(); } } },
        h('div.c-s', l.name === doc.cur.layer ? svgIcon('check', 13) : doc.layerUsage(l.name) ? h('span.lm-used') : null),
        h('div.c-name', l.name),
        h('div.c-on', toggle(l, 'on', ['eyeoff', 'sun'])), h('div.c-fr', toggle(l, 'frozen', ['sun', 'snow'])), h('div.c-lk', toggle(l, 'locked', ['unlock', 'lock'])),
        h('div.c-col', h('button.lm-btn', { onclick: async e => { e.stopPropagation(); const v = await ui.dialog('color', { value: l.color, allowByLayer: false }); if (v !== null && v !== undefined) { ui.edit('Layer color', d => { d.willChange(); for (const nm of this.sel.has(l.name) ? this.sel : [l.name]) { const x = d.layer(nm); if (x) x.color = v; } d.touch(); }); this.refresh(); } } }, h('span.cswatch', { style: { background: colorToCss(l.color) } }), describeColor(l.color))),
        h('div.c-lt', h('button.lm-btn', { onclick: async e => { e.stopPropagation(); const v = await ui.dialog('linetype', { value: l.ltype, pick: true }); if (v) { ui.edit('Layer linetype', d => { d.willChange(); for (const nm of this.sel.has(l.name) ? this.sel : [l.name]) { const x = d.layer(nm); if (x) x.ltype = v; } d.touch(); }); this.refresh(); } } }, ltypeDisplay(l.ltype || 'Continuous'))),
        h('div.c-lw', h('button.lm-btn', { onclick: async e => { e.stopPropagation(); const v = await ui.dialog('lweight', { value: l.lw, pick: true }); if (v !== null && v !== undefined && v !== 'BYLAYER') { ui.edit('Layer lineweight', d => { d.willChange(); for (const nm of this.sel.has(l.name) ? this.sel : [l.name]) { const x = d.layer(nm); if (x) x.lw = v; } d.touch(); }); this.refresh(); } } }, lwDisplay(l.lw))),
        h('div.c-pl', h('button.lm-ico', { title: 'Plot', onclick: e => { e.stopPropagation(); ui.edit('Layer plot', d => { d.willChange(); l.plot = !l.plot; d.touch(); }); this.refresh(); } }, svgIcon('printer', 15), l.plot === false ? h('span.lm-x') : null)),
        h('div.c-desc', l.desc || ''));
      this.table.append(row);
    }
    this.status.textContent = `${this.filter === 'all' ? 'All' : 'Used'}: ${list.length} layers displayed of ${doc.layers.length} total layers`;
  }
}

// ---------------------------------------------------------------- Quick Properties (floating panel over the viewport)

class QuickProps {
  constructor(ui, vp) {
    this.ui = ui; this.app = ui.app; this.visible = false;
    this.el = h('div.quickprops', { hidden: true }); vp.append(this.el);
  }
  toggle(show = !this.visible) { this.visible = show; this.app.sysvars.set('QPMODE', show ? 1 : -1); this.refresh(); this.ui.statusbar?.refresh(); }
  refresh() {
    const ed = this.app.active;
    if (!this.visible || !ed || !ed.sel.size) { this.el.hidden = true; return; }
    const sel = ed.selected, e = sel[0], doc = ed.doc;
    this.el.hidden = false; clear(this.el);
    const types = [...new Set(sel.map(labelOf))];
    this.el.append(h('div.qp-head', types.length === 1 ? `${types[0]}${sel.length > 1 ? ` (${sel.length})` : ''}` : `All (${sel.length})`, h('button.pal-x', { 'aria-label': 'Close quick properties', onclick: () => this.toggle(false) }, svgIcon('x', 9))));
    const rows = [['Layer', e.layer], ['Color', describeColor(e.color)], ['Linetype', e.ltype === undefined ? 'ByLayer' : ltypeDisplay(e.ltype)]];
    if (sel.length === 1) for (const f of (GEOM[e.type] || []).slice(0, 6)) { const v = f.get(e, doc); rows.push([f.label, typeof v === 'number' ? fmtNum(v, doc.settings.units.precision, true) : String(v)]); }
    for (const [k, v] of rows) this.el.append(h('div.qp-row', h('span', k), h('b', v)));
  }
}

void showMenu;
