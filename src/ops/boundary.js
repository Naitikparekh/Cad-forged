// Closed-region detection from an internal point (HATCH pick-points, BOUNDARY, REGION).
//
// All curve segments in the drawing are split at their mutual intersections to form a planar graph. Every
// half-edge belongs to exactly one face cycle. The region containing the point is the smallest
// counter-clockwise cycle that encloses it; clockwise cycles (outer boundaries of other connected pieces)
// lying inside it are islands.
import * as g from '../core/geom.js';
import { segsOf } from '../core/entities.js';
import { edgeSegs } from './path.js';

const MAX_SEGS = 6000;
const TOL = 1e-6;

/** Split a segment at the sorted params `ts`, returning sub-segments (tiny ones dropped). */
function splitSeg(s, ts) {
  const pts = [0, ...ts.filter(t => t > 1e-9 && t < 1 - 1e-9), 1].sort((a, b) => a - b);
  const out = [];
  for (let i = 0; i + 1 < pts.length; i++) {
    if (pts[i + 1] - pts[i] < 1e-9) continue;
    const piece = g.segSlice(s, pts[i], pts[i + 1]);
    if (g.segLength(piece) > TOL) out.push(piece);
  }
  return out;
}

export function planarEdges(segs) {
  const bounds = segs.map(g.segBounds);
  const cuts = segs.map(() => []);
  for (let i = 0; i < segs.length; i++) {
    for (let j = i + 1; j < segs.length; j++) {
      if (!g.boundsOverlap(g.growBounds(bounds[i], 1e-7), bounds[j])) continue;
      for (const h of g.segIntersect(segs[i], segs[j])) { cuts[i].push(h.t1); cuts[j].push(h.t2); }
    }
  }
  const out = [];
  segs.forEach((s, i) => {
    if (s.full) {
      // a closed circle needs a node even without intersections
      const ts = cuts[i].length ? cuts[i] : [0];
      const sorted = [...new Set(ts.map(t => Math.round(t * 1e9) / 1e9))].sort((a, b) => a - b);
      if (sorted.length === 1) { out.push(...[g.segSlice(s, sorted[0], sorted[0] + .5), g.segSlice(s, sorted[0] + .5, sorted[0] + 1)]); return; }
      for (let k = 0; k < sorted.length; k++) {
        const t0 = sorted[k], t1 = k + 1 < sorted.length ? sorted[k + 1] : sorted[0] + 1;
        out.push(g.segSlice(s, t0, t1));
      }
      return;
    }
    out.push(...splitSeg(s, cuts[i]));
  });
  return out;
}

class Graph {
  constructor(edges) {
    this.nodes = []; this.keyMap = new Map(); this.half = [];
    for (const e of edges) {
      const a = this.node(g.segStart(e)), b = this.node(g.segEnd(e));
      if (a === b && Math.abs(e.sw ?? 0) < g.TAU - 1e-6 && e.t === 'a') continue;
      if (a === b && e.t === 'l') continue;
      const i = this.half.length;
      this.half.push({ id: i, from: a, to: b, seg: e, twin: i + 1 }, { id: i + 1, from: b, to: a, seg: g.segReverse(e), twin: i });
    }
    this.out = this.nodes.map(() => []);
    for (const h of this.half) this.out[h.from].push(h.id);
    for (const h of this.half) h.angle = g.angleOf({ x: 0, y: 0 }, g.segTangent(h.seg, 0));
  }

