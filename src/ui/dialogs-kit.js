// Small form-building helpers shared by the dialogs.
import { h } from './dom.js';

export const row = (label, ...ctl) => h('label.frow', h('span.fl', label), h('span.fc', ctl));
export const group = (title, ...kids) => h('fieldset.fgroup', h('legend', title), kids);

export function num(value, o = {}) {
  return h('input.fi.num', { type: 'number', value, step: o.step ?? 'any', min: o.min, max: o.max, 'aria-label': o.label, style: o.width ? { width: o.width + 'px' } : null, disabled: o.disabled });
}
export function text(value = '', o = {}) { return h('input.fi', { type: 'text', value, spellcheck: false, placeholder: o.placeholder, 'aria-label': o.label, style: o.width ? { width: o.width + 'px' } : null }); }

export function select(options, value, o = {}) {
  const el = h('select.fi', { 'aria-label': o.label, style: o.width ? { width: o.width + 'px' } : null },
    options.map(opt => { const v = typeof opt === 'object' ? opt.value : opt, l = typeof opt === 'object' ? opt.label : opt; return h('option', { value: String(v) }, l); }));
  el.value = String(value);
  return el;
}

export function check(label, checked, onchange) {
  const input = h('input', { type: 'checkbox' });
  input.checked = !!checked;
  if (onchange) input.addEventListener('change', () => onchange(input.checked));
  const el = h('label.fchk', input, h('span', label));
  el.input = input;
  return el;
}

/** Tab strip: pages = [[title, element], ...]. */
export function tabs(pages, start = 0) {
  const head = h('div.ftabs', { role: 'tablist' }), body = h('div.fpages');
  const show = i => {
    [...head.children].forEach((b, k) => { b.classList.toggle('active', k === i); b.setAttribute('aria-selected', k === i); });
    [...body.children].forEach((p, k) => { p.hidden = k !== i; });
  };
  pages.forEach(([title, el], i) => { head.append(h('button.ftab', { type: 'button', role: 'tab', onclick: () => show(i) }, title)); el.classList.add('fpage'); body.append(el); });
  show(Math.max(0, Math.min(pages.length - 1, start)));
  const root = h('div.ftabset', head, body);
  root.show = show;
  return root;
}

export const swatch = css => h('span.cswatch', { style: { background: css } });
export const note = t => h('p.fnote', t);
