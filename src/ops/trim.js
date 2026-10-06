// TRIM, EXTEND, BREAK, JOIN and LENGTHEN on line / arc / circle / polyline paths.
import * as g from '../core/geom.js';
import { E } from '../core/entities.js';
import { pathOf, pathNearest, pathSlice, entityFromSegs, edgeSegs, inheritProps, sliceLinear, mergeSegs } from './path.js';

const EPS = 1e-9;

function cutParams(path, cutSegs, extendCutters) {
  const us = [];
  path.segs.forEach((ts, i) => {
    for (const cs of cutSegs) {
      for (const h of g.segIntersect(ts, cs, false, extendCutters)) us.push(i + Math.min(1, Math.max(0, h.t1)));
    }
  });
  us.sort((a, b) => a - b);
  const n = path.segs.length, out = [];
  for (const u of us) {
    const v = path.closed && n - u < 1e-7 ? 0 : u;
    if (!out.some(o => Math.abs(o - v) < 1e-7)) out.push(v);
  }
  return out.sort((a, b) => a - b);
}

/**
 * Trim the part of `target` under `pick` back to the nearest cutting edges.
 * @returns {{replace:Array}|{error:string}}  replace = entities that should take the target's place (possibly empty)
 */
export function trim(target, pick, cutters, doc, { extendCutters = false } = {}) {
  const path = pathOf(target, doc);
  if (!path) return { error: 'That object cannot be trimmed.' };
  const cutSegs = cutters.filter(c => c !== target).flatMap(c => edgeSegs(c, doc));
  const cuts = cutParams(path, cutSegs, extendCutters);
  const n = path.segs.length, up = pathNearest(path, pick).u;
  if (!cuts.length) return { error: 'Object does not intersect an edge.' };
  const mk = segs => entityFromSegs(segs, target, false);

  if (!path.closed) {
    const prev = [...cuts].reverse().find(u => u < up - EPS), next = cuts.find(u => u > up + EPS);
    const inner = cuts.filter(u => u > 1e-7 && u < n - 1e-7);
    if (!inner.length && !(prev !== undefined && next !== undefined)) return { error: 'Object does not intersect an edge.' };
    const a = prev ?? 0, b = next ?? n;
    const pieces = [];
    if (a > 1e-7) pieces.push(mk(sliceLinear(path.segs, 0, a)));
    if (b < n - 1e-7) pieces.push(mk(sliceLinear(path.segs, b, n)));
    return { replace: pieces.filter(Boolean) };
  }
  if (cuts.length < 2) return { error: 'A closed object needs at least two cutting points.' };
  const prev = [...cuts].reverse().find(u => u < up - EPS) ?? cuts[cuts.length - 1];
  const next = cuts.find(u => u > up + EPS) ?? cuts[0];
  const rest = pathSlice(path, next, prev);          // from next cut around to previous cut
  return { replace: [mk(mergeSegs(rest))].filter(Boolean) };
}

/** Extension of the last segment in its direction: candidate param t > 1 along the carrier. */
function extensionHits(seg, bounds, extendBounds) {
  const hits = [];
  const full = seg.t === 'a' ? g.TAU / Math.abs(seg.sw) : Infinity;     // param of one full lap
  for (const b of bounds) {
    for (const h of g.segIntersect(seg, b, true, extendBounds)) {
      let t = h.t1;
      if (seg.t === 'a' && t < 0) t += full;                            // behind the start = ahead after wrapping
      if (t > 1 + 1e-9 && t < full - 1e-9) hits.push({ t, pt: h.pt });
    }
  }
  return hits.sort((a, b) => a.t - b.t);
}

/**
 * Extend the end of `target` nearest `pick` to the closest boundary.
 * @returns {{replace:Array}|{error:string}}
 */
export function extend(target, pick, boundaries, doc, { extendBoundaries = false } = {}) {
  if (!['line', 'arc', 'pline'].includes(target.type) || (target.type === 'pline' && target.closed)) return { error: 'That object cannot be extended.' };
  let path = pathOf(target, doc);
  const nearStart = g.dist(pick, g.segStart(path.segs[0])) < g.dist(pick, g.segEnd(path.segs[path.segs.length - 1]));
  let segs = path.segs.slice();
  if (nearStart) segs = segs.reverse().map(g.segReverse);
  const last = segs[segs.length - 1];
  const bounds = boundaries.filter(b => b !== target).flatMap(b => edgeSegs(b, doc));
  const hit = extensionHits(last, bounds, extendBoundaries)[0];
  if (!hit) return { error: 'No boundary in the extension direction.' };
  let ext;
  if (last.t === 'l') ext = { t: 'l', a: last.a, b: hit.pt };
  else ext = { ...last, sw: last.sw * hit.t };
  segs[segs.length - 1] = ext;
  if (nearStart) segs = segs.reverse().map(g.segReverse);
  return { replace: [entityFromSegs(segs, target, false)] };
}

