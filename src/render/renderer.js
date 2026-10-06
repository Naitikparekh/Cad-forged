// Scene renderer: draws the drawing database to a 2D canvas using the camera's affine map.
// Geometry is traced in world coordinates under the world->screen transform and stroked after the
// transform is reset, so line widths and dashes are in screen pixels while arcs stay exact.
import * as g from '../core/geom.js';
import { M, TAU } from '../core/geom.js';
import { colorToCss, LINETYPES, lwPixels, LW_DEFAULT } from '../core/style.js';
import { segsOf, ellipseSweep, splinePoints, textsOf, hatchSegments, insertMatrix, leaderText, boundsOf } from '../core/entities.js';
import { dimGeometry, arrowPolygon, arrowLines } from '../core/dim.js';
import { textLayout, emSize, platform, FONT } from '../core/text.js';

const MAX_DEPTH = 8;

export function makeContext(ctx, canvasW, canvasH, dpr, editor, prefs, theme) {
  const { doc, view } = editor;
  const layers = new Map(doc.layers.map(l => [l.name.toLowerCase(), l]));
  const worldM = view.matrix();
  return {
    ctx, W: canvasW, H: canvasH, dpr, editor, doc, view, prefs, theme, layers, worldM, stack: [],
    lwOn: !!prefs.LWDISPLAY, ltscale: doc.settings.ltscale, fade: prefs.LAYLOCKFADECTL > 0 ? prefs.LAYLOCKFADECTL / 100 : 0,
    sel: editor.sel, tempSel: editor.tempSel, hoverId: editor.hoverId, transpOn: !!prefs.TRANSPARENCYDISPLAY, fill: !!prefs.FILLMODE,
    get k() { return Math.sqrt(Math.abs(M.det(this.worldM))); },
  };
}

const setWorld = rc => { const m = rc.worldM, d = rc.dpr; rc.ctx.setTransform(d * m[0], d * m[1], d * m[2], d * m[3], d * m[4], d * m[5]); };
const setScreen = rc => rc.ctx.setTransform(rc.dpr, 0, 0, rc.dpr, 0, 0);

export function layerOf(rc, e) { return rc.layers.get((e.layer || '0').toLowerCase()) || rc.layers.get('0'); }

/** Resolve color/linetype/lineweight/transparency with BYLAYER/BYBLOCK inheritance. */
export function styleOf(rc, e, parent) {
  const layer = layerOf(rc, e);
  let color = e.color;
  if (color === undefined || color === 'BYLAYER') color = layer.color;
  else if (color === 'BYBLOCK') color = parent ? parent.rawColor : layer.color;
  let ltype = e.ltype;
  if (ltype === undefined || ltype === 'BYLAYER') ltype = layer.ltype; else if (ltype === 'BYBLOCK') ltype = parent ? parent.ltype : layer.ltype;
  let lw = e.lw;
  if (lw === undefined || lw === -1) lw = layer.lw; else if (lw === -2) lw = parent ? parent.lw : layer.lw;
  if (lw === -3 || lw === undefined) lw = LW_DEFAULT;
  const transp = e.transp ?? layer.transp ?? 0;
  const fade = layer.locked && rc.fade ? Math.max(.12, 1 - rc.fade) : 1;
  return {
    rawColor: color, color: rc.mono ? '#000000' : colorToCss(color, { dark: rc.theme.dark }), ltype, lw, lts: (e.lts ?? 1) * (parent?.lts ?? 1),
    alpha: (rc.transpOn ? 1 - Math.min(90, transp) / 100 : 1) * fade,
  };
}

// ---------------------------------------------------------------- tracing (world coordinates)

function traceSegs(ctx, segs, closed) {
  let first = true;
  for (const s of segs) {
    if (s.t === 'l') { if (first) ctx.moveTo(s.a.x, s.a.y); ctx.lineTo(s.b.x, s.b.y); }
    else {
      if (first) ctx.moveTo(s.c.x + s.r * Math.cos(s.a0), s.c.y + s.r * Math.sin(s.a0));
      ctx.arc(s.c.x, s.c.y, s.r, s.a0, s.a0 + s.sw, s.sw < 0);
    }
    first = false;
  }
  if (closed) ctx.closePath();
}

