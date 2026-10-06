// 3D solid commands: BOX, CYLINDER, SPHERE, CONE, WEDGE, EXTRUDE, PRESSPULL, UNION, SUBTRACT, INTERSECT,
// 3DMOVE, MASSPROP, UPDATEEXTRUSION, VSCURRENT, OBJOUT/STLOUT. Solids are triangle meshes (see src/solid).
import * as g from '../core/geom.js';
import { E } from '../core/entities.js';
import { registry } from '../engine/registry.js';
import { findRegion } from '../ops/boundary.js';
import { fmtNum } from '../core/style.js';
import { memory } from './helpers.js';
import * as S from '../solid/mesh.js';
import { booleanAsync } from '../solid/boolean.js';

const def = d => registry.define({ group: '3D', pickfirst: false, ...d });
const solidMem = c => (memory(c.app).solid ??= { radius: 10, height: 20, length: 30, top: 0 });
const prec = c => c.doc.settings.units.precision;
const f = (c, v) => fmtNum(v, prec(c));

const addSolid = (c, mesh) => c.doc.addSolid(mesh);

// ---------------------------------------------------------------- helpers

/** Prompt for solids (clicks and windows); returns the solid objects. */
async function selectSolids(c, prompt) {
  const sel = await c.select({ prompt, solids: true, filter: () => false, pickfirst: true });
  return sel.solids || [];
}

const rectPreview = (a, b) => [E.pline([{ x: a.x, y: a.y }, { x: b.x, y: a.y }, { x: b.x, y: b.y }, { x: a.x, y: b.y }], true)];

/** Two-corner footprint shared by BOX and WEDGE. Returns {x0,y0,w,d} (signed extents) and the base z. */
async function boxFootprint(c, mem) {
  let r = await c.point({ prompt: 'Specify first corner', options: ['Center'] });
  let a, centre = false;
  if (r.kw === 'Center') { centre = true; r = await c.point({ prompt: 'Specify center' }); }
  if (!r.pt) return null;
  a = r.pt;
  for (;;) {
    const o = await c.corner({ prompt: 'Specify other corner', options: ['Cube', 'Length'], base: a, preview: pt => centre ? rectPreview(g.sub(a, g.sub(pt, a)), pt) : undefined });
    if (o.kw === 'Cube') {
      const l = await c.distance({ prompt: 'Specify length', base: a, default: mem.length, nonzero: true }); if (l.value === undefined) return null;
      mem.length = l.value;
      return centre ? { x0: a.x - l.value / 2, y0: a.y - l.value / 2, w: l.value, d: l.value, h: l.value, cube: true } : { x0: a.x, y0: a.y, w: l.value, d: l.value, h: l.value, cube: true };
    }
    if (o.kw === 'Length') {
      const l = await c.distance({ prompt: 'Specify length', base: a, default: mem.length, nonzero: true }); if (l.value === undefined) return null;
      const w = await c.distance({ prompt: 'Specify width', base: a, default: mem.length, nonzero: true }); if (w.value === undefined) return null;
      mem.length = l.value;
      return centre ? { x0: a.x - l.value / 2, y0: a.y - w.value / 2, w: l.value, d: w.value } : { x0: a.x, y0: a.y, w: l.value, d: w.value };
    }
    if (!o.pt) return null;
    const b = o.pt;
    if (centre) { const dx = Math.abs(b.x - a.x), dy = Math.abs(b.y - a.y); return { x0: a.x - dx, y0: a.y - dy, w: 2 * dx, d: 2 * dy }; }
    return { x0: Math.min(a.x, b.x), y0: Math.min(a.y, b.y), w: Math.abs(b.x - a.x), d: Math.abs(b.y - a.y), a, b };
  }
}

async function heightPrompt(c, mem, base, label = 'Specify height') {
  const h = await c.distance({ prompt: label, options: ['2Point'], base, default: mem.height  , nonzero: true });
  if (h.kw === '2Point') { const q = await c.distance({ prompt: 'Specify first point' }); return q.value; }
  return h.value;
}

// ---------------------------------------------------------------- primitives

