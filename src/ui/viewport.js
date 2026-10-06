// The drawing viewport: canvases, pointer interaction, viewport controls, navigation bar, ViewCube.
import { h, svgIcon, raf } from './dom.js';
import { showMenu, dropdown, attachTip } from './menu.js';
import { renderScene } from '../render/renderer.js';
import { drawOverlay } from '../render/overlay.js';
import { drawSolids } from '../render/solids.js';
import { pickAt } from '../engine/select.js';
import { ViewCube } from './viewcube.js';
import { DynInput } from './dyninput.js';

const DRAG_PX = 5;

export class Viewport {
  constructor(ui, el) {
    this.ui = ui; this.app = ui.app; this.el = el;
    this.base = h('canvas.cv.base'); this.over = h('canvas.cv.over');
    el.append(this.base, this.over);
    this.mouse = null; this.down = null; this.dragRect = null; this.panning = null; this.realtime = null; this.hover = null;
    this.vpctl = h('div.vpctl'); this.navbar = h('div.navbar'); this.cubeHost = h('div.viewcube');
    el.append(this.vpctl, this.cubeHost, this.navbar);
    this.cube = new ViewCube(ui, this.cubeHost);
    this.dyn = new DynInput(ui, el);
    this.buildControls();
    this.sceneDirty = true; this.overlayDirty = true; this.size = { w: 0, h: 0 };
    this.frame = raf(() => this.draw());
    new ResizeObserver(() => this.resize()).observe(el);
    this.bind();
    this.app.on('change', () => { this.sceneDirty = true; this.overlayDirty = true; this.schedule(); });
    this.app.on('overlay', () => { this.overlayDirty = true; this.schedule(); });
    this.app.on('active', () => { this.sceneDirty = true; this.resize(); this.buildControls(); });
  }

  get editor() { return this.ui.editor; }
  canvasFocus() { this.el.focus({ preventScroll: true }); }
  schedule() { this.frame(); }
  invalidate() { this.sceneDirty = true; this.overlayDirty = true; this.schedule(); }

  resize() {
    const r = this.el.getBoundingClientRect(), dpr = window.devicePixelRatio || 1;
    if (r.width < 2 || r.height < 2) return;
    this.size = { w: r.width, h: r.height, dpr };
    for (const c of [this.base, this.over]) { c.width = Math.round(r.width * dpr); c.height = Math.round(r.height * dpr); c.style.width = r.width + 'px'; c.style.height = r.height + 'px'; }
    if (this.editor) {
      const v = this.editor.view; v.w = r.width; v.h = r.height;
      if (!this.editor.zoomed) { this.editor.zoomExtents(.1); if (!this.editor.extents()) { v.target = { x: 0, y: 0, z: 0 }; v.scale = 4; } this.editor.zoomed = true; v.prev = []; }
    }
    this.invalidate();
  }

  // ------------------------------------------------------------ viewport controls ([-][Top][2D Wireframe]), nav bar

  buildControls() {
    const ui = this.ui;
    this.vpctl.replaceChildren(
      h('button', { onclick: e => this.vpMenu(e) }, '[–]'),
      h('button.vp-view', { onclick: e => this.viewMenu(e) }, '[', h('span', this.editor?.view.presetName() === 'Custom View' ? 'Custom View' : (this.editor?.view.presetName() ?? 'Top')), ']'),
      h('button.vp-style', { onclick: e => this.styleMenu(e) }, '[', h('span', ui.visualStyle()), ']'));
    this.navbar.replaceChildren(
      this.navBtn('pan', 'Pan', 'Pans the view in the current viewport.', () => ui.runCommand('PAN')),
      this.navBtn('zoomextents', 'Zoom Extents', 'Zooms to display the extents of all objects.', () => ui.runCommand('ZOOM', { mode: 'extents' }), [
        { label: 'Zoom Extents', icon: 'zoomextents', action: () => ui.runCommand('ZOOM', { mode: 'extents' }) }, { label: 'Zoom Window', icon: 'zoomwindow', action: () => ui.runCommand('ZOOM', { mode: 'window' }) },
        { label: 'Zoom Previous', icon: 'zoomprev', action: () => ui.runCommand('ZOOM', { mode: 'previous' }) }, { label: 'Zoom In', icon: 'zoomin', action: () => ui.runCommand('ZOOM', { mode: 'in' }) }, { label: 'Zoom Out', icon: 'zoomout', action: () => ui.runCommand('ZOOM', { mode: 'out' }) }]),
      this.navBtn('orbit', 'Orbit', 'Rotates the view in 3D space.', () => ui.runCommand('3DORBIT')));
    this.cubeHost.hidden = !this.app.sysvars.prefs.NAVVCUBE;
    this.navbar.hidden = !this.app.sysvars.prefs.NAVBAR;
    this.vpctl.hidden = !this.app.sysvars.prefs.VPCONTROL;
    this.cube.draw();
  }

