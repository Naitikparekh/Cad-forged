// The drawing database: layers, blocks, model/paper-space entities, solids and settings, plus
// snapshot-based undo. This module has no DOM dependency.
import { newLayer, defaultSettings, LW_BYLAYER } from './style.js';

export const FORMAT_VERSION = 2;
const MAX_UNDO = 100, MAX_UNDO_BYTES = 96e6;

export class Drawing {
  constructor(name = 'Drawing1') {
    this.name = name;
    this.rev = 0;                 // bumped on every change; caches key off it
    this.nextId = 1;
    this.layers = [newLayer('0')];
    this.blocks = {};             // name -> { name, base:{x,y}, entities:[] }
    this.settings = defaultSettings();
    this.model = { entities: [] };
    this.layouts = [];            // [{ name, paper:{w,h,name,landscape}, entities:[] }]
    this.solids = [];
    this.views = {};              // named views: name -> { cx, cy, scale }
    this.cur = { layer: '0', color: undefined, ltype: undefined, lw: undefined };
    this.space = 'model';         // active space: 'model' or a layout name
    this.view = null;             // last camera state (persisted)
    this.dirty = false;
    this.fileName = null;
    this.undoStack = []; this.redoStack = [];
    this._pre = null; this._cmdActive = false; this._cmdRev = 0;
  }

  // ------------------------------------------------------------ spaces & entities

  get entities() { return this.spaceObj(this.space).entities; }
  spaceObj(id) { return id === 'model' ? this.model : this.layouts.find(l => l.name === id) || this.model; }
  get isModel() { return this.space === 'model'; }

  *allEntities() {
    yield* this.model.entities;
    for (const l of this.layouts) yield* l.entities;
    for (const b of Object.values(this.blocks)) yield* b.entities;
  }

  byId(id) {
    for (const e of this.allEntities()) if (e.id === id) return e;
    return null;
  }

  /** Add an entity to the active space, filling in id and the current layer/color/linetype/lineweight. */
  add(e, space = this.space) {
    this.willChange();
    if (e.id === undefined) e.id = this.nextId++;
    else if (e.id >= this.nextId) this.nextId = e.id + 1;
    if (e.layer === undefined) e.layer = this.cur.layer;
    for (const k of ['color', 'ltype', 'lw']) if (e[k] === undefined && this.cur[k] !== undefined) e[k] = this.cur[k];
    this.spaceObj(space).entities.push(e);
    this.touch();
    return e;
  }

  addAll(list, space = this.space) { return list.map(e => this.add(e, space)); }

  remove(e) {
    this.willChange();
    const arr = this.entities, i = arr.indexOf(e);
    if (i >= 0) arr.splice(i, 1);
    this.touch();
  }

  removeAll(list) {
    this.willChange();
    const set = new Set(list), arr = this.entities;
    for (let i = arr.length - 1; i >= 0; i--) if (set.has(arr[i])) arr.splice(i, 1);
    this.touch();
  }

  /** Replace an entity in place, keeping its id and draw order. */
  replace(old, next) {
    this.willChange();
    const arr = this.entities, i = arr.indexOf(old);
    if (i < 0) return null;
    next.id = old.id;
    for (const k of ['layer', 'color', 'ltype', 'lw', 'lts', 'grp']) if (next[k] === undefined && old[k] !== undefined) next[k] = old[k];
    arr[i] = next;
    this.touch();
    return next;
  }

  /** Run a mutation inside a change notification (call before mutating entities directly). */
  change(fn) { this.willChange(); const r = fn(); this.touch(); return r; }

  touch() { this.rev++; this.dirty = true; }

  // ------------------------------------------------------------ layers

  layer(name) { const k = String(name).toLowerCase(); return this.layers.find(l => l.name.toLowerCase() === k); }
  layerOf(e) { return this.layer(e.layer) || this.layers[0]; }
  get curLayer() { return this.layer(this.cur.layer) || this.layers[0]; }
  isVisible(e) { const l = this.layerOf(e); return l.on && !l.frozen; }
  isLocked(e) { return this.layerOf(e).locked; }

  addLayer(name, props = {}) {
    if (this.layer(name)) throw Error(`A layer named "${name}" already exists.`);
    this.willChange();
    const l = newLayer(name, props);
    this.layers.push(l);
    this.touch();
    return l;
  }

  ensureLayer(name) { return this.layer(name) || this.addLayer(name); }

  layerUsage(name) {
    const k = name.toLowerCase();
    for (const e of this.allEntities()) if ((e.layer || '').toLowerCase() === k) return true;
    return false;
  }

  deleteLayer(name) {
    const l = this.layer(name);
    if (!l) return false;
    if (l.name === '0' || l.name.toLowerCase() === 'defpoints') throw Error(`Layer "${l.name}" cannot be deleted.`);
    if (l === this.curLayer) throw Error('The current layer cannot be deleted.');
    if (this.layerUsage(l.name)) throw Error(`Layer "${l.name}" contains objects and cannot be deleted.`);
    this.willChange();
    this.layers.splice(this.layers.indexOf(l), 1);
    this.touch();
    return true;
  }

