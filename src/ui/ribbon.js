// The ribbon: tabs, panels (with slideouts and launchers), large/small/split buttons and combo controls.
import { h, svgIcon, clear } from './dom.js';
import { showMenu, dropdown, attachTip, closeMenus } from './menu.js';
import { RIBBON, HATCH_TAB, HATCHEDIT_TAB } from './ribbon-data.js';
import { registry } from '../engine/registry.js';
import { PATTERNS, hatchLines } from '../core/hatch.js';
import { colorToCss, aciHex, LINETYPES, LINEWEIGHTS, lwDisplay, describeColor, ltypeKey } from '../core/style.js';
import * as g from '../core/geom.js';

const aliasOf = name => { let best = null; for (const [a, n] of registry.aliases) if (n === name && (!best || a.length < best.length)) best = a; return best; };
const plain = s => String(s).replace(/\n/g, ' ');

export class Ribbon {
  constructor(ui, host) {
    this.ui = ui; this.app = ui.app; this.host = host;
    this.activeId = 'home'; this.mode = 0;            // 0 full, 1 tabs only, 2 panel titles
    this.hidden = new Set(); this.pinned = null; this.flyout = null;
    this.live = [];                                    // live widgets updated by refresh()
    this.render();
    this.app.on('change', () => this.refresh());
    this.app.on('hatchSession', s => this.contextual(s));
    let w = host.clientWidth;
    new ResizeObserver(() => { if (host.clientWidth !== w) { w = host.clientWidth; this.closeFlyout(); this.render(); } }).observe(host);
  }

  get tabs() {
    const t = RIBBON.filter(x => !this.hidden.has(x.id));
    const s = this.app.hatchSession;
    if (s) t.push(s.editing ? HATCHEDIT_TAB : HATCH_TAB);
    return t;
  }

  contextual(session) {
    if (session) { this.prevTab = this.activeId; this.activeId = session.editing ? HATCHEDIT_TAB.id : HATCH_TAB.id; }
    else if ([HATCH_TAB.id, HATCHEDIT_TAB.id].includes(this.activeId)) this.activeId = this.prevTab || 'home';
    this.render();
  }

  render() {
    clear(this.host);
    this.live = [];
    const tabs = this.tabs;
    if (!tabs.some(t => t.id === this.activeId)) this.activeId = tabs[0].id;
    const strip = h('div.rtabs', { role: 'tablist' });
    for (const t of tabs) {
      const el = h('button.rtab' + (t.id === this.activeId ? '.active' : '') + (t.contextual ? '.contextual' : ''), { role: 'tab', 'aria-selected': t.id === this.activeId, onclick: () => this.select(t.id), oncontextmenu: e => { e.preventDefault(); this.tabMenu(e); } }, t.title);
      strip.append(el);
    }
    strip.append(h('div.rtabs-fill'),
      h('button.rmin', { title: ['Minimize to Tabs', 'Minimize to Panel Titles', 'Show Full Ribbon'][this.mode] ?? 'Minimize Ribbon', onclick: () => this.cycleMode() }, svgIcon(this.mode === 0 ? 'chevup' : 'chevdown', 12)));
    this.host.append(strip);
    this.host.dataset.mode = this.mode;
    const tab = tabs.find(t => t.id === this.activeId);
    const body = h('div.rbody');
    const placed = [];
    if (this.mode !== 1 || this.flyout) {
      for (const p of tab.panels) { const el = this.panel(p); if (el) { body.append(el, h('div.rsep')); placed.push({ p, el }); } }
    }
    this.host.append(body);
    this.fit(body, placed);
    this.refresh();
  }

  /** When the panels overflow the window, collapse the right-most ones into drop-down buttons (like AutoCAD). */
  fit(body, placed) {
    if (!body.clientWidth || this.mode === 2) return;
    const order = placed.map((x, i) => ({ ...x, i })).filter(x => x.i >= 2).sort((a, b) => (b.p.prio ?? b.i) - (a.p.prio ?? a.i));
    for (const x of order) { if (body.scrollWidth <= body.clientWidth + 1) break; x.el.replaceWith(this.collapsedPanel(x.p)); }
  }