  navBtn(icon, title, desc, action, menu) {
    const b = h('button.nb', { onclick: action }, svgIcon(icon, 18));
    attachTip(b, { title, desc });
    if (!menu) return b;
    const wrap = h('div.nb-split', b, h('button.nb-arrow', { onclick: e => { e.stopPropagation(); showMenu(menu, wrap.getBoundingClientRect().left - 150, wrap.getBoundingClientRect().top); } }, svgIcon('chevleft', 8)));
    return wrap;
  }

  vpMenu(e) {
    const p = this.app.sysvars.prefs, set = (k, v) => { this.app.sysvars.set(k, v ? 1 : 0); this.buildControls(); };
    dropdown(e.currentTarget, [
      { label: 'Maximize Viewport', disabled: true }, { sep: true }, { header: 'Viewport Configuration List' }, { label: 'Single', checked: true }, { sep: true },
      { label: 'ViewCube', checked: !!p.NAVVCUBE, action: () => set('NAVVCUBE', !p.NAVVCUBE) }, { label: 'Navigation Bar', checked: !!p.NAVBAR, action: () => set('NAVBAR', !p.NAVBAR) }, { label: 'UCS Icon', checked: !!p.UCSICON, action: () => { this.app.sysvars.set('UCSICON', p.UCSICON ? 0 : 1); this.invalidate(); } },
    ]);
  }

  viewMenu(e) {
    const cur = this.editor.view.presetName(), go = n => () => this.cube.preset(n);
    dropdown(e.currentTarget, [
      ...[['Top', 'top'], ['Bottom', 'bottom'], ['Left', 'left'], ['Right', 'right'], ['Front', 'front'], ['Back', 'back']].map(([l, n]) => ({ label: l, checked: cur === l, action: go(n) })), { sep: true },
      ...[['SW Isometric', 'swiso'], ['SE Isometric', 'seiso'], ['NE Isometric', 'neiso'], ['NW Isometric', 'nwiso']].map(([l, n]) => ({ label: l, checked: cur === l, action: go(n) })), { sep: true },
      { label: 'Parallel', checked: true }, { label: 'Perspective', disabled: true }, { sep: true }, { label: 'ViewCube', checked: !!this.app.sysvars.prefs.NAVVCUBE, action: () => { this.app.sysvars.set('NAVVCUBE', this.app.sysvars.prefs.NAVVCUBE ? 0 : 1); this.buildControls(); } },
    ]);
  }

  styleMenu(e) {
    const ui = this.ui;
    dropdown(e.currentTarget, ['2D Wireframe', 'Wireframe', 'Hidden', 'Shaded', 'Shaded with Edges'].map(n => ({ label: n, checked: ui.visualStyle() === n, action: () => { ui.setVisualStyle(n); this.buildControls(); } })));
  }

  viewChanged() { this.sceneDirty = true; this.overlayDirty = true; this.schedule(); this.buildControlsText(); }
  buildControlsText() {
    const s = this.vpctl.querySelector('.vp-view span'); if (s) s.textContent = this.editor.view.presetName();
    const t = this.vpctl.querySelector('.vp-style span'); if (t) t.textContent = this.ui.visualStyle();
    this.cube.draw();
  }

  // ------------------------------------------------------------ drawing

