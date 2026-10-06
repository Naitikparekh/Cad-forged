// ViewCube: a clickable orientation cube with compass, home and rotate arrows.
import { h } from './dom.js';
import * as g from '../core/geom.js';

const FACES = [
  { name: 'TOP', n: [0, 0, 1], u: [1, 0, 0], v: [0, 1, 0] },
  { name: 'BOTTOM', n: [0, 0, -1], u: [1, 0, 0], v: [0, -1, 0] },
  { name: 'FRONT', n: [0, -1, 0], u: [1, 0, 0], v: [0, 0, 1] },
  { name: 'BACK', n: [0, 1, 0], u: [-1, 0, 0], v: [0, 0, 1] },
  { name: 'RIGHT', n: [1, 0, 0], u: [0, 1, 0], v: [0, 0, 1] },
  { name: 'LEFT', n: [-1, 0, 0], u: [0, -1, 0], v: [0, 0, 1] },
];
const SIZE = 118, C = SIZE / 2, R = 25;

export class ViewCube {
  constructor(ui, host) {
    this.ui = ui; this.host = host;
    this.canvas = h('canvas.vc-canvas', { width: SIZE * 2, height: SIZE * 2, style: { width: SIZE + 'px', height: SIZE + 'px' } });
    this.label = h('button.vc-wcs', { onclick: e => this.ui.showMenu([{ label: 'WCS', checked: true }, { label: 'New UCS...', disabled: true }], e.clientX, e.clientY) }, 'WCS ▾');
    host.append(this.canvas, this.label);
    this.hover = null; this.drag = null;
    this.canvas.addEventListener('pointermove', e => this.move(e));
    this.canvas.addEventListener('pointerleave', () => { if (!this.drag) { this.hover = null; this.draw(); } });
    this.canvas.addEventListener('pointerdown', e => this.down(e));
    this.canvas.addEventListener('pointerup', e => this.up(e));
    this.canvas.addEventListener('contextmenu', e => { e.preventDefault(); this.ui.showMenu([{ label: 'Home', action: () => this.preset('swiso') }, { label: 'Parallel', checked: true }, { label: 'Perspective', disabled: true }, { sep: true }, { label: 'ViewCube Settings...', disabled: true }], e.clientX, e.clientY); });
    this.canvas.addEventListener('wheel', e => e.stopPropagation());
  }

  get view() { return this.ui.editor.view; }

  /** Rotate a vector into camera space: [right, up, towardViewer]. */
  rot(v) {
    const { yaw, pitch } = this.view, cy = Math.cos(yaw), sy = Math.sin(yaw), cp = Math.cos(pitch), sp = Math.sin(pitch);
    const x1 = v[0] * cy - v[1] * sy, y1 = v[0] * sy + v[1] * cy;
    return [x1, y1 * sp + v[2] * cp, -y1 * cp + v[2] * sp];
  }

  faceGeom(f) {
    const c = this.rot(f.n), u = this.rot(f.u), v = this.rot(f.v);
    const P = (x, y) => ({ x: C + R * (c[0] + x * u[0] + y * v[0]), y: C - R * (c[1] + x * u[1] + y * v[1]) });
    return { c, u, v, depth: c[2], P, origin: P(0, 0), U: { x: R * u[0], y: -R * u[1] }, V: { x: R * v[0], y: -R * v[1] } };
  }

  visibleFaces() { return FACES.map(f => ({ f, geo: this.faceGeom(f) })).filter(x => x.geo.depth > 0.001).sort((a, b) => a.geo.depth - b.geo.depth); }

  cellAt(x, y) {
    for (const { f, geo } of [...this.visibleFaces()].reverse()) {
      const dx = x - geo.origin.x, dy = y - geo.origin.y, det = geo.U.x * geo.V.y - geo.U.y * geo.V.x;
      if (Math.abs(det) < 1e-6) continue;
      const s = (dx * geo.V.y - dy * geo.V.x) / det, t = (geo.U.x * dy - geo.U.y * dx) / det;
      if (Math.abs(s) <= 1 && Math.abs(t) <= 1) {
        const i = Math.abs(s) < .58 ? 0 : Math.sign(s), j = Math.abs(t) < .58 ? 0 : Math.sign(t);
        return { face: f, i, j, dir: [f.n[0] + i * f.u[0] + j * f.v[0], f.n[1] + i * f.u[1] + j * f.v[1], f.n[2] + i * f.u[2] + j * f.v[2]] };
      }
    }
    return null;
  }

