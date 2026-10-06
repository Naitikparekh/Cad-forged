// DXF reader: tolerant ASCII parser for the entities CadForge understands. DOM-free.
// Supported: LINE CIRCLE ARC LWPOLYLINE POLYLINE ELLIPSE SPLINE POINT TEXT MTEXT INSERT HATCH SOLID/TRACE XLINE RAY LEADER
// and DIMENSION (imported as the geometry of its anonymous block, so it looks exactly like the source drawing).
import * as g from '../core/geom.js';
import { E } from '../core/entities.js';
import { Drawing } from '../core/drawing.js';
import { newLayer, ltypeKey, LW_BYLAYER } from '../core/style.js';
import { PATTERNS } from '../core/hatch.js';

const TAU = Math.PI * 2;

// ---------------------------------------------------------------- text decoding

/** Decode %%c, %%d, %%p and \U+XXXX escapes in TEXT / names. */
export function decodeText(s) {
  return String(s)
    .replace(/\\U\+([0-9a-fA-F]{4})/g, (_, h) => String.fromCharCode(parseInt(h, 16)))
    .replace(/%%[cC]/g, '⌀').replace(/%%[dD]/g, '°').replace(/%%[pP]/g, '±').replace(/%%[uUoO]/g, '').replace(/%%%/g, '%');
}

