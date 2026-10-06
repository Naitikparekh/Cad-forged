// Picking solids through the camera: nearest triangle under a screen point, and projected-box window selection.
import * as g from '../core/geom.js';

function project(solid, view) { return solid.vertices.map(p => view.project(p)); }

function bary(p, a, b, c) {
  const d = (b.y - c.y) * (a.x - c.x) + (c.x - b.x) * (a.y - c.y);
  if (Math.abs(d) < 1e-12) return null;
  const u = ((b.y - c.y) * (p.x - c.x) + (c.x - b.x) * (p.y - c.y)) / d, v = ((c.y - a.y) * (p.x - c.x) + (a.x - c.x) * (p.y - c.y)) / d;
  return [u, v, 1 - u - v];
}

/** Distance from p to segment ab and the interpolated depth at the nearest point. */
function segHit(p, a, b) {
  const dx = b.x - a.x, dy = b.y - a.y, L = dx * dx + dy * dy, t = L < 1e-12 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / L));
  return { d: Math.hypot(p.x - a.x - dx * t, p.y - a.y - dy * t), depth: a.depth + (b.depth - a.depth) * t };
}

/**
 * Nearest solid under screen point `sp` ({x,y} in pixels). With `interior` false (wireframe) only edges within
 * `aperture` pixels count. Returns {solid, depth} or null.
 */
export function pickSolid(solids, view, sp, { aperture = 6, interior = true, isVisible = () => true } = {}) {
  let best = null;
  for (const s of solids) {
    if (!isVisible(s)) continue;
    const P = project(s, view);
    for (const f of s.faces) {
      const a = P[f[0]], b = P[f[1]], c = P[f[2]];
      let depth = null;
      const w = interior ? bary(sp, a, b, c) : null;
      if (w && w[0] >= -1e-9 && w[1] >= -1e-9 && w[2] >= -1e-9) depth = w[0] * a.depth + w[1] * b.depth + w[2] * c.depth;
      else {
        for (const [p, q] of [[a, b], [b, c], [c, a]]) { const h = segHit(sp, p, q); if (h.d <= aperture && (depth === null || h.depth < depth)) depth = h.depth - 1e-3; }
      }
      if (depth !== null && (!best || depth < best.depth)) best = { solid: s, depth };
    }
  }
  return best;
}

/** Screen-space bounding box of a solid. */
export function solidScreenBounds(solid, view) {
  const P = project(solid, view);
  return g.ptsBounds(P);
}

/** Solids inside (window) or touching (crossing) a screen rectangle. */
export function solidsInScreenRect(solids, view, rect, crossing, isVisible = () => true) {
  return solids.filter(s => {
    if (!isVisible(s)) return false;
    const b = solidScreenBounds(s, view);
    return crossing ? g.boundsOverlap(b, rect) : g.boundsInside(b, rect);
  });
}
