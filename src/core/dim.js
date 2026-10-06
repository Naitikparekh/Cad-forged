// Dimension geometry. A dimension entity stores only its defining points; the drawn
// geometry (extension lines, dimension line, arrowheads, text) is derived from the dimension style.
import { angleOf, dist, sub, add, mul, unit, dot, perp, polar, mid, normAngleSigned, sweepCCW, TAU } from './geom.js';
import { platform } from './text.js';
import { fmtNum } from './style.js';

const readable = a => { a = normAngleSigned(a); return a > Math.PI / 2 + 1e-9 ? a - Math.PI : a <= -Math.PI / 2 + 1e-9 ? a + Math.PI : a; };

export function dimStyleOf(e, doc) {
  const s = doc.settings;
  return s.dimStyles[e.ds] || s.dimStyles[s.curDimStyle] || Object.values(s.dimStyles)[0];
}

function measureLabel(value, st) {
  let s = fmtNum(value * st.lfac, st.dec);
  if (st.zin & 8 && s.includes('.')) s = s.replace(/0+$/, '').replace(/\.$/, '');
  return s;
}

function labelFor(e, st, measured, kind) {
  let base;
  if (kind === 'ang') base = fmtNum(measured * 180 / Math.PI, 0).replace(/\.0+$/, '') + '°';
  else {
    base = measureLabel(measured, st);
    if (kind === 'rad') base = 'R' + base;
    else if (kind === 'dia') base = '⌀' + base;
  }
  base = st.pre + base + st.suf;
  if (e.txt === undefined || e.txt === '') return base;
  return e.txt.includes('<>') ? e.txt.replace('<>', base) : e.txt;
}

/**
 * Derive drawable geometry from a dimension entity.
 * @returns {{segs:Array, arrows:Array<{tip,dir,size,style}>, text:{p,rot,s,h,jh,jv}, measured:number, kind:string, style:object}}
 */