  draw() {
    const ed = this.editor;
    if (!ed || !this.size.w) return;
    const { w, h: hh, dpr } = this.size, theme = this.ui.theme, prefs = this.app.sysvars.prefs;
    if (this.sceneDirty) {
      const t = this.vpctl.querySelector('.vp-style span'); if (t && t.textContent !== this.ui.visualStyle()) t.textContent = this.ui.visualStyle();
      this.sceneDirty = false;
      ed.view.w = w; ed.view.h = hh;
      renderScene(this.base.getContext('2d'), w, hh, dpr, ed, prefs, theme, { solids: rc => drawSolids(rc, this.ui.visualStyle()) });
    }
    if (this.overlayDirty) {
      this.overlayDirty = false;
      const rt = this.app.runtime, p = rt.pending;
      const kind = !rt.busy ? 'idle' : p?.kind === 'select' ? 'select' : p?.kind === 'entity' ? 'entity' : p ? 'point' : 'none';
      let win = null;
      if (this.dragRect) win = { a: this.dragRect.a, b: this.dragRect.b, crossing: this.dragRect.b.x < this.dragRect.a.x };
      else if (rt.idleWindow && this.mouse) win = { a: rt.idleWindow.a, b: rt.cursor.raw, crossing: rt.cursor.raw.x < rt.idleWindow.a.x };
      else if (p?.kind === 'select' && p.window?.a) win = { a: p.window.a, b: rt.cursor.raw, crossing: p.window.mode ? p.window.mode === 'C' : rt.cursor.raw.x < p.window.a.x };
      drawOverlay(this.over.getContext('2d'), w, hh, dpr, ed, prefs, theme, {
        mouse: this.mouse, cursor: rt.cursor, preview: rt.preview, window: win, hover: this.hover, kind, hotGrip: rt.hotGrip,
        acquired: rt.snapper.acquired, showCross: !!this.mouse && !this.realtime, hideGrips: !!this.realtime,
      });
      this.dyn.update(this.mouse, this.ui.cmdline?.dynamicText());
    }
  }

  // ------------------------------------------------------------ pointer input

