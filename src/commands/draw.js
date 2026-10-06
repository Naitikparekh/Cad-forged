// Draw commands: LINE, PLINE, CIRCLE, ARC, RECTANG, POLYGON, ELLIPSE, SPLINE, POINT, XLINE, RAY, DONUT.
import * as g from '../core/geom.js';
import { E } from '../core/entities.js';
import { registry } from '../engine/registry.js';
import { filletPline, chamferPline } from '../ops/fillet.js';
import { memory, rectVerts, tangentBulge, plinePreview, num } from './helpers.js';
import { segsOf } from '../core/entities.js';

const def = d => registry.define({ group: 'Draw', ...d });

// ---------------------------------------------------------------- LINE

def({
  name: 'LINE', aliases: ['L'], title: 'Line', icon: 'line', desc: 'Creates straight line segments.',
  async run(c) {
    const mem = memory(c.app);
    let r = await c.point({ prompt: 'Specify first point', allowEnter: true });
    let start;
    if (r.enter) { if (!mem.lastLineEnd) return; start = mem.lastLineEnd; } else if (r.pt) start = r.pt; else return;
    const made = [];
    let prev = start;
    for (;;) {
      const options = made.length >= 2 ? ['Close', 'Undo'] : made.length ? ['Undo'] : [];
      r = await c.point({ prompt: 'Specify next point', base: prev, options, allowEnter: true, preview: pt => [E.line(prev, pt)] });
      if (r.kw === 'Undo') { if (made.length) { c.doc.remove(made.pop()); prev = made.length ? made[made.length - 1].b : start; } continue; }
      if (r.kw === 'Close') { if (!g.samePt(prev, start)) c.add(E.line(prev, start)); prev = start; break; }
      if (!r.pt) break;
      if (g.samePt(prev, r.pt)) continue;
      made.push(c.add(E.line(prev, r.pt)));
      prev = r.pt;
    }
    mem.lastLineEnd = prev;
  },
});

// ---------------------------------------------------------------- PLINE

function arcBulge(mode, p0, dir, input, p1) {
  // Returns bulge for an arc segment from p0 to p1.
  switch (mode) {
    case 'tangent': return tangentBulge(p0, dir, p1);
    case 'angle': return Math.tan(input.angle / 4);
    case 'direction': return tangentBulge(p0, input.dir, p1);
    case 'second': { const a = g.arcThrough(p0, input.second, p1); return a ? Math.tan(a.sw / 4) : 0; }
    case 'radius': {
      const chord = g.dist(p0, p1), R = Math.abs(input.radius);
      if (chord > 2 * R) return null;
      const half = 2 * Math.asin(chord / (2 * R)), sweep = input.radius < 0 ? g.TAU - half : half;
      return Math.tan(sweep / 4);
    }
    default: return 0;
  }
}