export function dimGeometry(e, doc) {
  const st = dimStyleOf(e, doc), k = st.scale;
  const asz = st.asz * k, exo = st.exo * k, exe = st.exe * k, gap = st.gap * k, h = st.txt * k;
  const segs = [], arrows = [];
  const line = (a, b) => segs.push({ t: 'l', a, b });
  const arrow = (tip, dir) => arrows.push({ tip, dir, size: asz, style: st.blk });
  const out = { segs, arrows, kind: e.kind, style: st };

  if (e.kind === 'lin' || e.kind === 'ali') {
    const ang = e.kind === 'ali' ? angleOf(e.p1, e.p2) : (e.ang || 0);
    const d = { x: Math.cos(ang), y: Math.sin(ang) }, n = perp(d);
    const side = dot(sub(e.dp, e.p1), n) >= 0 ? 1 : -1;
    const foot = p => add(p, mul(n, dot(sub(e.dp, p), n)));
    const A = foot(e.p1), B = foot(e.p2);
    const measured = Math.abs(dot(sub(e.p2, e.p1), d));
    const label = labelFor(e, st, measured, e.kind);
    const tw = platform.measureText(label, h);
    for (const [p, F] of [[e.p1, A], [e.p2, B]]) {
      const dir = mul(n, side), far = add(F, mul(dir, exe));
      line(dist(p, F) < exo ? F : add(p, mul(dir, exo)), far);
    }
    const L = dist(A, B), lineDir = L > 1e-12 ? unit(sub(B, A)) : d;
    const arrowsFit = L >= asz * 2.2, textFits = L >= tw + asz * 2 + gap * 2;
    if (arrowsFit) { arrow(A, lineDir); arrow(B, mul(lineDir, -1)); line(A, B); }
    else { arrow(A, mul(lineDir, -1)); arrow(B, lineDir); line(add(A, mul(lineDir, -asz * 2)), add(B, mul(lineDir, asz * 2))); }
    const textAng = readable(Math.atan2(lineDir.y, lineDir.x));
    const nTxt = { x: -Math.sin(textAng), y: Math.cos(textAng) }, mp = mid(A, B);
    let tp, jv = 'middle';
    if (e.tp) tp = e.tp;
    else if (textFits || !arrowsFit) { if (st.tad) { tp = add(mp, mul(nTxt, gap)); jv = 'bottom'; } else tp = mp; }
    else { tp = add(B, mul(lineDir, asz * 2 + gap + tw / 2)); if (st.tad) tp = add(tp, mul(nTxt, gap + h / 2)); }
    out.text = { p: tp, rot: textAng, s: label, h, jh: 'center', jv };
    out.measured = measured;
    out.dimLine = { A, B, dir: lineDir, normal: n, side };
    return out;
  }

  if (e.kind === 'rad' || e.kind === 'dia') {
    const c = e.c, r = dist(c, e.pt), u = unit(sub(e.pt, c));
    const measured = e.kind === 'rad' ? r : 2 * r;
    const label = labelFor(e, st, measured, e.kind);
    const far = e.kind === 'dia' ? sub(c, mul(u, r)) : c;
    if (e.kind === 'dia') arrow(far, u);
    arrow(e.pt, mul(u, -1));
    line(far, e.pt);
    if (dist(c, e.dp) < r) {
      out.text = { p: e.tp || mid(c, e.pt), rot: readable(angleOf(c, e.pt)), s: label, h, jh: 'center', jv: st.tad ? 'bottom' : 'middle' };
    } else {
      line(e.pt, e.dp);
      const right = e.dp.x >= e.pt.x, sgn = right ? 1 : -1;
      line(e.dp, add(e.dp, { x: sgn * asz, y: 0 }));
      out.text = { p: add(e.dp, { x: sgn * (asz + gap), y: 0 }), rot: 0, s: label, h, jh: right ? 'left' : 'right', jv: 'middle' };
    }
    if (st.cen > 0) {
      const cm = st.cen * k / 2;
      line({ x: c.x - cm, y: c.y }, { x: c.x + cm, y: c.y });
      line({ x: c.x, y: c.y - cm }, { x: c.x, y: c.y + cm });
    }
    out.measured = measured;
    return out;
  }

  if (e.kind === 'ang') {
    const c = e.c, a1 = angleOf(c, e.p1), a2 = angleOf(c, e.p2), rr = dist(c, e.dp);
    let a0 = a1, sw = sweepCCW(a1, a2);
    if (sweepCCW(a1, angleOf(c, e.dp)) > sw) { a0 = a2; sw = TAU - sw; }     // dim point lies on the reflex side
    out.arc = { t: 'a', c, r: rr, a0, sw };
    segs.push(out.arc);
    for (const [p, ang] of [[e.p1, a1], [e.p2, a2]]) {
      const d = { x: Math.cos(ang), y: Math.sin(ang) }, tipPt = polar(c, ang, rr);
      const from = dist(c, p) > exo ? add(p, mul(d, exo)) : c;
      if (dist(c, from) < rr + exe) line(from, add(tipPt, mul(d, exe)));
    }
    arrow(polar(c, a0, rr), { x: Math.sin(a0), y: -Math.cos(a0) });
    arrow(polar(c, a0 + sw, rr), { x: -Math.sin(a0 + sw), y: Math.cos(a0 + sw) });
    const am = a0 + sw / 2;
    out.text = { p: e.tp || polar(c, am, rr + gap), rot: readable(am + Math.PI / 2), s: labelFor(e, st, sw, 'ang'), h, jh: 'center', jv: 'bottom' };
    out.measured = sw;
    return out;
  }
  throw Error('Unknown dimension kind ' + e.kind);
}

/** Closed-triangle arrowhead polygon (tip, then two base corners). Null for non-filled styles. */
export function arrowPolygon(a) {
  if (a.style !== 'closed') return null;
  const { tip, dir, size } = a, n = { x: -dir.y, y: dir.x }, base = add(tip, mul(dir, size));
  return [tip, add(base, mul(n, size / 6)), add(base, mul(n, -size / 6))];
}

/** Open/tick arrowhead strokes as point pairs. */
export function arrowLines(a) {
  const { tip, dir, size } = a, n = { x: -dir.y, y: dir.x };
  if (a.style === 'open') { const base = add(tip, mul(dir, size)); return [[tip, add(base, mul(n, size / 4))], [tip, add(base, mul(n, -size / 4))]]; }
  if (a.style === 'tick') { const q = mul(unit({ x: dir.x + n.x, y: dir.y + n.y }), size / 2); return [[sub(tip, q), add(tip, q)]]; }
  return [];
}
