// Triangle-mesh solids: primitives, extrusion of planar profiles (with holes), measurements and export. DOM-free.
// A solid is { id, name, color?, layer, vertices:[{x,y,z}], faces:[[a,b,c]], program? } with outward (CCW) faces.
import * as g from '../core/geom.js';
import { segsOf, ellipsePoint, ellipseSweep, splinePoints } from '../core/entities.js';
import { earcut } from './earcut.js';

const TAU = Math.PI * 2;

// ---------------------------------------------------------------- profiles -> planar loops

/** Tessellated outline of one entity, or null when it is not a usable closed curve. */
function loopOfEntity(e, doc, tol) {
  switch (e.type) {
    case 'circle': { const n = Math.max(24, Math.min(256, Math.ceil(TAU / (2 * Math.acos(Math.max(0, 1 - tol / Math.max(e.r, tol * 2))))))); return Array.from({ length: n }, (_, i) => ({ x: e.c.x + e.r * Math.cos(TAU * i / n), y: e.c.y + e.r * Math.sin(TAU * i / n) })); }
    case 'ellipse': { if (ellipseSweep(e) < TAU - 1e-9) return null; return Array.from({ length: 96 }, (_, i) => ellipsePoint(e, TAU * i / 96)); }
    case 'pline': return e.closed ? dedupe(g.tessSegs(g.plineSegs(e.v, true), tol)) : null;
    case 'spline': return e.closed ? dedupe(splinePoints(e, 16).slice(0, -1)) : null;
    case 'hatch': return null;
    default: return null;
  }
}

function dedupe(pts) {
  const out = [];
  for (const p of pts) if (!out.length || g.dist(p, out[out.length - 1]) > 1e-9) out.push({ x: p.x, y: p.y });
  if (out.length > 1 && g.dist(out[0], out[out.length - 1]) < 1e-9) out.pop();
  return out;
}

/** Join open entities (lines, arcs, polylines) end to end into closed loops. */
function chainOpen(entities, doc, tol) {
  const paths = entities.map(e => segsOf(e, doc).map(s => ({ ...s }))).filter(p => p.length);
  const loops = [];
  const eps = 1e-6;
  const used = new Set();
  const start = s => g.segStart(s), end = s => g.segEnd(s);
  for (let i = 0; i < paths.length; i++) {
    if (used.has(i)) continue;
    used.add(i);
    let chain = paths[i].slice();
    let grew = true;
    while (grew && g.dist(start(chain[0]), end(chain[chain.length - 1])) > eps) {
      grew = false;
      for (let j = 0; j < paths.length; j++) {
        if (used.has(j)) continue;
        const p = paths[j], tail = end(chain[chain.length - 1]), head = start(chain[0]);
        if (g.dist(start(p[0]), tail) <= eps) { chain = [...chain, ...p]; }
        else if (g.dist(end(p[p.length - 1]), tail) <= eps) { chain = [...chain, ...p.slice().reverse().map(g.segReverse)]; }
        else if (g.dist(end(p[p.length - 1]), head) <= eps) { chain = [...p, ...chain]; }
        else if (g.dist(start(p[0]), head) <= eps) { chain = [...p.slice().reverse().map(g.segReverse), ...chain]; }
        else continue;
        used.add(j); grew = true; break;
      }
    }
    if (g.dist(start(chain[0]), end(chain[chain.length - 1])) <= eps) loops.push(dedupe(g.tessSegs(chain, tol)));
    else return { loops, open: true };
  }
  return { loops, open: false };
}

/**
 * Closed planar loops from selected entities. Closed polylines, circles, ellipses and closed splines are used
 * directly; lines, arcs and open polylines are joined into closed loops. Returns {loops, error?}.
 */
export function profileLoops(entities, doc, tol = 0.02) {
  const loops = [], open = [];
  for (const e of entities) {
    const l = loopOfEntity(e, doc, tol);
    if (l && l.length >= 3) loops.push(l);
    else if (['line', 'arc', 'pline'].includes(e.type)) open.push(e);
  }
  if (open.length) {
    const r = chainOpen(open, doc, tol);
    if (r.open) return { loops, error: 'The selected lines and arcs do not form a closed outline.' };
    loops.push(...r.loops.filter(l => l.length >= 3));
  }
  if (!loops.length) return { loops, error: 'Select closed polylines, circles, ellipses or a closed chain of lines and arcs.' };
  return { loops };
}

