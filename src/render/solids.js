// Triangle-mesh solids: painter's-algorithm shading with smoothed normals, feature edges and the AutoCAD visual styles.
import * as g from '../core/geom.js';
import { colorToCss } from '../core/style.js';

const CREASE = Math.cos(g.rad(35));         // faces meeting at a sharper angle keep a hard edge
const EDGE_ANGLE = Math.cos(g.rad(25));
const cache = new WeakMap();

const faceNormal = (V, f) => {
  const a = V[f[0]], b = V[f[1]], c = V[f[2]], u = [b.x - a.x, b.y - a.y, b.z - a.z], v = [c.x - a.x, c.y - a.y, c.z - a.z];
  const n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
  const l = Math.hypot(...n);
  return { n: l < 1e-15 ? [0, 0, 1] : n.map(x => x / l), area: l / 2 };
};

/** Per-solid derived data: face normals, smoothed per-face shading normals and feature edges. */
function derived(s, rev) {
  const c = cache.get(s);
  if (c && c.rev === rev && c.nv === s.vertices.length && c.nf === s.faces.length) return c;
  const V = s.vertices, F = s.faces;
  const fn = F.map(f => faceNormal(V, f));
  const incident = Array.from({ length: V.length }, () => []);
  F.forEach((f, i) => { for (const v of f) incident[v].push(i); });
  const smooth = F.map((f, i) => {
    const acc = [0, 0, 0], n0 = fn[i].n;
    for (const v of f) for (const j of incident[v]) { const nj = fn[j]; if (n0[0] * nj.n[0] + n0[1] * nj.n[1] + n0[2] * nj.n[2] >= CREASE) { acc[0] += nj.n[0] * nj.area; acc[1] += nj.n[1] * nj.area; acc[2] += nj.n[2] * nj.area; } }
    const l = Math.hypot(...acc) || 1;
    return [acc[0] / l, acc[1] / l, acc[2] / l];
  });
  const map = new Map(), adj = F.map(() => [-1, -1, -1]), feat = F.map(() => [true, true, true]);
  F.forEach((f, fi) => {
    for (let k = 0; k < 3; k++) {
      const a = f[k], b = f[(k + 1) % 3], key = a < b ? a * 4294967296 + b : b * 4294967296 + a, e = map.get(key);
      if (e) { e.n2 = fn[fi].n; adj[fi][k] = e.fi; adj[e.fi][e.k] = fi; const sharp = e.n1[0] * e.n2[0] + e.n1[1] * e.n2[1] + e.n1[2] * e.n2[2] < EDGE_ANGLE; feat[fi][k] = feat[e.fi][e.k] = sharp; }
      else map.set(key, { a, b, n1: fn[fi].n, n2: null, fi, k });
    }
  });
  const edges = [];
  for (const e of map.values()) {
    if (!e.n2) { edges.push([e.a, e.b]); continue; }
    if (e.n1[0] * e.n2[0] + e.n1[1] * e.n2[1] + e.n1[2] * e.n2[2] < EDGE_ANGLE) edges.push([e.a, e.b]);
  }
  const d = { rev, nv: V.length, nf: F.length, fn, smooth, edges, adj, feat };
  cache.set(s, d);
  return d;
}

const LIGHT = (() => { const l = [-.35, .45, .82], n = Math.hypot(...l); return l.map(x => x / n); })();