function traceEntity(rc, e) {
  const ctx = rc.ctx;
  switch (e.type) {
    case 'line': ctx.moveTo(e.a.x, e.a.y); ctx.lineTo(e.b.x, e.b.y); return true;
    case 'circle': ctx.moveTo(e.c.x + e.r, e.c.y); ctx.arc(e.c.x, e.c.y, e.r, 0, TAU); return true;
    case 'arc': { const sw = g.sweepCCW(e.a0, e.a1) || TAU; ctx.moveTo(e.c.x + e.r * Math.cos(e.a0), e.c.y + e.r * Math.sin(e.a0)); ctx.arc(e.c.x, e.c.y, e.r, e.a0, e.a0 + sw, false); return true; }
    case 'pline': traceSegs(ctx, segsOf(e, rc.doc), e.closed); return true;
    case 'ellipse': {
      const rx = g.len(e.m), rot = Math.atan2(e.m.y, e.m.x), sw = ellipseSweep(e);
      ctx.moveTo(e.c.x + e.m.x * Math.cos(e.t0) - e.m.y * e.ratio * Math.sin(e.t0), e.c.y + e.m.y * Math.cos(e.t0) + e.m.x * e.ratio * Math.sin(e.t0));
      ctx.ellipse(e.c.x, e.c.y, rx, rx * e.ratio, rot, e.t0, e.t0 + sw, false);
      return true;
    }
    case 'spline': { const pts = splinePoints(e); pts.forEach((p, i) => i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y)); if (e.closed) ctx.closePath(); return true; }
    case 'xline': case 'ray': {
      const b = visibleWorld(rc), r = g.clipLineToRect(e.p, e.d, b, e.type === 'xline');
      if (!r) return false;
      ctx.moveTo(r[0].x, r[0].y); ctx.lineTo(r[1].x, r[1].y); return true;
    }
    case 'leader': e.pts.forEach((p, i) => i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y)); return true;
    default: return false;
  }
}

function visibleWorld(rc) {
  if (!rc._vis) {
    const inv = M.invert(rc.worldM), c = [[0, 0], [rc.W, 0], [rc.W, rc.H], [0, rc.H]].map(([x, y]) => M.apply(inv, { x, y }));
    rc._vis = g.growBounds(g.ptsBounds(c), 1);
  }
  return rc._vis;
}

// ---------------------------------------------------------------- stroking

function dashFor(rc, style) {
  const lt = LINETYPES[style.ltype];
  if (!lt || !lt.pattern.length) return null;
  const unit = rc.ltscale * style.lts * rc.k;
  const arr = lt.pattern.map(v => Math.max(Math.abs(v) < 1e-9 ? .01 : Math.abs(v) * unit, Math.abs(v) < 1e-9 ? .01 : .6)).map((v, i) => v);
  // pattern alternates dash,gap; zero-length dashes (dots) remain at ~0
  const out = []; lt.pattern.forEach((v, i) => out.push(v === 0 ? .01 : Math.abs(v) * unit));
  const total = out.reduce((a, b) => a + b, 0);
  if (total < 3.5 || total > 20000) return null;      // too dense to be useful: draw solid
  if (out.length % 2) out.push(...out);
  void arr;
  return out;
}

function strokeCurrent(rc, style, { selected = false, hover = false, forceWidth = 0 } = {}) {
  const ctx = rc.ctx;
  setScreen(rc);
  ctx.lineCap = 'round'; ctx.lineJoin = 'round';
  const w = forceWidth || (rc.lwPx ? rc.lwPx(style.lw) : lwPixels(style.lw, rc.lwOn));
  if (selected) {
    ctx.globalAlpha = .38 * style.alpha; ctx.strokeStyle = rc.theme.select; ctx.lineWidth = w + 5; ctx.setLineDash([]); ctx.stroke();
  }
  ctx.globalAlpha = style.alpha;
  ctx.strokeStyle = hover ? rc.theme.hover : style.color;
  ctx.lineWidth = hover ? w + 1.2 : w;
  const dash = selected ? [7, 5] : dashFor(rc, style);
  ctx.setLineDash(dash || []);
  ctx.stroke();
  ctx.setLineDash([]);
  ctx.globalAlpha = 1;
}

// ---------------------------------------------------------------- entities