def({
  name: 'PLINE', aliases: ['PL'], title: 'Polyline', icon: 'pline', desc: 'Creates a 2D polyline: a single object made of connected line and arc segments.',
  async run(c) {
    let r = await c.point({ prompt: 'Specify start point' });
    if (!r.pt) return;
    const verts = [{ x: r.pt.x, y: r.pt.y, b: 0 }];
    let arcMode = false, dir = { x: 1, y: 0 }, closed = false;
    const cur = () => verts[verts.length - 1];
    const push = (p, bulge) => { cur().b = bulge; verts.push({ x: p.x, y: p.y, b: 0 }); dir = bulge ? endTangent(verts[verts.length - 2], p, bulge) : g.unit(g.sub(p, verts[verts.length - 2])); };
    const endTangent = (p0, p1, b) => { const a = g.bulgeSeg(p0, p1, b); return g.segTangent(a, 1); };
    const preview = (bulge) => pt => { const vs = verts.map(v => ({ ...v })); vs[vs.length - 1].b = bulge(pt); return plinePreview(vs, pt); };
    for (;;) {
      if (!arcMode) {
        const opts = ['Arc'].concat(verts.length > 2 ? ['Close'] : [], ['Length', 'Undo']);
        r = await c.point({ prompt: 'Specify next point', options: opts, base: cur(), allowEnter: true, preview: preview(() => 0) });
        if (r.kw === 'Arc') { arcMode = true; continue; }
        if (r.kw === 'Close') { closed = true; break; }
        if (r.kw === 'Undo') { if (verts.length > 1) { verts.pop(); cur().b = 0; if (verts.length > 1) dir = g.unit(g.sub(cur(), verts[verts.length - 2])); } continue; }
        if (r.kw === 'Length') {
          const len = await c.distance({ prompt: 'Specify length of line', min: 0, nonzero: true });
          if (len.value) { const a = verts.length > 1 ? dir : { x: 1, y: 0 }; push(g.add(cur(), g.mul(a, len.value)), 0); }
          continue;
        }
        if (!r.pt) break;
        push(r.pt, 0);
      } else {
        const opts = ['Angle', 'CEnter', verts.length > 2 ? 'CLose' : null, 'Direction', 'Line', 'Radius', 'Second pt', 'Undo'].filter(Boolean);
        r = await c.point({ prompt: 'Specify endpoint of arc', options: opts, base: cur(), allowEnter: true, preview: preview(pt => tangentBulge(cur(), dir, pt)) });
        if (r.kw === 'Line') { arcMode = false; continue; }
        if (r.kw === 'CLose') { const p0 = cur(); const b = tangentBulge(p0, dir, verts[0]); cur().b = b; closed = true; break; }
        if (r.kw === 'Undo') { if (verts.length > 1) { verts.pop(); cur().b = 0; } continue; }
        if (r.kw === 'Angle') {
          const a = await c.angle({ prompt: 'Specify included angle' });
          if (a.value === undefined) continue;
          const e = await c.point({ prompt: 'Specify endpoint of arc', base: cur(), preview: preview(() => Math.tan(a.value / 4)) });
          if (e.pt) push(e.pt, Math.tan(a.value / 4));
          continue;
        }
        if (r.kw === 'CEnter') {
          const cc = await c.point({ prompt: 'Specify center point of arc', base: cur() });
          if (!cc.pt) continue;
          const rad = g.dist(cc.pt, cur());
          const calc = pt => { const a0 = g.angleOf(cc.pt, cur()), a1 = g.angleOf(cc.pt, pt); return { end: g.polar(cc.pt, a1, rad), bulge: Math.tan(g.sweepCCW(a0, a1) / 4) }; };
          const e = await c.point({ prompt: 'Specify endpoint of arc', base: cc.pt, preview: pt => { const k = calc(pt); const vs = verts.map(v => ({ ...v })); vs[vs.length - 1].b = k.bulge; return plinePreview(vs, k.end); } });
          if (e.pt) { const k = calc(e.pt); push(k.end, k.bulge); }
          continue;
        }
        if (r.kw === 'Direction') {
          const d = await c.point({ prompt: 'Specify the tangent direction for the start point of the arc', base: cur() });
          if (!d.pt) continue;
          const dd = g.unit(g.sub(d.pt, cur()));
          const e = await c.point({ prompt: 'Specify endpoint of arc', base: cur(), preview: preview(pt => tangentBulge(cur(), dd, pt)) });
          if (e.pt) push(e.pt, tangentBulge(cur(), dd, e.pt));
          continue;
        }
        if (r.kw === 'Radius') {
          const rr = await c.distance({ prompt: 'Specify radius of arc', nonzero: true });
          if (rr.value === undefined) continue;
          const e = await c.point({ prompt: 'Specify endpoint of arc', base: cur(), preview: preview(pt => arcBulge('radius', cur(), dir, { radius: rr.value }, pt) ?? 0) });
          if (e.pt) { const b = arcBulge('radius', cur(), dir, { radius: rr.value }, e.pt); if (b === null) c.log('The radius is too small for that endpoint.', 'error'); else push(e.pt, b); }
          continue;
        }
        if (r.kw === 'Second pt') {
          const s = await c.point({ prompt: 'Specify second point on arc', base: cur() });
          if (!s.pt) continue;
          const e = await c.point({ prompt: 'Specify end point of arc', base: cur(), preview: pt => { const vs = verts.map(v => ({ ...v })); vs[vs.length - 1].b = arcBulge('second', cur(), dir, { second: s.pt }, pt); return plinePreview(vs, pt); } });
          if (e.pt) push(e.pt, arcBulge('second', cur(), dir, { second: s.pt }, e.pt));
          continue;
        }
        if (!r.pt) break;
        push(r.pt, tangentBulge(cur(), dir, r.pt));
      }
    }
    if (verts.length > 1) c.add(E.pline(verts, closed));
  },
});

// ---------------------------------------------------------------- CIRCLE

const pickCurveSegs = async (c, prompt) => {
  const r = await c.entity({ prompt, filter: e => ['line', 'circle', 'arc', 'pline'].includes(e.type) });
  if (!r.entity) return null;
  const pick = r.pt;
  const segs = segsOf(r.entity, c.doc);
  let best = null, bd = Infinity;
  for (const s of segs) { const n = g.segNearest(s, pick); if (n.d < bd) { bd = n.d; best = s; } }
  return best ? { seg: best, pick } : null;
};

