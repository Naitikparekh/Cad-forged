// Command runtime: runs commands as async functions that await prompts, and routes pointer / keyboard
// input (clicks, typed text, Enter, Escape) to whatever prompt is pending. DOM-free: tests drive it directly.
import * as g from '../core/geom.js';
import { Snapper, activeMask } from './snap.js';
import { parseInput, parseOverride, matchOption, optionName, optionsText, OS_BITS, OS_FULL } from './parse.js';
import { pickAt, selectRect, selectPolygon, selectFence, rectOf } from './select.js';
import { gripsOf, boundsOf } from '../core/entities.js';

export class Cancel extends Error { constructor() { super('*Cancel*'); this.cancel = true; } }
export const flush = () => new Promise(r => setTimeout(r, 0));

const fmt = (n, p = 4) => { const s = (Math.round(n * 10 ** p) / 10 ** p).toString(); return s; };
export const fmtPt = (p, prec = 4) => `${fmt(p.x, prec)},${fmt(p.y, prec)}`;

// ---------------------------------------------------------------- command context

class Ctx {
  constructor(rt, def, editor, args) {
    this.rt = rt; this.def = def; this.name = def.name; this.editor = editor; this.args = args || {};
    this.pending = null; this.previewFn = null;
  }
  get app() { return this.rt.app; }
  get doc() { return this.editor.doc; }
  get view() { return this.editor.view; }
  get sv() { return this.rt.app.sysvars; }
  var(n) { return this.sv.get(n); }
  setvar(n, v) { this.sv.set(n, v); }
  log(text, kind = 'info') { this.app.log(text, kind); }

  ask(spec) {
    return new Promise((resolve, reject) => {
      spec.resolve = resolve; spec.reject = reject; spec.ctx = this; spec.options = spec.options || [];
      this.pending = spec;
      this.rt._promptShown(this, spec);
    });
  }

  /** Specify a point. Result: {pt} | {kw} | {enter:true}. */
  point(spec) { return this.ask({ kind: 'point', ...spec }); }
  /** Second corner of a rectangle (shows the rubber-band rectangle from `base`). */
  corner(spec) { return this.ask({ kind: 'point', corner: true, ...spec, preview: spec.preview || (pt => [{ __rect: true, a: spec.base, b: pt }]) }); }
  /** Typed number. Result: {value} | {kw} | {enter:true,value:default}. */
  number(spec) { return this.ask({ kind: 'number', ...spec }); }
  /** Distance typed or measured by picking two points (or one with a base point). */
  distance(spec) { return this.ask({ kind: 'distance', ...spec }); }
  /** Angle in degrees typed (returned in radians) or measured by picking points. */
  angle(spec) { return this.ask({ kind: 'angle', ...spec }); }
  string(spec) { return this.ask({ kind: 'string', ...spec }); }
  keyword(spec) { return this.ask({ kind: 'keyword', ...spec }); }
  /** Pick one object. Result: {entity, pt} | {kw} | {enter:true}. */
  entity(spec) { return this.ask({ kind: 'entity', ...spec }); }

  /** "Select objects:" loop. Returns the array of selected entities (honours pick-first selection). */
  async select(spec = {}) {
    const ed = this.editor;
    if ((spec.pickfirst ?? this.def.pickfirst ?? true) && this.sv.prefs.PICKFIRST && ed.sel.size) {
      const pre = ed.selected.filter(e => this.rt.selectableBy(e, spec));
      if (pre.length) { this.log(`${pre.length} found`); ed.tempSel = new Set(pre.map(e => e.id)); return pre; }
    }
    const set = new Map();
    const result = await this.ask({ kind: 'select', message: spec.prompt || 'Select objects', ...spec, set, remove: false, window: null, previous: spec.previous });
    void result;
    return [...set.values()];
  }

  /** Single object with pick-box. Returns entity or null. */
  async pickEntity(prompt, filter, extra = {}) {
    const r = await this.entity({ prompt, filter, ...extra });
    return r.entity || null;
  }

