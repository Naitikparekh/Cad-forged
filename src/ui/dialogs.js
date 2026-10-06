// Modal dialog framework. Each dialog definition receives {ui, app, doc, data, close} and returns
// {title, width, body, ok?, cancel?, commit?, buttons?}. commit() returns the dialog result, or false to stay open.
import { h, svgIcon } from './dom.js';
import { closeMenus } from './menu.js';
import { BASIC } from './dialogs-basic.js';
import { SETTINGS } from './dialogs-settings.js';
import { PLOT } from './dialogs-plot.js';

const DEFS = { ...BASIC, ...SETTINGS, ...PLOT };

export class Dialogs {
  constructor(ui) { this.ui = ui; this.app = ui.app; this.stack = []; }

  top() { return this.stack[this.stack.length - 1] || null; }
  cancelTop() { const t = this.top(); if (t) t.finish(t.spec.cancelValue ?? null); }

  open(name, data = {}) {
    const def = DEFS[name];
    if (!def) { this.ui.toast(`The ${name} dialog is not available.`); return Promise.resolve(null); }
    closeMenus();
    return new Promise(resolve => {
      const entry = { name };
      const api = { ui: this.ui, app: this.app, doc: this.app.doc, data, close: v => entry.finish(v) };
      const spec = def(api);
      entry.spec = spec;
      const titleEl = h('div.dlg-title', { onpointerdown: e => this.drag(e, win) }, h('span', spec.title), h('button.dlg-x', { type: 'button', 'aria-label': 'Close', onclick: () => entry.finish(spec.cancelValue ?? null) }, svgIcon('x', 10)));
      const buttons = h('div.dlg-btns');
      for (const b of spec.buttons || []) buttons.append(h('button.btn', { type: 'button', onclick: () => { const r = b.action?.(); if (r !== false && b.close !== false) entry.finish(r); } }, b.label));
      if (spec.ok !== null) buttons.append(h('button.btn.primary', { type: 'button', onclick: () => entry.commit() }, spec.ok || 'OK'));
      if (spec.cancel !== null) buttons.append(h('button.btn', { type: 'button', onclick: () => entry.finish(spec.cancelValue ?? null) }, spec.cancel || 'Cancel'));
      const win = h('div.dlg', { role: 'dialog', 'aria-modal': 'true', 'aria-label': spec.title, style: { width: (spec.width || 420) + 'px' } }, titleEl, h('div.dlg-body', spec.body), buttons);
      const back = h('div.dlg-back', win);
      let done = false;
      entry.commit = () => { const r = spec.commit ? spec.commit() : true; if (r !== false) entry.finish(r); };
      entry.finish = v => {
        if (done) return; done = true;
        spec.onClose?.(v);
        back.remove(); this.stack = this.stack.filter(x => x !== entry);
        this.ui.viewport?.invalidate();
        resolve(v);
        if (!this.stack.length) (this.ui.app.runtime.busy ? this.ui.cmdline.focus() : this.ui.viewport.canvasFocus());
      };
      win.addEventListener('keydown', e => {
        if (e.key === 'Enter' && !(e.target instanceof HTMLTextAreaElement) && !(e.target instanceof HTMLSelectElement) && !e.target.closest('button') && spec.ok !== null) { e.preventDefault(); entry.commit(); }
        e.stopPropagation();
      });
      this.ui.els.overlay.append(back);
      this.stack.push(entry);
      const first = win.querySelector('input:not([type=checkbox]),select,textarea') || win.querySelector('.btn.primary');
      setTimeout(() => { first?.focus?.(); if (first instanceof HTMLInputElement && first.type !== 'number') first.select?.(); spec.onOpen?.(win); }, 0);
    });
  }

  drag(e, win) {
    if (e.target.closest('button')) return;
    const r = win.getBoundingClientRect(), dx = e.clientX - r.left, dy = e.clientY - r.top;
    win.style.position = 'absolute'; win.style.left = r.left + 'px'; win.style.top = r.top + 'px'; win.style.margin = '0';
    const mv = ev => { win.style.left = Math.max(0, Math.min(innerWidth - 60, ev.clientX - dx)) + 'px'; win.style.top = Math.max(0, Math.min(innerHeight - 30, ev.clientY - dy)) + 'px'; };
    const up = () => { document.removeEventListener('pointermove', mv); document.removeEventListener('pointerup', up); };
    document.addEventListener('pointermove', mv); document.addEventListener('pointerup', up);
  }
}
