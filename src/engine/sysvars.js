// System variables. Application preferences live in `prefs`; drawing variables are backed by the Drawing.
import { LINETYPES, ltypeKey, LW_BYLAYER, LINEWEIGHTS } from '../core/style.js';

export const PREF_DEFAULTS = {
  OSMODE: 4133, AUTOSNAP: 63, APERTURE: 10, PICKBOX: 3, GRIPSIZE: 5, GRIPS: 2, GRIPOBJLIMIT: 100, PICKFIRST: 1, PICKADD: 2, PICKAUTO: 1, PICKDRAG: 0,
  CURSORSIZE: 5, DYNMODE: 3, POLARANG: 90, ORTHOMODE: 0, SNAPMODE: 0, GRIDMODE: 1, GRIDMAJOR: 5, GRIDUNIT_X: 10, GRIDUNIT_Y: 10, SNAPUNIT_X: 10, SNAPUNIT_Y: 10,
  SNAPANG: 0, LWDISPLAY: 0, TRANSPARENCYDISPLAY: 1, FILLMODE: 1, HIGHLIGHT: 1, SELECTIONPREVIEW: 3, VPCONTROL: 1, MENUBAR: 0, LAYLOCKFADECTL: 50,
  MAXHATCH: 60000, TOOLTIPS: 1, NAVVCUBE: 1, NAVBAR: 1, UCSICON: 1, QPMODE: -1, OTRACKMODE: 1, POLARMODE: 0, SHORTCUTMENU: 11, INPUTHISTORYMODE: 15,
  AUTOCOMPLETE: 1, INPUTSEARCHDELAY: 300, THEME: 'dark', BACKGROUND: '', PLINEWID: 0,
};

const flag = v => (v ? 1 : 0);

/** Variables backed by the drawing: [get(doc), set(doc, value), type] */
const DRAWING_VARS = {
  LTSCALE: [d => d.settings.ltscale, (d, v) => { d.settings.ltscale = pos(v); }, 'real'],
  CELTSCALE: [d => d.settings.celtscale, (d, v) => { d.settings.celtscale = pos(v); }, 'real'],
  PDMODE: [d => d.settings.pdmode, (d, v) => { d.settings.pdmode = v; }, 'int'],
  PDSIZE: [d => d.settings.pdsize, (d, v) => { d.settings.pdsize = v; }, 'real'],
  TEXTSIZE: [d => d.settings.textHeight, (d, v) => { d.settings.textHeight = pos(v); }, 'real'],
  FILLETRAD: [d => d.settings.filletRadius, (d, v) => { d.settings.filletRadius = Math.max(0, v); }, 'real'],
  CHAMFERA: [d => d.settings.chamferD1, (d, v) => { d.settings.chamferD1 = Math.max(0, v); }, 'real'],
  CHAMFERB: [d => d.settings.chamferD2, (d, v) => { d.settings.chamferD2 = Math.max(0, v); }, 'real'],
  OFFSETDIST: [d => d.settings.offsetGap, (d, v) => { d.settings.offsetGap = v; }, 'real'],
  LUPREC: [d => d.settings.units.precision, (d, v) => { d.settings.units.precision = clampInt(v, 0, 8); }, 'int'],
  AUPREC: [d => d.settings.units.angPrecision, (d, v) => { d.settings.units.angPrecision = clampInt(v, 0, 8); }, 'int'],
  INSUNITS: [d => d.settings.units.insUnits, (d, v) => { d.settings.units.insUnits = clampInt(v, 0, 24); }, 'int'],
  CLAYER: [d => d.cur.layer, (d, v) => d.setCurrentLayer(v), 'string'],
  CECOLOR: [d => colorText(d.cur.color), (d, v) => { d.cur.color = parseColor(v); }, 'string'],
  CELTYPE: [d => d.cur.ltype ?? 'BYLAYER', (d, v) => { d.cur.ltype = /^bylayer$/i.test(v) ? undefined : (ltypeKey(v) || badValue(v)); }, 'string'],
  CELWEIGHT: [d => d.cur.lw ?? LW_BYLAYER, (d, v) => { d.cur.lw = v === LW_BYLAYER ? undefined : (LINEWEIGHTS.includes(v) || v === -2 || v === -3 ? v : badValue(v)); }, 'int'],
  DIMSCALE: [d => dim(d).scale, (d, v) => { dim(d).scale = pos(v); }, 'real'],
  DIMTXT: [d => dim(d).txt, (d, v) => { dim(d).txt = pos(v); }, 'real'],
  DIMASZ: [d => dim(d).asz, (d, v) => { dim(d).asz = Math.max(0, v); }, 'real'],
  DIMEXO: [d => dim(d).exo, (d, v) => { dim(d).exo = Math.max(0, v); }, 'real'],
  DIMEXE: [d => dim(d).exe, (d, v) => { dim(d).exe = Math.max(0, v); }, 'real'],
  DIMGAP: [d => dim(d).gap, (d, v) => { dim(d).gap = Math.max(0, v); }, 'real'],
  DIMDEC: [d => dim(d).dec, (d, v) => { dim(d).dec = clampInt(v, 0, 8); }, 'int'],
  DIMCEN: [d => dim(d).cen, (d, v) => { dim(d).cen = Math.max(0, v); }, 'real'],
  DIMTAD: [d => dim(d).tad, (d, v) => { dim(d).tad = flag(v); }, 'int'],
};

