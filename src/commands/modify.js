// Modify commands.
import * as g from '../core/geom.js';
import { M } from '../core/geom.js';
import { E, clone, boundsOf, explodeEntity, stretchEntity, gripsOf, gripEdit, segsOf } from '../core/entities.js';
import { registry } from '../engine/registry.js';
import { selectRect, rectOf } from '../engine/select.js';
import { trim, extend, breakAt, splitAt, join, lengthen } from '../ops/trim.js';
import { offsetEntity } from '../ops/offset.js';
import { filletSegs, chamferSegs, chamferAngle, filletPline, chamferPline } from '../ops/fillet.js';
import { pathOf, pathNearest, isPathEntity, entityFromSegs, inheritProps } from '../ops/path.js';
import { memory, transformedClones, addCopies, applyTransform, num } from './helpers.js';

const def = d => registry.define({ group: 'Modify', pickfirst: true, ...d });
const SELECT = 'Select objects';

// ---------------------------------------------------------------- ERASE / OOPS

def({
  name: 'ERASE', aliases: ['E', 'DEL'], title: 'Erase', icon: 'erase', desc: 'Removes objects from the drawing.',
  async run(c) {
    const sel = await c.select({ prompt: SELECT });
    if (!sel.length) return;
    memory(c.app).erased = sel.map(e => clone(e));
    c.doc.removeAll(sel);
    c.clearSel();
  },
});

def({
  name: 'OOPS', title: 'Oops', desc: 'Restores objects deleted by the last ERASE command.',
  async run(c) {
    const m = memory(c.app);
    if (!m.erased?.length) { c.log('Nothing to restore.', 'error'); return; }
    for (const e of m.erased) c.add(clone(e));
    m.erased = null;
  },
});

// ---------------------------------------------------------------- MOVE / COPY

def({
  name: 'MOVE', aliases: ['M'], title: 'Move', icon: 'move', desc: 'Moves objects a specified distance in a specified direction.',
  async run(c) {
    const sel = await c.select({ prompt: SELECT });
    if (!sel.length) return;
    let r = await c.point({ prompt: 'Specify base point', options: ['Displacement'], default: undefined });
    let delta;
    if (r.kw === 'Displacement') {
      const d = await c.point({ prompt: 'Specify displacement', default: { x: 0, y: 0 }, noSnap: false });
      if (!d.pt) return; delta = d.pt;
    } else {
      if (!r.pt) return;
      const base = r.pt;
      const r2 = await c.point({ prompt: 'Specify second point or <use first point as displacement>', base, allowEnter: true, preview: pt => transformedClones(sel, M.translate(pt.x - base.x, pt.y - base.y), c.doc) });
      delta = r2.pt ? g.sub(r2.pt, base) : base;
    }
    applyTransform(c.doc, sel, M.translate(delta.x, delta.y));
    c.clearSel();
  },
});

def({
  name: 'COPY', aliases: ['CO', 'CP'], title: 'Copy', icon: 'copy', desc: 'Copies objects a specified distance in a specified direction.',
  async run(c) {
    const sel = await c.select({ prompt: SELECT });
    if (!sel.length) return;
    const r = await c.point({ prompt: 'Specify base point', options: ['Displacement', 'mOde'] });
    if (r.kw === 'Displacement') { const d = await c.point({ prompt: 'Specify displacement', default: { x: 0, y: 0 } }); if (d.pt) addCopies(c.doc, sel, M.translate(d.pt.x, d.pt.y)); c.clearSel(); return; }
    if (!r.pt) return;
    const base = r.pt, made = [];
    for (;;) {
      const opts = made.length ? ['Array', 'Exit', 'Undo'] : ['Array'];
      const n = await c.point({ prompt: 'Specify second point' + (made.length ? '' : ' or [Array]') + (made.length ? '' : ''), options: made.length ? opts : [], base, allowEnter: true, preview: pt => transformedClones(sel, M.translate(pt.x - base.x, pt.y - base.y), c.doc) });
      if (n.kw === 'Undo') { const last = made.pop(); if (last) c.doc.removeAll(last); continue; }
      if (n.kw === 'Exit' || !n.pt) break;
      made.push(addCopies(c.doc, sel, M.translate(n.pt.x - base.x, n.pt.y - base.y)));
    }
    c.clearSel();
  },
});

// ---------------------------------------------------------------- ROTATE / SCALE / MIRROR

def({
  name: 'ROTATE', aliases: ['RO'], title: 'Rotate', icon: 'rotate', desc: 'Rotates objects around a base point.',
  async run(c) {
    const sel = await c.select({ prompt: SELECT });
    if (!sel.length) return;
    const b = await c.point({ prompt: 'Specify base point' }); if (!b.pt) return;
    const base = b.pt;
    let copy = false, refAngle = 0;
    for (;;) {
      const a = await c.angle({ prompt: 'Specify rotation angle', options: ['Copy', 'Reference'], base, default: 0, preview: pt => transformedClones(sel, M.rotate(g.angleOf(base, pt) - refAngle, base), c.doc) });
      if (a.kw === 'Copy') { copy = true; continue; }
      if (a.kw === 'Reference') {
        const ra = await c.angle({ prompt: 'Specify the reference angle', base, default: 0 });
        if (ra.value === undefined) return; refAngle = ra.value;
        const na = await c.angle({ prompt: 'Specify the new angle', base, default: 0, preview: pt => transformedClones(sel, M.rotate(g.angleOf(base, pt) - refAngle, base), c.doc) });
        if (na.value === undefined) return;
        doRotate(na.value - refAngle); break;
      }
      if (a.value === undefined) return;
      doRotate(a.pt ? a.value - refAngle : a.value); break;
    }
    function doRotate(ang) {
      const m = M.rotate(ang, base);
      if (copy) addCopies(c.doc, sel, m); else applyTransform(c.doc, sel, m);
      c.clearSel();
    }
  },
});

