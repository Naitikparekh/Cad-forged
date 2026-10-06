// Minimal vector PDF writer (PDF 1.4, standard Helvetica) for recorded plot operations.
const WIN_ANSI = { 0x2300: 0xD8, 0x2013: 0x96, 0x2014: 0x97, 0x2018: 0x91, 0x2019: 0x92, 0x201C: 0x93, 0x201D: 0x94, 0x2022: 0x95, 0x20AC: 0x80, 0x2122: 0x99 };
const f = v => { const s = (Math.round(v * 1000) / 1000).toString(); return s === '-0' ? '0' : s; };

function rgb(css) {
  let m = /^#([0-9a-f]{6})$/i.exec(css);
  if (m) { const n = parseInt(m[1], 16); return [(n >> 16 & 255) / 255, (n >> 8 & 255) / 255, (n & 255) / 255]; }
  m = /^#([0-9a-f]{3})$/i.exec(css);
  if (m) return [...m[1]].map(c => parseInt(c + c, 16) / 255);
  m = /^rgba?\(([^)]+)\)/.exec(css);
  if (m) { const p = m[1].split(',').map(Number); return [p[0] / 255, p[1] / 255, p[2] / 255]; }
  return [0, 0, 0];
}

function pdfString(str) {
  let out = '(';
  for (const ch of String(str)) {
    let c = ch.codePointAt(0);
    if (c > 255) c = WIN_ANSI[c] ?? 63;
    if (c === 40 || c === 41 || c === 92) out += '\\' + String.fromCharCode(c);
    else if (c < 32 || c > 126) out += '\\' + c.toString(8).padStart(3, '0');
    else out += String.fromCharCode(c);
  }
  return out + ')';
}

/**
 * @param ops recorded operations in "device units" of `unitMm` millimetres (y down, origin top-left)
 * @param page {w,h} page size in device units
 */
export function writePDF(ops, page, { unitMm = 0.1, title = 'CadForge drawing' } = {}) {
  const pt = unitMm * 72 / 25.4, W = page.w * pt, H = page.h * pt;
  const alphas = new Map(); const gs = a => { const k = Math.round(a * 100) / 100; if (!alphas.has(k)) alphas.set(k, `GS${alphas.size + 1}`); return alphas.get(k); };
  const c = [];
  c.push(`q ${f(pt)} 0 0 ${f(-pt)} 0 ${f(H)} cm`);
  c.push('1 J 1 j');
  const pathOps = paths => { for (const p of paths) { p.pts.forEach(([x, y], i) => c.push(`${f(x)} ${f(y)} ${i ? 'l' : 'm'}`)); if (p.closed) c.push('h'); } };
  for (const op of ops) {
    const a = op.alpha ?? 1;
    if (a <= 0) continue;
    if (a < 1) c.push(`/${gs(a)} gs`); else if (alphas.size) c.push('/GS0 gs');
    if (op.t === 'stroke') {
      const [r, g, b] = rgb(op.color);
      c.push(`${f(r)} ${f(g)} ${f(b)} RG ${f(Math.max(op.w, 0.01))} w ${op.cap === 'round' ? 1 : op.cap === 'square' ? 2 : 0} J ${op.join === 'round' ? 1 : op.join === 'bevel' ? 2 : 0} j`);
      c.push(op.dash.length ? `[${op.dash.map(f).join(' ')}] 0 d` : '[] 0 d');
      pathOps(op.paths); c.push('S');
    } else if (op.t === 'fill') {
      const [r, g, b] = rgb(op.color);
      c.push(`${f(r)} ${f(g)} ${f(b)} rg`); pathOps(op.paths); c.push(op.rule === 'evenodd' ? 'f*' : 'f');
    } else if (op.t === 'text') {
      const [r, g, b] = rgb(op.color), m = op.m;
      c.push(`q ${m.map(f).join(' ')} cm 1 0 0 -1 0 0 cm BT /F1 ${f(op.em)} Tf ${f(r)} ${f(g)} ${f(b)} rg ${f(op.x)} ${f(-op.y)} Td ${pdfString(op.str)} Tj ET Q`);
    }
  }
  c.push('Q');
  const content = c.join('\n');
  // alpha 1 reset state
  const gsDict = [`/GS0 << /ca 1 /CA 1 >>`, ...[...alphas].map(([a, name]) => `/${name} << /ca ${a} /CA ${a} >>`)].join(' ');
  const objs = [
    '<< /Type /Catalog /Pages 2 0 R >>',
    '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
    `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${f(W)} ${f(H)}] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> /ExtGState << ${gsDict} >> >> >>`,
    `<< /Length ${content.length} >>\nstream\n${content}\nendstream`,
    '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>',
    `<< /Title ${pdfString(title)} /Producer (CadForge) /Creator (CadForge) >>`,
  ];
  let out = '%PDF-1.4\n%\xE2\xE3\xCF\xD3\n';
  const offsets = [];
  objs.forEach((o, i) => { offsets.push(out.length); out += `${i + 1} 0 obj\n${o}\nendobj\n`; });
  const xref = out.length;
  out += `xref\n0 ${objs.length + 1}\n0000000000 65535 f \n` + offsets.map(o => String(o).padStart(10, '0') + ' 00000 n \n').join('');
  out += `trailer\n<< /Size ${objs.length + 1} /Root 1 0 R /Info 6 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  // all characters are <= 0xFF, so a latin-1 byte mapping preserves the offsets
  const bytes = new Uint8Array(out.length);
  for (let i = 0; i < out.length; i++) bytes[i] = out.charCodeAt(i) & 255;
  return bytes;
}
