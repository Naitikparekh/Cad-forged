// Point acquisition: object snaps, polar/ortho tracking, object-snap tracking and grid snap.
import * as g from '../core/geom.js';
import { segsOf, boundsOf, snapPointsOf } from '../core/entities.js';
import { OS_BITS, OS_FULL } from './parse.js';

const INTRINSIC = ['END', 'MID', 'CEN', 'NOD', 'QUA', 'INS'];
const PRIORITY = { END: 0, INT: 1, MID: 2, CEN: 3, NOD: 3, QUA: 4, INS: 4, PER: 5, TAN: 5, APP: 6, NEA: 9 };
export const MAX_ACQUIRED = 7;

/**
 * @typedef {object} SnapOptions
 * @property {number} scale           pixels per drawing unit
 * @property {number} [aperturePx]    snap radius in pixels
 * @property {number} [osmask]        enabled object snap bits (already resolved from OSMODE / overrides)
 * @property {{x,y}} [base]           previous point (for PER/TAN/polar/DDE)
 * @property {boolean} [ortho]
 * @property {boolean} [polar]
 * @property {number} [polarInc]      radians
 * @property {number[]} [polarAdd]    extra absolute polar angles (radians)
 * @property {boolean} [otrack]
 * @property {{on:boolean,sx:number,sy:number,base:{x,y},angle:number}} [grid]
 */

export class Snapper {
  constructor() { this.acquired = []; this.dwell = null; }

  reset() { this.acquired = []; this.dwell = null; }

  /** Record that the cursor is resting on a snap point; returns true when acquisition state changed. */
  noteHover(snap, now) {
    if (!snap) { this.dwell = null; return false; }
    if (!this.dwell || !g.samePt(this.dwell.pt, snap.pt, 1e-7)) { this.dwell = { pt: snap.pt, since: now, done: false }; return false; }
    if (this.dwell.done || now - this.dwell.since < 300) return false;
    this.dwell.done = true;
    const i = this.acquired.findIndex(a => g.samePt(a, snap.pt, 1e-7));
    if (i >= 0) this.acquired.splice(i, 1);
    else { this.acquired.push({ ...snap.pt }); if (this.acquired.length > MAX_ACQUIRED) this.acquired.shift(); }
    return true;
  }

  /**
   * Resolve a raw world point into the point to use.
   * @returns {{pt,{x,y}, snap:{type,pt}|null, track:{lines:Array,marks:Array,label:string}|null}}
   */
  resolve(doc, raw, o) {
    const ap = (o.aperturePx ?? 12) / o.scale;
    const out = { pt: { x: raw.x, y: raw.y }, snap: null, track: null };

    // 1. object snaps
    if (o.osmask) {
      const hit = this.objectSnap(doc, raw, ap, o);
      if (hit) { out.pt = { ...hit.pt }; out.snap = hit; return out; }
    }

    // 2. ortho / polar / object-snap tracking
    const tracked = this.tracking(raw, ap, o);
    if (tracked) { out.pt = tracked.pt; out.track = tracked; return out; }

    // 3. grid snap
    if (o.grid?.on) out.pt = snapToGrid(raw, o.grid);
    return out;
  }

  // ------------------------------------------------------------ object snap

  objectSnap(doc, raw, ap, o) {
    const mask = o.osmask;
    const win = { minX: raw.x - ap, minY: raw.y - ap, maxX: raw.x + ap, maxY: raw.y + ap };
    const near = [];
    for (const e of doc.entities) {
      if (!doc.isVisible(e)) continue;
      const b = boundsOf(e, doc);
      if (e.type === 'xline' || e.type === 'ray') { near.push(e); continue; }
      if (b && g.boundsOverlap(b, win)) near.push(e);
    }
    const cands = [];
    const add = (type, pt, trigger = g.dist(raw, pt)) => { if (trigger <= ap) cands.push({ type, pt, d: trigger }); };

    for (const e of near) {
      for (const s of snapPointsOf(e, doc)) if (mask & OS_BITS[s.type]) add(s.type, s.pt);
    }
    const segsNear = near.map(e => ({ e, segs: segsOf(e, doc) }));
    if (mask & OS_BITS.INT || mask & OS_BITS.APP) {
      for (let i = 0; i < segsNear.length; i++) {
        for (let j = i; j < segsNear.length; j++) {
          for (const sa of segsNear[i].segs) for (const sb of segsNear[j].segs) {
            if (i === j && sa === sb) continue;
            for (const h of g.segIntersect(sa, sb)) add('INT', h.pt);
          }
        }
      }
    }
    if (o.base && (mask & (OS_BITS.PER | OS_BITS.TAN))) {
      for (const { segs } of segsNear) for (const s of segs) {
        const nearest = g.segNearest(s, raw);
        if (nearest.d > ap) continue;
        if (mask & OS_BITS.PER) for (const f of g.perpFeet(s, o.base)) if (f.t >= -1e-9 && f.t <= 1 + 1e-9) cands.push({ type: 'PER', pt: f.pt, d: nearest.d });
        if (mask & OS_BITS.TAN && s.t === 'a') {
          const tp = g.tangentPoints(s, o.base).filter(q => q.t >= -1e-9 && q.t <= 1 + 1e-9);
          tp.sort((p, q) => g.dist(p.pt, raw) - g.dist(q.pt, raw));
          if (tp[0]) cands.push({ type: 'TAN', pt: tp[0].pt, d: nearest.d });
        }
      }
    }
    if (mask & OS_BITS.NEA) {
      for (const { segs } of segsNear) for (const s of segs) {
        const r = g.segNearest(s, raw);
        if (r.d <= ap) cands.push({ type: 'NEA', pt: r.pt, d: r.d });
      }
    }
    // a hatch/dimension overlapping nothing else should not produce spurious results
    if (!cands.length) return null;
    cands.sort((a, b) => (a.d - b.d) || (PRIORITY[a.type] - PRIORITY[b.type]));
    // NEA is a fallback: take it only if nothing else qualifies
    const best = cands.find(c => c.type !== 'NEA') || cands[0];
    return { type: best.type, pt: best.pt, name: OS_FULL[best.type] };
  }

