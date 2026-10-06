// Parametric paths: an entity's segments in order, addressed by u = segmentIndex + t.
import * as g from '../core/geom.js';
import { E, segsOf } from '../core/entities.js';

const INHERIT = ['layer', 'color', 'ltype', 'lw', 'lts', 'transp', 'grp'];
export const inheritProps = (from, to) => { for (const k of INHERIT) if (from[k] !== undefined) to[k] = from[k]; return to; };

/** Editable path of a line/arc/circle/pline, or null for entities that cannot be edited parametrically. */
export function pathOf(e, doc) {
  switch (e.type) {
    case 'line': case 'arc': return { segs: segsOf(e, doc), closed: false };
    case 'circle': return { segs: segsOf(e, doc), closed: true };
    case 'pline': return { segs: segsOf(e, doc), closed: !!e.closed };
    default: return null;
  }
}

/** Nearest point on a path: { u, pt, d }. */
export function pathNearest(path, p) {
  let best = { u: 0, pt: null, d: Infinity };
  path.segs.forEach((s, i) => {
    const r = g.segNearest(s, p);
    if (r.d < best.d) best = { u: i + r.t, pt: r.pt, d: r.d };
  });
  return best;
}

export const pathLength = path => path.segs.reduce((s, q) => s + g.segLength(q), 0);

/** Segments from u0 to u1 on the same lap (0 <= u0 <= u1 <= n). */
export function sliceLinear(segs, u0, u1) {
  const out = [], eps = 1e-9;
  for (let i = Math.floor(u0 + eps); i < segs.length && i < u1 - eps; i++) {
    const ta = Math.max(0, u0 - i), tb = Math.min(1, u1 - i);
    if (tb - ta > eps) out.push(g.segSlice(segs[i], ta, tb));
  }
  return out;
}

/** Slice a path from u0 to u1; on closed paths u1<u0 wraps through the start. */
export function pathSlice(path, u0, u1) {
  const n = path.segs.length;
  if (u1 >= u0) return sliceLinear(path.segs, u0, u1);
  if (!path.closed) return sliceLinear(path.segs, u1, u0).reverse().map(g.segReverse);
  return [...sliceLinear(path.segs, u0, n), ...sliceLinear(path.segs, 0, u1)];
}

/** Merge consecutive arcs of the same circle into one (so a trimmed circle is a single arc). */
export function mergeSegs(segs) {
  const out = [];
  for (const s of segs) {
    const last = out[out.length - 1];
    if (last && last.t === 'a' && s.t === 'a' && g.samePt(last.c, s.c, 1e-9) && Math.abs(last.r - s.r) < 1e-9 && Math.sign(last.sw) === Math.sign(s.sw)
      && g.samePt(g.segEnd(last), g.segStart(s), 1e-7) && Math.abs(last.sw) + Math.abs(s.sw) <= g.TAU + 1e-9) {
      out[out.length - 1] = { ...last, sw: last.sw + s.sw };
    } else out.push(s);
  }
  return out;
}

/** Build the simplest entity for a run of connected segments. */
export function entityFromSegs(segs, template, closed = false) {
  segs = mergeSegs(segs);
  if (!segs.length) return null;
  let e;
  if (segs.length === 1 && !closed) {
    const s = segs[0];
    e = s.t === 'l' ? E.line(s.a, s.b) : E.arcSeg(s);
  } else {
    e = E.pline(g.segsToVerts(segs, closed), closed);
  }
  return inheritProps(template, e);
}

/** Is an entity of a type that TRIM/EXTEND/BREAK/JOIN can edit as a path? */
export const isPathEntity = e => ['line', 'arc', 'circle', 'pline'].includes(e.type);

/** Cutting/boundary geometry of any entity. */
export const edgeSegs = (e, doc) => ['line', 'arc', 'circle', 'pline', 'ellipse', 'spline', 'xline', 'ray', 'leader'].includes(e.type) ? segsOf(e, doc) : [];