  setPreview(fn) { this.previewFn = fn; this.rt._refreshPreview(); }
  clearSel() { this.editor.clearSel(); this.rt.emit(); }
  commitChange() { this.editor.prune(); }
  /** Convenience for commands that create entities. */
  add(e) { return this.doc.add(e); }
  /** Transform clones of entities for previews. */
  fail(msg) { throw Error(msg); }
}

// ---------------------------------------------------------------- runtime

export class Runtime {
  constructor(app) {
    this.app = app;
    this.stack = [];
    this.snapper = new Snapper();
    this.editor = null;
    this.cursor = { raw: { x: 0, y: 0 }, pt: { x: 0, y: 0 }, snap: null, track: null };
    this.override = null;
    this.lastCommand = null;
    this.recentCommands = [];
    this.recentInput = [];
    this.preview = [];
    this.idleWindow = null;
    this.lastPoint = null;
    this.hotGrip = null;
    this.dynamicTyping = false;      // typed text originated from the dynamic-input tooltip
    this.shiftOrtho = false;
    this.usage = {};
  }

  attach(ed) { this.editor = ed; this.snapper.reset(); this.preview = []; this.idleWindow = null; this.override = null; }
  get busy() { return this.stack.length > 0; }
  get ctx() { return this.stack[this.stack.length - 1] || null; }
  get pending() { return this.ctx?.pending || null; }
  get sv() { return this.app.sysvars; }
  emit() { this.app.emit('change'); }
  log(t, k) { this.app.log(t, k); }

  apertureWorld(px = null) { return (px ?? this.sv.prefs.APERTURE) / (this.editor?.view.scale || 1); }

  // ------------------------------------------------------------ prompts

  promptText(p = this.pending) {
    if (!p) return 'Command:';
    let msg = p.message || p.prompt || '';
    if (p.options?.length) msg += (/\bor\b/.test(msg) || !msg ? '' : ' or') + ' ' + optionsText(p.options).trim();
    if (p.default !== undefined && p.kind !== 'keyword' && p.default !== null) msg += ` <${typeof p.default === 'object' ? fmtPt(p.default) : (typeof p.default === 'number' ? fmt(p.kind === 'angle' ? g.deg(p.default) : p.default) : p.default)}>`;
    else if (p.kind === 'keyword' && p.default) msg += ` <${p.default}>`;
    return `${msg}:`;
  }

  _promptShown(ctx, p) {
    p.message = p.message || p.prompt;
    if (ctx === this.ctx) this.log(this.promptText(p), 'prompt');
    this._refreshPreview();
    this.app.emit('prompt', p);
    this.emit();
  }

  /** Recompute the rubber-band preview. Pointer moves only need an overlay repaint, not a scene redraw. */
  _refreshPreview(overlayOnly = false) {
    const ctx = this.ctx, p = ctx?.pending;
    this.preview = [];
    if (ctx) {
      const fn = p?.preview || ctx.previewFn;
      if (fn) { try { this.preview = fn(this.cursor.pt, p) || []; } catch (e) { this.preview = []; } }
    }
    if (overlayOnly) this.app.emit('overlay'); else this.emit();
  }

  _deliver(ctx, result) {
    const p = ctx.pending;
    if (!p) return;
    ctx.pending = null;
    this.override = null;
    this.preview = [];
    p.resolve(result);
  }

  // ------------------------------------------------------------ running commands

