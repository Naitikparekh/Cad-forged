// Default command aliases (the acad.pgp set that CadForge supports). Aliases for unsupported commands are skipped.
import { registry } from '../engine/registry.js';

export const DEFAULT_ALIASES = {
  AA: 'AREA', AL: 'ALIGN', AR: 'ARRAY', B: 'BLOCK', BH: 'HATCH', BO: 'BOUNDARY', BR: 'BREAK', C: 'CIRCLE', CH: 'PROPERTIES', CHA: 'CHAMFER',
  COL: 'COLOR', CO: 'COPY', CP: 'COPY', D: 'DIMSTYLE', DAL: 'DIMALIGNED', DAN: 'DIMANGULAR', DBA: 'DIMBASELINE', DCO: 'DIMCONTINUE', DDI: 'DIMDIAMETER',
  DI: 'DIST', DIV: 'DIVIDE', DLI: 'DIMLINEAR', DO: 'DONUT', DR: 'DRAWORDER', DRA: 'DIMRADIUS', DT: 'TEXT', E: 'ERASE', ED: 'TEXTEDIT', EL: 'ELLIPSE',
  EX: 'EXTEND', EXIT: 'QUIT', EXP: 'EXPORT', EXT: 'EXTRUDE', F: 'FILLET', G: 'GROUP', H: 'HATCH', HE: 'HATCHEDIT', I: 'INSERT', IN: 'INTERSECT',
  J: 'JOIN', L: 'LINE', LA: 'LAYER', LE: 'MLEADER', LEN: 'LENGTHEN', LI: 'LIST', LT: 'LINETYPE', LTS: 'LTSCALE', LW: 'LWEIGHT', M: 'MOVE',
  MA: 'MATCHPROP', ME: 'MEASURE', MI: 'MIRROR', MO: 'PROPERTIES', MT: 'MTEXT', O: 'OFFSET', OP: 'OPTIONS', OS: 'OSNAP', P: 'PAN', PE: 'PEDIT',
  PL: 'PLINE', PO: 'POINT', POL: 'POLYGON', PR: 'PROPERTIES', PU: 'PURGE', R: 'REDRAW', RE: 'REGEN', REA: 'REGENALL', REC: 'RECTANG', REN: '-RENAME',
  RO: 'ROTATE', S: 'STRETCH', SC: 'SCALE', SCR: 'SCRIPT', SET: 'SETVAR', SN: 'SNAP', SPL: 'SPLINE', ST: 'STYLE', SU: 'SUBTRACT', T: 'MTEXT',
  TP: 'TOOLPALETTES', TR: 'TRIM', UN: 'UNITS', UNI: 'UNION', V: '-VIEW', W: 'WBLOCK', X: 'EXPLODE', XL: 'XLINE', Z: 'ZOOM',
  // dimension / drafting extras
  QP: 'QUICKPROPERTIES', QSAVE: 'QSAVE', '3DO': '3DORBIT', BEDIT: 'BEDIT', RAY: 'RAY', ID: 'ID', REV: 'REVOLVE', WE: 'WEDGE', CYL: 'CYLINDER', SPH: 'SPHERE', SUB: 'SUBTRACT',
  // natural-language synonyms (acadSynonymsGlobalDB.pgp style)
};
export const DEFAULT_SYNONYMS = {
  BEVEL: 'CHAMFER', SYMBOL: 'BLOCK', RECTANGLE: 'RECTANG', DELETE: 'ERASE', CUT: 'TRIM', LABEL: 'MTEXT', ROUND: 'FILLET', DUPLICATE: 'COPY', STRETCH: 'STRETCH',
  PARALLEL: 'OFFSET', LAYERS: 'LAYER', EXTRUSION: 'EXTRUDE', GRIDON: 'GRID', MEASUREMENT: 'DIST', LEADER: 'MLEADER', SQUARE: 'RECTANG', PRINT: 'PLOT', SAVEAS: 'SAVEAS',
};

export function installAliases() {
  for (const [a, n] of Object.entries(DEFAULT_ALIASES)) if (registry.commands.has(n) && !registry.aliases.has(a) && !registry.commands.has(a)) registry.alias(a, n);
  for (const [w, n] of Object.entries(DEFAULT_SYNONYMS)) if (registry.commands.has(n) && !registry.commands.has(w)) registry.synonyms.set(w, n);
}
