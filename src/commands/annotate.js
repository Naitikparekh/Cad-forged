// Annotation commands: TEXT, MTEXT, TEXTEDIT, dimensions, LEADER, hatch commands and BOUNDARY.
import * as g from '../core/geom.js';
import { E, segsOf } from '../core/entities.js';
import { registry } from '../engine/registry.js';
import { Cancel } from '../engine/runtime.js';
import { findRegion } from '../ops/boundary.js';
import { memory } from './helpers.js';

const def = d => registry.define({ group: 'Annotation', pickfirst: false, ...d });

// ---------------------------------------------------------------- text input helper

/** In-place text editor when the UI provides one, otherwise a command-line prompt. */
export async function textInput(c, spec) {
  if (c.app.ui?.editText) {
    const r = await c.app.ui.editText(spec);
    if (r === null || r === undefined) throw new Cancel();
    return r;
  }
  const r = await c.string({ prompt: spec.prompt || 'Enter text', raw: true, allowEmpty: true });
  return r.value ?? '';
}

const JUST = {
  Left: ['left', 'baseline'], Center: ['center', 'baseline'], Right: ['right', 'baseline'], Middle: ['center', 'middle'],
  TL: ['left', 'top'], TC: ['center', 'top'], TR: ['right', 'top'], ML: ['left', 'middle'], MC: ['center', 'middle'], MR: ['right', 'middle'],
  BL: ['left', 'bottom'], BC: ['center', 'bottom'], BR: ['right', 'bottom'],
};

// ---------------------------------------------------------------- TEXT / MTEXT

def({
  name: 'TEXT', aliases: ['DT', 'DTEXT'], title: 'Single Line Text', icon: 'text', desc: 'Creates a single-line text object.',
  async run(c) {
    const mem = memory(c.app);
    let jh = 'left', jv = 'baseline';
    let r;
    for (;;) {
      r = await c.point({ prompt: 'Specify start point of text', options: ['Justify', 'Style'] });
      if (r.kw === 'Justify') { const j = await c.keyword({ prompt: 'Enter an option', options: Object.keys(JUST).map(k => k) , default: 'Left' }); [jh, jv] = JUST[j.kw] || JUST.Left; continue; }
      if (r.kw === 'Style') { c.log('Text style: Standard'); continue; }
      break;
    }
    if (!r.pt) return;
    let p = r.pt;
    const hh = await c.distance({ prompt: 'Specify height', base: p, default: mem.textHeight, min: 0, nonzero: true });
    if (hh.value === undefined) return;
    const h = hh.value; mem.textHeight = h; c.doc.settings.textHeight = h;
    const rr = await c.angle({ prompt: 'Specify rotation angle of text', base: p, default: 0 });
    if (rr.value === undefined) return;
    const rot = rr.value;
    for (;;) {
      const s = await textInput(c, { prompt: 'Enter text', p, h, rot, jh, jv, multi: false });
      if (!s) break;
      c.add(E.text(p, h, s, { rot, jh, jv }));
      p = g.add(p, g.polar({ x: 0, y: 0 }, rot - Math.PI / 2, h * 1.5));
      if (!c.app.ui?.editText) { /* command-line mode continues until an empty line */ } else break;
    }
  },
});