  /** Run a command by name/alias. Resolves when the command finishes. */
  async run(text, { transparent = false, args = null } = {}) {
    const def = typeof text === 'object' ? text : this.app.registry.find(text);
    if (!def) { this.log(`Unknown command "${String(text)}".  Press F1 for help.`, 'error'); this.emit(); return; }
    if (!this.editor) return;
    const ed = this.editor, isTop = !this.stack.length;
    const ctx = new Ctx(this, def, ed, args);
    if (!isTop && !transparent && !def.transparent) { this.log('That command may not be used transparently.', 'error'); return; }
    this.stack.push(ctx);
    if (!transparent && isTop) ed.doc.beginCommand();
    if (!def.silent) this.log(`Command: ${def.name}`, 'cmd');
    this.usage[def.name] = (this.usage[def.name] || 0) + 1;
    try {
      await def.run(ctx, args || {});
    } catch (e) {
      if (e && e.cancel) this.log('*Cancel*', 'info');
      else { this.log(e?.message || String(e), 'error'); if (!(e instanceof Error) || !/^(Invalid|Requires|Cannot|The )/.test(e.message)) console.debug?.(e); }
    } finally {
      this.stack.splice(this.stack.indexOf(ctx), 1);
      this.preview = []; this.override = null; this.idleWindow = null;
      if (!transparent && isTop) {
        ed.doc.endCommand(def.name);
        ed.tempSel.clear();
        ed.prune();
        this.snapper.reset();
        if (def.repeatable && !def.name.startsWith('_')) this.lastCommand = def.name;
        if (def.repeatable && !def.name.startsWith('_')) { this.recentCommands = [def.name, ...this.recentCommands.filter(n => n !== def.name)].slice(0, 12); }
        this.app.emit('commandEnd', def.name);
      } else if (this.ctx) this._refreshPreview();
      this.emit();
    }
  }

  /** From the UI (ribbon, menus): cancels whatever is running, then starts the command. */
  async start(name, opts = {}) {
    const def = this.app.registry.find(name);
    if (def?.transparent && this.busy) return this.run(def, { ...opts, transparent: true });
    if (this.busy) { this.escape(); await flush(); }
    return this.run(name, opts);
  }

  // ------------------------------------------------------------ point resolution

  resolve(raw, p = this.pending) {
    const sv = this.sv, ed = this.editor, view = ed.view;
    const pointy = p && (p.kind === 'point' || p.kind === 'distance' || p.kind === 'angle');
    if (!pointy) return { pt: { x: raw.x, y: raw.y }, snap: null, track: null };
    const base = p.base || p.firstPoint || null;
    const osmask = p.noSnap ? 0 : activeMask(sv.prefs.OSMODE, this.override);
    const ortho = (sv.ortho !== this.shiftOrtho) && !!base && !p.noOrtho;
    return this.snapper.resolve(ed.doc, raw, {
      scale: view.scale, aperturePx: sv.prefs.APERTURE + 2, osmask, base,
      ortho, polar: sv.polar && !!base && !ortho && !p.noOrtho, polarInc: g.rad(sv.prefs.POLARANG), otrack: sv.otrack && !p.noSnap,
      grid: sv.snapOn ? { on: true, sx: sv.prefs.SNAPUNIT_X, sy: sv.prefs.SNAPUNIT_Y, base: { x: 0, y: 0 }, angle: g.rad(sv.prefs.SNAPANG) } : null,
    });
  }

  /** Mouse move in world coordinates. */
  move(raw) {
    this.cursor.raw = raw;
    const p = this.pending, r = this.resolve(raw, p);
    this.cursor.pt = r.pt; this.cursor.snap = r.snap; this.cursor.track = r.track;
    this.snapper.noteHover(r.snap, this.app.now());
    this._refreshPreview(true);
  }

  // ------------------------------------------------------------ clicks

  async click(raw, mods = {}) {
    const ctx = this.ctx, p = ctx?.pending;
    if (!ctx) { this.idleClick(raw, mods); await flush(); return; }
    if (!p) return;
    const r = this.resolve(raw, p);
    this.cursor = { raw, pt: r.pt, snap: r.snap, track: r.track };
    switch (p.kind) {
      case 'point': return this._pointPicked(ctx, p, r.pt, raw);
      case 'distance': case 'angle': return this._measurePicked(ctx, p, r.pt);
      case 'entity': {
        const e = pickAt(ctx.doc, raw, this.apertureWorld(), { filter: p.filter });
        if (!e) { this.log('Nothing selected', 'error'); await flush(); return; }
        this._deliver(ctx, { entity: e, pt: raw, mods });
        break;
      }
      case 'select': this._selectClick(ctx, p, raw, mods); return;
      default: return;
    }
    await flush();
  }