def({
  name: 'CIRCLE', aliases: ['C'], title: 'Circle', icon: 'circle', desc: 'Creates a circle.',
  async run(c, args) {
    const mem = memory(c.app);
    let mode = args.mode || null, center;
    if (!mode) {
      const r = await c.point({ prompt: 'Specify center point for circle', options: ['3P', '2P', 'Ttr (tan tan radius)'] });
      if (r.kw) mode = r.kw === 'Ttr (tan tan radius)' ? 'TTR' : r.kw; else if (r.pt) { center = r.pt; mode = 'CR'; } else return;
    } else if (mode === 'CR' || mode === 'CD') {
      const r = await c.point({ prompt: 'Specify center point for circle' });
      if (!r.pt) return; center = r.pt;
    }
    if (mode === '3P') {
      const a = await c.point({ prompt: 'Specify first point on circle' }); if (!a.pt) return;
      const b = await c.point({ prompt: 'Specify second point on circle', base: a.pt, preview: pt => [E.line(a.pt, pt)] }); if (!b.pt) return;
      const d = await c.point({ prompt: 'Specify third point on circle', base: b.pt, preview: pt => { const k = g.circleThrough(a.pt, b.pt, pt); return k ? [E.circle(k.c, k.r)] : []; } }); if (!d.pt) return;
      const k = g.circleThrough(a.pt, b.pt, d.pt);
      if (!k) { c.log('The three points are collinear.', 'error'); return; }
      mem.circleRadius = k.r; c.add(E.circle(k.c, k.r)); return;
    }
    if (mode === '2P') {
      const a = await c.point({ prompt: 'Specify first end point of circle\'s diameter' }); if (!a.pt) return;
      const b = await c.point({ prompt: 'Specify second end point of circle\'s diameter', base: a.pt, preview: pt => g.dist(a.pt, pt) > 0 ? [E.circle(g.mid(a.pt, pt), g.dist(a.pt, pt) / 2)] : [] }); if (!b.pt) return;
      if (g.samePt(a.pt, b.pt)) return;
      mem.circleRadius = g.dist(a.pt, b.pt) / 2; c.add(E.circle(g.mid(a.pt, b.pt), mem.circleRadius)); return;
    }
    if (mode === 'TTR') {
      const p1 = await pickCurveSegs(c, 'Specify point on object for first tangent of circle'); if (!p1) return;
      const p2 = await pickCurveSegs(c, 'Specify point on object for second tangent of circle'); if (!p2) return;
      const rr = await c.distance({ prompt: 'Specify radius of circle', default: mem.circleRadius, min: 0, nonzero: true });
      const radius = rr.value; if (!radius) return;
      const centres = g.tangentCentres(p1.seg, p2.seg, radius);
      if (!centres.length) { c.log('Circle does not exist.', 'error'); return; }
      const m = g.mid(p1.pick, p2.pick);
      centres.sort((a, b) => g.dist(a, m) - g.dist(b, m));
      mem.circleRadius = radius;
      c.add(E.circle(centres[0], radius)); return;
    }
    // center + radius / diameter
    const isDia = mode === 'CD';
    const asked = await c.distance({
      prompt: isDia ? 'Specify diameter of circle' : 'Specify radius of circle', base: center, options: isDia ? [] : ['Diameter'], default: mem.circleRadius * (isDia ? 2 : 1), min: 0, nonzero: true,
      preview: pt => g.dist(center, pt) > 1e-12 ? [E.circle(center, isDia ? g.dist(center, pt) / 2 : g.dist(center, pt))] : [],
    });
    let radius;
    if (asked.kw === 'Diameter') {
      const d = await c.distance({ prompt: 'Specify diameter of circle', base: center, default: mem.circleRadius * 2, min: 0, nonzero: true, preview: pt => g.dist(center, pt) > 1e-12 ? [E.circle(center, g.dist(center, pt) / 2)] : [] });
      if (d.value === undefined) return; radius = d.value / 2;
    } else if (asked.value === undefined) return;
    else radius = isDia ? asked.value / 2 : asked.value;
    mem.circleRadius = radius;
    c.add(E.circle(center, radius));
  },
});

// ---------------------------------------------------------------- ARC