def({
  name: 'MTEXT', aliases: ['T', 'MT'], title: 'Multiline Text', icon: 'mtext', desc: 'Creates a multiline text object.',
  async run(c, args) {
    const mem = memory(c.app);
    let h = mem.textHeight, rot = 0, jh = 'left', jv = 'top';
    const a = await c.point({ prompt: 'Specify first corner' }); if (!a.pt) return;
    let b;
    for (;;) {
      b = await c.corner({ prompt: 'Specify opposite corner', options: ['Height', 'Justify', 'Rotation', 'Width'], base: a.pt });
      if (b.kw === 'Height') { const v = await c.distance({ prompt: 'Specify height', default: h, min: 0, nonzero: true }); if (v.value) { h = v.value; mem.textHeight = h; } continue; }
      if (b.kw === 'Justify') { const j = await c.keyword({ prompt: 'Enter justification', options: ['TL', 'TC', 'TR', 'ML', 'MC', 'MR', 'BL', 'BC', 'BR'], default: 'TL' }); [jh, jv] = JUST[j.kw] || JUST.TL; continue; }
      if (b.kw === 'Rotation') { const v = await c.angle({ prompt: 'Specify rotation angle', default: 0 }); if (v.value !== undefined) rot = v.value; continue; }
      if (b.kw === 'Width') { const v = await c.distance({ prompt: 'Specify width', min: 0, nonzero: true }); if (v.value) { b = { pt: g.add(a.pt, { x: v.value, y: -h * 3 }) }; break; } continue; }
      break;
    }
    if (!b.pt) return;
    const width = Math.abs(b.pt.x - a.pt.x), p = { x: Math.min(a.pt.x, b.pt.x), y: Math.max(a.pt.y, b.pt.y) };
    const s = await textInput(c, { prompt: 'MText', p, h, rot, jh, jv, multi: true, w: width, initial: args?.initial });
    if (!s) return;
    c.add(E.mtext(p, h, s, width, { rot, jh, jv }));
  },
});

def({
  name: 'TEXTEDIT', aliases: ['ED'], title: 'Edit Text', icon: 'textedit', desc: 'Edits a dimensional constraint, dimension text, or text object.',
  async run(c, args) {
    for (;;) {
      let e = args.entity;
      args = {};
      if (!e) e = (await c.entity({ prompt: 'Select an annotation object or [Undo]', options: ['Undo'], allowEnter: true, filter: x => ['text', 'dim', 'leader'].includes(x.type) })).entity;
      if (!e) return;
      c.editor.tempSel = new Set([e.id]);
      const initial = e.type === 'text' ? e.s : e.type === 'dim' ? (e.txt ?? '<>') : e.s;
      const s = await textInput(c, { prompt: 'Enter new text', p: e.p || e.pts?.[0], h: e.h, rot: e.rot || 0, multi: !!e.multi, w: e.w, initial, editing: e });
      c.editor.tempSel = new Set();
      c.doc.change(() => { if (e.type === 'dim') e.txt = s === '<>' ? '' : s; else e.s = s; });
      return;
    }
  },
});

// ---------------------------------------------------------------- dimensions

const dimDefaults = c => ({ ds: c.doc.settings.curDimStyle });
const hOrV = (p1, p2, dp) => { const m = g.mid(p1, p2); return Math.abs(dp.y - m.y) >= Math.abs(dp.x - m.x) ? 0 : Math.PI / 2; };

async function dimText(c, e) {
  const t = await c.string({ prompt: 'Enter dimension text <>', raw: true, default: '<>' });
  if (t.value !== undefined && t.value !== '<>') e.txt = t.value;
}

