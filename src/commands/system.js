// System commands: UNDO/REDO, SETVAR, drafting toggles, selection helpers, draw order and UI/file delegates.
import { registry } from '../engine/registry.js';
import { selectable } from '../engine/select.js';
import { parseNumber } from '../engine/parse.js';
import { OS_BITS, OS_FULL } from '../engine/parse.js';

const def = d => registry.define({ group: 'System', ...d });

// ---------------------------------------------------------------- undo / redo

def({
  name: 'U', title: 'Undo', icon: 'undo', desc: 'Reverses the most recent action.', pickfirst: false,
  async run(c) {
    const label = c.doc.undo();
    if (label === null) { c.log('Everything has been undone', 'info'); return; }
    c.editor.prune(); c.log(label || 'Undo'); c.rt.emit();
  },
});
def({
  name: 'UNDO', title: 'Undo', icon: 'undo', desc: 'Reverses the effect of commands.', pickfirst: false,
  async run(c) {
    const n = await c.number({ prompt: 'Enter the number of operations to undo or [Auto/Control/BEgin/End/Mark/Back]', default: 1, integer: true, min: 1, options: ['Auto', 'Control', 'BEgin', 'End', 'Mark', 'Back'] });
    if (n.kw) { c.log(`UNDO ${n.kw}: not supported; each command is one undo step.`); return; }
    let done = 0;
    for (let i = 0; i < (n.value ?? 1); i++) { if (c.doc.undo() === null) break; done++; }
    c.editor.prune(); c.log(done < (n.value ?? 1) ? 'Everything has been undone' : `${done} ${done === 1 ? 'command' : 'commands'} undone`); c.rt.emit();
  },
});
def({
  name: 'REDO', title: 'Redo', icon: 'redo', desc: 'Reverses the effects of the previous UNDO or U command.', pickfirst: false,
  async run(c) { const l = c.doc.redo(); if (l === null) c.log('Nothing to redo.', 'error'); else { c.editor.prune(); c.log(l || 'Redo'); } c.rt.emit(); },
});

// ---------------------------------------------------------------- SETVAR

def({
  name: 'SETVAR', aliases: ['SET'], title: 'Set Variable', desc: 'Lists or changes the values of system variables.', pickfirst: false,
  async run(c, args) {
    const sv = c.sv;
    let name = args.name;
    while (!name) {
      const r = await c.string({ prompt: 'Enter variable name or [?]', default: undefined });
      const t = (r.value || '').trim();
      if (!t) return;
      if (t === '?') { const w = (await c.string({ prompt: 'Enter variable(s) to list', default: '*' })).value || '*'; const re = new RegExp('^' + w.replace(/\*/g, '.*').replace(/\?/g, '.') + '$', 'i'); for (const n of sv.names().filter(n => re.test(n))) c.log(`${n.padEnd(20)}${sv.get(n)}${sv.isReadOnly(n) ? '  (read only)' : ''}`); c.app.emit('showHistory'); continue; }
      name = t;
    }
    name = name.toUpperCase();
    if (!sv.has(name)) { c.log(`Unknown variable name ${name}.`, 'error'); return; }
    const cur = sv.get(name);
    if (sv.isReadOnly(name)) { c.log(`${name} = ${cur} (read only)`); return; }
    const r = await c.string({ prompt: `Enter new value for ${name}`, default: String(cur) });
    const text = (r.value ?? '').trim();
    if (!text || text === String(cur)) return;
    const type = sv.type(name);
    const v = type === 'string' ? text : parseNumber(text);
    if (v === null) { c.log('Requires numeric value.', 'error'); return; }
    sv.set(name, v);
    c.rt.emit();
  },
});

// ---------------------------------------------------------------- toggles