export function drawEntity(rc, e, opts = {}) {
  const { parent = null, selected = false, hover = false, depth = 0 } = opts;
  const ctx = rc.ctx;
  const style = opts.style || styleOf(rc, e, parent);
  switch (e.type) {
    case 'line': case 'circle': case 'arc': case 'pline': case 'ellipse': case 'spline': case 'xline': case 'ray': {
      ctx.beginPath(); setWorld(rc);
      if (!traceEntity(rc, e)) { setScreen(rc); return; }
      strokeCurrent(rc, style, { selected, hover });
      return;
    }
    case 'point': return drawPoint(rc, e, style, selected, hover);
    case 'text': return drawText(rc, e, style, selected, hover);
    case 'hatch': return drawHatch(rc, e, style, selected, hover);
    case 'dim': return drawDim(rc, e, style, selected, hover);
    case 'leader': return drawLeader(rc, e, style, selected, hover);
    case 'insert': return drawInsert(rc, e, style, selected, hover, depth);
    case 'viewport': return drawViewport(rc, e, style, selected);
    default: return;
  }
}

function drawInsert(rc, e, style, selected, hover, depth) {
  const def = rc.doc.blocks[e.name];
  if (!def || depth > MAX_DEPTH) return;
  const saved = rc.worldM;
  rc.worldM = M.mul(saved, insertMatrix(e, def.base));
  rc._vis = null;
  for (const be of def.entities) {
    const layer = rc.layers.get((be.layer || '0').toLowerCase());
    if (layer && (!layer.on || layer.frozen) && be.layer !== '0') continue;
    const inherit = (be.layer === '0' || be.layer === undefined) ? { ...e } : null;
    drawEntity(rc, inherit ? { ...be, layer: e.layer } : be, { parent: style, selected, hover, depth: depth + 1 });
  }
  rc.worldM = saved; rc._vis = null;
}

function drawPoint(rc, e, style, selected, hover) {
  const ctx = rc.ctx, mode = rc.doc.settings.pdmode | 0, base = mode & 7;
  let size = rc.doc.settings.pdsize;
  const pxSize = size > 0 ? size * rc.k : size < 0 ? (-size / 100) * rc.H : rc.H * .05;
  const s = M.apply(rc.worldM, e.p);
  setScreen(rc);
  ctx.strokeStyle = hover ? rc.theme.hover : selected ? rc.theme.select : style.color; ctx.fillStyle = ctx.strokeStyle; ctx.lineWidth = 1.2; ctx.globalAlpha = style.alpha;
  ctx.beginPath();
  const half = pxSize / 2;
  if (base === 0) { ctx.fillRect(s.x - 1, s.y - 1, 2.4, 2.4); }
  else if (base === 2) { ctx.moveTo(s.x - half, s.y); ctx.lineTo(s.x + half, s.y); ctx.moveTo(s.x, s.y - half); ctx.lineTo(s.x, s.y + half); }
  else if (base === 3) { ctx.moveTo(s.x - half, s.y - half); ctx.lineTo(s.x + half, s.y + half); ctx.moveTo(s.x + half, s.y - half); ctx.lineTo(s.x - half, s.y + half); }
  else if (base === 4) { ctx.moveTo(s.x, s.y); ctx.lineTo(s.x, s.y - half); }
  if (mode & 32) ctx.arc(s.x, s.y, half, 0, TAU);
  if (mode & 64) ctx.rect(s.x - half, s.y - half, pxSize, pxSize);
  ctx.stroke(); ctx.globalAlpha = 1;
}