  arrows() {
    return [
      { id: 'left', x: C - 50, y: C, dir: [-1, 0] }, { id: 'right', x: C + 50, y: C, dir: [1, 0] },
      { id: 'up', x: C, y: C - 52, dir: [0, -1] }, { id: 'down', x: C, y: C + 52, dir: [0, 1] },
    ];
  }

  hit(x, y) {
    if (Math.hypot(x - 14, y - 14) < 11) return { kind: 'home' };
    for (const a of this.arrows()) if (Math.hypot(x - a.x, y - a.y) < 9) return { kind: 'arrow', a };
    const cell = this.cellAt(x, y);
    return cell ? { kind: 'cell', cell } : null;
  }

  pos(e) { const r = this.canvas.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top }; }

  move(e) {
    const p = this.pos(e);
    if (this.drag) { this.dragMove(e); return; }
    const hit = this.hit(p.x, p.y);
    const key = hit ? hit.kind + (hit.cell ? hit.cell.face.name + hit.cell.i + hit.cell.j : hit.a?.id || '') : '';
    if (key !== this.hoverKey) { this.hoverKey = key; this.hover = hit; this.draw(); }
    this.canvas.style.cursor = hit ? 'pointer' : 'default';
  }

  down(e) {
    this.canvas.setPointerCapture(e.pointerId);
    this.drag = { x: e.clientX, y: e.clientY, yaw: this.view.yaw, pitch: this.view.pitch, moved: false, hit: this.hit(this.pos(e).x, this.pos(e).y) };
  }

  dragMove(e) {
    const d = this.drag, dx = e.clientX - d.x, dy = e.clientY - d.y;
    if (!d.moved && Math.hypot(dx, dy) < 4) return;
    d.moved = true;
    const v = this.view; v.yaw = d.yaw - dx * .012; v.pitch = g.clamp(d.pitch + dy * .01, -Math.PI / 2, Math.PI / 2);
    this.ui.viewChanged();
  }

  up(e) {
    const d = this.drag; this.drag = null;
    if (!d || d.moved) return;
    const hit = d.hit || this.hit(this.pos(e).x, this.pos(e).y);
    if (!hit) return;
    if (hit.kind === 'home') this.preset('swiso');
    else if (hit.kind === 'arrow') {
      const v = this.view, a = hit.a;
      if (a.dir[0]) this.animate(v.yaw + a.dir[0] * Math.PI / 2, v.pitch);
      else this.animate(v.yaw, g.clamp(v.pitch + a.dir[1] * -Math.PI / 4, -Math.PI / 2, Math.PI / 2));
    } else if (hit.kind === 'cell') {
      const [dx, dy, dz] = hit.cell.dir, len = Math.hypot(dx, dy, dz);
      const pitch = Math.asin(dz / len), yaw = Math.hypot(dx, dy) < 1e-9 ? this.view.yaw : -Math.atan2(dx, -dy);
      this.animate(yaw, pitch);
    }
  }

  preset(name) { const v = this.view.clone(); v.setPreset(name); this.animate(v.yaw, v.pitch); }

  animate(yaw, pitch) {
    const v = this.view, y0 = v.yaw, p0 = v.pitch;
    let dy = g.normAngleSigned(yaw - y0);
    const t0 = performance.now(), dur = 260;
    this.ui.viewPush();
    const step = now => {
      const t = Math.min(1, (now - t0) / dur), k = 1 - Math.pow(1 - t, 3);
      v.yaw = y0 + dy * k; v.pitch = p0 + (pitch - p0) * k;
      if (t >= 1) { v.yaw = g.normAngleSigned(y0 + dy); v.pitch = pitch; }
      this.ui.viewChanged();
      if (t < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }

  draw() {
    if (!this.ui.editor) return;
    const ctx = this.canvas.getContext('2d'), dark = this.ui.theme.dark;
    ctx.setTransform(2, 0, 0, 2, 0, 0); ctx.clearRect(0, 0, SIZE, SIZE);
    const v = this.view, plan = Math.abs(v.pitch) > 1.05;
    // compass ring
    if (plan) {
      ctx.strokeStyle = dark ? 'rgba(230,236,243,.55)' : 'rgba(40,50,60,.5)'; ctx.lineWidth = 1.2;
      ctx.beginPath(); ctx.arc(C, C, 47, 0, g.TAU); ctx.stroke();
      ctx.font = '600 10px "Segoe UI", sans-serif'; ctx.fillStyle = dark ? '#dfe5ec' : '#2a333c'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
      for (const [label, ang] of [['N', Math.PI / 2], ['E', 0], ['S', -Math.PI / 2], ['W', Math.PI]]) {
        const r = this.rot([Math.cos(ang), Math.sin(ang), 0]);
        const x = C + 40 * r[0] / Math.max(.4, Math.hypot(r[0], r[1])) * Math.hypot(r[0], r[1]), y = C - 40 * r[1] / Math.max(.4, Math.hypot(r[0], r[1])) * Math.hypot(r[0], r[1]);
        ctx.fillText(label, x * 1 + (x - C) * .22, y + (y - C) * .22);
      }
    }
    for (const { f, geo } of this.visibleFaces()) {
      const shade = .55 + .45 * geo.depth;
      const base = dark ? [70, 82, 96] : [214, 222, 232];
      const col = base.map(c => Math.round(c * (dark ? shade + .25 : .86 + .14 * shade)));
      ctx.fillStyle = `rgb(${col})`; ctx.strokeStyle = dark ? '#9fb0c2' : '#6c7b8b'; ctx.lineWidth = 1;
      const q = [[-1, -1], [1, -1], [1, 1], [-1, 1]].map(([s, t]) => geo.P(s, t));
      ctx.beginPath(); q.forEach((p, i) => i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y)); ctx.closePath(); ctx.fill(); ctx.stroke();
      // hovered cell highlight
      const hv = this.hover?.cell;
      if (hv && hv.face === f) {
        const lo = i => i === 0 ? -.58 : i > 0 ? .58 : -1, hi = i => i === 0 ? .58 : i > 0 ? 1 : -.58;
        const cq = [[lo(hv.i), lo(hv.j)], [hi(hv.i), lo(hv.j)], [hi(hv.i), hi(hv.j)], [lo(hv.i), hi(hv.j)]].map(([s, t]) => geo.P(s, t));
        ctx.fillStyle = 'rgba(80,170,255,.55)'; ctx.beginPath(); cq.forEach((p, i) => i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y)); ctx.closePath(); ctx.fill();
      }
      // label mapped onto the face
      ctx.save();
      ctx.transform(geo.U.x / 10, geo.U.y / 10, geo.V.x / 10, geo.V.y / 10, geo.origin.x, geo.origin.y);
      ctx.scale(1, -1);
      ctx.fillStyle = dark ? '#eef3f8' : '#1d2630'; ctx.font = '700 3.9px "Segoe UI", Arial, sans-serif'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
      ctx.scale(10, 10); ctx.font = '700 .39px "Segoe UI", Arial, sans-serif'; ctx.fillText(f.name, 0, 0);
      ctx.restore();
    }
    // home + arrows
    ctx.setTransform(2, 0, 0, 2, 0, 0);
    const hov = this.hover;
    ctx.fillStyle = hov?.kind === 'home' ? '#5bb8ff' : (dark ? '#aeb9c6' : '#55616e');
    ctx.beginPath(); ctx.moveTo(14, 7); ctx.lineTo(21, 14); ctx.lineTo(18.5, 14); ctx.lineTo(18.5, 20); ctx.lineTo(9.5, 20); ctx.lineTo(9.5, 14); ctx.lineTo(7, 14); ctx.closePath(); ctx.fill();
    for (const a of this.arrows()) {
      ctx.fillStyle = hov?.kind === 'arrow' && hov.a.id === a.id ? '#5bb8ff' : (dark ? '#aeb9c6' : '#55616e');
      ctx.beginPath(); const [dx, dy] = a.dir; ctx.moveTo(a.x + dx * 5, a.y + dy * 5); ctx.lineTo(a.x - dy * 5 - dx * 3, a.y + dx * 5 - dy * 3); ctx.lineTo(a.x + dy * 5 - dx * 3, a.y - dx * 5 - dy * 3); ctx.closePath(); ctx.fill();
    }
  }
}