/** Orient loops by even-odd nesting: outer rings CCW, holes CW, each hole attached to its outer ring. */
export function groupLoops(loops) {
  const items = loops.map(l => ({ pts: l, area: g.polygonArea(l) })).map(it => ({ ...it, abs: Math.abs(it.area) })).sort((a, b) => b.abs - a.abs);
  for (const it of items) { it.parents = []; for (const o of items) if (o !== it && o.abs > it.abs && g.pointInPolygon(it.pts[0], o.pts)) it.parents.push(o); it.depth = it.parents.length; }
  const groups = [];
  for (const it of items) {
    if (it.depth % 2 === 0) { it.group = { outer: it.area >= 0 ? it.pts : it.pts.slice().reverse(), holes: [] }; groups.push(it.group); }
  }
  for (const it of items) {
    if (it.depth % 2 === 1) {
      const outer = it.parents.filter(p => p.depth === it.depth - 1).sort((a, b) => a.abs - b.abs)[0];
      outer?.group?.holes.push(it.area <= 0 ? it.pts : it.pts.slice().reverse());
    }
  }
  return groups;
}

// ---------------------------------------------------------------- extrusion

/** Extrude planar loops (even-odd holes) from z=startZ by `height` (negative extrudes downwards). */
export function extrudeLoops(loops, height, { startZ = 0, name = 'Extrusion', taper = 0 } = {}) {
  if (!Number.isFinite(height) || height === 0) throw Error('The extrusion height must not be zero.');
  const groups = groupLoops(loops);
  if (!groups.length) throw Error('There is no closed outline to extrude.');
  const z0 = height > 0 ? startZ : startZ + height, z1 = height > 0 ? startZ + height : startZ, h = Math.abs(height);
  const vertices = [], faces = [];
  const k = Math.tan(g.rad(taper));
  for (const grp of groups) {
    const rings = [grp.outer, ...grp.holes];
    const flat = [], holeStarts = [], map = [];
    const bottoms = [], tops = [];
    for (const [ri, ring] of rings.entries()) {
      if (ri > 0) holeStarts.push(flat.length / 2);
      const top = taper ? offsetRing(ring, (ri === 0 ? -1 : 1) * k * h) : ring;       // positive taper narrows the solid
      const b = [], t = [];
      for (let i = 0; i < ring.length; i++) { b.push(vertices.length); vertices.push({ x: ring[i].x, y: ring[i].y, z: z0 }); }
      for (let i = 0; i < ring.length; i++) { t.push(vertices.length); vertices.push({ x: top[i].x, y: top[i].y, z: z1 }); }
      bottoms.push(b); tops.push(t);
      for (let i = 0; i < ring.length; i++) { flat.push(ring[i].x, ring[i].y); map.push(i); }
    }
    // caps
    const tri = earcut(flat, holeStarts);
    const idxBottom = [], idxTop = [];
    rings.forEach((ring, ri) => { for (let i = 0; i < ring.length; i++) { idxBottom.push(bottoms[ri][i]); idxTop.push(tops[ri][i]); } });
    for (let i = 0; i < tri.length; i += 3) {
      let [a, b, c] = [tri[i], tri[i + 1], tri[i + 2]];
      const pa = { x: flat[a * 2], y: flat[a * 2 + 1] }, pb = { x: flat[b * 2], y: flat[b * 2 + 1] }, pc = { x: flat[c * 2], y: flat[c * 2 + 1] };
      if ((pb.x - pa.x) * (pc.y - pa.y) - (pb.y - pa.y) * (pc.x - pa.x) < 0) [b, c] = [c, b];       // make CCW
      faces.push([idxTop[a], idxTop[b], idxTop[c]]);
      faces.push([idxBottom[a], idxBottom[c], idxBottom[b]]);
    }
    // side walls: outer ring CCW and hole rings CW, so the right-hand side of each edge is outside the material
    rings.forEach((ring, ri) => {
      const n = ring.length;
      for (let i = 0; i < n; i++) {
        const j = (i + 1) % n, a0 = bottoms[ri][i], b0 = bottoms[ri][j], a1 = tops[ri][i], b1 = tops[ri][j];
        faces.push([a0, b0, b1], [a0, b1, a1]);
      }
    });
  }
  return { name, vertices, faces };
}

