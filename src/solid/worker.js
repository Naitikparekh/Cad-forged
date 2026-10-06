// Boolean-operation worker (CSG). Filled in with the 3D port; the build embeds this file as a Blob worker.
self.onmessage = e => { self.postMessage({ id: e.data?.id, error: 'Solid operations are not available yet.' }); };