/** Strip MTEXT formatting codes, returning plain text with \n line breaks. */
export function decodeMText(src) {
  let s = decodeText(src), out = '';
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (c === '\\') {
      const n = s[i + 1];
      if (n === 'P' || n === 'p') { out += '\n'; i++; }
      else if (n === '~') { out += ' '; i++; }
      else if (n === '\\' || n === '{' || n === '}') { out += n; i++; }
      else if ('LlOoKk'.includes(n)) i++;
      else if (n === 'S') { const m = /^\\S([^;]*);/.exec(s.slice(i)); if (m) { out += m[1].replace(/[\^#]/, '/'); i += m[0].length - 1; } }
      else if ('fFCcHhWwQqTtAaXx'.includes(n)) { const j = s.indexOf(';', i); i = j < 0 ? s.length : j; }
      else out += c;
    } else if (c === '{' || c === '}') { /* grouping */ } else out += c;
  }
  return out.replace(/\n+$/, '');
}

// ---------------------------------------------------------------- tokenizer

function tokenize(text) {
  const lines = text.split(/\r\n|\n|\r/);
  const out = [];
  for (let i = 0; i + 1 < lines.length; i += 2) {
    const code = parseInt(lines[i].trim(), 10);
    if (Number.isNaN(code)) continue;
    out.push([code, lines[i + 1].trim()]);
  }
  return out;
}

/** Split into records: each starts with code 0. Returns [{type, g:[[code,value],...]}]. */
function records(tokens) {
  const recs = []; let cur = null;
  for (const t of tokens) {
    if (t[0] === 0) { cur = { type: t[1], g: [] }; recs.push(cur); } else if (cur) cur.g.push(t);
  }
  return recs;
}

const first = (r, code, d) => { for (const [c, v] of r.g) if (c === code) return v; return d; };
const firstNum = (r, code, d = 0) => { const v = first(r, code); return v === undefined ? d : parseFloat(v); };
const all = (r, code) => r.g.filter(([c]) => c === code).map(([, v]) => v);
const allNum = (r, code) => all(r, code).map(parseFloat);
const pt = (r, i = 0) => ({ x: firstNum(r, 10 + i), y: firstNum(r, 20 + i) });

// ---------------------------------------------------------------- sections

function sections(recs) {
  const out = {}; let list = null;
  for (const r of recs) {
    if (r.type === 'SECTION') { const n = first(r, 2); list = n ? (out[n] = []) : null; }
    else if (r.type === 'ENDSEC') list = null; else if (list) list.push(r);
  }
  return out;
}

function parseHeader(tokens) {
  const vars = {}; let inHeader = false, cur = null;
  for (let i = 0; i < tokens.length; i++) {
    const [c, v] = tokens[i];
    if (c === 0 && v === 'SECTION' && tokens[i + 1]?.[1] === 'HEADER') { inHeader = true; i++; continue; }
    if (c === 0 && v === 'ENDSEC') { if (inHeader) break; }
    if (!inHeader) continue;
    if (c === 9) { cur = vars[v] = []; } else if (cur) cur.push([c, v]);
  }
  return vars;
}

// ---------------------------------------------------------------- colour / properties

function colorOf(r) {
  const tc = first(r, 420);
  if (tc !== undefined) { const n = parseInt(tc, 10) & 0xFFFFFF; return '#' + n.toString(16).padStart(6, '0'); }
  const c = first(r, 62);
  if (c === undefined) return undefined;
  const n = parseInt(c, 10);
  if (n === 256) return undefined;
  if (n === 0) return 'BYBLOCK';
  return Math.abs(n) <= 255 ? Math.abs(n) : undefined;
}

function ltypeOf(name) {
  if (!name) return undefined;
  const u = String(name).toUpperCase();
  if (u === 'BYLAYER') return undefined;
  if (u === 'BYBLOCK') return 'BYBLOCK';
  return ltypeKey(name);
}

function lwOf(r) {
  const v = first(r, 370);
  if (v === undefined) return undefined;
  const n = parseInt(v, 10);
  return n === LW_BYLAYER ? undefined : n;
}

// ---------------------------------------------------------------- B-spline evaluation (control-point splines)

function bsplineSample(deg, knots, ctrl, weights, perSpan = 12) {
  const n = ctrl.length - 1;
  const t0 = knots[deg], t1 = knots[n + 1];
  const spans = Math.max(1, new Set(knots.slice(deg, n + 2)).size - 1);
  const count = Math.min(2000, Math.max(24, spans * perSpan));
  const at = t => {
    let k = deg;
    while (k < n && t >= knots[k + 1]) k++;
    const d = ctrl.slice(k - deg, k + 1).map((p, i) => ({ x: p.x * (weights?.[k - deg + i] ?? 1), y: p.y * (weights?.[k - deg + i] ?? 1), w: weights?.[k - deg + i] ?? 1 }));
    for (let r = 1; r <= deg; r++) for (let j = deg; j >= r; j--) {
      const den = knots[j + 1 + k - r] - knots[j + k - deg], a = den === 0 ? 0 : (t - knots[j + k - deg]) / den;
      d[j] = { x: (1 - a) * d[j - 1].x + a * d[j].x, y: (1 - a) * d[j - 1].y + a * d[j].y, w: (1 - a) * d[j - 1].w + a * d[j].w };
    }
    return { x: d[deg].x / d[deg].w, y: d[deg].y / d[deg].w };
  };
  const pts = [];
  for (let i = 0; i < count; i++) pts.push(at(t0 + (t1 - t0) * i / count));
  pts.push(ctrl[n]);
  return pts;
}

// ---------------------------------------------------------------- hatch boundaries

function hatchLoops(r) {
  // walk groups in order: 91 loop count, then per loop 92 flags ...
  const gs = r.g, loops = [];
  let i = gs.findIndex(([c]) => c === 91);
  if (i < 0) return loops;
  const n = parseInt(gs[i][1], 10); i++;
  const num = (code) => { while (i < gs.length && gs[i][0] !== code) i++; return i < gs.length ? parseFloat(gs[i++][1]) : NaN; };
  for (let l = 0; l < n; l++) {
    const flags = num(92);
    if (Number.isNaN(flags)) break;
    if (flags & 2) {                                   // polyline path
      const hasBulge = num(72), nv = num(93);
      num(73);
      const verts = [];
      for (let k = 0; k < nv; k++) {
        const x = num(10), y = num(20); let b = 0;
        if (hasBulge && gs[i] && gs[i][0] === 42) b = parseFloat(gs[i++][1]);
        verts.push({ x, y, b });
      }
      if (verts.length > 2) loops.push(verts);
    } else {                                           // edge path
      const ne = num(93), edges = [];
      for (let k = 0; k < ne; k++) {
        const type = num(72);
        if (type === 1) { const a = { x: num(10), y: num(20) }, b = { x: num(11), y: num(21) }; edges.push({ t: 'l', a, b }); }
        else if (type === 2) {
          const c = { x: num(10), y: num(20) }, rr = num(40), a0 = g.rad(num(50)), a1 = g.rad(num(51)); const ccw = num(73);
          let sw = ccw ? g.sweepCCW(a0, a1) : -g.sweepCCW(a1, a0); if (Math.abs(sw) < 1e-12) sw = ccw ? TAU : -TAU;
          edges.push({ t: 'a', c, r: rr, a0, sw });
        } else if (type === 3) {
          const c = { x: num(10), y: num(20) }, m = { x: num(11), y: num(21) }, ratio = num(40), a0 = g.rad(num(50)), a1 = g.rad(num(51)); const ccw = num(73);
          const sw = ccw ? g.sweepCCW(a0, a1) || TAU : -(g.sweepCCW(a1, a0) || TAU), steps = 48, pts = [];
          for (let s = 0; s <= steps; s++) { const t = a0 + sw * s / steps; pts.push({ x: c.x + m.x * Math.cos(t) - m.y * ratio * Math.sin(t), y: c.y + m.y * Math.cos(t) + m.x * ratio * Math.sin(t) }); }
          for (let s = 0; s < steps; s++) edges.push({ t: 'l', a: pts[s], b: pts[s + 1] });
        } else if (type === 4) {                       // spline edge: approximate through its control polygon
          const deg = num(94); num(73); num(74); const nk = num(95), nc = num(96);
          const knots = []; for (let k = 0; k < nk; k++) knots.push(num(40));
          const ctrl = []; for (let k = 0; k < nc; k++) ctrl.push({ x: num(10), y: num(20) });
          if (ctrl.length > deg && knots.length === ctrl.length + deg + 1) { const pts = bsplineSample(deg, knots, ctrl, null, 10); for (let s = 0; s + 1 < pts.length; s++) edges.push({ t: 'l', a: pts[s], b: pts[s + 1] }); }
          else for (let s = 0; s + 1 < ctrl.length; s++) edges.push({ t: 'l', a: ctrl[s], b: ctrl[s + 1] });
        } else break;
      }
      const loop = chain(edges);
      if (loop.length > 2) loops.push(loop);
      // skip the source-object references so the next loop's 92 is found
      const cnt = num(97); void cnt;
    }
  }
  return loops;
}

const startOf = s => s.t === 'l' ? s.a : g.arcStart(s);
const endOf = s => s.t === 'l' ? s.b : g.arcEnd(s);
const rev = s => s.t === 'l' ? { t: 'l', a: s.b, b: s.a } : { t: 'a', c: s.c, r: s.r, a0: s.a0 + s.sw, sw: -s.sw };

/** Join edges end-to-start into a closed vertex loop with bulges. */
function chain(edges) {
  if (!edges.length) return [];
  const left = edges.slice(), seq = [left.shift()];
  const tol = 1e-5 * Math.max(1, ...edges.map(e => Math.abs(startOf(e).x) + Math.abs(startOf(e).y)));
  while (left.length) {
    const tail = endOf(seq[seq.length - 1]);
    let k = left.findIndex(e => g.dist(startOf(e), tail) <= tol), flip = false;
    if (k < 0) { k = left.findIndex(e => g.dist(endOf(e), tail) <= tol); flip = true; }
    if (k < 0) { k = 0; flip = false; }
    const e = left.splice(k, 1)[0];
    seq.push(flip ? rev(e) : e);
  }
  return seq.map(s => { const p = startOf(s); return { x: p.x, y: p.y, b: s.t === 'a' ? g.bulgeOf(s) : 0 }; });
}

// ---------------------------------------------------------------- entity conversion

function mirrorX(e) {       // extrusion (0,0,-1): the OCS x axis is mirrored
  const m = p => ({ x: -p.x, y: p.y });
  switch (e.type) {
    case 'circle': e.c = m(e.c); break;
    case 'arc': { e.c = m(e.c); const a0 = Math.PI - e.a1, a1 = Math.PI - e.a0; e.a0 = g.normAngle(a0); e.a1 = g.normAngle(a1); break; }
    case 'pline': e.v = e.v.map(v => ({ x: -v.x, y: v.y, b: -(v.b || 0) })); break;
    case 'text': e.p = m(e.p); break;
    default: break;
  }
  return e;
}

const JH = [['left', 0], ['center', 1], ['right', 2], ['left', 0], ['center', 1], ['left', 0]];
const JV = ['baseline', 'bottom', 'middle', 'top'];
const ATTACH = { 1: ['left', 'top'], 2: ['center', 'top'], 3: ['right', 'top'], 4: ['left', 'middle'], 5: ['center', 'middle'], 6: ['right', 'middle'], 7: ['left', 'bottom'], 8: ['center', 'bottom'], 9: ['right', 'bottom'] };

function convert(r, ctx) {
  const props = { layer: decodeText(first(r, 8, '0')) };
  const col = colorOf(r); if (col !== undefined) props.color = col;
  const lt = ltypeOf(first(r, 6)); if (lt !== undefined) props.ltype = lt;
  const lw = lwOf(r); if (lw !== undefined) props.lw = lw;
  const nz = firstNum(r, 230, 1), flip = nz < 0;
  const out = [];
  const push = (e, orient = true) => { if (e) { Object.assign(e, props); out.push(flip && orient ? mirrorX(e) : e); } };
  switch (r.type) {
    case 'LINE': push(E.line(pt(r), pt(r, 1)), false); break;
    case 'CIRCLE': { const rr = firstNum(r, 40); if (rr > 0) push(E.circle(pt(r), rr)); break; }
    case 'ARC': { const rr = firstNum(r, 40); if (rr > 0) push(E.arc(pt(r), rr, g.rad(firstNum(r, 50)), g.rad(firstNum(r, 51)))); break; }
    case 'POINT': push(E.point(pt(r)), false); break;
    case 'LWPOLYLINE': {
      const verts = []; let cur = null;
      for (const [c, v] of r.g) {
        if (c === 10) { cur = { x: parseFloat(v), y: 0, b: 0 }; verts.push(cur); } else if (c === 20 && cur) cur.y = parseFloat(v); else if (c === 42 && cur) cur.b = parseFloat(v);
      }
      if (verts.length > 1) push(E.pline(verts, (parseInt(first(r, 70, '0'), 10) & 1) === 1));
      break;
    }
    case 'POLYLINE': {
      const flags = parseInt(first(r, 70, '0'), 10);
      if (flags & (16 | 64)) break;                                  // meshes
      const verts = (r.vertices || []).filter(v => !(parseInt(first(v, 70, '0'), 10) & 16)).map(v => ({ x: firstNum(v, 10), y: firstNum(v, 20), b: firstNum(v, 42, 0) }));
      if (verts.length > 1) push(E.pline(verts, (flags & 1) === 1));
      break;
    }
    case 'ELLIPSE': {
      const m = pt(r, 1), ratio = firstNum(r, 40, 1);
      if (g.len(m) > 0 && ratio > 0) push(E.ellipse(pt(r), m, Math.min(1, ratio), firstNum(r, 41, 0), firstNum(r, 42, TAU)), false);
      break;
    }
    case 'SPLINE': {
      const flags = parseInt(first(r, 70, '0'), 10), deg = parseInt(first(r, 71, '3'), 10);
      const fit = []; { let cur = null; for (const [c, v] of r.g) { if (c === 11) { cur = { x: parseFloat(v), y: 0 }; fit.push(cur); } else if (c === 21 && cur) cur.y = parseFloat(v); } }
      const ctrl = []; { let cur = null; for (const [c, v] of r.g) { if (c === 10) { cur = { x: parseFloat(v), y: 0 }; ctrl.push(cur); } else if (c === 20 && cur) cur.y = parseFloat(v); } }
      const knots = allNum(r, 40), weights = allNum(r, 41);
      let pts = null;
      if (fit.length >= 2) pts = fit;
      else if (ctrl.length > deg && knots.length === ctrl.length + deg + 1) pts = bsplineSample(deg, knots, ctrl, weights.length === ctrl.length ? weights : null);
      else if (ctrl.length >= 2) pts = ctrl;
      if (pts && pts.length >= 2) {
        const closed = (flags & 1) === 1 && fit.length >= 2;
        if (!closed && pts.length > 2 && g.dist(pts[0], pts[pts.length - 1]) < 1e-9 && fit.length < 2) { pts = pts.slice(0, -1); push(E.spline(pts, true), false); }
        else push(E.spline(pts, closed), false);
      }
      break;
    }
    case 'TEXT': {
      const hh = firstNum(r, 40), s = decodeText(first(r, 1, ''));
      if (hh > 0 && s.length) {
        const jh = parseInt(first(r, 72, '0'), 10), jv = parseInt(first(r, 73, '0'), 10);
        const anchor = (jh || jv) && first(r, 11) !== undefined ? pt(r, 1) : pt(r);
        push(E.text(anchor, hh, s, { rot: g.rad(firstNum(r, 50, 0)), jh: (JH[jh] || JH[0])[0], jv: jh === 4 ? 'middle' : JV[jv] || 'baseline' }));
      }
      break;
    }
    case 'MTEXT': {
      const raw = all(r, 3).join('') + first(r, 1, ''), s = decodeMText(raw), hh = firstNum(r, 40, 2.5);
      if (s.length) {
        const [jh, jv] = ATTACH[parseInt(first(r, 71, '1'), 10)] || ATTACH[1];
        const rot = first(r, 11) !== undefined ? Math.atan2(firstNum(r, 21), firstNum(r, 11)) : g.rad(firstNum(r, 50, 0));
        push(E.mtext(pt(r), hh > 0 ? hh : 2.5, s, firstNum(r, 41, 0), { rot, jh, jv }), false);
      }
      break;
    }
    case 'INSERT': {
      const name = decodeText(first(r, 2, ''));
      if (name) push(E.insert(name, pt(r), { sx: firstNum(r, 41, 1) || 1, sy: firstNum(r, 42, 1) || 1, rot: g.rad(firstNum(r, 50, 0)) }), false);
      break;
    }
    case 'XLINE': { const d = pt(r, 1); if (g.len(d) > 0) push(E.xline(pt(r), d), false); break; }
    case 'RAY': { const d = pt(r, 1); if (g.len(d) > 0) push(E.ray(pt(r), d), false); break; }
    case 'LEADER': {
      const pts = []; let cur = null;
      for (const [c, v] of r.g) { if (c === 10) { cur = { x: parseFloat(v), y: 0 }; pts.push(cur); } else if (c === 20 && cur) cur.y = parseFloat(v); }
      if (pts.length >= 2) push(E.leader(pts, '', firstNum(r, 40, 2.5) || 2.5), false);
      break;
    }
    case 'HATCH': {
      const loops = hatchLoops(r);
      if (!loops.length) break;
      const name = String(first(r, 2, 'SOLID')).toUpperCase(), solid = parseInt(first(r, 70, '0'), 10) === 1 || name === 'SOLID';
      const pattern = solid ? 'SOLID' : (PATTERNS[name] ? name : 'ANSI31');
      push(E.hatch(loops, { pattern, ps: firstNum(r, 41, 1) || 1, pa: g.rad(firstNum(r, 52, 0)) }), false);
      break;
    }
    case 'SOLID': case 'TRACE': {
      const p = [pt(r, 0), pt(r, 1), pt(r, 3), pt(r, 2)];
      const uniq = p.filter((q, i) => i === 0 || g.dist(q, p[i - 1]) > 1e-12);
      if (uniq.length >= 3) push(E.hatch([uniq.map(q => ({ x: q.x, y: q.y, b: 0 }))], { pattern: 'SOLID' }), false);
      break;
    }
    case 'DIMENSION': {
      const bname = first(r, 2), blk = ctx.blocks.get(bname);
      if (blk) {
        for (const be of blk.entities) {
          for (const e of convert(be, ctx)) {
            if (e.type === 'point') continue;                                    // definition points
            e.layer = props.layer;                                              // the dimension's layer, colour and linetype apply to ByBlock content
            if (e.color === 'BYBLOCK' || e.color === undefined) { if (props.color !== undefined) e.color = props.color; else delete e.color; }
            if (e.ltype === 'BYBLOCK') { if (props.ltype !== undefined) e.ltype = props.ltype; else delete e.ltype; }
            out.push(e);
          }
        }
      }
      break;
    }
    default: ctx.skipped[r.type] = (ctx.skipped[r.type] || 0) + 1;
  }
  return out;
}

/** Merge POLYLINE/VERTEX/SEQEND and INSERT/ATTRIB/SEQEND runs into single records. */
function groupEntities(list) {
  const out = []; let cur = null;
  for (const r of list) {
    if (r.type === 'POLYLINE') { cur = r; r.vertices = []; out.push(r); }
    else if (r.type === 'VERTEX' && cur?.type === 'POLYLINE') cur.vertices.push(r);
    else if (r.type === 'SEQEND') cur = null;
    else if (r.type === 'ATTRIB') { /* attributes are not imported */ }
    else { cur = null; out.push(r); }
  }
  return out;
}

// ---------------------------------------------------------------- entry point

export function readDXF(text, name = 'Drawing') {
  if (!/^\s*0\s*\r?\n\s*SECTION/m.test(text.slice(0, 2000)) && !/SECTION/.test(text.slice(0, 2000))) throw Error('This does not look like an ASCII DXF file.');
  const tokens = tokenize(text);
  const secs = sections(records(tokens));
  const header = parseHeader(tokens);
  const doc = new Drawing(name);

  // ---- header
  const hv = (k, code = 70, d) => { const e = header[k]?.find(([c]) => c === code); return e ? parseFloat(e[1]) : d; };
  const units = hv('$INSUNITS', 70, 0);
  doc.settings.units.insUnits = [0, 1, 2, 4, 5, 6].includes(units) ? units : 0;
  doc.settings.units.precision = Math.max(0, Math.min(8, hv('$LUPREC', 70, 4)));
  doc.settings.ltscale = hv('$LTSCALE', 40, 1) || 1;
  doc.settings.pdmode = hv('$PDMODE', 70, 0); doc.settings.pdsize = hv('$PDSIZE', 40, 0);
  const th = hv('$TEXTSIZE', 40, 2.5); if (th > 0) doc.settings.textHeight = th;
  const ds = doc.settings.dimStyles.Standard;
  for (const [k, f] of [['$DIMTXT', 'txt'], ['$DIMASZ', 'asz'], ['$DIMEXO', 'exo'], ['$DIMEXE', 'exe'], ['$DIMGAP', 'gap'], ['$DIMCEN', 'cen'], ['$DIMSCALE', 'scale']]) { const v = hv(k, 40); if (v !== undefined && v > 0) ds[f] = v; }
  const dd = hv('$DIMDEC', 70); if (dd !== undefined && dd >= 0 && dd <= 8) ds.dec = dd;

  // ---- layers
  doc.layers = [];
  for (const r of (secs.TABLES || []).filter(x => x.type === 'LAYER')) {
    const lname = decodeText(first(r, 2, '')); if (!lname || doc.layer(lname)) continue;
    const c62 = parseInt(first(r, 62, '7'), 10), tc = first(r, 420);
    const flags = parseInt(first(r, 70, '0'), 10);
    const lt = ltypeKey(first(r, 6, 'Continuous')) || 'Continuous';
    const lwv = parseInt(first(r, 370, '25'), 10);
    doc.layers.push(newLayer(lname, { color: tc !== undefined ? '#' + (parseInt(tc, 10) & 0xFFFFFF).toString(16).padStart(6, '0') : (Math.abs(c62) || 7), on: c62 >= 0, frozen: !!(flags & 1), locked: !!(flags & 4), ltype: lt, lw: lwv >= 0 ? lwv : 25, plot: first(r, 290) !== '0' }));
  }
  if (!doc.layer('0')) doc.layers.unshift(newLayer('0'));

  // ---- blocks (including anonymous dimension blocks, used for DIMENSION import)
  const ctx = { blocks: new Map(), skipped: {} };
  {
    let cur = null;
    for (const r of secs.BLOCKS || []) {
      if (r.type === 'BLOCK') { cur = { name: decodeText(first(r, 2, '')), base: pt(r), raw: [] }; }
      else if (r.type === 'ENDBLK') { if (cur) ctx.blocks.set(cur.name, cur); cur = null; } else if (cur) cur.raw.push(r);
    }
    for (const b of ctx.blocks.values()) b.entities = groupEntities(b.raw);
  }
  const userBlocks = [...ctx.blocks.values()].filter(b => !b.name.startsWith('*'));
  for (const b of userBlocks) {
    const ents = [];
    for (const r of b.entities) for (const e of convert(r, ctx)) ents.push(e);
    doc.defineBlock(b.name, b.base, ents);
  }

  // ---- model space
  let paper = 0;
  for (const r of groupEntities(secs.ENTITIES || [])) {
    if (first(r, 67) === '1') { paper++; continue; }
    for (const e of convert(r, ctx)) doc.add(e, 'model');
  }

  // layers referenced by entities but missing from the table
  for (const e of doc.allEntities()) if (e.layer && !doc.layer(e.layer)) doc.layers.push(newLayer(e.layer));
  const cl = decodeText(header.$CLAYER?.find(([c]) => c === 8)?.[1] || '0');
  doc.cur.layer = doc.layer(cl)?.name || doc.layers[0].name;

  doc.skipped = ctx.skipped; doc.paperSkipped = paper;
  doc.validate();
  doc.dirty = false;
  return doc;
}