function offsetRing(ring, d) {
  // inward offset by moving each vertex along its bisector (for tapered extrusions of convex-ish outlines)
  const n = ring.length, out = [];
  for (let i = 0; i < n; i++) {
    const p = ring[(i + n - 1) % n], c = ring[i], q = ring[(i + 1) % n];
    const e1 = g.unit(g.sub(c, p)), e2 = g.unit(g.sub(q, c));
    const n1 = { x: e1.y, y: -e1.x }, n2 = { x: e2.y, y: -e2.x };
    const bis = g.unit(g.add(n1, n2)), cosHalf = Math.max(0.2, g.dot(bis, n1));
    out.push({ x: c.x + bis.x * d / cosHalf, y: c.y + bis.y * d / cosHalf });
  }
  return out;
}

// ---------------------------------------------------------------- primitives

export function boxMesh(x0, y0, z0, w, d, h, name = 'Box') {
  const x1 = x0 + w, y1 = y0 + d, z1 = z0 + h;
  const v = [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]].map(([x, y, z]) => ({ x, y, z }));
  const f = [[0, 3, 2], [0, 2, 1], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]];
  return orient({ name, vertices: v, faces: f });
}

export function wedgeMesh(x0, y0, z0, w, d, h, name = 'Wedge') {
  // sloped face rises from the front (y = y0) edge at z = z0 ... the high end is at x = x0 + w
  const x1 = x0 + w, y1 = y0 + d, z1 = z0 + h;
  const v = [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x1, y0, z1], [x1, y1, z1]].map(([x, y, z]) => ({ x, y, z }));
  const f = [[0, 3, 2], [0, 2, 1], [1, 2, 5], [1, 5, 4], [0, 1, 4], [3, 5, 2], [0, 4, 5], [0, 5, 3]];
  return orient({ name, vertices: v, faces: f });
}

export function cylinderMesh(cx, cy, z0, r, h, seg = 64, name = 'Cylinder') { return coneMesh(cx, cy, z0, r, r, h, seg, name); }

/** Cone or frustum: radius r1 at z0 and r2 at z0+h (r2 = 0 gives an apex). */
export function coneMesh(cx, cy, z0, r1, r2, h, seg = 64, name = 'Cone') {
  const vertices = [], faces = [], ring = (r, z) => { const s = vertices.length; if (r < 1e-12) { vertices.push({ x: cx, y: cy, z }); return [s]; } for (let i = 0; i < seg; i++) vertices.push({ x: cx + r * Math.cos(TAU * i / seg), y: cy + r * Math.sin(TAU * i / seg), z }); return Array.from({ length: seg }, (_, i) => s + i); };
  const bot = ring(r1, z0), top = ring(r2, z0 + h);
  const cap = (idx, up, centre) => {
    if (idx.length === 1) return;
    const c = vertices.length; vertices.push({ x: cx, y: cy, z: centre });
    for (let i = 0; i < seg; i++) { const a = idx[i], b = idx[(i + 1) % seg]; faces.push(up ? [c, a, b] : [c, b, a]); }
  };
  for (let i = 0; i < seg; i++) {
    const j = (i + 1) % seg;
    if (bot.length === 1) faces.push([bot[0], top[i], top[j]]);
    else if (top.length === 1) faces.push([bot[i], bot[j], top[0]]);
    else faces.push([bot[i], bot[j], top[j]], [bot[i], top[j], top[i]]);
  }
  cap(top, true, z0 + h); cap(bot, false, z0);
  return { name, vertices, faces };
}

