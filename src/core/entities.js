// Entity model. Entities are plain JSON objects: { id, type, layer, color?, ltype?, lw?, lts?, ...geometry }.
//
//   line    a b                      circle  c r                      arc     c r a0 a1 (CCW)
//   pline   v:[{x,y,b}] closed       ellipse c m ratio t0 t1          spline  fit:[...] closed
//   point   p                        text    p h rot s w jh jv multi  dim     kind + defining points
//   leader  pts s h                  hatch   loops:[[{x,y,b}]] pattern ps pa
//   insert  name p sx sy rot         xline   p d                      ray     p d
//
// All geometric queries go through `segsOf`, which reduces any entity to line/arc segments.
import * as g from './geom.js';
import { M, TAU } from './geom.js';
import { textLayout, textCorners } from './text.js';
import { dimGeometry, arrowPolygon } from './dim.js';
import { hatchLines } from './hatch.js';

// ---------------------------------------------------------------- factories

const pt = p => ({ x: p.x, y: p.y });
export const E = {
  line: (a, b, o = {}) => ({ type: 'line', a: pt(a), b: pt(b), ...o }),
  circle: (c, r, o = {}) => ({ type: 'circle', c: pt(c), r, ...o }),
  arc: (c, r, a0, a1, o = {}) => ({ type: 'arc', c: pt(c), r, a0: g.normAngle(a0), a1: g.normAngle(a1), ...o }),
  /** Arc entity from a directed segment {c,r,a0,sw}; negative sweeps are flipped to CCW. */
  arcSeg: (s, o = {}) => s.sw >= 0 ? E.arc(s.c, s.r, s.a0, s.a0 + s.sw, o) : E.arc(s.c, s.r, s.a0 + s.sw, s.a0, o),
  pline: (v, closed = false, o = {}) => ({ type: 'pline', v: v.map(q => ({ x: q.x, y: q.y, b: q.b || 0 })), closed: !!closed, ...o }),
  ellipse: (c, m, ratio, t0 = 0, t1 = TAU, o = {}) => ({ type: 'ellipse', c: pt(c), m: pt(m), ratio, t0, t1, ...o }),
  spline: (fit, closed = false, o = {}) => ({ type: 'spline', fit: fit.map(pt), closed, ...o }),
  point: (p, o = {}) => ({ type: 'point', p: pt(p), ...o }),
  text: (p, h, s, o = {}) => ({ type: 'text', p: pt(p), h, s, rot: 0, jh: 'left', jv: 'baseline', ...o }),
  mtext: (p, h, s, w = 0, o = {}) => ({ type: 'text', p: pt(p), h, s, w, rot: 0, jh: 'left', jv: 'top', multi: true, ...o }),
  dim: (kind, o) => ({ type: 'dim', kind, ...o }),
  leader: (pts, s, h = 2.5, o = {}) => ({ type: 'leader', pts: pts.map(pt), s, h, ...o }),
  hatch: (loops, o = {}) => ({ type: 'hatch', loops, pattern: 'ANSI31', ps: 1, pa: 0, ...o }),
  insert: (name, p, o = {}) => ({ type: 'insert', name, p: pt(p), sx: 1, sy: 1, rot: 0, ...o }),
  xline: (p, d, o = {}) => ({ type: 'xline', p: pt(p), d: g.unit(d), ...o }),
  ray: (p, d, o = {}) => ({ type: 'ray', p: pt(p), d: g.unit(d), ...o }),
};

export const clone = e => { const c = structuredClone(e); delete c.id; return c; };

export const TYPE_LABEL = {
  line: 'Line', circle: 'Circle', arc: 'Arc', pline: 'Polyline', ellipse: 'Ellipse', spline: 'Spline', point: 'Point',
  text: 'Text', dim: 'Dimension', leader: 'Leader', hatch: 'Hatch', insert: 'Block Reference', xline: 'Construction Line', ray: 'Ray',
  viewport: 'Viewport',
};
export const labelOf = e => e.type === 'text' && e.multi ? 'Multiline Text' : TYPE_LABEL[e.type] || e.type;

// ---------------------------------------------------------------- memoisation tied to document revisions

function memo(doc, e, key, fn) {
  let c = doc._memo;
  if (!c || c.rev !== doc.rev) c = doc._memo = { rev: doc.rev, map: new WeakMap() };
  let m = c.map.get(e);
  if (!m) c.map.set(e, m = {});
  return key in m ? m[key] : (m[key] = fn());
}

// ---------------------------------------------------------------- ellipse / spline helpers

export function ellipsePoint(e, t) {
  const n = { x: -e.m.y * e.ratio, y: e.m.x * e.ratio };
  return { x: e.c.x + e.m.x * Math.cos(t) + n.x * Math.sin(t), y: e.c.y + e.m.y * Math.cos(t) + n.y * Math.sin(t) };
}
export const ellipseSweep = e => { const s = g.sweepCCW(e.t0, e.t1); return s < 1e-12 ? TAU : s; };
export const ellipseIsFull = e => ellipseSweep(e) >= TAU - 1e-9;