/** BREAK between two points (pick order matters on circles: removal runs CCW from p1 to p2). */
export function breakAt(target, p1, p2, doc) {
  const path = pathOf(target, doc);
  if (!path) return { error: 'That object cannot be broken.' };
  const n = path.segs.length;
  let u1 = pathNearest(path, p1).u, u2 = pathNearest(path, p2).u;
  const mk = segs => entityFromSegs(segs, target, false);
  if (!path.closed) {
    if (u1 > u2) [u1, u2] = [u2, u1];
    const pieces = [];
    if (u1 > 1e-7) pieces.push(mk(sliceLinear(path.segs, 0, u1)));
    if (u2 < n - 1e-7) pieces.push(mk(sliceLinear(path.segs, u2, n)));
    return { replace: pieces.filter(Boolean) };
  }
  if (Math.abs(u1 - u2) < 1e-9) return { error: 'The break points coincide.' };
  const rest = pathSlice(path, u2, u1);
  return { replace: [mk(mergeSegs(rest))].filter(Boolean) };
}

/** Split an object in two at a point (BREAK with @ / BREAKATPOINT). */
export function splitAt(target, p, doc) {
  const path = pathOf(target, doc);
  if (!path || path.closed) return { error: 'That object cannot be split at a point.' };
  const n = path.segs.length, u = pathNearest(path, p).u;
  if (u < 1e-7 || u > n - 1e-7) return { error: 'The point is at an end of the object.' };
  return { replace: [entityFromSegs(sliceLinear(path.segs, 0, u), target), entityFromSegs(sliceLinear(path.segs, u, n), target)] };
}

/**
 * JOIN: chain connected lines, arcs and polylines. Returns the joined entities (one per chain) and which sources were used.
 */
export function join(entities, doc, tol = 1e-6) {
  const items = entities.map(e => ({ e, path: pathOf(e, doc) })).filter(i => i.path && !i.path.closed);
  const used = new Set(), results = [];
  for (const start of items) {
    if (used.has(start)) continue;
    used.add(start);
    let segs = start.path.segs.slice();
    let grew = true;
    while (grew) {
      grew = false;
      for (const it of items) {
        if (used.has(it)) continue;
        const head = g.segStart(segs[0]), tail = g.segEnd(segs[segs.length - 1]);
        const s0 = g.segStart(it.path.segs[0]), s1 = g.segEnd(it.path.segs[it.path.segs.length - 1]);
        const rev = () => it.path.segs.slice().reverse().map(g.segReverse);
        if (g.dist(tail, s0) <= tol) segs = [...segs, ...it.path.segs];
        else if (g.dist(tail, s1) <= tol) segs = [...segs, ...rev()];
        else if (g.dist(head, s1) <= tol) segs = [...it.path.segs, ...segs];
        else if (g.dist(head, s0) <= tol) segs = [...rev(), ...segs];
        else continue;
        used.add(it); grew = true;
      }
    }
    // merge collinear lines / co-circular arcs
    const merged = [];
    for (const s of segs) {
      const last = merged[merged.length - 1];
      if (last && last.t === 'l' && s.t === 'l' && Math.abs(g.cross(g.sub(last.b, last.a), g.sub(s.b, s.a))) < 1e-9 * g.len(g.sub(last.b, last.a)) * g.len(g.sub(s.b, s.a)) && g.dot(g.sub(last.b, last.a), g.sub(s.b, s.a)) > 0) merged[merged.length - 1] = { t: 'l', a: last.a, b: s.b };
      else merged.push(s);
    }
    const arcMerged = mergeSegs(merged);
    const closed = arcMerged.length > 1 && g.dist(g.segStart(arcMerged[0]), g.segEnd(arcMerged[arcMerged.length - 1])) <= tol;
    if (arcMerged.length === 1 && arcMerged[0].t === 'a' && Math.abs(arcMerged[0].sw) >= g.TAU - 1e-9) results.push(inheritProps(start.e, E.circle(arcMerged[0].c, arcMerged[0].r)));
    else results.push(entityFromSegs(arcMerged, start.e, closed));
  }
  return { results, sources: items.map(i => i.e), used: items.filter(i => used.has(i)).map(i => i.e) };
}

/**
 * LENGTHEN the end nearest `pick` of a line/arc/open polyline.
 * mode: 'delta' (add length), 'percent' (new length as % of old), 'total' (new total length).
 */
export function lengthen(target, pick, mode, value, doc) {
  const path = pathOf(target, doc);
  if (!path || path.closed || !['line', 'arc', 'pline'].includes(target.type)) return { error: 'That object cannot be lengthened.' };
  let segs = path.segs.slice();
  const nearStart = g.dist(pick, g.segStart(segs[0])) < g.dist(pick, g.segEnd(segs[segs.length - 1]));
  if (nearStart) segs = segs.reverse().map(g.segReverse);
  const total = segs.reduce((s, q) => s + g.segLength(q), 0);
  const delta = mode === 'delta' ? value : mode === 'percent' ? total * value / 100 - total : value - total;
  const last = segs[segs.length - 1], L = g.segLength(last), newL = L + delta;
  if (newL <= EPS) return { error: 'The resulting length would be zero or negative.' };
  let ext;
  if (last.t === 'l') { const u = g.unit(g.sub(last.b, last.a)); ext = { t: 'l', a: last.a, b: g.add(last.a, g.mul(u, newL)) }; }
  else { const sw = Math.sign(last.sw) * newL / last.r; if (Math.abs(sw) > g.TAU) return { error: 'The arc would exceed a full circle.' }; ext = { ...last, sw }; }
  segs[segs.length - 1] = ext;
  if (nearStart) segs = segs.reverse().map(g.segReverse);
  return { replace: [entityFromSegs(segs, target)] };
}
