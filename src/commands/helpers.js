// Shared helpers for command implementations.
import * as g from '../core/geom.js';
import { E, clone, transformEntity, segsOf } from '../core/entities.js';
import { transformSolid } from '../solid/mesh.js';

/** Persistent defaults between command runs (last radius, polygon sides, hatch settings...). */
export const memory = app => (app.mem ??= {
  circleRadius: 5, polygonSides: 4, polygonMode: 'Inscribed', donutIn: .5, donutOut: 1, rectFillet: 0, rectChamfer: [0, 0], filletRadius: undefined,
  hatch: { pattern: 'ANSI31', scale: 1, angle: 0, color: undefined, island: 'normal' }, arrayRect: { rows: 3, cols: 4, dr: 10, dc: 10 },
  arrayPolar: { n: 6, fill: 360, rotate: true }, textHeight: 2.5, dimStyle: 'Standard', insertScale: 1, insertRot: 0, qpOpen: false,
});

/** Clones of `entities` with `matrix` applied (new objects, no ids) – for previews and COPY. */
export function transformedClones(entities, matrix, doc) {
  return entities.map(e => { const c = clone(e); transformEntity(c, matrix, doc); return c; });
}

/** Apply a matrix to existing entities in place (inside a change notification). */
export function applyTransform(doc, entities, matrix) {
  doc.change(() => {
    for (const e of entities) transformEntity(e, matrix, doc);
    for (const s of entities.solids || []) { const t = transformSolid(s, matrix); s.vertices = t.vertices; s.faces = t.faces; }
  });
}

/** Add transformed copies to the drawing, remapping group ids so copies form their own groups. */
export function addCopies(doc, entities, matrix) {
  const groupMap = new Map(), out = [];
  for (const e of entities) {
    const c = clone(e); transformEntity(c, matrix, doc);
    if (c.grp) { if (!groupMap.has(c.grp)) groupMap.set(c.grp, `G${doc.nextId}_${groupMap.size}`); c.grp = groupMap.get(c.grp); }
    out.push(doc.add(c));
  }
  out.solids = [];
  for (const s of entities.solids || []) {
    const t = transformSolid(s, matrix), copy = { ...structuredClone({ ...s, vertices: undefined, faces: undefined, id: undefined, program: undefined }), vertices: t.vertices, faces: t.faces };
    if (s.program) copy.program = structuredClone(s.program);
    out.solids.push(doc.addSolid(copy));
  }
  return out;
}

export const ptStr = (p, prec = 4) => `${+p.x.toFixed(prec)},${+p.y.toFixed(prec)}`;
export const num = (v, prec = 4) => String(+Number(v).toFixed(prec));
export const degText = (rad, prec = 4) => `${+g.deg(rad).toFixed(prec)}`;

/** Bulge of an arc from p0 to p1 given the tangent direction at p0. */
export function tangentBulge(p0, dir, p1) {
  const chord = g.angleOf(p0, p1), theta = g.normAngleSigned(chord - Math.atan2(dir.y, dir.x));
  return Math.tan(theta / 2);
}

/** Closed rectangle outline (CCW) from two corners, optionally rotated about the first corner. */
export function rectVerts(a, b, rot = 0) {
  const u = { x: Math.cos(rot), y: Math.sin(rot) }, v = { x: -u.y, y: u.x };
  const d = g.sub(b, a), w = g.dot(d, u), h = g.dot(d, v);
  return [a, g.add(a, g.mul(u, w)), b, g.add(a, g.mul(v, h))].map(p => ({ x: p.x, y: p.y, b: 0 }));
}

/** Segments from a list of entities usable as geometry for object tangency etc. */
export const segsFor = (e, doc) => segsOf(e, doc);

/** Rubber-band preview of a polyline under construction. */
export function plinePreview(verts, tail) {
  if (!verts.length) return [];
  return [E.pline([...verts, tail], false)];
}

export const isClosedCurve = e => e.type === 'circle' || e.type === 'ellipse' || (e.type === 'pline' && e.closed) || (e.type === 'spline' && e.closed);
