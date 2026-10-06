// File handling for the UI: new/open/save/export, recent documents and download helpers.
import { Drawing } from '../core/drawing.js';
import { sampleDrawing } from '../io/sample.js';
import { toJSON, fromJSON, stripExt, FILE_EXT } from '../io/project.js';
import { readDXF, writeDXF } from '../io/dxf.js';
import { showMenu } from './menu.js';

const RECENT_KEY = 'cf.recent';
const MAX_RECENT = 6, MAX_RECENT_BYTES = 400_000;

function download(name, data, type = 'application/octet-stream') {
  const blob = data instanceof Blob ? data : new Blob([data], { type });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob); a.download = name;
  document.body.append(a); a.click(); a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 4000);
}

function pickFiles(accept, multiple = false) {
  return new Promise(resolve => {
    const input = document.createElement('input');
    input.type = 'file'; input.accept = accept; input.multiple = multiple;
    input.addEventListener('change', () => resolve([...input.files]));
    input.addEventListener('cancel', () => resolve([]));
    input.click();
  });
}

export class Files {
  constructor(ui) { this.ui = ui; this.app = ui.app; }

  // ------------------------------------------------------------ recent

  recent() { try { return JSON.parse(localStorage.getItem(RECENT_KEY) || '[]'); } catch { return []; } }
  remember(doc) {
    try {
      const json = toJSON(doc);
      const list = this.recent().filter(r => r.name !== doc.name);
      list.unshift({ name: doc.name, time: Date.now(), json: json.length < MAX_RECENT_BYTES ? json : null });
      localStorage.setItem(RECENT_KEY, JSON.stringify(list.slice(0, MAX_RECENT)));
    } catch { /* storage unavailable or full */ }
  }
  openRecent(r) {
    if (!r.json) { this.ui.toast(`${r.name} is too large to reopen from history. Use Open to choose the file.`); return; }
    this.adopt(fromJSON(r.json, r.name));
  }

  // ------------------------------------------------------------ new / open

  async newDrawing(opts = {}) {
    this.ui.showStart(false);
    return this.app.newDrawing(opts);
  }

  openSample() { this.adopt(sampleDrawing()); }

  adopt(doc) {
    this.ui.showStart(false);
    const ed = this.app.addDrawing(doc);
    ed.zoomed = false; this.ui.viewport.resize();
    this.app.log(`Opened ${doc.name}.`);
    return ed;
  }

  async open() {
    const files = await pickFiles('.json,.cadforge,.dxf', true);
    for (const f of files) await this.openFile(f);
    return files.length > 0;
  }

  async openFile(file) {
    try {
      const text = await file.text();
      const name = stripExt(file.name);
      const doc = /\.dxf$/i.test(file.name) ? readDXF(text, name) : fromJSON(text, name);
      doc.fileName = file.name; doc.dirty = false;
      this.adopt(doc);
    } catch (e) { this.app.log(`Could not open ${file.name}: ${e.message}`, 'error'); this.ui.toast(`Could not open ${file.name}: ${e.message}`, 6000); }
  }

  async importFile() {
    const [f] = await pickFiles('.json,.cadforge,.dxf'); if (!f) return false;
    try {
      const text = await f.text();
      const src = /\.dxf$/i.test(f.name) ? readDXF(text, stripExt(f.name)) : fromJSON(text, stripExt(f.name));
      const doc = this.app.doc; let n = 0;
      this.ui.edit('Import', d => {
        d.willChange();
        for (const l of src.layers) if (!d.layer(l.name)) d.layers.push({ ...l });
        for (const [k, b] of Object.entries(src.blocks)) if (!d.blocks[k]) d.blocks[k] = b;
        for (const e of src.model.entities) { const c = structuredClone(e); delete c.id; d.add(c); n++; }
        d.touch();
      });
      this.app.log(`Imported ${n} objects from ${f.name}.`);
      this.app.editor.zoomExtents(); this.ui.viewChanged();
      void doc; return true;
    } catch (e) { this.app.log(`Could not import ${f.name}: ${e.message}`, 'error'); return false; }
  }

  async pickText(accept) {
    const [f] = await pickFiles(accept); if (!f) return null;
    return { name: f.name, text: await f.text() };
  }

  // ------------------------------------------------------------ save

  async save() {
    const doc = this.app.doc; if (!doc) return false;
    if (!doc.fileName) return this.saveAs();
    return this.write(doc, doc.fileName);
  }

  async saveAs() {
    const doc = this.app.doc; if (!doc) return false;
    const suggested = (doc.fileName || doc.name) + (doc.fileName ? '' : FILE_EXT);
    const name = await this.ui.dialog('prompt', { title: 'Save Drawing As', label: 'File name (use .dxf for DXF, otherwise CadForge JSON)', value: suggested, ok: 'Save' });
    if (!name) return false;
    let n = name.trim(); if (!/\.(json|dxf|cadforge)$/i.test(n)) n += FILE_EXT;
    return this.write(doc, n);
  }

  async write(doc, fileName) {
    try {
      if (/\.dxf$/i.test(fileName)) download(fileName, writeDXF(doc), 'application/dxf');
      else download(fileName, toJSON(doc), 'application/json');
      doc.fileName = fileName; doc.name = stripExt(fileName); doc.dirty = false;
      this.remember(doc);
      this.app.log(`Saved ${fileName}.`);
      this.app.emit('change'); this.ui.doctabs.render();
      return true;
    } catch (e) { this.app.log(`Could not save: ${e.message}`, 'error'); this.ui.toast(`Could not save: ${e.message}`, 6000); return false; }
  }

  // ------------------------------------------------------------ export

  exportDXF() {
    const doc = this.app.doc; if (!doc) return false;
    try { download(`${doc.name}.dxf`, writeDXF(doc), 'application/dxf'); this.app.log(`Exported ${doc.name}.dxf.`); return true; }
    catch (e) { this.app.log(`DXF export failed: ${e.message}`, 'error'); return false; }
  }

  exportPDF() { return this.ui.dialog('plot', { format: 'pdf' }); }

  exportMenu() {
    const r = this.ui.viewport.el.getBoundingClientRect();
    showMenu([
      { label: 'DXF (*.dxf)', icon: 'dxf', action: () => this.exportDXF() },
      { label: 'PDF (*.pdf)', icon: 'pdf', action: () => this.exportPDF() },
      { label: 'SVG (*.svg)', action: () => this.ui.dialog('plot', { format: 'svg' }) },
      { label: 'PNG image (*.png)', action: () => this.ui.dialog('plot', { format: 'png' }) },
      { label: 'CadForge project (*.json)', action: () => { const d = this.app.doc; download(`${d.name}${FILE_EXT}`, toJSON(d), 'application/json'); } },
    ], r.left + 40, r.top + 40);
  }

  exportEntities(ents) {
    const src = this.app.doc, d = new Drawing('Block');
    d.layers = structuredClone(src.layers); d.settings = structuredClone(src.settings);
    for (const e of ents) { const c = structuredClone(e); delete c.id; d.add(c); }
    download('Block' + FILE_EXT, toJSON(d), 'application/json');
  }
}

export { download };