function ellipsePoints(e, n = 96) {
  const sw = ellipseSweep(e), steps = Math.max(8, Math.ceil(n * sw / TAU));
  return Array.from({ length: steps + 1 }, (_, i) => ellipsePoint(e, e.t0 + sw * i / steps));
}

/** Centripetal Catmull-Rom through fit points; returns a dense point list. */
export function splinePoints(e, perSpan = 14) {
  const P = e.fit, n = P.length;
  if (n < 2) return P.slice();
  const pts = e.closed ? P : [P[0], ...P, P[n - 1]];
  const out = [];
  const count = e.closed ? n : n - 1;
  const at = i => e.closed ? pts[((i % n) + n) % n] : pts[i + 1];
  const knot = (a, b) => Math.sqrt(g.dist(a, b)) || 1e-6;
  for (let i = 0; i < count; i++) {
    const p0 = e.closed ? at(i - 1) : (i === 0 ? { x: 2 * P[0].x - P[1].x, y: 2 * P[0].y - P[1].y } : at(i - 1));
    const p1 = at(i), p2 = at(i + 1);
    const p3 = e.closed ? at(i + 2) : (i + 2 >= n ? { x: 2 * P[n - 1].x - P[n - 2].x, y: 2 * P[n - 1].y - P[n - 2].y } : at(i + 2));
    const t0 = 0, t1 = t0 + knot(p0, p1), t2 = t1 + knot(p1, p2), t3 = t2 + knot(p2, p3);
    for (let s = 0; s < perSpan; s++) {
      const t = t1 + (t2 - t1) * s / perSpan;
      const A1 = g.lerp(p0, p1, (t - t0) / (t1 - t0)), A2 = g.lerp(p1, p2, (t - t1) / (t2 - t1)), A3 = g.lerp(p2, p3, (t - t2) / (t3 - t2));
      const B1 = g.lerp(A1, A2, (t - t0) / (t2 - t0)), B2 = g.lerp(A2, A3, (t - t1) / (t3 - t1));
      out.push(g.lerp(B1, B2, (t - t1) / (t2 - t1)));
    }
  }
  out.push(e.closed ? { ...P[0] } : { ...P[n - 1] });
  return out;
}

/** Exact cubic Bezier form of the spline: [{p0,c1,c2,p1}, ...] (used for DXF export). */
export function splineBeziers(e) {
  const P = e.fit, n = P.length;
  if (n < 2) return [];
  const pts = e.closed ? P : [P[0], ...P, P[n - 1]];
  const count = e.closed ? n : n - 1;
  const at = i => e.closed ? pts[((i % n) + n) % n] : pts[i + 1];
  const knot = (a, b) => Math.sqrt(g.dist(a, b)) || 1e-6;
  const out = [];
  for (let i = 0; i < count; i++) {
    const p0 = e.closed ? at(i - 1) : (i === 0 ? { x: 2 * P[0].x - P[1].x, y: 2 * P[0].y - P[1].y } : at(i - 1));
    const p1 = at(i), p2 = at(i + 1);
    const p3 = e.closed ? at(i + 2) : (i + 2 >= n ? { x: 2 * P[n - 1].x - P[n - 2].x, y: 2 * P[n - 1].y - P[n - 2].y } : at(i + 2));
    const d01 = knot(p0, p1), d12 = knot(p1, p2), d23 = knot(p2, p3);
    const t1 = d01, t2 = t1 + d12, t3 = t2 + d23;
    const D = (a, b, dt) => ({ x: (b.x - a.x) / dt, y: (b.y - a.y) / dt });
    const m1 = (() => { const u = D(p0, p1, t1), v = D(p0, p2, t2), w = D(p1, p2, t2 - t1); return { x: u.x - v.x + w.x, y: u.y - v.y + w.y }; })();
    const m2 = (() => { const u = D(p1, p2, t2 - t1), v = D(p1, p3, t3 - t1), w = D(p2, p3, t3 - t2); return { x: u.x - v.x + w.x, y: u.y - v.y + w.y }; })();
    out.push({ p0: p1, c1: { x: p1.x + m1.x * d12 / 3, y: p1.y + m1.y * d12 / 3 }, c2: { x: p2.x - m2.x * d12 / 3, y: p2.y - m2.y * d12 / 3 }, p1: p2 });
  }
  return out;
}

const linesFromPts = (pts, closed) => {
  const out = [];
  for (let i = 0; i + 1 < pts.length; i++) out.push({ t: 'l', a: pts[i], b: pts[i + 1], approx: true });
  if (closed && pts.length > 2) out.push({ t: 'l', a: pts[pts.length - 1], b: pts[0], approx: true });
  return out;
};

// ---------------------------------------------------------------- block references

export function insertMatrix(e, base = { x: 0, y: 0 }) {
  return M.mul(M.mul(M.translate(e.p.x, e.p.y), M.rotate(e.rot || 0)), M.mul(M.scale(e.sx ?? 1, e.sy ?? 1), M.translate(-base.x, -base.y)));
}

function transformSegGeneral(s, m) {
  if (s.t === 'l' || M.isSimilarity(m)) return { ...g.transformSeg(s, m), approx: s.approx, full: s.full };
  return null;   // arcs under non-uniform scale: caller tessellates
}