  collapsedPanel(p) {
    const first = (items) => { for (const it of items) { if (it.t === 'btn') { const m = this.meta(it); if (m) return m.icon; } else if (it.items) { const r = first(it.items); if (r) return r; } } return 'properties'; };
    const btn = h('button.rb.large.collapsed', { 'aria-haspopup': 'true', 'aria-label': `${p.title} panel`, onclick: () => this.flyoutPanel(btn, p) }, svgIcon(first(p.items), 32), h('span.rb-label', p.title, h('br'), svgIcon('chevdown', 8)));
    return h('section.rpanel.collapsed-panel', btn);
  }

  flyoutPanel(anchor, p) {
    closeMenus(); this.closeFlyout();
    const el = this.panel(p); if (!el) return;
    const wrap = h('div.rfly', el); document.getElementById('overlay-root').append(wrap);
    const r = anchor.getBoundingClientRect(); wrap.style.left = Math.max(2, Math.min(r.left, innerWidth - wrap.offsetWidth - 4)) + 'px'; wrap.style.top = r.bottom + 2 + 'px';
    this._fly = wrap; this.refresh();
    this._flyOff = ev => { if (!wrap.contains(ev.target) && !anchor.contains(ev.target)) this.closeFlyout(); else if (ev.target.closest('.rb') && !ev.target.closest('.rpanel-title')) setTimeout(() => this.closeFlyout(), 0); };
    setTimeout(() => document.addEventListener('pointerdown', this._flyOff, true), 0);
  }

  closeFlyout() { if (!this._fly) return; this._fly.remove(); this._fly = null; document.removeEventListener('pointerdown', this._flyOff, true); }

  select(id) {
    closeMenus();
    if (this.mode === 1) { this.flyout = this.flyout === id ? null : id; this.activeId = id; this.render(); this.host.classList.toggle('flyout', !!this.flyout); return; }
    this.activeId = id; this.render();
  }

  cycleMode() { this.mode = (this.mode + 1) % 3; this.flyout = null; this.host.classList.remove('flyout'); this.render(); this.ui.layoutChanged(); }

  tabMenu(e) {
    showMenu([
      { label: 'Show Tabs', submenu: () => RIBBON.map(t => ({ label: t.title, checked: !this.hidden.has(t.id), action: () => { if (this.hidden.has(t.id)) this.hidden.delete(t.id); else if (t.id !== this.activeId) this.hidden.add(t.id); this.render(); } })) },
      { label: 'Show Panels', submenu: () => (RIBBON.find(t => t.id === this.activeId)?.panels || []).map(p => ({ label: p.title, checked: true, disabled: true })) },
      { sep: true },
      { label: 'Minimize Ribbon', checked: this.mode !== 0, action: () => { this.mode = this.mode === 0 ? 1 : 0; this.render(); this.ui.layoutChanged(); } },
    ], e.clientX, e.clientY);
  }

  // ------------------------------------------------------------ panels

  panel(p) {
    const items = p.items.map(it => this.item(it)).filter(Boolean);
    if (!items.length) return null;
    const body = h('div.rpanel-body', items);
    const title = h('div.rpanel-title', h('span', p.title),
      p.slideout?.length ? h('span.rpanel-exp', svgIcon('chevdown', 10)) : null,
      p.launcher && registry.commands.has(p.launcher) ? h('button.rlaunch', { title: `${p.title} settings`, onclick: e => { e.stopPropagation(); this.ui.runCommand(p.launcher); } }, svgIcon('launcher', 10)) : null);
    const el = h('section.rpanel', { dataset: { panel: p.title } }, body, title);
    if (p.slideout?.length) {
      title.classList.add('expandable');
      title.addEventListener('click', () => this.slideout(el, p));
    }
    return el;
  }