def({
  name: 'SCALE', aliases: ['SC'], title: 'Scale', icon: 'scale', desc: 'Enlarges or reduces selected objects, keeping the proportions the same.',
  async run(c) {
    const sel = await c.select({ prompt: SELECT });
    if (!sel.length) return;
    const b = await c.point({ prompt: 'Specify base point' }); if (!b.pt) return;
    const base = b.pt;
    let copy = false;
    for (;;) {
      const s = await c.distance({ prompt: 'Specify scale factor', options: ['Copy', 'Reference'], base, default: 1, min: 0, nonzero: true, preview: undefined });
      if (s.kw === 'Copy') { copy = true; continue; }
      let factor;
      if (s.kw === 'Reference') {
        const rl = await c.distance({ prompt: 'Specify reference length', default: 1, nonzero: true, min: 0 }); if (rl.value === undefined) return;
        const nl = await c.distance({ prompt: 'Specify new length', base, nonzero: true, min: 0 }); if (nl.value === undefined) return;
        factor = nl.value / rl.value;
      } else if (s.value === undefined) return;
      else factor = s.pt ? s.value / 1 : s.value;
      if (!(factor > 0)) { c.log('The scale factor must be positive.', 'error'); return; }
      const m = M.scale(factor, factor, base);
      if (copy) addCopies(c.doc, sel, m); else applyTransform(c.doc, sel, m);
      c.clearSel(); return;
    }
  },
});

def({
  name: 'MIRROR', aliases: ['MI'], title: 'Mirror', icon: 'mirror', desc: 'Creates a mirrored copy of selected objects.',
  async run(c) {
    const sel = await c.select({ prompt: SELECT });
    if (!sel.length) return;
    const a = await c.point({ prompt: 'Specify first point of mirror line' }); if (!a.pt) return;
    const b = await c.point({ prompt: 'Specify second point of mirror line', base: a.pt, preview: pt => g.samePt(a.pt, pt) ? [] : [E.line(a.pt, pt), ...transformedClones(sel, M.mirror(a.pt, pt), c.doc)] });
    if (!b.pt || g.samePt(a.pt, b.pt)) return;
    const del = await c.keyword({ prompt: 'Erase source objects? [Yes/No]', options: ['Yes', 'No'], default: 'No' });
    const m = M.mirror(a.pt, b.pt);
    if (del.kw === 'Yes') applyTransform(c.doc, sel, m); else addCopies(c.doc, sel, m);
    c.clearSel();
  },
});

// ---------------------------------------------------------------- OFFSET

def({
  name: 'OFFSET', aliases: ['O'], title: 'Offset', icon: 'offset', desc: 'Creates concentric circles, parallel lines, and parallel curves.',
  pickfirst: false,
  async run(c) {
    const st = c.doc.settings;
    let dist = st.offsetGap > 0 ? st.offsetGap : null, erase = false, through = dist === null;
    for (;;) {
      const first = await c.distance({ prompt: 'Specify offset distance', options: ['Through', 'Erase', 'Layer'], default: through ? 'Through' : dist, min: 0 });
      if (first.kw === 'Erase') { const y = await c.keyword({ prompt: 'Erase source object after offsetting? [Yes/No]', options: ['Yes', 'No'], default: erase ? 'Yes' : 'No' }); erase = y.kw === 'Yes'; continue; }
      if (first.kw === 'Layer') { c.log('Offset objects are created on the layer of the source object.'); continue; }
      if (first.kw === 'Through' || (first.enter && through)) { through = true; break; }
      if (first.value === undefined) return;
      if (first.value <= 0) { c.log('Requires a positive, nonzero distance.', 'error'); continue; }
      dist = first.value; through = false; st.offsetGap = dist; break;
    }
    const done = [];
    for (;;) {
      const o = await c.entity({ prompt: 'Select object to offset', options: ['Exit', 'Undo'], allowEnter: true, filter: e => ['line', 'circle', 'arc', 'pline', 'ellipse', 'spline', 'xline'].includes(e.type) });
      if (o.kw === 'Undo') { const u = done.pop(); if (u) c.doc.remove(u); continue; }
      if (o.kw === 'Exit' || !o.entity) return;
      const src = o.entity;
      c.editor.tempSel = new Set([src.id]);
      const side = await c.point({ prompt: through ? 'Specify through point' : 'Specify point on side to offset', options: through ? ['Exit', 'Multiple', 'Undo'] : ['Exit', 'Multiple', 'Undo'], allowEnter: true, preview: pt => {
        const d = through ? Math.max(1e-9, nearestDistance(src, pt, c.doc)) : dist; const r = offsetEntity(src, d, pt, c.doc); return r.result ? [r.result] : [];
      } });
      c.editor.tempSel = new Set();
      if (side.kw === 'Undo') { const u = done.pop(); if (u) c.doc.remove(u); continue; }
      if (!side.pt) { if (side.kw === 'Exit' || side.enter) return; continue; }
      const d = through ? nearestDistance(src, side.pt, c.doc) : dist;
      const r = offsetEntity(src, d, side.pt, c.doc);
      if (r.error) { c.log(r.error, 'error'); continue; }
      const made = c.add(r.result); done.push(made);
      if (erase) c.doc.remove(src);
    }
  },
});
const nearestDistance = (e, pt, doc) => { let best = Infinity; for (const s of segsOf(e, doc)) best = Math.min(best, g.segNearest(s, pt).d); return best; };

// ---------------------------------------------------------------- ARRAY

