// Colors (AutoCAD Color Index + true color), linetypes, lineweights and default styles.

// ---------------------------------------------------------------- AutoCAD Color Index

const STD = ['#000000', '#ff0000', '#ffff00', '#00ff00', '#00ffff', '#0000ff', '#ff00ff', '#ffffff', '#808080', '#c0c0c0'];
const GRAYS = ['#333333', '#505050', '#696969', '#828282', '#bebebe', '#ffffff'];

function hsv(h, s, v) {
  const c = v * s, x = c * (1 - Math.abs((h / 60) % 2 - 1)), m = v - c;
  const [r, g, b] = h < 60 ? [c, x, 0] : h < 120 ? [x, c, 0] : h < 180 ? [0, c, x] : h < 240 ? [0, x, c] : h < 300 ? [x, 0, c] : [c, 0, x];
  const hex = n => Math.floor((n + m) * 255 + 1e-6).toString(16).padStart(2, '0');
  return '#' + hex(r) + hex(g) + hex(b);
}

const ACI = new Array(256);
for (let i = 0; i < 10; i++) ACI[i] = STD[i];
for (let i = 10; i < 250; i++) {
  const hue = Math.floor((i - 10) / 10) * 15, k = (i - 10) % 10;
  ACI[i] = hsv(hue, k % 2 ? .5 : 1, [1, 1, .8, .8, .6, .6, .5, .5, .3, .3][k]);
}
for (let i = 250; i < 256; i++) ACI[i] = GRAYS[i - 250];

export const aciHex = i => ACI[i] || '#ffffff';
export const ACI_NAMES = { 1: 'Red', 2: 'Yellow', 3: 'Green', 4: 'Cyan', 5: 'Blue', 6: 'Magenta', 7: 'White' };

/** Nearest ACI index for a hex color (used when writing DXF). */
export function hexToAci(hex) {
  const n = parseInt(hex.slice(1), 16), r = n >> 16 & 255, g = n >> 8 & 255, b = n & 255;
  let best = 7, bd = Infinity;
  for (let i = 1; i < 256; i++) {
    const c = parseInt(ACI[i].slice(1), 16), d = (r - (c >> 16 & 255)) ** 2 + (g - (c >> 8 & 255)) ** 2 + (b - (c & 255)) ** 2;
    if (d < bd) { bd = d; best = i; }
  }
  return best;
}

/**
 * Resolve an entity/layer color value to CSS.
 *   value: undefined|'BYLAYER'|'BYBLOCK'|aci number|'#rrggbb'
 * ACI 7 is white on dark backgrounds and black on light ones (and always black when plotting).
 */
export function colorToCss(value, { dark = true } = {}) {
  if (typeof value === 'string' && value[0] === '#') return value;
  if (value === 7 || value === undefined) return dark ? '#ffffff' : '#000000';
  return aciHex(value);
}

export function describeColor(v) {
  if (v === undefined || v === 'BYLAYER') return 'ByLayer';
  if (v === 'BYBLOCK') return 'ByBlock';
  if (typeof v === 'number') return ACI_NAMES[v] || String(v);
  return v.toUpperCase();
}

// ---------------------------------------------------------------- linetypes

/** Pattern arrays in drawing units at LTSCALE 1: positive = dash, negative = gap, 0 = dot. */
export const LINETYPES = {
  Continuous: { desc: 'Solid line', pattern: [] },
  DASHED: { desc: 'Dashed __ __ __ __ __ __', pattern: [.5, -.25] },
  HIDDEN: { desc: 'Hidden __ __ __ __ __ __', pattern: [.25, -.125] },
  CENTER: { desc: 'Center ____ _ ____ _ ____', pattern: [1.25, -.25, .25, -.25] },
  PHANTOM: { desc: 'Phantom ______ __ __ ______', pattern: [1.25, -.25, .25, -.25, .25, -.25] },
  DASHDOT: { desc: 'Dash dot __ . __ . __ .', pattern: [.5, -.25, 0, -.25] },
  DOT: { desc: 'Dot . . . . . . . . . .', pattern: [0, -.25] },
  BORDER: { desc: 'Border __ __ . __ __ .', pattern: [.5, -.25, .5, -.25, 0, -.25] },
  DIVIDE: { desc: 'Divide __ . . __ . . __', pattern: [.5, -.25, 0, -.25, 0, -.25] },
};

export const ltypeKey = name => Object.keys(LINETYPES).find(k => k.toLowerCase() === String(name).toLowerCase());
export const ltypeDisplay = name => name === 'Continuous' ? 'Continuous' : name[0] + name.slice(1).toLowerCase();

// ---------------------------------------------------------------- lineweights (hundredths of mm)

export const LINEWEIGHTS = [0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158, 200, 211];
export const LW_BYLAYER = -1, LW_BYBLOCK = -2, LW_DEFAULT = 25;
export const lwDisplay = v => v === undefined || v === LW_BYLAYER ? 'ByLayer' : v === LW_BYBLOCK ? 'ByBlock' : v === -3 ? 'Default' : (v / 100).toFixed(2) + ' mm';
/** Screen width in CSS pixels for a lineweight (hundredths of mm). */
export const lwPixels = (v, lwtOn) => !lwtOn ? 1 : Math.max(1, Math.round(v / 100 * 96 / 25.4 * .9));

// ---------------------------------------------------------------- styles

export function defaultDimStyle() {
  return {
    name: 'Standard',
    scale: 1,        // DIMSCALE
    txt: 2.5,        // DIMTXT text height
    asz: 2.5,        // DIMASZ arrow size
    exo: .625,       // DIMEXO extension offset
    exe: 1.25,       // DIMEXE extension beyond dim line
    gap: .625,       // DIMGAP text gap
    cen: 2.5,        // DIMCEN centre mark size
    dec: 2,          // DIMDEC decimal places
    tad: 1,          // text above dimension line (1) or centered (0)
    tih: false,      // text inside horizontal
    zin: 0,          // DIMZIN: 8 = suppress trailing zeros
    lfac: 1,         // DIMLFAC linear scale factor
    pre: '', suf: '',// prefix / suffix
    blk: 'closed',   // arrowhead: closed | open | dot | tick | none
    color: undefined,
  };
}

export function defaultSettings() {
  return {
    units: { type: 'decimal', precision: 4, angPrecision: 0, insUnits: 4 },   // insUnits 4 = millimetres (DXF $INSUNITS)
    ltscale: 1,
    celtscale: 1,
    pdmode: 0,
    pdsize: 0,
    textHeight: 2.5,
    textStyle: 'Standard',
    dimStyles: { Standard: defaultDimStyle() },
    curDimStyle: 'Standard',
    filletRadius: 0,
    chamferD1: 0,
    chamferD2: 0,
    offsetGap: 0,
    plinewid: 0,
  };
}

export function newLayer(name, props = {}) {
  return { name, color: 7, ltype: 'Continuous', lw: LW_DEFAULT, on: true, frozen: false, locked: false, plot: true, desc: '', ...props };
}

// ---------------------------------------------------------------- number formatting

export function fmtNum(x, precision = 4, trim = false) {
  if (!Number.isFinite(x)) return String(x);
  if (Math.abs(x) < 0.5 * Math.pow(10, -precision)) x = 0;
  let s = x.toFixed(precision);
  if (trim && s.includes('.')) s = s.replace(/0+$/, '').replace(/\.$/, '');
  return s;
}

export const fmtAngle = (radians, precision = 0) => fmtNum(radians * 180 / Math.PI, precision) + '°';