const arcEntity = (c, r, a0, a1) => E.arc(c, r, a0, a1);
/** Arc given center, start point and a ray toward the end (CCW). */
const arcCSE = (center, start, endRay) => arcEntity(center, g.dist(center, start), g.angleOf(center, start), g.angleOf(center, endRay));
/** CCW arc by center, start point and included angle (negative = CW). */
const arcCSA = (center, start, ang) => {
  const a0 = g.angleOf(center, start), r = g.dist(center, start);
  return ang >= 0 ? arcEntity(center, r, a0, a0 + ang) : arcEntity(center, r, a0 + ang, a0);
};
const arcSEA = (s, e, ang) => {       // start, end, included angle
  const chord = g.dist(s, e);
  if (chord < 1e-12 || Math.abs(ang) < 1e-9 || Math.abs(ang) >= g.TAU) return null;
  const R = chord / (2 * Math.sin(Math.abs(ang) / 2)), m = g.mid(s, e), u = g.unit(g.sub(e, s));
  const h = Math.sqrt(Math.max(0, R * R - chord * chord / 4)) * (Math.abs(ang) > Math.PI ? -1 : 1);
  const side = ang > 0 ? 1 : -1;                         // CCW arcs from s to e have their centre on the left
  const center = g.add(m, g.mul(g.perp(u), h * side));
  const sw = ang;
  return sw >= 0 ? arcEntity(center, R, g.angleOf(center, s), g.angleOf(center, s) + sw) : arcEntity(center, R, g.angleOf(center, s) + sw, g.angleOf(center, s));
};
const arcSED = (s, e, dir) => {       // start, end, tangent direction at start
  const b = tangentBulge(s, dir, e), a = g.bulgeSeg(s, e, b);
  return a.t === 'a' ? E.arcSeg(a) : null;
};
const arcSER = (s, e, R) => {
  const chord = g.dist(s, e), r = Math.abs(R);
  if (chord > 2 * r || chord < 1e-12) return null;
  const half = 2 * Math.asin(chord / (2 * r)), sweep = R < 0 ? g.TAU - half : half;
  return arcSEA(s, e, sweep);
};

def({
  name: 'ARC', aliases: ['A'], title: 'Arc', icon: 'arc', desc: 'Creates an arc.',
  async run(c, args) {
    const mode = args.mode || '3p';
    const keep = e => e && c.add(e);
    const P = (prompt, extra = {}) => c.point({ prompt, ...extra });
    if (mode === 'continue') {
      const last = [...c.doc.entities].reverse().find(e => e.type === 'line' || e.type === 'arc');
      if (!last) { c.log('No line or arc to continue from.', 'error'); return; }
      const s = last.type === 'line' ? last.b : g.polar(last.c, last.a1, last.r);
      const dir = last.type === 'line' ? g.unit(g.sub(last.b, last.a)) : { x: -Math.sin(last.a1), y: Math.cos(last.a1) };
      const e = await P('Specify end point of arc', { base: s, preview: pt => { const x = arcSED(s, pt, dir); return x ? [x] : []; } });
      if (e.pt) keep(arcSED(s, e.pt, dir));
      return;
    }
    // center-first variants
    if (['cse', 'csa', 'csl'].includes(mode)) {
      const ctr = await P('Specify center point of arc'); if (!ctr.pt) return;
      const s = await P('Specify start point of arc', { base: ctr.pt, preview: pt => [E.line(ctr.pt, pt)] }); if (!s.pt) return;
      return finishCenter(c, ctr.pt, s.pt, mode.slice(1), keep);
    }
    // start-first variants
    let s = await P('Specify start point of arc', { options: mode === '3p' ? ['Center'] : [] });
    if (s.kw === 'Center') {
      const ctr = await P('Specify center point of arc'); if (!ctr.pt) return;
      const st = await P('Specify start point of arc', { base: ctr.pt, preview: pt => [E.line(ctr.pt, pt)] }); if (!st.pt) return;
      return finishCenter(c, ctr.pt, st.pt, 'se', keep);
    }
    if (!s.pt) return;
    const start = s.pt;
    if (mode === '3p') {
      const second = await P('Specify second point of arc', { options: ['Center', 'End'], base: start, preview: pt => [E.line(start, pt)] });
      if (second.kw === 'Center') {
        const ctr = await P('Specify center point of arc', { base: start }); if (!ctr.pt) return;
        return finishCenter(c, ctr.pt, start, 'se', keep);
      }
      if (second.kw === 'End') return startEnd(c, start, null, keep);
      if (!second.pt) return;
      const end = await P('Specify end point of arc', { base: second.pt, preview: pt => { const a = g.arcThrough(start, second.pt, pt); return a ? [E.arcSeg(a)] : [E.line(start, pt)]; } });
      if (!end.pt) return;
      const a = g.arcThrough(start, second.pt, end.pt);
      if (!a) { c.log('The three points are collinear.', 'error'); return; }
      keep(E.arcSeg(a)); return;
    }
    if (mode === 'sce' || mode === 'sca' || mode === 'scl') {
      const ctr = await P('Specify center point of arc', { base: start }); if (!ctr.pt) return;
      return finishCenter(c, ctr.pt, start, mode.slice(1), keep);
    }
    // start, end ...
    const end = await P('Specify end point of arc', { base: start, preview: pt => [E.line(start, pt)] });
    if (!end.pt) return;
    return startEnd(c, start, end.pt, keep, mode);
  },
});