  async _pointPicked(ctx, p, pt, raw) {
    if (p.fromStage === 'base') { p.fromBase = pt; p.fromStage = 'offset'; this.log('<Offset>:', 'prompt'); return; }
    if (p.fromStage === 'offset') { p.fromStage = null; return this._finishPoint(ctx, p, pt); }
    if (p.m2p) { p.m2p.push(pt); if (p.m2p.length < 2) { this.log('Second point of mid between 2 points:', 'prompt'); return; } const m = g.mid(p.m2p[0], p.m2p[1]); p.m2p = null; return this._finishPoint(ctx, p, m); }
    return this._finishPoint(ctx, p, pt);
  }

  async _finishPoint(ctx, p, pt) {
    this.lastPoint = pt;
    this._deliver(ctx, { pt });
    await flush();
  }

  async _measurePicked(ctx, p, pt) {
    const base = p.base || p.firstPoint;
    if (!base) { p.firstPoint = pt; this.log('Specify second point:', 'prompt'); this._refreshPreview(); return; }
    const value = p.kind === 'distance' ? g.dist(base, pt) : g.angleOf(base, pt);
    this._deliver(ctx, { value, pt });
    await flush();
  }

  idleClick(raw, mods) {
    const ed = this.editor;
    if (!ed) return;
    if (this.idleWindow) { const a = this.idleWindow.a; this.idleWindow = null; this.windowSelect(a, raw, mods); return; }
    const grip = this.gripAt(raw);
    if (grip && !mods.shift) { this.run('_GRIPEDIT', { args: { grip } }); return; }
    const e = pickAt(ed.doc, raw, this.apertureWorld(this.sv.prefs.PICKBOX + 4));
    if (e) {
      const ids = this.groupIds(e);
      if (mods.shift) { const all = ids.every(id => ed.sel.has(id)); for (const id of ids) all ? ed.sel.delete(id) : ed.sel.add(id); }
      else for (const id of ids) ed.sel.add(id);
      this.emit(); return;
    }
    if (!mods.shift) { /* empty click starts an implied window */ }
    this.idleWindow = { a: raw };
    this.emit();
  }

  /** Entities sharing a group with e (GROUP selection). */
  groupIds(e) {
    if (!e.grp) return [e.id];
    return this.editor.doc.entities.filter(x => x.grp === e.grp).map(x => x.id);
  }

  windowSelect(a, b, mods = {}) {
    const ed = this.editor, crossing = b.x < a.x;
    const list = selectRect(ed.doc, rectOf(a, b), crossing);
    const ids = new Set(list.flatMap(e => this.groupIds(e)));
    if (mods.shift) for (const id of ids) ed.sel.delete(id);
    else for (const id of ids) ed.sel.add(id);
    this.log(`${ids.size} found`, 'info');
    this.emit();
  }

  /** Mouse drag (window/crossing) from a to b. */
  async drag(a, b, mods = {}) {
    const ctx = this.ctx, p = ctx?.pending;
    if (!ctx) { this.idleWindow = null; this.windowSelect(a, b, mods); await flush(); return; }
    if (p?.kind === 'select') { this._selectRect(ctx, p, a, b, mods); await flush(); }
  }

  /** Double click: edit the object under the cursor. */
  doubleClick(raw) {
    if (this.busy) return;
    const ed = this.editor, e = pickAt(ed.doc, raw, this.apertureWorld());
    if (!e) return;
    ed.sel.clear(); ed.sel.add(e.id);
    const map = { text: 'TEXTEDIT', pline: 'PEDIT', hatch: 'HATCHEDIT', dim: 'TEXTEDIT', leader: 'TEXTEDIT', insert: 'PROPERTIES' };
    this.start(map[e.type] || 'PROPERTIES', { args: { entity: e } });
  }

  // ------------------------------------------------------------ grips

