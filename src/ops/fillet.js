// FILLET and CHAMFER between lines/arcs, and the whole-polyline variants.
import * as g from '../core/geom.js';
import { pathOf, entityFromSegs } from './path.js';

/** Portion of `seg` that contains the point at param `tp`, bounded at param `tb` (tb may lie outside [0,1]). */
function keepSide(seg, tp, tb) {
  return tp < tb ? g.segSlice(seg, 0, tb) : g.segSlice(seg, tb, 1);
}

const paramOf = (seg, p) => g.segProject(seg, p);

/** Foot of the perpendicular from a fillet centre onto a carrier (the tangent point). */
function tangentFoot(seg, c) {
  if (seg.t === 'l') return g.perpFeet(seg, c)[0].pt;
  const u = g.unit(g.sub(c, seg.c));
  return g.add(seg.c, g.mul(u, seg.r));
}

/**
 * Fillet two segments. Pick points choose which ends survive and which corner is used.
 * @returns {{a:object|null, b:object|null, arc:object|null}|{error:string}}  a/b: replacement segments (null = leave untouched, e.g. full circles)
 */
export function filletSegs(sA, pickA, sB, pickB, r) {
  const keepA = !sA.full, keepB = !sB.full;
  if (r < 1e-12) {
    const hits = g.segIntersect(sA, sB, true, true);
    if (!hits.length) return { error: 'The objects are parallel or do not meet.' };
    const m = g.mid(pickA, pickB);
    const h = hits.reduce((best, c) => g.dist(c.pt, m) < g.dist(best.pt, m) ? c : best);
    return { a: keepA ? keepSide(sA, paramOf(sA, pickA), h.t1) : null, b: keepB ? keepSide(sB, paramOf(sB, pickB), h.t2) : null, arc: null };
  }
  const centres = g.tangentCentres(sA, sB, r);
  if (!centres.length) return { error: 'A fillet of that radius cannot be created between these objects.' };
  // The arc lies in the sector spanned by the picked portions: both tangent points must be on the picked side of the corner.
  const corners = g.segIntersect(sA, sB, true, true);
  const pA = paramOf(sA, pickA), pB = paramOf(sB, pickB);
  let best = null;
  for (const c of centres) {
    const ta = tangentFoot(sA, c), tb = tangentFoot(sB, c);
    const qa = paramOf(sA, ta), qb = paramOf(sB, tb);
    const onPickedSide = corners.some(h => Math.sign(pA - h.t1) === Math.sign(qa - h.t1) && Math.sign(pB - h.t2) === Math.sign(qb - h.t2));
    const score = (onPickedSide ? 0 : 1e9) + g.dist(ta, pickA) + g.dist(tb, pickB);
    if (!best || score < best.score) best = { c, ta, tb, score };
  }
  const { c, ta, tb } = best;
  const a0 = g.angleOf(c, ta), sw = g.normAngleSigned(g.angleOf(c, tb) - a0);
  const arc = { t: 'a', c, r, a0, sw };
  return {
    a: keepA ? keepSide(sA, paramOf(sA, pickA), paramOf(sA, ta)) : null,
    b: keepB ? keepSide(sB, paramOf(sB, pickB), paramOf(sB, tb)) : null,
    arc,
  };
}

/** Chamfer two line segments by distances d1 (on A) and d2 (on B). */
export function chamferSegs(sA, pickA, sB, pickB, d1, d2) {
  if (sA.t !== 'l' || sB.t !== 'l') return { error: 'Chamfer works on straight segments.' };
  const hits = g.segIntersect(sA, sB, true, true);
  if (!hits.length) return { error: 'The lines are parallel.' };
  const p = hits[0].pt;
  if (d1 < 1e-12 && d2 < 1e-12) {
    return { a: keepSide(sA, paramOf(sA, pickA), hits[0].t1), b: keepSide(sB, paramOf(sB, pickB), hits[0].t2), line: null };
  }
  const dirToward = (s, pick) => g.unit(g.sub(pick, p).x === 0 && g.sub(pick, p).y === 0 ? g.sub(s.b, s.a) : (g.dot(g.sub(pick, p), g.sub(s.b, s.a)) >= 0 ? g.sub(s.b, s.a) : g.sub(s.a, s.b)));
  const ua = dirToward(sA, pickA), ub = dirToward(sB, pickB);
  const ca = g.add(p, g.mul(ua, d1)), cb = g.add(p, g.mul(ub, d2));
  const A = { t: 'l', a: sA.a, b: sA.b }, B = { t: 'l', a: sB.a, b: sB.b };
  return {
    a: keepSide(A, paramOf(A, pickA), paramOf(A, ca)),
    b: keepSide(B, paramOf(B, pickB), paramOf(B, cb)),
    line: { t: 'l', a: ca, b: cb },
  };
}

