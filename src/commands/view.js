// View commands: ZOOM, PAN, REGEN, VIEW, 3DORBIT.
import * as g from '../core/geom.js';
import { boundsOf } from '../core/entities.js';
import { registry } from '../engine/registry.js';
import { rectOf } from '../engine/select.js';

const def = d => registry.define({ group: 'View', transparent: true, ...d });

const zoomRect = (c, rect, margin = 0) => { c.view.push(); c.view.fit(rect, margin); c.editor.zoomed = true; c.rt.emit(); };

def({
  name: 'ZOOM', aliases: ['Z'], title: 'Zoom', icon: 'zoom', desc: 'Increases or decreases the magnification of the view in the current viewport.',
  async run(c, args) {
    const view = c.view, ed = c.editor;
    const factor = f => { view.push(); view.zoomAt(view.w / 2, view.h / 2, f); c.rt.emit(); };
    const mode = args.mode;
    const extents = () => { ed.zoomExtents(); c.rt.emit(); };
    if (mode === 'extents' || mode === 'all') return extents();
    if (mode === 'in') return factor(2);
    if (mode === 'out') return factor(.5);
    if (mode === 'previous') { if (!view.popPrevious()) c.log('No previous view.', 'error'); c.rt.emit(); return; }
    let r = null;
    if (!mode) {
      r = await c.point({ prompt: 'Specify corner of window, enter a scale factor (nX or nXP), or', options: ['All', 'Center', 'Dynamic', 'Extents', 'Previous', 'Scale', 'Window', 'Object'], allowEnter: true, acceptNumber: true, noOrtho: true });
      if (r.enter && !r.pt && r.value === undefined && !r.kw) { await c.app.ui?.realtime?.('zoom', c); return; }
    } else r = { kw: mode === 'window' ? 'Window' : mode === 'object' ? 'Object' : mode === 'scale' ? 'Scale' : mode === 'center' ? 'Center' : null };
    if (r.value !== undefined && !r.kw) { applyScale(c, r.value, r.rel); return; }
    if (r.pt) {
      const b = await c.corner({ prompt: 'Specify opposite corner', base: r.pt }); if (!b.pt) return;
      return zoomRect(c, rectOf(r.pt, b.pt));
    }
    switch (r.kw) {
      case 'All': case 'Extents': return extents();
      case 'Dynamic': case 'Window': {
        const a = await c.point({ prompt: 'Specify first corner' }); if (!a.pt) return;
        const b = await c.corner({ prompt: 'Specify opposite corner', base: a.pt }); if (!b.pt) return;
        return zoomRect(c, rectOf(a.pt, b.pt));
      }
      case 'Previous': if (!view.popPrevious()) c.log('No previous view.', 'error'); c.rt.emit(); return;
      case 'Center': {
        const p = await c.point({ prompt: 'Specify center point' }); if (!p.pt) return;
        const h = await c.number({ prompt: 'Enter magnification or height', default: view.h / view.scale, min: 1e-12 }); if (h.value === undefined) return;
        view.push(); view.target = { x: p.pt.x, y: p.pt.y, z: view.target.z }; view.scale = view.h / h.value; c.rt.emit(); return;
      }
      case 'Scale': { const s = await c.number({ prompt: 'Enter a scale factor (nX or nXP)', default: 1, min: 1e-9 }); if (s.value) applyScale(c, s.value, 'x'); return; }
      case 'Object': {
        const sel = await c.select({ prompt: 'Select objects', pickfirst: true }); if (!sel.length) return;
        let b = null; for (const e of sel) b = g.unionBounds(b, boundsOf(e, c.doc));
        if (b) { const pad = Math.max(1e-6, (b.maxX - b.minX + b.maxY - b.minY) * 0.02); zoomRect(c, g.growBounds(b, pad), .04); }
        return;
      }
      default: return;
    }
  },
});