  gripAt(raw) {
    const ed = this.editor;
    if (!ed.sel.size || !this.sv.prefs.GRIPS || ed.sel.size > this.sv.prefs.GRIPOBJLIMIT) return null;
    const tol = 7 / ed.view.scale;
    for (const e of ed.selected) {
      if (ed.doc.isLocked(e)) continue;
      for (const gr of gripsOf(e, ed.doc)) if (g.dist(gr.pt, raw) <= tol) return { entity: e, grip: gr };
    }
    return null;
  }

  // ------------------------------------------------------------ selection prompt

  selectableBy(e, spec) {
    const d = this.editor.doc, l = d.layerOf(e);
    return (spec.allowOff ? !l.frozen : d.isVisible(e)) && !l.locked && (!spec.filter || spec.filter(e));
  }

  _selAdd(ctx, p, list) {
    const ed = ctx.editor;
    let added = 0;
    for (const e of list) {
      if (!this.selectableBy(e, p)) continue;
      for (const id of this.groupIds(e)) {
        const m = ed.doc.entities.find(x => x.id === id);
        if (!m || !this.selectableBy(m, p)) continue;
        if (p.remove) { if (p.set.delete(id)) added++; } else if (!p.set.has(id)) { p.set.set(id, m); added++; }
      }
    }
    // keep a map keyed by id for stable order
    ed.tempSel = new Set(p.set.keys());
    this.log(p.remove ? `${added} removed, ${p.set.size} total` : `${added} found${p.set.size !== added ? `, ${p.set.size} total` : ''}`);
    this.emit();
    if (p.single && p.set.size) { this.previousSelection = [...p.set.keys()]; this._deliver(ctx, { enter: true }); }
  }

  _selectClick(ctx, p, raw, mods) {
    const ed = ctx.editor;
    if (p.window && p.window.a === null) { p.window.a = raw; this.log('Specify opposite corner:', 'prompt'); this.emit(); return; }
    if (p.window) {
      const a = p.window.a, mode = p.window.mode;
      p.window = null;
      this._selectRect(ctx, p, a, raw, mods, mode);
      return;
    }
    if (p.polygon) { p.polygon.pts.push(raw); this.log(`Specify endpoint of ${p.polygon.kind === 'F' ? 'line' : 'polygon'} or [Undo]:`, 'prompt'); this.emit(); return; }
    const e = pickAt(ed.doc, raw, this.apertureWorld(this.sv.prefs.PICKBOX + 4), { filter: p.filter });
    if (e) { if (mods.shift) { const prev = p.remove; p.remove = true; this._selAdd(ctx, p, [e]); p.remove = prev; } else this._selAdd(ctx, p, [e]); return; }
    if (p.single) { this.log('Nothing selected', 'error'); return; }
    p.window = { a: raw, mode: null };
    this.log('Specify opposite corner:', 'prompt');
    this.emit();
  }

  _selectRect(ctx, p, a, b, mods = {}, mode = null) {
    const crossing = mode ? mode === 'C' : b.x < a.x;
    const list = selectRect(ctx.doc, rectOf(a, b), crossing, { filter: p.filter });
    this.lastWindow = { rect: rectOf(a, b), crossing };
    const prev = p.remove;
    if (mods.shift) p.remove = true;
    this._selAdd(ctx, p, list);
    p.remove = prev;
  }