function transformSegs(segs, m) {
  const out = [];
  for (const s of segs) {
    const t = transformSegGeneral(s, m);
    if (t) out.push(t);
    else for (const l of linesFromPts(g.tessSeg(s, 0.01 * Math.max(1, s.r * .01)).map(p => M.apply(m, p)), false)) out.push(l);
  }
  return out;
}

const MAX_BLOCK_DEPTH = 12;

// ---------------------------------------------------------------- segments

/** Line/arc segments describing an entity's pickable curve geometry (world coordinates). */
export function segsOf(e, doc, depth = 0) {
  return memo(doc, e, 'segs', () => computeSegs(e, doc, depth));
}

function computeSegs(e, doc, depth) {
  switch (e.type) {
    case 'line': return [{ t: 'l', a: e.a, b: e.b }];
    case 'circle': return [{ t: 'a', c: e.c, r: e.r, a0: 0, sw: TAU, full: true }];
    case 'arc': { const sw = g.sweepCCW(e.a0, e.a1); return [{ t: 'a', c: e.c, r: e.r, a0: e.a0, sw: sw < 1e-12 ? TAU : sw }]; }
    case 'pline': return g.plineSegs(e.v, e.closed);
    case 'ellipse': return linesFromPts(ellipsePoints(e), ellipseIsFull(e));
    case 'spline': return linesFromPts(splinePoints(e), false);
    case 'point': case 'text': return [];
    case 'dim': return dimGeometry(e, doc).segs;
    case 'leader': return linesFromPts(e.pts, false);
    case 'hatch': return [];     // fill region only: not snappable, picked via its pattern lines
    case 'insert': {
      const def = doc.blocks?.[e.name];
      if (!def || depth > MAX_BLOCK_DEPTH) return [];
      const m = insertMatrix(e, def.base);
      return def.entities.flatMap(be => transformSegs(segsOf(be, doc, depth + 1), m));
    }
    case 'xline': return [{ t: 'l', a: g.add(e.p, g.mul(e.d, -1e6)), b: g.add(e.p, g.mul(e.d, 1e6)) }];
    case 'ray': return [{ t: 'l', a: e.p, b: g.add(e.p, g.mul(e.d, 1e6)) }];
    case 'viewport': return [{ t: 'l', a: { x: e.c.x - e.w / 2, y: e.c.y - e.h / 2 }, b: { x: e.c.x + e.w / 2, y: e.c.y - e.h / 2 } }, { t: 'l', a: { x: e.c.x + e.w / 2, y: e.c.y - e.h / 2 }, b: { x: e.c.x + e.w / 2, y: e.c.y + e.h / 2 } }, { t: 'l', a: { x: e.c.x + e.w / 2, y: e.c.y + e.h / 2 }, b: { x: e.c.x - e.w / 2, y: e.c.y + e.h / 2 } }, { t: 'l', a: { x: e.c.x - e.w / 2, y: e.c.y + e.h / 2 }, b: { x: e.c.x - e.w / 2, y: e.c.y - e.h / 2 } }];
    default: return [];
  }
}

/** Text-like entities visible inside a block reference or dimension (for bounds / rendering of nested text). */
export function textsOf(e, doc, depth = 0) {
  if (e.type === 'text') return [e];
  if (e.type === 'dim') { const t = dimGeometry(e, doc).text; return [{ type: 'text', p: t.p, h: t.h, s: t.s, rot: t.rot, jh: t.jh, jv: t.jv }]; }
  if (e.type === 'leader' && e.s) return [leaderText(e)];
  if (e.type === 'insert') {
    const def = doc.blocks?.[e.name];
    if (!def || depth > MAX_BLOCK_DEPTH) return [];
    const m = insertMatrix(e, def.base), k = M.scaleFactor(m), ang = M.angle(m);
    return def.entities.flatMap(be => textsOf(be, doc, depth + 1)).map(t => ({
      ...t, p: M.apply(m, t.p), h: t.h * k, rot: (t.rot || 0) + ang, w: t.w ? t.w * k : t.w,
    }));
  }
  return [];
}

export function leaderText(e) {
  const last = e.pts[e.pts.length - 1], prev = e.pts[e.pts.length - 2] || last, right = last.x >= prev.x;
  return { type: 'text', p: { x: last.x + (right ? 1 : -1) * e.h * .4, y: last.y }, h: e.h, s: e.s, rot: 0, jh: right ? 'left' : 'right', jv: 'middle' };
}

// ---------------------------------------------------------------- bounds

export function boundsOf(e, doc) {
  return memo(doc, e, 'bounds', () => {
    if (e.type === 'xline' || e.type === 'ray') return null;
    let b = null;
    if (e.type === 'point') return { minX: e.p.x, minY: e.p.y, maxX: e.p.x, maxY: e.p.y };
    for (const s of segsOf(e, doc)) b = g.unionBounds(b, g.segBounds(s));
    if (e.type === 'hatch') for (const l of e.loops) for (const s of g.plineSegs(l, true)) b = g.unionBounds(b, g.segBounds(s));
    for (const t of textsOf(e, doc)) b = g.unionBounds(b, g.ptsBounds(textCorners(t)));
    if (e.type === 'dim') for (const a of dimGeometry(e, doc).arrows) b = g.unionBounds(b, g.ptsBounds([a.tip]));
    return b;
  });
}

