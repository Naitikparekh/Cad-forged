// Interactive overlay: rubber-band previews, AutoSnap markers, tracking vectors, selection windows,
// grips, hover highlight and the crosshair. Redrawn on every pointer move (the scene canvas is not).
import * as g from '../core/geom.js';
import { gripsOf } from '../core/entities.js';
import { drawEntity, makeContext } from './renderer.js';

const YELLOW = '#ffe14d';

function marker(ctx, type, x, y) {
  const s = 7;
  ctx.beginPath();
  switch (type) {
    case 'END': ctx.rect(x - s, y - s, s * 2, s * 2); break;
    case 'MID': ctx.moveTo(x, y - s); ctx.lineTo(x + s, y + s * .8); ctx.lineTo(x - s, y + s * .8); ctx.closePath(); break;
    case 'CEN': ctx.arc(x, y, s, 0, g.TAU); break;
    case 'NOD': ctx.arc(x, y, s, 0, g.TAU); ctx.moveTo(x - s * .7, y - s * .7); ctx.lineTo(x + s * .7, y + s * .7); ctx.moveTo(x + s * .7, y - s * .7); ctx.lineTo(x - s * .7, y + s * .7); break;
    case 'QUA': ctx.moveTo(x, y - s - 1); ctx.lineTo(x + s + 1, y); ctx.lineTo(x, y + s + 1); ctx.lineTo(x - s - 1, y); ctx.closePath(); break;
    case 'INT': ctx.moveTo(x - s, y - s); ctx.lineTo(x + s, y + s); ctx.moveTo(x + s, y - s); ctx.lineTo(x - s, y + s); break;
    case 'INS': ctx.rect(x - s, y - s, s * 1.4, s * 1.4); ctx.rect(x - s * .4, y - s * .4, s * 1.4, s * 1.4); break;
    case 'PER': ctx.moveTo(x - s, y + s); ctx.lineTo(x - s, y - s); ctx.moveTo(x - s, y + s); ctx.lineTo(x + s, y + s); ctx.rect(x - s, y + s - 5, 5, 5); break;
    case 'TAN': ctx.arc(x, y + 2, s - 1, 0, g.TAU); ctx.moveTo(x - s - 1, y - s + 1); ctx.lineTo(x + s + 1, y - s + 1); break;
    case 'NEA': ctx.moveTo(x - s, y - s); ctx.lineTo(x + s, y - s); ctx.lineTo(x - s, y + s); ctx.lineTo(x + s, y + s); ctx.closePath(); break;
    default: ctx.rect(x - s, y - s, s * 2, s * 2);
  }
  ctx.stroke();
}

function tipBox(ctx, text, x, y, dark = true) {
  ctx.font = '11px "Segoe UI", system-ui, Arial, sans-serif';
  const w = ctx.measureText(text).width + 10, h = 18;
  ctx.fillStyle = dark ? 'rgba(255,248,205,.97)' : 'rgba(255,248,205,1)'; ctx.strokeStyle = '#b3a24f'; ctx.lineWidth = 1;
  ctx.beginPath(); ctx.rect(x + .5, y + .5, w, h); ctx.fill(); ctx.stroke();
  ctx.fillStyle = '#2b2a1b'; ctx.textBaseline = 'middle'; ctx.fillText(text, x + 5, y + h / 2 + .5);
}

/**
 * @param st {mouse, cursor:{pt,snap,track}, preview[], window, hover, kind:'idle'|'point'|'select'|'entity'|'none', showCross, acquired[]}
 */