def({
  name: 'BOX', title: 'Box', icon: 'box', desc: 'Creates a 3D solid box.',
  async run(c) {
    const mem = solidMem(c), fp = await boxFootprint(c, mem); if (!fp) return;
    if (fp.w < 1e-9 || fp.d < 1e-9) throw Error('The box has zero width or depth.');
    let h = fp.h;
    if (h === undefined) { h = await heightPrompt(c, mem, fp.b || { x: fp.x0, y: fp.y0 }); if (h === undefined) return; mem.height = h; }
    addSolid(c, S.boxMesh(fp.x0, fp.y0, h < 0 ? h : 0, fp.w, fp.d, Math.abs(h), 'Box'));
    c.log(`Box ${f(c, fp.w)} x ${f(c, fp.d)} x ${f(c, Math.abs(h))} created.`);
  },
});

def({
  name: 'WEDGE', aliases: ['WE'], title: 'Wedge', icon: 'wedge', desc: 'Creates a 3D solid wedge.',
  async run(c) {
    const mem = solidMem(c), fp = await boxFootprint(c, mem); if (!fp) return;
    if (fp.w < 1e-9 || fp.d < 1e-9) throw Error('The wedge has zero width or depth.');
    let h = fp.h;
    if (h === undefined) { h = await heightPrompt(c, mem, fp.b || { x: fp.x0, y: fp.y0 }); if (h === undefined) return; mem.height = h; }
    addSolid(c, S.wedgeMesh(fp.x0, fp.y0, h < 0 ? h : 0, fp.w, fp.d, Math.abs(h), 'Wedge'));
  },
});

async function radiusPrompt(c, mem, centre, label = 'Specify radius') {
  const r = await c.distance({ prompt: label, options: ['Diameter'], base: centre, default: mem.radius, nonzero: true, min: 0, preview: pt => [E.circle(centre, Math.max(g.dist(centre, pt), 1e-9))] });
  if (r.kw === 'Diameter') { const d = await c.distance({ prompt: 'Specify diameter', base: centre, default: mem.radius * 2, nonzero: true, min: 0 }); return d.value === undefined ? undefined : d.value / 2; }
  return r.value;
}

def({
  name: 'CYLINDER', aliases: ['CYL'], title: 'Cylinder', icon: 'cylinder', desc: 'Creates a 3D solid cylinder with a circular base.',
  async run(c) {
    const mem = solidMem(c);
    const ctr = await c.point({ prompt: 'Specify center point of base' }); if (!ctr.pt) return;
    const r = await radiusPrompt(c, mem, ctr.pt, 'Specify base radius'); if (r === undefined) return;
    mem.radius = r;
    const h = await heightPrompt(c, mem, ctr.pt); if (h === undefined) return; mem.height = h;
    addSolid(c, S.cylinderMesh(ctr.pt.x, ctr.pt.y, h < 0 ? h : 0, r, Math.abs(h), segmentsFor(r), 'Cylinder'));
  },
});

def({
  name: 'CONE', title: 'Cone', icon: 'cone', desc: 'Creates a 3D solid cone or frustum.',
  async run(c) {
    const mem = solidMem(c);
    const ctr = await c.point({ prompt: 'Specify center point of base' }); if (!ctr.pt) return;
    const r = await radiusPrompt(c, mem, ctr.pt, 'Specify base radius'); if (r === undefined) return;
    mem.radius = r;
    let top = 0, h;
    for (;;) {
      const k = await c.distance({ prompt: 'Specify height', options: ['2Point', 'Top radius'], base: ctr.pt, default: mem.height, nonzero: true });
      if (k.kw === 'Top radius') { const t = await c.distance({ prompt: 'Specify top radius', base: ctr.pt, default: 0, min: 0 }); if (t.value !== undefined) top = t.value; continue; }
      if (k.kw === '2Point') { const q = await c.distance({ prompt: 'Specify first point' }); h = q.value; break; }
      h = k.value; break;
    }
    if (h === undefined) return;
    mem.height = h;
    addSolid(c, S.coneMesh(ctr.pt.x, ctr.pt.y, h < 0 ? h : 0, r, top, Math.abs(h), segmentsFor(Math.max(r, top)), top > 0 ? 'Frustum' : 'Cone'));
  },
});

