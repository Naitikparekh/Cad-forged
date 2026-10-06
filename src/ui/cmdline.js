// Command line: history, current prompt with clickable options, input with suggestion list, F2 text window.
import { h, svgIcon, clear } from './dom.js';
import { showMenu } from './menu.js';
import { optionName } from '../engine/parse.js';
import { registry } from '../engine/registry.js';
import { flush } from '../engine/runtime.js';

export class CommandLine {
  constructor(ui, host) {
    this.ui = ui; this.app = ui.app; this.host = host;
    this.lines = 3; this.recallIdx = -1; this.dyn = false; this.sugIdx = -1; this.sugs = [];
    this.history = h('div.cl-history', { role: 'log', 'aria-live': 'polite' });
    this.promptEl = h('span.cl-prompt');
    this.input = h('input.cl-input', { type: 'text', spellcheck: false, autocomplete: 'off', 'aria-label': 'Command line', placeholder: 'Type a command', onkeydown: e => this.key(e), oninput: () => this.onInput(), onfocus: () => this.host.classList.add('focus'), onblur: () => { this.host.classList.remove('focus'); setTimeout(() => this.hideSuggest(), 120); } });
    this.sug = h('div.cl-suggest', { hidden: true });
    this.closeBtn = h('button.cl-btn', { title: 'Close command line (Ctrl+9)', onclick: () => this.ui.togglePalette('command', false) }, svgIcon('x', 10));
    this.menuBtn = h('button.cl-btn', { title: 'Customize', onclick: e => this.customize(e) }, svgIcon('menubar', 12));
    this.grip = h('div.cl-grip', { title: 'Drag to resize', onpointerdown: e => this.startResize(e) });
    const bar = h('div.cl-bar', this.closeBtn, this.menuBtn);
    host.append(this.grip, bar, this.history, h('div.cl-row', h('span.cl-icon', svgIcon('chevright', 12)), this.promptEl, this.input), this.sug);
    this.apply();
    this.app.on('log', l => this.append(l));
    this.app.on('prompt', () => this.renderPrompt());
    this.app.on('change', () => this.renderPrompt());
    this.app.on('commandEnd', () => this.renderPrompt());
    for (const l of this.app.history) this.append(l);
    this.renderPrompt();
  }

  apply() { this.host.style.setProperty('--cl-lines', this.lines); }

  startResize(e) {
    e.preventDefault();
    const y0 = e.clientY, n0 = this.lines;
    const mv = ev => { this.lines = Math.max(1, Math.min(14, Math.round(n0 + (y0 - ev.clientY) / 17))); this.apply(); this.ui.layoutChanged(); };
    const up = () => { document.removeEventListener('pointermove', mv); document.removeEventListener('pointerup', up); };
    document.addEventListener('pointermove', mv); document.addEventListener('pointerup', up);
  }

  customize(e) {
    showMenu([
      { label: 'Recent Commands', submenu: () => (this.app.runtime.recentCommands.length ? this.app.runtime.recentCommands.map(c => ({ label: c, action: () => this.ui.runCommand(c) })) : [{ label: 'No recent commands', disabled: true }]) },
      { label: 'Input Settings', submenu: [
        { label: 'AutoComplete', checked: !!this.app.sysvars.prefs.AUTOCOMPLETE, action: () => this.app.sysvars.set('AUTOCOMPLETE', this.app.sysvars.prefs.AUTOCOMPLETE ? 0 : 1) },
        { label: 'Search system variables', checked: true, disabled: true }, { label: 'Search content', checked: false, disabled: true },
      ] },
      { sep: true },
      { label: 'Command line lines', submenu: [1, 2, 3, 5, 8, 12].map(n => ({ label: `${n} line${n > 1 ? 's' : ''}`, checked: this.lines === n, action: () => { this.lines = n; this.apply(); this.ui.layoutChanged(); } })) },
      { label: 'Copy History', action: () => navigator.clipboard?.writeText(this.app.history.map(l => l.text).join('\n')) },
      { label: 'Clear History', action: () => { this.app.history.length = 0; clear(this.history); } },
      { sep: true },
      { label: 'Close', shortcut: 'Ctrl+9', action: () => this.ui.togglePalette('command', false) },
    ], e.clientX, e.clientY);
  }

  // ------------------------------------------------------------ history

  append({ text, kind }) {
    for (const part of String(text).split('\n')) this.history.append(h('div.cl-line.' + kind, part));
    while (this.history.childElementCount > 600) this.history.firstElementChild.remove();
    this.history.scrollTop = this.history.scrollHeight;
    this.ui.textWindow?.append?.({ text, kind });
  }

  renderPrompt() {
    const rt = this.app.runtime, p = rt.pending;
    clear(this.promptEl);
    if (!rt.busy || !p) {
      this.promptEl.append(h('span.cl-cmd', rt.busy ? '' : 'Command:'));
      this.input.placeholder = rt.busy ? '' : 'Type a command';
      return;
    }
    this.input.placeholder = '';
    const name = rt.ctx.name.startsWith('_') ? '' : rt.ctx.name;
    const text = rt.promptText(p).replace(/\n/g, ' ');
    if (name) this.promptEl.append(h('span.cl-name', name), ' ');
    // render [A/B/C] options as clickable words
    const m = text.match(/^(.*?)\[(.+?)\](.*)$/);
    if (m && p.options?.length) {
      this.promptEl.append(m[1], '[');
      p.options.forEach((o, i) => { if (i) this.promptEl.append('/'); this.promptEl.append(h('button.cl-opt', { onmousedown: ev => ev.preventDefault(), onclick: () => { rt.keyword(o); this.focus(); } }, o.replace(/\s*\(.*\)\s*$/, ''))); });
      this.promptEl.append(']' + m[3]);
    } else this.promptEl.append(text);
  }

