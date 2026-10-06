// A recording stand-in for CanvasRenderingContext2D: the scene renderer draws into it and the resulting
// vector operations are written out as PDF or SVG. Only the canvas subset the renderer uses is implemented.
const TAU = Math.PI * 2;

export class RecordingContext {
  constructor(tol = 0.08) {
    this.ops = []; this.tol = tol;                         // tol: flatness of curves in device units
    this.m = [1, 0, 0, 1, 0, 0];
    this.strokeStyle = '#000000'; this.fillStyle = '#000000'; this.lineWidth = 1; this.globalAlpha = 1;
    this.lineCap = 'butt'; this.lineJoin = 'miter'; this.font = '10px sans-serif'; this.textBaseline = 'alphabetic';
    this.dash = []; this.path = []; this.cur = null;
  }

  // transforms
  setTransform(a, b, c, d, e, f) { this.m = [a, b, c, d, e, f]; }
  scale(sx, sy) { const m = this.m; this.m = [m[0] * sx, m[1] * sx, m[2] * sy, m[3] * sy, m[4], m[5]]; }
  setLineDash(a) { this.dash = a.slice(); }
  tp(x, y) { const m = this.m; return [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]; }

  // path construction (points are stored in device space, i.e. after the transform at call time)
  beginPath() { this.path = []; this.cur = null; }
  moveTo(x, y) { this.cur = { pts: [this.tp(x, y)], closed: false }; this.path.push(this.cur); }
  lineTo(x, y) { if (!this.cur) return this.moveTo(x, y); this.cur.pts.push(this.tp(x, y)); }
  closePath() { if (this.cur) { this.cur.closed = true; const p = this.cur.pts[0]; this.cur = { pts: [p], closed: false }; this.path.push(this.cur); } }
  rect(x, y, w, h) { this.moveTo(x, y); this.lineTo(x + w, y); this.lineTo(x + w, y + h); this.lineTo(x, y + h); this.closePath(); }
  arc(cx, cy, r, a0, a1, ccw = false) { this.ellipse(cx, cy, r, r, 0, a0, a1, ccw); }
  ellipse(cx, cy, rx, ry, rot, a0, a1, ccw = false) {
    let sw = a1 - a0;
    if (!ccw) { if (sw >= TAU) sw = TAU; else while (sw < 0) sw += TAU; } else { if (sw <= -TAU) sw = -TAU; else while (sw > 0) sw -= TAU; }
    const k = Math.sqrt(Math.abs(this.m[0] * this.m[3] - this.m[1] * this.m[2])), rDev = Math.max(rx, ry) * k;
    const stepMax = rDev > this.tol ? 2 * Math.acos(Math.max(0, 1 - this.tol / rDev)) : Math.PI / 2;
    const n = Math.max(4, Math.min(4000, Math.ceil(Math.abs(sw) / Math.max(stepMax, 1e-3)))), cr = Math.cos(rot), sr = Math.sin(rot);
    for (let i = 0; i <= n; i++) {
      const t = a0 + sw * i / n, x = rx * Math.cos(t), y = ry * Math.sin(t);
      const px = cx + x * cr - y * sr, py = cy + x * sr + y * cr;
      if (i === 0 && !this.cur) this.moveTo(px, py); else this.lineTo(px, py);
    }
  }

  // painting
  stroke() { const paths = this.paths(); if (paths.length) this.ops.push({ t: 'stroke', paths, color: this.strokeStyle, alpha: this.globalAlpha, w: this.lineWidth, dash: this.dash.slice(), cap: this.lineCap, join: this.lineJoin }); }
  fill(rule = 'nonzero') { const paths = this.paths(true); if (paths.length) this.ops.push({ t: 'fill', paths, color: this.fillStyle, alpha: this.globalAlpha, rule }); }
  fillRect(x, y, w, h) { const keep = [this.path, this.cur]; this.beginPath(); this.rect(x, y, w, h); this.fill(); [this.path, this.cur] = keep; }
  strokeRect(x, y, w, h) { const keep = [this.path, this.cur]; this.beginPath(); this.rect(x, y, w, h); this.stroke(); [this.path, this.cur] = keep; }
  fillText(str, x, y) {
    const em = parseFloat(this.font) || 10;
    this.ops.push({ t: 'text', str: String(str), x, y, m: this.m.slice(), em, color: this.fillStyle, alpha: this.globalAlpha });
  }
  paths(forFill = false) { return this.path.filter(p => p.pts.length > (forFill ? 2 : 1) || (p.pts.length === 2 && !forFill)).map(p => ({ pts: p.pts, closed: p.closed })); }
}
