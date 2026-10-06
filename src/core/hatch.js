// Hatch pattern definitions (acad.pat-style line families) and line generation clipped to boundary loops.
import { rad } from './geom.js';

/**
 * Each family: a = angle (deg), ox/oy = origin, dx = shift along the line between successive lines,
 * dy = perpendicular spacing, dash = optional pattern (positive dash, negative gap, 0 dot).
 */
export const PATTERNS = {
  SOLID: { desc: 'Solid fill', solid: true },
  ANSI31: { desc: 'ANSI Iron, Brick, Stone masonry', families: [{ a: 45, ox: 0, oy: 0, dx: 0, dy: 3.175 }] },
  ANSI32: { desc: 'ANSI Steel', families: [{ a: 45, ox: 0, oy: 0, dx: 0, dy: 9.525 }, { a: 45, ox: 4.49, oy: 0, dx: 0, dy: 9.525 }] },
  ANSI33: { desc: 'ANSI Bronze, Brass, Copper', families: [{ a: 45, ox: 0, oy: 0, dx: 0, dy: 6.35 }, { a: 45, ox: 4.49, oy: 0, dx: 0, dy: 6.35, dash: [3.175, -1.5875] }] },
  ANSI37: { desc: 'ANSI Lead, Zinc, Magnesium', families: [{ a: 45, ox: 0, oy: 0, dx: 0, dy: 3.175 }, { a: 135, ox: 0, oy: 0, dx: 0, dy: 3.175 }] },
  ANSI38: { desc: 'ANSI Aluminum', families: [{ a: 45, ox: 0, oy: 0, dx: 0, dy: 3.175 }, { a: 135, ox: 0, oy: 0, dx: 4.49, dy: 3.175, dash: [7.9375, -4.7625] }] },
  ANGLE: { desc: 'Angle steel', families: [{ a: 0, ox: 0, oy: 0, dx: 0, dy: 7.9375, dash: [6.35, -1.5875] }, { a: 90, ox: 0, oy: 0, dx: 0, dy: 7.9375, dash: [6.35, -1.5875] }] },
  BRICK: { desc: 'Brick or masonry', families: [{ a: 0, ox: 0, oy: 0, dx: 0, dy: 6.35 }, { a: 90, ox: 0, oy: 0, dx: 6.35, dy: 12.7, dash: [6.35, -6.35] }, { a: 90, ox: 6.35, oy: 6.35, dx: 6.35, dy: 12.7, dash: [6.35, -6.35] }] },
  NET: { desc: 'Horizontal / vertical grid', families: [{ a: 0, ox: 0, oy: 0, dx: 0, dy: 3.175 }, { a: 90, ox: 0, oy: 0, dx: 0, dy: 3.175 }] },
  DASH: { desc: 'Dashed lines', families: [{ a: 0, ox: 0, oy: 0, dx: 3.175, dy: 6.35, dash: [3.175, -3.175] }] },
  LINE: { desc: 'Horizontal lines', families: [{ a: 0, ox: 0, oy: 0, dx: 0, dy: 3.175 }] },
  DOTS: { desc: 'Dotted grid', families: [{ a: 0, ox: 0, oy: 0, dx: 0, dy: 3.175, dash: [0, -3.175] }] },
  HONEY: { desc: 'Honeycomb', families: [{ a: 0, ox: 0, oy: 0, dx: 4.7625, dy: 8.2487, dash: [2.7491, -5.4983] }, { a: 120, ox: 0, oy: 0, dx: 4.7625, dy: 8.2487, dash: [2.7491, -5.4983] }, { a: 60, ox: 0, oy: 0, dx: -4.7625, dy: 8.2487, dash: [2.7491, -5.4983] }] },
};

export const MAX_HATCH_LINES = 60000;

/**
 * @param {Array<Array<{x,y}>>} loops tessellated boundary polygons (even-odd filled)
 * @returns {Array<[number,number,number,number]>|null} line segments [x0,y0,x1,y1], or null if too dense
 */
export function hatchLines(loops, patternName, scale = 1, angle = 0) {
  const pat = PATTERNS[patternName];
  if (!pat || pat.solid || !loops.length) return [];
  const out = [];
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const l of loops) for (const p of l) { minX = Math.min(minX, p.x); minY = Math.min(minY, p.y); maxX = Math.max(maxX, p.x); maxY = Math.max(maxY, p.y); }
  const ua = rad(angle);
  for (const f of pat.families) {
    const th = rad(f.a) + ua, ux = Math.cos(th), uy = Math.sin(th), vx = -uy, vy = ux;
    const step = f.dy * scale;
    if (Math.abs(step) < 1e-9) continue;
    // origin of the family (pattern coordinates rotate with the hatch angle)
    const ca = Math.cos(ua), sa = Math.sin(ua);
    const O = { x: (f.ox * ca - f.oy * sa) * scale, y: (f.ox * sa + f.oy * ca) * scale };
    const ou = O.x * ux + O.y * uy, os = O.x * vx + O.y * vy;
    // polygon edges in the (U,S) frame
    const edges = [];
    let smin = Infinity, smax = -Infinity;
    for (const l of loops) {
      for (let i = 0; i < l.length; i++) {
        const a = l[i], b = l[(i + 1) % l.length];
        const au = a.x * ux + a.y * uy, as = a.x * vx + a.y * vy, bu = b.x * ux + b.y * uy, bs = b.x * vx + b.y * vy;
        edges.push([au, as, bu, bs]);
        smin = Math.min(smin, as, bs); smax = Math.max(smax, as, bs);
      }
    }
    const k0 = Math.ceil((Math.min((smin - os) / step, (smax - os) / step)) - 1e-9), k1 = Math.floor(Math.max((smin - os) / step, (smax - os) / step) + 1e-9);
    if (k1 - k0 > MAX_HATCH_LINES) return null;
    const dash = f.dash ? f.dash.map(d => d * scale) : null;
    const period = dash ? dash.reduce((s, d) => s + Math.abs(d), 0) : 0;
    for (let k = k0; k <= k1; k++) {
      const s0 = os + k * step, us = [];
      for (const [au, as, bu, bs] of edges) {
        if ((as < s0) !== (bs < s0)) us.push(au + (s0 - as) / (bs - as) * (bu - au));
      }
      us.sort((p, q) => p - q);
      for (let i = 0; i + 1 < us.length; i += 2) {
        const u0 = us[i], u1 = us[i + 1];
        if (u1 - u0 < 1e-9) continue;
        if (!dash || period < 1e-9) { out.push([u0 * ux + s0 * vx, u0 * uy + s0 * vy, u1 * ux + s0 * vx, u1 * uy + s0 * vy]); }
        else {
          const base = ou + k * f.dx * scale;
          let pos = base + Math.floor((u0 - base) / period) * period;
          while (pos < u1) {
            for (const d of dash) {
              const len = Math.abs(d);
              if (d >= 0) {
                const a = Math.max(pos, u0), b = Math.min(pos + len, u1);
                if (b >= a - 1e-12 && pos + len >= u0 && pos <= u1) out.push([a * ux + s0 * vx, a * uy + s0 * vy, b * ux + s0 * vx, b * uy + s0 * vy]);
              }
              pos += len;
              if (pos >= u1) break;
            }
            if (out.length > MAX_HATCH_LINES * 4) return null;
          }
        }
      }
      if (out.length > MAX_HATCH_LINES * 4) return null;
    }
  }
  return out;
}
