// Writes a DXF containing one of every entity type, for validation with ezdxf (see validate.py).
import fs from 'node:fs';
import { sampleDrawing } from '../../src/io/sample.js';
import { writeDXF } from '../../src/io/dxf-write.js';
import { E } from '../../src/core/entities.js';

const d = sampleDrawing();
d.add(E.arc({ x: 200, y: 20 }, 15, 0.3, 2.2, { layer: 'Outline' }));
d.add(E.ellipse({ x: 200, y: 70 }, { x: 25, y: 10 }, 0.5, 0, Math.PI * 2, { layer: 'Outline' }));
d.add(E.ellipse({ x: 200, y: 110 }, { x: 25, y: 10 }, 0.5, 0.5, 2.5, { layer: 'Outline', color: 3 }));
d.add(E.spline([{ x: 150, y: 0 }, { x: 160, y: 20 }, { x: 175, y: 5 }, { x: 190, y: 25 }], false, { layer: 'Outline' }));
d.add(E.point({ x: 120, y: 120 }, { layer: 'Notes' }));
d.add(E.pline([{ x: 220, y: 0, b: 0.5 }, { x: 260, y: 0, b: 0 }, { x: 260, y: 30, b: -0.3 }, { x: 220, y: 30 }], true, { layer: 'Outline', color: '#ff8800', lw: 70 }));
d.add(E.text({ x: 220, y: 60 }, 5, 'Hello DXF', { rot: 0.4, jh: 'center', jv: 'middle', layer: 'Notes' }));
d.add(E.xline({ x: 0, y: 0 }, { x: 1, y: 1 }, { layer: 'Centre' }));
d.add(E.ray({ x: 0, y: 0 }, { x: -1, y: 1 }, { layer: 'Centre' }));
d.add(E.dim('rad', { c: { x: 200, y: 20 }, pt: { x: 215, y: 20 }, dp: { x: 230, y: 40 }, layer: 'Dimensions' }));
d.add(E.dim('ali', { p1: { x: 0, y: 90 }, p2: { x: 30, y: 90 }, dp: { x: 15, y: 100 }, layer: 'Dimensions' }));
d.add(E.dim('ang', { c: { x: 0, y: 0 }, p1: { x: 60, y: 0 }, p2: { x: 40, y: 40 }, dp: { x: 50, y: 20 }, layer: 'Dimensions' }));
d.add(E.hatch([[{ x: 220, y: 0, b: 0 }, { x: 260, y: 0, b: 0 }, { x: 260, y: 30, b: 0 }, { x: 220, y: 30, b: 0 }]], { pattern: 'SOLID', layer: 'Hatch', color: 5 }));
d.defineBlock('BOLT', { x: 0, y: 0 }, [E.circle({ x: 0, y: 0 }, 3, {}), E.line({ x: -4, y: 0 }, { x: 4, y: 0 }, {})].map((e, i) => ({ ...e, id: 900 + i, layer: '0' })));
d.add(E.insert('BOLT', { x: 40, y: 60 }, { sx: 2, sy: 2, rot: 0.5, layer: 'Holes' }));
fs.writeFileSync(process.argv[2] || 'sample.dxf', writeDXF(d));
console.log('entities:', d.entities.length);