async function finishCenter(c, ctr, start, kind, keep) {
  const r = g.dist(ctr, start);
  if (kind === 'se' || kind === 'ce') {          // end point
    const e = await c.point({ prompt: 'Specify end point of arc', options: ['Angle', 'chord Length'], base: ctr, preview: pt => g.dist(ctr, pt) > 1e-12 ? [arcCSE(ctr, start, pt)] : [] });
    if (e.kw === 'Angle') return finishCenter(c, ctr, start, 'ca', keep);
    if (e.kw === 'chord Length') return finishCenter(c, ctr, start, 'cl', keep);
    if (e.pt) keep(arcCSE(ctr, start, e.pt));
  } else if (kind === 'ca' || kind === 'sa') {   // included angle
    const a = await c.angle({ prompt: 'Specify included angle', base: ctr, preview: undefined });
    if (a.value !== undefined) keep(arcCSA(ctr, start, a.value));
  } else if (kind === 'cl' || kind === 'sl') {   // chord length
    const L = await c.distance({ prompt: 'Specify length of chord', min: 0, nonzero: true });
    if (L.value === undefined) return;
    if (L.value > 2 * r) { c.log('The chord is longer than the diameter.', 'error'); return; }
    keep(arcCSA(ctr, start, 2 * Math.asin(L.value / (2 * r))));
  }
}

async function startEnd(c, start, end, keep, mode = 'se') {
  if (!end) { const e = await c.point({ prompt: 'Specify end point of arc', base: start, preview: pt => [E.line(start, pt)] }); if (!e.pt) return; end = e.pt; }
  if (mode === 'sea') { const a = await c.angle({ prompt: 'Specify included angle' }); if (a.value !== undefined) keep(arcSEA(start, end, a.value)); return; }
  if (mode === 'sed') { const d = await c.point({ prompt: 'Specify tangent direction for the start point of arc', base: start }); if (d.pt) keep(arcSED(start, end, g.unit(g.sub(d.pt, start)))); return; }
  if (mode === 'ser') { const R = await c.distance({ prompt: 'Specify radius of arc', nonzero: true }); if (R.value !== undefined) { const a = arcSER(start, end, R.value); if (a) keep(a); else c.log('The radius is too small.', 'error'); } return; }
  const r = await c.point({ prompt: 'Specify center point of arc', options: ['Angle', 'Direction', 'Radius'], base: start, preview: pt => { const a = g.arcThrough(start, pt, end); void a; return [E.line(start, end)]; } });
  if (r.kw === 'Angle') return startEnd(c, start, end, keep, 'sea');
  if (r.kw === 'Direction') return startEnd(c, start, end, keep, 'sed');
  if (r.kw === 'Radius') return startEnd(c, start, end, keep, 'ser');
  if (r.pt) keep(arcCSE(r.pt, start, end));
}

// ---------------------------------------------------------------- RECTANG

def({
  name: 'RECTANG', aliases: ['REC', 'RECTANGLE'], title: 'Rectangle', icon: 'rectangle', desc: 'Creates a rectangular polyline.',
  async run(c) {
    const mem = memory(c.app);
    let fillet = mem.rectFillet, chamfer = mem.rectChamfer;
    let first;
    for (;;) {
      const r = await c.point({ prompt: 'Specify first corner point', options: ['Chamfer', 'Fillet'], allowEnter: false });
      if (r.kw === 'Chamfer') {
        const a = await c.distance({ prompt: 'Specify first chamfer distance for rectangles', default: chamfer[0], min: 0 });
        const b = await c.distance({ prompt: 'Specify second chamfer distance for rectangles', default: a.value ?? chamfer[1], min: 0 });
        if (a.value !== undefined && b.value !== undefined) { chamfer = [a.value, b.value]; fillet = 0; mem.rectChamfer = chamfer; mem.rectFillet = 0; }
        continue;
      }
      if (r.kw === 'Fillet') { const f = await c.distance({ prompt: 'Specify fillet radius for rectangles', default: fillet, min: 0 }); if (f.value !== undefined) { fillet = f.value; chamfer = [0, 0]; mem.rectFillet = fillet; mem.rectChamfer = [0, 0]; } continue; }
      if (!r.pt) return;
      first = r.pt; break;
    }
    let rot = 0;
    const build = (a, b) => {
      const base = E.pline(rectVerts(a, b, rot), true);
      if (fillet > 0) { const r = filletPline(base, fillet, c.doc); if (r.entity) return r.entity; }
      if (chamfer[0] > 0 || chamfer[1] > 0) { const r = chamferPline(base, chamfer[0], chamfer[1], c.doc); if (r.entity) return r.entity; }
      return base;
    };
    for (;;) {
      const r = await c.corner({ prompt: 'Specify other corner point', options: ['Area', 'Dimensions', 'Rotation'], base: first, preview: pt => g.dist(first, pt) > 1e-12 ? [build(first, pt)] : [] });
      if (r.kw === 'Rotation') {
        const a = await c.angle({ prompt: 'Specify rotation angle or [Pick points]', default: rot, base: first });
        if (a.value !== undefined) rot = a.value; continue;
      }
      if (r.kw === 'Dimensions') {
        const L = await c.distance({ prompt: 'Specify length for rectangles', default: 10, nonzero: true });
        const W = await c.distance({ prompt: 'Specify width for rectangles', default: 10, nonzero: true });
        if (L.value === undefined || W.value === undefined) return;
        const side = await c.point({ prompt: 'Specify other corner point or [Dimensions]', base: first, preview: pt => { const sx = pt.x >= first.x ? 1 : -1, sy = pt.y >= first.y ? 1 : -1; return [build(first, { x: first.x + sx * L.value, y: first.y + sy * W.value })]; } });
        if (!side.pt) return;
        const sx = side.pt.x >= first.x ? 1 : -1, sy = side.pt.y >= first.y ? 1 : -1;
        c.add(build(first, { x: first.x + sx * L.value, y: first.y + sy * W.value })); return;
      }
      if (r.kw === 'Area') {
        const area = await c.number({ prompt: 'Enter area of rectangle', min: 0, nonzero: true, default: 100 });
        const how = await c.keyword({ prompt: 'Calculate rectangle dimensions based on [Length/Width]', options: ['Length', 'Width'], default: 'Length' });
        const v = await c.distance({ prompt: how.kw === 'Width' ? 'Enter rectangle width' : 'Enter rectangle length', default: 10, nonzero: true });
        if (area.value === undefined || v.value === undefined) return;
        const other = area.value / v.value;
        const [L, W] = how.kw === 'Width' ? [other, v.value] : [v.value, other];
        const side = await c.point({ prompt: 'Specify direction', base: first, preview: pt => { const sx = pt.x >= first.x ? 1 : -1, sy = pt.y >= first.y ? 1 : -1; return [build(first, { x: first.x + sx * L, y: first.y + sy * W })]; } });
        if (!side.pt) return;
        const sx = side.pt.x >= first.x ? 1 : -1, sy = side.pt.y >= first.y ? 1 : -1;
        c.add(build(first, { x: first.x + sx * L, y: first.y + sy * W })); return;
      }
      if (!r.pt) return;
      if (g.dist(first, r.pt) < 1e-12) return;
      c.add(build(first, r.pt)); return;
    }
  },
});

