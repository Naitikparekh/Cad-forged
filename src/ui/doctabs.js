// Drawing tabs (file tabs) and the Start page.
import { h, svgIcon, clear } from './dom.js';
import { showMenu } from './menu.js';

export class DocTabs {
  constructor(ui, host) {
    this.ui = ui; this.app = ui.app; this.host = host;
    this.render();
    this.app.on('drawings', () => this.render());
    this.app.on('active', () => this.render());
    this.app.on('change', () => this.refreshDirty());
  }

  render() {
    clear(this.host);
    const start = h('button.dt.start' + (this.ui.showingStart ? '.active' : ''), { title: 'Start', 'aria-label': 'Start', onclick: () => this.ui.showStart(true) }, svgIcon('home', 14));
    this.host.append(start);
    for (const ed of this.app.editors) {
      const active = ed === this.app.active && !this.ui.showingStart;
      const tab = h('div.dt' + (active ? '.active' : ''), { role: 'tab', dataset: { id: ed.doc.name }, onclick: () => { this.ui.showStart(false); this.app.setActive(ed); }, oncontextmenu: e => { e.preventDefault(); this.menu(e, ed); }, onauxclick: e => { if (e.button === 1) this.ui.closeDrawing(ed); } },
        h('span.dt-name', ed.doc.name + (ed.doc.dirty ? '*' : '')), h('button.dt-x', { 'aria-label': `Close ${ed.doc.name}`, onclick: e => { e.stopPropagation(); this.ui.closeDrawing(ed); } }, svgIcon('x', 9)));
      this.host.append(tab);
    }
    this.host.append(h('button.dt.plus', { title: 'New drawing (Ctrl+N)', 'aria-label': 'New drawing', onclick: () => this.ui.files.newDrawing() }, svgIcon('plus', 12)), h('div.dt-fill'));
  }

  refreshDirty() {
    for (const [i, ed] of this.app.editors.entries()) { const el = this.host.querySelectorAll('.dt:not(.start):not(.plus) .dt-name')[i]; if (el) el.textContent = ed.doc.name + (ed.doc.dirty ? '*' : ''); }
  }

  menu(e, ed) {
    showMenu([
      { label: 'New', action: () => this.ui.files.newDrawing() }, { label: 'Open...', action: () => this.ui.files.open() }, { label: 'Save', action: () => { this.app.setActive(ed); this.ui.files.save(); } },
      { sep: true }, { label: 'Close', action: () => this.ui.closeDrawing(ed) },
      { label: 'Close All Other Drawings', action: () => { for (const o of [...this.app.editors]) if (o !== ed) this.ui.closeDrawing(o); } },
    ], e.clientX, e.clientY);
  }
}

export class StartPage {
  constructor(ui, host) {
    this.ui = ui; this.host = host; this.build();
  }
  build() {
    const ui = this.ui, files = ui.files;
    clear(this.host);
    const recent = files.recent();
    this.host.append(h('div.start',
      h('div.start-col',
        h('h1', 'Start'),
        h('div.start-tiles',
          h('button.tile.primary', { onclick: () => { files.newDrawing(); } }, svgIcon('new', 40), h('b', 'Start Drawing'), h('span', 'Metric drawing, 420 × 297')),
          h('button.tile', { onclick: () => files.newDrawing({ template: 'imperial' }) }, svgIcon('new', 40), h('b', 'Imperial Drawing'), h('span', 'Inches')),
          h('button.tile', { onclick: () => files.open() }, svgIcon('open', 40), h('b', 'Open Files'), h('span', 'DXF or CadForge JSON')),
          h('button.tile', { onclick: () => files.openSample() }, svgIcon('box', 40), h('b', 'Sample'), h('span', 'L-bracket'))),
        h('h2', 'Recent Documents'),
        recent.length ? h('div.start-recent', recent.map(r => h('button.rec', { onclick: () => files.openRecent(r) }, svgIcon('dxf', 22), h('div', h('b', r.name), h('span', new Date(r.time).toLocaleString()))))) : h('p.start-empty', 'Drawings you save appear here.')),
      h('div.start-col.side', h('h2', 'Tips'),
        h('ul', h('li', 'Type commands such as ', h('code', 'L'), ' (Line), ', h('code', 'C'), ' (Circle), ', h('code', 'REC'), ' (Rectangle) at the command line.'), h('li', 'Press ', h('kbd', 'Space'), ' or ', h('kbd', 'Enter'), ' to repeat the last command.'), h('li', 'Use ', h('kbd', 'F3'), ' ', h('kbd', 'F8'), ' ', h('kbd', 'F10'), ' to toggle Osnap, Ortho and Polar.'), h('li', 'Type ', h('code', 'ALIASEDIT'), ' to define your own command aliases.')))));
  }
}