const arrayCommand = (kind) => async function run(c, args) {
  const sel = await c.select({ prompt: SELECT });
  if (!sel.length) return;
  const mem = memory(c.app);
  let type = kind;
  if (!type) { const t = await c.keyword({ prompt: 'Enter array type [Rectangular/Path/POlar]', options: ['Rectangular', 'POlar'], default: 'Rectangular' }); type = t.kw === 'POlar' ? 'polar' : 'rect'; }
  if (type === 'rect') {
    const r = await c.number({ prompt: 'Enter the number of rows (---)', default: mem.arrayRect.rows, integer: true, min: 1 });
    const cl = await c.number({ prompt: 'Enter the number of columns (|||)', default: mem.arrayRect.cols, integer: true, min: 1 });
    if (r.value === undefined || cl.value === undefined) return;
    let dr = 0, dc = 0;
    if (r.value > 1) { const d = await c.distance({ prompt: 'Specify the distance between rows (---)', default: mem.arrayRect.dr }); if (d.value === undefined) return; dr = d.value; }
    if (cl.value > 1) { const d = await c.distance({ prompt: 'Specify the distance between columns (|||)', default: mem.arrayRect.dc }); if (d.value === undefined) return; dc = d.value; }
    mem.arrayRect = { rows: r.value, cols: cl.value, dr: dr || mem.arrayRect.dr, dc: dc || mem.arrayRect.dc };
    if (r.value * cl.value > 100000) { c.log('The array would contain too many objects.', 'error'); return; }
    for (let i = 0; i < r.value; i++) for (let j = 0; j < cl.value; j++) if (i || j) addCopies(c.doc, sel, M.translate(j * dc, i * dr));
  } else {
    const cp = await c.point({ prompt: 'Specify center point of array', options: [] }); if (!cp.pt) return;
    const n = await c.number({ prompt: 'Enter number of items in the array', default: mem.arrayPolar.n, integer: true, min: 2 }); if (n.value === undefined) return;
    const fill = await c.angle({ prompt: 'Specify the angle to fill (+=ccw, -=cw)', default: g.rad(mem.arrayPolar.fill) }); if (fill.value === undefined) return;
    const rot = await c.keyword({ prompt: 'Rotate arrayed objects as they are copied? [Yes/No]', options: ['Yes', 'No'], default: mem.arrayPolar.rotate ? 'Yes' : 'No' });
    const full = Math.abs(Math.abs(fill.value) - g.TAU) < 1e-6;
    const step = fill.value / (full ? n.value : n.value - 1);
    mem.arrayPolar = { n: n.value, fill: g.deg(fill.value), rotate: rot.kw !== 'No' };
    for (let i = 1; i < n.value; i++) {
      const a = step * i;
      if (rot.kw !== 'No') addCopies(c.doc, sel, M.rotate(a, cp.pt));
      else { const delta = g.sub(M.apply(M.rotate(a, cp.pt), g.mid(...(centroidPair(sel, c.doc)))), centroidPair(sel, c.doc)[0]); addCopies(c.doc, sel, M.translate(delta.x, delta.y)); }
    }
  }
  c.clearSel();
  void args;
};
const centroidPair = (sel, doc) => { let b = null; for (const e of sel) b = g.unionBounds(b, boundsOf(e, doc)); const m = { x: (b.minX + b.maxX) / 2, y: (b.minY + b.maxY) / 2 }; return [m, m]; };
def({ name: 'ARRAY', aliases: ['AR'], title: 'Array', icon: 'arrayrect', desc: 'Creates copies of objects arranged in a pattern.', run: arrayCommand(null) });
def({ name: 'ARRAYRECT', title: 'Rectangular Array', icon: 'arrayrect', desc: 'Distributes copies of the selected object into any combination of rows and columns.', run: arrayCommand('rect') });
def({ name: 'ARRAYPOLAR', title: 'Polar Array', icon: 'arraypolar', desc: 'Evenly distributes copies of the selected object in a circular pattern around a center point.', run: arrayCommand('polar') });

// ---------------------------------------------------------------- STRETCH

def({
  name: 'STRETCH', aliases: ['S'], title: 'Stretch', icon: 'stretch', desc: 'Stretches objects crossed by a selection window or polygon.',
  async run(c) {
    c.rt.lastWindow = null;
    const sel = await c.select({ prompt: 'Select objects to stretch by crossing-window or crossing-polygon', pickfirst: false });
    if (!sel.length) return;
    const win = c.rt.lastWindow?.rect;
    const rectFor = e => { const b = boundsOf(e, c.doc); return win && g.boundsOverlap(win, b) ? win : g.growBounds(b, 1); };
    const b = await c.point({ prompt: 'Specify base point or [Displacement]', options: ['Displacement'] });
    let delta;
    if (b.kw === 'Displacement') { const d = await c.point({ prompt: 'Specify displacement', default: { x: 0, y: 0 } }); if (!d.pt) return; delta = d.pt; }
    else {
      if (!b.pt) return;
      const base = b.pt;
      const preview = pt => sel.map(e => { const cl = clone(e); stretchEntity(cl, rectFor(e), g.sub(pt, base), c.doc); return cl; });
      const s = await c.point({ prompt: 'Specify second point or <use first point as displacement>', base, allowEnter: true, preview });
      delta = s.pt ? g.sub(s.pt, base) : base;
    }
    c.doc.change(() => { for (const e of sel) stretchEntity(e, rectFor(e), delta, c.doc); });
    c.clearSel();
  },
});

// ---------------------------------------------------------------- TRIM / EXTEND

