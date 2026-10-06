// Headless driver for the command runtime: simulates the user at the command line and canvas.
import '../../src/commands/index.js';
import { App } from '../../src/engine/app.js';
import { flush } from '../../src/engine/runtime.js';

export class Sim {
  constructor() {
    this.app = new App();
    this.ed = this.app.newDrawing();
    this.rt = this.app.runtime;
    this.ed.view.scale = 50;          // small snap aperture in drawing units, like a zoomed-in session
    this.doc = this.ed.doc;
    this.done = null;
  }
  async cmd(name, opts) { this.done = this.rt.run(name, opts); await flush(); return this; }
  async click(x, y, mods) { this.rt.move({ x, y }); await this.rt.click({ x, y }, mods); return this; }
  async move(x, y) { this.rt.move({ x, y }); return this; }
  async type(text, opts) { await this.rt.text(text, opts); return this; }
  async enter() { await this.rt.enter(); return this; }
  async esc() { this.rt.escape(); await flush(); return this; }
  async finish() { if (this.done) await this.done; await flush(); return this; }
  /** run a full scripted interaction: list of ['click',x,y] | ['type','..'] | ['enter'] */
  async play(name, steps) {
    await this.cmd(name);
    for (const s of steps) {
      if (s[0] === 'click') await this.click(s[1], s[2], s[3]);
      else if (s[0] === 'type') await this.type(s[1]);
      else if (s[0] === 'enter') await this.enter();
      else if (s[0] === 'move') await this.move(s[1], s[2]);
      else if (s[0] === 'esc') await this.esc();
    }
    return this.finish();
  }
  get ents() { return this.doc.entities; }
  get pending() { return this.rt.pending; }
  get prompt() { return this.rt.promptText(); }
}
export const near = (a, b, t = 1e-6) => { if (!(Math.abs(a - b) <= t)) throw new Error(`${a} != ${b}`); };
