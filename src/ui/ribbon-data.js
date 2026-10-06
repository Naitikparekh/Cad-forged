// Ribbon structure: tabs -> panels -> items. Labels, icons and tooltips come from the command registry.
// Item kinds: button (large|small|icon, optional dropdown menu), stack (column of small items), combo, sep.

const B = (cmd, o = {}) => ({ t: 'btn', cmd, size: 'small', ...o });
const L = (cmd, o = {}) => ({ t: 'btn', cmd, size: 'large', ...o });
const I = (cmd, o = {}) => ({ t: 'btn', cmd, size: 'icon', ...o });
const stack = (...items) => ({ t: 'stack', items });
const row = (...items) => ({ t: 'row', items });
const combo = (kind, o = {}) => ({ t: 'combo', kind, ...o });
const m = (cmd, label, args, icon) => ({ cmd, label, args, icon });
const sep = { sep: true };

const ARC_MENU = [
  m('ARC', '3-Point', { mode: '3p' }, 'arc'), m('ARC', 'Start, Center, End', { mode: 'sce' }, 'arc'), m('ARC', 'Start, Center, Angle', { mode: 'sca' }, 'arc'),
  m('ARC', 'Start, Center, Length', { mode: 'scl' }, 'arc'), m('ARC', 'Start, End, Angle', { mode: 'sea' }, 'arc'), m('ARC', 'Start, End, Direction', { mode: 'sed' }, 'arc'),
  m('ARC', 'Start, End, Radius', { mode: 'ser' }, 'arc'), m('ARC', 'Center, Start, End', { mode: 'cse' }, 'arc'), m('ARC', 'Center, Start, Angle', { mode: 'csa' }, 'arc'),
  m('ARC', 'Center, Start, Length', { mode: 'csl' }, 'arc'), sep, m('ARC', 'Continue', { mode: 'continue' }, 'arc'),
];
const CIRCLE_MENU = [
  m('CIRCLE', 'Center, Radius', { mode: 'CR' }, 'circle'), m('CIRCLE', 'Center, Diameter', { mode: 'CD' }, 'circle'), m('CIRCLE', '2-Point', { mode: '2P' }, 'circle'),
  m('CIRCLE', '3-Point', { mode: '3P' }, 'circle'), m('CIRCLE', 'Tan, Tan, Radius', { mode: 'TTR' }, 'circle'),
];
const ELLIPSE_MENU = [m('ELLIPSE', 'Center', { mode: 'center' }, 'ellipse'), m('ELLIPSE', 'Axis, End', {}, 'ellipse'), m('ELLIPSE', 'Elliptical Arc', { arc: true }, 'ellipse')];
const DIM_MENU = [m('DIMLINEAR'), m('DIMALIGNED'), m('DIMANGULAR'), m('DIMRADIUS'), m('DIMDIAMETER'), sep, m('DIMCONTINUE'), m('DIMBASELINE'), sep, m('DIM', 'Dimension')];
const TEXT_MENU = [m('MTEXT'), m('TEXT')];
const MEASURE_MENU = [m('DIST', 'Distance'), m('MEASUREGEOM', 'Radius'), m('MEASUREGEOM', 'Angle'), m('AREA', 'Area'), m('MEASUREGEOM', 'Volume'), sep, m('ID', 'ID Point')];
const ZOOM_MENU = [m('ZOOM', 'Zoom Window', { mode: 'window' }, 'zoomwindow'), m('ZOOM', 'Zoom Previous', { mode: 'previous' }, 'zoomprev'), m('ZOOM', 'Zoom Extents', { mode: 'extents' }, 'zoomextents'), m('ZOOM', 'Zoom In', { mode: 'in' }, 'zoomin'), m('ZOOM', 'Zoom Out', { mode: 'out' }, 'zoomout'), m('ZOOM', 'Zoom Object', { mode: 'object' }, 'zoom')];
const ARRAY_MENU = [m('ARRAYRECT'), m('ARRAYPOLAR')];
const PASTE_MENU = [m('PASTECLIP', 'Paste')];

