// Tiny DOM helpers.
import { icon } from './icons.js';

/** h('div.cls#id', {onclick, dataset:{}, style:{}}, ...children) */
export function h(spec, props, ...kids) {
  if (props && (typeof props === 'string' || props instanceof Node || Array.isArray(props))) { kids.unshift(props); props = null; }
  const m = spec.match(/^([a-z0-9-]*)((?:[.#][\w-]+)*)$/i);
  const el = document.createElement(m?.[1] || 'div');
  for (const t of (m?.[2] || '').match(/[.#][\w-]+/g) || []) { if (t[0] === '#') el.id = t.slice(1); else el.classList.add(t.slice(1)); }
  for (const [k, v] of Object.entries(props || {})) {
    if (v === undefined || v === null || v === false) continue;
    if (k === 'style' && typeof v === 'object') Object.assign(el.style, v);
    else if (k === 'dataset') Object.assign(el.dataset, v);
    else if (k === 'class') el.className += ' ' + v;
    else if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2), v);
    else if (k === 'html') el.innerHTML = v;
    else if (v === true) el.setAttribute(k, '');
    else el.setAttribute(k, v);
  }
  append(el, kids);
  return el;
}

export function append(el, kids) {
  for (const k of kids.flat(Infinity)) {
    if (k === null || k === undefined || k === false) continue;
    el.append(k instanceof Node ? k : document.createTextNode(String(k)));
  }
  return el;
}

/** Element holding an inline SVG icon. */
export const ico = (name, size = 16) => { const t = document.createElement('span'); t.className = 'ico-wrap'; t.innerHTML = icon(name, size); return t.firstElementChild ? Object.assign(t, { }) : t; };
export const svgIcon = (name, size = 16) => { const t = document.createElement('template'); t.innerHTML = icon(name, size); return t.content.firstElementChild; };

export const $ = (sel, root = document) => root.querySelector(sel);
export const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
export const clear = el => { while (el.firstChild) el.removeChild(el.firstChild); return el; };

export const debounce = (fn, ms) => { let t; return (...a) => { clearTimeout(t); t = setTimeout(() => fn(...a), ms); }; };

/** Run `fn` at most once per animation frame. */
export const raf = fn => { let queued = false; return () => { if (queued) return; queued = true; requestAnimationFrame(() => { queued = false; fn(); }); }; };
