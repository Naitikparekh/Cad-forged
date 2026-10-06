// Dynamic input: prompt and coordinate / dimension tooltips that follow the crosshair.
import { h, clear } from './dom.js';
import * as g from '../core/geom.js';
import { fmtNum } from '../core/style.js';

export class DynInput {
  constructor(ui, host) {
    this.ui = ui;
    this.prompt = h('div.dyn-prompt'); this.field = h('div.dyn-field');
    this.el = h('div.dyn', { hidden: true }, this.prompt, this.field);
    host.append(this.el);
  }

  update(mouse, typed) {
    const { app } = this.ui, rt = app.runtime, sv = app.sysvars, p = rt.pending;
    if (!p || !mouse || sv.prefs.DYNMODE <= 0 || ['select'].includes(p.kind) && !p.window) { this.el.hidden = !(p && mouse && sv.prefs.DYNMODE > 0); }
    else this.el.hidden = false;
    if (!p || !mouse || sv.prefs.DYNMODE <= 0) { this.el.hidden = true; return; }
    this.el.hidden = false;
    const prec = app.doc.settings.units.precision;
    // prompt tooltip (dynamic prompts), without the trailing colon
    const msg = rt.promptText(p).replace(/:$/, '').replace(/\n/g, ' ');
    this.prompt.textContent = msg + (p.options?.length ? '  ▾' : '');
    // coordinate / dimension input
    clear(this.field);
    const cur = rt.cursor.pt;
    if (typed) { this.field.append(h('span.dyn-typed', typed), h('span.dyn-caret')); this.field.hidden = false; }
    else if (['point', 'distance', 'angle'].includes(p.kind) && sv.pointerInput) {
      const base = p.base || p.firstPoint;
      if (base && sv.dimensionInput) {
        const d = g.dist(base, cur), a = g.deg(g.normAngle(g.angleOf(base, cur)));
        this.field.append(h('span.dyn-k', 'L'), h('span.dyn-v', fmtNum(d, prec)), h('span.dyn-k', '∠'), h('span.dyn-v', fmtNum(a, 0) + '°'));
      } else this.field.append(h('span.dyn-v', `${fmtNum(cur.x, Math.min(prec, 4))},  ${fmtNum(cur.y, Math.min(prec, 4))}`));
      this.field.hidden = false;
    } else this.field.hidden = true;
    const vp = this.ui.viewport.el.getBoundingClientRect(), w = this.el.offsetWidth, hh = this.el.offsetHeight;
    let x = mouse.x + 24, y = mouse.y + 22;
    if (x + w > vp.width - 6) x = mouse.x - w - 16;
    if (y + hh > vp.height - 6) y = mouse.y - hh - 14;
    this.el.style.transform = `translate(${Math.max(4, x)}px, ${Math.max(4, y)}px)`;
  }
}
