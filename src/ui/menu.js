// Popup menus (context menus, dropdowns, submenus) and rich tooltips.
import { h, svgIcon, clear } from './dom.js';

let openRoots = [];
const root = () => document.getElementById('overlay-root');

export function closeMenus() {
  for (const m of openRoots) m.remove();
  openRoots = [];
  document.removeEventListener('pointerdown', outside, true);
  document.removeEventListener('keydown', keys, true);
  hideTip();
}

function outside(e) { if (!openRoots.some(m => m.contains(e.target))) closeMenus(); }
function keys(e) {
  if (!openRoots.length) return;
  const top = openRoots[openRoots.length - 1];
  if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); if (openRoots.length > 1) { openRoots.pop().remove(); } else closeMenus(); return; }
  const items = [...top.querySelectorAll(':scope > .mi:not(.disabled)')];
  const cur = items.findIndex(i => i.classList.contains('focus'));
  const focus = i => { items.forEach(x => x.classList.remove('focus')); items[i]?.classList.add('focus'); items[i]?.scrollIntoView({ block: 'nearest' }); };
  if (e.key === 'ArrowDown') { e.preventDefault(); focus((cur + 1) % items.length); }
  else if (e.key === 'ArrowUp') { e.preventDefault(); focus((cur - 1 + items.length) % items.length); }
  else if (e.key === 'ArrowRight' && items[cur]?._open) { e.preventDefault(); items[cur]._open(); }
  else if (e.key === 'ArrowLeft' && openRoots.length > 1) { e.preventDefault(); openRoots.pop().remove(); }
  else if (e.key === 'Enter' && items[cur]) { e.preventDefault(); items[cur].click(); }
}

/**
 * Show a menu at (x, y). Items: {label, icon, shortcut, checked, disabled, action, submenu, sep, header}.
 * Returns the menu element.
 */
export function showMenu(items, x, y, opts = {}) {
  if (!opts.sub) closeMenus();
  const menu = h('div.menu', { role: 'menu' });
  if (opts.minWidth) menu.style.minWidth = opts.minWidth + 'px';
  for (const it of items) {
    if (!it) continue;
    if (it.sep) { menu.append(h('div.msep')); continue; }
    if (it.header) { menu.append(h('div.mhead', it.header)); continue; }
    const el = h('div.mi' + (it.disabled ? '.disabled' : ''), { role: 'menuitem' },
      h('span.mi-check', it.checked ? svgIcon('check', 12) : null),
      h('span.mi-ico', it.icon ? svgIcon(it.icon, 16) : null),
      h('span.mi-label', it.html ? { html: it.html } : null, it.html ? null : it.label),
      h('span.mi-short', it.shortcut || ''),
      h('span.mi-arrow', it.submenu ? svgIcon('chevright', 10) : null));
    if (it.tip) attachTip(el, it.tip);
    if (it.submenu) {
      let timer;
      const open = () => {
        clearTimeout(timer);
        while (openRoots[openRoots.length - 1] !== menu && openRoots.length) openRoots.pop().remove();
        const r = el.getBoundingClientRect();
        showMenu(typeof it.submenu === 'function' ? it.submenu() : it.submenu, r.right - 2, r.top - 4, { sub: true });
      };
      el._open = open;
      el.addEventListener('pointerenter', () => { timer = setTimeout(open, 140); });
      el.addEventListener('pointerleave', () => clearTimeout(timer));
      el.addEventListener('click', e => { e.stopPropagation(); open(); });
    } else if (!it.disabled) {
      el.addEventListener('pointerenter', () => { while (openRoots[openRoots.length - 1] !== menu && openRoots.length) openRoots.pop().remove(); });
      el.addEventListener('click', e => { e.stopPropagation(); closeMenus(); it.action?.(); });
    }
    menu.append(el);
  }
  root().append(menu);
  openRoots.push(menu);
  const r = menu.getBoundingClientRect();
  const maxX = window.innerWidth - r.width - 4, maxY = window.innerHeight - r.height - 4;
  menu.style.left = Math.max(2, Math.min(x, maxX)) + 'px';
  menu.style.top = Math.max(2, Math.min(y, maxY)) + 'px';
  if (maxY < 0) { menu.style.maxHeight = window.innerHeight - 8 + 'px'; menu.style.overflowY = 'auto'; }
  if (!opts.sub) { setTimeout(() => { document.addEventListener('pointerdown', outside, true); document.addEventListener('keydown', keys, true); }, 0); }
  return menu;
}

/** Show a dropdown anchored under an element. */
export function dropdown(anchor, items, opts = {}) {
  const r = anchor.getBoundingClientRect();
  return showMenu(items, opts.right ? r.right : r.left, opts.up ? r.top - 4 : r.bottom + 1, { minWidth: Math.max(r.width, opts.minWidth || 0) });
}

// ---------------------------------------------------------------- tooltips

let tip, tipTimer, tipFor;
function ensureTip() { if (!tip) { tip = h('div.tooltip'); root().append(tip); } return tip; }
export function hideTip() { clearTimeout(tipTimer); tipFor = null; if (tip) tip.classList.remove('show'); }

/** spec: {title, desc, cmd, alias, extra} or a function returning one. */
export function attachTip(el, spec, delay = 550) {
  el.addEventListener('pointerenter', () => {
    clearTimeout(tipTimer);
    tipTimer = setTimeout(() => {
      const s = typeof spec === 'function' ? spec() : spec;
      if (!s) return;
      const t = ensureTip();
      clear(t);
      t.append(h('div.tt-title', s.title), s.desc ? h('div.tt-desc', s.desc) : null,
        s.cmd ? h('div.tt-cmd', h('span', 'Command: '), h('b', s.cmd), s.alias ? h('span', `   Alias: ${s.alias}`) : null) : null,
        s.extra ? h('div.tt-extra', s.extra) : null);
      tipFor = el;
      t.classList.add('show');
      const r = el.getBoundingClientRect(), tr = t.getBoundingClientRect();
      let x = r.left, y = r.bottom + 8;
      if (y + tr.height > window.innerHeight - 4) y = r.top - tr.height - 8;
      x = Math.max(4, Math.min(x, window.innerWidth - tr.width - 4));
      t.style.left = x + 'px'; t.style.top = y + 'px';
    }, delay);
  });
  el.addEventListener('pointerleave', () => { if (tipFor === el || !tipFor) hideTip(); });
  el.addEventListener('pointerdown', hideTip);
}
