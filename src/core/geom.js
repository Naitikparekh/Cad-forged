// Pure 2D geometry. No DOM, no document state: everything is points {x,y} and segments.
//
// A segment is either a line  { t:'l', a, b }
// or a circular arc           { t:'a', c, r, a0, sw }   (start angle a0, signed sweep sw; sw>0 is CCW)
// Segments are directed: start -> end. Polylines, arcs, circles and spline tessellations are all
// reduced to segments, so intersection, trim, offset and snapping share one implementation.

export const TAU = Math.PI * 2;
export const EPS = 1e-9;

export const P = (x, y) => ({ x, y });
export const dist = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);
export const add = (a, b) => ({ x: a.x + b.x, y: a.y + b.y });
export const sub = (a, b) => ({ x: a.x - b.x, y: a.y - b.y });
export const mul = (a, s) => ({ x: a.x * s, y: a.y * s });
export const dot = (a, b) => a.x * b.x + a.y * b.y;
export const cross = (a, b) => a.x * b.y - a.y * b.x;
export const len = a => Math.hypot(a.x, a.y);
export const unit = a => { const l = len(a); return l < EPS ? { x: 1, y: 0 } : { x: a.x / l, y: a.y / l }; };
export const lerp = (a, b, t) => ({ x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t });
export const mid = (a, b) => ({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });
export const perp = a => ({ x: -a.y, y: a.x });
export const polar = (c, ang, d) => ({ x: c.x + d * Math.cos(ang), y: c.y + d * Math.sin(ang) });
export const angleOf = (from, to) => Math.atan2(to.y - from.y, to.x - from.x);
export const near = (a, b, tol = 1e-7) => Math.abs(a - b) <= tol * Math.max(1, Math.abs(a), Math.abs(b));
export const samePt = (a, b, tol = 1e-7) => near(a.x, b.x, tol) && near(a.y, b.y, tol);
export const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
export const deg = r => r * 180 / Math.PI;
export const rad = d => d * Math.PI / 180;

export const normAngle = a => { a %= TAU; return a < 0 ? a + TAU : a; };
export const normAngleSigned = a => { a = normAngle(a); return a > Math.PI ? a - TAU : a; };
/** CCW sweep from a0 to a1 in [0, 2π). */
export const sweepCCW = (a0, a1) => normAngle(a1 - a0);

// ---------------------------------------------------------------- arcs

/** Is angle `a` inside the directed arc starting at a0 with signed sweep sw? */
export function angleInArc(a, a0, sw, tol = 1e-9) {
  if (Math.abs(sw) >= TAU - tol) return true;
  const d = sw >= 0 ? normAngle(a - a0) : normAngle(a0 - a);
  const s = Math.abs(sw);
  return d <= s + tol || d >= TAU - tol;
}

/** Param t of angle `a` along the directed arc (0 at start, 1 at end). Outside the arc t<0 or t>1. */
export function arcParam(a, arc) {
  const s = Math.abs(arc.sw);
  if (s < EPS) return 0;
  const d = arc.sw >= 0 ? normAngle(a - arc.a0) : normAngle(arc.a0 - a);
  if (d <= s) return d / s;
  return (d - s < TAU - d) ? d / s : (d - TAU) / s;
}

export const arcStart = s => polar(s.c, s.a0, s.r);
export const arcEnd = s => polar(s.c, s.a0 + s.sw, s.r);

