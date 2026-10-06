// Text layout shared by rendering, hit-testing, bounds and plotting.
import { M, polar } from './geom.js';

/** Pluggable width measurement; the browser build installs a canvas-based measurer. */
export const platform = {
  /** @returns width in drawing units of `str` at text (cap) height `h`. */
  measureText: (str, h) => str.length * h * .62,
};

export const FONT = 'Arial, "Liberation Sans", "Helvetica Neue", sans-serif';
export const CAP_RATIO = .72;            // cap height / em size for the UI font
export const LINE_PITCH = 1.5;           // line spacing as a multiple of text height
export const emSize = h => h / CAP_RATIO;

/** Break text to lines, honouring explicit newlines and (optionally) wrapping at width `w`. */
export function wrapLines(str, h, w = 0) {
  const raw = String(str).replace(/\\P/g, '\n').split('\n');
  if (!(w > 0)) return raw;
  const out = [];
  for (const line of raw) {
    let cur = '';
    for (const word of line.split(' ')) {
      const next = cur ? cur + ' ' + word : word;
      if (cur && platform.measureText(next, h) > w) { out.push(cur); cur = word; } else cur = next;
    }
    out.push(cur);
  }
  return out;
}

/**
 * Local layout of a text entity. Local origin = insertion point, +x along the baseline.
 * Returns the lines (with baseline y) and the local bounding box.
 */
export function textLayout(e) {
  const h = e.h, lines = wrapLines(e.s ?? '', h, e.w);
  const widths = lines.map(l => platform.measureText(l, h));
  const maxW = Math.max(0, ...widths);
  const boxW = e.w > 0 ? Math.max(e.w, maxW) : maxW;
  const pitch = h * LINE_PITCH, n = lines.length;
  const jv = e.jv || 'baseline', jh = e.jh || 'left';
  // y of the FIRST baseline relative to the insertion point
  let y0;
  const blockH = (n - 1) * pitch + h;
  if (jv === 'top') y0 = -h;
  else if (jv === 'middle') y0 = blockH / 2 - h;
  else if (jv === 'bottom') y0 = (n - 1) * pitch;
  else y0 = 0;                                   // baseline
  const ox = jh === 'center' ? -boxW / 2 : jh === 'right' ? -boxW : 0;
  const out = lines.map((str, i) => {
    const w = widths[i];
    const x = jh === 'center' ? ox + (boxW - w) / 2 : jh === 'right' ? ox + boxW - w : ox;
    return { str, x, y: y0 - i * pitch, w };
  });
  const top = y0 + h, bottom = y0 - (n - 1) * pitch - h * .3;
  return { lines: out, box: { x0: ox, x1: ox + boxW, y0: bottom, y1: top }, em: emSize(h) };
}

/** World-space corners (CCW) of the text bounding box. */
export function textCorners(e) {
  const { box } = textLayout(e);
  const m = M.mul(M.translate(e.p.x, e.p.y), M.rotate(e.rot || 0));
  return [[box.x0, box.y0], [box.x1, box.y0], [box.x1, box.y1], [box.x0, box.y1]].map(([x, y]) => M.apply(m, { x, y }));
}

/** World-space start of each line's baseline (for plotting / DXF). */
export function textLineOrigins(e) {
  const { lines } = textLayout(e);
  const m = M.mul(M.translate(e.p.x, e.p.y), M.rotate(e.rot || 0));
  return lines.map(l => ({ str: l.str, p: M.apply(m, { x: l.x, y: l.y }), w: l.w }));
}

/** Text anchored along a direction (used by dimensions): returns an entity-like object. */
export function makeText(p, h, s, rot = 0, jh = 'center', jv = 'middle') {
  return { type: 'text', p, h, s, rot, jh, jv };
}

export const textEnd = (e, w) => polar(e.p, e.rot || 0, w);