  slideout(el, p) {
    if (el.classList.contains('open')) { this.closeSlideout(); return; }
    this.closeSlideout();
    const sl = h('div.rslideout', p.slideout.map(it => this.item(it)).filter(Boolean));
    el.append(sl);
    el.classList.add('open');
    const off = ev => { if (!el.contains(ev.target)) this.closeSlideout(); else if (ev.target.closest('.rb') && !ev.target.closest('.rpanel-title') && !this.pinned) setTimeout(() => this.closeSlideout(), 0); };
    this._off = off;
    setTimeout(() => document.addEventListener('pointerdown', off, true), 0);
    this._open = el;
  }

  closeSlideout() {
    if (!this._open) return;
    this._open.classList.remove('open');
    this._open.querySelector('.rslideout')?.remove();
    document.removeEventListener('pointerdown', this._off, true);
    this._open = null;
  }

  // ------------------------------------------------------------ items

  item(it) {
    switch (it.t) {
      case 'btn': return this.button(it);
      case 'stack': { const k = it.items.map(i => this.item(i)).filter(Boolean); return k.length ? h('div.rstack', k) : null; }
      case 'col': { const k = it.items.map(i => this.item(i)).filter(Boolean); return k.length ? h('div.rcol', k) : null; }
      case 'row': { const k = it.items.map(i => this.item(i)).filter(Boolean); return k.length ? h('div.rrow', k) : null; }
      case 'combo': return this.combo(it);
      case 'hatchpick': return this.hatchPick();
      case 'patterns': return this.patternGallery();
      case 'hatchprops': return this.hatchProps();
      case 'hatchclose': return this.hatchClose();
      default: return null;
    }
  }

  meta(it) {
    const def = registry.commands.get(it.cmd.toUpperCase());
    return def ? { def, label: it.label ?? def.title ?? def.name, icon: it.icon ?? def.icon ?? def.name.toLowerCase() } : null;
  }

  tipFor(it, m) {
    return () => ({ title: plain(m.label), desc: m.def.desc, cmd: m.def.name.replace(/^-/, '-'), alias: aliasOf(m.def.name) || undefined });
  }

  button(it) {
    const m = this.meta(it);
    if (!m) return null;
    const label = it.size === 'icon' ? null : m.label;
    const run = () => this.ui.runCommand(it.cmd, it.args);
    const face = h('button.rb.' + it.size, { dataset: { cmd: it.cmd.toUpperCase(), args: it.args ? JSON.stringify(it.args) : '' }, 'aria-label': plain(m.label), onclick: run },
      svgIcon(m.icon, it.size === 'large' ? 32 : 16),
      label ? h('span.rb-label', label.split('\n').map((l, i) => i ? [h('br'), l] : l)) : null,
      it.size === 'large' && it.menu ? h('span.rb-caret', svgIcon('chevdown', 8)) : null);
    attachTip(face, this.tipFor(it, m));
    face.addEventListener('contextmenu', e => { e.preventDefault(); showMenu([{ label: 'Add to Quick Access Toolbar', action: () => this.ui.titlebar?.addQat(it.cmd, it.args, m.icon) }], e.clientX, e.clientY); });
    this.live.push({ el: face, cmd: it.cmd.toUpperCase() });
    if (!it.menu) return face;
    if (it.size === 'large') {
      const wrap = h('div.rsplit.large', face);
      const arrow = h('button.rb-arrow', { 'aria-label': 'More options', onclick: e => { e.stopPropagation(); this.openMenu(wrap, it); } }, svgIcon('chevdown', 9));
      wrap.append(arrow);
      return wrap;
    }
    const wrap = h('div.rsplit', face, h('button.rb-arrow', { 'aria-label': 'More options', onclick: e => { e.stopPropagation(); this.openMenu(wrap, it); } }, svgIcon('chevdown', 9)));
    return wrap;
  }

  menuItems(it) {
    const mk = e => e.sep ? { sep: true } : (() => {
      const def = registry.commands.get(e.cmd.toUpperCase());
      if (!def) return null;
      return { label: e.label ?? def.title, icon: e.icon ?? def.icon, action: () => this.ui.runCommand(e.cmd, e.args), tip: { title: e.label ?? def.title, desc: def.desc, cmd: def.name } };
    })();
    if (it.menu === 'blocks') {
      const names = Object.keys(this.app.doc?.blocks || {});
      return [{ label: 'Insert Block...', icon: 'insert', action: () => this.ui.runCommand('INSERT') }, { sep: true }, ...(names.length ? names.map(n => ({ label: n, icon: 'block', action: () => this.ui.runCommand('INSERT', { name: n }) })) : [{ label: 'No blocks defined', disabled: true }])];
    }
    return it.menu.map(mk).filter(Boolean);
  }