  _selectText(ctx, p, text) {
    const t = text.trim().toUpperCase();
    const ed = ctx.editor, doc = ctx.doc;
    const polyDone = () => { const pg = p.polygon; p.polygon = null; if (pg.pts.length < (pg.kind === 'F' ? 2 : 3)) return; const list = pg.kind === 'F' ? selectFence(doc, pg.pts, { filter: p.filter }) : selectPolygon(doc, pg.pts, pg.kind === 'CP', { filter: p.filter }); this._selAdd(ctx, p, list); };
    if (p.polygon) {
      if (t === '' ) { polyDone(); return; }
      if (t === 'U' || t === 'UNDO') { p.polygon.pts.pop(); return; }
      const r = parseInput(text, { base: p.polygon.pts[p.polygon.pts.length - 1] });
      if (r?.kind === 'point') { p.polygon.pts.push(r.pt); this.emit(); return; }
      this.log('Invalid point.', 'error'); return;
    }
    const typed = parseInput(text, {});
    if (typed?.kind === 'point') { this._selectClick(ctx, p, typed.pt, {}); return; }
    const opts = {
      // ALL picks everything on thawed, unlocked layers, including objects on layers that are switched off
      ALL: () => { p.allowOff = true; this._selAdd(ctx, p, ed.doc.entities.filter(e => { const l = ed.doc.layerOf(e); return !l.frozen && !l.locked; })); p.allowOff = false; },
      W: () => { p.window = { a: null, mode: 'W', pending: true }; this.log('Specify first corner:', 'prompt'); },
      WINDOW: 'W', C: () => { p.window = { a: null, mode: 'C', pending: true }; this.log('Specify first corner:', 'prompt'); }, CROSSING: 'C',
      WP: () => { p.polygon = { kind: 'WP', pts: [] }; this.log('First polygon point:', 'prompt'); }, CP: () => { p.polygon = { kind: 'CP', pts: [] }; this.log('First polygon point:', 'prompt'); },
      F: () => { p.polygon = { kind: 'F', pts: [] }; this.log('First fence point:', 'prompt'); }, FENCE: 'F',
      L: () => { const last = [...ed.doc.entities].reverse().find(e => this.selectableBy(e, p)); if (last) this._selAdd(ctx, p, [last]); else this.log('0 found'); }, LAST: 'L',
      P: () => { const prev = this.previousSelection?.map(id => ed.doc.entities.find(e => e.id === id)).filter(Boolean) || []; this._selAdd(ctx, p, prev); }, PREVIOUS: 'P',
      R: () => { p.remove = true; this.log('Remove objects:', 'prompt'); }, REMOVE: 'R',
      A: () => { p.remove = false; this.log('Select objects:', 'prompt'); }, ADD: 'A',
      U: () => { const last = [...p.set.keys()].pop(); if (last !== undefined) { p.set.delete(last); ed.tempSel = new Set(p.set.keys()); this.emit(); } }, UNDO: 'U',
      SI: () => { p.single = true; }, SINGLE: 'SI', M: () => {}, MULTIPLE: 'M', ALLSEL: () => {},
    };
    let h = opts[t];
    if (typeof h === 'string') h = opts[h];
    if (h) { h(); return; }
    this.log('*Invalid selection* Expects a point or Window/Last/Crossing/BOX/ALL/Fence/WPolygon/CPolygon/Add/Remove/Multiple/Previous/Undo/SIngle', 'error');
  }

  // ------------------------------------------------------------ typed input

  /** Text submitted from the command line (or dynamic input). */
  async text(raw, { dynamic = false } = {}) {
    const text = String(raw);
    this.dynamicTyping = dynamic;
    const ctx = this.ctx, p = ctx?.pending;
    if (!ctx) { await this._idleText(text); return; }
    if (!p) return;
    if (text.trim() === '' && p.kind !== 'string') { await this.enter(); return; }
    this.recentInput = [text.trim(), ...this.recentInput.filter(t => t !== text.trim())].slice(0, 20);
    this.log(text, 'input');
    // object-snap overrides and transparent apostrophe commands
    const t = text.trim();
    if (t.startsWith("'") && t.length > 1) { this.run(t.slice(1), { transparent: true }); return; }
    switch (p.kind) {
      case 'select': this._selectText(ctx, p, text); break;
      case 'string': this._deliver(ctx, text === '' && p.default !== undefined ? { enter: true, value: p.default } : { value: p.raw ? text : text.trim() }); break;
      case 'keyword': { const kw = matchOption(t, p.options); if (kw) this._deliver(ctx, { kw }); else this.log('Invalid option keyword.', 'error'); break; }
      case 'number': this._numberText(ctx, p, t); break;
      case 'distance': case 'angle': this._measureText(ctx, p, t); break;
      case 'entity': { const kw = matchOption(t, p.options); if (kw) this._deliver(ctx, { kw }); else this.log('Select an object or option keyword.', 'error'); break; }
      case 'point': this._pointText(ctx, p, t); break;
      default: break;
    }
    await flush();
  }

