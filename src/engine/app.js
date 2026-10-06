// Application state: open drawings (editors), shared preferences, command history and the runtime.
import { Drawing } from '../core/drawing.js';
import { View } from './view.js';
import { Sysvars } from './sysvars.js';
import { registry } from './registry.js';
import { Runtime } from './runtime.js';
import { boundsOf } from '../core/entities.js';
import * as g from '../core/geom.js';

/** Per-drawing editing state. */
export class Editor {
  constructor(doc) {
    this.doc = doc;
    this.view = new View();
    this.sel = new Set();          // selected entity ids
    this.tempSel = new Set();      // entities highlighted while a Select objects prompt is active
    this.hoverId = null;
    this.lastView = null;
    this.zoomed = false;
  }

  entityById(id) { return this.doc.entities.find(e => e.id === id) || null; }
  get selected() { return this.doc.entities.filter(e => this.sel.has(e.id)); }
  select(list) { for (const e of list) this.sel.add(e.id); }
  deselect(list) { for (const e of list) this.sel.delete(e.id); }
  clearSel() { this.sel.clear(); this.tempSel.clear(); }
  /** Drop ids that no longer exist (after undo, erase...). */
  prune() {
    const live = new Set(this.doc.entities.map(e => e.id));
    for (const id of [...this.sel]) if (!live.has(id)) this.sel.delete(id);
    for (const id of [...this.tempSel]) if (!live.has(id)) this.tempSel.delete(id);
  }

  extents() {
    let b = null;
    for (const e of this.doc.entities) if (this.doc.isVisible(e)) b = g.unionBounds(b, boundsOf(e, this.doc));
    return b;
  }

  zoomExtents(margin = .06) {
    this.view.push();
    const solids = this.doc.solids.flatMap(s => s.vertices);
    if (!this.view.isPlan && solids.length) { this.view.fitPoints3D(solids); return; }
    const b = this.extents();
    this.view.fit(b, margin);
    this.zoomed = true;
  }
}

export class App {
  constructor({ storage = null, now = () => Date.now() } = {}) {
    this.storage = storage;
    this.now = now;
    this.editors = [];
    this.active = null;
    this.sysvars = new Sysvars(() => this.doc);
    this.registry = registry;
    this.history = [];             // command-line transcript [{text, kind}]
    this.clipboard = null;
    this.listeners = new Map();
    this.counter = 0;
    this.runtime = new Runtime(this);
    this.recent = [];
  }

  get editor() { return this.active; }
  get doc() { return this.active?.doc; }
  get view() { return this.active?.view; }

  on(evt, fn) { if (!this.listeners.has(evt)) this.listeners.set(evt, new Set()); this.listeners.get(evt).add(fn); return () => this.listeners.get(evt).delete(fn); }
  emit(evt, data) { for (const fn of this.listeners.get(evt) || []) fn(data); }

  log(text, kind = 'info') {
    this.history.push({ text, kind });
    if (this.history.length > 2000) this.history.splice(0, this.history.length - 2000);
    this.emit('log', { text, kind });
  }

  newDrawing({ name, template = 'metric' } = {}) {
    const doc = new Drawing(name || `Drawing${++this.counter}`);
    if (template === 'imperial') { doc.settings.units.insUnits = 1; doc.settings.units.precision = 4; doc.settings.textHeight = .2; Object.assign(doc.settings.dimStyles.Standard, { txt: .18, asz: .18, exo: .0625, exe: .18, gap: .09, cen: .09, dec: 4 }); }
    return this.addDrawing(doc);
  }

  addDrawing(doc) {
    const ed = new Editor(doc);
    if (doc.view) ed.view.setState(doc.view);
    this.editors.push(ed);
    this.setActive(ed);
    this.emit('drawings');
    return ed;
  }

  setActive(ed) {
    if (this.active === ed) return;
    if (this.active && this.runtime.busy) this.runtime.escape();
    this.active = ed;
    this.runtime.attach(ed);
    this.emit('active', ed);
    this.emit('change');
  }

  closeEditor(ed) {
    const i = this.editors.indexOf(ed);
    if (i < 0) return;
    this.editors.splice(i, 1);
    if (this.active === ed) { this.active = null; this.setActive(this.editors[Math.max(0, i - 1)] || null); }
    this.emit('drawings');
  }
}