const trimExtend = (isTrim) => async function run(c) {
  const mem = memory(c.app);
  c.log(`Current settings: Projection=UCS, Edge=${mem.edgeExtend ? 'Extend' : 'None'}`);
  const edges = await c.select({ prompt: isTrim ? 'Select cutting edges... Select objects or <select all>' : 'Select boundary edges... Select objects or <select all>', pickfirst: false });
  const cutters = edges.length ? edges : c.doc.entities.filter(e => c.doc.isVisible(e));
  const history = [];
  const apply = (target, pick, shift) => {
    const op = (isTrim !== !!shift) ? trim : extend;
    const r = op(target, pick, cutters, c.doc, isTrim !== !!shift ? { extendCutters: !!mem.edgeExtend } : { extendBoundaries: !!mem.edgeExtend });
    if (r.error) { c.log(r.error, 'error'); return false; }
    const idx = c.doc.entities.indexOf(target);
    c.doc.change(() => { c.doc.entities.splice(idx, 1, ...r.replace.map(e => { e.id = c.doc.nextId++; return e; })); });
    history.push({ idx, old: target, news: r.replace });
    return true;
  };
  for (;;) {
    const r = await c.entity({
      prompt: isTrim ? 'Select object to trim or shift-select to extend or' : 'Select object to extend or shift-select to trim or',
      options: ['Fence', 'Crossing', 'Project', 'Edge', ...(isTrim ? ['eRase'] : []), 'Undo'], allowEnter: true, filter: e => ['line', 'arc', 'pline', 'circle'].includes(e.type),
    });
    if (r.kw === 'Undo') {
      const h = history.pop(); if (!h) continue;
      c.doc.change(() => { for (const n of h.news) { const i = c.doc.entities.indexOf(n); if (i >= 0) c.doc.entities.splice(i, 1); } c.doc.entities.splice(Math.min(h.idx, c.doc.entities.length), 0, h.old); });
      continue;
    }
    if (r.kw === 'Edge') { const k = await c.keyword({ prompt: 'Enter an implied edge extension mode [Extend/No extend]', options: ['Extend', 'No extend'], default: mem.edgeExtend ? 'Extend' : 'No extend' }); mem.edgeExtend = k.kw === 'Extend'; continue; }
    if (r.kw === 'Project') { c.log('Projection=None; only UCS plane trimming is supported.'); continue; }
    if (r.kw === 'eRase') { const sel = await c.select({ prompt: 'Select objects to erase', pickfirst: false }); c.doc.removeAll(sel); continue; }
    if (r.kw === 'Fence') { await fenceOrCrossing(c, 'fence', apply); continue; }
    if (r.kw === 'Crossing') { await fenceOrCrossing(c, 'crossing', apply); continue; }
    if (!r.entity) break;
    apply(r.entity, r.pt, r.mods?.shift);
  }
};

async function fenceOrCrossing(c, mode, apply) {
  const doc = c.doc, pts = [];
  if (mode === 'fence') {
    for (;;) { const p = await c.point({ prompt: pts.length ? 'Specify next fence point' : 'Specify first fence point', allowEnter: true, base: pts[pts.length - 1], preview: pt => pts.length ? [E.pline([...pts, pt], false)] : [] }); if (!p.pt) break; pts.push(p.pt); }
    if (pts.length < 2) return;
    const edges = pts.slice(0, -1).map((a, i) => ({ t: 'l', a, b: pts[i + 1] }));
    for (const e of [...doc.entities]) {
      if (!['line', 'arc', 'pline', 'circle'].includes(e.type) || !doc.isVisible(e)) continue;
      for (const s of segsOf(e, doc)) { const hit = edges.map(q => g.segIntersect(s, q)).flat()[0]; if (hit) { apply(e, hit.pt, false); break; } }
    }
  } else {
    const a = await c.point({ prompt: 'Specify first corner' }); if (!a.pt) return;
    const b = await c.corner({ prompt: 'Specify opposite corner', base: a.pt }); if (!b.pt) return;
    const rect = rectOf(a.pt, b.pt), centre = { x: (rect.minX + rect.maxX) / 2, y: (rect.minY + rect.maxY) / 2 };
    for (const e of selectRect(doc, rect, true)) {
      if (!['line', 'arc', 'pline', 'circle'].includes(e.type)) continue;
      const path = pathOf(e, doc), n = pathNearest(path, centre);
      apply(e, n.pt, false);
    }
  }
}

def({ name: 'TRIM', aliases: ['TR'], title: 'Trim', icon: 'trim', desc: 'Trims objects to meet the edges of other objects.', pickfirst: false, run: trimExtend(true) });
def({ name: 'EXTEND', aliases: ['EX'], title: 'Extend', icon: 'extend', desc: 'Extends objects to meet the edges of other objects.', pickfirst: false, run: trimExtend(false) });

// ---------------------------------------------------------------- BREAK / JOIN / LENGTHEN

def({
  name: 'BREAK', aliases: ['BR'], title: 'Break', icon: 'break', desc: 'Breaks the selected object between two points.', pickfirst: false,
  async run(c) {
    const o = await c.entity({ prompt: 'Select object', filter: isPathEntity }); if (!o.entity) return;
    let p1 = o.pt;
    for (;;) {
      const r = await c.point({ prompt: 'Specify second break point or [First point]', options: ['First point'] });
      if (r.kw === 'First point') { const f = await c.point({ prompt: 'Specify first break point' }); if (f.pt) p1 = f.pt; continue; }
      if (!r.pt) return;
      if (g.dist(r.pt, p1) < 1e-9) { const s = splitAt(o.entity, p1, c.doc); finish(s); return; }
      finish(breakAt(o.entity, p1, r.pt, c.doc)); return;
    }
    function finish(res) {
      if (res.error) { c.log(res.error, 'error'); return; }
      const idx = c.doc.entities.indexOf(o.entity);
      c.doc.change(() => c.doc.entities.splice(idx, 1, ...res.replace.map(e => { e.id = c.doc.nextId++; return e; })));
    }
  },
});
def({
  name: 'BREAKATPOINT', title: 'Break at Point', icon: 'breakpoint', desc: 'Breaks the selected object at a single point.', pickfirst: false,
  async run(c) {
    const o = await c.entity({ prompt: 'Select object', filter: e => isPathEntity(e) && e.type !== 'circle' && !e.closed }); if (!o.entity) return;
    const p = await c.point({ prompt: 'Specify break point' }); if (!p.pt) return;
    const res = splitAt(o.entity, p.pt, c.doc);
    if (res.error) { c.log(res.error, 'error'); return; }
    const idx = c.doc.entities.indexOf(o.entity);
    c.doc.change(() => c.doc.entities.splice(idx, 1, ...res.replace.map(e => { e.id = c.doc.nextId++; return e; })));
  },
});