/** Draw a text entity (also used by dimensions and leaders). */
export function drawTextAt(rc, t, color, alpha, selected, hover) {
  const ctx = rc.ctx, lay = textLayout(t), kpx = rc.k * lay.em;
  const m = M.mul(rc.worldM, M.mul(M.translate(t.p.x, t.p.y), M.rotate(t.rot || 0)));
  const d = rc.dpr;
  const apply = () => ctx.setTransform(d * m[0], d * m[1], d * m[2], d * m[3], d * m[4], d * m[5]);
  const hpx = t.h * rc.k;
  if (hpx < 1.6) {                                  // too small to read: draw a faint baseline
    setScreen(rc); ctx.strokeStyle = color; ctx.globalAlpha = .55 * alpha; ctx.lineWidth = 1; ctx.beginPath();
    for (const l of lay.lines) { const a = M.apply(m, { x: l.x, y: l.y }), b = M.apply(m, { x: l.x + l.w, y: l.y }); ctx.moveTo(a.x, a.y); ctx.lineTo(b.x, b.y); }
    ctx.stroke(); ctx.globalAlpha = 1; return;
  }
  void kpx;
  apply(); ctx.scale(1, -1);
  ctx.font = `${lay.em}px ${FONT}`; ctx.textBaseline = 'alphabetic'; ctx.fillStyle = hover ? rc.theme.hover : color; ctx.globalAlpha = alpha;
  for (const l of lay.lines) ctx.fillText(l.str, l.x, -l.y);
  ctx.globalAlpha = 1;
  if (selected || hover) {
    ctx.setTransform(d, 0, 0, d, 0, 0);
    const b = lay.box, pts = [[b.x0, b.y0], [b.x1, b.y0], [b.x1, b.y1], [b.x0, b.y1]].map(([x, y]) => M.apply(m, { x, y }));
    ctx.strokeStyle = selected ? rc.theme.select : rc.theme.hover; ctx.lineWidth = 1; ctx.setLineDash(selected ? [4, 3] : []); ctx.beginPath(); pts.forEach((p, i) => i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y)); ctx.closePath(); ctx.stroke(); ctx.setLineDash([]);
  }
  setScreen(rc);
}

function drawText(rc, e, style, selected, hover) { drawTextAt(rc, e, style.color, style.alpha, selected, hover); }

function drawLeader(rc, e, style, selected, hover) {
  const ctx = rc.ctx;
  ctx.beginPath(); setWorld(rc);
  e.pts.forEach((p, i) => i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y));
  strokeCurrent(rc, style, { selected, hover });
  // arrowhead
  if (e.pts.length > 1) {
    const dim = rc.doc.settings.dimStyles[rc.doc.settings.curDimStyle], size = (dim?.asz || 2.5) * (dim?.scale || 1);
    const dir = g.unit(g.sub(e.pts[1], e.pts[0]));
    fillPoly(rc, arrowPolygon({ tip: e.pts[0], dir, size, style: 'closed' }), style);
  }
  if (e.s) drawTextAt(rc, leaderText(e), style.color, style.alpha, false, hover);
}

function fillPoly(rc, pts, style) {
  if (!pts) return;
  const ctx = rc.ctx; ctx.beginPath(); setWorld(rc);
  pts.forEach((p, i) => i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y)); ctx.closePath();
  setScreen(rc); ctx.fillStyle = style.color; ctx.globalAlpha = style.alpha; ctx.fill(); ctx.globalAlpha = 1;
}

function drawDim(rc, e, style, selected, hover) {
  const ctx = rc.ctx, geo = dimGeometry(e, rc.doc);
  ctx.beginPath(); setWorld(rc);
  traceSegs(ctx, geo.segs.filter(s => s !== geo.arc), false);
  // traceSegs joins segments into one path; lines are independent so restart each
  ctx.beginPath(); setWorld(rc);
  for (const s of geo.segs) { if (s.t === 'l') { ctx.moveTo(s.a.x, s.a.y); ctx.lineTo(s.b.x, s.b.y); } else { ctx.moveTo(s.c.x + s.r * Math.cos(s.a0), s.c.y + s.r * Math.sin(s.a0)); ctx.arc(s.c.x, s.c.y, s.r, s.a0, s.a0 + s.sw, s.sw < 0); } }
  for (const a of geo.arrows) for (const [p, q] of arrowLines(a)) { ctx.moveTo(p.x, p.y); ctx.lineTo(q.x, q.y); }
  strokeCurrent(rc, style, { selected, hover, forceWidth: 0 });
  for (const a of geo.arrows) fillPoly(rc, arrowPolygon(a), style);
  drawTextAt(rc, { p: geo.text.p, h: geo.text.h, s: geo.text.s, rot: geo.text.rot, jh: geo.text.jh, jv: geo.text.jv }, style.color, style.alpha, false, hover);
}

function loopsPath(rc, loops) {
  const ctx = rc.ctx;
  ctx.beginPath(); setWorld(rc);
  for (const l of loops) traceSegs(ctx, g.plineSegs(l, true), true);
}

