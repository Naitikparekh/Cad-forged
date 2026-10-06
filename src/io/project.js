// CadForge project files (JSON, format version 2). Legacy v1 projects are migrated on load.
import { Drawing } from '../core/drawing.js';

export const FILE_EXT = '.cadforge.json';

export function stripExt(name) { return String(name).replace(/\.(cadforge\.json|json|cadforge|dxf)$/i, ''); }

export function toJSON(doc) { return JSON.stringify(doc.serialize()); }

export function fromJSON(text, name) {
  let obj;
  try { obj = JSON.parse(text); } catch { throw Error('This file is not valid JSON.'); }
  return Drawing.deserialize(obj, name);
}
