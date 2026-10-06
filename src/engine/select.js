// Picking and rectangle/polygon/fence selection.
import * as g from '../core/geom.js';
import { boundsOf, inWindow, crossesRect, pickDistance, segsOf, textPolygons } from '../core/entities.js';

/** May the entity be selected (visible, unlocked layer)? */
export const selectable = (doc, e) => doc.isVisible(e) && !doc.isLocked(e) && e.type !== 'viewport';

/**
 * Topmost entity within `ap` world units of p. Nearest wins; ties go to the later (upper) entity.
 * Filled hatches yield to line work.
 */
export function pickAt(doc, p, ap, { filter = null, exclude = null } = {}) {
  let best = null, bestScore = Infinity;
  const win = { minX: p.x - ap, minY: p.y - ap, maxX: p.x + ap, maxY: p.y + ap };
  for (const e of doc.entities) {
    if (!selectable(doc, e) || (filter && !filter(e)) || (exclude && exclude.has(e.id))) continue;
    const b = boundsOf(e, doc);
    if (e.type !== 'xline' && e.type !== 'ray' && (!b || !g.boundsOverlap(b, win))) continue;
    let d = pickDistance(e, p, doc);
    if (d > ap) continue;
    if (e.type === 'hatch') d += ap * .9;
    if (d <= bestScore + 1e-12) { best = e; bestScore = d; }
  }
  return best;
}

/** All entities under p, nearest first (for selection cycling). */
export function pickAll(doc, p, ap, opts = {}) {
  const list = [];
  const win = { minX: p.x - ap, minY: p.y - ap, maxX: p.x + ap, maxY: p.y + ap };
  for (const e of doc.entities) {
    if (!selectable(doc, e) || (opts.filter && !opts.filter(e))) continue;
    const b = boundsOf(e, doc);
    if (e.type !== 'xline' && e.type !== 'ray' && (!b || !g.boundsOverlap(b, win))) continue;
    const d = pickDistance(e, p, doc);
    if (d <= ap) list.push({ e, d });
  }
  return list.sort((a, b) => a.d - b.d).map(x => x.e);
}

export const rectOf = (a, b) => ({ minX: Math.min(a.x, b.x), minY: Math.min(a.y, b.y), maxX: Math.max(a.x, b.x), maxY: Math.max(a.y, b.y) });

/** Window (fully inside) or crossing (touching) selection. */
export function selectRect(doc, rect, crossing, { filter = null } = {}) {
  const out = [];
  for (const e of doc.entities) {
    if (!selectable(doc, e) || (filter && !filter(e))) continue;
    if (crossing ? crossesRect(e, rect, doc) : inWindow(e, rect, doc)) out.push(e);
  }
  return out;
}

const samplePoints = (e, doc) => {
  const pts = [];
  for (const s of segsOf(e, doc)) pts.push(...g.tessSeg(s, .05));
  for (const poly of textPolygons(e, doc)) pts.push(...poly);
  if (e.type === 'point') pts.push(e.p);
  if (e.type === 'hatch') for (const l of e.loops) pts.push(...g.tessSegs(g.plineSegs(l, true), .05));
  return pts;
};

const polyEdges = (poly, closed = true) => poly.map((a, i) => ({ t: 'l', a, b: poly[(i + 1) % poly.length] })).slice(0, closed ? poly.length : poly.length - 1);

const crossesPath = (e, doc, edges) => {
  for (const s of segsOf(e, doc)) for (const q of edges) if (g.segIntersect(s, q).length) return true;
  for (const tp of textPolygons(e, doc)) for (let i = 0; i < tp.length; i++) for (const q of edges) if (g.segIntersect({ t: 'l', a: tp[i], b: tp[(i + 1) % tp.length] }, q).length) return true;
  return false;
};

/** Window polygon (all inside) or crossing polygon (any touch). */
export function selectPolygon(doc, poly, crossing, { filter = null } = {}) {
  const edges = polyEdges(poly), out = [];
  for (const e of doc.entities) {
    if (!selectable(doc, e) || (filter && !filter(e))) continue;
    const pts = samplePoints(e, doc);
    if (!pts.length) continue;
    const allIn = pts.every(p => g.pointInPolygon(p, poly)), anyIn = pts.some(p => g.pointInPolygon(p, poly));
    if (crossing ? (anyIn || crossesPath(e, doc, edges)) : (allIn && !crossesPath(e, doc, edges))) out.push(e);
  }
  return out;
}

/** Fence: entities crossed by an open polyline. */
export function selectFence(doc, pts, { filter = null } = {}) {
  const edges = polyEdges(pts, false);
  return doc.entities.filter(e => selectable(doc, e) && (!filter || filter(e)) && crossesPath(e, doc, edges));
}
