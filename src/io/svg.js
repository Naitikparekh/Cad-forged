// SVG writer for recorded plot operations.
const f = v => (Math.round(v * 100) / 100).toString();
const esc = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const col = c => (/^rgba?\(/.test(c) ? c : c);

export function writeSVG(ops, page, { unitMm = 0.1, title = 'CadForge drawing' } = {}) {
  const out = [`<?xml version="1.0" encoding="UTF-8"?>`, `<svg xmlns="http://www.w3.org/2000/svg" width="${f(page.w * unitMm)}mm" height="${f(page.h * unitMm)}mm" viewBox="0 0 ${f(page.w)} ${f(page.h)}">`, `<title>${esc(title)}</title>`, `<rect width="100%" height="100%" fill="#ffffff"/>`];
  const d = paths => paths.map(p => 'M' + p.pts.map(([x, y]) => `${f(x)} ${f(y)}`).join('L') + (p.closed ? 'Z' : '')).join('');
  for (const op of ops) {
    const a = op.alpha ?? 1;
    if (a <= 0) continue;
    if (op.t === 'stroke') {
      out.push(`<path d="${d(op.paths)}" fill="none" stroke="${col(op.color)}"${a < 1 ? ` stroke-opacity="${f(a)}"` : ''} stroke-width="${f(Math.max(op.w, 0.01))}" stroke-linecap="${op.cap}" stroke-linejoin="${op.join}"${op.dash.length ? ` stroke-dasharray="${op.dash.map(f).join(' ')}"` : ''}/>`);
    } else if (op.t === 'fill') {
      out.push(`<path d="${d(op.paths)}" fill="${col(op.color)}"${a < 1 ? ` fill-opacity="${f(a)}"` : ''} fill-rule="${op.rule === 'evenodd' ? 'evenodd' : 'nonzero'}" stroke="none"/>`);
    } else if (op.t === 'text') {
      out.push(`<text transform="matrix(${op.m.map(v => Math.round(v * 1e4) / 1e4).join(' ')})" x="${f(op.x)}" y="${f(op.y)}" font-family="Arial, Helvetica, sans-serif" font-size="${f(op.em)}" fill="${col(op.color)}"${a < 1 ? ` fill-opacity="${f(a)}"` : ''}>${esc(op.str)}</text>`);
    }
  }
  out.push('</svg>');
  return out.join('\n');
}
