// Layer, color, linetype and lineweight commands.
import { registry } from '../engine/registry.js';
import { LINETYPES, ltypeKey, LINEWEIGHTS, LW_BYLAYER, newLayer, lwDisplay, describeColor } from '../core/style.js';
import { parseColor } from '../engine/sysvars.js';

const def = d => registry.define({ group: 'Layers', ...d });

/** Match layer names against a comma separated list with * and ? wildcards. */
export function matchLayers(doc, spec) {
  const out = new Set();
  for (const part of String(spec).split(',').map(s => s.trim()).filter(Boolean)) {
    const re = new RegExp('^' + part.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*').replace(/\?/g, '.') + '$', 'i');
    for (const l of doc.layers) if (re.test(l.name)) out.add(l);
  }
  return [...out];
}

const layerOfEntities = (c, sel) => [...new Set(sel.map(e => c.doc.layerOf(e)))];

// ---------------------------------------------------------------- LAYER / -LAYER

def({
  name: 'LAYER', aliases: ['LA'], title: 'Layer Properties', icon: 'layers', desc: 'Manages layers and layer properties.',
  async run(c) { if (c.app.ui?.togglePalette) c.app.ui.togglePalette('layers', true); else return c.rt.run('-LAYER'); },
});

def({
  name: '-LAYER', title: 'Layer (command line)', icon: 'layers', desc: 'Manages layers from the command line.',
  async run(c) {
    const doc = c.doc;
    const need = async (prompt, deflt) => (await c.string({ prompt, default: deflt, raw: false })).value;
    for (;;) {
      const r = await c.keyword({ prompt: 'Enter an option [?/Make/Set/New/Rename/ON/OFF/Color/Ltype/LWeight/Plot/Freeze/Thaw/LOck/Unlock/stAte/Description]', options: ['?', 'Make', 'Set', 'New', 'Rename', 'ON', 'OFF', 'Color', 'Ltype', 'LWeight', 'Plot', 'Freeze', 'Thaw', 'LOck', 'Unlock', 'stAte', 'Description'], allowEnter: true });
      if (!r.kw) return;
      try {
        switch (r.kw) {
          case '?': { for (const l of doc.layers) c.log(`${l.name.padEnd(24)} ${l.on ? 'On ' : 'Off'} ${l.frozen ? 'Frozen' : 'Thawed'} ${l.locked ? 'Locked' : 'Unlocked'}  ${describeColor(l.color)}  ${l.ltype}`); break; }
          case 'Make': { const n = await need('Enter name for new layer (becomes the current layer)', doc.cur.layer); if (n) { doc.ensureLayer(n); doc.setCurrentLayer(n); } break; }
          case 'Set': { const n = await need('Enter layer name to make current', doc.cur.layer); if (n) doc.setCurrentLayer(n); break; }
          case 'New': { const n = await need('Enter name list for new layer(s)'); for (const name of (n || '').split(',').map(s => s.trim()).filter(Boolean)) doc.addLayer(name); break; }
          case 'Rename': { const o = await need('Enter old layer name'); const n = await need('Enter new layer name'); if (o && n) doc.renameLayer(o, n); break; }
          case 'Description': { const n = await need('Enter layer name(s)'); const d = (await c.string({ prompt: 'Enter description', raw: true, allowEmpty: true })).value; for (const l of matchLayers(doc, n)) { doc.willChange(); l.desc = d; doc.touch(); } break; }
          case 'Color': { const col = await need('New color [Truecolor/COlorbook]'); const n = await need('Enter name list of layer(s) for color', doc.cur.layer); const v = parseColor(col); doc.willChange(); for (const l of matchLayers(doc, n)) l.color = v === undefined ? 7 : v; doc.touch(); break; }
          case 'Ltype': { const t = await need('Enter loaded linetype name or [?]', 'Continuous'); const k = ltypeKey(t); if (!k) throw Error(`Linetype "${t}" is not loaded.`); const n = await need('Enter name list of layer(s) for linetype', doc.cur.layer); doc.willChange(); for (const l of matchLayers(doc, n)) l.ltype = k; doc.touch(); break; }
          case 'LWeight': { const w = Number(await need('Enter lineweight in millimeters (0.00 to 2.11)')); const hw = Math.round(w * 100); if (!LINEWEIGHTS.includes(hw)) throw Error('Invalid lineweight.'); const n = await need('Enter name list of layer(s) for lineweight', doc.cur.layer); doc.willChange(); for (const l of matchLayers(doc, n)) l.lw = hw; doc.touch(); break; }
          case 'stAte': { const k = await c.keyword({ prompt: 'Enter an option [?/Save/Restore/Delete]', options: ['?', 'Save', 'Restore', 'Delete'] }); doc.layerStates ??= {}; if (k.kw === '?') { c.log(Object.keys(doc.layerStates).join('\n') || 'No layer states.'); break; } const n = await need('Enter layer state name'); if (k.kw === 'Save') { doc.willChange(); doc.layerStates[n] = structuredClone(doc.layers); doc.touch(); } else if (k.kw === 'Restore') { const st = doc.layerStates[n]; if (!st) throw Error('Layer state not found.'); doc.willChange(); for (const s of st) { const l = doc.layer(s.name); if (l) Object.assign(l, s); } doc.touch(); } else delete doc.layerStates[n]; break; }
          default: {
            const verb = r.kw, n = await need(`Enter name list of layer(s) to ${verb}`, doc.cur.layer);
            const ls = matchLayers(doc, n); if (!ls.length) throw Error(`No layer matched "${n}".`);
            doc.willChange();
            for (const l of ls) { if (verb === 'ON') l.on = true; else if (verb === 'OFF') l.on = false; else if (verb === 'Freeze') { if (l.name === doc.cur.layer) throw Error('The current layer cannot be frozen.'); l.frozen = true; } else if (verb === 'Thaw') l.frozen = false; else if (verb === 'LOck') l.locked = true; else if (verb === 'Unlock') l.locked = false; else if (verb === 'Plot') l.plot = !l.plot; }
            doc.touch();
          }
        }
      } catch (e) { c.log(e.message, 'error'); }
    }
  },
});

// ---------------------------------------------------------------- one-shot layer tools

const setLayers = (c, ls, patch) => { c.doc.willChange(); for (const l of ls) Object.assign(l, patch); c.doc.touch(); };

def({
  name: 'LAYMCUR', title: 'Make Current', icon: 'laymcur', desc: "Sets the current layer to that of a selected object.",
  async run(c) { const e = await c.pickEntity('Select object whose layer will become current'); if (!e) return; c.doc.setCurrentLayer(e.layer); c.log(`${e.layer} is now the current layer.`); },
});
def({
  name: 'LAYOFF', title: 'Layer Off', icon: 'layoff', desc: "Turns off the layer of selected objects.",
  async run(c) { const e = await c.pickEntity('Select an object on the layer to be turned off'); if (!e) return; setLayers(c, [c.doc.layerOf(e)], { on: false }); c.editor.prune(); },
});
def({ name: 'LAYON', title: 'Turn All Layers On', icon: 'layon', desc: 'Turns on all layers in the drawing.', async run(c) { setLayers(c, c.doc.layers, { on: true }); } });
def({
  name: 'LAYFRZ', title: 'Layer Freeze', icon: 'layfrz', desc: 'Freezes the layer of selected objects.',
  async run(c) { const e = await c.pickEntity('Select an object on the layer to be frozen'); if (!e) return; const l = c.doc.layerOf(e); if (l.name === c.doc.cur.layer) { c.log('The current layer cannot be frozen.', 'error'); return; } setLayers(c, [l], { frozen: true }); },
});
def({ name: 'LAYTHW', title: 'Thaw All Layers', icon: 'laythw', desc: 'Thaws all layers in the drawing.', async run(c) { setLayers(c, c.doc.layers, { frozen: false }); } });
def({ name: 'LAYLCK', title: 'Layer Lock', icon: 'laylck', desc: 'Locks the layer of a selected object.', async run(c) { const e = await c.pickEntity('Select an object on the layer to be locked'); if (e) setLayers(c, [c.doc.layerOf(e)], { locked: true }); c.editor.prune(); } });
def({ name: 'LAYULK', title: 'Layer Unlock', icon: 'layulk', desc: 'Unlocks the layer of a selected object.', async run(c) { const e = await c.pickEntity('Select an object on the layer to be unlocked'); if (e) setLayers(c, [c.doc.layerOf(e)], { locked: false }); } });
def({
  name: 'LAYISO', title: 'Layer Isolate', icon: 'layiso', desc: 'Hides or locks all layers except those of the selected objects.', pickfirst: true,
  async run(c) {
    const sel = await c.select({ prompt: 'Select objects on the layer(s) to be isolated' });
    const keep = new Set(layerOfEntities(c, sel).map(l => l));
    if (!keep.size) return;
    const doc = c.doc;
    doc.layerIso = { states: doc.layers.map(l => ({ name: l.name, on: l.on, locked: l.locked })) };
    doc.willChange();
    const fade = c.app.sysvars.prefs.LAYLOCKFADECTL > 0;
    for (const l of doc.layers) if (!keep.has(l) && l.name !== doc.cur.layer) { if (c.app.sysvars.prefs.LAYISOMODE === 'off' || !fade) l.on = false; else l.locked = true; }
    doc.touch();
    c.log(`Layer${keep.size === 1 ? '' : 's'} ${[...keep].map(l => l.name).join(', ')} isolated.`);
    c.editor.clearSel();
  },
});
def({
  name: 'LAYUNISO', title: 'Layer Unisolate', icon: 'layuniso', desc: 'Restores layers hidden or locked by LAYISO.',
  async run(c) { const iso = c.doc.layerIso; if (!iso) { c.log('No layers are isolated.'); return; } c.doc.willChange(); for (const s of iso.states) { const l = c.doc.layer(s.name); if (l) { l.on = s.on; l.locked = s.locked; } } c.doc.layerIso = null; c.doc.touch(); },
});
def({
  name: 'LAYCUR', title: 'Change to Current Layer', icon: 'laycur', desc: 'Changes the layer of selected objects to the current layer.', pickfirst: true,
  async run(c) { const sel = await c.select({ prompt: 'Select objects to be changed to the current layer' }); if (!sel.length) return; c.doc.change(() => { for (const e of sel) e.layer = c.doc.cur.layer; }); c.log(`${sel.length} object${sel.length === 1 ? '' : 's'} changed to layer ${c.doc.cur.layer}.`); },
});
def({
  name: 'LAYMCH', aliases: ['LAYMATCH'], title: 'Match Layer', icon: 'laymatch', desc: "Changes the layer of selected objects to match the layer of a destination object.", pickfirst: true,
  async run(c) { const sel = await c.select({ prompt: 'Select objects to be changed' }); if (!sel.length) return; const d = await c.pickEntity('Select object on destination layer'); if (!d) return; c.doc.change(() => { for (const e of sel) e.layer = d.layer; }); },
});
def({
  name: 'LAYDEL', title: 'Layer Delete', icon: 'laydel', desc: 'Deletes the layers of selected objects and the objects on them.',
  async run(c) {
    const e = await c.pickEntity('Select object on the layer to be deleted'); if (!e) return;
    const name = e.layer; if (name === '0' || name === c.doc.cur.layer) { c.log(`Layer ${name} cannot be deleted.`, 'error'); return; }
    const yes = await c.keyword({ prompt: `Delete layer "${name}" and all of its objects? [Yes/No]`, options: ['Yes', 'No'], default: 'No' });
    if (yes.kw !== 'Yes') return;
    c.doc.change(() => { c.doc.entities = c.doc.entities.filter(x => x.layer.toLowerCase() !== name.toLowerCase()); c.doc.layers = c.doc.layers.filter(l => l.name.toLowerCase() !== name.toLowerCase()); });
    c.editor.prune();
  },
});

// ---------------------------------------------------------------- COLOR / LINETYPE / LWEIGHT / LTSCALE

def({
  name: 'COLOR', aliases: ['COL'], title: 'Color', icon: 'color', desc: 'Sets the color for new objects.',
  async run(c, args) {
    if (c.app.ui?.dialog && !args.cmdline) { const v = await c.app.ui.dialog('color', { value: c.doc.cur.color, allowByLayer: true }); if (v !== null && v !== undefined) c.doc.cur.color = v === 'BYLAYER' ? undefined : v; return; }
    const r = await c.string({ prompt: 'Enter default object color [Truecolor/COlorbook]', default: 'BYLAYER' });
    c.doc.cur.color = parseColor(r.value);
  },
});
def({
  name: 'LINETYPE', aliases: ['LT', 'LTYPE'], title: 'Linetype', icon: 'linetype', desc: 'Loads, sets, and modifies linetypes.',
  async run(c) {
    if (c.app.ui?.dialog) { await c.app.ui.dialog('linetype'); return; }
    const t = await c.string({ prompt: 'Enter linetype name', default: 'BYLAYER' });
    const v = t.value; if (/^bylayer$/i.test(v)) c.doc.cur.ltype = undefined; else { const k = ltypeKey(v); if (!k) throw Error(`Linetype "${v}" not found.`); c.doc.cur.ltype = k; }
  },
});
def({
  name: 'LWEIGHT', aliases: ['LW', 'LINEWEIGHT'], title: 'Lineweight', icon: 'lweight', desc: 'Sets the current lineweight, lineweight display options, and lineweight units.',
  async run(c) {
    if (c.app.ui?.dialog) { await c.app.ui.dialog('lweight'); return; }
    const v = await c.number({ prompt: 'Enter lineweight in mm (0.00-2.11), or -1 for ByLayer', default: -1 });
    if (v.value === -1) c.doc.cur.lw = undefined; else { const hw = Math.round(v.value * 100); if (!LINEWEIGHTS.includes(hw)) throw Error('Invalid lineweight.'); c.doc.cur.lw = hw; }
  },
});
def({
  name: 'LTSCALE', aliases: ['LTS'], title: 'Linetype Scale', icon: 'ltscale', desc: 'Sets the global linetype scale factor.',
  async run(c) { const v = await c.number({ prompt: 'Enter new linetype scale factor', default: c.doc.settings.ltscale, min: 1e-9 }); if (v.value) c.setvar('LTSCALE', v.value); },
});

void LINETYPES; void LW_BYLAYER; void newLayer; void lwDisplay;
