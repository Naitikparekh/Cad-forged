// Entry point: creates the application, builds the UI and opens an empty drawing.
import './commands/index.js';
import { App } from './engine/app.js';
import { UI } from './ui/ui.js';

const $ = id => document.getElementById(id);

function boot() {
  const store = (() => { try { return window.localStorage; } catch { return null; } })();
  const app = new App({ storage: store });
  let theme = 'dark';
  try { theme = store?.getItem('cf.theme') || 'dark'; } catch { /* ignore */ }
  app.sysvars.prefs.THEME = theme === 'light' ? 'light' : 'dark';
  const ui = new UI(app, {
    titlebar: $('titlebar'), menubar: $('menubar'), ribbon: $('ribbon-host'), doctabs: $('doctabs'), workarea: $('workarea'), viewport: $('viewport'),
    dockLeft: $('dock-left'), dockRight: $('dock-right'), cmdline: $('cmdline-host'), statusbar: $('statusbar'), overlay: $('overlay-root'), layouttabs: $('layouttabs'),
  });
  app.newDrawing();
  app.log('CadForge 2.0 — type a command, or choose one from the ribbon. Press F1 for help.');
  window.cadforge = { app, ui };
  ui.layoutChanged();
  ui.cmdline.focus();
}

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', boot); else boot();