// ---------------------------------------------------------------- hit testing helpers

export function textPolygons(e, doc) { return textsOf(e, doc).map(textCorners); }

/** Distance from p to the pickable geometry of e (0 inside text boxes / filled hatches). */
export function pickDistance(e, p, doc) {
  if (e.type === 'point') return g.dist(e.p, p);
  let best = Infinity;
  for (const s of segsOf(e, doc)) { const d = g.segNearest(s, p).d; if (d < best) best = d; }
  for (const poly of textPolygons(e, doc)) {
    if (g.pointInPolygon(p, poly)) return 0;
    for (let i = 0; i < 4; i++) best = Math.min(best, g.segNearest({ t: 'l', a: poly[i], b: poly[(i + 1) % 4] }, p).d);
  }
  if (e.type === 'hatch') {
    const d = hatchPickDistance(e, p, doc);
    if (d < best) best = d;
  }
  return best;
}

function hatchPickDistance(e, p, doc) {
  if (e.pattern === 'SOLID') return pointInHatch(e, p) ? 0 : Infinity;
  let best = Infinity;
  for (const [x0, y0, x1, y1] of hatchSegments(e, doc) || []) best = Math.min(best, g.segNearest({ t: 'l', a: { x: x0, y: y0 }, b: { x: x1, y: y1 } }, p).d);
  return best;
}

export function pointInHatch(e, p) {
  let inside = false;
  for (const l of e.loops) if (g.pointInPolygon(p, g.tessSegs(g.plineSegs(l, true), .05))) inside = !inside;
  return inside;
}

/** Hatch pattern lines, cached per document revision. */
export function hatchSegments(e, doc) {
  return memo(doc, e, 'hatch', () => {
    const loops = e.loops.map(l => g.tessSegs(g.plineSegs(l, true), Math.max(.01, 0.002 * (boundsOf(e, doc) ? Math.hypot(boundsOf(e, doc).maxX - boundsOf(e, doc).minX, boundsOf(e, doc).maxY - boundsOf(e, doc).minY) : 1))));
    return hatchLines(loops, e.pattern, e.ps || 1, g.deg(e.pa || 0));
  });
}

export function hatchLoopsTess(e) { return e.loops.map(l => g.tessSegs(g.plineSegs(l, true), .02)); }

// ---------------------------------------------------------------- selection by rectangle

/** Window selection: entity fully inside the rect. */
export function inWindow(e, r, doc) {
  if (e.type === 'xline' || e.type === 'ray') return false;
  const b = boundsOf(e, doc);
  return !!b && g.boundsInside(b, r);
}

/** Crossing selection: entity touches the rect. */
export function crossesRect(e, r, doc) {
  if (e.type === 'point') return e.p.x >= r.minX && e.p.x <= r.maxX && e.p.y >= r.minY && e.p.y <= r.maxY;
  const b = boundsOf(e, doc);
  if (b && !g.boundsOverlap(b, r) && e.type !== 'xline' && e.type !== 'ray') return false;
  if (e.type === 'hatch' && e.pattern === 'SOLID' && b && g.boundsOverlap(b, r)) {
    if (pointInHatch(e, { x: (r.minX + r.maxX) / 2, y: (r.minY + r.maxY) / 2 })) return true;
  }
  const edges = e.type === 'hatch' ? e.loops.flatMap(l => g.plineSegs(l, true)) : segsOf(e, doc);
  for (const s of edges) if (g.segTouchesRect(s, r)) return true;
  const rc = [{ x: r.minX, y: r.minY }, { x: r.maxX, y: r.minY }, { x: r.maxX, y: r.maxY }, { x: r.minX, y: r.maxY }];
  for (const poly of textPolygons(e, doc)) {
    if (poly.some(q => q.x >= r.minX && q.x <= r.maxX && q.y >= r.minY && q.y <= r.maxY)) return true;
    if (g.pointInPolygon(rc[0], poly)) return true;
    for (let i = 0; i < 4; i++) if (g.segTouchesRect({ t: 'l', a: poly[i], b: poly[(i + 1) % 4] }, r)) return true;
  }
  return false;
}

// ---------------------------------------------------------------- transforms

/** Decompose a similarity matrix into rotation, uniform scale and reflection flag. */
export function decompose(m) {
  return { k: M.scaleFactor(m), theta: M.angle(m), flip: M.det(m) < 0 };
}