const toggle = (name, varName, label, desc, icon) => def({
  name, title: label, icon, desc, transparent: true,
  async run(c, args) {
    const sv = c.sv, cur = !!sv.prefs[varName];
    let next = args.value ?? !cur;
    if (args.value === undefined && args.ask) { const r = await c.keyword({ prompt: `Enter mode [ON/OFF]`, options: ['ON', 'OFF'], default: cur ? 'ON' : 'OFF' }); next = r.kw === 'ON'; }
    sv.set(varName, next ? 1 : 0);
    c.log(`<${label} ${next ? 'on' : 'off'}>`);
    c.rt.emit();
  },
});
toggle('ORTHO', 'ORTHOMODE', 'Ortho', 'Constrains cursor movement to the horizontal or vertical direction.', 'ortho');
toggle('GRID', 'GRIDMODE', 'Grid', 'Displays a grid pattern in the current viewport.', 'grid');
toggle('SNAP', 'SNAPMODE', 'Snap', 'Restricts cursor movement to specified intervals.', 'snap');
toggle('POLAR', 'POLARMODE', 'Polar', 'Restricts cursor movement to specified angle increments.', 'polar');
toggle('OTRACK', 'OTRACKMODE', 'Object Snap Tracking', 'Tracks from acquired object snap points.', 'otrack');
toggle('LWDISPLAY', 'LWDISPLAY', 'Lineweight', 'Displays or hides lineweights.', 'lweight');
toggle('QUICKPROPERTIES', 'QPMODE', 'Quick Properties', 'Displays the Quick Properties panel.', 'qprops');

def({
  name: 'DYN', title: 'Dynamic Input', icon: 'dyn', desc: 'Turns dynamic input on or off.', transparent: true,
  async run(c) { c.sv.set('DYNMODE', c.sv.prefs.DYNMODE > 0 ? -Math.abs(c.sv.prefs.DYNMODE) : Math.abs(c.sv.prefs.DYNMODE) || 3); c.log(`<Dynamic input ${c.sv.prefs.DYNMODE > 0 ? 'on' : 'off'}>`); c.rt.emit(); },
});
def({
  name: 'OSNAPTOGGLE', title: 'Object Snap', icon: 'osnap', desc: 'Turns running object snaps on or off.', transparent: true, repeatable: false,
  async run(c) { c.sv.set('OSMODE', c.sv.prefs.OSMODE ^ 16384); c.log(`<Object Snap ${c.sv.osnapOn ? 'on' : 'off'}>`); c.rt.emit(); },
});
def({
  name: 'OSNAP', aliases: ['OS'], title: 'Object Snap Settings', icon: 'osnap', desc: 'Sets running object snap modes.', transparent: true,
  async run(c) { if (c.app.ui?.dialog) await c.app.ui.dialog('dsettings', { tab: 'osnap' }); else c.log(`OSMODE = ${c.sv.prefs.OSMODE}`); },
});
for (const [k, bit] of Object.entries(OS_BITS)) {
  if (['QUI', 'APP'].includes(k)) continue;
  def({ name: `OSNAP_${k}`, title: OS_FULL[k], icon: 'osnap', desc: `Toggles the ${OS_FULL[k]} running object snap.`, transparent: true, repeatable: false, silent: true, async run(c) { c.sv.set('OSMODE', c.sv.prefs.OSMODE ^ bit); c.rt.emit(); } });
}

// ---------------------------------------------------------------- selection helpers

