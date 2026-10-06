// A small sample drawing: a dimensioned L-bracket with bolt holes, centre lines and a hatched section.
import { Drawing } from '../core/drawing.js';
import { E } from '../core/entities.js';
import { newLayer } from '../core/style.js';

export function sampleDrawing() {
  const d = new Drawing('L-Bracket');
  d.layers.push(newLayer('Outline', { color: 7, lw: 50 }), newLayer('Holes', { color: 4, lw: 25 }), newLayer('Centre', { color: 1, ltype: 'CENTER', lw: 13 }),
    newLayer('Dimensions', { color: 3, lw: 13 }), newLayer('Hatch', { color: 8, lw: 9 }), newLayer('Notes', { color: 2, lw: 13 }));
  const L = n => ({ layer: n });
  const outline = [[0, 0], [120, 0], [120, 30], [30, 30], [30, 90], [0, 90]].map(([x, y]) => ({ x, y }));
  d.add(E.pline(outline, true, L('Outline')));
  const holes = [[75, 15, 5], [15, 60, 5], [15, 75, 5]];
  for (const [x, y, r] of holes) {
    d.add(E.circle({ x, y }, r, L('Holes')));
    d.add(E.line({ x: x - r - 4, y }, { x: x + r + 4, y }, L('Centre')));
    d.add(E.line({ x, y: y - r - 4 }, { x, y: y + r + 4 }, L('Centre')));
  }
  d.add(E.hatch([[{ x: 0, y: 0, b: 0 }, { x: 8, y: 0, b: 0 }, { x: 8, y: 30, b: 0 }, { x: 0, y: 30, b: 0 }]], { pattern: 'ANSI31', ps: 1.2, pa: 0, layer: 'Hatch' }));
  d.add(E.dim('lin', { p1: { x: 0, y: 0 }, p2: { x: 120, y: 0 }, dp: { x: 60, y: -16 }, ang: 0, layer: 'Dimensions' }));
  d.add(E.dim('lin', { p1: { x: 0, y: 0 }, p2: { x: 0, y: 90 }, dp: { x: -16, y: 45 }, ang: Math.PI / 2, layer: 'Dimensions' }));
  d.add(E.dim('lin', { p1: { x: 120, y: 0 }, p2: { x: 120, y: 30 }, dp: { x: 136, y: 15 }, ang: Math.PI / 2, layer: 'Dimensions' }));
  d.add(E.dim('dia', { c: { x: 75, y: 15 }, pt: { x: 75 + 5 * Math.SQRT1_2, y: 15 + 5 * Math.SQRT1_2 }, dp: { x: 94, y: 29 }, layer: 'Dimensions' }));
  d.add(E.leader([{ x: 15, y: 75 }, { x: 52, y: 100 }], 'Ø10 THRU, 2 HOLES', 3.5, { layer: 'Notes' }));
  d.add(E.mtext({ x: 0, y: -42 }, 4, 'L-BRACKET\nMaterial: 6061-T6 aluminium\nScale 1:1', 100, { layer: 'Notes' }));
  d.cur.layer = 'Outline';
  d.dirty = false;
  return d;
}