/** Mutate an entity by an affine similarity transform. */
export function transformEntity(e, m, doc) {
  const { k, theta, flip } = decompose(m);
  const mp = p => M.apply(m, p);
  switch (e.type) {
    case 'line': e.a = mp(e.a); e.b = mp(e.b); break;
    case 'circle': e.c = mp(e.c); e.r *= k; break;
    case 'arc': {
      const s = g.transformSeg({ t: 'a', c: e.c, r: e.r, a0: e.a0, sw: g.sweepCCW(e.a0, e.a1) || TAU }, m);
      const a0 = s.sw < 0 ? s.a0 + s.sw : s.a0, sw = Math.abs(s.sw);
      e.c = s.c; e.r = s.r; e.a0 = g.normAngle(a0); e.a1 = g.normAngle(a0 + sw);
      break;
    }
    case 'pline': e.v = e.v.map(q => ({ ...mp(q), b: flip ? -(q.b || 0) : (q.b || 0) })); break;
    case 'ellipse': {
      e.c = mp(e.c); e.m = M.applyVec(m, e.m);
      if (flip) { const t0 = -e.t1, t1 = -e.t0; e.t0 = g.normAngle(t0); e.t1 = g.normAngle(t1); }
      break;
    }
    case 'spline': e.fit = e.fit.map(mp); break;
    case 'point': e.p = mp(e.p); break;
    case 'text': {
      e.p = mp(e.p); e.h *= k; if (e.w) e.w *= k;
      if (flip) {
        const v = M.applyVec(m, { x: Math.cos(e.rot || 0), y: Math.sin(e.rot || 0) });
        let a = Math.atan2(v.y, v.x);
        if (Math.cos(a) < -1e-9 || (Math.abs(Math.cos(a)) <= 1e-9 && Math.sin(a) < 0)) a += Math.PI;
        e.rot = g.normAngle(a);
      } else e.rot = g.normAngle((e.rot || 0) + theta);
      break;
    }
    case 'dim': {
      for (const key of ['p1', 'p2', 'dp', 'c', 'pt', 'tp']) if (e[key]) e[key] = mp(e[key]);
      if (e.kind === 'lin') { const v = M.applyVec(m, { x: Math.cos(e.ang || 0), y: Math.sin(e.ang || 0) }); e.ang = Math.atan2(v.y, v.x); }
      break;
    }
    case 'leader': e.pts = e.pts.map(mp); break;
    case 'hatch': e.loops = e.loops.map(l => l.map(q => ({ ...mp(q), b: flip ? -(q.b || 0) : (q.b || 0) }))); e.pa = flip ? theta - (e.pa || 0) : (e.pa || 0) + theta; e.ps = (e.ps || 1) * k; break;
    case 'insert': {
      e.p = mp(e.p);
      if (flip) { e.rot = g.normAngle(theta - (e.rot || 0)); e.sx = (e.sx ?? 1) * k; e.sy = -(e.sy ?? 1) * k; }
      else { e.rot = g.normAngle((e.rot || 0) + theta); e.sx = (e.sx ?? 1) * k; e.sy = (e.sy ?? 1) * k; }
      break;
    }
    case 'xline': case 'ray': e.p = mp(e.p); e.d = g.unit(M.applyVec(m, e.d)); break;
    case 'viewport': e.c = mp(e.c); break;
    default: break;
  }
  void doc;
  return e;
}

// ---------------------------------------------------------------- stretch (STRETCH command)

/**
 * Move the parts of an entity that lie inside `win` (a rect) by `d`. Entities fully inside move entirely.
 * @returns {boolean} whether anything changed
 */
export function stretchEntity(e, win, d, doc) {
  const inside = p => p.x >= win.minX && p.x <= win.maxX && p.y >= win.minY && p.y <= win.maxY;
  const mv = p => { p.x += d.x; p.y += d.y; };
  const fully = () => { const b = boundsOf(e, doc); return b && g.boundsInside(b, win); };
  switch (e.type) {
    case 'line': { let c = false; if (inside(e.a)) { mv(e.a); c = true; } if (inside(e.b)) { mv(e.b); c = true; } return c; }
    case 'pline': { let c = false; for (const v of e.v) if (inside(v)) { mv(v); c = true; } return c; }
    case 'spline': { let c = false; for (const v of e.fit) if (inside(v)) { mv(v); c = true; } return c; }
    case 'leader': { let c = false; for (const v of e.pts) if (inside(v)) { mv(v); c = true; } return c; }
    case 'arc': {
      const s = g.arcStart({ c: e.c, r: e.r, a0: e.a0 }), en = g.arcEnd({ c: e.c, r: e.r, a0: e.a0, sw: g.sweepCCW(e.a0, e.a1) });
      const si = inside(s), ei = inside(en);
      if (si && ei) { mv(e.c); return true; }
      if (!si && !ei) return false;
      const sw = g.sweepCCW(e.a0, e.a1) || TAU, mid = g.polar(e.c, e.a0 + sw / 2, e.r);
      const ns = si ? g.add(s, d) : s, ne = ei ? g.add(en, d) : en;
      const a = g.arcThrough(ns, g.add(mid, { x: d.x / 2, y: d.y / 2 }), ne);
      if (!a) return false;
      e.c = a.c; e.r = a.r; e.a0 = g.normAngle(a.sw >= 0 ? a.a0 : a.a0 + a.sw); e.a1 = g.normAngle(a.sw >= 0 ? a.a0 + a.sw : a.a0);
      return true;
    }
    case 'circle': case 'ellipse': if (inside(e.c)) { mv(e.c); return true; } return false;
    case 'point': case 'text': case 'insert': case 'xline': case 'ray': if (inside(e.p)) { mv(e.p); return true; } return false;
    case 'dim': {
      let c = false;
      for (const key of ['p1', 'p2', 'dp', 'c', 'pt', 'tp']) if (e[key] && inside(e[key])) { mv(e[key]); c = true; }
      return c;
    }
    case 'hatch': if (fully()) { for (const l of e.loops) for (const v of l) mv(v); return true; } return false;
    default: return false;
  }
}