def({
  name: 'SPHERE', aliases: ['SPH'], title: 'Sphere', icon: 'sphere', desc: 'Creates a 3D solid sphere.',
  async run(c) {
    const mem = solidMem(c);
    const ctr = await c.point({ prompt: 'Specify center point' }); if (!ctr.pt) return;
    const r = await radiusPrompt(c, mem, ctr.pt); if (r === undefined) return;
    mem.radius = r;
    addSolid(c, S.sphereMesh(ctr.pt.x, ctr.pt.y, 0, r, 48, 'Sphere'));
  },
});

const segmentsFor = r => Math.max(32, Math.min(96, Math.round(24 + Math.sqrt(Math.max(r, 0)) * 8)));

// ---------------------------------------------------------------- extrusion

const EXTRUDABLE = e => ['pline', 'circle', 'ellipse', 'spline', 'line', 'arc'].includes(e.type);

/** Mesh for one extrusion feature (entities looked up by id so the source outline stays editable). */
export function featureMesh(doc, feature) {
  const ents = feature.profile.map(id => doc.byId(id)).filter(Boolean);
  if (ents.length !== feature.profile.length) throw Error('The source outline of this extrusion was deleted. Draw it again and use EXTRUDE.');
  const pr = S.profileLoops(ents, doc);
  if (pr.error) throw Error(pr.error);
  return S.extrudeLoops(pr.loops, feature.height, { startZ: feature.startZ || 0, taper: feature.taper || 0 });
}

async function applyFeature(cur, mesh, op) {
  if (op === 'new' || !cur) return mesh;
  const r = await booleanAsync(cur, mesh, op === 'add' ? 'union' : 'subtract');
  return r;
}

def({
  name: 'EXTRUDE', aliases: ['EXT'], title: 'Extrude', icon: 'extrude', desc: 'Extends the dimensions of a 2D closed outline into 3D space, creating a solid.',
  async run(c, args) {
    const sel = await c.select({ prompt: 'Select objects to extrude', filter: EXTRUDABLE, pickfirst: true });
    if (!sel.length) return;
    const pr = S.profileLoops(sel, c.doc);
    if (pr.error) throw Error(pr.error);
    const mem = solidMem(c);
    let taper = 0, start = 0, op = args.op || 'new', h;
    for (;;) {
      const k = await c.distance({ prompt: 'Specify height of extrusion', options: ['Taper angle', 'Start elevation', 'Operation'], default: mem.height, nonzero: true, noSnap: false });
      if (k.kw === 'Taper angle') { const t = await c.angle({ prompt: 'Specify angle of taper for extrusion', default: 0 }); if (t.value !== undefined) taper = g.deg(t.value); continue; }
      if (k.kw === 'Start elevation') { const z = await c.number({ prompt: 'Specify start elevation', default: start }); if (z.value !== undefined) start = z.value; continue; }
      if (k.kw === 'Operation') { const o = await c.keyword({ prompt: 'Combine with an existing solid', options: ['New', 'Add', 'Subtract'], default: 'New' }); op = (o.kw || 'New').toLowerCase(); continue; }
      h = k.value; break;
    }
    if (h === undefined) return;
    if (Math.abs(taper) >= 89) throw Error('The taper angle must be between -89 and 89 degrees.');
    mem.height = h;
    const feature = { op, profile: sel.map(e => e.id), height: h, startZ: start, taper };
    const mesh = S.extrudeLoops(pr.loops, h, { startZ: start, taper });
    if (op === 'new') {
      const s = addSolid(c, mesh); s.program = { features: [feature] };
      c.log(`Extruded solid created (volume ${f(c, S.meshVolume(mesh))}).`);
      return;
    }
    const t = await c.select({ prompt: 'Select the solid to modify', solids: true, filter: () => false, pickfirst: false });
    const target = t.solids?.[0]; if (!target) return;
    c.log(op === 'add' ? 'Computing union...' : 'Computing difference...');
    const res = await booleanAsync(target, mesh, op === 'add' ? 'union' : 'subtract');
    const base = target.program ? target.program.base : { vertices: structuredClone(target.vertices), faces: structuredClone(target.faces) };
    replaceSolid(c, target, res, { features: [...(target.program?.features || []), feature], ...(base ? { base } : {}) });
  },
});