  _pointText(ctx, p, t) {
    const kw = matchOption(t, p.options);
    if (kw) return this._deliver(ctx, { kw });
    const ov = parseOverride(t);
    if (ov) {
      if (ov.kind === 'osnap') { this.override = OS_BITS[ov.mode]; this.log(`_${ov.mode.toLowerCase()} of`, 'prompt'); }
      else if (ov.kind === 'none') { this.override = 'none'; this.log('_non', 'prompt'); }
      else if (ov.kind === 'from') { p.fromStage = 'base'; this.log('_from Base point:', 'prompt'); }
      else if (ov.kind === 'm2p') { p.m2p = []; this.log('First point of mid between 2 points:', 'prompt'); }
      else this.log('Temporary tracking is not supported; use object snap tracking.', 'error');
      return;
    }
    const base = p.fromStage === 'offset' ? p.fromBase : p.base;
    const rel = this.dynamicTyping && this.sv.pointerInput && !!p.base && p.fromStage !== 'offset';
    const scaleMatch = p.acceptNumber && t.match(/^([+-]?(?:\d+\.?\d*|\.\d+))\s*(xp|x)$/i);
    if (scaleMatch) return this._deliver(ctx, { value: Number(scaleMatch[1]), rel: scaleMatch[2].toLowerCase(), typed: true });
    const r = parseInput(t, { base: p.fromStage === 'offset' ? p.fromBase : base, relativeDefault: rel || p.fromStage === 'offset' });
    if (!r) { this.log('Point or option keyword required.', 'error'); return; }
    if (p.fromStage === 'base') { p.fromBase = r.pt || this.cursor.pt; p.fromStage = 'offset'; this.log('<Offset>:', 'prompt'); return; }
    if (r.kind === 'point') { p.fromStage = null; this.lastPoint = r.pt; return this._deliver(ctx, { pt: r.pt, typed: true }); }
    // bare number: scale factors (ZOOM), or direct distance entry along the current cursor direction
    if (r.kind === 'number' && p.acceptNumber) {
      const m = t.match(/^([+-]?(?:\d+\.?\d*|\.\d+))\s*(xp|x)?$/i);
      return this._deliver(ctx, { value: Number(m[1]), rel: m[2] ? m[2].toLowerCase() : null, typed: true });
    }
    if (r.kind === 'number') {
      if (!p.base) { this.log('Point or option keyword required.', 'error'); return; }
      const dir = g.dist(p.base, this.cursor.pt) < 1e-12 ? { x: 1, y: 0 } : g.unit(g.sub(this.cursor.pt, p.base));
      const pt = g.add(p.base, g.mul(dir, r.value));
      this.lastPoint = pt;
      return this._deliver(ctx, { pt, typed: true, dde: true });
    }
  }

  _numberText(ctx, p, t) {
    const kw = matchOption(t, p.options);
    if (kw) return this._deliver(ctx, { kw });
    const r = parseInput(t);
    if (r?.kind !== 'number') { this.log('Requires numeric value.', 'error'); return; }
    let v = r.value;
    if (p.integer && !Number.isInteger(v)) { this.log('Requires an integer value.', 'error'); return; }
    if ((p.min !== undefined && v < p.min) || (p.max !== undefined && v > p.max) || (p.nonzero && v === 0)) { this.log(p.rangeMessage || '*Invalid* Value out of range.', 'error'); return; }
    this._deliver(ctx, { value: v });
  }