function drawHatch(rc, e, style, selected, hover) {
  const ctx = rc.ctx;
  const color = e.color !== undefined && !rc.mono ? colorToCss(e.color, { dark: rc.theme.dark }) : style.color;
  if (e.pattern === 'SOLID') {
    if (rc.fill) { loopsPath(rc, e.loops); setScreen(rc); ctx.globalAlpha = style.alpha * .9; ctx.fillStyle = hover ? rc.theme.hover : color; ctx.fill('evenodd'); ctx.globalAlpha = 1; }
    else { loopsPath(rc, e.loops); strokeCurrent(rc, { ...style, color }, {}); }
    if (selected) { loopsPath(rc, e.loops); strokeCurrent(rc, style, { selected: true }); }
    return;
  }
  const lines = hatchSegments(e, rc.doc);
  if (lines === null) { loopsPath(rc, e.loops); setScreen(rc); ctx.globalAlpha = .35; ctx.fillStyle = color; ctx.fill('evenodd'); ctx.globalAlpha = 1; return; }
  const vis = rc.view.isPlan ? visibleWorld(rc) : null;
  ctx.beginPath(); setWorld(rc);
  for (const [x0, y0, x1, y1] of lines) {
    if (vis && (Math.max(x0, x1) < vis.minX || Math.min(x0, x1) > vis.maxX || Math.max(y0, y1) < vis.minY || Math.min(y0, y1) > vis.maxY)) continue;
    ctx.moveTo(x0, y0); ctx.lineTo(x1, y1);
  }
  setScreen(rc);
  ctx.strokeStyle = hover ? rc.theme.hover : color; ctx.globalAlpha = style.alpha; ctx.lineWidth = hover ? 2 : 1; ctx.lineCap = 'round'; ctx.stroke(); ctx.globalAlpha = 1;
  if (selected) { loopsPath(rc, e.loops); strokeCurrent(rc, { ...style, color: rc.theme.select }, { selected: true }); }
}

function drawViewport(rc, e, style, selected) {
  const ctx = rc.ctx; ctx.beginPath(); setWorld(rc);
  ctx.rect(e.c.x - e.w / 2, e.c.y - e.h / 2, e.w, e.h);
  strokeCurrent(rc, style, { selected });
}

// ---------------------------------------------------------------- grid, axes, UCS icon

export function drawGrid(rc) {
  const { ctx, prefs, theme, view } = rc;
  if (!prefs.GRIDMODE || !view.canDraft) return;
  const b = (() => { const c = [[0, 0], [rc.W, 0], [rc.W, rc.H], [0, rc.H]].map(([x, y]) => view.toWorld(x, y)); return g.ptsBounds(c); })();
  let gx = prefs.GRIDUNIT_X, gy = prefs.GRIDUNIT_Y;
  const major = Math.max(1, prefs.GRIDMAJOR | 0);
  while (gx * view.scale < 9) { gx *= major; gy *= major; if (gx > 1e12) return; }
  const nx = (b.maxX - b.minX) / gx, ny = (b.maxY - b.minY) / gy;
  if (nx > 800 || ny > 800) return;
  const line = (alpha, step, stepY, every) => {
    ctx.beginPath(); setWorld(rc);
    for (let i = Math.ceil(b.minX / step); i * step <= b.maxX; i++) { if (every && i % every === 0) continue; ctx.moveTo(i * step, b.minY); ctx.lineTo(i * step, b.maxY); }
    for (let j = Math.ceil(b.minY / stepY); j * stepY <= b.maxY; j++) { if (every && j % every === 0) continue; ctx.moveTo(b.minX, j * stepY); ctx.lineTo(b.maxX, j * stepY); }
    setScreen(rc); ctx.strokeStyle = theme.grid; ctx.globalAlpha = alpha; ctx.lineWidth = 1; ctx.stroke(); ctx.globalAlpha = 1;
  };
  line(theme.gridAlpha, gx, gy, major);
  // major lines
  ctx.beginPath(); setWorld(rc);
  const mx = gx * major, my = gy * major;
  for (let i = Math.ceil(b.minX / mx); i * mx <= b.maxX; i++) { ctx.moveTo(i * mx, b.minY); ctx.lineTo(i * mx, b.maxY); }
  for (let j = Math.ceil(b.minY / my); j * my <= b.maxY; j++) { ctx.moveTo(b.minX, j * my); ctx.lineTo(b.maxX, j * my); }
  setScreen(rc); ctx.strokeStyle = theme.gridMajor; ctx.lineWidth = 1; ctx.stroke();
  // axes
  for (const [axis, col] of [['x', 'rgba(230,90,90,.55)'], ['y', 'rgba(90,200,120,.55)']]) {
    ctx.beginPath(); setWorld(rc);
    if (axis === 'x') { ctx.moveTo(b.minX, 0); ctx.lineTo(b.maxX, 0); } else { ctx.moveTo(0, b.minY); ctx.lineTo(0, b.maxY); }
    setScreen(rc); ctx.strokeStyle = col; ctx.lineWidth = 1; ctx.stroke();
  }
}

