// Title bar: application menu, Quick Access Toolbar, document title and InfoCenter search.
import { h, svgIcon, clear } from './dom.js';
import { showMenu, dropdown, attachTip } from './menu.js';
import { registry } from '../engine/registry.js';

const DEFAULT_QAT = [
  { cmd: 'NEW', label: 'New' }, { cmd: 'OPEN', label: 'Open' }, { cmd: 'QSAVE', label: 'Save' }, { cmd: 'SAVEAS', label: 'Save As' }, { cmd: 'PLOT', label: 'Plot' },
  { sep: true }, { cmd: 'U', label: 'Undo', menu: 'undo' }, { cmd: 'REDO', label: 'Redo' },
];

export class TitleBar {
  constructor(ui, host) {
    this.ui = ui; this.app = ui.app; this.host = host;
    this.custom = JSON.parse(localStorage.getItem('cf.qat.custom') || '[]');
    this.build();
    this.app.on('change', () => this.refresh());
    this.app.on('active', () => this.refresh());
  }

  build() {
    clear(this.host);
    this.appBtn = h('button.app-btn', { 'aria-label': 'Application menu', title: 'Application menu', onclick: () => this.ui.appMenu.toggle(this.appBtn) }, h('span.logo', svgIcon('cube', 0)));
    this.appBtn.innerHTML = '<svg viewBox="0 0 28 28" width="22" height="22" aria-hidden="true"><path d="M14 2.5 24 8.2v11.6L14 25.5 4 19.8V8.2z" fill="#2b8fe0"/><path d="M14 2.5 24 8.2 14 14 4 8.2z" fill="#5bb8ff"/><path d="M14 14v11.5L4 19.8V8.2z" fill="#1a6fb5"/><path d="M19.4 11.4a6 6 0 1 0 0 5.2" fill="none" stroke="#fff" stroke-width="2.2" stroke-linecap="round"/></svg>';
    this.qat = h('div.qat');
    this.titleEl = h('div.title-text');
    this.search = this.buildSearch();
    this.host.append(this.appBtn, this.qat, this.titleEl, h('div.tb-fill'), this.search.wrap, h('button.tb-btn', { title: 'Help (F1)', 'aria-label': 'Help', onclick: () => this.ui.runCommand('HELP') }, svgIcon('help', 18)));
    this.renderQat();
    this.refresh();
  }

  renderQat() {
    clear(this.qat);
    const items = [...DEFAULT_QAT, ...(this.custom.length ? [{ sep: true }] : []), ...this.custom];
    for (const it of items) {
      if (it.sep) { this.qat.append(h('div.qat-sep')); continue; }
      const def = registry.commands.get(it.cmd); if (!def) continue;
      const b = h('button.qat-btn', { dataset: { cmd: it.cmd }, 'aria-label': it.label || def.title, onclick: () => this.ui.runCommand(it.cmd, it.args), oncontextmenu: e => { e.preventDefault(); showMenu([{ label: 'Remove from Quick Access Toolbar', action: () => this.removeQat(it) }], e.clientX, e.clientY); } }, svgIcon(it.icon || def.icon || it.cmd.toLowerCase(), 18));
      attachTip(b, { title: it.label || def.title, desc: def.desc, cmd: def.name });
      this.qat.append(b);
      if (it.menu === 'undo') this.qat.append(h('button.qat-arrow', { 'aria-label': 'Undo list', onclick: e => this.undoMenu(e) }, svgIcon('chevdown', 8)));
    }
    const ws = h('button.qat-ws', { onclick: e => this.ui.statusbar.workspaceMenu({ currentTarget: e.currentTarget }) }, h('span', this.ui.workspace), svgIcon('chevdown', 9));
    this.qat.append(h('div.qat-sep'), ws, h('button.qat-arrow', { title: 'Customize Quick Access Toolbar', 'aria-label': 'Customize Quick Access Toolbar', onclick: e => this.customMenu(e) }, svgIcon('chevdown', 8)));
    this.wsBtn = ws;
  }

