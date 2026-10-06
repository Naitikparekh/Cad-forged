// DXF writer (AutoCAD R2000 / AC1015). DOM-free. Output is validated against ezdxf in tests/dxf.
import * as g from '../core/geom.js';
import { boundsOf, splineBeziers, ellipseSweep } from '../core/entities.js';
import { dimGeometry, arrowPolygon, arrowLines } from '../core/dim.js';
import { PATTERNS } from '../core/hatch.js';
import { LINETYPES, hexToAci, LW_BYLAYER } from '../core/style.js';

const num = v => {
  if (!Number.isFinite(v)) return '0';
  if (Number.isInteger(v)) return String(v);
  const s = String(+v.toPrecision(15));
  return /e/i.test(s) ? v.toFixed(12).replace(/0+$/, '').replace(/\.$/, '') : s;
};
const clean = s => String(s).replace(/[\r\n]+/g, ' ');
/** R2000 files are code-page encoded: write anything outside ASCII as \U+XXXX (and the usual %% symbols). */
const SYMBOLS = { '\u2300': '%%c', '\u00B0': '%%d', '\u00B1': '%%p' };
const enc = s => String(s).replace(/[^\x20-\x7E\r\n\t]/gu, ch => SYMBOLS[ch] || (ch.codePointAt(0) > 0xFFFF ? '?' : `\\U+${ch.charCodeAt(0).toString(16).toUpperCase().padStart(4, '0')}`));
const JH = { left: 0, center: 1, right: 2 }, JV = { baseline: 0, bottom: 1, middle: 2, top: 3 };
const ATTACH = { 'left,top': 1, 'center,top': 2, 'right,top': 3, 'left,middle': 4, 'center,middle': 5, 'right,middle': 6, 'left,bottom': 7, 'center,bottom': 8, 'right,bottom': 9, 'left,baseline': 7, 'center,baseline': 8, 'right,baseline': 9 };
const INSUNITS_OK = new Set([0, 1, 2, 4, 5, 6]);

class Out {
  constructor() { this.lines = []; }
  c(code, val) { this.lines.push(String(code), typeof val === 'number' ? num(val) : enc(val)); return this; }
  pt(p, i = 0, z = 0) { return this.c(10 + i, p.x).c(20 + i, p.y).c(30 + i, z); }
  toString() { return this.lines.join('\r\n') + '\r\n'; }
}