// ---------------------------------------------------------------- grips

/**
 * Grip descriptors for an entity: { id, pt, kind }. `kind` is 'stretch' (moves one defining point),
 * 'move' (translates the whole entity) or 'radius'.
 */
export function gripsOf(e, doc) {
  switch (e.type) {
    case 'line': return [{ id: 'a', pt: e.a, kind: 'stretch' }, { id: 'm', pt: g.mid(e.a, e.b), kind: 'move' }, { id: 'b', pt: e.b, kind: 'stretch' }];
    case 'circle': return [{ id: 'c', pt: e.c, kind: 'move' }, ...[0, 1, 2, 3].map(i => ({ id: 'q' + i, pt: g.polar(e.c, i * Math.PI / 2, e.r), kind: 'radius' }))];
    case 'arc': {
      const sw = g.sweepCCW(e.a0, e.a1) || TAU;
      return [{ id: 's', pt: g.polar(e.c, e.a0, e.r), kind: 'stretch' }, { id: 'm', pt: g.polar(e.c, e.a0 + sw / 2, e.r), kind: 'stretch' }, { id: 'e', pt: g.polar(e.c, e.a0 + sw, e.r), kind: 'stretch' }, { id: 'c', pt: e.c, kind: 'move' }];
    }
    case 'pline': {
      const out = e.v.map((v, i) => ({ id: 'v' + i, pt: v, kind: 'stretch' }));
      g.plineSegs(e.v, e.closed).forEach((s, i) => out.push({ id: 'm' + i, pt: g.segAt(s, .5), kind: s.t === 'a' ? 'stretch' : 'move' }));
      return out;
    }
    case 'ellipse': {
      const n = { x: -e.m.y * e.ratio, y: e.m.x * e.ratio };
      return [{ id: 'c', pt: e.c, kind: 'move' }, { id: 'a0', pt: g.add(e.c, e.m), kind: 'stretch' }, { id: 'a1', pt: g.sub(e.c, e.m), kind: 'stretch' }, { id: 'b0', pt: g.add(e.c, n), kind: 'stretch' }, { id: 'b1', pt: g.sub(e.c, n), kind: 'stretch' }];
    }
    case 'spline': return e.fit.map((p, i) => ({ id: 'f' + i, pt: p, kind: 'stretch' }));
    case 'point': return [{ id: 'p', pt: e.p, kind: 'move' }];
    case 'text': return [{ id: 'p', pt: e.p, kind: 'move' }];
    case 'insert': return [{ id: 'p', pt: e.p, kind: 'move' }];
    case 'xline': case 'ray': return [{ id: 'p', pt: e.p, kind: 'move' }];
    case 'leader': return e.pts.map((p, i) => ({ id: 'l' + i, pt: p, kind: 'stretch' }));
    case 'hatch': { const b = boundsOf(e, doc); return b ? [{ id: 'c', pt: { x: (b.minX + b.maxX) / 2, y: (b.minY + b.maxY) / 2 }, kind: 'move' }] : []; }
    case 'dim': {
      const out = [];
      const add = (id, p) => p && out.push({ id, pt: p, kind: id === 'dp' || id === 'tp' ? 'stretch' : 'stretch' });
      const geo = dimGeometry(e, doc);
      if (e.kind === 'lin' || e.kind === 'ali') { add('p1', e.p1); add('p2', e.p2); add('dp', geo.dimLine ? g.mid(geo.dimLine.A, geo.dimLine.B) : e.dp); }
      else if (e.kind === 'ang') { add('p1', e.p1); add('p2', e.p2); add('dp', e.dp); }
      else { add('pt', e.pt); add('dp', e.dp); }
      add('tp', geo.text.p);
      return out;
    }
    case 'viewport': return [{ id: 'c', pt: e.c, kind: 'move' }, ...[[-1, -1], [1, -1], [1, 1], [-1, 1]].map(([sx, sy], i) => ({ id: 'k' + i, pt: { x: e.c.x + sx * e.w / 2, y: e.c.y + sy * e.h / 2 }, kind: 'stretch' }))];
    default: return [];
  }
}

