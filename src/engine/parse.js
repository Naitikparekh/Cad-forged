// Parsing of command-line input: coordinates, numbers, object-snap overrides and keyword options.
import { polar, rad } from '../core/geom.js';

const NUM = '[+-]?(?:\\d+\\.?\\d*|\\.\\d+)(?:[eE][+-]?\\d+)?';
const reNum = new RegExp('^' + NUM + '$');
const reCart = new RegExp(`^(${NUM})\\s*,\\s*(${NUM})(?:\\s*,\\s*(${NUM}))?$`);
const rePolar = new RegExp(`^(${NUM})\\s*<\\s*(${NUM})$`);

export const parseNumber = text => {
  const t = String(text).trim();
  if (reNum.test(t)) return Number(t);
  const frac = t.match(/^([+-]?)(?:(\d+)[\s-])?(\d+)\/(\d+)$/);        // 1/2, 3-1/4, -2 1/8
  if (frac && Number(frac[4]) !== 0) return (frac[1] === '-' ? -1 : 1) * (Number(frac[2] || 0) + Number(frac[3]) / Number(frac[4]));
  return null;
};

export const OS_BITS = { END: 1, MID: 2, CEN: 4, NOD: 8, QUA: 16, INT: 32, INS: 64, PER: 128, TAN: 256, NEA: 512, QUI: 1024, APP: 2048, EXT: 4096, PAR: 8192 };
export const OS_FULL = {
  END: 'Endpoint', MID: 'Midpoint', CEN: 'Center', NOD: 'Node', QUA: 'Quadrant', INT: 'Intersection', INS: 'Insertion',
  PER: 'Perpendicular', TAN: 'Tangent', NEA: 'Nearest', APP: 'Apparent Intersection', EXT: 'Extension', PAR: 'Parallel',
};
const OS_ALIASES = { ENDPOINT: 'END', MIDPOINT: 'MID', CENTER: 'CEN', CENTRE: 'CEN', NODE: 'NOD', QUADRANT: 'QUA', INTERSECTION: 'INT', INSERTION: 'INS', INSERT: 'INS', PERPENDICULAR: 'PER', TANGENT: 'TAN', NEAREST: 'NEA', APPINT: 'APP', EXTENSION: 'EXT', PARALLEL: 'PAR' };

/** Recognise typed object-snap overrides such as END, MID, CEN, FROM, TT, NONE. */
export function parseOverride(text) {
  const t = String(text).trim().toUpperCase();
  const three = t.slice(0, 3);
  if (OS_BITS[three] && (t.length === 3 || OS_ALIASES[t] === three)) return { kind: 'osnap', mode: three };
  if (OS_ALIASES[t]) return { kind: 'osnap', mode: OS_ALIASES[t] };
  if (['FROM', 'FRO'].includes(t)) return { kind: 'from' };
  if (['TT', 'TRACK'].includes(t)) return { kind: 'tt' };
  if (['NON', 'NONE'].includes(t)) return { kind: 'none' };
  if (['M2P', 'MTP'].includes(t)) return { kind: 'm2p' };
  return null;
}

/**
 * Interpret typed text as a point or a bare number.
 * @param opts {base?:{x,y}, relativeDefault?:boolean}  relativeDefault: dynamic-input rule (2nd point is relative unless '#')
 * @returns {{kind:'point',pt}|{kind:'number',value}|null}
 */
export function parseInput(text, { base = null, relativeDefault = false } = {}) {
  let t = String(text).trim();
  if (!t) return null;
  let forceAbs = false, forceRel = false;
  if (t[0] === '#') { forceAbs = true; t = t.slice(1).trim(); }
  else if (t[0] === '@') { forceRel = true; t = t.slice(1).trim(); }
  const rel = !forceAbs && (forceRel || (relativeDefault && !!base));
  const origin = rel ? (base || { x: 0, y: 0 }) : { x: 0, y: 0 };
  if (forceRel && t === '') return { kind: 'point', pt: { ...origin } };                // "@" alone = last point
  let m = t.match(reCart);
  if (m) return { kind: 'point', pt: { x: origin.x + Number(m[1]), y: origin.y + Number(m[2]) }, z: m[3] !== undefined ? Number(m[3]) : undefined };
  m = t.match(rePolar);
  if (m) return { kind: 'point', pt: polar(origin, rad(Number(m[2])), Number(m[1])), polar: { d: Number(m[1]), a: rad(Number(m[2])) } };
  const n = parseNumber(t);
  if (n !== null && !forceAbs && !forceRel) return { kind: 'number', value: n };
  return null;
}

// ---------------------------------------------------------------- keyword options

/** Abbreviation of an option label: its capital letters and digits (e.g. "Ttr" -> "T", "eXit" -> "X", "3P" -> "3P"). */
export function optionKey(label) {
  const main = label.replace(/\s*\(.*\)\s*$/, '');
  const caps = main.replace(/[^A-Z0-9]/g, '');
  return caps || main[0].toUpperCase();
}

export const optionName = label => label.replace(/\s*\(.*\)\s*$/, '');

/** Match typed text to one of the option labels; returns the label or null. */
export function matchOption(input, options) {
  const t = String(input).trim().toLowerCase();
  if (!t) return null;
  for (const o of options) if (optionName(o).toLowerCase() === t) return o;
  for (const o of options) if (optionKey(o).toLowerCase() === t) return o;
  const prefixed = options.filter(o => optionName(o).toLowerCase().startsWith(t));
  return prefixed.length === 1 ? prefixed[0] : null;
}

/** "[Close/Undo]" style suffix for a prompt. */
export function optionsText(options) {
  return options.length ? ' [' + options.map(o => o.replace(/\s*\(.*\)\s*$/, m => m)).join('/') + ']' : '';
}