  openMenu(anchor, it) { dropdown(anchor, this.menuItems(it)); }

  // ------------------------------------------------------------ combos

  combo(it) {
    const el = h('button.rcombo.' + it.kind, { onclick: () => this.comboMenu(it.kind, el) });
    const sw = h('span.cswatch'), txt = h('span.ctext'), ic = h('span.cstate');
    el.append(ic, sw, txt, h('span.ccaret', svgIcon('chevdown', 9)));
    const update = () => {
      const st = this.ui.currentProps();
      if (it.kind === 'layer') {
        const l = st.layerObj;
        txt.textContent = st.layer;
        clear(ic); if (l) ic.append(svgIcon(l.on ? 'sun' : 'eyeoff', 14), svgIcon(l.frozen ? 'snow' : 'sun', 0), svgIcon(l.locked ? 'lock' : 'unlock', 13));
        ic.querySelector('svg:nth-child(2)')?.remove();
        sw.style.background = l ? colorToCss(l.color, { dark: true }) : '';
        sw.hidden = !l;
      } else if (it.kind === 'color') {
        txt.textContent = describeColor(st.color); sw.hidden = st.color === undefined && false; sw.style.background = st.color === undefined ? 'linear-gradient(135deg, #888 50%, #bbb 50%)' : st.color === 'BYBLOCK' ? 'repeating-linear-gradient(45deg,#888,#888 3px,#bbb 3px,#bbb 6px)' : colorToCss(st.color); ic.hidden = true;
      } else if (it.kind === 'lweight') {
        txt.textContent = st.lw === undefined ? 'ByLayer' : lwDisplay(st.lw); sw.hidden = true; ic.hidden = false; clear(ic); ic.append(h('span.lwsample', { style: { height: Math.max(1, (st.lwResolved ?? 25) / 100 * 1.6) + 'px' } }));
      } else if (it.kind === 'ltype') {
        txt.textContent = st.ltype === undefined ? 'ByLayer' : st.ltype; sw.hidden = true; ic.hidden = false; clear(ic); ic.append(this.ltSample(st.ltypeResolved || 'Continuous'));
      } else if (it.kind === 'visualstyle') {
        txt.textContent = this.ui.visualStyle(); sw.hidden = true; ic.hidden = true;
      }
    };
    this.live.push({ update, el });
    update();
    return el;
  }

  ltSample(name) {
    const pat = (LINETYPES[name] || LINETYPES.Continuous).pattern;
    const w = 44, svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    svg.setAttribute('width', w); svg.setAttribute('height', 8); svg.classList.add('ltsample');
    const p = document.createElementNS('http://www.w3.org/2000/svg', 'path');
    p.setAttribute('d', `M0 4H${w}`); p.setAttribute('stroke', 'currentColor'); p.setAttribute('stroke-width', '1.4');
    if (pat.length) p.setAttribute('stroke-dasharray', pat.map(v => Math.max(.1, Math.abs(v) * 8)).join(' '));
    svg.append(p);
    return svg;
  }