/** Return a modified clone of `orig` with grip `id` dragged to `to` (and optionally moved by whole-entity delta). */
export function gripEdit(orig, id, to, doc) {
  const e = structuredClone(orig);
  const grip = gripsOf(orig, doc).find(q => q.id === id);
  if (!grip) return e;
  const delta = { x: to.x - grip.pt.x, y: to.y - grip.pt.y };
  if (grip.kind === 'move') { transformEntity(e, M.translate(delta.x, delta.y), doc); return e; }
  switch (e.type) {
    case 'line': if (id === 'a') e.a = pt(to); else e.b = pt(to); break;
    case 'circle': e.r = Math.max(1e-9, g.dist(e.c, to)); break;
    case 'arc': {
      const sw = g.sweepCCW(orig.a0, orig.a1) || TAU;
      const s = g.polar(orig.c, orig.a0, orig.r), m = g.polar(orig.c, orig.a0 + sw / 2, orig.r), en = g.polar(orig.c, orig.a0 + sw, orig.r);
      const tri = id === 's' ? [to, m, en] : id === 'e' ? [s, m, to] : [s, to, en];
      const a = g.arcThrough(...tri);
      if (a) { e.c = a.c; e.r = a.r; e.a0 = g.normAngle(a.sw >= 0 ? a.a0 : a.a0 + a.sw); e.a1 = g.normAngle(a.sw >= 0 ? a.a0 + a.sw : a.a0); }
      break;
    }
    case 'pline': {
      if (id[0] === 'v') { const i = +id.slice(1); e.v[i].x = to.x; e.v[i].y = to.y; }
      else {
        const i = +id.slice(1), n = e.v.length, j = (i + 1) % n, segs = g.plineSegs(orig.v, orig.closed);
        if (segs[i].t === 'a') {
          const a = g.arcThrough(orig.v[i], to, orig.v[j]);
          if (a) e.v[i].b = Math.tan(a.sw / 4);
        } else { e.v[i].x += delta.x; e.v[i].y += delta.y; e.v[j].x += delta.x; e.v[j].y += delta.y; }
      }
      break;
    }
    case 'ellipse': {
      if (id === 'a0' || id === 'a1') {
        const sign = id === 'a0' ? 1 : -1, m = g.mul(g.sub(to, e.c), sign), L = g.len(m), L0 = g.len(orig.m);
        if (L > 1e-9) { e.m = m; e.ratio = orig.ratio * L0 / L; }
      } else {
        const n = g.unit({ x: -orig.m.y, y: orig.m.x });
        e.ratio = Math.max(1e-6, Math.min(1, Math.abs(g.dot(g.sub(to, e.c), n)) / g.len(orig.m)));
      }
      break;
    }
    case 'spline': e.fit[+id.slice(1)] = pt(to); break;
    case 'leader': e.pts[+id.slice(1)] = pt(to); break;
    case 'dim': {
      if (id === 'tp') e.tp = pt(to);
      else if (id === 'dp' && (e.kind === 'lin' || e.kind === 'ali')) { e.dp = pt(to); }
      else e[id] = pt(to);
      break;
    }
    case 'viewport': {
      const i = +id.slice(1), sx = [-1, 1, 1, -1][i], sy = [-1, -1, 1, 1][i];
      const opp = { x: orig.c.x - sx * orig.w / 2, y: orig.c.y - sy * orig.h / 2 };
      e.c = g.mid(opp, to); e.w = Math.abs(to.x - opp.x); e.h = Math.abs(to.y - opp.y);
      break;
    }
    default: break;
  }
  return e;
}

// ---------------------------------------------------------------- object snap points

/** Snap candidates {pt, type} intrinsic to an entity: END, MID, CEN, QUA, NOD, INS. */
export function snapPointsOf(e, doc) {
  return memo(doc, e, 'snaps', () => computeSnaps(e, doc));
}

function computeSnaps(e, doc) {
  const out = [];
  const push = (pt, type) => out.push({ pt, type });
  switch (e.type) {
    case 'line': push(e.a, 'END'); push(e.b, 'END'); push(g.mid(e.a, e.b), 'MID'); return out;
    case 'circle': push(e.c, 'CEN'); for (let i = 0; i < 4; i++) push(g.polar(e.c, i * Math.PI / 2, e.r), 'QUA'); return out;
    case 'ellipse': {
      push(e.c, 'CEN');
      const n = { x: -e.m.y * e.ratio, y: e.m.x * e.ratio };
      if (ellipseIsFull(e)) { for (const q of [g.add(e.c, e.m), g.sub(e.c, e.m), g.add(e.c, n), g.sub(e.c, n)]) push(q, 'QUA'); }
      else { push(ellipsePoint(e, e.t0), 'END'); push(ellipsePoint(e, e.t1), 'END'); }
      return out;
    }
    case 'spline': push(e.fit[0], 'END'); push(e.fit[e.fit.length - 1], 'END'); for (const p of e.fit.slice(1, -1)) push(p, 'NOD'); return out;
    case 'point': push(e.p, 'NOD'); return out;
    case 'text': push(e.p, 'INS'); return out;
    case 'insert': {
      push(e.p, 'INS');
      for (const s of segsOf(e, doc)) segSnaps(s, push);
      return out;
    }
    case 'dim': {
      const geo = dimGeometry(e, doc);
      for (const s of geo.segs) if (s.t === 'l') { push(s.a, 'END'); push(s.b, 'END'); }
      push(geo.text.p, 'INS');
      return out;
    }
    case 'leader': for (const p of e.pts) push(p, 'END'); return out;
    case 'hatch': return out;
    case 'xline': case 'ray': push(e.p, 'END'); return out;
    default:
      for (const s of segsOf(e, doc)) segSnaps(s, push);
      return dedupeSnaps(out);
  }
}

