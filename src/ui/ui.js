// The application shell: builds every UI component, owns global keyboard handling, context menus,
// palettes, dialogs and the `app.ui` interface the command layer talks to.
import { h, svgIcon } from './dom.js';
import { showMenu, closeMenus } from './menu.js';
import { Ribbon } from './ribbon.js';
import { TitleBar } from './titlebar.js';
import { DocTabs, StartPage } from './doctabs.js';
import { Viewport } from './viewport.js';
import { CommandLine } from './cmdline.js';
import { StatusBar } from './statusbar.js';
import { AppMenu } from './appmenu.js';
import { MenuBar } from './menubar.js';
import { Dialogs } from './dialogs.js';
import { Palettes } from './palettes.js';
import { TextEditor } from './texteditor.js';
import { TextWindow } from './textwindow.js';
import { Files } from './files.js';
import { THEMES } from '../render/renderer.js';
import { registry } from '../engine/registry.js';
import { OS_BITS, OS_FULL } from '../engine/parse.js';
import { LINETYPES, LW_BYLAYER } from '../core/style.js';

const VARIES = '*VARIES*';

export class UI {
  constructor(app, els) {
    this.app = app; this.els = els; app.ui = this;
    this.workspace = 'Drafting & Annotation';
    this.theme = THEMES[app.sysvars.prefs.THEME] || THEMES.dark;
    this.showingStart = false;
    this.toastTimer = null;

    this.files = new Files(this);
    this.dialogs = new Dialogs(this);
    this.textEditor = new TextEditor(this);
    this.appMenu = new AppMenu(this, els.overlay);
    this.titlebar = new TitleBar(this, els.titlebar);
    this.menubarUI = new MenuBar(this, els.menubar);
    this.ribbon = new Ribbon(this, els.ribbon);
    this.doctabs = new DocTabs(this, els.doctabs);
    this.viewport = new Viewport(this, els.viewport);
    this.startEl = h('div.startpage', { hidden: true }); els.workarea.append(this.startEl);
    this.start = new StartPage(this, this.startEl);
    this.cmdline = new CommandLine(this, els.cmdline);
    this.statusbar = new StatusBar(this, els.statusbar);
    this.palettes = new Palettes(this, { left: els.dockLeft, right: els.dockRight, root: els.overlay });
    this.textWindow = new TextWindow(this, els.overlay);
    this.toastEl = h('div.toast', { hidden: true, role: 'status' }); els.overlay.append(this.toastEl);

    this.bindKeys();
    this.applyTheme(this.theme === THEMES.light ? 'light' : 'dark');
    this.menubarUI.sync();
    app.on('active', () => { this.viewport.buildControls(); });
    window.addEventListener('beforeunload', e => { if (app.editors.some(x => x.doc.dirty)) { e.preventDefault(); e.returnValue = ''; } });
    window.addEventListener('resize', () => this.layoutChanged());
  }

  get editor() { return this.app.active; }

  // ------------------------------------------------------------ commands & feedback

  /** Start a command from a ribbon button, menu or the command line. */
  runCommand(name, args) {
    if (!this.app.active) return Promise.resolve();
    closeMenus();
    if (this.showingStart) this.showStart(false);
    const p = this.app.runtime.start(name, args ? { args } : {});
    this.cmdline.focus();
    return p.then(() => { this.viewport.invalidate(); });
  }

  escape() {
    this.cmdline.reset();
    if (this.textEditor.active) { this.textEditor.cancel(); return; }
    if (this.dialogs.top()) { this.dialogs.cancelTop(); return; }
    if (this.viewport.endRealtime()) return;
    closeMenus();
    this.app.runtime.escape();
    this.viewport.invalidate();
  }