  comboMenu(kind, anchor) {
    const ui = this.ui, doc = this.app.doc;
    if (!doc) return;
    if (kind === 'layer') return this.layerDropdown(anchor);
    if (kind === 'color') {
      const set = v => ui.setProperty('color', v);
      const cols = [['Red', 1], ['Yellow', 2], ['Green', 3], ['Cyan', 4], ['Blue', 5], ['Magenta', 6], ['White', 7]];
      return dropdown(anchor, [
        { label: 'ByLayer', action: () => set(undefined) }, { label: 'ByBlock', action: () => set('BYBLOCK') }, { sep: true },
        ...cols.map(([n, i]) => ({ label: n, html: `<span class="cswatch" style="background:${aciHex(i)}"></span> ${n}`, action: () => set(i) })),
        { sep: true }, { label: 'Select Color...', action: async () => { const v = await ui.dialog('color', { value: ui.currentProps().color, allowByLayer: true }); if (v !== null && v !== undefined) set(v === 'BYLAYER' ? undefined : v); } },
      ], { minWidth: 160 });
    }
    if (kind === 'lweight') {
      const set = v => ui.setProperty('lw', v);
      return dropdown(anchor, [{ label: 'ByLayer', action: () => set(undefined) }, { label: 'ByBlock', action: () => set(-2) }, { label: 'Default', action: () => set(-3) }, { sep: true },
        ...LINEWEIGHTS.map(w => ({ label: w === 0 ? '0.00 mm' : (w / 100).toFixed(2) + ' mm', html: `<span class="lwsample" style="height:${Math.max(1, w / 100 * 1.6)}px"></span> ${(w / 100).toFixed(2)} mm`, action: () => set(w) }))], { minWidth: 150 });
    }
    if (kind === 'ltype') {
      const set = v => ui.setProperty('ltype', v);
      return dropdown(anchor, [{ label: 'ByLayer', action: () => set(undefined) }, { label: 'ByBlock', action: () => set('BYBLOCK') }, { sep: true },
        ...Object.keys(LINETYPES).map(n => ({ label: n, html: `<span class="ltm">${this.ltSample(n).outerHTML}</span> ${n === 'Continuous' ? n : n[0] + n.slice(1).toLowerCase()}`, action: () => set(n) })), { sep: true }, { label: 'Other...', action: () => ui.runCommand('LINETYPE') }], { minWidth: 190 });
    }
    if (kind === 'visualstyle') {
      return dropdown(anchor, ['2D Wireframe', 'Wireframe', 'Hidden', 'Shaded', 'Shaded with Edges'].map(n => ({ label: n, checked: ui.visualStyle() === n, action: () => ui.setVisualStyle(n) })), { minWidth: 160 });
    }
  }