  worldAt(e) { const r = this.el.getBoundingClientRect(); return this.editor.view.toWorld(e.clientX - r.left, e.clientY - r.top); }
  screenAt(e) { const r = this.el.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top }; }

  bind() {
    const o = this.over;
    o.addEventListener('pointerdown', e => this.onDown(e));
    o.addEventListener('pointermove', e => this.onMove(e));
    o.addEventListener('pointerup', e => this.onUp(e));
    o.addEventListener('pointerleave', () => { if (!this.down && !this.panning) { this.mouse = null; this.hover = null; this.overlayDirty = true; this.schedule(); } });
    o.addEventListener('pointercancel', () => { this.down = null; this.panning = null; });
    o.addEventListener('wheel', e => this.onWheel(e), { passive: false });
    o.addEventListener('dblclick', e => { if (e.button === 0 && !this.realtime) this.app.runtime.doubleClick(this.worldAt(e)); });
    o.addEventListener('contextmenu', e => e.preventDefault());
    o.addEventListener('auxclick', e => { if (e.button === 1 && e.detail === 2) this.ui.runCommand('ZOOM', { mode: 'extents' }); });
  }

  onDown(e) {
    this.el.focus({ preventScroll: true });
    o_capture(this.over, e);
    const view = this.editor.view, s = this.screenAt(e);
    if (e.button === 1 || (e.button === 0 && this.realtime?.mode === 'pan')) { this.panning = { x: e.clientX, y: e.clientY, orbit: e.shiftKey && e.button === 1 }; return; }
    if (e.button === 0 && this.realtime) { this.panning = { x: e.clientX, y: e.clientY, mode: this.realtime.mode, view: view.state() }; return; }
    if (e.button === 2) { this.down = { x: e.clientX, y: e.clientY, button: 2, shift: e.shiftKey, ctrl: e.ctrlKey }; return; }
    if (e.button !== 0) return;
    this.down = { x: e.clientX, y: e.clientY, s, w: this.worldAt(e), button: 0, shift: e.shiftKey, ctrl: e.ctrlKey, moved: false };
  }

  onMove(e) {
    const rt = this.app.runtime, view = this.editor.view, s = this.screenAt(e);
    this.mouse = s;
    if (this.panning) {
      const p = this.panning;
      if (p.orbit || p.mode === 'orbit') { view.orbit(-(e.clientX - p.x) * .008, (e.clientY - p.y) * .008); p.x = e.clientX; p.y = e.clientY; this.viewChanged(); return; }
      if (p.mode === 'zoom') { const f = Math.exp(-(e.clientY - p.y) * .01); view.zoomAt(this.size.w / 2, this.size.h / 2, f); p.x = e.clientX; p.y = e.clientY; this.viewChanged(); return; }
      view.pan(e.clientX - p.x, e.clientY - p.y); p.x = e.clientX; p.y = e.clientY; this.viewChanged(); return;
    }
    if (!view.canDraft && !this.realtime) { this.overlayDirty = true; this.schedule(); return; }
    const raw = view.toWorld(s.x, s.y);
    if (this.down && this.down.button === 0 && !this.realtime) {
      const d = Math.hypot(e.clientX - this.down.x, e.clientY - this.down.y);
      const p = rt.pending;
      const canDrag = !rt.busy || p?.kind === 'select';
      if (d > DRAG_PX && canDrag) { this.down.moved = true; this.dragRect = { a: this.down.w, b: raw }; }
    }
    rt.move(raw);
    // hover highlight when picking objects
    const p = rt.pending;
    if (!rt.busy || p?.kind === 'select' || p?.kind === 'entity') {
      const e2 = pickAt(this.editor.doc, raw, this.app.sysvars.prefs.PICKBOX * 2 / view.scale + 2 / view.scale);
      this.hover = e2 && !this.dragRect ? e2 : null;
    } else this.hover = null;
    this.overlayDirty = true; this.schedule();
  }

  async onUp(e) {
    const rt = this.app.runtime, view = this.editor.view;
    if (this.panning) { this.panning = null; return; }
    const d = this.down; this.down = null;
    if (!d) return;
    if (d.button === 2) { if (Math.hypot(e.clientX - d.x, e.clientY - d.y) < 4) this.ui.contextMenu(e.clientX, e.clientY, { shift: d.shift, ctrl: d.ctrl }); return; }
    if (!view.canDraft) { this.ui.toast('The drawing plane is edge-on in this view. Switch to Top or an isometric view to draw.'); return; }
    if (this.dragRect) {
      const { a, b } = this.dragRect; this.dragRect = null;
      await rt.drag(a, b, { shift: d.shift });
    } else if (!this.realtime) {
      await rt.click(this.worldAt(e), { shift: d.shift, ctrl: d.ctrl, screen: this.screenAt(e) });
    }
    this.overlayDirty = true; this.sceneDirty = true; this.schedule();
  }

  onWheel(e) {
    e.preventDefault();
    const view = this.editor.view, s = this.screenAt(e);
    if (e.shiftKey) { view.pan(0, -e.deltaY); } else if (e.ctrlKey) { view.pan(-e.deltaY, 0); }
    else { const f = Math.pow(1.0015, -e.deltaY * (e.deltaMode === 1 ? 33 : 1)); view.zoomAt(s.x, s.y, Math.max(.5, Math.min(2, f))); }
    this.mouse = s; this.app.runtime.move(view.toWorld(s.x, s.y));
    this.viewChanged();
  }

  /** ZOOM/PAN/ORBIT realtime: drag with the primary button until Esc, Enter or right click. */
  startRealtime(mode) {
    return new Promise(resolve => {
      this.realtime = { mode, resolve };
      this.over.style.cursor = mode === 'pan' ? 'grab' : mode === 'zoom' ? 'zoom-in' : 'move';
      this.ui.toast(`Realtime ${mode}: drag to ${mode}. Press Esc, Enter or right-click to exit.`);
      this.overlayDirty = true; this.schedule();
    });
  }
  endRealtime() { if (!this.realtime) return false; const r = this.realtime; this.realtime = null; this.over.style.cursor = ''; r.resolve(); this.invalidate(); return true; }
}

function o_capture(el, e) { try { el.setPointerCapture(e.pointerId); } catch { /* ignore */ } }