  toast(msg, ms = 3200) {
    this.toastEl.textContent = msg; this.toastEl.hidden = false;
    clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => { this.toastEl.hidden = true; }, ms);
  }

  showMenu(items, x, y, opts) { return showMenu(items, x, y, opts); }

  /** Wrap a non-command document edit in an undo step. */
  edit(label, fn) {
    const doc = this.app.doc; if (!doc) return;
    const nested = doc._cmdActive;                       // already inside a command: it owns the undo step
    if (!nested) doc.beginCommand();
    try { fn(doc); } finally { if (!nested) doc.endCommand(label); }
    this.editor.prune();
    this.app.emit('change');
  }

  layoutChanged() { requestAnimationFrame(() => { this.viewport?.resize(); }); }
  viewChanged() { this.viewport.viewChanged(); }
  viewPush() { this.editor?.view.push(); }

  // ------------------------------------------------------------ start page / documents

  showStart(on) {
    this.showingStart = !!on;
    this.startEl.hidden = !on;
    this.viewport.el.hidden = !!on;
    if (on) this.start.build();
    this.doctabs.render();
    if (!on) this.layoutChanged();
  }

  async closeDrawing(a) {
    const ed = a?.rt ? this.app.active : (a || this.app.active);
    if (!ed) return;
    if (ed.doc.dirty) {
      const r = await this.dialogs.open('confirm', { title: 'CadForge', message: `Save changes to ${ed.doc.name}?`, buttons: ['Save', "Don't Save", 'Cancel'] });
      if (r === 'Cancel' || r === null) return;
      if (r === 'Save') { this.app.setActive(ed); const ok = await this.files.save(); if (!ok) return; }
    }
    this.app.closeEditor(ed);
    if (!this.app.editors.length) this.showStart(true);
    this.doctabs.render();
  }

  // ------------------------------------------------------------ appearance

  applyTheme(name) {
    this.theme = THEMES[name] || THEMES.dark;
    document.documentElement.dataset.theme = name;
    this.viewport?.invalidate();
  }
  setTheme(name) { this.app.sysvars.set('THEME', name); this.applyTheme(name); try { localStorage.setItem('cf.theme', name); } catch { /* ignore */ } }

  setWorkspace(n) {
    this.workspace = n;
    if (n === 'Drafting & Annotation') { this.ribbon.mode = 0; this.ribbon.activeId = 'home'; this.toggleMenubar(false); this.palettes.show('properties', false); }
    else if (n === '3D Modeling') { this.ribbon.mode = 0; this.ribbon.activeId = 'solid'; this.toggleMenubar(false); this.palettes.show('properties', true); this.ribbon.render(); this.app.active?.view.setPreset('swiso'); this.viewChanged(); }
    else { this.ribbon.mode = 1; this.toggleMenubar(true); this.palettes.show('properties', true); }
    this.ribbon.render(); this.titlebar.renderQat(); this.layoutChanged();
  }

  toggleMenubar(on) {
    const pr = this.app.sysvars.prefs;
    this.app.sysvars.set('MENUBAR', on === undefined ? (pr.MENUBAR ? 0 : 1) : (on ? 1 : 0));
    this.menubarUI.sync(); this.layoutChanged();
  }
  menubar() { this.toggleMenubar(); }

  cleanScreen() { document.body.classList.toggle('clean'); this.statusbar.refresh(); this.layoutChanged(); }
  cleanScreenToggle() { this.cleanScreen(); }

  togglePalette(name, show) { this.palettes.toggle(name, show); this.statusbar?.refresh(); }

  // ------------------------------------------------------------ current properties (ribbon combos)

  currentProps() {
    const doc = this.app.doc, ed = this.app.active;
    if (!doc) return { layer: '0', layerObj: null };
    const sel = [...ed.selected, ...ed.selectedSolids];
    const one = (get) => { const set = new Set(sel.map(e => JSON.stringify(get(e) ?? null))); return set.size === 1 ? get(sel[0]) : VARIES; };
    const layerName = sel.length ? one(e => e.layer) : doc.cur.layer;
    const color = sel.length ? one(e => e.color) : doc.cur.color;
    const lw = sel.length ? one(e => e.lw) : doc.cur.lw;
    const ltype = sel.length ? one(e => e.ltype) : doc.cur.ltype;
    const layerObj = layerName === VARIES ? null : doc.layer(layerName);
    const ent = sel.length === 1 ? sel[0] : null;
    return {
      layer: layerName, layerObj, color, lw, ltype,
      lwResolved: lw === undefined || lw === LW_BYLAYER ? (layerObj?.lw ?? 25) : (lw >= 0 ? lw : (layerObj?.lw ?? 25)),
      ltypeResolved: ltype === undefined || ltype === 'BYBLOCK' ? (layerObj?.ltype || 'Continuous') : ltype,
      entity: ent, count: sel.length,
    };
  }

  /** Set layer / color / lw / ltype for the selection, or the current setting when nothing is selected. */
  setProperty(kind, value) {
    const doc = this.app.doc, ed = this.app.active; if (!doc) return;
    const sel = [...ed.selected, ...ed.selectedSolids].filter(e => !(doc.layer(e.layer || '0')?.locked));
    if (sel.length) {
      this.edit('Properties', d => {
        d.willChange();
        for (const e of sel) { if (value === undefined) delete e[kind]; else e[kind] = value; }
        d.touch();
      });
    } else {
      this.edit('Properties', d => { if (kind === 'layer') d.setCurrentLayer(value); else d.cur[kind] = value; });
    }
    this.viewport.invalidate();
  }

  chooseLayer(name) {
    const doc = this.app.doc; const l = doc.layer(name); if (!l) return;
    if (this.app.active.sel.size === 0 && (l.frozen)) { this.toast('Cannot make a frozen layer current.'); return; }
    this.setProperty('layer', name);
  }

  layerToggle(l, key) {
    const doc = this.app.doc;
    if (key === 'frozen' && !l.frozen && l.name === doc.cur.layer) { this.toast('Cannot freeze the current layer.'); return; }
    if (key === 'on' && l.on && l.name === doc.cur.layer) this.toast('The current layer is turned off.');
    this.edit(`Layer ${key}`, d => { d.willChange(); l[key] = key === 'on' ? !l.on : !l[key]; d.touch(); });
    this.viewport.invalidate();
  }

  visualStyle() { return this.app.active?.visualStyle || '2D Wireframe'; }
  setVisualStyle(n) {
    const ed = this.app.active; if (!ed) return;
    ed.visualStyle = n;
    if (n !== '2D Wireframe' && ed.view.isPlan && ed.doc.solids.length) { ed.view.setPreset('swiso'); }
    this.ribbon.refresh(); this.viewport.buildControls(); this.viewport.invalidate();
  }

  // ------------------------------------------------------------ dialogs / palettes / editors (app.ui interface)

  dialog(name, data) { return this.dialogs.open(name, data); }
  editText(spec) { return this.textEditor.edit(spec); }

  async realtime(mode, ctx) {
    this.viewport.canvasFocus();
    await this.viewport.startRealtime(mode);
    void ctx;
  }

  openText(accept = '.txt') { return this.files.pickText(accept); }
  async exportEntities(c, ents) { return this.files.exportEntities(ents); }

  async hatchEditSession(c, e, live) {
    for (;;) {
      const r = await c.keyword({ prompt: 'Adjust the hatch in the ribbon, then press Enter to accept', options: ['seTtings'], default: 'Accept' });
      if (r.kw === 'seTtings') await this.dialog('hatch', live); else break;
    }
  }
  hatchDone() { this.viewport.invalidate(); }

  // delegates from registry commands: fn(ctx, args)
  async dsettings(c, args) { await this.dialog('dsettings', { tab: args?.tab }); }
  async options() { await this.dialog('options'); }
  async units() { await this.dialog('units'); }
  async dimstyle() { await this.dialog('dimstyle'); }
  async plot() { await this.dialog('plot'); }
  async qselect() { await this.dialog('qselect'); }
  async aliasEdit() { await this.dialog('aliasedit'); }
  async textStyle() { await this.dialog('textstyle'); }
  async layerState() { await this.dialog('layerstate'); }
  async about() { await this.dialog('about'); }
  async help() { await this.dialog('help'); }
  async calc() { await this.dialog('calc'); }
  async purgeDialog() { await this.dialog('purge'); }
  async newDrawing(c, args) { return this.files.newDrawing(args?.template ? { template: args.template } : {}); }
  async open() { return this.files.open(); }
  async save() { return this.files.save(); }
  async saveAs() { return this.files.saveAs(); }
  async exportDXF() { return this.files.exportDXF(); }
  async exportPDF() { return this.files.exportPDF(); }
  async exportMenu() { return this.files.exportMenu(); }
  async importFile() { return this.files.importFile(); }
  async exportMesh(fmt) { return this.files.exportMesh(fmt); }
  async quit() { const dirty = this.app.editors.some(e => e.doc.dirty); if (!dirty || await this.dialogs.open('confirm', { title: 'CadForge', message: 'Discard unsaved changes and close all drawings?', buttons: ['Close All', 'Cancel'] }) === 'Close All') { for (const ed of [...this.app.editors]) this.app.closeEditor(ed); this.showStart(true); } }

  // ------------------------------------------------------------ context menus

  contextMenu(x, y, { shift } = {}) {
    const rt = this.app.runtime, ed = this.app.active, p = rt.pending, sv = this.app.sysvars;
    if (shift) return this.osnapMenu(x, y);
    if (rt.busy) {
      if (!p) return;
      const hasOpts = !!p.options?.length && p.kind !== 'select', mode = sv.prefs.SHORTCUTMENU;
      if (p.kind === 'select' || !((mode & 4) || (hasOpts && (mode & 8)))) { rt.enter(); return; }
      const opts = (p.options || []).map(o => ({ label: o.replace(/\s*\(.*\)\s*$/, ''), action: () => rt.keyword(o) }));
      return showMenu([
        { label: 'Enter', shortcut: 'Enter', action: () => rt.enter() }, { label: 'Cancel', shortcut: 'Esc', action: () => this.escape() },
        { label: 'Recent Input', submenu: () => (rt.recentInput.length ? rt.recentInput.map(t => ({ label: t, action: () => rt.text(t) })) : [{ label: 'No recent input', disabled: true }]) },
        { sep: true }, ...(opts.length ? [...opts, { sep: true }] : []),
        { label: 'Snap Overrides', submenu: () => this.osnapItems() },
        { sep: true }, { label: 'Pan', icon: 'pan', action: () => this.runCommand('PAN') }, { label: 'Zoom', icon: 'zoom', action: () => this.runCommand('ZOOM') },
      ], x, y);
    }
    const hot = rt.hotGrip;
    if (hot) return showMenu([{ label: 'Enter', action: () => rt.enter() }, { label: 'Move', action: () => rt.text('MO') }, { label: 'Mirror', action: () => rt.text('MI') }, { label: 'Rotate', action: () => rt.text('RO') }, { label: 'Scale', action: () => rt.text('SC') }, { label: 'Stretch', action: () => rt.text('ST') }, { sep: true }, { label: 'Exit', action: () => this.escape() }], x, y);
    const sel = ed.sel.size;
    const paste = { label: 'Paste', icon: 'paste', shortcut: 'Ctrl+V', disabled: !this.app.clipboard, action: () => this.runCommand('PASTECLIP') };
    const clipboard = { label: 'Clipboard', submenu: [
      { label: 'Cut', icon: 'cutclip', shortcut: 'Ctrl+X', disabled: !sel, action: () => this.runCommand('CUTCLIP') }, { label: 'Copy', icon: 'copyclip', shortcut: 'Ctrl+C', disabled: !sel, action: () => this.runCommand('COPYCLIP') }, paste] };
    const recent = { label: 'Recent Input', submenu: () => (rt.recentCommands.length ? rt.recentCommands.map(c => ({ label: c, action: () => this.runCommand(c) })) : [{ label: 'No recent commands', disabled: true }]) };
    const repeat = rt.lastCommand ? { label: `Repeat ${rt.lastCommand}`, action: () => this.runCommand(rt.lastCommand) } : null;
    if (!sel) {
      return showMenu([
        ...(repeat ? [repeat] : []), recent, { sep: true }, clipboard, { sep: true },
        { label: 'Undo', icon: 'undo', shortcut: 'Ctrl+Z', disabled: !ed.doc.canUndo, action: () => this.runCommand('U') }, { label: 'Redo', icon: 'redo', shortcut: 'Ctrl+Y', disabled: !ed.doc.canRedo, action: () => this.runCommand('REDO') }, { sep: true },
        { label: 'Pan', icon: 'pan', action: () => this.runCommand('PAN') }, { label: 'Zoom', icon: 'zoom', action: () => this.runCommand('ZOOM') }, { sep: true },
        { label: 'Select All', shortcut: 'Ctrl+A', action: () => this.runCommand('SELECTALL') }, { label: 'Quick Select...', icon: 'qselect', action: () => this.runCommand('QSELECT') }, { sep: true },
        { label: 'Options...', icon: 'options', action: () => this.runCommand('OPTIONS') },
      ], x, y);
    }
    return showMenu([
      ...(repeat ? [repeat] : []), recent, { sep: true }, clipboard, { sep: true },
      { label: 'Erase', icon: 'erase', shortcut: 'Del', action: () => this.runCommand('ERASE') }, { label: 'Move', icon: 'move', action: () => this.runCommand('MOVE') }, { label: 'Copy Selection', icon: 'copy', action: () => this.runCommand('COPY') },
      { label: 'Scale', icon: 'scale', action: () => this.runCommand('SCALE') }, { label: 'Rotate', icon: 'rotate', action: () => this.runCommand('ROTATE') },
      { label: 'Draw Order', submenu: [{ label: 'Bring to Front', action: () => this.runCommand('TOFRONT') }, { label: 'Send to Back', action: () => this.runCommand('TOBACK') }] },
      { sep: true }, { label: 'Select Similar', action: () => this.runCommand('SELECTSIMILAR') }, { label: 'Deselect All', action: () => { ed.clearSel(); this.app.emit('change'); } }, { label: 'Quick Select...', icon: 'qselect', action: () => this.runCommand('QSELECT') },
      { sep: true }, { label: 'Quick Properties', action: () => this.togglePalette('quick', true) }, { label: 'Properties', icon: 'properties', shortcut: 'Ctrl+1', action: () => this.togglePalette('properties', true) },
    ], x, y);
  }

  osnapItems() {
    const rt = this.app.runtime, send = c => () => { if (rt.pending && ['point', 'distance', 'angle'].includes(rt.pending.kind)) rt.text(c); };
    const mk = (k, label = OS_FULL[k]) => ({ label, action: send(k) });
    return [
      { label: 'Temporary track point', disabled: true }, { label: 'From', action: send('FROM') }, { sep: true },
      mk('END'), mk('MID'), mk('INT'), mk('CEN'), mk('QUA'), mk('TAN'), mk('PER'), mk('INS'), mk('NOD'), mk('NEA'), { sep: true },
      { label: 'None', action: send('NON') }, { label: 'Osnap Settings...', action: () => this.dialog('dsettings', { tab: 'osnap' }) },
    ];
  }
  osnapMenu(x, y) { showMenu(this.osnapItems(), x, y); }

  // ------------------------------------------------------------ keyboard

  bindKeys() {
    const rt = this.app.runtime;
    document.addEventListener('keydown', e => this.onKey(e), true);
    document.addEventListener('keyup', e => { if (e.key === 'Shift' && rt.shiftOrtho) { rt.shiftOrtho = false; this.app.emit('overlay'); this.statusbar.refresh(); } });
    window.addEventListener('blur', () => { rt.shiftOrtho = false; });
  }

  onKey(e) {
    const app = this.app, rt = app.runtime;
    const t = e.target, typing = t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement || t instanceof HTMLSelectElement || t?.isContentEditable;
    const inCmd = t === this.cmdline.input;
    const modal = !!this.dialogs.top();
    if (modal) { if (e.key === 'Escape') { e.preventDefault(); this.dialogs.cancelTop(); } return; }
    const k = e.key, ctrl = e.ctrlKey || e.metaKey;
    const act = fn => { e.preventDefault(); e.stopPropagation(); fn(); };
    // function keys always work
    const F = { F1: () => this.runCommand('HELP'), F2: () => this.textWindow.toggle(), F3: () => this.toggleSv('OSMODE', 16384, true), F7: () => this.toggleSv('GRIDMODE'), F8: () => this.toggleOrtho(), F9: () => this.toggleSv('SNAPMODE'), F10: () => this.togglePolar(), F11: () => this.toggleSv('OTRACKMODE'), F12: () => this.toggleDyn() };
    if (F[k] && !ctrl) return act(F[k]);
    if (k === 'Shift' && !e.repeat && rt.busy) { rt.shiftOrtho = true; this.app.emit('overlay'); }
    if (k === 'Escape') { if (typing && !inCmd) return; return act(() => this.escape()); }
    if (ctrl) {
      const map = { n: 'NEW', o: 'OPEN', s: e.shiftKey ? 'SAVEAS' : 'QSAVE', p: e.shiftKey ? 'QUICKPROPERTIES' : 'PLOT', z: 'U', y: 'REDO', a: 'SELECTALL', c: 'COPYCLIP', x: 'CUTCLIP', v: 'PASTECLIP', '1': 'PROPERTIES', '2': 'TOOLPALETTES', '3': 'TOOLPALETTES', '0': 'CLEANSCREENON', '9': 'COMMANDLINE' };
      const lk = k.toLowerCase();
      if (typing && ['a', 'c', 'x', 'v', 'z', 'y'].includes(lk) && (!inCmd || t.value)) return;   // normal text editing
      if (map[lk]) return act(() => this.runCommand(map[lk]));
      return;
    }
    if (typing && !inCmd) return;
    if (inCmd) return;                                                      // the command line handles its own keys
    if (e.altKey) return;
    if (k === 'Delete' && !rt.busy && this.app.active?.sel.size) return act(() => this.runCommand('ERASE'));
    if (k === 'Enter' || k === ' ') {
      if (k === ' ' && rt.pending?.kind === 'string') return act(() => this.cmdline.typeFromCanvas(' '));
      return act(() => { if (this.viewport.endRealtime()) return; rt.enter(); this.viewport.invalidate(); });
    }
    if (k.length === 1 && !e.metaKey) return act(() => this.cmdline.typeFromCanvas(k));
    if (k === 'Backspace' && this.cmdline.input.value) return act(() => this.cmdline.focus());
  }

  toggleSv(name, bit, invert) {
    const sv = this.app.sysvars, v = sv.prefs[name];
    sv.set(name, bit ? v ^ bit : (v ? 0 : 1));
    void invert;
    this.viewport.invalidate(); this.statusbar.refresh();
  }
  toggleOrtho() { const sv = this.app.sysvars; sv.set('ORTHOMODE', sv.prefs.ORTHOMODE ? 0 : 1); if (sv.prefs.ORTHOMODE) sv.set('POLARMODE', 0); this.viewport.invalidate(); this.statusbar.refresh(); }
  togglePolar() { const sv = this.app.sysvars; sv.set('POLARMODE', sv.prefs.POLARMODE ? 0 : 1); if (sv.prefs.POLARMODE) sv.set('ORTHOMODE', 0); this.viewport.invalidate(); this.statusbar.refresh(); }
  toggleDyn() { const sv = this.app.sysvars; const d = sv.prefs.DYNMODE; sv.set('DYNMODE', d > 0 ? -d : (Math.abs(d) || 3)); this.viewport.invalidate(); this.statusbar.refresh(); }
}

void svgIcon; void registry; void OS_BITS; void LINETYPES;