  renameLayer(oldName, newName) {
    const l = this.layer(oldName);
    if (!l || l.name === '0') throw Error('That layer cannot be renamed.');
    if (this.layer(newName) && this.layer(newName) !== l) throw Error(`A layer named "${newName}" already exists.`);
    this.willChange();
    for (const e of this.allEntities()) if ((e.layer || '').toLowerCase() === oldName.toLowerCase()) e.layer = newName;
    if (this.cur.layer.toLowerCase() === oldName.toLowerCase()) this.cur.layer = newName;
    l.name = newName;
    this.touch();
  }

  setCurrentLayer(name) {
    const l = this.layer(name);
    if (!l) throw Error(`Layer "${name}" not found.`);
    if (l.frozen) throw Error(`Layer "${l.name}" is frozen and cannot be made current.`);
    this.cur.layer = l.name;
  }

  // ------------------------------------------------------------ solids

  addSolid(s) {
    this.willChange();
    if (s.id === undefined) s.id = this.nextId++; else if (s.id >= this.nextId) this.nextId = s.id + 1;
    if (s.layer === undefined) s.layer = this.cur.layer;
    if (s.color === undefined && this.cur.color !== undefined && typeof this.cur.color !== 'string') s.color = this.cur.color;
    this.solids.push(s);
    this.touch();
    return s;
  }

  removeSolid(s) {
    this.willChange();
    const i = this.solids.indexOf(s);
    if (i >= 0) this.solids.splice(i, 1);
    this.touch();
  }

  solidById(id) { return this.solids.find(s => s.id === id) || null; }
  solidVisible(s) { const l = this.layer(s.layer || '0') || this.layers[0]; return l.on && !l.frozen; }

  // ------------------------------------------------------------ blocks

  defineBlock(name, base, entities) {
    this.willChange();
    this.blocks[name] = { name, base: { x: base.x, y: base.y }, entities };
    for (const e of entities) if (e.id === undefined) e.id = this.nextId++;
    this.touch();
    return this.blocks[name];
  }

  // ------------------------------------------------------------ undo / redo

  /** Call before mutating. Takes the pre-image snapshot once per command. */
  willChange() {
    if (this._cmdActive && this._pre === null) this._pre = this.snapshot();
  }

  beginCommand() { this._cmdActive = true; this._pre = null; this._cmdRev = this.rev; }

  /** End a command; if anything changed, the pre-image becomes an undo step. */
  endCommand(label = '') {
    if (this._cmdActive && this._pre !== null && this.rev !== this._cmdRev) {
      this.undoStack.push({ snap: this._pre, label });
      this.redoStack.length = 0;
      this.trimUndo();
    }
    this._cmdActive = false; this._pre = null;
  }

  /** Push an undo step right now (for changes made outside a command). */
  checkpoint(label = '') {
    this.undoStack.push({ snap: this.snapshot(), label });
    this.redoStack.length = 0;
    this.trimUndo();
  }

  trimUndo() {
    let bytes = 0;
    for (const s of this.undoStack) bytes += s.snap.length;
    while (this.undoStack.length > MAX_UNDO || (bytes > MAX_UNDO_BYTES && this.undoStack.length > 1)) bytes -= this.undoStack.shift().snap.length;
  }

  snapshot() {
    return JSON.stringify({ layers: this.layers, blocks: this.blocks, settings: this.settings, model: this.model, layouts: this.layouts, solids: this.solids, views: this.views, cur: this.cur, nextId: this.nextId });
  }

  restore(snap) {
    const s = JSON.parse(snap);
    this.layers = s.layers; this.blocks = s.blocks; this.settings = s.settings; this.model = s.model; this.layouts = s.layouts;
    this.solids = s.solids; this.views = s.views; this.cur = s.cur; this.nextId = Math.max(this.nextId, s.nextId);
    if (!this.layouts.some(l => l.name === this.space) && this.space !== 'model') this.space = 'model';
    this.touch();
  }

  get canUndo() { return this.undoStack.length > 0; }
  get canRedo() { return this.redoStack.length > 0; }

  undo() {
    const step = this.undoStack.pop();
    if (!step) return null;
    this.redoStack.push({ snap: this.snapshot(), label: step.label });
    this.restore(step.snap);
    return step.label;
  }

  redo() {
    const step = this.redoStack.pop();
    if (!step) return null;
    this.undoStack.push({ snap: this.snapshot(), label: step.label });
    this.restore(step.snap);
    return step.label;
  }

  // ------------------------------------------------------------ persistence

  serialize() {
    return {
      format: 'cadforge', version: FORMAT_VERSION, name: this.name, settings: this.settings, layers: this.layers, blocks: this.blocks,
      model: this.model, layouts: this.layouts, solids: this.solids, views: this.views, cur: this.cur, nextId: this.nextId, view: this.view,
    };
  }