function replaceSolid(c, target, mesh, program) {
  c.doc.change(() => {
    if (!mesh.faces.length) { c.doc.solids.splice(c.doc.solids.indexOf(target), 1); c.log('No solid remains.'); return; }
    target.vertices = mesh.vertices; target.faces = mesh.faces;
    if (program) target.program = program;
  });
}

def({
  name: 'UPDATEEXTRUSION', aliases: ['EXTUPDATE'], title: 'Update Extrusion', icon: 'extrude', desc: 'Rebuilds solids from their source outlines after the outlines were edited.',
  async run(c) {
    const sel = (await selectSolids(c, 'Select extruded solids to update')).filter(s => s.program);
    if (!sel.length) { c.log('The selected solids have no extrusion history.', 'error'); return; }
    for (const s of sel) {
      let cur = s.program.base ? { vertices: s.program.base.vertices, faces: s.program.base.faces } : null;
      for (const ft of s.program.features) cur = await applyFeature(cur, featureMesh(c.doc, ft), ft.op);
      replaceSolid(c, s, cur, s.program);
    }
    c.log(`${sel.length} solid${sel.length === 1 ? '' : 's'} updated.`);
    c.clearSel();
  },
});

def({
  name: 'PRESSPULL', aliases: ['PP'], title: 'Presspull', icon: 'presspull', desc: 'Presses or pulls bounded areas into 3D.',
  async run(c) {
    const mem = solidMem(c);
    const r = await c.point({ prompt: 'Click inside bounded areas to press or pull', allowEnter: false }); if (!r.pt) return;
    const reg = findRegion(c.doc, r.pt);
    if (reg.error) throw Error(c.rt.solidAt(c.editor, r.pt, { screen: c.view.toScreen(r.pt) }) ? 'Pressing faces of existing solids is not supported; use EXTRUDE with Add or Subtract.' : reg.error);
    const loops = reg.loops.map(l => tess(l));
    const h = await c.distance({ prompt: 'Specify extrusion height', default: mem.height, nonzero: true, base: r.pt });
    if (h.value === undefined) return;
    mem.height = h.value;
    addSolid(c, S.extrudeLoops(loops, h.value, { name: 'Presspull' }));
  },
});

const tess = loop => { const pts = []; for (const p of g.tessSegs(g.plineSegs(loop, true), 0.02)) if (!pts.length || g.dist(p, pts[pts.length - 1]) > 1e-9) pts.push({ x: p.x, y: p.y }); if (pts.length > 1 && g.dist(pts[0], pts[pts.length - 1]) < 1e-9) pts.pop(); return pts; };

// ---------------------------------------------------------------- booleans

async function combine(c, op, label) {
  const sol = await selectSolids(c, 'Select solids to ' + label);
  if (sol.length < 2) { c.log(`${op[0].toUpperCase() + op.slice(1)} needs at least two solids.`, 'error'); return; }
  c.log('Computing...');
  let acc = sol[0];
  for (const s of sol.slice(1)) acc = await booleanAsync(acc, s, op);
  finishBoolean(c, sol, acc, { union: 'Union', intersect: 'Intersect' }[op]);
}

function finishBoolean(c, operands, result, name) {
  const first = operands[0];
  c.doc.change(() => {
    for (const s of operands) c.doc.solids.splice(c.doc.solids.indexOf(s), 1);
    if (result.faces.length) { const m = { name, vertices: result.vertices, faces: result.faces, layer: first.layer, ...(first.color !== undefined ? { color: first.color } : {}) }; m.id = c.doc.nextId++; c.doc.solids.push(m); c.log(`${name} complete (volume ${f(c, S.meshVolume(m))}).`); }
    else c.log('The result is empty; no solid remains.');
  });
  c.clearSel();
}

def({ name: 'UNION', aliases: ['UNI'], title: 'Union', icon: 'union', desc: 'Combines selected 3D solids into a single solid.', async run(c) { await combine(c, 'union', 'combine'); } });
def({ name: 'INTERSECT', aliases: ['IN'], title: 'Intersect', icon: 'intersect', desc: 'Creates a 3D solid from the volume common to two or more solids.', async run(c) { await combine(c, 'intersect', 'intersect'); } });
def({
  name: 'SUBTRACT', aliases: ['SU'], title: 'Subtract', icon: 'subtract', desc: 'Subtracts one set of 3D solids from another.',
  async run(c) {
    const a = await selectSolids(c, 'Select solids to subtract from');
    if (!a.length) return;
    c.editor.tempSel = new Set(a.map(s => s.id));
    const b = (await selectSolids(c, 'Select solids to subtract')).filter(s => !a.includes(s));
    if (!b.length) { c.log('Nothing to subtract.', 'error'); return; }
    c.log('Computing...');
    let acc = a[0];
    for (const s of a.slice(1)) acc = await booleanAsync(acc, s, 'union');
    for (const s of b) acc = await booleanAsync(acc, s, 'subtract');
    finishBoolean(c, [...a, ...b], acc, 'Subtract');
  },
});

