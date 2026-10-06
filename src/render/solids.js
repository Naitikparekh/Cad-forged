// Triangle-mesh solids: painter's-algorithm shading, feature edges and the AutoCAD visual styles.
import * as g from '../core/geom.js';

const featureCache = new WeakMap();

/** Edges shared by faces whose normals differ by more than ~25 degrees (plus boundary edges). */
function featureEdges(s) {
  let r = featureCache.get(s);
  if (r && r.nv === s.vertices.length && r.nf === s.faces.length) return r.edges;
  const V = s.vertices, norm = f => { const [a, b, c] = f.map(i => V[i]); const u = [b.x - a.x, b.y - a.y, b.z - a.z], v = [c.x - a.x, c.y - a.y, c.z - a.z]; const n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]; const l = Math.hypot(...n) || 1; return n.map(x => x / l); };
  const map = new Map();
  s.faces.forEach((f, fi) => {
    const n = norm(f);
    for (let k = 0; k < 3; k++) {
      const a = f[k], b = f[(k + 1) % 3], key = a < b ? a + ',' + b : b + ',' + a;
      const entry = map.get(key); if (entry) entry.n2 = n; else map.set(key, { a, b, n1: n, n2: null });
    }
    void fi;
  });
  const edges = [];
  for (const e of map.values()) {
    if (!e.n2) { edges.push([e.a, e.b]); continue; }
    const dot = e.n1[0] * e.n2[0] + e.n1[1] * e.n2[1] + e.n1[2] * e.n2[2];
    if (dot < Math.cos(g.rad(25))) edges.push([e.a, e.b]);
  }
  featureCache.set(s, { nv: V.length, nf: s.faces.length, edges });
  return edges;
}

const LIGHT = [-.35, .45, .82];

export function drawSolids(rc, style = '2D Wireframe') {
  const solids = rc.doc.solids;
  if (!solids?.length) return;
  const { ctx, view, theme } = rc;
  const wire = style === '2D Wireframe' || style === 'Wireframe', edges = style !== 'Shaded', fill = !wire;
  const cy = Math.cos(view.yaw), sy = Math.sin(view.yaw), cp = Math.cos(view.pitch), sp = Math.sin(view.pitch);
  const camN = n => { const x1 = n[0] * cy - n[1] * sy, y1 = n[0] * sy + n[1] * cy; return [x1, y1 * sp + n[2] * cp, -y1 * cp + n[2] * sp]; };
  const L = (() => { const l = Math.hypot(...LIGHT); return LIGHT.map(x => x / l); })();
  ctx.setTransform(rc.dpr, 0, 0, rc.dpr, 0, 0);
  const polys = [];
  for (const s of solids) {
    const P = s.vertices.map(v => view.project(v));
    const base = s.color && /^#[0-9a-f]{6}$/i.test(s.color) ? s.color : '#5aa0d0', rgb = [1, 3, 5].map(i => parseInt(base.slice(i, i + 2), 16));
    if (fill) {
      for (const f of s.faces) {
        const a = P[f[0]], b = P[f[1]], c = P[f[2]];
        const area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        if (area >= 0) continue;                                            // back face (screen y points down: front faces are clockwise)
        const A = s.vertices[f[0]], B = s.vertices[f[1]], C = s.vertices[f[2]];
        const u = [B.x - A.x, B.y - A.y, B.z - A.z], v = [C.x - A.x, C.y - A.y, C.z - A.z];
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]]; const l = Math.hypot(...n) || 1; n = camN(n.map(x => x / l));
        const lum = style === 'Hidden' ? 1 : .32 + .68 * Math.max(0, n[0] * L[0] + n[1] * L[1] + n[2] * L[2]);
        polys.push({ pts: [a, b, c], depth: (a.depth + b.depth + c.depth) / 3, col: style === 'Hidden' ? theme.canvas : `rgb(${rgb.map(x => Math.round(x * lum))})` });
      }
    }
  }
  polys.sort((p, q) => q.depth - p.depth);
  for (const p of polys) {
    ctx.beginPath(); ctx.moveTo(p.pts[0].x, p.pts[0].y); ctx.lineTo(p.pts[1].x, p.pts[1].y); ctx.lineTo(p.pts[2].x, p.pts[2].y); ctx.closePath();
    ctx.fillStyle = p.col; ctx.fill(); ctx.strokeStyle = p.col; ctx.lineWidth = .6; ctx.stroke();
  }
  if (edges || wire) {
    ctx.beginPath();
    for (const s of solids) {
      const P = s.vertices.map(v => view.project(v));
      for (const [a, b] of featureEdges(s)) { ctx.moveTo(P[a].x, P[a].y); ctx.lineTo(P[b].x, P[b].y); }
    }
    ctx.strokeStyle = wire ? (theme.dark ? '#7fd0ff' : '#0b5a96') : (theme.dark ? 'rgba(10,18,26,.85)' : 'rgba(10,20,30,.8)'); ctx.lineWidth = wire ? 1.1 : 1; ctx.lineJoin = 'round'; ctx.stroke();
  }
}