  _measureText(ctx, p, t) {
    const kw = matchOption(t, p.options);
    if (kw) return this._deliver(ctx, { kw });
    const r = parseInput(t, { base: p.base });
    if (r?.kind === 'number') {
      if (p.min !== undefined && r.value < p.min) { this.log('Requires a positive value.', 'error'); return; }
      if (p.nonzero && r.value === 0) { this.log('Requires a nonzero value.', 'error'); return; }
      return this._deliver(ctx, { value: p.kind === 'angle' ? g.rad(r.value) : r.value });
    }
    if (r?.kind === 'point') {                         // a typed point sets the base, then measure to the next
      const base = p.base || p.firstPoint;
      if (!base) { p.firstPoint = r.pt; this.log('Specify second point:', 'prompt'); return; }
      return this._deliver(ctx, { value: p.kind === 'distance' ? g.dist(base, r.pt) : g.angleOf(base, r.pt), pt: r.pt });
    }
    this.log(p.kind === 'angle' ? 'Requires numeric angle or second point.' : 'Requires numeric distance or second point.', 'error');
  }

  // ------------------------------------------------------------ Enter / Escape

  async enter() {
    const ctx = this.ctx, p = ctx?.pending;
    if (!ctx) { await this._repeat(); return; }
    if (!p) return;
    if (p.kind === 'select') {
      if (p.window?.pending) { p.window = null; }
      if (p.polygon) { this._selectText(ctx, p, ''); await flush(); return; }
      this.previousSelection = [...p.set.keys()].length ? [...p.set.keys()] : this.previousSelection;
      this._deliver(ctx, { enter: true });
    } else if (p.kind === 'string') {
      this._deliver(ctx, { enter: true, value: p.default ?? '' });
    } else if (p.default !== undefined && p.default !== null && p.kind !== 'keyword') {
      this._deliver(ctx, p.kind === 'point' ? { enter: true, pt: p.default } : { enter: true, value: p.default });
    } else if (p.kind === 'keyword') {
      if (p.default) this._deliver(ctx, { enter: true, kw: p.default }); else this._deliver(ctx, { enter: true });
    } else if (p.allowEnter || p.kind === 'entity') {
      this._deliver(ctx, { enter: true });
    } else if (p.noEnterEnds) {
      this._deliver(ctx, { enter: true });
    } else {
      this.log(p.kind === 'point' ? 'Point or option keyword required.' : 'Invalid input.', 'error');
    }
    await flush();
  }

  /** A keyword chosen from a menu / clicked in the command line. */
  async keyword(label) {
    const ctx = this.ctx, p = ctx?.pending;
    if (!p) return;
    if (p.kind === 'select') { this._selectText(ctx, p, label); await flush(); return; }
    const kw = matchOption(label, p.options);
    if (kw) { this.log(optionName(kw), 'input'); this._deliver(ctx, { kw }); await flush(); }
  }

  escape() {
    this.idleWindow = null; this.override = null;
    const ctx = this.ctx;
    if (!ctx) { this.editor?.clearSel(); this.hotGrip = null; this.emit(); return; }
    for (let i = this.stack.length - 1; i >= 0; i--) {
      const c = this.stack[i];
      if (c.pending) { const p = c.pending; c.pending = null; p.reject(new Cancel()); }
    }
    this.preview = [];
    this.emit();
  }

  async _repeat() {
    if (this.lastCommand) { this.run(this.lastCommand); await flush(); }
  }

  // ------------------------------------------------------------ idle typing: commands, aliases, system variables

  async _idleText(text) {
    const t = text.trim();
    if (!t) { await this._repeat(); return; }
    this.log(`Command: ${t}`, 'input');
    const name = t.replace(/^'/, '');
    if (this.app.registry.find(name)) { this.run(name, { transparent: false }); await flush(); return; }
    if (this.sv.has(name)) { this.run('SETVAR', { args: { name: name.toUpperCase() } }); await flush(); return; }
    // typed object snap names / coordinates at idle are meaningless
    this.log(`Unknown command "${t}".  Press F1 for help.`, 'error');
    this.emit();
  }

  /** Run a list of command-line lines (SCRIPT). */
  async script(lines) {
    for (const line of lines) {
      if (/^\s*;/.test(line)) continue;
      await this.text(line);
      await flush();
    }
  }
}