const dim = d => d.settings.dimStyles[d.settings.curDimStyle];
const pos = v => v > 0 ? v : badValue(v);
const clampInt = (v, lo, hi) => { if (!Number.isInteger(v) || v < lo || v > hi) badValue(v); return v; };
function badValue(v) { throw Error(`Requires a valid value; ${v} is not allowed.`); }
const colorText = c => c === undefined ? 'BYLAYER' : c === 'BYBLOCK' ? 'BYBLOCK' : String(c);
export function parseColor(v) {
  const t = String(v).trim().toUpperCase();
  if (t === 'BYLAYER') return undefined;
  if (t === 'BYBLOCK') return 'BYBLOCK';
  if (/^#[0-9A-F]{6}$/.test(t)) return t.toLowerCase();
  const names = { RED: 1, YELLOW: 2, GREEN: 3, CYAN: 4, BLUE: 5, MAGENTA: 6, WHITE: 7 };
  if (names[t]) return names[t];
  const n = Number(t);
  if (Number.isInteger(n) && n >= 1 && n <= 255) return n;
  const rgb = t.match(/^(\d{1,3}),(\d{1,3}),(\d{1,3})$/);
  if (rgb && rgb.slice(1).every(x => +x <= 255)) return '#' + rgb.slice(1).map(x => (+x).toString(16).padStart(2, '0')).join('');
  return badValue(v);
}

const READ_ONLY = {
  DWGNAME: d => d.name, CMDNAMES: () => '', USERR1: () => 0, LASTPOINT: () => '', LASTANGLE: () => 0, PERIMETER: () => 0, AREA: () => 0,
  VPOINTX: () => 0, VPOINTY: () => 0, VPOINTZ: () => 1, CTAB: d => d.space === 'model' ? 'Model' : d.space,
};

export class Sysvars {
  constructor(getDoc, prefs = {}) {
    this.getDoc = getDoc;
    this.prefs = { ...PREF_DEFAULTS, ...prefs };
    this.listeners = new Set();
  }

  onChange(fn) { this.listeners.add(fn); return () => this.listeners.delete(fn); }
  notify(name) { for (const fn of this.listeners) fn(name); }

  has(name) { name = name.toUpperCase(); return name in this.prefs || name in DRAWING_VARS || name in READ_ONLY; }
  names() { return [...new Set([...Object.keys(this.prefs), ...Object.keys(DRAWING_VARS), ...Object.keys(READ_ONLY)])].filter(n => !n.includes('_')).sort(); }

  type(name) {
    name = name.toUpperCase();
    if (DRAWING_VARS[name]) return DRAWING_VARS[name][2];
    if (name in READ_ONLY) return 'string';
    const v = this.prefs[name];
    return typeof v === 'string' ? 'string' : Number.isInteger(v) ? 'int' : 'real';
  }

  isReadOnly(name) { return name.toUpperCase() in READ_ONLY; }

  get(name) {
    name = name.toUpperCase();
    if (DRAWING_VARS[name]) return DRAWING_VARS[name][0](this.getDoc());
    if (READ_ONLY[name]) return READ_ONLY[name](this.getDoc());
    if (name in this.prefs) return this.prefs[name];
    throw Error(`Unknown variable name ${name}.`);
  }

  set(name, value) {
    name = name.toUpperCase();
    if (READ_ONLY[name]) throw Error(`${name} is read-only.`);
    if (DRAWING_VARS[name]) {
      const [, set, type] = DRAWING_VARS[name];
      const v = type === 'string' ? String(value) : Number(value);
      if (type !== 'string' && !Number.isFinite(v)) badValue(value);
      if (type === 'int' && !Number.isInteger(v)) badValue(value);
      const doc = this.getDoc();
      doc.willChange(); set(doc, v); doc.touch();
    } else if (name in this.prefs) {
      const cur = this.prefs[name];
      if (typeof cur === 'string') this.prefs[name] = String(value);
      else {
        const v = Number(value);
        if (!Number.isFinite(v)) badValue(value);
        this.prefs[name] = Number.isInteger(cur) && !Number.isInteger(v) && !['LAYLOCKFADECTL'].includes(name) ? Math.round(v) : v;
      }
    } else throw Error(`Unknown variable name ${name}.`);
    this.notify(name);
  }

  /** Convenience flags used throughout the engine. */
  get ortho() { return !!this.prefs.ORTHOMODE; }
  get polar() { return !!this.prefs.POLARMODE; }
  get otrack() { return !!this.prefs.OTRACKMODE; }
  get osnapOn() { return !(this.prefs.OSMODE & 16384); }
  get gridOn() { return !!this.prefs.GRIDMODE; }
  get snapOn() { return !!this.prefs.SNAPMODE; }
  get dyn() { return this.prefs.DYNMODE; }
  get pointerInput() { return (this.prefs.DYNMODE & 1) === 1; }
  get dimensionInput() { return (this.prefs.DYNMODE & 2) === 2; }
}

void LINETYPES;