def({
  name: 'JOIN', aliases: ['J'], title: 'Join', icon: 'join', desc: 'Joins the endpoints of linear and curved objects to create a single object.',
  async run(c) {
    const sel = await c.select({ prompt: 'Select source object or multiple objects to join at once', filter: isPathEntity });
    if (sel.length < 2) { if (sel.length) c.log('1 object cannot be joined by itself.', 'error'); return; }
    const res = join(sel, c.doc);
    const joined = res.sources.length - res.results.length;
    if (!res.results.length) return;
    c.doc.change(() => {
      const used = new Set(res.used);
      c.doc.entities = c.doc.entities.filter(e => !used.has(e));
      for (const e of res.results) { e.id = c.doc.nextId++; c.doc.entities.push(e); }
    });
    c.log(`${joined} object${joined === 1 ? '' : 's'} joined to ${res.results.length} object${res.results.length === 1 ? '' : 's'}`);
    c.clearSel();
  },
});

def({
  name: 'LENGTHEN', aliases: ['LEN'], title: 'Lengthen', icon: 'lengthen', desc: 'Changes the length of objects and the included angle of arcs.', pickfirst: false,
  async run(c) {
    let mode = null, value = 0;
    for (;;) {
      const r = await c.entity({ prompt: 'Select an object or [DElta/Percent/Total/DYnamic]', options: ['DElta', 'Percent', 'Total', 'DYnamic'], allowEnter: true, filter: e => ['line', 'arc', 'pline'].includes(e.type) });
      if (r.kw) {
        const m = r.kw.toLowerCase();
        if (m === 'dynamic') { c.log('Dynamic lengthening: use grips on the object.'); continue; }
        const prompts = { delta: 'Enter delta length', percent: 'Enter percentage length', total: 'Specify total length' };
        const v = await c.distance({ prompt: prompts[m], default: m === 'percent' ? 100 : 0 });
        if (v.value === undefined) return;
        mode = m; value = v.value; continue;
      }
      if (!r.entity) return;
      if (!mode) { const e = r.entity, L = segsOf(e, c.doc).reduce((s, q) => s + g.segLength(q), 0); c.log(`Current length: ${num(L)}`); continue; }
      const res = lengthen(r.entity, r.pt, mode, value, c.doc);
      if (res.error) { c.log(res.error, 'error'); continue; }
      c.doc.replace(r.entity, res.replace[0]);
    }
  },
});

// ---------------------------------------------------------------- FILLET / CHAMFER

const segAtPick = (e, pick, doc) => {
  const path = pathOf(e, doc);
  if (!path) return null;
  const n = pathNearest(path, pick);
  return { path, seg: path.segs[Math.min(path.segs.length - 1, Math.floor(n.u))], index: Math.min(path.segs.length - 1, Math.floor(n.u)), pick };
};

def({
  name: 'FILLET', aliases: ['F'], title: 'Fillet', icon: 'fillet', desc: 'Rounds and fillets the edges of objects.', pickfirst: false,
  async run(c) {
    const st = c.doc.settings, mem = memory(c.app);
    let trimMode = mem.filletTrim !== false, multiple = false;
    for (;;) {
      c.log(`Current settings: Mode = ${trimMode ? 'TRIM' : 'NOTRIM'}, Radius = ${num(st.filletRadius)}`);
      const a = await c.entity({ prompt: 'Select first object or', options: ['Undo', 'Polyline', 'Radius', 'Trim', 'Multiple'], allowEnter: true, filter: e => isPathEntity(e) });
      if (a.kw === 'Radius') { const r = await c.distance({ prompt: 'Specify fillet radius', default: st.filletRadius, min: 0 }); if (r.value !== undefined) st.filletRadius = r.value; continue; }
      if (a.kw === 'Trim') { const t = await c.keyword({ prompt: 'Enter Trim mode option [Trim/No trim]', options: ['Trim', 'No trim'], default: trimMode ? 'Trim' : 'No trim' }); trimMode = t.kw !== 'No trim'; mem.filletTrim = trimMode; continue; }
      if (a.kw === 'Multiple') { multiple = true; continue; }
      if (a.kw === 'Undo') { c.log('Nothing to undo.'); continue; }
      if (a.kw === 'Polyline') {
        const p = await c.entity({ prompt: 'Select 2D polyline', filter: e => e.type === 'pline' }); if (!p.entity) continue;
        const res = filletPline(p.entity, st.filletRadius, c.doc);
        if (!res.applied) { c.log('No corners could be filleted with that radius.', 'error'); continue; }
        c.doc.replace(p.entity, res.entity); c.log(`${res.applied} were filleted`); if (!multiple) return; continue;
      }
      if (!a.entity) return;
      const b = await c.entity({ prompt: 'Select second object or shift-select to apply corner or [Radius]', options: ['Radius'], filter: e => isPathEntity(e) });
      if (b.kw === 'Radius') { const r = await c.distance({ prompt: 'Specify fillet radius', default: st.filletRadius, min: 0 }); if (r.value !== undefined) st.filletRadius = r.value; continue; }
      if (!b.entity) return;
      const radius = b.mods?.shift ? 0 : st.filletRadius;
      if (a.entity === b.entity) {
        // two segments of the same polyline
        const A = segAtPick(a.entity, a.pt, c.doc), B = segAtPick(b.entity, b.pt, c.doc), n = A.path.segs.length;
        if (a.entity.type !== 'pline' || Math.abs(A.index - B.index) !== 1 && !(a.entity.closed && Math.abs(A.index - B.index) === n - 1)) { c.log('Select two different objects or adjacent segments.', 'error'); continue; }
        const segs = A.path.segs.slice(), i = Math.min(A.index, B.index), j = (i + 1) % n;
        const r = filletSegs(segs[i], g.segAt(segs[i], .05), segs[j], g.segAt(segs[j], .95), radius);
        if (r.error) { c.log(r.error, 'error'); continue; }
        segs[i] = r.a; segs[j] = r.b;
        const out = []; segs.forEach((s, k) => { out.push(s); if (k === i && r.arc) out.push(r.arc); });
        c.doc.replace(a.entity, entityFromSegs(out, a.entity, a.entity.closed));
        if (!multiple) return; continue;
      }
      const A = segAtPick(a.entity, a.pt, c.doc), B = segAtPick(b.entity, b.pt, c.doc);
      if (!A || !B || a.entity.type === 'pline' || b.entity.type === 'pline') { c.log('Fillet between separate objects requires lines, arcs or circles.', 'error'); continue; }
      const r = filletSegs(A.seg, a.pt, B.seg, b.pt, radius);
      if (r.error) { c.log(r.error, 'error'); continue; }
      c.doc.change(() => {
        if (trimMode) {
          if (r.a) replaceSeg(c, a.entity, r.a);
          if (r.b) replaceSeg(c, b.entity, r.b);
        }
        if (r.arc) c.add(inheritProps(a.entity, E.arcSeg(r.arc)));
      });
      if (!multiple) return;
    }
  },
});

