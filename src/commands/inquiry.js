// Inquiry commands: DIST, AREA, LIST, ID, MEASUREGEOM.
import * as g from '../core/geom.js';
import { E, segsOf, labelOf, ellipseSweep } from '../core/entities.js';
import { registry } from '../engine/registry.js';
import { fmtNum, describeColor, lwDisplay } from '../core/style.js';
import { num } from './helpers.js';

const def = d => registry.define({ group: 'Inquiry', pickfirst: false, ...d });
const prec = c => c.doc.settings.units.precision;
const f = (c, v) => fmtNum(v, prec(c));
const angText = (c, a) => fmtNum(g.deg(g.normAngle(a)), c.doc.settings.units.angPrecision === 0 ? 4 : c.doc.settings.units.angPrecision);

def({
  name: 'DIST', aliases: ['DI'], title: 'Distance', icon: 'dist', desc: 'Measures the distance and angle between two points.',
  async run(c) {
    const a = await c.point({ prompt: 'Specify first point' }); if (!a.pt) return;
    const b = await c.point({ prompt: 'Specify second point or [Multiple points]', options: ['Multiple points'], base: a.pt, preview: pt => [E.line(a.pt, pt)] }); if (!b.pt) return;
    const d = g.sub(b.pt, a.pt);
    c.log(`Distance = ${f(c, g.len(d))},  Angle in XY Plane = ${angText(c, Math.atan2(d.y, d.x))},  Angle from XY Plane = 0`);
    c.log(`  Delta X = ${f(c, d.x)},  Delta Y = ${f(c, d.y)},  Delta Z = ${f(c, 0)}`);
    c.app.mem ??= {}; c.app.mem.lastDistance = g.len(d);
  },
});

/** Area and perimeter of a closed entity, or null. */
export function entityArea(e, doc) {
  switch (e.type) {
    case 'circle': return { area: Math.PI * e.r * e.r, perimeter: 2 * Math.PI * e.r };
    case 'ellipse': { if (ellipseSweep(e) < g.TAU - 1e-9) return null; const a = g.len(e.m), b = a * e.ratio; const h = ((a - b) / (a + b)) ** 2; return { area: Math.PI * a * b, perimeter: Math.PI * (a + b) * (1 + 3 * h / (10 + Math.sqrt(4 - 3 * h))) }; }
    case 'pline': return { area: Math.abs(g.plineArea(e.v)), perimeter: g.plineLength(e.v, e.closed) };
    case 'spline': { const pts = segsOf(e, doc).map(s => s.a); return { area: Math.abs(g.polygonArea(pts)), perimeter: segsOf(e, doc).reduce((s, q) => s + g.segLength(q), 0) }; }
    case 'hatch': { let a = 0, p = 0; e.loops.forEach((l, i) => { a += (i === 0 ? 1 : -1) * Math.abs(g.plineArea(l)); p += g.plineLength(l, true); }); return { area: a, perimeter: p }; }
    default: return null;
  }
}

def({
  name: 'AREA', aliases: ['AA'], title: 'Area', icon: 'area', desc: 'Calculates the area and perimeter of objects or of defined areas.',
  async run(c) {
    let total = 0;
    const r0 = await c.point({ prompt: 'Specify first corner point or', options: ['Object', 'Add area', 'Subtract area', 'eXit'] });
    if (r0.kw === 'Object' || r0.kw === 'Add area' || r0.kw === 'Subtract area') {
      let sign = r0.kw === 'Subtract area' ? -1 : 1;
      for (;;) {
        const o = await c.entity({ prompt: 'Select objects', allowEnter: true, filter: e => !!entityArea(e, c.doc) }); if (!o.entity) break;
        const r = entityArea(o.entity, c.doc);
        total += sign * r.area;
        c.log(`Area = ${f(c, r.area)},  ${o.entity.type === 'circle' ? 'Circumference' : 'Perimeter'} = ${f(c, r.perimeter)}`);
        if (r0.kw !== 'Object') c.log(`Total area = ${f(c, total)}`);
      }
      return;
    }
    if (!r0.pt) return;
    const pts = [r0.pt];
    for (;;) {
      const r = await c.point({ prompt: 'Specify next corner point or press ENTER for total', base: pts[pts.length - 1], allowEnter: true, preview: pt => [E.pline([...pts, pt], true)] });
      if (!r.pt) break; pts.push(r.pt);
    }
    if (pts.length < 3) { c.log('At least three points are required.', 'error'); return; }
    const area = Math.abs(g.polygonArea(pts)), per = g.plineLength(pts.map(p => ({ ...p, b: 0 })), true);
    c.log(`Area = ${f(c, area)},  Perimeter = ${f(c, per)}`);
  },
});