export const RIBBON = [
  {
    id: 'home', title: 'Home', panels: [
      {
        title: 'Draw', items: [
          L('LINE'), L('PLINE'), L('CIRCLE', { menu: CIRCLE_MENU, label: 'Circle' }), L('ARC', { menu: ARC_MENU, label: 'Arc' }),
          stack(B('RECTANG', { menu: [m('RECTANG'), m('POLYGON')], label: 'Rectangle' }), B('POLYGON'), B('ELLIPSE', { menu: ELLIPSE_MENU, label: 'Ellipse' })),
          stack(B('HATCH', { menu: [m('HATCH'), m('BOUNDARY')] }), B('SPLINE'), B('BOUNDARY')),
        ],
        slideout: [B('POINT', { args: { multiple: true }, label: 'Multiple Points' }), B('DIVIDE'), B('MEASURE'), B('RAY'), B('XLINE', { label: 'Construction Line' }), B('DONUT')],
      },
      {
        title: 'Modify', items: [
          stack(B('MOVE'), B('COPY'), B('STRETCH')), stack(B('ROTATE'), B('MIRROR'), B('SCALE')),
          stack(B('TRIM', { menu: [m('TRIM'), m('EXTEND')] }), B('FILLET', { menu: [m('FILLET'), m('CHAMFER')] }), B('ARRAYRECT', { menu: ARRAY_MENU, label: 'Array' })),
        ],
        slideout: [B('ERASE'), B('EXPLODE'), B('OFFSET'), B('JOIN'), B('BREAK'), B('LENGTHEN'), B('PEDIT'), B('MATCHPROP')],
      },
      {
        title: 'Annotation', launcher: 'DIMSTYLE', prio: 4, items: [
          L('MTEXT', { menu: TEXT_MENU, label: 'Text' }), L('DIM', { menu: DIM_MENU, label: 'Dimension', icon: 'dimlinear' }),
          stack(B('DIMLINEAR', { label: 'Linear' }), B('MLEADER', { label: 'Leader' }), B('TEXTEDIT', { label: 'Edit Text' })),
        ],
      },
      {
        title: 'Layers', launcher: 'LAYER', prio: 2, items: [
          L('LAYER', { label: 'Layer\nProperties' }),
          { t: 'col', items: [combo('layer'), row(I('LAYMCUR'), I('LAYMCH'), I('LAYISO'), I('LAYUNISO'), I('LAYOFF'), I('LAYFRZ'), I('LAYLCK')) ] },
        ],
        slideout: [I('LAYON'), I('LAYTHW'), I('LAYULK'), I('LAYCUR'), I('LAYDEL'), I('LAYERSTATE')],
      },
      {
        title: 'Block', prio: 5, items: [L('INSERT', { menu: 'blocks' }), stack(B('BLOCK', { label: 'Create' }), B('WBLOCK', { label: 'Write Block' }))],
      },
      {
        title: 'Properties', launcher: 'PROPERTIES', prio: 3, items: [
          L('MATCHPROP', { label: 'Match\nProperties' }),
          { t: 'col', items: [combo('color'), combo('lweight'), combo('ltype')] },
        ],
      },
      { title: 'Groups', prio: 6, items: [L('GROUP'), B('UNGROUP')] },
      { title: 'Utilities', prio: 7, items: [L('MEASUREGEOM', { menu: MEASURE_MENU, label: 'Measure', icon: 'dist' }), stack(B('QSELECT', { label: 'Quick Select' }), B('QUICKCALC', { label: 'QuickCalc' }))] },
      { title: 'Clipboard', prio: 8, items: [L('PASTECLIP', { menu: PASTE_MENU, label: 'Paste' }), stack(B('CUTCLIP', { label: 'Cut' }), B('COPYCLIP', { label: 'Copy' }), B('MATCHPROP', { label: 'Match' }))] },
      { title: 'View', prio: 9, items: [stack(B('PAN'), B('ZOOM', { args: { mode: 'window' }, label: 'Window', icon: 'zoomwindow' }), B('ZOOM', { args: { mode: 'extents' }, label: 'Extents', icon: 'zoomextents' })), stack(B('ZOOM', { args: { mode: 'previous' }, label: 'Previous', icon: 'zoomprev' }), B('3DORBIT', { label: 'Orbit' }), B('REGEN'))] },
    ],
  },
  {
    id: 'insert', title: 'Insert', panels: [
      { title: 'Block', items: [L('INSERT', { menu: 'blocks' }), stack(B('BLOCK', { label: 'Create Block' }), B('WBLOCK', { label: 'Write Block' }), B('EXPLODE'))] },
      { title: 'Import', items: [L('IMPORT', { label: 'Import\nDXF / JSON' })] },
      { title: 'Data', items: [L('LIST'), stack(B('DIST', { label: 'Distance' }), B('AREA'), B('ID', { label: 'ID Point' }))] },
    ],
  },
  {
    id: 'annotate', title: 'Annotate', panels: [
      { title: 'Text', launcher: 'STYLE', items: [L('MTEXT', { label: 'Multiline\nText', menu: TEXT_MENU }), stack(B('TEXT', { label: 'Single Line' }), B('TEXTEDIT', { label: 'Edit Text' }), B('STYLE', { label: 'Text Style' }))] },
      {
        title: 'Dimensions', launcher: 'DIMSTYLE', items: [
          L('DIM', { label: 'Dimension', icon: 'dimlinear', menu: DIM_MENU }),
          stack(B('DIMLINEAR', { label: 'Linear' }), B('DIMALIGNED', { label: 'Aligned' }), B('DIMANGULAR', { label: 'Angular' })),
          stack(B('DIMRADIUS', { label: 'Radius' }), B('DIMDIAMETER', { label: 'Diameter' }), B('DIMCONTINUE', { label: 'Continue' })),
          stack(B('DIMBASELINE', { label: 'Baseline' }), B('DIMSTYLE', { label: 'Dimension Style' })),
        ],
      },
      { title: 'Leaders', items: [L('MLEADER', { label: 'Multileader' })] },
      { title: 'Hatch', items: [L('HATCH'), stack(B('BOUNDARY'), B('HATCHEDIT', { label: 'Edit Hatch' }))] },
    ],
  },
  {
    id: 'view', title: 'View', panels: [
      { title: 'Navigate', items: [L('PAN'), L('ZOOM', { args: { mode: 'window' }, label: 'Zoom\nWindow', icon: 'zoomwindow', menu: ZOOM_MENU }), L('ZOOM', { args: { mode: 'extents' }, label: 'Zoom\nExtents', icon: 'zoomextents' }), stack(B('ZOOM', { args: { mode: 'previous' }, label: 'Previous', icon: 'zoomprev' }), B('ZOOM', { args: { mode: 'in' }, label: 'Zoom In', icon: 'zoomin' }), B('ZOOM', { args: { mode: 'out' }, label: 'Zoom Out', icon: 'zoomout' })), L('3DORBIT', { label: 'Orbit' })] },
      { title: 'Named Views', items: [L('-VIEW', { label: 'Named\nViews', icon: 'view' }), stack(B('REGEN'), B('REGENALL', { label: 'Regen All' }))] },
      { title: 'Visual Styles', items: [{ t: 'combo', kind: 'visualstyle' }] },
      {
        title: 'Palettes', items: [
          L('PROPERTIES'), L('LAYER', { label: 'Layer\nProperties' }),
          stack(B('TOOLPALETTES', { label: 'Tool Palettes' }), B('QUICKPROPERTIES', { label: 'Quick Properties' }), B('COMMANDLINE', { label: 'Command Line' })),
        ],
      },
      { title: 'User Interface', items: [L('CLEANSCREENON', { label: 'Clean\nScreen' }), stack(B('MENUBAR', { label: 'Menu Bar' }), B('OPTIONS'), B('HELP', { label: 'Shortcut Keys' }))] },
    ],
  },
  {
    id: 'manage', title: 'Manage', panels: [
      { title: 'Customization', items: [L('ALIASEDIT', { label: 'Edit\nAliases' }), L('TOOLPALETTES', { label: 'Tool\nPalettes' }), L('OPTIONS')] },
      { title: 'Drawing Utilities', items: [L('UNITS'), stack(B('PURGE'), B('-RENAME', { label: 'Rename' }), B('LAYERSTATE', { label: 'Layer States' }))] },
      { title: 'Styles', items: [L('STYLE', { label: 'Text\nStyle' }), L('DIMSTYLE', { label: 'Dimension\nStyle' }), L('LINETYPE', { label: 'Linetype\nManager' })] },
      { title: 'Applications', items: [L('SCRIPT', { label: 'Run\nScript' })] },
    ],
  },
  {
    id: 'output', title: 'Output', panels: [
      { title: 'Plot', items: [L('PLOT'), stack(B('EXPORTPDF', { label: 'Export PDF' }), B('DXFOUT', { label: 'Export DXF' }), B('EXPORT', { label: 'Other Formats' })), stack(B('OBJOUT', { label: 'Export OBJ' }), B('STLOUT', { label: 'Export STL' }))] },
      { title: 'File', items: [L('QSAVE', { label: 'Save' }), L('SAVEAS', { label: 'Save As' }), L('OPEN'), L('NEW')] },
    ],
  },
  {
    id: 'solid', title: '3D Tools', panels: [
      { title: 'Primitives', items: [L('BOX'), L('CYLINDER'), L('SPHERE'), stack(B('CONE'), B('WEDGE'))] },
      { title: 'Solid', items: [L('EXTRUDE'), L('PRESSPULL'), L('UPDATEEXTRUSION', { label: 'Update\nExtrusion' })] },
      { title: 'Boolean', items: [L('UNION'), L('SUBTRACT'), L('INTERSECT')] },
      { title: 'Modify', items: [L('3DMOVE', { label: '3D\nMove' }), L('MASSPROP', { label: 'Mass\nProperties' })] },
      { title: 'Visual Styles', items: [{ t: 'combo', kind: 'visualstyle' }] },
      {
        title: 'Views', items: [
          L('3DORBIT', { label: 'Orbit' }),
          stack(B('3DORBIT', { args: { preset: 'top' }, label: 'Top', icon: 'viewcube' }), B('3DORBIT', { args: { preset: 'front' }, label: 'Front', icon: 'viewcube' }), B('3DORBIT', { args: { preset: 'right' }, label: 'Right', icon: 'viewcube' })),
          stack(B('3DORBIT', { args: { preset: 'swiso' }, label: 'SW Iso', icon: 'viewcube' }), B('3DORBIT', { args: { preset: 'seiso' }, label: 'SE Iso', icon: 'viewcube' }), B('ZOOM', { args: { mode: 'extents' }, label: 'Extents', icon: 'zoomextents' })),
        ],
      },
    ],
  },
];

/** Contextual tab shown while HATCH is active (like AutoCAD's Hatch Creation tab). */
export const HATCH_TAB = {
  id: 'hatchcreate', title: 'Hatch Creation', contextual: true, panels: [
    { title: 'Boundaries', items: [{ t: 'hatchpick' }] },
    { title: 'Pattern', items: [{ t: 'patterns' }] },
    { title: 'Properties', items: [{ t: 'hatchprops' }] },
    { title: 'Close', items: [{ t: 'hatchclose' }] },
  ],
};

/** Contextual tab for editing a hatch object. */
export const HATCHEDIT_TAB = { ...HATCH_TAB, id: 'hatchedit', title: 'Hatch Editor' };
