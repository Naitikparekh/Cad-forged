// AutoCAD-style text window (F2): the full command history in a floating window.
import { h, svgIcon, clear } from './dom.js';

export class TextWindow {
  constructor(ui, root) {
    this.ui = ui; this.app = ui.app;
    this.list = h('div.tw-list', { role: 'log' });
    this.input = h('input.tw-input', { type: 'text', 'aria-label': 'Command', placeholder: 'Type a command', spellcheck: false, onkeydown: e => { e.stopPropagation(); if (e.key === 'Enter') { const v = this.input.value; this.input.value = ''; this.app.runtime.text(v); } if (e.key === 'Escape') this.ui.escape(); } });
    this.el = h('div.textwin', { hidden: true },
      h('div.tw-title', { onpointerdown: e => this.drag(e) }, h('span', 'CadForge Text Window — ' + 'Drawing'), h('button.tw-x', { 'aria-label': 'Close text window', onclick: () => this.toggle(false) }, svgIcon('x', 10))),
      this.list, h('div.tw-row', h('span.tw-prompt', 'Command:'), this.input));
    root.append(this.el);
    this.title = this.el.querySelector('.tw-title span');
  }

  toggle(show = this.el.hidden) {
    this.el.hidden = !show;
    if (show) {
      clear(this.list);
      for (const l of this.app.history.slice(-300)) this.append(l, true);
      this.title.textContent = `CadForge Text Window — ${this.app.doc?.name ?? ''}`;
      this.list.scrollTop = this.list.scrollHeight;
      this.input.focus();
    }
  }

  append({ text, kind }, bulk) {
    if (this.el.hidden && !bulk) return;
    for (const part of String(text).split('\n')) this.list.append(h('div.cl-line.' + kind, part));
    if (!bulk) this.list.scrollTop = this.list.scrollHeight;
  }

  drag(e) {
    if (e.target.closest('button')) return;
    const r = this.el.getBoundingClientRect(), dx = e.clientX - r.left, dy = e.clientY - r.top;
    const mv = ev => { this.el.style.left = Math.max(0, ev.clientX - dx) + 'px'; this.el.style.top = Math.max(0, ev.clientY - dy) + 'px'; this.el.style.right = 'auto'; this.el.style.bottom = 'auto'; };
    const up = () => { document.removeEventListener('pointermove', mv); document.removeEventListener('pointerup', up); };
    document.addEventListener('pointermove', mv); document.addEventListener('pointerup', up);
  }
}