// ---------------------------------------------------------------- POLYGON

const polygonVerts = (center, n, r, ang, circumscribed) => {
  const R = circumscribed ? r / Math.cos(Math.PI / n) : r;
  return Array.from({ length: n }, (_, i) => ({ ...g.polar(center, ang + i * g.TAU / n, R), b: 0 }));
};

def({
  name: 'POLYGON', aliases: ['POL'], title: 'Polygon', icon: 'polygon', desc: 'Creates an equilateral closed polyline.',
  async run(c) {
    const mem = memory(c.app);
    const nSides = await c.number({ prompt: 'Enter number of sides', default: mem.polygonSides, integer: true, min: 3, max: 1024, rangeMessage: 'Requires an integer between 3 and 1024.' });
    if (nSides.value === undefined) return;
    const n = nSides.value; mem.polygonSides = n;
    const r = await c.point({ prompt: 'Specify center of polygon', options: ['Edge'] });
    if (r.kw === 'Edge') {
      const a = await c.point({ prompt: 'Specify first endpoint of edge' }); if (!a.pt) return;
      const b = await c.point({ prompt: 'Specify second endpoint of edge', base: a.pt, preview: pt => edge(a.pt, pt) }); if (!b.pt) return;
      function edge(p, q) {
        const L = g.dist(p, q); if (L < 1e-12) return [];
        const R = L / (2 * Math.sin(Math.PI / n)), mid = g.mid(p, q), h = L / (2 * Math.tan(Math.PI / n));
        const center = g.add(mid, g.mul(g.perp(g.unit(g.sub(q, p))), h));
        const a0 = g.angleOf(center, p);
        return [E.pline(Array.from({ length: n }, (_, i) => ({ ...g.polar(center, a0 - i * g.TAU / n, R), b: 0 })), true)];
      }
      c.add(edge(a.pt, b.pt)[0]); return;
    }
    if (!r.pt) return;
    const center = r.pt;
    const how = await c.keyword({ prompt: 'Enter an option [Inscribed in circle/Circumscribed about circle]', options: ['Inscribed in circle', 'Circumscribed about circle'], default: mem.polygonMode === 'Circumscribed' ? 'Circumscribed about circle' : 'Inscribed in circle' });
    const circ = how.kw === 'Circumscribed about circle'; mem.polygonMode = circ ? 'Circumscribed' : 'Inscribed';
    const build = pt => { const rr = g.dist(center, pt); if (rr < 1e-12) return []; const ang = g.angleOf(center, pt) - (circ ? Math.PI / n : 0); return [E.pline(polygonVerts(center, n, rr, ang, circ), true)]; };
    const rad = await c.point({ prompt: 'Specify radius of circle', base: center, preview: build });
    if (!rad.pt) return;
    const e = build(rad.pt)[0]; if (e) c.add(e);
  },
});