  layerDropdown(anchor) {
    const ui = this.ui, doc = this.app.doc;
    const items = [...doc.layers].sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true })).map(l => {
      const row = h('div.lrow',
        h('button.lstate', { title: l.on ? 'Turn layer off' : 'Turn layer on', onclick: e => { e.stopPropagation(); ui.layerToggle(l, 'on'); this.layerDropdown(anchor); } }, svgIcon(l.on ? 'sun' : 'eyeoff', 14)),
        h('button.lstate', { title: l.frozen ? 'Thaw layer' : 'Freeze layer', onclick: e => { e.stopPropagation(); ui.layerToggle(l, 'frozen'); this.layerDropdown(anchor); } }, svgIcon(l.frozen ? 'snow' : 'sun', 14)),
        h('button.lstate', { title: l.locked ? 'Unlock layer' : 'Lock layer', onclick: e => { e.stopPropagation(); ui.layerToggle(l, 'locked'); this.layerDropdown(anchor); } }, svgIcon(l.locked ? 'lock' : 'unlock', 14)),
        h('span.cswatch', { style: { background: colorToCss(l.color) } }), h('span.lname' + (l.name === doc.cur.layer ? '.cur' : ''), l.name));
      return { html: row.outerHTML, _row: row, label: l.name, action: () => ui.chooseLayer(l.name) };
    });
    const menu = showMenu(items.map(i => ({ label: i.label, html: '' , action: i.action })), anchor.getBoundingClientRect().left, anchor.getBoundingClientRect().bottom + 1, { minWidth: 230 });
    // replace placeholder contents with live row elements (so toggle buttons work without closing)
    [...menu.querySelectorAll(':scope > .mi')].forEach((el, i) => { const lbl = el.querySelector('.mi-label'); lbl.replaceChildren(items[i]._row); el.querySelector('.mi-check').remove(); el.querySelector('.mi-ico').remove(); });
    return menu;
  }

  // ------------------------------------------------------------ hatch creation widgets

  hatchPick() {
    return h('div.rcol', h('button.rb.large', { onclick: () => this.app.runtime.keyword('Select objects') }, svgIcon('hatch', 32), h('span.rb-label', 'Select'), ),
      );
  }

  patternGallery() {
    const sess = () => this.app.hatchSession;
    const cells = Object.keys(PATTERNS).map(name => {
      const cv = h('canvas.pat-thumb', { width: 64, height: 44 });
      this.drawPattern(cv, name);
      const b = h('button.pat' + ((sess()?.settings.pattern === name) ? '.sel' : ''), { title: `${name} — ${PATTERNS[name].desc}`, dataset: { pattern: name }, onclick: () => { const s = sess(); if (!s) return; s.settings.pattern = name; this.app.runtime._refreshPreview(); this.app.emit('change'); this.render(); } }, cv, h('span.pat-name', name));
      return b;
    });
    return h('div.patgrid', cells);
  }

  drawPattern(cv, name) {
    const ctx = cv.getContext('2d'), w = cv.width, hh = cv.height;
    ctx.fillStyle = '#10151a'; ctx.fillRect(0, 0, w, hh);
    ctx.strokeStyle = '#d9dee4'; ctx.fillStyle = '#d9dee4'; ctx.lineWidth = 1;
    const pat = PATTERNS[name];
    if (pat.solid) { ctx.fillRect(3, 3, w - 6, hh - 6); return; }
    const sc = Math.max(1, .55 * (10 / (pat.families[0].dy || 3))), loop = [[3, 3], [w - 3, 3], [w - 3, hh - 3], [3, hh - 3]].map(([x, y]) => ({ x, y }));
    const lines = hatchLines([loop], name, (.6) * (name === 'BRICK' ? .5 : 1) * (sc > 4 ? 1 : 1) / 2.4 * 1, 0) || [];
    ctx.beginPath(); for (const [x0, y0, x1, y1] of lines) { ctx.moveTo(x0, y0); ctx.lineTo(x1, y1); } ctx.stroke();
    ctx.strokeStyle = '#7f8b98'; ctx.strokeRect(3.5, 3.5, w - 7, hh - 7);
  }

  hatchProps() {
    const s = () => this.app.hatchSession?.settings;
    const apply = () => { this.app.runtime._refreshPreview(); this.app.emit('change'); };
    const num = (label, key, step) => { const inp = h('input', { type: 'number', step, value: s()?.[key] ?? 0, onchange: e => { const v = Number(e.target.value); if (Number.isFinite(v) && s()) { if (key === 'scale' && v <= 0) return; s()[key] = v; apply(); } } }); return h('label.hrow', h('span', label), inp); };
    const colorBtn = h('button.hcolor', { onclick: async () => { const v = await this.ui.dialog('color', { value: s()?.color, allowByLayer: true }); if (v !== null && v !== undefined && s()) { s().color = v === 'BYLAYER' ? undefined : v; apply(); this.render(); } } }, h('span.cswatch', { style: { background: s()?.color === undefined ? 'linear-gradient(135deg,#888 50%,#bbb 50%)' : colorToCss(s().color) } }), h('span', describeColor(s()?.color)));
    return h('div.rcol.hprops', h('label.hrow', h('span', 'Color'), colorBtn), num('Scale', 'scale', 0.1), num('Angle', 'angle', 1));
  }

  hatchClose() { return h('button.rb.large.primary', { onclick: () => this.app.runtime.enter().then(() => this.ui.hatchDone?.()) }, svgIcon('check', 32), h('span.rb-label', h('span', 'Close Hatch'), h('br'), 'Creation')); }

  // ------------------------------------------------------------ refresh

  refresh() {
    const running = this.app.runtime?.ctx?.name;
    const args = this.app.runtime?.ctx?.args;
    for (const w of this.live) {
      if (w.update) w.update();
      else if (w.cmd) {
        const on = running === w.cmd && (!w.el.dataset.args || JSON.stringify(args || {}) === w.el.dataset.args);
        w.el.classList.toggle('active', !!on);
      }
    }
  }
}

void g; void ltypeKey;