/** Chamfer by length on A and an angle (radians) measured from A. */
export function chamferAngle(sA, pickA, sB, pickB, d1, angle) {
  const hits = g.segIntersect(sA, sB, true, true);
  if (!hits.length) return { error: 'The lines are parallel.' };
  const p = hits[0].pt;
  const ua = g.unit(g.dot(g.sub(pickA, p), g.sub(sA.b, sA.a)) >= 0 ? g.sub(sA.b, sA.a) : g.sub(sA.a, sA.b));
  const ca = g.add(p, g.mul(ua, d1));
  const ub = g.unit(g.dot(g.sub(pickB, p), g.sub(sB.b, sB.a)) >= 0 ? g.sub(sB.b, sB.a) : g.sub(sB.a, sB.b));
  // direction from ca at `angle` from A's direction back toward the corner, towards B's side
  const base = g.angleOf(ca, p);
  const side = g.cross(ua, ub) >= 0 ? 1 : -1;
  const dir = { x: Math.cos(base - side * angle), y: Math.sin(base - side * angle) };
  const probe = { t: 'l', a: ca, b: g.add(ca, g.mul(dir, 1e6)) };
  const h = g.segIntersect(probe, sB, false, true)[0];
  if (!h) return { error: 'The chamfer angle does not meet the second line.' };
  const A = { t: 'l', a: sA.a, b: sA.b }, B = { t: 'l', a: sB.a, b: sB.b };
  return { a: keepSide(A, paramOf(A, pickA), paramOf(A, ca)), b: keepSide(B, paramOf(B, pickB), paramOf(B, h.pt)), line: { t: 'l', a: ca, b: h.pt } };
}

/** Walk every vertex of a polyline applying `corner(segA, segB)` (returns {a,b,add}). */
function cornerPass(e, doc, corner) {
  const path = pathOf(e, doc), segs = path.segs.map(s => ({ ...s })), n = segs.length;
  const inserts = new Array(n).fill(null);
  let applied = 0;
  const joints = path.closed ? n : n - 1;
  for (let i = 0; i < joints; i++) {
    const j = (i + 1) % n, A = segs[i], B = segs[j];
    const pa = g.segAt(A, .05), pb = g.segAt(B, .95);        // pick the far portions: those are the parts that survive
    const r = corner(A, pa, B, pb);
    if (r.error || !r.a || !r.b) continue;
    // the radius must fit: the trimmed segments may not vanish, and may not grow (an extension means it did not fit)
    if (g.segLength(r.a) < 1e-9 || g.segLength(r.b) < 1e-9) continue;
    if (g.segLength(r.a) > g.segLength(A) + 1e-9 || g.segLength(r.b) > g.segLength(B) + 1e-9) continue;
    segs[i] = r.a; segs[j] = r.b; inserts[i] = r.arc || r.line || null; applied++;
  }
  const out = [];
  segs.forEach((s, i) => { out.push(s); if (inserts[i]) out.push(inserts[i]); });
  return { entity: entityFromSegs(out, e, path.closed), applied };
}

export function filletPline(e, r, doc) {
  return cornerPass(e, doc, (A, pa, B, pb) => filletSegs(A, pa, B, pb, r));
}

export function chamferPline(e, d1, d2, doc) {
  return cornerPass(e, doc, (A, pa, B, pb) => chamferSegs(A, pa, B, pb, d1, d2));
}