function baseColor(rc, s) {
  const doc = rc.doc, layer = doc.layer(s.layer || '0') || doc.layers[0];
  let col = s.color !== undefined ? s.color : layer.color;
  if (typeof s.color === 'string' && /^#[0-9a-f]{6}$/i.test(s.color)) col = s.color;
  if (col === 7 || col === undefined || col === 'BYBLOCK') return rc.theme.dark ? '#c3ccd6' : '#8695a4';
  return colorToCss(col, { dark: rc.theme.dark, contrast: rc.theme.contrast });
}

export function drawSolids(rc, style = '2D Wireframe') {
  const solids = rc.doc.solids;
  if (!solids?.length) return;
  const { ctx, view, theme, doc } = rc;
  const wire = style === '2D Wireframe' || style === 'Wireframe', fill = !wire, edges = style !== 'Shaded';
  const cy = Math.cos(view.yaw), sy = Math.sin(view.yaw), cp = Math.cos(view.pitch), sp = Math.sin(view.pitch);
  const camN = n => { const x1 = n[0] * cy - n[1] * sy, y1 = n[0] * sy + n[1] * cy; return [x1, y1 * sp + n[2] * cp, -y1 * cp + n[2] * sp]; };
  ctx.setTransform(rc.dpr, 0, 0, rc.dpr, 0, 0);
  const polys = [], edgePaths = [];
  for (const s of solids) {
    if (!doc.solidVisible(s)) continue;
    const P = s.vertices.map(v => view.project(v)), D = derived(s, doc.rev);
    const sel = rc.sel.has(s.id) || rc.tempSel.has(s.id);
    const base = baseColor(rc, s), m = /^#([0-9a-f]{6})$/i.exec(base), rgb = m ? [0, 2, 4].map(i => parseInt(m[1].slice(i, i + 2), 16)) : [160, 170, 180];
    if (fill) {
      const front = P2 => (P2[1].x - P2[0].x) * (P2[2].y - P2[0].y) - (P2[1].y - P2[0].y) * (P2[2].x - P2[0].x) < 0;
      const faceFront = s.faces.map(f => front([P[f[0]], P[f[1]], P[f[2]]]));
      s.faces.forEach((f, i) => {
        if (!faceFront[i]) return;                                                                  // back face
        const a = P[f[0]], b = P[f[1]], c = P[f[2]], n = camN(D.smooth[i]);
        const lum = style === 'Hidden' ? 1 : .3 + .7 * Math.max(0, n[0] * LIGHT[0] + n[1] * LIGHT[1] + n[2] * LIGHT[2]);
        // edges are painted with their face so nearer faces cover them: feature edges and silhouettes only
        const pe = [];
        for (let k = 0; k < 3; k++) { const o = D.adj[i][k]; if ((edges || sel) && (D.feat[i][k] || o < 0 || !faceFront[o])) pe.push([P[f[k]], P[f[(k + 1) % 3]]]); }
        polys.push({ pts: [a, b, c], depth: (a.depth + b.depth + c.depth) / 3, col: style === 'Hidden' ? theme.canvas : `rgb(${rgb.map(x => Math.min(255, Math.round(x * lum * 1.08)))})`, pe, sel });
      });
    }
    edgePaths.push({ P, D, sel });
  }
  polys.sort((p, q) => q.depth - p.depth);
  const edgeCol = theme.dark ? 'rgba(8,14,20,.9)' : 'rgba(10,20,30,.85)';
  for (const p of polys) {
    ctx.beginPath(); ctx.moveTo(p.pts[0].x, p.pts[0].y); ctx.lineTo(p.pts[1].x, p.pts[1].y); ctx.lineTo(p.pts[2].x, p.pts[2].y); ctx.closePath();
    ctx.fillStyle = p.col; ctx.fill(); ctx.strokeStyle = p.col; ctx.lineWidth = .7; ctx.lineJoin = 'round'; ctx.stroke();
    if (p.pe.length) {
      ctx.beginPath(); for (const [u, v] of p.pe) { ctx.moveTo(u.x, u.y); ctx.lineTo(v.x, v.y); }
      ctx.strokeStyle = p.sel ? theme.select : edgeCol; ctx.lineWidth = p.sel ? 2 : 1; ctx.stroke();
    }
  }
  if (wire) {
    ctx.beginPath();
    for (const { P, D, sel } of edgePaths) if (!sel) for (const [a, b] of D.edges) { ctx.moveTo(P[a].x, P[a].y); ctx.lineTo(P[b].x, P[b].y); }
    ctx.strokeStyle = theme.dark ? '#7fd0ff' : '#0b5a96'; ctx.lineWidth = 1.15; ctx.lineJoin = 'round'; ctx.stroke();
  }
  for (const { P, D, sel } of edgePaths) {
    if (!sel || !wire) continue;
    ctx.beginPath();
    for (const [a, b] of D.edges) { ctx.moveTo(P[a].x, P[a].y); ctx.lineTo(P[b].x, P[b].y); }
    ctx.strokeStyle = theme.select; ctx.lineWidth = 2.2; ctx.stroke();
  }
}
