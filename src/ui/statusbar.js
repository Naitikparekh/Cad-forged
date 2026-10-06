// Status bar: coordinates readout, drafting-aid toggles, units, workspace and customization.
import { h, svgIcon, clear } from './dom.js';
import { showMenu, attachTip } from './menu.js';
import { OS_BITS, OS_FULL } from '../engine/parse.js';
import * as g from '../core/geom.js';
import { fmtNum } from '../core/style.js';

const OSNAP_ORDER = ['END', 'MID', 'CEN', 'NOD', 'QUA', 'INT', 'INS', 'PER', 'TAN', 'NEA'];

export class StatusBar {
  constructor(ui, host) {
    this.ui = ui; this.app = ui.app; this.host = host;
    this.hidden = new Set(JSON.parse(localStorage.getItem('cf.status.hidden') || '[]'));
    this.coordMode = 1;
    this.items = this.defs();
    this.build();
    this.app.on('change', () => this.refresh());
    this.app.on('overlay', () => this.coords());
    this.app.sysvars.onChange(() => this.refresh());
  }

  defs() {
    const sv = this.app.sysvars, pr = sv.prefs, set = (k, v) => { sv.set(k, v); this.ui.viewport.invalidate(); this.refresh(); };
    const tog = (k) => () => set(k, pr[k] ? 0 : 1);
    return [
      { id: 'model', icon: 'viewcube', title: 'Model', desc: 'Model space. Paper space layouts are not available yet.', on: () => true, act: () => {}, label: 'MODEL' },
      { id: 'grid', icon: 'grid', title: 'Display Grid', desc: 'Displays a reference grid in the drawing area (F7).', key: 'F7', on: () => !!pr.GRIDMODE, act: tog('GRIDMODE'), menu: e => this.gridMenu(e) },
      { id: 'snap', icon: 'snap', title: 'Snap Mode', desc: 'Restricts cursor movement to specified intervals (F9).', key: 'F9', on: () => !!pr.SNAPMODE, act: tog('SNAPMODE'), menu: e => this.snapMenu(e) },
      { id: 'dyn', icon: 'dyn', title: 'Dynamic Input', desc: 'Displays command prompts and coordinate input near the cursor (F12).', key: 'F12', on: () => pr.DYNMODE > 0, act: () => set('DYNMODE', pr.DYNMODE > 0 ? -pr.DYNMODE : (Math.abs(pr.DYNMODE) || 3)) },
      { id: 'ortho', icon: 'ortho', title: 'Ortho Mode', desc: 'Restricts cursor movement to horizontal or vertical (F8).', key: 'F8', on: () => !!pr.ORTHOMODE, act: () => { sv.set('ORTHOMODE', pr.ORTHOMODE ? 0 : 1); if (pr.ORTHOMODE) sv.set('POLARMODE', 0); this.ui.viewport.invalidate(); this.refresh(); } },
      { id: 'polar', icon: 'polar', title: 'Polar Tracking', desc: 'Restricts cursor movement to specified angle increments (F10).', key: 'F10', on: () => !!pr.POLARMODE, act: () => { sv.set('POLARMODE', pr.POLARMODE ? 0 : 1); if (pr.POLARMODE) sv.set('ORTHOMODE', 0); this.ui.viewport.invalidate(); this.refresh(); }, menu: e => this.polarMenu(e) },
      { id: 'otrack', icon: 'otrack', title: 'Object Snap Tracking', desc: 'Tracks from acquired object snap points (F11).', key: 'F11', on: () => !!pr.OTRACKMODE, act: tog('OTRACKMODE') },
      { id: 'osnap', icon: 'osnap', title: 'Object Snap', desc: 'Snaps to geometric points on objects (F3).', key: 'F3', on: () => !(pr.OSMODE & 16384), act: () => set('OSMODE', pr.OSMODE ^ 16384), menu: e => this.osnapMenu(e) },
      { id: 'lwt', icon: 'lweight', title: 'Show/Hide Lineweight', desc: 'Displays objects with their assigned lineweights.', on: () => !!pr.LWDISPLAY, act: tog('LWDISPLAY') },
      { id: 'qp', icon: 'qprops', title: 'Quick Properties', desc: 'Displays a Quick Properties panel for selected objects.', on: () => pr.QPMODE === 1, act: () => this.ui.togglePalette('quick') },
      { id: 'units', text: () => 'Decimal', title: 'Drawing Units', desc: 'Sets linear and angular units.', on: () => false, act: e => this.unitsMenu(e), kind: 'text' },
      { id: 'ws', icon: 'gear', title: 'Workspace Switching', desc: 'Switches between workspaces.', on: () => false, act: e => this.workspaceMenu(e) },
      { id: 'clean', icon: 'cleanscreen', title: 'Clean Screen', desc: 'Clears the screen of toolbars and dockable windows (Ctrl+0).', key: 'Ctrl+0', on: () => document.body.classList.contains('clean'), act: () => this.ui.cleanScreen() },
    ];
  }