export function writeDXF(doc) {
  let handle = 0x10;
  const H = () => (handle++).toString(16).toUpperCase();
  const o = new Out();

  // ---------- collect names
  const layers = doc.layers.map(l => l.name);
  const usedLtypes = new Set(['Continuous']);
  for (const l of doc.layers) usedLtypes.add(l.ltype || 'Continuous');
  for (const e of doc.allEntities()) if (e.ltype && e.ltype !== 'BYBLOCK') usedLtypes.add(e.ltype);
  const ltypeNames = [...usedLtypes].filter(n => LINETYPES[n]);
  const userBlocks = Object.values(doc.blocks);

  // handles of fixed table objects
  const hs = { vport: H(), ltype: H(), layer: H(), style: H(), view: H(), ucs: H(), appid: H(), dimstyle: H(), blockRecord: H(), dictRoot: H(), dictGroup: H(), vportRec: H(), styleRec: H(), appidRec: H(), dimstyleRec: H(), msRec: H(), psRec: H(), msBlock: H(), msEnd: H(), psBlock: H(), psEnd: H() };
  const blockRec = new Map(userBlocks.map(b => [b.name, { rec: H(), blk: H(), end: H() }]));
  const layerHandle = new Map(layers.map(n => [n, H()]));
  const ltypeHandle = new Map([['ByBlock', H()], ['ByLayer', H()], ...ltypeNames.map(n => [n, H()])]);

  // ---------- extents
  let bb = null;
  for (const e of doc.model.entities) { try { bb = g.unionBounds(bb, boundsOf(e, doc)); } catch { /* skip */ } }
  const ext = bb || { minX: 0, minY: 0, maxX: 1, maxY: 1 };

  // ---------- HEADER
  o.c(0, 'SECTION').c(2, 'HEADER');
  o.c(9, '$ACADVER').c(1, 'AC1015').c(9, '$DWGCODEPAGE').c(3, 'ANSI_1252');
  o.c(9, '$HANDSEED').c(5, '__HANDSEED__');
  o.c(9, '$INSBASE').pt({ x: 0, y: 0 });
  o.c(9, '$EXTMIN').pt({ x: ext.minX, y: ext.minY });
  o.c(9, '$EXTMAX').pt({ x: ext.maxX, y: ext.maxY });
  o.c(9, '$LIMMIN').c(10, 0).c(20, 0); o.c(9, '$LIMMAX').c(10, 420).c(20, 297);
  o.c(9, '$LTSCALE').c(40, doc.settings.ltscale);
  o.c(9, '$PDMODE').c(70, doc.settings.pdmode); o.c(9, '$PDSIZE').c(40, doc.settings.pdsize);
  o.c(9, '$TEXTSIZE').c(40, doc.settings.textHeight);
  o.c(9, '$CLAYER').c(8, doc.cur.layer);
  o.c(9, '$INSUNITS').c(70, INSUNITS_OK.has(doc.settings.units.insUnits) ? doc.settings.units.insUnits : 0);
  o.c(9, '$MEASUREMENT').c(70, doc.settings.units.insUnits === 1 || doc.settings.units.insUnits === 2 ? 0 : 1);
  o.c(9, '$LUNITS').c(70, 2).c(9, '$LUPREC').c(70, doc.settings.units.precision);
  o.c(9, '$AUPREC').c(70, doc.settings.units.angPrecision);
  const ds = doc.settings.dimStyles[doc.settings.curDimStyle] || Object.values(doc.settings.dimStyles)[0];
  o.c(9, '$DIMSCALE').c(40, ds.scale).c(9, '$DIMTXT').c(40, ds.txt).c(9, '$DIMASZ').c(40, ds.asz).c(9, '$DIMEXO').c(40, ds.exo).c(9, '$DIMEXE').c(40, ds.exe).c(9, '$DIMGAP').c(40, ds.gap).c(9, '$DIMDEC').c(70, ds.dec).c(9, '$DIMCEN').c(40, ds.cen);
  o.c(0, 'ENDSEC');

  // ---------- TABLES
  o.c(0, 'SECTION').c(2, 'TABLES');
  const table = (name, handleKey, count, body) => {
    o.c(0, 'TABLE').c(2, name).c(5, hs[handleKey]).c(330, 0).c(100, 'AcDbSymbolTable').c(70, count);
    body();
    o.c(0, 'ENDTAB');
  };
  table('VPORT', 'vport', 1, () => {
    o.c(0, 'VPORT').c(5, hs.vportRec).c(330, hs.vport).c(100, 'AcDbSymbolTableRecord').c(100, 'AcDbViewportTableRecord').c(2, '*Active').c(70, 0)
      .c(10, 0).c(20, 0).c(11, 1).c(21, 1).c(12, (ext.minX + ext.maxX) / 2).c(22, (ext.minY + ext.maxY) / 2).c(13, 0).c(23, 0).c(14, 10).c(24, 10).c(15, 10).c(25, 10)
      .c(16, 0).c(26, 0).c(36, 1).c(17, 0).c(27, 0).c(37, 0).c(40, Math.max(1, ext.maxY - ext.minY) * 1.2).c(41, 1.5).c(42, 50).c(43, 0).c(44, 0).c(50, 0).c(51, 0).c(71, 0).c(72, 100).c(73, 1).c(74, 3).c(75, 0).c(76, 1).c(77, 0).c(78, 0);
  });
  table('LTYPE', 'ltype', 2 + ltypeNames.length, () => {
    const ent = (name, desc, pattern, key) => {
      o.c(0, 'LTYPE').c(5, ltypeHandle.get(key || name)).c(330, hs.ltype).c(100, 'AcDbSymbolTableRecord').c(100, 'AcDbLinetypeTableRecord').c(2, name).c(70, 0).c(3, desc).c(72, 65).c(73, pattern.length);
      o.c(40, pattern.reduce((s, v) => s + Math.abs(v), 0));
      for (const v of pattern) o.c(49, v).c(74, 0);
    };
    ent('ByBlock', '', [], 'ByBlock'); ent('ByLayer', '', [], 'ByLayer');
    for (const n of ltypeNames) ent(n, LINETYPES[n].desc, LINETYPES[n].pattern);
  });
  table('LAYER', 'layer', layers.length, () => {
    for (const l of doc.layers) {
      const aci = typeof l.color === 'string' && l.color[0] === '#' ? hexToAci(l.color) : (l.color || 7);
      o.c(0, 'LAYER').c(5, layerHandle.get(l.name)).c(330, hs.layer).c(100, 'AcDbSymbolTableRecord').c(100, 'AcDbLayerTableRecord').c(2, l.name)
        .c(70, (l.frozen ? 1 : 0) | (l.locked ? 4 : 0)).c(62, l.on ? aci : -aci);
      if (typeof l.color === 'string' && l.color[0] === '#') o.c(420, parseInt(l.color.slice(1), 16));
      o.c(6, l.ltype && LINETYPES[l.ltype] ? l.ltype : 'Continuous').c(290, l.plot === false ? 0 : 1).c(370, l.lw ?? 25);
    }
  });
  table('STYLE', 'style', 1, () => {
    o.c(0, 'STYLE').c(5, hs.styleRec).c(330, hs.style).c(100, 'AcDbSymbolTableRecord').c(100, 'AcDbTextStyleTableRecord').c(2, 'Standard').c(70, 0).c(40, 0).c(41, 1).c(50, 0).c(71, 0).c(42, 2.5).c(3, 'arial.ttf').c(4, '');
  });
  table('VIEW', 'view', 0, () => {});
  table('UCS', 'ucs', 0, () => {});
  table('APPID', 'appid', 1, () => { o.c(0, 'APPID').c(5, hs.appidRec).c(330, hs.appid).c(100, 'AcDbSymbolTableRecord').c(100, 'AcDbRegAppTableRecord').c(2, 'ACAD').c(70, 0); });
  const dimBlocks = [];     // anonymous *D blocks, filled while writing entities
  // DIMSTYLE and BLOCK_RECORD tables are written after the entities are known (dimension blocks)

  // ---------- entity writers (collected per space)
  const entityOut = (list, ownerHandle, blockName) => {
    const out = new Out();
    const common = (type, e, sub, extra = {}) => {
      out.c(0, type).c(5, H()).c(330, ownerHandle).c(100, 'AcDbEntity');
      if (blockName === '*Paper_Space') out.c(67, 1);
      out.c(8, extra.layer ?? (e.layer ?? '0'));
      const lt = e.ltype === 'BYBLOCK' ? 'ByBlock' : e.ltype;
      if (lt && (LINETYPES[lt] || lt === 'ByBlock')) out.c(6, lt);
      const col = e.color;
      if (col === 'BYBLOCK') out.c(62, 0);
      else if (typeof col === 'number') out.c(62, col);
      else if (typeof col === 'string' && col[0] === '#') out.c(62, hexToAci(col)).c(420, parseInt(col.slice(1), 16));
      if (e.lw !== undefined && e.lw !== LW_BYLAYER) out.c(370, e.lw);
      if (sub) out.c(100, sub);
    };
    const text = (p, h, s, rot, jh, jv, e, layer) => {
      common('TEXT', e, 'AcDbText', { layer });
      out.pt(p).c(40, h).c(1, clean(s)).c(50, g.deg(rot || 0)).c(7, 'Standard');
      if ((JH[jh] || 0) !== 0 || (JV[jv] || 0) !== 0) { out.c(72, JH[jh] || 0); out.pt(p, 1); }
      out.c(100, 'AcDbText').c(73, JV[jv] || 0);
    };
    const mtext = (e) => {
      common('MTEXT', e, 'AcDbMText');
      out.pt(e.p).c(40, e.h).c(41, e.w || 0).c(71, ATTACH[`${e.jh || 'left'},${e.jv || 'top'}`] || 1).c(72, 1)
        .c(1, e.s.split('\n').map(clean).join('\\P')).c(7, 'Standard').c(11, Math.cos(e.rot || 0)).c(21, Math.sin(e.rot || 0)).c(31, 0);
    };
    const line = (a, b, e, layer) => { common('LINE', e, 'AcDbLine', { layer }); out.pt(a).pt(b, 1); };
    const solid = (pts, e, layer) => {
      common('SOLID', e, 'AcDbTrace', { layer });
      const p = pts.length === 3 ? [pts[0], pts[1], pts[2], pts[2]] : pts;
      out.pt(p[0], 0).pt(p[1], 1).pt(p[3], 2).pt(p[2], 3);
    };
    const lwpoly = (verts, closed, e, layer) => {
      common('LWPOLYLINE', e, 'AcDbPolyline', { layer });
      out.c(90, verts.length).c(70, closed ? 1 : 0).c(43, 0);
      for (const v of verts) { out.c(10, v.x).c(20, v.y); if (v.b) out.c(42, v.b); }
    };

    const write = (e) => {
      switch (e.type) {
        case 'line': line(e.a, e.b, e); break;
        case 'circle': common('CIRCLE', e, 'AcDbCircle'); out.pt(e.c).c(40, e.r); break;
        case 'arc': common('ARC', e, 'AcDbCircle'); out.pt(e.c).c(40, e.r).c(100, 'AcDbArc').c(50, g.deg(e.a0)).c(51, g.deg(e.a1)); break;
        case 'pline': lwpoly(e.v, e.closed, e); break;
        case 'point': common('POINT', e, 'AcDbPoint'); out.pt(e.p); break;
        case 'ellipse': {
          common('ELLIPSE', e, 'AcDbEllipse'); out.pt(e.c).pt(e.m, 1).c(210, 0).c(220, 0).c(230, 1).c(40, e.ratio);
          const full = ellipseSweep(e) >= g.TAU - 1e-9;
          out.c(41, full ? 0 : e.t0).c(42, full ? g.TAU : e.t0 + ellipseSweep(e)); break;
        }
        case 'spline': {
          const bz = splineBeziers(e);
          if (!bz.length) break;
          const ctrl = [bz[0].p0]; for (const b of bz) ctrl.push(b.c1, b.c2, b.p1);
          const knots = [0, 0, 0, 0]; for (let i = 1; i < bz.length; i++) knots.push(i, i, i); knots.push(bz.length, bz.length, bz.length, bz.length);
          common('SPLINE', e, 'AcDbSpline');
          out.c(210, 0).c(220, 0).c(230, 1).c(70, 8).c(71, 3).c(72, knots.length).c(73, ctrl.length).c(74, 0).c(42, 1e-7).c(43, 1e-7).c(44, 1e-10);
          for (const k of knots) out.c(40, k);
          for (const p of ctrl) out.pt(p);
          break;
        }
        case 'text': if (e.multi) mtext(e); else text(e.p, e.h, e.s, e.rot, e.jh, e.jv, e); break;
        case 'xline': common('XLINE', e, 'AcDbXline'); out.pt(e.p).pt(e.d, 1); break;
        case 'ray': common('RAY', e, 'AcDbRay'); out.pt(e.p).pt(e.d, 1); break;
        case 'insert': {
          if (!blockRec.has(e.name)) break;
          common('INSERT', e, 'AcDbBlockReference'); out.c(2, e.name).pt(e.p).c(41, e.sx ?? 1).c(42, e.sy ?? 1).c(43, 1).c(50, g.deg(e.rot || 0)); break;
        }
        case 'leader': {
          common('LEADER', e, 'AcDbLeader');
          out.c(3, 'Standard').c(71, 1).c(72, 0).c(73, 3).c(74, 1).c(75, 0).c(40, e.h).c(41, 0).c(76, e.pts.length);
          for (const p of e.pts) out.pt(p);
          if (e.s) text({ x: e.pts[e.pts.length - 1].x + e.h * .4, y: e.pts[e.pts.length - 1].y }, e.h, e.s, 0, 'left', 'middle', e);
          break;
        }
        case 'hatch': {
          const pat = PATTERNS[e.pattern] || PATTERNS.ANSI31, solidFill = !!pat.solid;
          common('HATCH', e, 'AcDbHatch');
          out.pt({ x: 0, y: 0 }).c(210, 0).c(220, 0).c(230, 1).c(2, solidFill ? 'SOLID' : e.pattern).c(70, solidFill ? 1 : 0).c(71, 0).c(91, e.loops.length);
          e.loops.forEach((loop, i) => {
            out.c(92, i === 0 ? 3 : 2).c(72, 1).c(73, 1).c(93, loop.length);
            for (const v of loop) { out.c(10, v.x).c(20, v.y).c(42, v.b || 0); }
            out.c(97, 0);
          });
          out.c(75, 0).c(76, 1);
          if (!solidFill) {
            out.c(52, g.deg(e.pa || 0)).c(41, e.ps || 1).c(77, 0).c(78, pat.families.length);
            for (const f of pat.families) {
              out.c(53, f.a).c(43, f.ox).c(44, f.oy).c(45, f.dx).c(46, f.dy).c(79, f.dash ? f.dash.length : 0);
              for (const d of f.dash || []) out.c(49, d);
            }
          }
          out.c(98, 0);
          break;
        }
        case 'dim': {
          let geo;
          try { geo = dimGeometry(e, doc); } catch { break; }
          const sub = { layer: e.layer, color: e.color };
          const emit = () => {
            for (const s of geo.segs) {
              if (s.t === 'l') line(s.a, s.b, sub, e.layer);
              else { common('ARC', sub, 'AcDbCircle'); out.pt(s.c).c(40, s.r).c(100, 'AcDbArc').c(50, g.deg(s.sw >= 0 ? s.a0 : s.a0 + s.sw)).c(51, g.deg(s.sw >= 0 ? s.a0 + s.sw : s.a0)); }
            }
            for (const a of geo.arrows) {
              const poly = arrowPolygon(a);
              if (poly) solid(poly, sub, e.layer);
              else for (const [p, q] of arrowLines(a)) line(p, q, sub, e.layer);
            }
            if (geo.text) text(geo.text.p, geo.text.h, geo.text.s, geo.text.rot, geo.text.jh, geo.text.jv, sub);
          };
          if (e.kind === 'ang') { emit(); break; }               // angular dimensions are written as plain geometry
          const rec = { name: `*D${dimBlocks.length + 1}`, rec: H(), blk: H(), end: H(), layer: e.layer ?? '0', out: new Out() };
          dimBlocks.push(rec);
          const keepLines = out.lines, keepOwner = ownerHandle;
          out.lines = rec.out.lines; ownerHandle = rec.rec; emit();
          out.lines = keepLines; ownerHandle = keepOwner;
          const type = { lin: 0, ali: 1, dia: 3, rad: 4 }[e.kind] ?? 0;
          common('DIMENSION', e, 'AcDbDimension');
          out.c(2, rec.name).pt(e.kind === 'rad' ? e.c : e.kind === 'dia' ? g.add(e.c, g.sub(e.c, e.pt)) : e.dp);
          out.pt(geo.text?.p || e.dp, 1).c(70, 32 | type).c(71, 5).c(3, 'Standard');
          if (e.txt) out.c(1, e.txt);
          if (e.kind === 'lin' || e.kind === 'ali') {
            out.c(100, 'AcDbAlignedDimension').pt(e.p1, 3).pt(e.p2, 4);
            if (e.kind === 'lin') out.c(50, g.deg(e.ang || 0)).c(100, 'AcDbRotatedDimension');
          } else out.c(100, e.kind === 'dia' ? 'AcDbDiametricDimension' : 'AcDbRadialDimension').pt(e.pt, 5).c(40, 0);
          break;
        }
        default: break;
      }
    };
    for (const e of list) { try { write(e); } catch (err) { /* skip entities that cannot be exported */ void err; } }
    return out;
  };

  // model space (written first to discover dimension blocks), then user blocks
  const modelOut = entityOut(doc.model.entities, hs.msRec, '*Model_Space');
  const blockOuts = userBlocks.map(b => ({ b, out: entityOut(b.entities, blockRec.get(b.name).rec, b.name) }));
  const layoutOuts = doc.layouts.map(l => entityOut(l.entities.filter(e => e.type !== 'viewport'), hs.psRec, '*Paper_Space'));

  // ---------- DIMSTYLE and BLOCK_RECORD tables
  const tail = new Out();
  const savedLines = o.lines; o.lines = tail.lines;
  table('DIMSTYLE', 'dimstyle', 1, () => {
    o.c(0, 'DIMSTYLE').c(105, hs.dimstyleRec).c(330, hs.dimstyle).c(100, 'AcDbSymbolTableRecord').c(100, 'AcDbDimStyleTableRecord').c(2, 'Standard').c(70, 0)
      .c(40, ds.scale).c(41, ds.asz).c(42, ds.exo).c(43, 3.75).c(44, ds.exe).c(140, ds.txt).c(141, ds.cen).c(147, ds.gap).c(271, ds.dec).c(77, ds.tad ? 1 : 0);
  });
  const brCount = 2 + userBlocks.length + dimBlocks.length;
  table('BLOCK_RECORD', 'blockRecord', brCount, () => {
    const rec = (name, h) => o.c(0, 'BLOCK_RECORD').c(5, h).c(330, hs.blockRecord).c(100, 'AcDbSymbolTableRecord').c(100, 'AcDbBlockTableRecord').c(2, name).c(70, 0).c(280, 1).c(281, 0);
    rec('*Model_Space', hs.msRec); rec('*Paper_Space', hs.psRec);
    for (const b of userBlocks) rec(b.name, blockRec.get(b.name).rec);
    for (const d of dimBlocks) rec(d.name, d.rec);
  });
  o.c(0, 'ENDSEC');
  o.lines = savedLines;
  o.lines.push(...tail.lines);

  // ---------- BLOCKS
  o.c(0, 'SECTION').c(2, 'BLOCKS');
  const block = (name, h, endH, ownerRec, body, base = { x: 0, y: 0 }, layer = '0') => {
    o.c(0, 'BLOCK').c(5, h).c(330, ownerRec).c(100, 'AcDbEntity').c(8, layer).c(100, 'AcDbBlockBegin').c(2, name).c(70, name.startsWith('*') && !/^\*(Model|Paper)_Space/.test(name) ? 1 : 0).pt(base).c(3, name).c(1, '');
    o.lines.push(...body);
    o.c(0, 'ENDBLK').c(5, endH).c(330, ownerRec).c(100, 'AcDbEntity').c(8, layer).c(100, 'AcDbBlockEnd');
  };
  block('*Model_Space', hs.msBlock, hs.msEnd, hs.msRec, []);
  block('*Paper_Space', hs.psBlock, hs.psEnd, hs.psRec, layoutOuts.flatMap(x => x.lines));
  for (const { b, out } of blockOuts) block(b.name, blockRec.get(b.name).blk, blockRec.get(b.name).end, blockRec.get(b.name).rec, out.lines, b.base || { x: 0, y: 0 });
  for (const d of dimBlocks) block(d.name, d.blk, d.end, d.rec, d.out.lines, { x: 0, y: 0 }, d.layer);
  o.c(0, 'ENDSEC');

  // ---------- ENTITIES
  o.c(0, 'SECTION').c(2, 'ENTITIES');
  o.lines.push(...modelOut.lines);
  o.c(0, 'ENDSEC');

  // ---------- OBJECTS (root dictionary only)
  o.c(0, 'SECTION').c(2, 'OBJECTS');
  o.c(0, 'DICTIONARY').c(5, hs.dictRoot).c(330, 0).c(100, 'AcDbDictionary').c(281, 1).c(3, 'ACAD_GROUP').c(350, hs.dictGroup);
  o.c(0, 'DICTIONARY').c(5, hs.dictGroup).c(330, hs.dictRoot).c(100, 'AcDbDictionary').c(281, 1);
  o.c(0, 'ENDSEC').c(0, 'EOF');

  return o.toString().replace('__HANDSEED__', handle.toString(16).toUpperCase());
}