// ---------------------------------------------------------------- ELLIPSE

def({
  name: 'ELLIPSE', aliases: ['EL'], title: 'Ellipse', icon: 'ellipse', desc: 'Creates an ellipse or an elliptical arc.',
  async run(c, args) {
    let mode = args.mode || null, arc = args.arc || false, center = null, axisEnd = null;
    if (!mode) {
      const r = await c.point({ prompt: 'Specify axis endpoint of ellipse', options: ['Arc', 'Center'] });
      if (r.kw === 'Arc') { arc = true; const r2 = await c.point({ prompt: 'Specify axis endpoint of elliptical arc', options: ['Center'] }); if (r2.kw === 'Center') mode = 'center'; else if (r2.pt) axisEnd = r2.pt; else return; }
      else if (r.kw === 'Center') mode = 'center'; else if (r.pt) axisEnd = r.pt; else return;
    } else if (mode === 'center') arc = !!args.arc;
    let m, ratio;
    const ell = (pt, t0 = 0, t1 = g.TAU) => { const L = g.dist(center, pt); return L < 1e-12 ? null : E.ellipse(center, m, Math.min(1, L / g.len(m)), t0, t1); };
    if (mode === 'center') {
      const cc = await c.point({ prompt: 'Specify center of ellipse' }); if (!cc.pt) return; center = cc.pt;
      const ae = await c.point({ prompt: 'Specify endpoint of axis', base: center, preview: pt => [E.line(center, pt)] }); if (!ae.pt) return;
      m = g.sub(ae.pt, center);
    } else {
      const other = await c.point({ prompt: 'Specify other endpoint of axis', base: axisEnd, preview: pt => [E.line(axisEnd, pt)] }); if (!other.pt) return;
      center = g.mid(axisEnd, other.pt); m = g.sub(other.pt, center);
    }
    if (g.len(m) < 1e-12) return;
    const d = await c.point({ prompt: 'Specify distance to other axis', options: ['Rotation'], base: center, preview: pt => { const e = ell(pt); return e ? [e] : []; } });
    if (d.kw === 'Rotation') {
      const a = await c.angle({ prompt: 'Specify rotation around major axis', base: center });
      if (a.value === undefined) return; ratio = Math.cos(a.value);
    } else if (d.pt) { const n = g.unit({ x: -m.y, y: m.x }); ratio = Math.abs(g.dot(g.sub(d.pt, center), n)) / g.len(m); } else return;
    ratio = Math.max(1e-4, Math.min(1, ratio));
    if (!arc) { c.add(E.ellipse(center, m, ratio)); return; }
    const major = Math.atan2(m.y, m.x), param = pt => { const v = g.sub(pt, center), x = g.dot(v, g.unit(m)) / g.len(m), y = g.dot(v, g.unit({ x: -m.y, y: m.x })) / (g.len(m) * ratio); return Math.atan2(y, x); };
    void major;
    const s = await c.point({ prompt: 'Specify start angle', options: ['Parameter'], base: center, preview: pt => [E.line(center, pt)] }); if (!s.pt) return;
    const t0 = param(s.pt);
    const e = await c.point({ prompt: 'Specify end angle', base: center, preview: pt => [E.ellipse(center, m, ratio, t0, g.normAngle(param(pt)))] }); if (!e.pt) return;
    c.add(E.ellipse(center, m, ratio, g.normAngle(t0), g.normAngle(param(e.pt))));
  },
});

// ---------------------------------------------------------------- SPLINE

def({
  name: 'SPLINE', aliases: ['SPL'], title: 'Spline', icon: 'spline', desc: 'Creates a smooth curve through fit points.',
  async run(c) {
    const first = await c.point({ prompt: 'Specify first point' });
    if (!first.pt) return;
    const fit = [first.pt];
    let closed = false;
    for (;;) {
      const r = await c.point({ prompt: 'Enter next point', options: fit.length > 2 ? ['Undo', 'Close'] : ['Undo'], base: fit[fit.length - 1], allowEnter: true, preview: pt => [E.spline([...fit, pt], false)] });
      if (r.kw === 'Undo') { if (fit.length > 1) fit.pop(); continue; }
      if (r.kw === 'Close') { closed = true; break; }
      if (!r.pt) break;
      fit.push(r.pt);
    }
    if (fit.length > 1) c.add(E.spline(fit, closed));
  },
});

// ---------------------------------------------------------------- POINT / XLINE / RAY / DONUT