async function linearLike(c, kind, args = {}) {
  let p1, p2;
  const first = await c.point({ prompt: 'Specify first extension line origin or <select object>', allowEnter: true });
  if (first.enter && !first.pt) {
    const o = await c.entity({ prompt: 'Select object to dimension', filter: e => ['line', 'arc', 'circle', 'pline'].includes(e.type) }); if (!o.entity) return;
    const segs = segsOf(o.entity, c.doc);
    let best = null, bd = Infinity; for (const s of segs) { const n = g.segNearest(s, o.pt); if (n.d < bd) { bd = n.d; best = s; } }
    if (o.entity.type === 'circle') { p1 = g.add(o.entity.c, { x: -o.entity.r, y: 0 }); p2 = g.add(o.entity.c, { x: o.entity.r, y: 0 }); }
    else if (best) { p1 = g.segStart(best); p2 = g.segEnd(best); }
  } else if (first.pt) {
    p1 = first.pt;
    const second = await c.point({ prompt: 'Specify second extension line origin', base: p1 }); if (!second.pt) return; p2 = second.pt;
  } else return;
  let fixed = kind === 'ali' ? null : args.angle ?? null, override = null;
  const make = dp => { const e = E.dim(kind, { p1, p2, dp, ...dimDefaults(c) }); if (kind === 'lin') e.ang = fixed ?? hOrV(p1, p2, dp); if (override !== null) e.txt = override; return e; };
  for (;;) {
    const opts = kind === 'lin' ? ['Mtext', 'Text', 'Angle', 'Horizontal', 'Vertical', 'Rotated'] : ['Mtext', 'Text', 'Angle'];
    const r = await c.point({ prompt: 'Specify dimension line location', options: opts, base: g.mid(p1, p2), noOrtho: true, preview: pt => [make(pt)] });
    if (r.kw === 'Mtext' || r.kw === 'Text') { const t = await c.string({ prompt: 'Enter dimension text <>', raw: true, default: '<>' }); if (t.value !== undefined && t.value !== '<>') override = t.value; continue; }
    if (r.kw === 'Horizontal') { fixed = 0; continue; }
    if (r.kw === 'Vertical') { fixed = Math.PI / 2; continue; }
    if (r.kw === 'Rotated') { const a = await c.angle({ prompt: 'Specify angle of dimension line', default: 0 }); if (a.value !== undefined) fixed = a.value; continue; }
    if (r.kw === 'Angle') { c.log('Text angle is determined automatically.'); continue; }
    if (!r.pt) return;
    const e = c.add(make(r.pt));
    c.app.mem ??= {}; c.app.mem.lastDim = e;
    return;
  }
}

def({ name: 'DIMLINEAR', aliases: ['DLI'], title: 'Linear', icon: 'dimlinear', desc: 'Creates a linear dimension.', run: c => linearLike(c, 'lin') });
def({ name: 'DIMALIGNED', aliases: ['DAL'], title: 'Aligned', icon: 'dimaligned', desc: 'Creates an aligned linear dimension.', run: c => linearLike(c, 'ali') });

const radial = kind => async function run(c) {
  const o = await c.entity({ prompt: kind === 'rad' ? 'Select arc or circle' : 'Select arc or circle', filter: e => e.type === 'circle' || e.type === 'arc' }); if (!o.entity) return;
  const e0 = o.entity, ctr = e0.c, dir = g.unit(g.sub(o.pt, ctr)), pt = g.add(ctr, g.mul(dir, e0.r));
  const make = dp => E.dim(kind, { c: ctr, pt: kind === 'dia' ? pt : pt, dp, ...dimDefaults(c) });
  const r = await c.point({ prompt: 'Specify dimension line location', options: ['Mtext', 'Text', 'Angle'], base: ctr, noOrtho: true, preview: p => [make(p)] });
  if (r.kw === 'Mtext' || r.kw === 'Text') { const e = make(g.polar(ctr, g.angleOf(ctr, o.pt), e0.r * 1.5)); await dimText(c, e); c.add(e); return; }
  if (!r.pt) return;
  c.add(make(r.pt));
};
def({ name: 'DIMRADIUS', aliases: ['DRA'], title: 'Radius', icon: 'dimradius', desc: 'Creates a radius dimension for a circle or an arc.', run: radial('rad') });
def({ name: 'DIMDIAMETER', aliases: ['DDI'], title: 'Diameter', icon: 'dimdiameter', desc: 'Creates a diameter dimension for a circle or an arc.', run: radial('dia') });