  build() {
    clear(this.host);
    this.coordEl = h('button.sb-coords', { title: 'Coordinates (click to toggle absolute / relative display)', onclick: () => { this.coordMode = this.coordMode === 1 ? 2 : 1; this.coords(); } });
    this.host.append(this.coordEl, h('div.sb-sep'));
    this.btns = new Map();
    const group = h('div.sb-group');
    for (const it of this.items) {
      if (this.hidden.has(it.id)) continue;
      const b = h('button.sb' + (it.kind === 'text' ? '.text' : ''), { dataset: { id: it.id }, 'aria-pressed': 'false', onclick: e => it.act(e) },
        it.icon ? svgIcon(it.icon, 16) : null, it.kind === 'text' ? h('span.sb-text', it.text()) : null, it.label ? h('span.sb-label', it.label) : null);
      attachTip(b, { title: it.title, desc: it.desc, extra: it.key ? `Shortcut: ${it.key}` : undefined });
      const cell = h('div.sb-cell', b);
      if (it.menu) cell.append(h('button.sb-arrow', { 'aria-label': `${it.title} settings`, onclick: e => { e.stopPropagation(); it.menu(e); } }, svgIcon('chevup', 8)));
      group.append(cell);
      this.btns.set(it.id, b);
    }
    this.host.append(group, h('div.sb-fill'), h('button.sb.cust', { title: 'Customization', 'aria-label': 'Customize status bar', onclick: e => this.customMenu(e) }, svgIcon('menubar', 14)));
    this.refresh();
  }

  customMenu(e) {
    showMenu([
      { header: 'Status bar items' },
      ...this.items.filter(i => i.id !== 'model').map(i => ({ label: i.title, checked: !this.hidden.has(i.id), action: () => { if (this.hidden.has(i.id)) this.hidden.delete(i.id); else this.hidden.add(i.id); localStorage.setItem('cf.status.hidden', JSON.stringify([...this.hidden])); this.build(); } })),
      { sep: true }, { label: 'Coordinates', checked: true, disabled: true },
    ], e.clientX - 200, e.clientY - 330);
  }

  refresh() {
    for (const it of this.items) {
      const b = this.btns.get(it.id); if (!b) continue;
      const on = !!it.on();
      b.classList.toggle('on', on); b.setAttribute('aria-pressed', on);
      if (it.kind === 'text') b.querySelector('.sb-text').textContent = it.text();
    }
    const units = this.btns.get('units')?.querySelector('.sb-text');
    if (units) units.textContent = `Decimal (${this.app.doc?.settings.units.precision ?? 4})`;
    this.coords();
  }

  coords() {
    const rt = this.app.runtime, c = rt.cursor.pt, prec = Math.min(this.app.doc?.settings.units.precision ?? 4, 4);
    const base = rt.pending?.base;
    if (this.coordMode === 2 && base) {
      this.coordEl.textContent = `${fmtNum(g.dist(base, c), prec)} < ${fmtNum(g.deg(g.normAngle(g.angleOf(base, c))), 0)}°, ${fmtNum(0, prec)}`;
    } else this.coordEl.textContent = `${fmtNum(c.x, prec)}, ${fmtNum(c.y, prec)}, ${fmtNum(0, prec)}`;
  }