  node(p) {
    const k = `${Math.round(p.x / TOL)},${Math.round(p.y / TOL)}`;
    let id = this.keyMap.get(k);
    if (id === undefined) {
      for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1], [1, 1], [-1, -1], [1, -1], [-1, 1]]) {
        const [x, y] = k.split(',').map(Number);
        const near = this.keyMap.get(`${x + dx},${y + dy}`);
        if (near !== undefined && g.dist(this.nodes[near], p) < TOL * 2) { id = near; break; }
      }
    }
    if (id === undefined) { id = this.nodes.length; this.nodes.push({ x: p.x, y: p.y }); this.keyMap.set(k, id); }
    return id;
  }

  /** Remove edges that cannot be part of a cycle (degree-1 chains). */
  prune() {
    const alive = new Set(this.half.map(h => h.id));
    const deg = this.nodes.map((_, n) => this.out[n].length);
    const queue = deg.map((d, n) => d === 1 ? n : -1).filter(n => n >= 0);
    while (queue.length) {
      const n = queue.pop();
      for (const hid of this.out[n]) {
        if (!alive.has(hid)) continue;
        const h = this.half[hid];
        alive.delete(hid); alive.delete(h.twin);
        deg[h.from]--; deg[h.to]--;
        if (deg[h.to] === 1) queue.push(h.to);
      }
    }
    this.alive = alive;
    this.out = this.out.map(list => list.filter(id => alive.has(id)));
  }

  /** All face cycles as arrays of half-edge ids. */
  cycles() {
    const seen = new Set(), res = [];
    for (const hid of this.alive) {
      if (seen.has(hid)) continue;
      const cyc = [];
      let cur = hid, guard = 0;
      while (!seen.has(cur) && guard++ < this.half.length + 2) {
        seen.add(cur); cyc.push(cur);
        const h = this.half[cur], twin = this.half[h.twin];
        // next edge: first outgoing at h.to clockwise from the twin direction
        const twinAngle = twin.angle;
        let best = null, bestTurn = Infinity;
        for (const oid of this.out[h.to]) {
          if (oid === h.twin && this.out[h.to].length > 1) continue;
          const turn = g.normAngle(twinAngle - this.half[oid].angle) || g.TAU;
          if (turn < bestTurn) { bestTurn = turn; best = oid; }
        }
        if (best === null) { cur = h.twin; } else cur = best;
      }
      if (cyc.length && cur === hid) res.push(cyc);
    }
    return res;
  }
}

const tessCycle = (graph, cyc, tol = .02) => g.tessSegs(cyc.map(id => graph.half[id].seg), tol);

/** Verts with bulges for a cycle of half-edges. */
function cycleVerts(graph, cyc) {
  return g.segsToVerts(cyc.map(id => graph.half[id].seg), true);
}

/**
 * Find the region around `pt`.
 * @returns {{loops:Array<Array<{x,y,b}>>, outer:Array}|{error:string}}
 */
export function findRegion(doc, pt, entities = null) {
  const list = entities || doc.entities.filter(e => doc.isVisible(e));
  const segs = list.flatMap(e => e.type === 'hatch' ? [] : edgeSegs(e, doc)).filter(s => !(s.t === 'l' && Math.abs(s.a.x) > 5e5));
  if (segs.length > MAX_SEGS) return { error: 'The drawing has too many objects for boundary detection.' };
  if (!segs.length) return { error: 'No closed boundary found.' };
  const graph = new Graph(planarEdges(segs));
  graph.prune();
  const cycles = graph.cycles().map(c => {
    const poly = tessCycle(graph, c);
    return { c, poly, area: g.polygonArea(poly) };
  });
  const enclosing = cycles.filter(c => c.area > 1e-9 && g.pointInPolygon(pt, c.poly)).sort((a, b) => a.area - b.area);
  if (!enclosing.length) return { error: 'Unable to determine a valid closed boundary at that point.' };
  const outer = enclosing[0];
  // component id for each node, to exclude the outer's own component from islands
  const comp = new Array(graph.nodes.length).fill(-1);
  let cid = 0;
  for (let n = 0; n < graph.nodes.length; n++) {
    if (comp[n] >= 0 || !graph.out[n].length) continue;
    const stack = [n]; comp[n] = cid;
    while (stack.length) {
      const m = stack.pop();
      for (const hid of graph.out[m]) { const t = graph.half[hid].to; if (comp[t] < 0) { comp[t] = cid; stack.push(t); } }
    }
    cid++;
  }
  const outerComp = comp[graph.half[outer.c[0]].from];
  const loops = [cycleVerts(graph, outer.c)];
  for (const c of cycles) {
    if (c.area >= -1e-9 || c === outer) continue;
    if (comp[graph.half[c.c[0]].from] === outerComp) continue;
    const probe = c.poly[0];
    if (g.pointInPolygon(probe, outer.poly)) loops.push(cycleVerts(graph, c.c));
  }
  return { loops, outer: loops[0] };
}

/** All closed loops found when picking from a window of points (used by BOUNDARY multi-pick). */
export { segsOf };