/** Replace a line/arc entity's geometry with a segment (in place). */
function replaceSeg(c, e, seg) {
  if (e.type === 'line') { e.a = seg.a; e.b = seg.b; }
  else if (e.type === 'arc') { const n = E.arcSeg(seg); e.c = n.c; e.r = n.r; e.a0 = n.a0; e.a1 = n.a1; }
}

def({
  name: 'CHAMFER', aliases: ['CHA'], title: 'Chamfer', icon: 'chamfer', desc: 'Bevels the edges of objects.', pickfirst: false,
  async run(c) {
    const st = c.doc.settings;
    let method = 'distance', angleLen = 0, angle = g.rad(45), trimMode = true;
    for (;;) {
      c.log(`(TRIM mode) Current chamfer Dist1 = ${num(st.chamferD1)}, Dist2 = ${num(st.chamferD2)}`);
      const a = await c.entity({ prompt: 'Select first line or', options: ['Undo', 'Polyline', 'Distance', 'Angle', 'Trim', 'mEthod', 'Multiple'], allowEnter: true, filter: e => e.type === 'line' || e.type === 'pline' });
      if (a.kw === 'Distance') { const d1 = await c.distance({ prompt: 'Specify first chamfer distance', default: st.chamferD1, min: 0 }); if (d1.value === undefined) continue; const d2 = await c.distance({ prompt: 'Specify second chamfer distance', default: d1.value, min: 0 }); if (d2.value === undefined) continue; st.chamferD1 = d1.value; st.chamferD2 = d2.value; method = 'distance'; continue; }
      if (a.kw === 'Angle') { const l = await c.distance({ prompt: 'Specify chamfer length on the first line', default: angleLen || 1, min: 0 }); const an = await c.angle({ prompt: 'Specify chamfer angle from the first line', default: angle }); if (l.value !== undefined && an.value !== undefined) { angleLen = l.value; angle = an.value; method = 'angle'; } continue; }
      if (a.kw === 'Trim') { const t = await c.keyword({ prompt: 'Enter Trim mode option [Trim/No trim]', options: ['Trim', 'No trim'], default: 'Trim' }); trimMode = t.kw !== 'No trim'; continue; }
      if (a.kw === 'mEthod') { const m = await c.keyword({ prompt: 'Enter trim method [Distance/Angle]', options: ['Distance', 'Angle'], default: method === 'angle' ? 'Angle' : 'Distance' }); method = m.kw === 'Angle' ? 'angle' : 'distance'; continue; }
      if (a.kw === 'Polyline') {
        const p = await c.entity({ prompt: 'Select 2D polyline', filter: e => e.type === 'pline' }); if (!p.entity) continue;
        const res = chamferPline(p.entity, st.chamferD1, st.chamferD2, c.doc);
        if (!res.applied) { c.log('No corners could be chamfered with those distances.', 'error'); continue; }
        c.doc.replace(p.entity, res.entity); c.log(`${res.applied} lines were chamfered`); return;
      }
      if (a.kw === 'Undo' || a.kw === 'Multiple') continue;
      if (!a.entity) return;
      if (a.entity.type !== 'line') { c.log('Select a straight line.', 'error'); continue; }
      const b = await c.entity({ prompt: 'Select second line or shift-select to apply corner', filter: e => e.type === 'line' });
      if (!b.entity) return;
      const A = segsOf(a.entity, c.doc)[0], B = segsOf(b.entity, c.doc)[0];
      const shift = b.mods?.shift;
      const r = shift ? chamferSegs(A, a.pt, B, b.pt, 0, 0) : method === 'angle' ? chamferAngle(A, a.pt, B, b.pt, angleLen, angle) : chamferSegs(A, a.pt, B, b.pt, st.chamferD1, st.chamferD2);
      if (r.error) { c.log(r.error, 'error'); continue; }
      c.doc.change(() => {
        if (trimMode) { replaceSeg(c, a.entity, r.a); replaceSeg(c, b.entity, r.b); }
        if (r.line) c.add(inheritProps(a.entity, E.line(r.line.a, r.line.b)));
      });
      return;
    }
  },
});

// ---------------------------------------------------------------- EXPLODE / PEDIT

def({
  name: 'EXPLODE', aliases: ['X'], title: 'Explode', icon: 'explode', desc: 'Breaks a compound object into its component objects.',
  async run(c) {
    const sel = await c.select({ prompt: SELECT, filter: e => ['pline', 'insert', 'dim', 'leader', 'text'].includes(e.type) && (e.type !== 'text' || e.multi) });
    if (!sel.length) return;
    let count = 0;
    c.doc.change(() => {
      for (const e of sel) {
        const parts = explodeEntity(e, c.doc);
        if (!parts) continue;
        const idx = c.doc.entities.indexOf(e);
        const made = parts.map(p => { p.id = c.doc.nextId++; if (p.layer === undefined) p.layer = e.layer; return p; });
        c.doc.entities.splice(idx, 1, ...made);
        count += made.length;
      }
    });
    c.log(`${count} objects created`);
    c.clearSel();
  },
});

