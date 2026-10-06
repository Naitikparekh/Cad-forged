// Application menu (the big "A" button): file commands on the left, recent documents on the right.
import { h, svgIcon, clear } from './dom.js';
import { closeMenus } from './menu.js';

export class AppMenu {
  constructor(ui, root) { this.ui = ui; this.root = root; this.el = null; }

  get open() { return !!this.el; }

  toggle(anchor) { if (this.el) this.close(); else this.show(anchor); }

  close() {
    this.el?.remove(); this.el = null;
    document.removeEventListener('pointerdown', this._out, true); document.removeEventListener('keydown', this._key, true);
  }

  show(anchor) {
    closeMenus();
    const ui = this.ui;
    const run = (cmd, args) => () => { this.close(); ui.runCommand(cmd, args); };
    const item = (icon, label, action, sub) => {
      const el = h('div.am-item', { role: 'menuitem', tabindex: 0, onclick: action || (() => {}) }, svgIcon(icon, 24), h('span.am-label', label), sub ? h('span.am-arrow', svgIcon('chevright', 10)) : null);
      if (sub) { el.addEventListener('pointerenter', () => this.sub(el, sub)); el.addEventListener('click', e => { e.stopPropagation(); this.sub(el, sub); }); }
      else el.addEventListener('pointerenter', () => this.sub(null));
      return el;
    };
    const left = h('div.am-left',
      item('new', 'New', run('NEW')), item('open', 'Open', run('OPEN')), item('save', 'Save', run('QSAVE')),
      item('saveas', 'Save As', run('SAVEAS'), [{ icon: 'saveas', label: 'CadForge Drawing (*.json)', action: run('SAVEAS') }, { icon: 'dxf', label: 'DXF Drawing (*.dxf)', action: run('DXFOUT') }]),
      item('import', 'Import', run('IMPORT')),
      item('export', 'Export', null, [{ icon: 'dxf', label: 'DXF', action: run('DXFOUT') }, { icon: 'pdf', label: 'PDF', action: run('EXPORTPDF') }, { icon: 'export', label: 'Other Formats...', action: run('EXPORT') }]),
      item('plot', 'Print', run('PLOT')),
      item('purge', 'Drawing Utilities', null, [{ icon: 'units', label: 'Units...', action: run('UNITS') }, { icon: 'purge', label: 'Purge...', action: run('PURGEDIALOG') }, { icon: 'rename', label: 'Rename...', action: run('-RENAME') }]),
      item('close', 'Close', null, [{ label: 'Current Drawing', action: run('CLOSE') }]));
    this.subEl = h('div.am-sub', { hidden: true });
    const recent = ui.files.recent();
    const right = h('div.am-right', h('div.am-head', 'Recent Documents'),
      recent.length ? recent.map(r => h('button.am-rec', { onclick: () => { this.close(); ui.files.openRecent(r); } }, svgIcon('dxf', 20), h('span', r.name))) : h('div.am-empty', 'No recent documents'), this.subEl);
    const foot = h('div.am-foot', h('button.am-btn', { onclick: run('OPTIONS') }, 'Options'), h('button.am-btn.exit', { onclick: run('QUIT') }, 'Exit CadForge'));
    this.el = h('div.appmenu', { role: 'menu' }, h('div.am-body', left, right), foot);
    this.root.append(this.el);
    const r = anchor.getBoundingClientRect();
    this.el.style.left = Math.max(0, r.left) + 'px'; this.el.style.top = r.bottom + 2 + 'px';
    this._out = e => { if (!this.el?.contains(e.target) && !anchor.contains(e.target)) this.close(); };
    this._key = e => { if (e.key === 'Escape') { e.stopPropagation(); this.close(); } };
    setTimeout(() => { document.addEventListener('pointerdown', this._out, true); document.addEventListener('keydown', this._key, true); }, 0);
  }

  sub(anchor, items) {
    if (!this.subEl) return;
    this.el.querySelectorAll('.am-item.open').forEach(x => x.classList.remove('open'));
    if (!items) { this.subEl.hidden = true; return; }
    anchor.classList.add('open');
    clear(this.subEl);
    this.subEl.append(h('div.am-head', anchor.textContent), ...items.map(i => h('button.am-rec', { onclick: () => i.action() }, i.icon ? svgIcon(i.icon, 20) : null, h('span', i.label))));
    this.subEl.hidden = false;
  }
}
