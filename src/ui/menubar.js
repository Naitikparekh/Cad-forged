// Classic pull-down menu bar (File, Edit, View, Insert, Format, Tools, Draw, Dimension, Modify, Help).
import { h, clear } from './dom.js';
import { showMenu } from './menu.js';
import { registry } from '../engine/registry.js';

const c = (cmd, label, args, shortcut) => ({ cmd, label, args, shortcut });
const S = { sep: true };

const MENUS = [
  ['File', [c('NEW', 'New', null, 'Ctrl+N'), c('OPEN', 'Open...', null, 'Ctrl+O'), S, c('QSAVE', 'Save', null, 'Ctrl+S'), c('SAVEAS', 'Save As...', null, 'Ctrl+Shift+S'), S, c('IMPORT', 'Import...'), c('DXFOUT', 'Export DXF...'), c('EXPORTPDF', 'Export PDF...'), S, c('PLOT', 'Plot...', null, 'Ctrl+P'), S, c('CLOSE', 'Close'), c('QUIT', 'Exit')]],
  ['Edit', [c('U', 'Undo', null, 'Ctrl+Z'), c('REDO', 'Redo', null, 'Ctrl+Y'), S, c('CUTCLIP', 'Cut', null, 'Ctrl+X'), c('COPYCLIP', 'Copy', null, 'Ctrl+C'), c('PASTECLIP', 'Paste', null, 'Ctrl+V'), S, c('SELECTALL', 'Select All', null, 'Ctrl+A'), c('QSELECT', 'Quick Select...')]],
  ['View', [c('REGEN', 'Regen'), c('REGENALL', 'Regen All'), S, c('ZOOM', 'Zoom Window', { mode: 'window' }), c('ZOOM', 'Zoom Previous', { mode: 'previous' }), c('ZOOM', 'Zoom Extents', { mode: 'extents' }), c('PAN', 'Pan'), S, c('3DORBIT', '3D Orbit'), c('-VIEW', 'Named Views...'), S, c('PROPERTIES', 'Properties', null, 'Ctrl+1'), c('TOOLPALETTES', 'Tool Palettes', null, 'Ctrl+3'), c('COMMANDLINE', 'Command Line', null, 'Ctrl+9'), c('CLEANSCREENON', 'Clean Screen', null, 'Ctrl+0')]],
  ['Insert', [c('INSERT', 'Block...'), c('BLOCK', 'Make Block...'), c('WBLOCK', 'Write Block...'), S, c('IMPORT', 'Import...')]],
  ['Format', [c('LAYER', 'Layer...'), c('COLOR', 'Color...'), c('LINETYPE', 'Linetype...'), c('LWEIGHT', 'Lineweight...'), S, c('STYLE', 'Text Style...'), c('DIMSTYLE', 'Dimension Style...'), c('UNITS', 'Units...')]],
  ['Tools', [c('PROPERTIES', 'Properties'), c('TOOLPALETTES', 'Tool Palettes'), S, c('DSETTINGS', 'Drafting Settings...'), c('ALIASEDIT', 'Edit Aliases...'), c('SCRIPT', 'Run Script...'), S, c('OPTIONS', 'Options...')]],
  ['Draw', [c('LINE', 'Line'), c('XLINE', 'Construction Line'), c('RAY', 'Ray'), c('PLINE', 'Polyline'), c('POLYGON', 'Polygon'), c('RECTANG', 'Rectangle'), S, c('ARC', 'Arc', { mode: '3p' }), c('CIRCLE', 'Circle'), c('DONUT', 'Donut'), c('SPLINE', 'Spline'), c('ELLIPSE', 'Ellipse'), S, c('HATCH', 'Hatch...'), c('BOUNDARY', 'Boundary...'), S, c('POINT', 'Point'), c('MTEXT', 'Multiline Text...'), c('TEXT', 'Single Line Text')]],
  ['Dimension', [c('DIMLINEAR', 'Linear'), c('DIMALIGNED', 'Aligned'), c('DIMANGULAR', 'Angular'), c('DIMRADIUS', 'Radius'), c('DIMDIAMETER', 'Diameter'), S, c('DIMBASELINE', 'Baseline'), c('DIMCONTINUE', 'Continue'), c('MLEADER', 'Multileader'), S, c('DIMSTYLE', 'Dimension Style...')]],
  ['Modify', [c('PROPERTIES', 'Properties'), c('MATCHPROP', 'Match Properties'), S, c('ERASE', 'Erase'), c('COPY', 'Copy'), c('MIRROR', 'Mirror'), c('OFFSET', 'Offset'), c('ARRAYRECT', 'Rectangular Array'), c('ARRAYPOLAR', 'Polar Array'), c('MOVE', 'Move'), c('ROTATE', 'Rotate'), c('SCALE', 'Scale'), c('STRETCH', 'Stretch'), c('LENGTHEN', 'Lengthen'), c('TRIM', 'Trim'), c('EXTEND', 'Extend'), c('BREAK', 'Break'), c('JOIN', 'Join'), c('CHAMFER', 'Chamfer'), c('FILLET', 'Fillet'), c('EXPLODE', 'Explode')]],
  ['Help', [c('HELP', 'Help', null, 'F1'), c('ABOUT', 'About CadForge')]],
];

export class MenuBar {
  constructor(ui, host) { this.ui = ui; this.host = host; this.build(); }

  build() {
    clear(this.host);
    for (const [title, items] of MENUS) {
      const b = h('button.mb', { onclick: () => this.open(b, items), onpointerenter: () => { if (this.host.querySelector('.mb.open')) this.open(b, items); } }, title);
      this.host.append(b);
    }
  }

  open(btn, items) {
    this.host.querySelectorAll('.mb.open').forEach(x => x.classList.remove('open'));
    btn.classList.add('open');
    const r = btn.getBoundingClientRect(), ui = this.ui;
    const m = showMenu(items.map(i => {
      if (i.sep) return i;
      const def = registry.commands.get(i.cmd);
      if (!def) return null;
      return { label: i.label, icon: def.icon, shortcut: i.shortcut, action: () => ui.runCommand(i.cmd, i.args) };
    }).filter(Boolean), r.left, r.bottom);
    const obs = new MutationObserver(() => { if (!m.isConnected) { btn.classList.remove('open'); obs.disconnect(); } });
    obs.observe(m.parentElement, { childList: true });
  }

  sync() { this.host.hidden = !this.ui.app.sysvars.prefs.MENUBAR; }
}