  undoMenu(e) {
    const stack = this.app.doc?.undoStack || [];
    const items = stack.slice(-12).reverse().map((s, i) => ({ label: s.label || 'Edit', action: async () => { for (let k = 0; k <= i; k++) await this.ui.runCommand('U'); } }));
    dropdown(e.currentTarget, items.length ? items : [{ label: 'Nothing to undo', disabled: true }]);
  }

  customMenu(e) {
    showMenu([
      { header: 'Customize Quick Access Toolbar' },
      ...['NEW', 'OPEN', 'QSAVE', 'SAVEAS', 'PLOT', 'U', 'REDO'].map(c => ({ label: registry.commands.get(c)?.title || c, checked: true, disabled: true })),
      { sep: true }, { label: 'More Commands...', action: () => this.ui.toast('Right-click any ribbon button and choose Add to Quick Access Toolbar.') },
      { label: 'Show Menu Bar', checked: !!this.app.sysvars.prefs.MENUBAR, action: () => this.ui.toggleMenubar() },
      { label: 'Show Below the Ribbon', disabled: true },
    ], e.clientX, e.clientY + 6);
  }

  addQat(cmd, args, icon) {
    if (!this.custom.some(c => c.cmd === cmd && JSON.stringify(c.args) === JSON.stringify(args))) this.custom.push({ cmd, args, icon });
    localStorage.setItem('cf.qat.custom', JSON.stringify(this.custom)); this.renderQat();
  }
  removeQat(it) { this.custom = this.custom.filter(c => c !== it && !(c.cmd === it.cmd && JSON.stringify(c.args) === JSON.stringify(it.args))); localStorage.setItem('cf.qat.custom', JSON.stringify(this.custom)); this.renderQat(); }

  refresh() {
    const d = this.app.doc;
    this.titleEl.textContent = d ? `CadForge   ${d.fileName || d.name}${d.dirty ? ' *' : ''}` : 'CadForge';
    document.title = d ? `${d.name}${d.dirty ? ' *' : ''} — CadForge` : 'CadForge';
    for (const b of this.qat.querySelectorAll('[data-cmd="U"]')) b.classList.toggle('disabled', !d?.canUndo);
    for (const b of this.qat.querySelectorAll('[data-cmd="REDO"]')) b.classList.toggle('disabled', !d?.canRedo);
  }

  // ------------------------------------------------------------ InfoCenter search

  buildSearch() {
    const input = h('input.ic-input', { type: 'search', placeholder: 'Type a keyword or phrase', 'aria-label': 'Search commands and help', autocomplete: 'off' });
    const results = h('div.ic-results', { hidden: true });
    const wrap = h('div.infocenter', input, h('span.ic-icon', svgIcon('search', 14)), results);
    const run = () => {
      const q = input.value.trim().toLowerCase();
      if (!q) { results.hidden = true; return; }
      const hits = [...registry.commands.values()].filter(d => !d.name.startsWith('_') && !/^OSNAP_/.test(d.name) && d.title && (d.name.toLowerCase().includes(q) || d.title.toLowerCase().includes(q) || (d.desc || '').toLowerCase().includes(q))).slice(0, 10);
      clear(results);
      results.append(h('div.ic-head', hits.length ? 'Commands' : 'No results'), ...hits.map(d => h('button.ic-hit', { onmousedown: e => { e.preventDefault(); input.value = ''; results.hidden = true; this.ui.runCommand(d.name); } }, svgIcon(d.icon || d.name.toLowerCase(), 18), h('span.ic-name', d.title), h('span.ic-desc', d.desc || ''))));
      results.hidden = false;
    };
    input.addEventListener('input', run);
    input.addEventListener('keydown', e => { if (e.key === 'Escape') { input.value = ''; results.hidden = true; input.blur(); } if (e.key === 'Enter') { const f = results.querySelector('.ic-hit'); f?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, cancelable: true })); } });
    input.addEventListener('blur', () => setTimeout(() => { results.hidden = true; }, 150));
    return { wrap, input };
  }
}

