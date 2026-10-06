// Orthographic camera shared by 2D drafting (plan view) and 3D inspection.
//
// World z=0 is the drawing plane. The view looks at `target` from direction (yaw, pitch):
//   pitch = 90deg  -> plan view (looking straight down), yaw rotates the plan
//   pitch = 0      -> looking horizontally (front/right/... views)
// Screen x grows right, screen y grows down.
import * as g from '../core/geom.js';

export const DEFAULT_SCALE = 4;

export class View {
  constructor() {
    this.target = { x: 0, y: 0, z: 0 };
    this.scale = DEFAULT_SCALE;          // pixels per drawing unit
    this.yaw = 0;
    this.pitch = Math.PI / 2;
    this.w = 800; this.h = 600;          // viewport size in CSS pixels
    this.prev = [];                      // ZOOM Previous stack
  }

  clone() { const v = new View(); Object.assign(v, JSON.parse(JSON.stringify({ target: this.target, scale: this.scale, yaw: this.yaw, pitch: this.pitch, w: this.w, h: this.h }))); return v; }
  state() { return { cx: this.target.x, cy: this.target.y, cz: this.target.z, scale: this.scale, yaw: this.yaw, pitch: this.pitch }; }
  setState(s) { this.target = { x: s.cx, y: s.cy, z: s.cz || 0 }; this.scale = s.scale; this.yaw = s.yaw ?? 0; this.pitch = s.pitch ?? Math.PI / 2; }

  get isPlan() { return Math.abs(this.pitch - Math.PI / 2) < 1e-6 && Math.abs(this.yaw) < 1e-6; }
  /** Can points be picked on the z=0 plane in this orientation? (not edge-on) */
  get canDraft() { return Math.sin(this.pitch) > 0.2; }

  push() { this.prev.push(this.state()); if (this.prev.length > 30) this.prev.shift(); }
  popPrevious() { const s = this.prev.pop(); if (s) this.setState(s); return !!s; }

  /** Affine matrix [a b c d e f] mapping world (x,y,0) to screen pixels. */
  matrix() {
    const cy = Math.cos(this.yaw), sy = Math.sin(this.yaw), sp = Math.sin(this.pitch), s = this.scale, t = this.target;
    return [s * cy, -s * sp * sy, -s * sy, -s * sp * cy, this.w / 2 - s * (cy * t.x - sy * t.y), this.h / 2 + s * sp * (sy * t.x + cy * t.y)];
  }

  /** Project a 3D world point: screen x,y plus depth (larger = farther from the viewer). */
  project(p) {
    const x = p.x - this.target.x, y = p.y - this.target.y, z = (p.z || 0) - this.target.z;
    const cy = Math.cos(this.yaw), sy = Math.sin(this.yaw), cp = Math.cos(this.pitch), sp = Math.sin(this.pitch);
    const x1 = x * cy - y * sy, y1 = x * sy + y * cy;
    const up = y1 * sp + z * cp, depth = -(y1 * cp) + z * sp;
    return { x: this.w / 2 + x1 * this.scale, y: this.h / 2 - up * this.scale, depth: -depth };
  }

  toScreen(p) { const m = this.matrix(); return { x: m[0] * p.x + m[2] * p.y + m[4], y: m[1] * p.x + m[3] * p.y + m[5] }; }

  /** Screen pixel -> world point on the plane z=0. */
  toWorld(sx, sy) {
    const m = this.matrix(), det = m[0] * m[3] - m[1] * m[2];
    if (Math.abs(det) < 1e-12) return { x: this.target.x, y: this.target.y };
    const dx = sx - m[4], dy = sy - m[5];
    return { x: (m[3] * dx - m[2] * dy) / det, y: (-m[1] * dx + m[0] * dy) / det };
  }

  /** Visible world rectangle (plan views only). */
  worldBounds() {
    const c = [[0, 0], [this.w, 0], [this.w, this.h], [0, this.h]].map(([x, y]) => this.toWorld(x, y));
    return g.ptsBounds(c);
  }

  pan(dxPx, dyPx) {
    const a = this.toWorld(0, 0), b = this.toWorld(dxPx, dyPx);
    this.target = { x: this.target.x - (b.x - a.x), y: this.target.y - (b.y - a.y), z: this.target.z };
  }