def({
  name: 'PEDIT', aliases: ['PE'], title: 'Edit Polyline', icon: 'pedit', desc: 'Edits polylines and 3D polygon meshes.', pickfirst: true,
  async run(c, args) {
    let e = args.entity || null;
    if (!e) {
      const pre = c.editor.selected;
      if (pre.length === 1 && (pre[0].type === 'pline' || pre[0].type === 'line' || pre[0].type === 'arc')) e = pre[0];
    }
    if (!e) { const r = await c.entity({ prompt: 'Select polyline or [Multiple]', options: ['Multiple'], filter: x => ['pline', 'line', 'arc'].includes(x.type) }); e = r.entity; }
    if (!e) return;
    if (e.type !== 'pline') {
      const y = await c.keyword({ prompt: 'Object selected is not a polyline\nDo you want to turn it into one? <Y>', options: ['Yes', 'No'], default: 'Yes' });
      if (y.kw === 'No') return;
      const segs = segsOf(e, c.doc);
      const pl = entityFromSegs(segs, e, false);
      const np = pl.type === 'pline' ? pl : E.pline(g.segsToVerts(segs, false), false);
      inheritProps(e, np);
      e = c.doc.replace(e, np);
    }
    for (;;) {
      const opts = [e.closed ? 'Open' : 'Close', 'Join', 'Width', 'Edit vertex', 'Fit', 'Spline', 'Decurve', 'Ltype gen', 'Reverse', 'Undo'];
      const r = await c.keyword({ prompt: 'Enter an option', options: opts.filter(o => !['Width', 'Edit vertex', 'Fit', 'Spline', 'Decurve', 'Ltype gen'].includes(o)), allowEnter: true });
      if (!r.kw) return;
      if (r.kw === 'Close' || r.kw === 'Open') c.doc.change(() => { e.closed = r.kw === 'Close'; });
      else if (r.kw === 'Reverse') c.doc.change(() => { const segs = segsOf(e, c.doc).slice().reverse().map(g.segReverse); const ne = entityFromSegs(segs, e, e.closed); const keep = e.id; Object.assign(e, { v: ne.type === 'pline' ? ne.v : g.segsToVerts(segs, e.closed) }); e.id = keep; });
      else if (r.kw === 'Join') {
        const more = await c.select({ prompt: 'Select objects to join', filter: x => isPathEntity(x) && x !== e, pickfirst: false });
        const res = join([e, ...more], c.doc);
        c.doc.change(() => {
          const used = new Set(res.used); c.doc.entities = c.doc.entities.filter(x => !used.has(x));
          for (const n of res.results) { n.id = c.doc.nextId++; c.doc.entities.push(n); }
        });
        e = res.results[0] || e;
      } else if (r.kw === 'Undo') c.doc.undo();
    }
  },
});

// ---------------------------------------------------------------- MATCHPROP

def({
  name: 'MATCHPROP', aliases: ['MA'], title: 'Match Properties', icon: 'matchprop', desc: 'Applies the properties of a selected object to other objects.', pickfirst: false,
  async run(c) {
    const src = await c.pickEntity('Select source object'); if (!src) return;
    c.log(`Current active settings = Color Layer Ltype Ltscale Lineweight Transparency Thickness Dim Text Hatch Polyline`);
    for (;;) {
      const t = await c.entity({ prompt: 'Select destination object(s) or [Settings]', options: ['Settings'], allowEnter: true });
      if (t.kw) { c.log('Settings: all basic properties are matched.'); continue; }
      if (!t.entity) return;
      c.doc.change(() => {
        const d = t.entity;
        for (const k of ['layer', 'color', 'ltype', 'lw', 'lts', 'transp']) { if (src[k] === undefined) delete d[k]; else d[k] = src[k]; }
        if (src.type === 'text' && d.type === 'text') { d.h = src.h; d.jh = src.jh; }
        if (src.type === 'dim' && d.type === 'dim') d.ds = src.ds;
        if (src.type === 'hatch' && d.type === 'hatch') { d.pattern = src.pattern; d.ps = src.ps; d.pa = src.pa; }
      });
    }
  },
});

// ---------------------------------------------------------------- DIVIDE / MEASURE

const markAlong = (isDivide) => async function run(c) {
  const o = await c.pickEntity(isDivide ? 'Select object to divide' : 'Select object to measure', e => isPathEntity(e) || e.type === 'ellipse' || e.type === 'spline'); if (!o) return;
  const path = pathOf(o, c.doc) || { segs: segsOf(o, c.doc), closed: o.type === 'ellipse' };
  const lengths = path.segs.map(s => g.segLength(s)), total = lengths.reduce((a, b) => a + b, 0);
  const pointAt = d => { let acc = 0; for (let i = 0; i < path.segs.length; i++) { if (d <= acc + lengths[i] + 1e-9) return g.segAt(path.segs[i], lengths[i] ? (d - acc) / lengths[i] : 0); acc += lengths[i]; } return g.segEnd(path.segs[path.segs.length - 1]); };
  const pts = [];
  if (isDivide) {
    const n = await c.number({ prompt: 'Enter the number of segments', integer: true, min: 2, max: 32767 }); if (n.value === undefined) return;
    for (let i = 1; i < n.value; i++) pts.push(pointAt(total * i / n.value));
  } else {
    const L = await c.distance({ prompt: 'Specify length of segment', min: 0, nonzero: true }); if (L.value === undefined) return;
    for (let d = L.value; d < total - 1e-9; d += L.value) pts.push(pointAt(d));
  }
  c.doc.change(() => { for (const p of pts) c.add(E.point(p)); });
  c.log(`${pts.length} points created`);
};
def({ name: 'DIVIDE', aliases: ['DIV'], title: 'Divide', icon: 'divide', desc: 'Creates evenly spaced point objects or blocks along the length or perimeter of an object.', pickfirst: false, run: markAlong(true) });
def({ name: 'MEASURE', aliases: ['ME'], title: 'Measure', icon: 'measure', desc: 'Creates point objects or blocks at measured intervals along the length or perimeter of an object.', pickfirst: false, run: markAlong(false) });

// ---------------------------------------------------------------- clipboard