  // ------------------------------------------------------------ settings menus

  osnapMenu(e) {
    const sv = this.app.sysvars;
    const items = [
      { label: 'Object Snap Settings...', action: () => this.ui.dialog('dsettings', { tab: 'osnap' }) }, { sep: true },
      ...OSNAP_ORDER.map(k => ({ label: OS_FULL[k], checked: !!(sv.prefs.OSMODE & OS_BITS[k]), action: () => { sv.set('OSMODE', sv.prefs.OSMODE ^ OS_BITS[k]); this.refresh(); } })),
      { sep: true }, { label: 'Clear All', action: () => { sv.set('OSMODE', 16384 | (sv.prefs.OSMODE & 16384 ? 0 : 0)); this.refresh(); } },
    ];
    const r = e.currentTarget.getBoundingClientRect(); showMenu(items, r.left - 120, r.top - 330);
  }

  gridMenu(e) {
    const sv = this.app.sysvars, r = e.currentTarget.getBoundingClientRect();
    showMenu([{ label: 'Grid Settings...', action: () => this.ui.dialog('dsettings', { tab: 'snap' }) }, { sep: true }, ...[1, 2.5, 5, 10, 25, 50, 100].map(v => ({ label: `Spacing ${v}`, checked: sv.prefs.GRIDUNIT_X === v, action: () => { sv.set('GRIDUNIT_X', v); sv.set('GRIDUNIT_Y', v); this.ui.viewport.invalidate(); } }))], r.left - 20, r.top - 230);
  }
  snapMenu(e) {
    const sv = this.app.sysvars, r = e.currentTarget.getBoundingClientRect();
    showMenu([{ label: 'Snap Settings...', action: () => this.ui.dialog('dsettings', { tab: 'snap' }) }, { sep: true }, ...[1, 2.5, 5, 10, 25].map(v => ({ label: `Snap spacing ${v}`, checked: sv.prefs.SNAPUNIT_X === v, action: () => { sv.set('SNAPUNIT_X', v); sv.set('SNAPUNIT_Y', v); sv.set('SNAPMODE', 1); this.refresh(); } }))], r.left - 20, r.top - 190);
  }
  polarMenu(e) {
    const sv = this.app.sysvars, r = e.currentTarget.getBoundingClientRect();
    showMenu([{ label: 'Tracking Settings...', action: () => this.ui.dialog('dsettings', { tab: 'polar' }) }, { sep: true }, ...[90, 45, 30, 22.5, 18, 15, 10, 5].map(v => ({ label: `${v}`, checked: sv.prefs.POLARANG === v, action: () => { sv.set('POLARANG', v); sv.set('POLARMODE', 1); sv.set('ORTHOMODE', 0); this.refresh(); } }))], r.left - 20, r.top - 260);
  }
  unitsMenu(e) {
    const doc = this.app.doc, r = e.currentTarget.getBoundingClientRect();
    showMenu([{ header: 'Linear precision' }, ...[0, 1, 2, 3, 4, 5, 6, 7, 8].map(p => ({ label: p === 0 ? '0' : '0.' + '0'.repeat(p), checked: doc.settings.units.precision === p, action: () => { this.app.sysvars.set('LUPREC', p); this.refresh(); } })), { sep: true }, { label: 'Units...', action: () => this.ui.runCommand('UNITS') }], r.left - 20, r.top - 320);
  }
  workspaceMenu(e) {
    const r = e.currentTarget.getBoundingClientRect();
    showMenu(['Drafting & Annotation', '3D Modeling', 'CadForge Classic'].map(n => ({ label: n, checked: this.ui.workspace === n, action: () => this.ui.setWorkspace(n) })), r.left - 40, r.top - 100);
  }
}