const describe = (c, e) => {
  const d = c.doc, out = [];
  out.push(`${labelOf(e).toUpperCase().padEnd(14)} Layer: "${e.layer}"`);
  out.push(`                Space: ${d.isModel ? 'Model space' : 'Paper space'}`);
  out.push(`                Handle = ${e.id.toString(16).toUpperCase()}`);
  if (e.color !== undefined) out.push(`                Color: ${describeColor(e.color)}`);
  if (e.ltype !== undefined) out.push(`                Linetype: "${e.ltype}"`);
  if (e.lw !== undefined) out.push(`                Lineweight: ${lwDisplay(e.lw)}`);
  const P = p => `X=${f(c, p.x).padStart(10)}  Y=${f(c, p.y).padStart(10)}  Z=${f(c, 0).padStart(10)}`;
  switch (e.type) {
    case 'line': out.push(`      from point, ${P(e.a)}`, `        to point, ${P(e.b)}`, `  Length =  ${f(c, g.dist(e.a, e.b))},  Angle in XY Plane =  ${angText(c, g.angleOf(e.a, e.b))}`, `       Delta X =  ${f(c, e.b.x - e.a.x)},  Delta Y =  ${f(c, e.b.y - e.a.y)},  Delta Z =  ${f(c, 0)}`); break;
    case 'circle': out.push(`       center point, ${P(e.c)}`, `  radius  ${f(c, e.r)}`, `  circumference  ${f(c, 2 * Math.PI * e.r)}`, `  area  ${f(c, Math.PI * e.r * e.r)}`); break;
    case 'arc': { const sw = g.sweepCCW(e.a0, e.a1) || g.TAU; out.push(`       center point, ${P(e.c)}`, `  radius  ${f(c, e.r)}`, `  start angle  ${angText(c, e.a0)}`, `  end angle  ${angText(c, e.a1)}`, `  length  ${f(c, e.r * sw)}`); break; }
    case 'pline': out.push(`  ${e.closed ? 'Closed' : 'Open'}`, `  Area  ${f(c, Math.abs(g.plineArea(e.v)))}`, `  Perimeter  ${f(c, g.plineLength(e.v, e.closed))}`, ...e.v.map(v => `  at point  ${P(v)}${v.b ? `  bulge ${num(v.b)}` : ''}`)); break;
    case 'ellipse': out.push(`       center point, ${P(e.c)}`, `  major axis  ${f(c, g.len(e.m) * 2)}`, `  minor axis  ${f(c, g.len(e.m) * 2 * e.ratio)}`); break;
    case 'text': out.push(`  Style = "Standard"`, `  Annotative: No`, `  Height  ${f(c, e.h)}`, `  Text  ${e.s}`, `  insertion point, ${P(e.p)}`); break;
    case 'insert': out.push(`  "${e.name}"`, `  at point, ${P(e.p)}`, `  X scale factor  ${f(c, e.sx)}`, `  Y scale factor  ${f(c, e.sy)}`, `  rotation angle  ${angText(c, e.rot || 0)}`); break;
    case 'point': out.push(`  at point, ${P(e.p)}`); break;
    case 'hatch': { const a = entityArea(e, d); out.push(`  Pattern name  ${e.pattern}`, `  Scale  ${f(c, e.ps)}`, `  Angle  ${angText(c, e.pa || 0)}`, `  Area  ${f(c, a.area)}`); break; }
    case 'dim': out.push(`  Dimension type ${e.kind}`); break;
    default: break;
  }
  return out.join('\n');
};

def({
  name: 'LIST', aliases: ['LI', 'LS'], title: 'List', icon: 'list', desc: 'Displays property data for selected objects.', pickfirst: true,
  async run(c) {
    const sel = await c.select({ prompt: 'Select objects' });
    if (!sel.length) return;
    for (const e of sel) c.log(describe(c, e));
    c.app.emit('showHistory');
    c.clearSel();
  },
});

def({
  name: 'ID', title: 'ID Point', icon: 'idpoint', desc: 'Displays the UCS coordinate values of a specified location.',
  async run(c) {
    const r = await c.point({ prompt: 'Specify point' }); if (!r.pt) return;
    c.log(`X = ${f(c, r.pt.x)}  Y = ${f(c, r.pt.y)}  Z = ${f(c, 0)}`);
    c.app.runtime.lastPoint = r.pt;
  },
});

def({
  name: 'MEASUREGEOM', aliases: ['MEA'], title: 'Measure', icon: 'dist', desc: 'Measures distances, radii, angles, areas, and volumes of selected objects.',
  async run(c) {
    const k = await c.keyword({ prompt: 'Enter an option [Distance/Radius/Angle/ARea/Volume/Quick]', options: ['Distance', 'Radius', 'Angle', 'ARea', 'Volume', 'Quick'], default: 'Distance' });
    if (k.kw === 'Radius') {
      const o = await c.pickEntity('Select arc or circle', e => e.type === 'circle' || e.type === 'arc'); if (!o) return;
      c.log(`Radius = ${f(c, o.r)}`); c.log(`Diameter = ${f(c, o.r * 2)}`);
    } else if (k.kw === 'ARea') return c.rt.run('AREA', { transparent: true });
    else if (k.kw === 'Angle') {
      const a = await c.entity({ prompt: 'Select arc, circle, line, or <specify vertex>', filter: e => e.type === 'line' || e.type === 'arc' }); if (!a.entity) return;
      if (a.entity.type === 'arc') { c.log(`Angle = ${angText(c, g.sweepCCW(a.entity.a0, a.entity.a1))}`); return; }
      const b = await c.pickEntity('Select second line', e => e.type === 'line'); if (!b) return;
      const d1 = g.sub(a.entity.b, a.entity.a), d2 = g.sub(b.b, b.a);
      c.log(`Angle = ${angText(c, Math.acos(Math.min(1, Math.abs(g.dot(g.unit(d1), g.unit(d2))))))}`);
    } else if (k.kw === 'Volume') {
      const s = c.doc.solids; if (!s.length) { c.log('No solids in the drawing.', 'error'); return; }
      c.log(`Volume = ${f(c, s.reduce((v, m) => v + meshVolume(m), 0))}`);
    } else return c.rt.run('DIST', { transparent: true });
  },
});

export function meshVolume(m) {
  let v = 0;
  for (const fc of m.faces) { const [a, b, cc] = fc.map(i => m.vertices[i]); v += (a.x * (b.y * cc.z - b.z * cc.y) - a.y * (b.x * cc.z - b.z * cc.x) + a.z * (b.x * cc.y - b.y * cc.x)) / 6; }
  return Math.abs(v);
}