  /** Zoom keeping the world point under (sx,sy) fixed. */
  zoomAt(sx, sy, factor) {
    const before = this.toWorld(sx, sy);
    this.scale = g.clamp(this.scale * factor, 1e-5, 1e7);
    const after = this.toWorld(sx, sy);
    this.target = { x: this.target.x + before.x - after.x, y: this.target.y + before.y - after.y, z: this.target.z };
  }

  /** Fit a world rectangle with a margin (fraction of viewport). */
  fit(b, margin = .06) {
    if (!b) { this.target = { x: 0, y: 0, z: 0 }; this.scale = DEFAULT_SCALE; return; }
    const w = Math.max(b.maxX - b.minX, 1e-6), h = Math.max(b.maxY - b.minY, 1e-6);
    this.target = { x: (b.minX + b.maxX) / 2, y: (b.minY + b.maxY) / 2, z: this.target.z };
    // plan fit works in the rotated frame
    const c = Math.abs(Math.cos(this.yaw)), s = Math.abs(Math.sin(this.yaw));
    const rw = w * c + h * s, rh = w * s + h * c;
    this.scale = g.clamp(Math.min(this.w * (1 - 2 * margin) / rw, this.h * (1 - 2 * margin) / (rh * Math.max(Math.sin(this.pitch), .3))), 1e-5, 1e7);
  }

  fitPoints3D(pts, margin = .08) {
    if (!pts.length) return;
    const lo = { x: Infinity, y: Infinity, z: Infinity }, hi = { x: -Infinity, y: -Infinity, z: -Infinity };
    for (const p of pts) for (const k of ['x', 'y', 'z']) { lo[k] = Math.min(lo[k], p[k] || 0); hi[k] = Math.max(hi[k], p[k] || 0); }
    this.target = { x: (lo.x + hi.x) / 2, y: (lo.y + hi.y) / 2, z: (lo.z + hi.z) / 2 };
    this.scale = 1;
    let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
    for (const p of pts) { const q = this.project(p); minX = Math.min(minX, q.x); maxX = Math.max(maxX, q.x); minY = Math.min(minY, q.y); maxY = Math.max(maxY, q.y); }
    const w = Math.max(maxX - minX, 1e-6), h = Math.max(maxY - minY, 1e-6);
    this.scale = g.clamp(Math.min(this.w * (1 - 2 * margin) / w, this.h * (1 - 2 * margin) / h), 1e-5, 1e7);
  }

  orbit(dyaw, dpitch) {
    this.yaw = this.yaw + dyaw;
    this.pitch = g.clamp(this.pitch + dpitch, -Math.PI / 2 + .01, Math.PI / 2);
  }

  /** Standard view directions (names as in the ViewCube). */
  setPreset(name) {
    const d = Math.PI / 180;
    const P = {
      top: [0, 90], bottom: [0, -90], front: [0, 0], back: [180, 0], left: [90, 0], right: [-90, 0],
      swiso: [45, 35.264], seiso: [-45, 35.264], neiso: [-135, 35.264], nwiso: [135, 35.264],
    };
    const p = P[name];
    if (!p) return false;
    this.yaw = p[0] * d; this.pitch = p[1] * d;
    return true;
  }

  /** Closest standard view name for the current orientation. */
  presetName() {
    const d = 180 / Math.PI, yaw = ((this.yaw * d % 360) + 360) % 360, pitch = this.pitch * d;
    if (Math.abs(pitch - 90) < .5) return Math.abs(yaw) < .5 || Math.abs(yaw - 360) < .5 ? 'Top' : 'Custom View';
    if (Math.abs(pitch + 90) < .5) return 'Bottom';
    if (Math.abs(pitch) < .5) { for (const [n, a] of [['Front', 0], ['Left', 90], ['Back', 180], ['Right', 270]]) if (Math.abs(yaw - a) < .5) return n; }
    if (Math.abs(pitch - 35.264) < .5) { for (const [n, a] of [['SW Isometric', 45], ['NW Isometric', 135], ['NE Isometric', 225], ['SE Isometric', 315]]) if (Math.abs(yaw - a) < .5) return n; }
    return 'Custom View';
  }
}