export function drawUcsIcon(rc) {
  if (!rc.prefs.UCSICON) return;
  const { ctx, view } = rc, o = { x: 54, y: rc.H - 52 };
  setScreen(rc);
  const ax = (v, color, label) => {
    const p = view.isPlan ? { x: v.x * 38, y: -v.y * 38 } : (() => { const a = view.project({ x: v.x, y: v.y, z: v.z || 0 }), z = view.project({ x: 0, y: 0, z: 0 }); return { x: (a.x - z.x) / view.scale * 38, y: (a.y - z.y) / view.scale * 38 }; })();
    ctx.strokeStyle = color; ctx.fillStyle = color; ctx.lineWidth = 1.6; ctx.beginPath(); ctx.moveTo(o.x, o.y); ctx.lineTo(o.x + p.x, o.y + p.y); ctx.stroke();
    const a = Math.atan2(p.y, p.x), tip = { x: o.x + p.x, y: o.y + p.y };
    ctx.beginPath(); ctx.moveTo(tip.x, tip.y); ctx.lineTo(tip.x - 7 * Math.cos(a - .38), tip.y - 7 * Math.sin(a - .38)); ctx.lineTo(tip.x - 7 * Math.cos(a + .38), tip.y - 7 * Math.sin(a + .38)); ctx.closePath(); ctx.fill();
    ctx.font = `11px ${FONT}`; ctx.fillText(label, tip.x + (p.x >= 0 ? 6 : -14) + (Math.abs(p.x) < 4 ? -3 : 0), tip.y + (p.y <= 0 ? -4 : 12) + (Math.abs(p.y) < 4 ? 4 : 0));
  };
  if (view.isPlan) {
    // 2D icon: X right, Y up, square at the origin ("W" = world)
    ax({ x: 1, y: 0 }, '#ff6b5e', 'X'); ax({ x: 0, y: 1 }, '#6bdc86', 'Y');
    ctx.strokeStyle = rc.theme.dark ? '#cfd6dd' : '#445'; ctx.lineWidth = 1; ctx.strokeRect(o.x - 5, o.y - 5, 10, 10);
  } else {
    ax({ x: 1, y: 0, z: 0 }, '#ff6b5e', 'X'); ax({ x: 0, y: 1, z: 0 }, '#6bdc86', 'Y'); ax({ x: 0, y: 0, z: 1 }, '#5fb4ff', 'Z');
  }
}

// ---------------------------------------------------------------- frame

export function renderScene(ctx, W, H, dpr, editor, prefs, theme, extra = {}) {
  const rc = makeContext(ctx, W, H, dpr, editor, prefs, theme);
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.fillStyle = theme.canvas; ctx.fillRect(0, 0, W, H);
  drawGrid(rc);
  if (extra.solids) extra.solids(rc);
  const doc = editor.doc;
  for (const e of doc.entities) {
    if (!doc.isVisible(e)) continue;
    if (e.type === 'xline' || e.type === 'ray' || true) {
      const b = rc.view.isPlan ? boundsOf(e, doc) : null;
      if (b) { const v = visibleWorld(rc); if (b.maxX < v.minX || b.minX > v.maxX || b.maxY < v.minY || b.minY > v.maxY) continue; }
    }
    const selected = rc.sel.has(e.id) || rc.tempSel.has(e.id);
    drawEntity(rc, e, { selected });
  }
  drawUcsIcon(rc);
  setScreen(rc);
  return rc;
}

export const THEMES = {
  dark: { dark: true, canvas: '#212830', grid: '#ffffff', gridMajor: 'rgba(255,255,255,.10)', select: '#4aa8ff', hover: '#9fd3ff', cross: '#ffffff' },
  light: { dark: false, canvas: '#ffffff', grid: '#000000', gridMajor: 'rgba(0,0,0,.12)', select: '#1769c2', hover: '#2a8be0', cross: '#000000' },
};
THEMES.dark.gridAlpha = .07; THEMES.light.gridAlpha = .09;

void platform; void emSize; void g;