export function sphereMesh(cx, cy, cz, r, seg = 48, name = 'Sphere') {
  const rings = Math.max(6, seg >> 1), vertices = [{ x: cx, y: cy, z: cz - r }], faces = [];
  for (let k = 1; k < rings; k++) { const phi = -Math.PI / 2 + Math.PI * k / rings; for (let i = 0; i < seg; i++) vertices.push({ x: cx + r * Math.cos(phi) * Math.cos(TAU * i / seg), y: cy + r * Math.cos(phi) * Math.sin(TAU * i / seg), z: cz + r * Math.sin(phi) }); }
  vertices.push({ x: cx, y: cy, z: cz + r });
  const at = (k, i) => 1 + (k - 1) * seg + (i % seg), south = 0, north = vertices.length - 1;
  for (let i = 0; i < seg; i++) faces.push([south, at(1, i + 1), at(1, i)]);
  for (let k = 1; k < rings - 1; k++) for (let i = 0; i < seg; i++) faces.push([at(k, i), at(k, i + 1), at(k + 1, i + 1)], [at(k, i), at(k + 1, i + 1), at(k + 1, i)]);
  for (let i = 0; i < seg; i++) faces.push([north, at(rings - 1, i), at(rings - 1, i + 1)]);
  return { name, vertices, faces };
}

/** Flip faces if the signed volume is negative (so primitives built by hand always face outward). */
function orient(m) { if (meshVolume(m) < 0) m.faces = m.faces.map(([a, b, c]) => [a, c, b]); return m; }

// ---------------------------------------------------------------- measurements

export function meshVolume(m) {
  let v = 0;
  for (const [i, j, k] of m.faces) { const a = m.vertices[i], b = m.vertices[j], c = m.vertices[k]; v += (a.x * (b.y * c.z - b.z * c.y) - a.y * (b.x * c.z - b.z * c.x) + a.z * (b.x * c.y - b.y * c.x)) / 6; }
  return v;
}

export function meshArea(m) {
  let s = 0;
  for (const [i, j, k] of m.faces) { const a = m.vertices[i], b = m.vertices[j], c = m.vertices[k], u = sub3(b, a), w = sub3(c, a); s += Math.hypot(u.y * w.z - u.z * w.y, u.z * w.x - u.x * w.z, u.x * w.y - u.y * w.x) / 2; }
  return s;
}

export function meshBounds(m) {
  const b = { min: { x: Infinity, y: Infinity, z: Infinity }, max: { x: -Infinity, y: -Infinity, z: -Infinity } };
  for (const p of m.vertices) for (const k of 'xyz') { if (p[k] < b.min[k]) b.min[k] = p[k]; if (p[k] > b.max[k]) b.max[k] = p[k]; }
  return b;
}

export function meshCentroid(m) {
  let V = 0, cx = 0, cy = 0, cz = 0;
  for (const [i, j, k] of m.faces) { const a = m.vertices[i], b = m.vertices[j], c = m.vertices[k], v = (a.x * (b.y * c.z - b.z * c.y) - a.y * (b.x * c.z - b.z * c.x) + a.z * (b.x * c.y - b.y * c.x)) / 6; V += v; cx += v * (a.x + b.x + c.x) / 4; cy += v * (a.y + b.y + c.y) / 4; cz += v * (a.z + b.z + c.z) / 4; }
  return Math.abs(V) < 1e-12 ? { x: 0, y: 0, z: 0 } : { x: cx / V, y: cy / V, z: cz / V };
}

/** Every undirected edge is used exactly twice, once in each direction (closed, consistently oriented surface). */
export function isWatertight(m) {
  const dir = new Map();
  const key = (a, b) => `${a},${b}`;
  for (const [i, j, k] of m.faces) for (const [a, b] of [[i, j], [j, k], [k, i]]) dir.set(key(a, b), (dir.get(key(a, b)) || 0) + 1);
  for (const [k, n] of dir) { const [a, b] = k.split(','); if (n !== 1 || dir.get(key(b, a)) !== 1) return false; }
  return true;
}

