// Plot / export dialog. Replaced with the full implementation in the file I/O step.
import { h } from './dom.js';

export const PLOT = {
  plot: () => ({ title: 'Plot', width: 380, body: h('p.fnote', 'Plotting is not available yet.'), ok: null, cancel: 'Close' }),
};
