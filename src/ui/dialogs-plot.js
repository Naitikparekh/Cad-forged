// Plot / export dialog: sheet setup with a live preview, writing PDF, SVG or PNG files.
import { h } from './dom.js';
import { row, num, select, check, note } from './dialogs-kit.js';
import { PAPERS, SCALES, defaultPlotOptions, plotLayout, renderPlot, exportVector, UNIT_MM } from '../io/plot.js';
import { download } from './files.js';

const remembered = new Map();

export const PLOT = {
  plot: ({ ui, doc, data }) => {
    const o = { ...defaultPlotOptions(doc), ...(remembered.get(doc.name) || {}) };
    if (data.format) o.format = data.format;
    const view = ui.editor.view, viewBounds = (() => { try { return view.worldBounds(); } catch { return null; } })();
    const format = select([['pdf', 'PDF (vector)'], ['svg', 'SVG (vector)'], ['png', 'PNG (raster)']].map(([value, label]) => ({ value, label })), o.format, { width: 150 });
    const paper = select(Object.keys(PAPERS), o.paper, { width: 150 });
    const orient = select([{ value: 'l', label: 'Landscape' }, { value: 'p', label: 'Portrait' }], o.landscape ? 'l' : 'p', { width: 150 });
    const area = select([{ value: 'extents', label: 'Extents' }, { value: 'display', label: 'Display' }], o.area, { width: 150 });
    const scale = select(SCALES.map(([label, v]) => ({ value: String(v), label })), String(o.scale), { width: 150 });
    const center = check('Center the plot', o.center), ox = num(o.offsetX, { width: 70 }), oy = num(o.offsetY, { width: 70 });
    const lw = check('Plot with lineweights', o.lineweights), mono = check('Monochrome (black ink)', o.mono);
    const dpi = select([100, 150, 200, 300, 600], o.dpi, { width: 150 });
    const canvas = h('canvas.plot-prev', { width: 420, height: 300 }), info = h('div.fnote');
    const read = () => ({ ...o, format: format.value, paper: paper.value, landscape: orient.value === 'l', area: area.value, scale: scale.value === 'fit' ? 'fit' : Number(scale.value), center: center.input.checked, offsetX: Number(ox.value) || 0, offsetY: Number(oy.value) || 0, lineweights: lw.input.checked, mono: mono.input.checked, dpi: Number(dpi.value) });
    const draw = () => {
      const opts = read(), ctx = canvas.getContext('2d');
      ox.disabled = oy.disabled = opts.center; dpi.disabled = opts.format !== 'png';
      ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.fillStyle = '#5b6470'; ctx.fillRect(0, 0, canvas.width, canvas.height);
      let layout;
      try { layout = plotLayout(doc, opts, viewBounds); } catch (e) { info.textContent = e.message; return; }
      const k = Math.min((canvas.width - 24) / layout.W, (canvas.height - 24) / layout.H), dx = (canvas.width - layout.W * k) / 2, dy = (canvas.height - layout.H * k) / 2;
      ctx.save(); ctx.translate(dx, dy); ctx.beginPath(); ctx.rect(0, 0, layout.W * k, layout.H * k); ctx.clip();
      renderPlot(ctx, doc, opts, layout, k);
      ctx.restore();
      ctx.save(); ctx.translate(dx, dy); ctx.setLineDash([4, 3]); ctx.strokeStyle = 'rgba(0,0,0,.35)'; const m = opts.margin * 10 * k; ctx.strokeRect(m, m, layout.W * k - 2 * m, layout.H * k - 2 * m); ctx.restore();
      const sc = layout.mmPerUnit / (opts.scale === 'fit' ? 1 : 1);
      info.textContent = `${layout.page.w} × ${layout.page.h} mm · 1 drawing unit = ${+sc.toFixed(4)} mm on paper${layout.fits ? '' : ' · the drawing is larger than the printable area'}`;
    };
    for (const el of [format, paper, orient, area, scale, dpi]) el.addEventListener('change', draw);
    for (const el of [center, lw, mono]) el.input.addEventListener('change', draw);
    for (const el of [ox, oy]) el.addEventListener('input', draw);
    setTimeout(draw, 0);
    return {
      title: 'Plot — Model', width: 760, ok: 'Plot',
      body: h('div.plot', h('div.plot-form', row('Output', format), row('Paper size', paper), row('Orientation', orient), row('Plot area', area), row('Scale', scale), h('div.frow', center), row('Offset X, Y (mm)', ox, oy), h('div.frow', lw), h('div.frow', mono), row('PNG resolution (dpi)', dpi)),
        h('div.plot-pv', canvas, info)),
      commit: () => {
        const opts = read(); remembered.set(doc.name, opts);
        try {
          const base = `${doc.name}`;
          if (opts.format === 'png') {
            const layout = plotLayout(doc, opts, viewBounds), s = opts.dpi / 25.4 * UNIT_MM;
            const cv = document.createElement('canvas'); cv.width = Math.round(layout.W * s); cv.height = Math.round(layout.H * s);
            renderPlot(cv.getContext('2d'), doc, opts, layout, s);
            cv.toBlob(b => download(`${base}.png`, b, 'image/png'));
          } else {
            const r = exportVector(doc, opts, viewBounds);
            download(`${base}.${r.ext}`, r.data, r.mime);
          }
          ui.app.log(`Plotted ${doc.name} to ${opts.format.toUpperCase()}.`);
          return true;
        } catch (e) { ui.toast(e.message, 5000); return false; }
      },
    };
  },
};
void note;