function segSnaps(s, push) {
  if (s.approx) return;
  if (s.t === 'l') { push(s.a, 'END'); push(s.b, 'END'); push(g.mid(s.a, s.b), 'MID'); return; }
  if (s.full) { push(s.c, 'CEN'); for (let i = 0; i < 4; i++) push(g.polar(s.c, i * Math.PI / 2, s.r), 'QUA'); return; }
  push(g.arcStart(s), 'END'); push(g.arcEnd(s), 'END'); push(g.segAt(s, .5), 'MID'); push(s.c, 'CEN');
  for (let i = 0; i < 4; i++) if (g.angleInArc(i * Math.PI / 2, s.a0, s.sw)) push(g.polar(s.c, i * Math.PI / 2, s.r), 'QUA');
}

function dedupeSnaps(list) {
  const out = [];
  for (const s of list) if (!out.some(o => o.type === s.type && g.samePt(o.pt, s.pt, 1e-9))) out.push(s);
  return out;
}

// ---------------------------------------------------------------- explode

/** Entities replacing `e` when exploded, or null if the entity can't be exploded. Inherits layer/color. */
export function explodeEntity(e, doc) {
  const inherit = o => { for (const k of ['layer', 'color', 'ltype', 'lw']) if (e[k] !== undefined) o[k] = e[k]; return o; };
  switch (e.type) {
    case 'pline': return g.plineSegs(e.v, e.closed).map(s => inherit(s.t === 'l' ? E.line(s.a, s.b) : E.arcSeg(s)));
    case 'insert': {
      const def = doc.blocks?.[e.name];
      if (!def) return [];
      const m = insertMatrix(e, def.base), out = [];
      const nonUniform = !M.isSimilarity(m);
      for (const be of def.entities) {
        let c = clone(be);
        if (nonUniform && (c.type === 'circle' || c.type === 'arc')) c = circleToEllipse(c, m);
        else transformEntity(c, m, doc);
        if (c.layer === undefined || c.layer === '0') c.layer = e.layer;
        if (c.color === 'BYBLOCK') c.color = e.color;
        out.push(c);
      }
      return out;
    }
    case 'dim': {
      const geo = dimGeometry(e, doc), out = [];
      for (const s of geo.segs) out.push(inherit(s.t === 'l' ? E.line(s.a, s.b) : E.arcSeg(s)));
      for (const a of geo.arrows) {
        const poly = arrowPolygon(a);
        if (poly) out.push(inherit(E.hatch([poly.map(p => ({ x: p.x, y: p.y, b: 0 }))], { pattern: 'SOLID' })));
      }
      out.push(inherit(E.text(geo.text.p, geo.text.h, geo.text.s, { rot: geo.text.rot, jh: geo.text.jh, jv: geo.text.jv })));
      return out;
    }
    case 'leader': {
      const out = [];
      for (let i = 0; i + 1 < e.pts.length; i++) out.push(inherit(E.line(e.pts[i], e.pts[i + 1])));
      if (e.s) { const t = leaderText(e); out.push(inherit(E.text(t.p, t.h, t.s, { jh: t.jh, jv: t.jv }))); }
      return out;
    }
    case 'text': {
      if (!e.multi) return null;
      const lines = textLayout(e).lines;
      const m = M.mul(M.translate(e.p.x, e.p.y), M.rotate(e.rot || 0));
      return lines.map(l => inherit(E.text(M.apply(m, { x: l.x, y: l.y }), e.h, l.str, { rot: e.rot })));
    }
    default: return null;
  }
}

/**
 * Image of a circle/arc entity under an arbitrary affine map: an ellipse (arc) entity. The principal axes
 * come from the singular value decomposition of the map's linear part.
 */
export function circleToEllipse(c, m) {
  const [a, b, cc, d] = m;                       // columns: (a,b) and (cc,d)
  const E_ = a * a + b * b, F = a * cc + b * d, G = cc * cc + d * d;
  const psi = .5 * Math.atan2(2 * F, E_ - G);   // direction of the major right-singular vector
  const v1 = { x: Math.cos(psi), y: Math.sin(psi) }, v2 = { x: -v1.y, y: v1.x };
  const mv = g.mul(M.applyVec(m, v1), c.r), nv = g.mul(M.applyVec(m, v2), c.r);
  const L1 = g.len(mv), L2 = g.len(nv);
  const flip = M.det(m) < 0, swapAxes = L2 > L1;
  const maj = swapAxes ? nv : mv, ratio = Math.min(L1, L2) / Math.max(L1, L2, 1e-12);
  // parameter of circle angle alpha on the new ellipse
  let tOf = alpha => alpha - psi, shift = 0;
  if (swapAxes) shift = -Math.PI / 2;           // major axis is the image of v2
  const t = alpha => tOf(alpha) + shift;
  let t0 = 0, t1 = TAU;
  if (c.type === 'arc') { t0 = t(c.a0); t1 = t(c.a1); if (flip) [t0, t1] = [-t1, -t0]; }
  const out = E.ellipse(M.apply(m, c.c), flip ? maj : maj, ratio, g.normAngle(t0), c.type === 'arc' ? g.normAngle(t1) : TAU);
  if (flip && swapAxes) out.m = maj;
  for (const k of ['layer', 'color', 'ltype', 'lw']) if (c[k] !== undefined) out[k] = c[k];
  return out;
}

/** Entity list used by hatch boundary detection and TRIM/EXTEND edge selection. */
export const isCurve = e => ['line', 'circle', 'arc', 'pline', 'ellipse', 'spline'].includes(e.type);