function applyScale(c, n, rel) {
  const view = c.view;
  view.push();
  if (rel) { view.zoomAt(view.w / 2, view.h / 2, n); }
  else { const b = c.editor.extents(); const base = view.clone(); base.fit(b || { minX: 0, minY: 0, maxX: 420, maxY: 297 }, .06); view.scale = base.scale * n; }
  c.rt.emit();
}

def({
  name: 'PAN', aliases: ['P'], title: 'Pan', icon: 'pan', desc: 'Shifts the view in the current viewport.',
  async run(c, args) {
    if (args.dx !== undefined) { c.view.pan(args.dx, args.dy); c.rt.emit(); return; }
    await c.app.ui?.realtime?.('pan', c);
  },
});
def({
  name: '-PAN', title: 'Pan by displacement', icon: 'pan', desc: 'Shifts the view by a specified displacement.',
  async run(c) {
    const a = await c.point({ prompt: 'Specify base point or displacement' }); if (!a.pt) return;
    const b = await c.point({ prompt: 'Specify second point', base: a.pt, allowEnter: true }); if (!b.pt) return;
    const d = g.sub(b.pt, a.pt);
    c.view.target = { x: c.view.target.x - d.x, y: c.view.target.y - d.y, z: c.view.target.z }; c.rt.emit();
  },
});
def({ name: 'REGEN', aliases: ['RE'], title: 'Regen', desc: 'Regenerates the drawing and redraws the current viewport.', async run(c) { c.app.emit('regen'); c.rt.emit(); } });
def({ name: 'REGENALL', aliases: ['REA'], title: 'Regen All', desc: 'Regenerates the drawing and redraws all viewports.', async run(c) { c.app.emit('regen'); c.rt.emit(); } });
def({ name: 'REDRAW', aliases: ['R'], title: 'Redraw', desc: 'Refreshes the display in the current viewport.', async run(c) { c.rt.emit(); } });

def({
  name: '-VIEW', aliases: ['VIEW', 'V'], title: 'Named View', icon: 'view', desc: 'Saves and restores named views.', transparent: false,
  async run(c, args) {
    const doc = c.doc;
    const r = await c.keyword({ prompt: 'Enter an option [?/Delete/Orthographic/Restore/Save/sEttings/Window]', options: ['?', 'Delete', 'Restore', 'Save', 'Window'], allowEnter: true }); if (!r.kw) return;
    if (r.kw === '?') { c.log(Object.keys(doc.views).join('\n') || 'No named views.'); return; }
    if (r.kw === 'Window') return c.rt.run('ZOOM', { args: { mode: 'window' }, transparent: true });
    const n = (await c.string({ prompt: `Enter view name to ${r.kw.toLowerCase()}` })).value; if (!n) return;
    if (r.kw === 'Save') { doc.willChange(); doc.views[n] = c.view.state(); doc.touch(); c.log(`View ${n} saved.`); }
    else if (r.kw === 'Restore') { const v = doc.views[n]; if (!v) { c.log(`View ${n} not found.`, 'error'); return; } c.view.push(); c.view.setState(v); c.rt.emit(); }
    else if (r.kw === 'Delete') { doc.willChange(); delete doc.views[n]; doc.touch(); }
    void args;
  },
});

def({
  name: '3DORBIT', aliases: ['3DO', 'ORBIT'], title: 'Orbit', icon: 'orbit', desc: 'Rotates the view in 3D space, but constrained to horizontal and vertical orbit only.',
  async run(c, args) {
    if (args.preset) { c.view.push(); c.view.setPreset(args.preset); c.view.fitPoints3D?.(c.doc.solids.flatMap(s => s.vertices)); c.rt.emit(); return; }
    if (c.view.isPlan) { c.view.yaw = Math.PI / 4; c.view.pitch = Math.atan(1 / Math.SQRT2); const pts = c.doc.solids.flatMap(s => s.vertices); if (pts.length) c.view.fitPoints3D(pts); c.rt.emit(); }
    await c.app.ui?.realtime?.('orbit', c);
  },
});
