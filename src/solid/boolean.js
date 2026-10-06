// Boolean operations on triangle meshes (union / subtract / intersect) using the bundled csg.js BSP library.
import CSG from './vendor/csg.js';
import { meshVolume } from './mesh.js';

const MAX_TRIANGLES = 100000;

function toCSG(mesh) {
  const polys = [];
  for (const f of mesh.faces) {
    const ps = f.map(i => new CSG.Vector(mesh.vertices[i].x, mesh.vertices[i].y, mesh.vertices[i].z));
    const n = ps[1].minus(ps[0]).cross(ps[2].minus(ps[0]));
    if (n.length() < 1e-12) continue;                                  // skip degenerate triangles
    const normal = n.unit();
    polys.push(new CSG.Polygon(ps.map(p => new CSG.Vertex(p, normal))));
  }
  return CSG.fromPolygons(polys);
}

function fromCSG(csg, name, extra = {}) {
  const vertices = [], faces = [], lookup = new Map();
  const index = p => {
    const key = `${p.x.toFixed(7)},${p.y.toFixed(7)},${p.z.toFixed(7)}`;
    let i = lookup.get(key);
    if (i === undefined) { i = vertices.length; vertices.push({ x: +p.x.toFixed(9), y: +p.y.toFixed(9), z: +p.z.toFixed(9) }); lookup.set(key, i); }
    return i;
  };
  for (const poly of csg.toPolygons()) {
    const ids = poly.vertices.map(v => index(v.pos));
    for (let i = 1; i + 1 < ids.length; i++) if (ids[0] !== ids[i] && ids[i] !== ids[i + 1] && ids[0] !== ids[i + 1]) faces.push([ids[0], ids[i], ids[i + 1]]);
  }
  return { name, vertices, faces, ...extra };
}

/** Synchronous boolean of two solids. Returns a mesh (possibly empty). */
export function booleanSync(a, b, op) {
  if (!['union', 'subtract', 'intersect'].includes(op)) throw Error('Unknown boolean operation.');
  if (a.faces.length + b.faces.length > MAX_TRIANGLES) throw Error(`The solids have more than ${MAX_TRIANGLES} triangles.`);
  const A = toCSG(a), B = toCSG(b);
  const r = op === 'union' ? A.union(B) : op === 'subtract' ? A.subtract(B) : A.intersect(B);
  const m = fromCSG(r, { union: 'Union', subtract: 'Subtract', intersect: 'Intersect' }[op]);
  return m;
}

export { meshVolume };

export const stats = { worker: 0, sync: 0 };
let workerSource = null;
/** The build embeds the worker bundle in the page; main.js hands the text to this module. */
export function setWorkerSource(src) { workerSource = src; }

/** Boolean on a worker thread when available (keeps the UI responsive), otherwise synchronously. */
export function booleanAsync(a, b, op, { timeout = 60000 } = {}) {
  if (typeof Worker === 'undefined' || !workerSource || typeof Blob === 'undefined') return Promise.resolve().then(() => { stats.sync++; return booleanSync(a, b, op); });
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(new Blob([workerSource], { type: 'text/javascript' }));
    const w = new Worker(url); URL.revokeObjectURL(url);
    const timer = setTimeout(() => { w.terminate(); reject(Error('The boolean operation took longer than a minute and was stopped. Try simpler solids.')); }, timeout);
    w.onmessage = e => { clearTimeout(timer); w.terminate(); stats.worker++; if (e.data.error) reject(Error(e.data.error)); else resolve(e.data.result); };
    w.onerror = e => { clearTimeout(timer); w.terminate(); reject(Error(e.message || 'Boolean worker failed.')); };
    w.postMessage({ a: { vertices: a.vertices, faces: a.faces }, b: { vertices: b.vertices, faces: b.faces }, op });
  }).catch(err => { if (/Boolean worker failed|Failed to construct|SecurityError/i.test(err.message)) { stats.sync++; return booleanSync(a, b, op); } throw err; });
}