// ---------------------------------------------------------------- solid editing and queries

def({
  name: '3DMOVE', aliases: ['3M'], title: '3D Move', icon: 'move', desc: 'Moves 3D solids a specified distance in a specified direction.',
  async run(c) {
    const sol = await selectSolids(c, 'Select solids to move'); if (!sol.length) return;
    const b = await c.point({ prompt: 'Specify base point' }); if (!b.pt) return;
    const r = await c.point({ prompt: 'Specify second point or <use first point as displacement>', base: b.pt, allowEnter: true });
    const d = r.pt ? g.sub(r.pt, b.pt) : b.pt;
    const z = await c.number({ prompt: 'Specify Z displacement', default: 0 });
    const dz = z.value ?? 0;
    c.doc.change(() => { for (const s of sol) { const t = S.transformMesh(s, S.M3.translate(d.x, d.y, dz)); s.vertices = t.vertices; } });
    c.clearSel();
  },
});

def({
  name: 'MASSPROP', title: 'Mass Properties', icon: 'list', desc: 'Calculates the mass properties of regions or 3D solids.',
  async run(c) {
    const sol = await selectSolids(c, 'Select objects'); if (!sol.length) return;
    for (const s of sol) {
      const b = S.meshBounds(s), cen = S.meshCentroid(s);
      c.log(`---------------- SOLID: ${s.name || 'Solid'} ----------------`);
      c.log(`Volume:           ${f(c, S.meshVolume(s))}`);
      c.log(`Surface area:     ${f(c, S.meshArea(s))}`);
      c.log(`Bounding box:     X: ${f(c, b.min.x)} -- ${f(c, b.max.x)}`);
      c.log(`                  Y: ${f(c, b.min.y)} -- ${f(c, b.max.y)}`);
      c.log(`                  Z: ${f(c, b.min.z)} -- ${f(c, b.max.z)}`);
      c.log(`Centroid:         X: ${f(c, cen.x)}  Y: ${f(c, cen.y)}  Z: ${f(c, cen.z)}`);
      c.log(`Triangles:        ${s.faces.length}`);
    }
  },
});

def({
  name: 'VSCURRENT', aliases: ['VS'], title: 'Visual Style', icon: 'view', desc: 'Sets the visual style in the current viewport.', transparent: true,
  async run(c, args) {
    let name = args.style;
    if (!name) { const k = await c.keyword({ prompt: 'Enter an option', options: ['2dwireframe', 'Wireframe', 'Hidden', 'Shaded', 'shaded with Edges'], default: 'Shaded' }); name = k.kw; }
    const map = { '2dwireframe': '2D Wireframe', wireframe: 'Wireframe', hidden: 'Hidden', shaded: 'Shaded', 'shaded with edges': 'Shaded with Edges' };
    name = map[String(name).toLowerCase()] || name;
    if (c.app.ui?.setVisualStyle) c.app.ui.setVisualStyle(name); else c.editor.visualStyle = name;
    c.rt.emit();
  },
});

for (const [name, fmt, title] of [['OBJOUT', 'obj', 'Export OBJ'], ['STLOUT', 'stl', 'Export STL']]) {
  def({
    name, title, icon: 'export', desc: `Writes the solids of the drawing as a ${fmt.toUpperCase()} mesh file.`, repeatable: false,
    async run(c) {
      if (!c.doc.solids.length) { c.log('There are no solids to export.', 'error'); return; }
      const fn = c.app.ui?.exportMesh;
      if (!fn) { c.log(`${fmt.toUpperCase()} export is not available in this environment.`, 'error'); return; }
      await fn.call(c.app.ui, fmt);
    },
  });
}