/** Arc through three points (start, any point on it, end), or null if collinear. */
export function arcThrough(p1, p2, p3) {
  const ax = p1.x, ay = p1.y, bx = p2.x, by = p2.y, cx = p3.x, cy = p3.y;
  const d = 2 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
  if (Math.abs(d) < 1e-12) return null;
  const a2 = ax * ax + ay * ay, b2 = bx * bx + by * by, c2 = cx * cx + cy * cy;
  const c = { x: (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d, y: (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d };
  const r = dist(c, p1);
  const a0 = angleOf(c, p1), am = angleOf(c, p2), a1 = angleOf(c, p3);
  const ccw = sweepCCW(a0, am) < sweepCCW(a0, a1);
  const sw = ccw ? sweepCCW(a0, a1) : -sweepCCW(a1, a0);
  return { t: 'a', c, r, a0, sw };
}

/** Circle through three points or null. */
export function circleThrough(p1, p2, p3) {
  const a = arcThrough(p1, p2, p3);
  return a ? { c: a.c, r: a.r } : null;
}

/** Arc segment of a polyline edge p1->p2 with the given bulge (tan(sweep/4)). */
export function bulgeSeg(p1, p2, bulge) {
  if (Math.abs(bulge) < 1e-12) return { t: 'l', a: { x: p1.x, y: p1.y }, b: { x: p2.x, y: p2.y } };
  const chord = dist(p1, p2);
  if (chord < EPS) return { t: 'l', a: { x: p1.x, y: p1.y }, b: { x: p2.x, y: p2.y } };
  const sw = 4 * Math.atan(bulge);
  const r = chord / (2 * Math.sin(Math.abs(sw) / 2));
  // centre lies on the perpendicular bisector, on the left of the chord for positive bulge
  const m = mid(p1, p2), u = unit(sub(p2, p1));
  const h = chord / 2 / Math.tan(sw / 2);
  const c = { x: m.x - u.y * h, y: m.y + u.x * h };
  return { t: 'a', c, r, a0: angleOf(c, p1), sw };
}

export const bulgeOf = seg => seg.t === 'a' ? Math.tan(seg.sw / 4) : 0;

// ---------------------------------------------------------------- generic segment ops

export const segStart = s => s.t === 'l' ? s.a : arcStart(s);
export const segEnd = s => s.t === 'l' ? s.b : arcEnd(s);
export const segLength = s => s.t === 'l' ? dist(s.a, s.b) : s.r * Math.abs(s.sw);

export function segAt(s, t) {
  if (s.t === 'l') return lerp(s.a, s.b, t);
  return polar(s.c, s.a0 + s.sw * t, s.r);
}

export function segReverse(s) {
  if (s.t === 'l') return { t: 'l', a: s.b, b: s.a };
  return { t: 'a', c: s.c, r: s.r, a0: s.a0 + s.sw, sw: -s.sw };
}

/** Portion of the segment between params t0 and t1 (t0<t1 not required). */
export function segSlice(s, t0, t1) {
  if (s.t === 'l') return { t: 'l', a: lerp(s.a, s.b, t0), b: lerp(s.a, s.b, t1) };
  return { t: 'a', c: s.c, r: s.r, a0: s.a0 + s.sw * t0, sw: s.sw * (t1 - t0) };
}

export function segBounds(s) {
  if (s.t === 'l') return { minX: Math.min(s.a.x, s.b.x), minY: Math.min(s.a.y, s.b.y), maxX: Math.max(s.a.x, s.b.x), maxY: Math.max(s.a.y, s.b.y) };
  const pts = [arcStart(s), arcEnd(s)];
  for (let k = 0; k < 4; k++) {
    const a = k * Math.PI / 2;
    if (angleInArc(a, s.a0, s.sw)) pts.push(polar(s.c, a, s.r));
  }
  return ptsBounds(pts);
}

export function ptsBounds(pts) {
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const p of pts) { if (p.x < minX) minX = p.x; if (p.y < minY) minY = p.y; if (p.x > maxX) maxX = p.x; if (p.y > maxY) maxY = p.y; }
  return { minX, minY, maxX, maxY };
}

export const unionBounds = (a, b) => !a ? b : !b ? a : { minX: Math.min(a.minX, b.minX), minY: Math.min(a.minY, b.minY), maxX: Math.max(a.maxX, b.maxX), maxY: Math.max(a.maxY, b.maxY) };
export const boundsOverlap = (a, b) => a.minX <= b.maxX && a.maxX >= b.minX && a.minY <= b.maxY && a.maxY >= b.minY;
export const boundsInside = (inner, outer) => inner.minX >= outer.minX && inner.maxX <= outer.maxX && inner.minY >= outer.minY && inner.maxY <= outer.maxY;
export const growBounds = (b, d) => ({ minX: b.minX - d, minY: b.minY - d, maxX: b.maxX + d, maxY: b.maxY + d });

/** Nearest point on a segment: {pt, t, d}. t is clamped to [0,1]. */
export function segNearest(s, p) {
  if (s.t === 'l') {
    const dx = s.b.x - s.a.x, dy = s.b.y - s.a.y, l2 = dx * dx + dy * dy;
    const t = l2 < 1e-24 ? 0 : clamp(((p.x - s.a.x) * dx + (p.y - s.a.y) * dy) / l2, 0, 1);
    const pt = { x: s.a.x + dx * t, y: s.a.y + dy * t };
    return { pt, t, d: dist(p, pt) };
  }
  const ang = angleOf(s.c, p);
  let t = arcParam(ang, s);
  if (t < 0 || t > 1) {
    const e0 = arcStart(s), e1 = arcEnd(s);
    return dist(p, e0) <= dist(p, e1) ? { pt: e0, t: 0, d: dist(p, e0) } : { pt: e1, t: 1, d: dist(p, e1) };
  }
  if (dist(p, s.c) < EPS) { const pt = arcStart(s); return { pt, t: 0, d: s.r }; }
  const pt = polar(s.c, ang, s.r);
  return { pt, t, d: dist(p, pt) };
}

/** Param of the point on the (infinite) carrier of the segment closest to p; not clamped. */
export function segProject(s, p) {
  if (s.t === 'l') {
    const dx = s.b.x - s.a.x, dy = s.b.y - s.a.y, l2 = dx * dx + dy * dy;
    return l2 < 1e-24 ? 0 : ((p.x - s.a.x) * dx + (p.y - s.a.y) * dy) / l2;
  }
  return arcParam(angleOf(s.c, p), s);
}

/** Tangent direction (unit) at param t. */
export function segTangent(s, t) {
  if (s.t === 'l') return unit(sub(s.b, s.a));
  const a = s.a0 + s.sw * t;
  const d = s.sw >= 0 ? 1 : -1;
  return { x: -Math.sin(a) * d, y: Math.cos(a) * d };
}

/** Offset a segment to its left by d (negative = right). Null if an arc collapses. */
export function segOffset(s, d) {
  if (s.t === 'l') {
    const u = unit(sub(s.b, s.a)), n = { x: -u.y * d, y: u.x * d };
    return { t: 'l', a: add(s.a, n), b: add(s.b, n) };
  }
  const r = s.sw >= 0 ? s.r - d : s.r + d;
  if (r < 1e-9) return null;
  return { t: 'a', c: s.c, r, a0: s.a0, sw: s.sw };
}

// ---------------------------------------------------------------- intersection

const TOL_T = 1e-9;

/**
 * Intersections of two segments. `ext1`/`ext2` let the corresponding segment be treated as its infinite
 * carrier (line extension / full circle). Returned t1,t2 are params along each segment (outside [0,1] only
 * for extended hits).
 */
export function segIntersect(s1, s2, ext1 = false, ext2 = false) {
  let hits;
  if (s1.t === 'l' && s2.t === 'l') hits = lineLine(s1, s2);
  else if (s1.t === 'l') hits = lineArc(s1, s2);
  else if (s2.t === 'l') hits = lineArc(s2, s1).map(h => ({ pt: h.pt, t1: h.t2, t2: h.t1 }));
  else hits = arcArc(s1, s2);
  return hits.filter(h => (ext1 || inRange(h.t1)) && (ext2 || inRange(h.t2)));
}

const inRange = t => t >= -TOL_T && t <= 1 + TOL_T;

function lineLine(s1, s2) {
  const r = sub(s1.b, s1.a), q = sub(s2.b, s2.a), den = cross(r, q);
  if (Math.abs(den) < 1e-14 * Math.max(1, len(r) * len(q))) return [];
  const w = sub(s2.a, s1.a);
  const t1 = cross(w, q) / den, t2 = cross(w, r) / den;
  return [{ pt: { x: s1.a.x + r.x * t1, y: s1.a.y + r.y * t1 }, t1, t2 }];
}

function lineArc(l, a) {
  // |a + t d - c|^2 = r^2
  const d = sub(l.b, l.a), f = sub(l.a, a.c);
  const A = dot(d, d);
  if (A < 1e-24) return [];
  const B = 2 * dot(f, d), C = dot(f, f) - a.r * a.r;
  let disc = B * B - 4 * A * C;
  const scale = Math.max(1, B * B);
  if (disc < -1e-12 * scale) return [];
  if (disc < 0) disc = 0;
  const sq = Math.sqrt(disc);
  const ts = sq < 1e-9 * Math.sqrt(scale) ? [-B / (2 * A)] : [(-B - sq) / (2 * A), (-B + sq) / (2 * A)];
  const out = [];
  for (const t1 of ts) {
    const pt = { x: l.a.x + d.x * t1, y: l.a.y + d.y * t1 };
    out.push({ pt, t1, t2: arcParam(angleOf(a.c, pt), a) });
  }
  return out;
}

function arcArc(a, b) {
  const d = dist(a.c, b.c);
  if (d < EPS) return [];
  if (d > a.r + b.r + 1e-9 || d < Math.abs(a.r - b.r) - 1e-9) return [];
  const x = (d * d + a.r * a.r - b.r * b.r) / (2 * d);
  let h2 = a.r * a.r - x * x;
  if (h2 < 0) h2 = 0;
  const h = Math.sqrt(h2), u = unit(sub(b.c, a.c));
  const base = add(a.c, mul(u, x));
  const pts = h < 1e-9 ? [base] : [add(base, mul(perp(u), h)), add(base, mul(perp(u), -h))];
  return pts.map(pt => ({ pt, t1: arcParam(angleOf(a.c, pt), a), t2: arcParam(angleOf(b.c, pt), b) }));
}

// ---------------------------------------------------------------- tangent / perpendicular helpers

/** Foot of the perpendicular from p onto the (infinite) carrier; candidate points for the PER snap. */
export function perpFeet(s, p) {
  if (s.t === 'l') {
    const t = segProject(s, p);
    return [{ pt: lerp(s.a, s.b, t), t }];
  }
  if (dist(p, s.c) < EPS) return [];
  const a = angleOf(s.c, p);
  return [polar(s.c, a, s.r), polar(s.c, a + Math.PI, s.r)].map(pt => ({ pt, t: arcParam(angleOf(s.c, pt), s) }));
}

/** Tangent points on a circle/arc carrier from an external point. */
export function tangentPoints(s, p) {
  if (s.t !== 'a') return [];
  const d = dist(p, s.c);
  if (d < s.r - 1e-9) return [];
  const base = angleOf(s.c, p), off = Math.acos(clamp(s.r / d, -1, 1));
  return [base + off, base - off].map(a => ({ pt: polar(s.c, a, s.r), t: arcParam(a, s) }));
}

/** Circle centres tangent to two segment carriers with radius r (for FILLET / CIRCLE TTR). */
export function tangentCentres(s1, s2, r) {
  const sides = [-1, 1];
  const carriers = s => sides.map(k => s.t === 'l'
    ? segOffset(s, k * r)
    : segOffset({ ...s, sw: Math.abs(s.sw) }, k * r));
  const out = [];
  for (const o1 of carriers(s1)) for (const o2 of carriers(s2)) {
    if (!o1 || !o2) continue;
    for (const h of segIntersect(o1, o2, true, true)) out.push(h.pt);
  }
  return out;
}

// ---------------------------------------------------------------- polylines

/** Segments of a bulge polyline; vertices are {x,y,b}. */
export function plineSegs(v, closed) {
  const n = v.length, segs = [];
  const last = closed ? n : n - 1;
  for (let i = 0; i < last; i++) segs.push(bulgeSeg(v[i], v[(i + 1) % n], v[i].b || 0));
  return segs;
}

/** Rebuild bulge-vertices from consecutive segments. Gaps are bridged by the segment starts. */
export function segsToVerts(segs, closed) {
  const v = segs.map(s => ({ x: segStart(s).x, y: segStart(s).y, b: bulgeOf(s) }));
  if (!closed && segs.length) { const e = segEnd(segs[segs.length - 1]); v.push({ x: e.x, y: e.y, b: 0 }); }
  return v;
}

export function polygonArea(pts) {
  let s = 0;
  for (let i = 0; i < pts.length; i++) { const a = pts[i], b = pts[(i + 1) % pts.length]; s += a.x * b.y - b.x * a.y; }
  return s / 2;
}

/** Signed area of a closed bulge polyline including arc segments (positive = CCW). */
export function plineArea(v) {
  const n = v.length;
  let area = polygonArea(v);
  for (let i = 0; i < n; i++) {
    const b = v[i].b || 0;
    if (!b) continue;
    const a = v[i], c = v[(i + 1) % n], ch = dist(a, c), sw = 4 * Math.atan(b);
    const r = ch / (2 * Math.sin(Math.abs(sw) / 2));
    const seg = r * r / 2 * (Math.abs(sw) - Math.sin(Math.abs(sw)));
    area += Math.sign(b) * seg;
  }
  return area;
}

export function plineLength(v, closed) {
  return plineSegs(v, closed).reduce((s, g) => s + segLength(g), 0);
}

export function pointInPolygon(p, pts) {
  let inside = false;
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const a = pts[i], b = pts[j];
    if ((a.y > p.y) !== (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x) inside = !inside;
  }
  return inside;
}

/** Tessellate a segment into points. `tol` is the max chord error in drawing units. */
export function tessSeg(s, tol = 0.01) {
  if (s.t === 'l') return [s.a, s.b];
  const n = arcSteps(s.r, Math.abs(s.sw), tol);
  const out = [];
  for (let i = 0; i <= n; i++) out.push(polar(s.c, s.a0 + s.sw * i / n, s.r));
  return out;
}

export function arcSteps(r, sweep, tol) {
  const maxStep = r > tol ? 2 * Math.acos(clamp(1 - tol / r, -1, 1)) : Math.PI / 2;
  return clamp(Math.ceil(sweep / Math.max(maxStep, 1e-3)), 2, 2048);
}

/** Concatenate tessellations of consecutive segments into one point list. */
export function tessSegs(segs, tol) {
  const out = [];
  for (const s of segs) {
    const pts = tessSeg(s, tol);
    for (let i = out.length ? 1 : 0; i < pts.length; i++) out.push(pts[i]);
  }
  return out;
}

// ---------------------------------------------------------------- affine matrices  [a b c d e f]  (canvas convention)

export const M = {
  identity: () => [1, 0, 0, 1, 0, 0],
  mul: (m, n) => [m[0] * n[0] + m[2] * n[1], m[1] * n[0] + m[3] * n[1], m[0] * n[2] + m[2] * n[3], m[1] * n[2] + m[3] * n[3], m[0] * n[4] + m[2] * n[5] + m[4], m[1] * n[4] + m[3] * n[5] + m[5]],
  translate: (dx, dy) => [1, 0, 0, 1, dx, dy],
  rotate: (ang, c = { x: 0, y: 0 }) => {
    const co = Math.cos(ang), si = Math.sin(ang);
    return [co, si, -si, co, c.x - co * c.x + si * c.y, c.y - si * c.x - co * c.y];
  },
  scale: (sx, sy = sx, c = { x: 0, y: 0 }) => [sx, 0, 0, sy, c.x - sx * c.x, c.y - sy * c.y],
  /** Reflection across the line through p1,p2. */
  mirror: (p1, p2) => {
    const u = unit(sub(p2, p1)), c2 = u.x * u.x - u.y * u.y, s2 = 2 * u.x * u.y;
    return [c2, s2, s2, -c2, p1.x - c2 * p1.x - s2 * p1.y, p1.y - s2 * p1.x + c2 * p1.y];
  },
  apply: (m, p) => ({ x: m[0] * p.x + m[2] * p.y + m[4], y: m[1] * p.x + m[3] * p.y + m[5] }),
  applyVec: (m, p) => ({ x: m[0] * p.x + m[2] * p.y, y: m[1] * p.x + m[3] * p.y }),
  det: m => m[0] * m[3] - m[1] * m[2],
  invert: m => {
    const d = m[0] * m[3] - m[1] * m[2];
    return [m[3] / d, -m[1] / d, -m[2] / d, m[0] / d, (m[2] * m[5] - m[3] * m[4]) / d, (m[1] * m[4] - m[0] * m[5]) / d];
  },
  /** Similarity = uniform scale + rotation (+ optional reflection). */
  isSimilarity: m => Math.abs(Math.hypot(m[0], m[1]) - Math.hypot(m[2], m[3])) < 1e-9 * Math.max(1, Math.hypot(m[0], m[1])) && Math.abs(m[0] * m[2] + m[1] * m[3]) < 1e-9 * Math.max(1, m[0] * m[0] + m[1] * m[1]),
  scaleFactor: m => Math.sqrt(Math.abs(m[0] * m[3] - m[1] * m[2])),
  angle: m => Math.atan2(m[1], m[0]),
};

/** Apply an affine matrix to a segment. Similarity transforms keep arcs; otherwise arcs are tessellated by the caller. */
export function transformSeg(s, m) {
  if (s.t === 'l') return { t: 'l', a: M.apply(m, s.a), b: M.apply(m, s.b) };
  const mirrored = M.det(m) < 0, k = M.scaleFactor(m);
  const c = M.apply(m, s.c);
  const a0v = M.applyVec(m, { x: Math.cos(s.a0), y: Math.sin(s.a0) });
  const a0 = Math.atan2(a0v.y, a0v.x);
  return { t: 'a', c, r: s.r * k, a0, sw: mirrored ? -s.sw : s.sw };
}

// ---------------------------------------------------------------- misc

/** Closest approach / bounds helpers for hit testing. */
export function distToSegs(segs, p) {
  let best = Infinity, bestPt = null;
  for (const s of segs) {
    const r = segNearest(s, p);
    if (r.d < best) { best = r.d; bestPt = r.pt; }
  }
  return { d: best, pt: bestPt };
}

/** Line through points a,b clipped to the rectangle; returns [p,q] or null. Used for XLINE / RAY drawing. */
export function clipLineToRect(a, d, r, bothWays = true) {
  let t0 = bothWays ? -Infinity : 0, t1 = Infinity;
  for (const [p, q, lo, hi] of [[a.x, d.x, r.minX, r.maxX], [a.y, d.y, r.minY, r.maxY]]) {
    if (Math.abs(q) < 1e-14) { if (p < lo || p > hi) return null; continue; }
    let u0 = (lo - p) / q, u1 = (hi - p) / q;
    if (u0 > u1) [u0, u1] = [u1, u0];
    t0 = Math.max(t0, u0); t1 = Math.min(t1, u1);
    if (t0 > t1) return null;
  }
  return [{ x: a.x + d.x * t0, y: a.y + d.y * t0 }, { x: a.x + d.x * t1, y: a.y + d.y * t1 }];
}

export function segInRect(s, r) {
  const b = segBounds(s);
  return b.minX >= r.minX && b.maxX <= r.maxX && b.minY >= r.minY && b.maxY <= r.maxY;
}

/** Does a segment touch (cross or lie inside) a rectangle? Used for crossing selection. */
export function segTouchesRect(s, r) {
  const b = segBounds(s);
  if (!boundsOverlap(b, r)) return false;
  const inside = p => p.x >= r.minX && p.x <= r.maxX && p.y >= r.minY && p.y <= r.maxY;
  if (inside(segStart(s)) || inside(segEnd(s))) return true;
  const edges = [
    { t: 'l', a: { x: r.minX, y: r.minY }, b: { x: r.maxX, y: r.minY } },
    { t: 'l', a: { x: r.maxX, y: r.minY }, b: { x: r.maxX, y: r.maxY } },
    { t: 'l', a: { x: r.maxX, y: r.maxY }, b: { x: r.minX, y: r.maxY } },
    { t: 'l', a: { x: r.minX, y: r.maxY }, b: { x: r.minX, y: r.minY } },
  ];
  return edges.some(e => segIntersect(s, e).length > 0);
}