def({
  name: 'POINT', aliases: ['PO'], title: 'Point', icon: 'point', desc: 'Creates a point object.',
  async run(c, args) {
    c.log(`Current point modes: PDMODE=${c.var('PDMODE')}  PDSIZE=${num(c.var('PDSIZE'))}`);
    for (;;) {
      const r = await c.point({ prompt: 'Specify a point', allowEnter: !!args.multiple });
      if (!r.pt) return;
      c.add(E.point(r.pt));
      if (!args.multiple) return;
    }
  },
});

def({
  name: 'XLINE', aliases: ['XL'], title: 'Construction Line', icon: 'xline', desc: 'Creates a line of infinite length.',
  async run(c) {
    let mode = 'point', angle = 0, p0 = null;
    for (;;) {
      if (mode === 'point') {
        const r = await c.point({ prompt: p0 ? 'Specify through point' : 'Specify a point', options: p0 ? [] : ['Hor', 'Ver', 'Ang', 'Bisect', 'Offset'], allowEnter: true, base: p0, noOrtho: !p0, preview: pt => p0 && !g.samePt(p0, pt) ? [E.xline(p0, g.sub(pt, p0))] : [] });
        if (r.kw === 'Hor') { mode = 'dir'; angle = 0; continue; }
        if (r.kw === 'Ver') { mode = 'dir'; angle = Math.PI / 2; continue; }
        if (r.kw === 'Ang') { const a = await c.angle({ prompt: 'Enter angle of xline (0) or [Reference]', default: 0 }); if (a.value === undefined) return; angle = a.value; mode = 'dir'; continue; }
        if (r.kw === 'Bisect') {
          const v = await c.point({ prompt: 'Specify angle vertex point' }); if (!v.pt) return;
          const s = await c.point({ prompt: 'Specify angle start point', base: v.pt }); if (!s.pt) return;
          const e = await c.point({ prompt: 'Specify angle end point', base: v.pt, allowEnter: true, preview: pt => [E.xline(v.pt, g.add(g.unit(g.sub(s.pt, v.pt)), g.unit(g.sub(pt, v.pt))))] });
          if (!e.pt) return;
          c.add(E.xline(v.pt, g.add(g.unit(g.sub(s.pt, v.pt)), g.unit(g.sub(e.pt, v.pt))))); return;
        }
        if (r.kw === 'Offset') { c.log('Use the OFFSET command on a construction line.'); return; }
        if (!r.pt) return;
        if (!p0) { p0 = r.pt; continue; }
        c.add(E.xline(p0, g.sub(r.pt, p0)));
      } else {
        const r = await c.point({ prompt: 'Specify through point', allowEnter: true, preview: pt => [E.xline(pt, { x: Math.cos(angle), y: Math.sin(angle) })] });
        if (!r.pt) return;
        c.add(E.xline(r.pt, { x: Math.cos(angle), y: Math.sin(angle) }));
      }
    }
  },
});

def({
  name: 'RAY', title: 'Ray', icon: 'ray', desc: 'Creates a semi-infinite line.',
  async run(c) {
    const s = await c.point({ prompt: 'Specify start point' }); if (!s.pt) return;
    for (;;) {
      const r = await c.point({ prompt: 'Specify through point', base: s.pt, allowEnter: true, preview: pt => g.samePt(pt, s.pt) ? [] : [E.ray(s.pt, g.sub(pt, s.pt))] });
      if (!r.pt) return;
      if (!g.samePt(r.pt, s.pt)) c.add(E.ray(s.pt, g.sub(r.pt, s.pt)));
    }
  },
});

def({
  name: 'DONUT', aliases: ['DO'], title: 'Donut', icon: 'donut', desc: 'Creates a filled circle or a wide ring.',
  async run(c) {
    const mem = memory(c.app);
    const inner = await c.distance({ prompt: 'Specify inside diameter of donut', default: mem.donutIn, min: 0 });
    if (inner.value === undefined) return;
    const outer = await c.distance({ prompt: 'Specify outside diameter of donut', default: mem.donutOut, min: 0, nonzero: true });
    if (outer.value === undefined) return;
    mem.donutIn = inner.value; mem.donutOut = outer.value;
    if (outer.value <= inner.value) { c.log('The outside diameter must be larger than the inside diameter.', 'error'); return; }
    const loops = ctr => {
      const ring = r => r > 0 ? [{ x: ctr.x + r, y: ctr.y, b: 1 }, { x: ctr.x - r, y: ctr.y, b: 1 }] : null;
      return [ring(outer.value / 2), ring(inner.value / 2)].filter(Boolean);
    };
    for (;;) {
      const r = await c.point({ prompt: 'Specify center of donut or <exit>', allowEnter: true, preview: pt => [E.hatch(loops(pt), { pattern: 'SOLID' })] });
      if (!r.pt) return;
      c.add(E.hatch(loops(r.pt), { pattern: 'SOLID' }));
    }
  },
});