export function drawOverlay(ctx, W, H, dpr, editor, prefs, theme, st) {
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, W, H);
  const view = editor.view, doc = editor.doc;
  const rc = makeContext(ctx, W, H, dpr, editor, prefs, theme);
  const toS = p => view.toScreen(p);

  // hover highlight
  if (st.hover) { ctx.save(); drawEntity(rc, st.hover, { hover: true }); ctx.restore(); }

  // rubber-band preview
  for (const p of st.preview || []) {
    if (p.__rect) {
      const a = toS(p.a), b = toS(p.b), c = toS({ x: p.b.x, y: p.a.y }), d = toS({ x: p.a.x, y: p.b.y });
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0); ctx.strokeStyle = theme.dark ? '#e9edf1' : '#222'; ctx.lineWidth = 1; ctx.setLineDash([]);
      ctx.beginPath(); ctx.moveTo(a.x, a.y); ctx.lineTo(c.x, c.y); ctx.lineTo(b.x, b.y); ctx.lineTo(d.x, d.y); ctx.closePath(); ctx.stroke();
      continue;
    }
    const e = { layer: doc.cur.layer, ...(doc.cur.color !== undefined ? { color: doc.cur.color } : {}), ...p };
    ctx.save(); drawEntity(rc, e, { selected: false }); ctx.restore();
  }

  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

  // acquired object-snap tracking points
  for (const a of st.acquired || []) {
    const s = toS(a); ctx.strokeStyle = 'rgba(255,225,77,.85)'; ctx.lineWidth = 1.4; ctx.beginPath(); ctx.moveTo(s.x - 4, s.y); ctx.lineTo(s.x + 4, s.y); ctx.moveTo(s.x, s.y - 4); ctx.lineTo(s.x, s.y + 4); ctx.stroke();
  }
  // tracking vectors
  const tr = st.cursor?.track;
  if (tr?.lines?.length) {
    ctx.strokeStyle = theme.dark ? 'rgba(210,225,240,.7)' : 'rgba(60,70,90,.7)'; ctx.lineWidth = 1; ctx.setLineDash([3, 4]);
    for (const l of tr.lines) { const a = toS(l.a), b = toS(l.b); ctx.beginPath(); ctx.moveTo(a.x, a.y); ctx.lineTo(b.x, b.y); ctx.stroke(); }
    ctx.setLineDash([]);
  }

  // selection window
  if (st.window) {
    const a = toS(st.window.a), b = toS(st.window.b), crossing = st.window.crossing;
    const x = Math.min(a.x, b.x), y = Math.min(a.y, b.y), w = Math.abs(a.x - b.x), hh = Math.abs(a.y - b.y);
    ctx.fillStyle = crossing ? 'rgba(80,190,110,.18)' : 'rgba(70,140,230,.20)'; ctx.fillRect(x, y, w, hh);
    ctx.strokeStyle = crossing ? '#79e29a' : '#6cb2ff'; ctx.lineWidth = 1; ctx.setLineDash(crossing ? [5, 3] : []); ctx.strokeRect(x + .5, y + .5, w, hh); ctx.setLineDash([]);
  }

  // grips
  if (editor.sel.size && prefs.GRIPS && editor.sel.size <= prefs.GRIPOBJLIMIT && !st.hideGrips) {
    for (const e of editor.selected) {
      if (doc.isLocked(e)) continue;
      for (const gr of gripsOf(e, doc)) {
        const s = toS(gr.pt); if (s.x < -10 || s.y < -10 || s.x > W + 10 || s.y > H + 10) continue;
        const hot = st.hotGrip && st.hotGrip.entity === e && st.hotGrip.grip.id === gr.id;
        const hov = st.mouse && Math.hypot(s.x - st.mouse.x, s.y - st.mouse.y) <= 6;
        ctx.fillStyle = hot ? '#ff3b30' : hov ? '#ffa63d' : '#2f7bff'; ctx.strokeStyle = theme.dark ? '#0a1c3d' : '#12306b'; ctx.lineWidth = 1;
        const sz = prefs.GRIPSIZE + 2;
        if (gr.kind === 'move' && gr.id === 'm') { ctx.beginPath(); ctx.rect(s.x - sz / 2 - 1, s.y - sz / 2 + 1, sz + 2, sz - 2); ctx.fill(); ctx.stroke(); }
        else { ctx.fillRect(s.x - sz / 2, s.y - sz / 2, sz, sz); ctx.strokeRect(s.x - sz / 2 + .5, s.y - sz / 2 + .5, sz - 1, sz - 1); }
      }
    }
  }

  // snap marker + tooltips
  const cur = st.cursor, m = st.mouse;
  if (cur?.snap) {
    const s = toS(cur.snap.pt); ctx.strokeStyle = YELLOW; ctx.lineWidth = 2; marker(ctx, cur.snap.type, s.x, s.y);
    if (prefs.AUTOSNAP & 2) tipBox(ctx, cur.snap.name, s.x + 14, s.y + 12, theme.dark);
  } else if (tr?.label && (prefs.AUTOSNAP & 32) && m) tipBox(ctx, tr.label, m.x + 16, m.y + 18, theme.dark);

  // crosshair / pickbox
  if (m && st.showCross !== false) {
    const col = theme.cross, kind = st.kind;
    ctx.strokeStyle = col; ctx.lineWidth = 1; ctx.setLineDash([]);
    const x = Math.round(m.x) + .5, y = Math.round(m.y) + .5;
    const frac = Math.max(1, Math.min(100, prefs.CURSORSIZE)) / 100, armX = frac >= 1 ? 4000 : frac * W / 2, armY = frac >= 1 ? 4000 : frac * H / 2;
    const arm = Math.min(armX, armY) * (frac >= 1 ? 1 : 1);
    const pb = prefs.PICKBOX * 2 + 1;
    if (kind !== 'select') {
      ctx.beginPath(); ctx.moveTo(x - arm, y); ctx.lineTo(x + arm, y); ctx.moveTo(x, y - arm); ctx.lineTo(x, y + arm); ctx.stroke();
    }
    if (kind === 'idle' || kind === 'select' || kind === 'entity') { ctx.strokeRect(x - pb / 2, y - pb / 2, pb, pb); }
  }
}