  focus() { this.input.focus({ preventScroll: true }); }
  reset() { this.input.value = ''; this.dyn = false; this.recallIdx = -1; this.hideSuggest(); }

  /** Text currently typed through the dynamic-input route (shown at the cursor). */
  dynamicText() { return this.dyn && this.input.value ? this.input.value : ''; }

  // ------------------------------------------------------------ typing

  /** Send a typed character from the canvas into the command line. */
  typeFromCanvas(ch) {
    this.dyn = this.app.sysvars.pointerInput && !!this.app.runtime.pending;
    this.focus();
    this.input.value += ch;
    this.onInput();
  }

  async submit() {
    const text = this.input.value, dyn = this.dyn;
    this.input.value = ''; this.dyn = false; this.recallIdx = -1; this.hideSuggest();
    const rt = this.app.runtime;
    if (!rt.busy && text.trim()) this.remember(text.trim());
    await rt.text(text, { dynamic: dyn });
    this.ui.viewport.overlayDirty = true; this.ui.viewport.schedule();
  }

  remember(t) { const a = this.past ??= []; if (a[0] !== t) a.unshift(t); if (a.length > 60) a.pop(); }

  key(e) {
    const rt = this.app.runtime;
    if (e.key === 'Enter') { e.preventDefault(); if (this.sugIdx >= 0 && this.sugs[this.sugIdx]) { this.input.value = this.sugs[this.sugIdx]; this.hideSuggest(); } this.submit(); return; }
    if (e.key === ' ' && rt.pending?.kind !== 'string' && !/\s/.test(this.input.value.trim())) { e.preventDefault(); if (this.sugIdx >= 0 && this.sugs[this.sugIdx]) this.input.value = this.sugs[this.sugIdx]; this.submit(); return; }
    if (e.key === 'Escape') { e.preventDefault(); this.ui.escape(); return; }
    if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
      e.preventDefault();
      if (!this.sug.hidden && this.sugs.length) { const n = this.sugs.length; this.sugIdx = e.key === 'ArrowDown' ? (this.sugIdx + 1) % n : this.sugIdx <= 0 ? n - 1 : this.sugIdx - 1; this.renderSuggest(); return; }
      if (e.key === 'ArrowDown' && rt.pending?.options?.length && !this.input.value) { this.optionMenu(); return; }
      const src = rt.busy ? rt.recentInput : (this.past || []);
      this.recallIdx = Math.max(-1, Math.min(src.length - 1, this.recallIdx + (e.key === 'ArrowUp' ? 1 : -1)));
      this.input.value = this.recallIdx >= 0 ? src[this.recallIdx] : '';
      return;
    }
    if (e.key === 'Tab' && this.sugs.length) { e.preventDefault(); this.input.value = this.sugs[Math.max(0, this.sugIdx)]; this.hideSuggest(); }
  }

  optionMenu() {
    const p = this.app.runtime.pending, r = this.input.getBoundingClientRect();
    showMenu(p.options.map(o => ({ label: optionName(o), action: () => this.app.runtime.keyword(o) })), r.left, r.top - 4 - p.options.length * 24);
  }

  // ------------------------------------------------------------ suggestions

  onInput() {
    const rt = this.app.runtime, v = this.input.value.trim();
    this.recallIdx = -1;
    if (rt.busy || !v || !this.app.sysvars.prefs.AUTOCOMPLETE || /^[\d@#.,<-]/.test(v)) { this.hideSuggest(); return; }
    const sv = this.app.sysvars;
    const cmds = registry.suggest(v, { midString: true, limit: 14 }).filter(n => !n.startsWith('_') && !/^OSNAP_/.test(n));
    const vars = v.length >= 3 ? sv.names().filter(n => n.startsWith(v.toUpperCase()) && !cmds.includes(n)).slice(0, 4) : [];
    this.sugs = [...cmds.slice(0, 10), ...vars];
    this.sugIdx = -1;                                      // Enter runs what was typed; arrows / Tab pick a suggestion
    if (!this.sugs.length) { this.hideSuggest(); return; }
    this.renderSuggest();
  }

  renderSuggest() {
    clear(this.sug);
    this.sugs.forEach((n, i) => {
      const def = registry.commands.get(n), isVar = !def;
      this.sug.append(h('div.sg' + (i === this.sugIdx ? '.sel' : ''), { onmousedown: e => { e.preventDefault(); this.input.value = n; this.hideSuggest(); this.submit(); } },
        h('span.sg-ico', def ? svgIcon(def.icon || n.toLowerCase(), 14) : h('span.sg-var', 'V')), h('span.sg-name', n), h('span.sg-desc', def?.title && def.title !== n ? def.title : isVar ? 'System variable' : '')));
    });
    this.sug.hidden = false;
  }

  hideSuggest() { this.sug.hidden = true; this.sugs = []; this.sugIdx = -1; }
}

void flush;