  // ------------------------------------------------------------ tracking

  tracking(raw, ap, o) {
    const base = o.base;
    const vectors = [];       // {origin, dir, kind, angle}
    if (base && o.ortho) {
      const dx = raw.x - base.x, dy = raw.y - base.y;
      const horizontal = Math.abs(dx) >= Math.abs(dy);
      const pt = horizontal ? { x: raw.x, y: base.y } : { x: base.x, y: raw.y };
      const d = g.dist(base, pt);
      const a = horizontal ? (dx >= 0 ? 0 : Math.PI) : (dy >= 0 ? Math.PI / 2 : -Math.PI / 2);
      return { pt, lines: [], marks: [], label: `Ortho: ${fmt(d)} < ${degs(a)}`, kind: 'ortho', dist: d, angle: a };
    }
    const angles = o.polar ? polarAngles(o.polarInc || Math.PI / 2, o.polarAdd) : [0, Math.PI / 2, Math.PI, 3 * Math.PI / 2];
    if (base && o.polar) for (const a of angles) vectors.push({ origin: base, dir: { x: Math.cos(a), y: Math.sin(a) }, kind: 'polar', angle: a });
    if (o.otrack) for (const acq of this.acquired) for (const a of angles) vectors.push({ origin: acq, dir: { x: Math.cos(a), y: Math.sin(a) }, kind: 'otrack', angle: a });
    if (!vectors.length) return null;

    const hits = [];
    for (const v of vectors) {
      const w = g.sub(raw, v.origin), t = g.dot(w, v.dir), d = Math.abs(g.cross(w, v.dir));
      if (t > ap * .25 && d <= ap) hits.push({ v, t, d });
    }
    if (!hits.length) return null;
    hits.sort((a, b) => a.d - b.d);
    const first = hits[0];
    // intersection of two tracking vectors from different origins
    for (const h2 of hits.slice(1)) {
      if (g.samePt(h2.v.origin, first.v.origin, 1e-9) || Math.abs(g.cross(first.v.dir, h2.v.dir)) < 1e-6) continue;
      const a = { t: 'l', a: first.v.origin, b: g.add(first.v.origin, g.mul(first.v.dir, 1e7)) }, b = { t: 'l', a: h2.v.origin, b: g.add(h2.v.origin, g.mul(h2.v.dir, 1e7)) };
      const x = g.segIntersect(a, b)[0];
      if (x && g.dist(x.pt, raw) <= ap * 1.5) {
        return { pt: x.pt, lines: [mkLine(first.v, x.pt), mkLine(h2.v, x.pt)], marks: this.acquired, label: `Intersection`, kind: 'intersection' };
      }
    }
    const pt = g.add(first.v.origin, g.mul(first.v.dir, first.t));
    const label = first.v.kind === 'polar' ? `Polar: ${fmt(first.t)} < ${degs(first.v.angle)}` : `Track: ${fmt(first.t)} < ${degs(first.v.angle)}`;
    return { pt, lines: [mkLine(first.v, pt)], marks: this.acquired, label, kind: first.v.kind, dist: first.t, angle: first.v.angle };
  }
}

const fmt = n => (Math.round(n * 10000) / 10000).toString();
const degs = a => (Math.round(g.deg(g.normAngle(a)) * 100) / 100) + '°';
const mkLine = (v, to) => ({ a: v.origin, b: g.add(to, g.mul(v.dir, 1e6)), through: to, kind: v.kind });

export function polarAngles(inc, extra = []) {
  const out = [];
  const n = Math.max(1, Math.round(g.TAU / inc));
  for (let i = 0; i < n; i++) out.push(i * inc);
  for (const a of extra || []) out.push(a);
  return out;
}

export function snapToGrid(p, grid) {
  const c = Math.cos(-(grid.angle || 0)), s = Math.sin(-(grid.angle || 0));
  const dx = p.x - grid.base.x, dy = p.y - grid.base.y;
  const u = dx * c - dy * s, v = dx * s + dy * c;
  const su = Math.round(u / grid.sx) * grid.sx, sv = Math.round(v / grid.sy) * grid.sy;
  const c2 = Math.cos(grid.angle || 0), s2 = Math.sin(grid.angle || 0);
  return { x: grid.base.x + su * c2 - sv * s2, y: grid.base.y + su * s2 + sv * c2 };
}

/** Resolve OSMODE + an optional one-shot override into the active bit mask. */
export function activeMask(osmode, override) {
  if (override === 'none') return 0;
  if (typeof override === 'number') return override;
  if (osmode & 16384) return 0;
  return osmode & ~(16384 | 1024);
}

export { INTRINSIC };