def({
  name: 'DIMANGULAR', aliases: ['DAN'], title: 'Angular', icon: 'dimangular', desc: 'Creates an angular dimension.',
  async run(c) {
    const o = await c.entity({ prompt: 'Select arc, circle, line, or <specify vertex>', allowEnter: true, filter: e => ['line', 'arc', 'circle'].includes(e.type) });
    let ctr, p1, p2;
    if (o.entity?.type === 'line') {
      const o2 = await c.entity({ prompt: 'Select second line', filter: e => e.type === 'line' }); if (!o2.entity) return;
      const a = segsOf(o.entity, c.doc)[0], b = segsOf(o2.entity, c.doc)[0];
      const hit = g.segIntersect(a, b, true, true)[0];
      if (!hit) { c.log('The lines are parallel.', 'error'); return; }
      ctr = hit.pt;
      const far = (s, pick) => g.dist(g.segStart(s), pick) > g.dist(g.segEnd(s), pick) ? g.segStart(s) : g.segEnd(s);
      p1 = far(a, o.pt); p2 = far(b, o2.pt);
      p1 = g.dist(g.segStart(a), ctr) > g.dist(g.segEnd(a), ctr) ? g.segStart(a) : g.segEnd(a); p2 = g.dist(g.segStart(b), ctr) > g.dist(g.segEnd(b), ctr) ? g.segStart(b) : g.segEnd(b);
    } else if (o.entity?.type === 'arc') {
      ctr = o.entity.c; p1 = g.polar(ctr, o.entity.a0, o.entity.r); p2 = g.polar(ctr, o.entity.a1, o.entity.r);
    } else if (o.entity?.type === 'circle') {
      ctr = o.entity.c; p1 = o.pt; const s = await c.point({ prompt: 'Specify second angle endpoint', base: ctr }); if (!s.pt) return; p2 = s.pt;
    } else {
      const v = await c.point({ prompt: 'Specify angle vertex' }); if (!v.pt) return; ctr = v.pt;
      const a = await c.point({ prompt: 'Specify first angle endpoint', base: ctr }); if (!a.pt) return; p1 = a.pt;
      const b = await c.point({ prompt: 'Specify second angle endpoint', base: ctr }); if (!b.pt) return; p2 = b.pt;
    }
    const make = dp => E.dim('ang', { c: ctr, p1, p2, dp, ...dimDefaults(c) });
    const r = await c.point({ prompt: 'Specify dimension arc line location', options: ['Mtext', 'Text'], base: ctr, noOrtho: true, preview: p => [make(p)] });
    if (!r.pt) return;
    c.add(make(r.pt));
  },
});

def({
  name: 'DIMCONTINUE', aliases: ['DCO'], title: 'Continue', icon: 'dimcontinue', desc: 'Creates a dimension that starts from an extension line of a previously created dimension.',
  async run(c) {
    let base = c.app.mem?.lastDim;
    if (!base || !c.doc.entities.includes(base)) { const o = await c.entity({ prompt: 'Select continued dimension', filter: e => e.type === 'dim' && (e.kind === 'lin' || e.kind === 'ali') }); if (!o.entity) return; base = o.entity; }
    let prev = base.p2;
    for (;;) {
      const make = pt => E.dim(base.kind, { p1: prev, p2: pt, dp: base.dp, ang: base.ang, ds: base.ds });
      const r = await c.point({ prompt: 'Specify a second extension line origin or [Undo/Select]', options: ['Undo', 'Select'], allowEnter: true, base: prev, preview: pt => [make(pt)] });
      if (!r.pt) return;
      const e = c.add(make(r.pt)); prev = r.pt; c.app.mem.lastDim = e;
    }
  },
});
def({
  name: 'DIMBASELINE', aliases: ['DBA'], title: 'Baseline', icon: 'dimbaseline', desc: 'Creates a linear, angular, or ordinate dimension from the baseline of the previous or selected dimension.',
  async run(c) {
    let base = c.app.mem?.lastDim;
    if (!base || !c.doc.entities.includes(base)) { const o = await c.entity({ prompt: 'Select base dimension', filter: e => e.type === 'dim' && (e.kind === 'lin' || e.kind === 'ali') }); if (!o.entity) return; base = o.entity; }
    const st = c.doc.settings.dimStyles[base.ds] || c.doc.settings.dimStyles.Standard, space = 3.75 * st.scale;
    const ang = base.kind === 'ali' ? g.angleOf(base.p1, base.p2) : base.ang || 0, n = { x: -Math.sin(ang), y: Math.cos(ang) };
    const side = g.dot(g.sub(base.dp, base.p1), n) >= 0 ? 1 : -1;
    let dp = base.dp;
    for (;;) {
      const make = pt => E.dim(base.kind, { p1: base.p1, p2: pt, dp: g.add(dp, g.mul(n, side * space)), ang: base.ang, ds: base.ds });
      const r = await c.point({ prompt: 'Specify a second extension line origin or [Undo/Select]', options: ['Undo', 'Select'], allowEnter: true, base: base.p1, preview: pt => [make(pt)] });
      if (!r.pt) return;
      const e = c.add(make(r.pt)); dp = e.dp;
    }
  },
});

