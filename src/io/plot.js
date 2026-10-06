// Plotting: lays a drawing out on a sheet and renders it with the scene renderer into any 2D context
// (a real canvas for preview/PNG, or a RecordingContext for PDF/SVG). Device unit = 0.1 mm.
import * as g from '../core/geom.js';
import { boundsOf } from '../core/entities.js';
import { View } from '../engine/view.js';
import { makeContext, drawEntity } from '../render/renderer.js';
import { RecordingContext } from './recorder.js';
import { writePDF } from './pdf.js';
import { writeSVG } from './svg.js';

export const UNIT_MM = 0.1;
export const PAPERS = { A4: [297, 210], A3: [420, 297], A2: [594, 420], A1: [841, 594], A0: [1189, 841], Letter: [279.4, 215.9], Legal: [355.6, 215.9], Tabloid: [431.8, 279.4] };
/** Millimetres per drawing unit for $INSUNITS values. */
export const MM_PER_INSUNIT = { 0: 1, 1: 25.4, 2: 304.8, 4: 1, 5: 10, 6: 1000 };
export const SCALES = [['Fit to paper', 'fit'], ['1:1', 1], ['1:2', 1 / 2], ['1:5', 1 / 5], ['1:10', 1 / 10], ['1:20', 1 / 20], ['1:50', 1 / 50], ['1:100', 1 / 100], ['1:200', 1 / 200], ['2:1', 2], ['5:1', 5], ['10:1', 10]];

const plottable = (doc, e) => { const l = doc.layerOf(e); return l.on && !l.frozen && l.plot !== false; };

export function defaultPlotOptions(doc) {
  return { paper: 'A4', landscape: true, area: 'extents', scale: 'fit', center: true, offsetX: 0, offsetY: 0, margin: 5, lineweights: true, mono: true, format: 'pdf', dpi: 200, title: doc.name };
}

/** Page size in mm for the options. */
export function pageSize(o) { const [a, b] = PAPERS[o.paper] || PAPERS.A4; return o.landscape ? { w: Math.max(a, b), h: Math.min(a, b) } : { w: Math.min(a, b), h: Math.max(a, b) }; }

export function plotBounds(doc, o, viewBounds) {
  if (o.area === 'display' && viewBounds) return viewBounds;
  let b = null;
  for (const e of doc.model.entities) if (plottable(doc, e)) { try { b = g.unionBounds(b, boundsOf(e, doc)); } catch { /* ignore */ } }
  return b;
}

/** Work out the world -> sheet mapping. Throws when there is nothing to plot. */
export function plotLayout(doc, o, viewBounds) {
  const page = pageSize(o), bounds = plotBounds(doc, o, viewBounds);
  if (!bounds) throw Error('There is nothing to plot.');
  const bw = Math.max(bounds.maxX - bounds.minX, 1e-9), bh = Math.max(bounds.maxY - bounds.minY, 1e-9);
  const pw = page.w - 2 * o.margin, ph = page.h - 2 * o.margin;
  let s;
  if (o.scale === 'fit') s = Math.min(pw / bw, ph / bh);
  else s = (MM_PER_INSUNIT[doc.settings.units.insUnits] ?? 1) * Number(o.scale);
  const ox = o.center ? (page.w - bw * s) / 2 : o.margin + (o.offsetX || 0), oy = o.center ? (page.h - bh * s) / 2 : o.margin + (o.offsetY || 0);
  const worldM = [10 * s, 0, 0, -10 * s, 10 * (ox - s * bounds.minX), 10 * (page.h - oy + s * bounds.minY)];
  const fits = bw * s <= pw + 1e-6 && bh * s <= ph + 1e-6;
  return { page, bounds, mmPerUnit: s, worldM, fits, W: Math.round(page.w * 10), H: Math.round(page.h * 10) };
}

const THEME = { dark: false, canvas: '#ffffff', grid: '#000000', gridMajor: 'rgba(0,0,0,.1)', select: '#000000', hover: '#000000', cross: '#000000', gridAlpha: 0 };

/** Draw the sheet. `dpr` converts device units (0.1 mm) to the context's units (1 for recording). */
export function renderPlot(ctx, doc, o, layout, dpr = 1, { background = true } = {}) {
  const { W, H } = layout;
  const prefs = { LWDISPLAY: 1, LAYLOCKFADECTL: 0, TRANSPARENCYDISPLAY: 1, FILLMODE: 1 };
  const editor = { doc, view: new View(), sel: new Set(), tempSel: new Set(), hoverId: null };
  const rc = makeContext(ctx, W, H, dpr, editor, prefs, THEME);
  rc.worldM = layout.worldM; rc._vis = null; rc.mono = !!o.mono; rc.fade = 0;
  rc.lwPx = lw => (o.lineweights ? Math.max(lw / 10, 0.6) : 2);
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  if (background) { ctx.fillStyle = '#ffffff'; ctx.fillRect(0, 0, W, H); }
  for (const e of doc.model.entities) { if (plottable(doc, e)) drawEntity(rc, e); }
  return rc;
}

/** Vector export. Returns {data, mime, ext}. */
export function exportVector(doc, o, viewBounds) {
  const layout = plotLayout(doc, o, viewBounds);
  const rec = new RecordingContext();
  renderPlot(rec, doc, o, layout, 1, { background: false });
  const page = { w: layout.W, h: layout.H };
  if (o.format === 'svg') return { data: writeSVG(rec.ops, page, { unitMm: UNIT_MM, title: o.title }), mime: 'image/svg+xml', ext: 'svg', layout };
  return { data: writePDF(rec.ops, page, { unitMm: UNIT_MM, title: o.title }), mime: 'application/pdf', ext: 'pdf', layout };
}