def({
  name: 'COPYCLIP', title: 'Copy', icon: 'copyclip', desc: 'Copies selected objects to the clipboard.', repeatable: false,
  async run(c) {
    const sel = await c.select({ prompt: SELECT }); if (!sel.length) return;
    let b = null; for (const e of sel) b = g.unionBounds(b, boundsOf(e, c.doc));
    c.app.clipboard = { entities: sel.map(e => clone(e)), base: { x: b.minX, y: b.minY } };
    c.app.emit('clipboard', c.app.clipboard);
    c.log(`${sel.length} object${sel.length === 1 ? '' : 's'} copied to the clipboard`);
    c.editor.tempSel = new Set();
  },
});
def({
  name: 'CUTCLIP', title: 'Cut', icon: 'cutclip', desc: 'Copies selected objects to the clipboard and removes them from the drawing.', repeatable: false,
  async run(c) {
    const sel = await c.select({ prompt: SELECT }); if (!sel.length) return;
    let b = null; for (const e of sel) b = g.unionBounds(b, boundsOf(e, c.doc));
    c.app.clipboard = { entities: sel.map(e => clone(e)), base: { x: b.minX, y: b.minY } };
    c.doc.removeAll(sel); c.clearSel();
  },
});
def({
  name: 'PASTECLIP', title: 'Paste', icon: 'paste', desc: 'Inserts data from the clipboard into the drawing.', repeatable: false,
  async run(c) {
    const clip = c.app.clipboard;
    if (!clip?.entities.length) { c.log('The clipboard is empty.', 'error'); return; }
    const p = await c.point({ prompt: 'Specify insertion point', preview: pt => transformedClones(clip.entities, M.translate(pt.x - clip.base.x, pt.y - clip.base.y), c.doc) });
    if (!p.pt) return;
    const made = addCopies(c.doc, clip.entities, M.translate(p.pt.x - clip.base.x, p.pt.y - clip.base.y));
    c.editor.clearSel(); c.editor.select(made);
  },
});

// ---------------------------------------------------------------- grip editing (internal)

const GRIP_MODES = ['STRETCH', 'MOVE', 'ROTATE', 'SCALE', 'MIRROR'];

registry.define({
  name: '_GRIPEDIT', silent: true, repeatable: false, group: 'Internal',
  async run(c, { grip }) {
    const ed = c.editor, hot = grip.grip, orig = grip.entity;
    const sels = ed.selected;
    let mode = hot.kind === 'move' || hot.kind === 'radius' ? (hot.kind === 'move' ? 'MOVE' : 'STRETCH') : 'STRETCH';
    if (hot.kind === 'move') mode = 'MOVE';
    let base = hot.pt, copy = false;
    const shared = e => gripsOf(e, c.doc).filter(q => g.dist(q.pt, hot.pt) < 1e-7 * Math.max(1, Math.abs(hot.pt.x)));
    for (;;) {
      const opts = ['Base point', 'Copy', 'Undo', 'eXit'];
      const rotMode = mode === 'ROTATE', scaleMode = mode === 'SCALE';
      const label = `** ${mode} ${copy ? '(multiple) ' : ''}**\n${mode === 'STRETCH' ? 'Specify stretch point' : mode === 'MOVE' ? 'Specify move point' : mode === 'ROTATE' ? 'Specify rotation angle' : mode === 'SCALE' ? 'Specify scale factor' : 'Specify second point'}`;
      const result = rotMode ? await c.angle({ prompt: label, options: opts, base, default: 0, allowEnter: true, preview: pt => gripPreview(pt) })
        : scaleMode ? await c.distance({ prompt: label, options: opts, base, default: 1, allowEnter: true, preview: pt => gripPreview(pt) })
          : await c.point({ prompt: label, options: opts, base: mode === 'MOVE' || mode === 'MIRROR' ? base : hot.pt, allowEnter: true, preview: pt => gripPreview(pt) });
      if (result.kw === 'eXit') return;
      if (result.kw === 'Copy') { copy = true; continue; }
      if (result.kw === 'Base point') { const b = await c.point({ prompt: 'Specify base point', allowEnter: false }); if (b.pt) base = b.pt; continue; }
      if (result.kw === 'Undo') continue;
      if (result.enter && result.pt === undefined && result.value === undefined) { mode = GRIP_MODES[(GRIP_MODES.indexOf(mode) + 1) % GRIP_MODES.length]; continue; }
      if (result.enter && (result.value !== undefined || result.pt)) { mode = GRIP_MODES[(GRIP_MODES.indexOf(mode) + 1) % GRIP_MODES.length]; continue; }
      const pt = result.pt;
      applyGrip(result);
      if (!copy) return;
      void pt;
    }
    function matrixFor(r) {
      if (mode === 'MOVE') return M.translate(r.pt.x - base.x, r.pt.y - base.y);
      if (mode === 'ROTATE') return M.rotate(r.pt ? g.angleOf(base, r.pt) : r.value, base);
      if (mode === 'SCALE') { const f = r.pt ? g.dist(base, r.pt) / Math.max(1e-12, g.dist(base, hot.pt)) : r.value; return M.scale(Math.max(1e-9, f), Math.max(1e-9, f), base); }
      if (mode === 'MIRROR') return M.mirror(base, r.pt);
      return null;
    }
    function gripPreview(pt) {
      const r = { pt, value: pt && g.dist(base, pt) };
      if (mode === 'STRETCH') return sels.map(e => { const gr = shared(e)[0]; return gr ? gripEdit(e, gr.id, pt, c.doc) : e; });
      if (!pt) return [];
      const m = matrixFor({ pt, value: r.value });
      return m ? transformedClones(sels, m, c.doc) : [];
    }
    function applyGrip(r) {
      if (mode === 'STRETCH') {
        c.doc.change(() => {
          for (const e of sels) {
            const gr = shared(e)[0]; if (!gr) continue;
            const ne = gripEdit(e, gr.id, r.pt, c.doc);
            if (copy) { const n = clone(ne); c.doc.add(n); } else { const keep = e.id; Object.assign(e, ne); e.id = keep; }
          }
        });
        return;
      }
      const m = matrixFor(r);
      if (!m) return;
      if (copy) addCopies(c.doc, sels, m); else applyTransform(c.doc, sels, m);
    }
    void orig;
  },
});