def({
  name: 'SELECTALL', title: 'Select All', desc: 'Selects all objects except those on locked or frozen layers.', repeatable: false, transparent: false,
  async run(c) { const list = c.doc.entities.filter(e => selectable(c.doc, e)); c.editor.sel = new Set(list.map(e => e.id)); c.log(`${list.length} found`); c.rt.emit(); },
});
def({
  name: 'SELECTSIMILAR', aliases: ['SELSIM'], title: 'Select Similar', desc: 'Adds similar objects to the selection set based on selected objects.',
  async run(c) {
    const sel = await c.select({ prompt: 'Select objects' }); if (!sel.length) return;
    const types = new Set(sel.map(e => e.type)), layers = new Set(sel.map(e => e.layer));
    const list = c.doc.entities.filter(e => selectable(c.doc, e) && types.has(e.type) && layers.has(e.layer));
    c.editor.sel = new Set(list.map(e => e.id)); c.log(`${list.length} found`);
  },
});
def({
  name: 'DRAWORDER', aliases: ['DR'], title: 'Draw Order', icon: 'draworder', desc: 'Changes the draw order of images and other objects.', pickfirst: true,
  async run(c, args) {
    const sel = await c.select({ prompt: 'Select objects' }); if (!sel.length) return;
    let where = args.where;
    if (!where) { const k = await c.keyword({ prompt: 'Enter object ordering option [Above objects/Under objects/Front/Back]', options: ['Above objects', 'Under objects', 'Front', 'Back'], default: 'Back' }); where = k.kw; }
    c.doc.change(() => {
      const arr = c.doc.entities, set = new Set(sel), moved = arr.filter(e => set.has(e)), rest = arr.filter(e => !set.has(e));
      if (where === 'Front') c.doc.entities.splice(0, arr.length, ...rest, ...moved); else c.doc.entities.splice(0, arr.length, ...moved, ...rest);
    });
  },
});
def({ name: 'TOFRONT', title: 'Bring to Front', icon: 'draworder', desc: 'Brings objects to the front.', async run(c) { return c.rt.run('DRAWORDER', { args: { where: 'Front' }, transparent: true }); } });
def({ name: 'TOBACK', title: 'Send to Back', icon: 'draworder', desc: 'Sends objects to the back.', async run(c) { return c.rt.run('DRAWORDER', { args: { where: 'Back' }, transparent: true }); } });

def({
  name: 'SCRIPT', aliases: ['SCR'], title: 'Run Script', icon: 'script', desc: 'Executes a sequence of commands from a script file.',
  async run(c, args) {
    let lines = args.lines;
    if (!lines) { const f = await c.app.ui?.openText?.('.scr,.txt'); if (!f) { c.log('No script selected.'); return; } lines = f.text.split(/\r?\n/); }
    c.log(`Running script (${lines.length} lines)`);
    // scripts feed the same input channel as the command line
    Promise.resolve().then(() => c.rt.script(lines));
  },
});

// ---------------------------------------------------------------- delegates to the UI / file layer

const ui = (name, method, extra = {}) => def({
  name, silent: false, ...extra,
  async run(c, args) {
    const fn = c.app.ui?.[method];
    if (!fn) { c.log(`${name} is not available in this environment.`, 'error'); return; }
    await fn.call(c.app.ui, c, args);
  },
});
const palette = (name, pal, extra) => def({ name, ...extra, async run(c, args) { c.app.ui?.togglePalette?.(pal, args.show); } });