const sub3 = (a, b) => ({ x: a.x - b.x, y: a.y - b.y, z: a.z - b.z });

// ---------------------------------------------------------------- transforms

/** Affine 3D matrix as 12 numbers [m00 m01 m02 tx, m10 m11 m12 ty, m20 m21 m22 tz]. */
export const M3 = {
  identity: () => [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0],
  translate: (x, y, z) => [1, 0, 0, x, 0, 1, 0, y, 0, 0, 1, z],
  rotateZ: (a, c = { x: 0, y: 0 }) => { const co = Math.cos(a), si = Math.sin(a); return [co, -si, 0, c.x - co * c.x + si * c.y, si, co, 0, c.y - si * c.x - co * c.y, 0, 0, 1, 0]; },
  scale: (s, c = { x: 0, y: 0, z: 0 }) => [s, 0, 0, c.x * (1 - s), 0, s, 0, c.y * (1 - s), 0, 0, s, c.z * (1 - s)],
  mirrorXY: (a, b) => { const d = g.unit(g.sub(b, a)), c2 = d.x * d.x - d.y * d.y, s2 = 2 * d.x * d.y; return [c2, s2, 0, a.x - c2 * a.x - s2 * a.y, s2, -c2, 0, a.y - s2 * a.x + c2 * a.y, 0, 0, 1, 0]; },
  apply: (m, p) => ({ x: m[0] * p.x + m[1] * p.y + m[2] * p.z + m[3], y: m[4] * p.x + m[5] * p.y + m[6] * p.z + m[7], z: m[8] * p.x + m[9] * p.y + m[10] * p.z + m[11] }),
  det: m => m[0] * (m[5] * m[10] - m[6] * m[9]) - m[1] * (m[4] * m[10] - m[6] * m[8]) + m[2] * (m[4] * m[9] - m[5] * m[8]),
};

/** Transformed copy of a solid (faces are re-wound when the transform mirrors). */
export function transformMesh(s, m) {
  const out = { ...s, vertices: s.vertices.map(p => M3.apply(m, p)), faces: s.faces.map(f => f.slice()) };
  if (M3.det(m) < 0) out.faces = out.faces.map(([a, b, c]) => [a, c, b]);
  return out;
}

/** 3D matrix for a plan-view 2D affine map [a b c d e f]; z is scaled by the map's uniform scale factor. */
export function solidMatrix(m) { const [a, b, c, d, e, f] = m, k = Math.sqrt(Math.abs(a * d - b * c)); return [a, c, 0, e, b, d, 0, f, 0, 0, k, 0]; }
export const transformSolid = (s, m2d) => transformMesh(s, solidMatrix(m2d));

// ---------------------------------------------------------------- export

export function toOBJ(solids) {
  const out = ['# CadForge solids (drawing units)']; let off = 1;
  solids.forEach((s, n) => {
    out.push(`o ${String(s.name || `solid${n + 1}`).replace(/[^\w-]/g, '_')}`);
    for (const p of s.vertices) out.push(`v ${p.x} ${p.y} ${p.z}`);
    for (const f of s.faces) out.push(`f ${f.map(i => i + off).join(' ')}`);
    off += s.vertices.length;
  });
  return out.join('\n') + '\n';
}

export function toSTL(solids) {
  const out = ['solid CadForge'];
  for (const s of solids) for (const [i, j, k] of s.faces) {
    const a = s.vertices[i], b = s.vertices[j], c = s.vertices[k], u = sub3(b, a), w = sub3(c, a);
    let n = { x: u.y * w.z - u.z * w.y, y: u.z * w.x - u.x * w.z, z: u.x * w.y - u.y * w.x }; const l = Math.hypot(n.x, n.y, n.z) || 1; n = { x: n.x / l, y: n.y / l, z: n.z / l };
    out.push(` facet normal ${n.x} ${n.y} ${n.z}`, '  outer loop', ...[a, b, c].map(p => `   vertex ${p.x} ${p.y} ${p.z}`), '  endloop', ' endfacet');
  }
  out.push('endsolid CadForge');
  return out.join('\n') + '\n';
}