def({
  name: 'DIM', title: 'Dimension', icon: 'dim', desc: 'Creates multiple dimension types with a single command.',
  async run(c) {
    const o = await c.entity({ prompt: 'Select object or specify first extension line origin or', options: ['Angular', 'Baseline', 'Continue', 'aLign'], allowEnter: true, filter: e => ['line', 'arc', 'circle', 'pline'].includes(e.type) });
    if (o.kw === 'Angular') return c.rt.run('DIMANGULAR', { transparent: true });
    if (o.kw === 'Baseline') return c.rt.run('DIMBASELINE', { transparent: true });
    if (o.kw === 'Continue') return c.rt.run('DIMCONTINUE', { transparent: true });
    if (o.kw === 'aLign') return linearLike(c, 'ali');
    if (!o.entity) return;
    const e0 = o.entity;
    if (e0.type === 'circle' || e0.type === 'arc') {
      const ctr = e0.c, dir = g.unit(g.sub(o.pt, ctr)), pt = g.add(ctr, g.mul(dir, e0.r));
      const make = dp => E.dim('rad', { c: ctr, pt, dp, ...dimDefaults(c) });
      const r = await c.point({ prompt: 'Specify dimension line location or [Mtext/Text/Angle/Diameter/Radius]', options: ['Diameter', 'Radius'], base: ctr, noOrtho: true, preview: p => [make(p)] });
      if (r.kw === 'Diameter') { const r2 = await c.point({ prompt: 'Specify dimension line location', base: ctr, noOrtho: true }); if (r2.pt) c.add(E.dim('dia', { c: ctr, pt, dp: r2.pt, ...dimDefaults(c) })); return; }
      if (r.pt) c.add(make(r.pt));
      return;
    }
    const s = segsOf(e0, c.doc).map(q => ({ q, d: g.segNearest(q, o.pt).d })).sort((a, b) => a.d - b.d)[0].q;
    const p1 = g.segStart(s), p2 = g.segEnd(s);
    const kind = Math.abs(p1.x - p2.x) < 1e-9 || Math.abs(p1.y - p2.y) < 1e-9 ? 'lin' : 'ali';
    const make = dp => { const e = E.dim(kind, { p1, p2, dp, ...dimDefaults(c) }); if (kind === 'lin') e.ang = hOrV(p1, p2, dp); return e; };
    const r = await c.point({ prompt: 'Specify dimension line location', base: g.mid(p1, p2), noOrtho: true, preview: p => [make(p)] });
    if (r.pt) { const e = c.add(make(r.pt)); c.app.mem ??= {}; c.app.mem.lastDim = e; }
  },
});

// ---------------------------------------------------------------- LEADER

def({
  name: 'MLEADER', aliases: ['MLD', 'LE', 'LEADER', 'QLEADER'], title: 'Multileader', icon: 'mleader', desc: 'Creates a leader line with annotation text.',
  async run(c) {
    const mem = memory(c.app);
    const a = await c.point({ prompt: 'Specify leader arrowhead location' }); if (!a.pt) return;
    const pts = [a.pt];
    const r = await c.point({ prompt: 'Specify leader landing location', base: a.pt, noOrtho: false, preview: pt => [E.leader([a.pt, pt], '', mem.textHeight)] }); if (!r.pt) return;
    pts.push(r.pt);
    const s = await textInput(c, { prompt: 'Enter text', p: r.pt, h: mem.textHeight, rot: 0, multi: false, jh: r.pt.x >= a.pt.x ? 'left' : 'right', jv: 'middle' });
    c.add(E.leader(pts, s || '', mem.textHeight));
  },
});

// ---------------------------------------------------------------- HATCH

