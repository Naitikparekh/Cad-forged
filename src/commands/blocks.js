// Blocks, groups, purge and rename.
import { M } from '../core/geom.js';
import { E, clone, transformEntity } from '../core/entities.js';
import { registry } from '../engine/registry.js';
import { memory } from './helpers.js';

const def = d => registry.define({ group: 'Block', ...d });
const validName = n => n && !/[<>/\\":;?*|,=`]/.test(n) && n.length <= 255;

def({
  name: 'BLOCK', aliases: ['B'], title: 'Create Block', icon: 'block', desc: 'Creates a block definition from selected objects.', pickfirst: true,
  async run(c, args) {
    let name = args.name;
    if (!name) { const r = await c.string({ prompt: 'Enter block name or [?]' }); name = r.value; }
    if (!name) return;
    if (name === '?') { c.log(Object.keys(c.doc.blocks).join('\n') || 'No blocks defined.'); return; }
    if (!validName(name)) { c.log('Invalid block name.', 'error'); return; }
    if (c.doc.blocks[name]) { const y = await c.keyword({ prompt: `Block "${name}" already exists. Redefine it? [Yes/No]`, options: ['Yes', 'No'], default: 'No' }); if (y.kw !== 'Yes') return; }
    const base = args.base || (await c.point({ prompt: 'Specify insertion base point' })).pt; if (!base) return;
    const sel = args.sel || await c.select({ prompt: 'Select objects' });
    if (!sel.length) return;
    const mode = args.retain || 'convert';
    const inner = sel.map(e => { const k = clone(e); delete k.grp; return k; });
    c.doc.defineBlock(name, base, inner);
    c.doc.willChange();
    if (mode !== 'retain') {
      c.doc.removeAll(sel);
      if (mode === 'convert') c.add(E.insert(name, base));
    }
    c.clearSel();
    c.log(`Block "${name}" defined with ${sel.length} object${sel.length === 1 ? '' : 's'}.`);
  },
});

def({
  name: 'INSERT', aliases: ['I', '-INSERT'], title: 'Insert Block', icon: 'insert', desc: 'Inserts a block or drawing into the current drawing.',
  async run(c, args) {
    const mem = memory(c.app);
    let name = args.name;
    const names = Object.keys(c.doc.blocks);
    if (!name) {
      if (c.app.ui?.dialog) name = await c.app.ui.dialog('insert', { names });
      else name = (await c.string({ prompt: 'Enter block name or [?]', default: names[0] })).value;
    }
    if (!name) return;
    if (name === '?') { c.log(names.join('\n') || 'No blocks defined.'); return; }
    const key = names.find(n => n.toLowerCase() === name.toLowerCase());
    if (!key) { c.log(`Block "${name}" not found.`, 'error'); return; }
    let sx = args.scale ?? mem.insertScale, sy = args.scale ?? mem.insertScale, rot = args.rotation ?? mem.insertRot;
    for (;;) {
      const make = pt => E.insert(key, pt, { sx, sy, rot });
      const r = await c.point({ prompt: 'Specify insertion point', options: ['Basepoint', 'Scale', 'X', 'Y', 'Rotate'], preview: pt => [make(pt)] });
      if (r.kw === 'Scale') { const s = await c.number({ prompt: 'Specify scale factor for XYZ axes', default: sx, nonzero: true }); if (s.value) sx = sy = s.value; continue; }
      if (r.kw === 'X') { const s = await c.number({ prompt: 'Specify X scale factor', default: sx, nonzero: true }); if (s.value) sx = s.value; continue; }
      if (r.kw === 'Y') { const s = await c.number({ prompt: 'Specify Y scale factor', default: sy, nonzero: true }); if (s.value) sy = s.value; continue; }
      if (r.kw === 'Rotate') { const s = await c.angle({ prompt: 'Specify rotation angle', default: rot }); if (s.value !== undefined) rot = s.value; continue; }
      if (r.kw === 'Basepoint') { c.log('Base point is fixed by the block definition.'); continue; }
      if (!r.pt) return;
      mem.insertScale = sx; mem.insertRot = rot;
      c.add(make(r.pt));
      if (args.once !== false && args.multiple !== true) return;
    }
  },
});

def({
  name: 'WBLOCK', aliases: ['W'], title: 'Write Block', icon: 'wblock', desc: 'Writes objects or a block to a new drawing file.', pickfirst: true,
  async run(c) {
    const sel = await c.select({ prompt: 'Select objects' }); if (!sel.length) return;
    const base = (await c.point({ prompt: 'Specify insertion base point' })).pt; if (!base) return;
    const shifted = sel.map(e => { const k = clone(e); transformEntity(k, M.translate(-base.x, -base.y), c.doc); return k; });
    await c.app.ui?.exportEntities?.(c, shifted);
  },
});

let groupSerial = 0;
def({
  name: 'GROUP', aliases: ['G'], title: 'Group', icon: 'group', desc: 'Creates and manages saved sets of objects called groups.', pickfirst: true,
  async run(c) {
    const sel = await c.select({ prompt: 'Select objects or [Name/Description]' }); if (sel.length < 2) { c.log('Select at least two objects to group.', 'error'); return; }
    const id = `*A${++groupSerial}_${c.doc.nextId}`;
    c.doc.change(() => { for (const e of sel) e.grp = id; });
    c.log(`Group *A${groupSerial} has been created.`);
  },
});
def({
  name: 'UNGROUP', aliases: ['UG'], title: 'Ungroup', icon: 'ungroup', desc: 'Dissolves a group.', pickfirst: true,
  async run(c) { const sel = await c.select({ prompt: 'Select a group' }); c.doc.change(() => { for (const e of sel) delete e.grp; }); },
});

def({
  name: 'PURGE', aliases: ['PU'], title: 'Purge', icon: 'purge', desc: 'Removes unused items, such as block definitions and layers, from the drawing.',
  async run(c) {
    const doc = c.doc;
    const usedBlocks = new Set();
    const walk = list => { for (const e of list) if (e.type === 'insert' && !usedBlocks.has(e.name)) { usedBlocks.add(e.name); walk(doc.blocks[e.name]?.entities || []); } };
    walk([...doc.model.entities, ...doc.layouts.flatMap(l => l.entities)]);
    const unusedBlocks = Object.keys(doc.blocks).filter(n => !usedBlocks.has(n));
    const unusedLayers = doc.layers.filter(l => l.name !== '0' && l.name !== doc.cur.layer && !doc.layerUsage(l.name));
    if (!unusedBlocks.length && !unusedLayers.length) { c.log('No unreferenced named objects found to purge.'); return; }
    doc.change(() => { for (const n of unusedBlocks) delete doc.blocks[n]; doc.layers = doc.layers.filter(l => !unusedLayers.includes(l)); });
    for (const n of unusedBlocks) c.log(`Deleting block "${n}".`);
    for (const l of unusedLayers) c.log(`Deleting layer "${l.name}".`);
    c.log(`${unusedBlocks.length + unusedLayers.length} items purged.`);
  },
});

def({
  name: '-RENAME', aliases: ['REN', 'RENAME'], title: 'Rename', icon: 'rename', desc: 'Changes the names assigned to items such as layers and blocks.',
  async run(c) {
    const k = await c.keyword({ prompt: 'Enter object type to rename [Block/LAyer/LType/Style]', options: ['Block', 'LAyer'] , default: 'LAyer' });
    const oldName = (await c.string({ prompt: `Enter old ${k.kw === 'Block' ? 'block' : 'layer'} name` })).value; if (!oldName) return;
    const newName = (await c.string({ prompt: `Enter new ${k.kw === 'Block' ? 'block' : 'layer'} name` })).value; if (!newName) return;
    if (k.kw === 'Block') {
      const doc = c.doc; if (!doc.blocks[oldName]) throw Error('Block not found.'); if (doc.blocks[newName]) throw Error('That name is already in use.');
      doc.change(() => { doc.blocks[newName] = { ...doc.blocks[oldName], name: newName }; delete doc.blocks[oldName]; for (const e of doc.allEntities()) if (e.type === 'insert' && e.name === oldName) e.name = newName; });
    } else c.doc.renameLayer(oldName, newName);
  },
});

