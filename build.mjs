// Bundles src/ into one self-contained CadForge.html (no network, no server).
//   node build.mjs          -> CadForge.html
//   node build.mjs --min    -> minified
import { build } from 'esbuild';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(fileURLToPath(import.meta.url));
const minify = process.argv.includes('--min');
const out = path.join(root, process.argv.find(a => a.endsWith('.html')) || 'CadForge.html');

const bundle = async entry => {
  const r = await build({
    entryPoints: [path.join(root, entry)],
    bundle: true,
    format: 'iife',
    target: 'es2022',
    write: false,
    minify,
    legalComments: 'none',
    logLevel: 'warning',
  });
  return r.outputFiles[0].text;
};

// The boolean worker is embedded as inert text and turned into a Blob worker at runtime.
const worker = await bundle('src/solid/worker.js');
const main = await bundle('src/main.js');

const cssDir = path.join(root, 'src/styles');
const css = fs.readdirSync(cssDir).filter(f => f.endsWith('.css')).sort()
  .map(f => fs.readFileSync(path.join(cssDir, f), 'utf8')).join('\n');

const escapeScript = s => s.replace(/<\/script/gi, '<\\/script').replace(/<!--/g, '<\\!--');
let html = fs.readFileSync(path.join(root, 'src/index.html'), 'utf8');
html = html
  .replace('/*__CSS__*/', () => css)
  .replace('/*__WORKER__*/', () => escapeScript(worker))
  .replace('/*__JS__*/', () => escapeScript(main));
fs.writeFileSync(out, html);
console.log(`built ${path.relative(root, out)} (${(html.length / 1024).toFixed(0)} KB)`);