palette('PROPERTIES', 'properties', { aliases: ['PR', 'MO', 'CH', 'PROPS', 'DDMODIFY'], title: 'Properties', icon: 'properties', desc: 'Controls properties of existing objects.', transparent: true, repeatable: false });
palette('TOOLPALETTES', 'tools', { aliases: ['TP'], title: 'Tool Palettes', icon: 'toolpal', desc: 'Opens the Tool Palettes window.', transparent: true, repeatable: false });
palette('COMMANDLINE', 'command', { title: 'Command Line', desc: 'Displays the command line.', transparent: true, repeatable: false });
ui('CLEANSCREENON', 'cleanScreen', { title: 'Clean Screen', icon: 'cleanscreen', desc: 'Clears the screen of toolbars and dockable windows.', transparent: true, repeatable: false });
for (const [name, method, extra] of [
  ['DSETTINGS', 'dsettings', { aliases: ['DS', 'SE', 'RM'], title: 'Drafting Settings', icon: 'dsettings', desc: 'Sets grid and snap, polar and object snap tracking, object snap modes, dynamic input, and quick properties.' }],
  ['OPTIONS', 'options', { aliases: ['OP', 'PREFERENCES'], title: 'Options', icon: 'options', desc: 'Customizes the program settings.' }],
  ['UNITS', 'units', { aliases: ['UN', 'DDUNITS'], title: 'Units', icon: 'units', desc: 'Controls coordinate and angle display formats and precision.' }],
  ['DIMSTYLE', 'dimstyle', { aliases: ['D', 'DST', 'DDIM'], title: 'Dimension Style', icon: 'dimstyle', desc: 'Creates and modifies dimension styles.' }],
  ['PLOT', 'plot', { aliases: ['PRINT'], title: 'Plot', icon: 'plot', desc: 'Plots a drawing to a plotter, printer, or file.' }],
  ['NEW', 'newDrawing', { aliases: ['QNEW'], title: 'New', icon: 'new', desc: 'Creates a new drawing file.' }],
  ['OPEN', 'open', { title: 'Open', icon: 'open', desc: 'Opens an existing drawing file.' }],
  ['QSAVE', 'save', { aliases: ['SAVE'], title: 'Save', icon: 'save', desc: 'Saves the current drawing.' }],
  ['SAVEAS', 'saveAs', { title: 'Save As', icon: 'saveas', desc: 'Saves a copy of the current drawing under a new file name.' }],
  ['DXFOUT', 'exportDXF', { title: 'Export DXF', icon: 'dxf', desc: 'Writes the drawing as an ASCII DXF file.' }],
  ['EXPORT', 'exportMenu', { aliases: ['EXP'], title: 'Export', icon: 'export', desc: 'Saves the objects in a drawing to a different file format.' }],
  ['EXPORTPDF', 'exportPDF', { title: 'Export to PDF', icon: 'pdf', desc: 'Exports the layout to a PDF file.' }],
  ['IMPORT', 'importFile', { aliases: ['IMP', 'DXFIN'], title: 'Import', icon: 'import', desc: 'Imports files of various formats into the current drawing.' }],
  ['CLOSE', 'closeDrawing', { title: 'Close', icon: 'close', desc: 'Closes the current drawing.' }],
  ['QUIT', 'quit', { aliases: ['EXIT'], title: 'Exit', icon: 'exit', desc: 'Exits the program.' }],
  ['QSELECT', 'qselect', { title: 'Quick Select', icon: 'qselect', desc: 'Creates a selection set based on filtering criteria.' }],
  ['ALIASEDIT', 'aliasEdit', { aliases: ['AE'], title: 'Edit Aliases', icon: 'aliasedit', desc: 'Defines and modifies command aliases.' }],
  ['STYLE', 'textStyle', { aliases: ['ST', 'DDSTYLE'], title: 'Text Style', icon: 'textstyle', desc: 'Creates, modifies, or specifies text styles.' }],
  ['LAYERSTATE', 'layerState', { aliases: ['LAS'], title: 'Layer States Manager', icon: 'layerstate', desc: 'Saves, restores, and manages named layer states.' }],
  ['ABOUT', 'about', { title: 'About CadForge', icon: 'about', desc: 'Displays information about the application.' }],
  ['HELP', 'help', { aliases: ['?'], title: 'Help', icon: 'help', desc: 'Displays command and keyboard help.' }],
  ['MENUBAR', 'menubar', { title: 'Menu Bar', icon: 'menubar', desc: 'Controls the display of the menu bar.' }],
  ['QUICKCALC', 'calc', { aliases: ['QC', 'CAL'], title: 'QuickCalc', icon: 'calc', desc: 'Opens the QuickCalc calculator.' }],
  ['PURGEDIALOG', 'purgeDialog', { title: 'Purge', icon: 'purge', desc: 'Purge unused named objects.' }],
]) ui(name, method, { repeatable: !['QUIT', 'CLOSE', 'NEW', 'OPEN', 'QSAVE', 'SAVEAS'].includes(name), ...extra });
