// Boolean-operation worker: bundled by build.mjs and started from an embedded Blob.
import { booleanSync } from './boolean.js';

self.onmessage = e => {
  try { self.postMessage({ result: booleanSync(e.data.a, e.data.b, e.data.op) }); }
  catch (err) { self.postMessage({ error: err.message || String(err) }); }
};