const hatchEntity = (loops, h) => E.hatch(loops, { pattern: h.pattern, ps: h.scale, pa: g.rad(h.angle), ...(h.color !== undefined ? { color: h.color } : {}) });

def({
  name: 'HATCH', aliases: ['H', 'BHATCH'], title: 'Hatch', icon: 'hatch', desc: 'Fills an enclosed area or selected objects with a hatch pattern.',
  async run(c, args) {
    const mem = memory(c.app), h = mem.hatch;
    const loops = [];
    c.app.hatchSession = { settings: h, active: true, count: () => loops.length };
    c.app.emit('hatchSession', c.app.hatchSession);
    const preview = () => loops.length ? [hatchEntity(loops, h)] : [];
    c.setPreview(preview);
    try {
      if (args.pattern) h.pattern = args.pattern;
      for (;;) {
        const r = await c.point({ prompt: 'Pick internal point or', options: ['Select objects', 'seTtings', 'Undo'], allowEnter: true, preview, noOrtho: true, noSnap: false });
        if (r.kw === 'seTtings') { await c.app.ui?.dialog?.('hatch', h); continue; }
        if (r.kw === 'Undo') { loops.length = 0; continue; }
        if (r.kw === 'Select objects') {
          const sel = await c.select({ prompt: 'Select objects', filter: e => ['circle', 'pline', 'ellipse'].includes(e.type) || e.type === 'line' });
          for (const e of sel) { const l = loopFromEntity(e); if (l) loops.push(l); }
          continue;
        }
        if (!r.pt) break;
        const reg = findRegion(c.doc, r.pt);
        if (reg.error) { c.log(reg.error, 'error'); continue; }
        loops.push(...reg.loops);
        c.log(`${reg.loops.length} loop${reg.loops.length === 1 ? '' : 's'} extracted`);
        c.setPreview(preview);
      }
      if (loops.length) { const e = hatchEntity(loops, mem.hatch); c.add(e); c.log('Hatch created.'); }
    } finally {
      c.app.hatchSession = null; c.app.emit('hatchSession', null);
    }
  },
});

function loopFromEntity(e) {
  if (e.type === 'circle') return [{ x: e.c.x + e.r, y: e.c.y, b: 1 }, { x: e.c.x - e.r, y: e.c.y, b: 1 }];
  if (e.type === 'pline' && e.closed) return e.v.map(v => ({ ...v }));
  return null;
}

def({
  name: 'HATCHEDIT', aliases: ['HE'], title: 'Edit Hatch', icon: 'hatchedit', desc: 'Modifies an existing hatch or fill.',
  async run(c, args) {
    let e = args.entity || (await c.entity({ prompt: 'Select hatch object', filter: x => x.type === 'hatch' })).entity;
    if (!e) return;
    const live = { pattern: e.pattern, scale: e.ps, angle: g.deg(e.pa || 0), color: e.color };
    c.app.hatchSession = { settings: live, editing: e, active: true };
    c.app.emit('hatchSession', c.app.hatchSession);
    try {
      await c.app.ui?.hatchEditSession?.(c, e, live);
    } finally {
      c.doc.change(() => { e.pattern = live.pattern; e.ps = live.scale; e.pa = g.rad(live.angle); });
      c.app.hatchSession = null; c.app.emit('hatchSession', null);
    }
  },
});

def({
  name: 'BOUNDARY', aliases: ['BO'], title: 'Boundary', icon: 'boundary', desc: 'Creates a region or a polyline from an area enclosed by objects.',
  async run(c) {
    for (;;) {
      const r = await c.point({ prompt: 'Pick internal point', options: ['Advanced options'], allowEnter: true });
      if (!r.pt) return;
      const reg = findRegion(c.doc, r.pt);
      if (reg.error) { c.log(reg.error, 'error'); continue; }
      for (const l of reg.loops) c.add(E.pline(l, true));
      c.log(`BOUNDARY created ${reg.loops.length} polyline${reg.loops.length === 1 ? '' : 's'}`);
    }
  },
});

