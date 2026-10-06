// OFFSET: parallel copies of lines, arcs, circles, polylines (with extended corners), ellipses and splines.
import * as g from '../core/geom.js';
import { E, segsOf, ellipsePoint, ellipseSweep, ellipseIsFull, splinePoints } from '../core/entities.js';
import { pathOf, entityFromSegs, inheritProps } from './path.js';

/** Which side of a path (left=+1, right=-1) is the point on? */
export function sideOf(segs, p) {
  let best = { d: Infinity, side: 1 };
  for (const s of segs) {
    const r = g.segNearest(s, p);
    if (r.d < best.d) {
      const tan = g.segTangent(s, r.t), v = g.sub(p, r.pt);
      let side = g.cross(tan, v) >= 0 ? 1 : -1;
      if (s.t === 'a' && r.d > 0) side = (g.dist(p, s.c) < s.r) === (s.sw > 0) ? 1 : -1;   // inside a CCW arc is its left
      best = { d: r.d, side };
    }
  }
  return best.side;
}

/**
 * Offset segments by distance d to the left (negative = right).
 * @returns {Array|null} new segments, or null if the offset collapses completely
 */
export function offsetSegs(segs, d, closed) {
  let offs = segs.map(s => g.segOffset(s, d));
  // iterative removal of collapsed / reversed segments
  for (let pass = 0; pass < 4; pass++) {
    const alive = offs.map((o, i) => ({ o, i })).filter(x => x.o);
    if (!alive.length) return null;
    const n = alive.length, pts = [];
    // joint k joins alive[k-1] and alive[k]
    for (let k = 0; k < n; k++) {
      const prev = alive[(k + n - 1) % n], cur = alive[k];
      if (!closed && k === 0) { pts.push(g.segStart(cur.o)); continue; }
      const a = prev.o, b = cur.o, endA = g.segEnd(a), startB = g.segStart(b);
      if (g.dist(endA, startB) < 1e-9) { pts.push(endA); continue; }
      // original joint is where the unoffset segments meet; choose the carrier intersection closest to the offset ends
      const hits = g.segIntersect(a, b, true, true);
      const mid = g.mid(endA, startB);
      let best = null;
      for (const h of hits) if (!best || g.dist(h.pt, mid) < g.dist(best, mid)) best = h.pt;
      pts.push(best || mid);
    }
    const lastPt = closed ? null : g.segEnd(alive[n - 1].o);
    // rebuild each segment between its joints; detect reversal
    const rebuilt = [], bad = [];
    for (let k = 0; k < n; k++) {
      const o = alive[k].o, p0 = pts[k], p1 = closed ? pts[(k + 1) % n] : (k + 1 < n ? pts[k + 1] : lastPt);
      let ns;
      if (o.t === 'l') {
        ns = { t: 'l', a: p0, b: p1 };
        if (g.dot(g.sub(p1, p0), g.sub(o.b, o.a)) < -1e-12) bad.push(alive[k].i);
      } else {
        const a0 = g.angleOf(o.c, p0), a1 = g.angleOf(o.c, p1);
        let sw = o.sw >= 0 ? g.sweepCCW(a0, a1) : -g.sweepCCW(a1, a0);
        if (Math.abs(sw) > Math.abs(o.sw) + 1e-6 && Math.abs(o.sw) < g.TAU - 1e-6) bad.push(alive[k].i);
        if (g.dist(p0, p1) < 1e-12 && Math.abs(o.sw) >= g.TAU - 1e-9) sw = o.sw;
        ns = { t: 'a', c: o.c, r: o.r, a0, sw };
      }
      rebuilt.push(ns);
    }
    if (!bad.length) return rebuilt;
    for (const i of bad) offs[i] = null;
  }
  return null;
}

/** Offset any supported entity. `side` is +1 (left of travel) or -1; `through` lets the caller pick by point. */
export function offsetEntity(e, dist, sidePoint, doc) {
  const lineSide = segs => sideOf(segs, sidePoint);
  switch (e.type) {
    case 'line': {
      const segs = segsOf(e, doc), d = dist * lineSide(segs);
      const o = g.segOffset(segs[0], d);
      return { result: inheritProps(e, E.line(o.a, o.b)) };
    }
    case 'circle': {
      const outside = g.dist(sidePoint, e.c) > e.r, r = outside ? e.r + dist : e.r - dist;
      if (r <= 1e-9) return { error: 'The offset distance is larger than the circle radius.' };
      return { result: inheritProps(e, E.circle(e.c, r)) };
    }
    case 'arc': {
      const outside = g.dist(sidePoint, e.c) > e.r, r = outside ? e.r + dist : e.r - dist;
      if (r <= 1e-9) return { error: 'The offset distance is larger than the arc radius.' };
      return { result: inheritProps(e, E.arc(e.c, r, e.a0, e.a1)) };
    }
    case 'pline': {
      const path = pathOf(e, doc), d = dist * lineSide(path.segs);
      const segs = offsetSegs(path.segs, d, path.closed);
      if (!segs) return { error: 'The offset distance is too large for this polyline.' };
      return { result: entityFromSegs(segs, e, path.closed) };
    }
    case 'ellipse': case 'spline': {
      const pts = e.type === 'ellipse' ? Array.from({ length: 129 }, (_, i) => ellipsePoint(e, e.t0 + ellipseSweep(e) * i / 128)) : splinePoints(e);
      const lines = [];
      for (let i = 0; i + 1 < pts.length; i++) lines.push({ t: 'l', a: pts[i], b: pts[i + 1] });
      const closed = e.type === 'ellipse' ? ellipseIsFull(e) : !!e.closed;
      const segs = offsetSegs(lines, dist * sideOf(lines, sidePoint), closed);
      if (!segs) return { error: 'The offset distance is too large.' };
      return { result: entityFromSegs(segs, e, closed), approximated: true };
    }
    case 'xline': return { result: inheritProps(e, E.xline(g.add(e.p, g.mul(g.perp(e.d), dist * (g.dot(g.sub(sidePoint, e.p), g.perp(e.d)) >= 0 ? 1 : -1))), e.d)) };
    default: return { error: 'That object cannot be offset.' };
  }
}