  static deserialize(obj, name) {
    if (!obj || typeof obj !== 'object') throw Error('Not a CadForge drawing.');
    if (obj.format !== 'cadforge') return Drawing.fromLegacy(obj, name);
    if (obj.version > FORMAT_VERSION) throw Error('This drawing was saved by a newer version of CadForge.');
    const d = new Drawing(name || obj.name || 'Drawing1');
    d.settings = { ...defaultSettings(), ...obj.settings, units: { ...defaultSettings().units, ...obj.settings?.units }, dimStyles: { ...defaultSettings().dimStyles, ...obj.settings?.dimStyles } };
    d.layers = obj.layers?.length ? obj.layers : [newLayer('0')];
    d.blocks = obj.blocks || {}; d.model = obj.model || { entities: [] }; d.layouts = obj.layouts || [];
    d.solids = obj.solids || []; d.views = obj.views || {}; d.cur = { ...d.cur, ...obj.cur }; d.view = obj.view || null;
    d.nextId = obj.nextId || 1;
    for (const e of d.allEntities()) if (e.id >= d.nextId) d.nextId = e.id + 1;
    d.validate();
    d.dirty = false;
    return d;
  }

  validate() {
    const fin = n => Number.isFinite(n), okp = p => p && fin(p.x) && fin(p.y);
    if (!this.layers.length) throw Error('Drawing has no layers.');
    for (const e of this.allEntities()) {
      let ok = true;
      switch (e.type) {
        case 'line': ok = okp(e.a) && okp(e.b); break;
        case 'circle': ok = okp(e.c) && fin(e.r) && e.r > 0; break;
        case 'arc': ok = okp(e.c) && fin(e.r) && e.r > 0 && fin(e.a0) && fin(e.a1); break;
        case 'pline': ok = Array.isArray(e.v) && e.v.length > 1 && e.v.every(okp); break;
        case 'point': ok = okp(e.p); break;
        case 'text': ok = okp(e.p) && fin(e.h) && e.h > 0 && typeof e.s === 'string'; break;
        case 'insert': ok = okp(e.p) && typeof e.name === 'string'; break;
        default: break;
      }
      if (!ok) throw Error(`Invalid ${e.type} entity (id ${e.id}).`);
    }
    for (const s of this.solids) {
      if (!Array.isArray(s.vertices) || !Array.isArray(s.faces)) throw Error('Invalid solid.');
      for (const f of s.faces) if (f.length !== 3 || f.some(i => !Number.isInteger(i) || i < 0 || i >= s.vertices.length)) throw Error('Invalid solid geometry.');
    }
  }

  /** Import a version-1 CadForge project (flat entities, points[] arrays, group-based blocks). */
  static fromLegacy(old, name) {
    if (!old || !Array.isArray(old.layers) || !Array.isArray(old.entities)) throw Error('Not a CadForge drawing.');
    const d = new Drawing(name || 'Drawing1');
    d.layers = old.layers.map(l => newLayer(l.name, { color: l.color, on: l.visible !== false }));
    if (!d.layer('0')) d.layers.unshift(newLayer('0'));
    d.cur.layer = d.layers[0].name;
    const uuidMap = new Map();
    const conv = o => {
      const base = { layer: o.layer, color: undefined };
      let e;
      if (o.type === 'line') e = { type: 'line', a: { ...o.points[0] }, b: { ...o.points[1] } };
      else if (o.type === 'circle') e = { type: 'circle', c: { ...o.center }, r: o.radius };
      else if (o.type === 'polyline') e = { type: 'pline', v: o.points.map(p => ({ x: p.x, y: p.y, b: 0 })), closed: !!o.closed };
      else if (o.type === 'text') e = { type: 'text', p: { ...o.points[0] }, h: o.height, s: o.text, rot: 0, jh: 'left', jv: 'baseline' };
      else return null;
      return { ...e, ...base };
    };
    for (const o of old.entities) {
      const e = conv(o);
      if (!e) continue;
      d.add(e, 'model');
      if (o.uuid) uuidMap.set(o.uuid, e.id);
    }
    for (const [bn, b] of Object.entries(old.blocks || {})) {
      d.blocks[bn] = { name: bn, base: { x: 0, y: 0 }, entities: b.items.map(conv).filter(Boolean).map(e => ({ ...e, id: d.nextId++ })) };
    }
    d.solids = (old.solids || []).map(s => {
      const m = structuredClone(s);
      const fixFeature = f => ({ ...f, profile: uuidMap.get(f.profile) ?? f.profile });
      if (m.extrusionProgram) m.extrusionProgram.features = m.extrusionProgram.features.map(fixFeature);
      return m;
    });
    for (const [vn, l] of Object.entries(old.layouts || {})) if (l.view) d.views[vn] = { cx: l.view.x, cy: l.view.y, scale: l.view.scale };
    d.dirty = false;
    return d;
  }
}

export const lwOf = e => e.lw === undefined ? LW_BYLAYER : e.lw;
