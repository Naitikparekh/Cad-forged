// In-place text editor shown over the viewport at the text insertion point (TEXT, MTEXT, TEXTEDIT, MLEADER).
import { h } from './dom.js';

export class TextEditor {
  constructor(ui) { this.ui = ui; this.active = null; }

  /** Resolves with the entered string, or null when cancelled. */
  edit(spec) {
    this.cancel();
    const ui = this.ui, vp = ui.viewport, view = ui.editor.view;
    return new Promise(resolve => {
      const s = view.toScreen(spec.p);
      const px = Math.max(11, Math.min(72, spec.h * view.scale));
      const multi = !!spec.multi;
      const width = multi && spec.w ? Math.max(120, spec.w * view.scale) : multi ? 240 : 200;
      const ta = h('textarea.te-input', { rows: multi ? 4 : 1, spellcheck: false, wrap: multi ? 'soft' : 'off', 'aria-label': spec.prompt || 'Text', style: { fontSize: px + 'px', width: width + 'px' } });
      ta.value = spec.initial ?? '';
      const bar = h('div.te-bar',
        h('span.te-title', spec.editing ? 'Edit Text' : multi ? 'Multiline Text' : 'Text'),
        h('span.te-hint', multi ? 'Ctrl+Enter or click outside to finish · Esc cancels' : 'Enter to finish · Esc cancels'),
        h('button.te-ok', { title: 'Close Text Editor', onclick: () => done(ta.value) }, 'OK'),
        h('button.te-cancel', { title: 'Cancel', onclick: () => done(null) }, '✕'));
      const box = h('div.te', { style: { left: Math.round(s.x) + 'px', top: Math.round(s.y - (spec.jv === 'baseline' || !spec.jv ? px : 0)) + 'px', transform: spec.rot ? `rotate(${-spec.rot}rad)` : '', transformOrigin: '0 0' } }, bar, ta);
      let finished = false;
      const done = v => {
        if (finished) return; finished = true;
        document.removeEventListener('pointerdown', outside, true);
        box.remove(); this.active = null;
        vp.canvasFocus();
        resolve(v === null ? null : v);
      };
      const outside = e => { if (!box.contains(e.target)) done(ta.value); };
      ta.addEventListener('keydown', e => {
        e.stopPropagation();
        if (e.key === 'Escape') { e.preventDefault(); done(null); }
        else if (e.key === 'Enter' && (!multi || e.ctrlKey || e.metaKey)) { e.preventDefault(); done(ta.value); }
      });
      ta.addEventListener('input', () => { ta.rows = Math.max(multi ? 4 : 1, ta.value.split('\n').length); });
      vp.el.append(box);
      this.active = { cancel: () => done(null) };
      setTimeout(() => { ta.focus(); ta.select(); document.addEventListener('pointerdown', outside, true); }, 0);
    });
  }

  cancel() { this.active?.cancel(); }
}
