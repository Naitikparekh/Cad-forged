# CadForge — local CAD application

CadForge is a self-contained browser CAD program: 2D drafting, a triangle-mesh 3D modeler, and an interface and command set modelled on desktop CAD conventions (application menu, tabbed ribbon, command line, status-bar drafting aids). Open **CadForge.html** (identical copy: CadForge-Repaired.html) in a current desktop browser. No installation, server or network access is needed. Use **Save project** to keep an editable copy; browser autosave is best effort.

CadForge is an independent project. It uses familiar AutoCAD-style layout, command names and prompt wording so the workflow is recognisable, but it is not AutoCAD, does not read or write DWG, and uses original icons and artwork.

## The interface

| Area | What it does |
|---|---|
| **Application button / menu** | New, Open, Save, Save As, Import, Export (DXF, SVG, OBJ, STL, PDF), Plot, plus a command search box. |
| **Quick Access Toolbar** | New, Open, Save, Save As, Plot, Undo, Redo, and the **workspace** switch (Drafting & Annotation / 3D Modeling). |
| **Ribbon** | Tabs and panels (Home, Insert, Annotate, View, Manage, Output; the 3D Modeling workspace adds Solid and Mesh). Split buttons open drop-downs; double-click the active tab to minimise. |
| **File tabs** | Start page and the current drawing (`*` marks unsaved changes). |
| **Drawing area** | Viewport label, ViewCube, navigation bar (pan, zoom extents/window, orbit) and UCS icon. |
| **Command line** | Prompts such as `LINE Specify first point:`, clickable `[Option]` keywords, AutoComplete while typing, history (F2 expands it). |
| **Dynamic input** | Prompt and distance/angle readout beside the cursor. |
| **Status bar** | Model / Layout tabs, coordinates, Grid, Snap, Ortho, Polar, Object Snap, Dynamic Input, workspace and Properties toggles. Right-click Object Snap and Polar for their settings. |
| **Properties palette** | Edit the selected object's layer and geometry (coordinates, radius, text, vertices). For meshes it shows statistics and the boolean controls. |
| **Layer Properties Manager** | Create, rename, colour, switch on/off, set current and delete layers. |

Right-click in the drawing area for the shortcut menu (Repeat, Enter, Cancel, selection operations).

## Commands

Type a command name or alias at the command line and press Enter or Space; an empty Enter repeats the previous command; Up/Down recall earlier input. There are 90+ commands; **HELP** (F1) lists them by category.

- **Draw:** LINE (L), PLINE (PL), CIRCLE (C), ARC (A), RECTANG (REC), POLYGON (POL), ELLIPSE (EL), TEXT (DT, T), MTEXT (MT), HATCH (H; click inside an area, or select boundary objects), DIMLINEAR (DLI, DIM), DIMALIGNED (DAL)
- **Modify:** MOVE (M), COPY (CO), ROTATE (RO), SCALE (SC), MIRROR (MI), OFFSET (O), TRIM (TR), EXTEND (EX), FILLET (F), CHAMFER (CHA), ARRAYRECT (AR), ERASE (E), EXPLODE (X), JOIN (J)
- **Blocks and properties:** BLOCK (B), INSERT (I), MATCHPROP (MA), PROPERTIES (PR, CH), LAYER (LA), LAYMCUR, LAYON
- **Inquiry and clean-up:** DIST (DI), ID, LIST (LI), MEASUREGEOM (MEA), PURGE (PU)
- **Selection and edit:** SELECTALL, UNDO (U), REDO, COPYCLIP, CUTCLIP, PASTECLIP
- **View:** ZOOM (Z; Extents, All, Window, Previous, In, Out, scale factor), PAN (P), REGEN, 3DORBIT, PLAN, VIEW (V, named views), VSCURRENT (VS, visual styles), NAVVCUBE, NAVBAR, UCSICON, MODEL, LAYOUT
- **Drafting aids:** GRID (F7), SNAP (F9), ORTHO (F8), POLAR (F10), OSNAP (F3), DYNMODE (F12), DSETTINGS (DS)
- **Files:** NEW, OPEN, QSAVE, SAVEAS, PLOT, PAGESETUP, EXPORT, DXFOUT, DXFIN, SVGOUT, STLOUT, OBJEXPORT
- **3D mesh:** EXTRUDE (EXT), BOX, UNION (UNI), SUBTRACT (SU), INTERSECT (IN), UPDATEEXTRUSION, 3DMOVE (3M), MESHCOPY, MESHCLEAR, MESHFIT
- **Interface:** PROPERTIES, PROPERTIESCLOSE, COMMANDLINE, COMMANDLINEHIDE, CLEANSCREENON/OFF, TEXTSCR, RIBBON, RIBBONCLOSE, WSCURRENT

**Point input:** `x,y` absolute, `@x,y` relative, `@distance<angle` polar, or just a number to move that distance in the direction of the cursor (direct distance entry; combine with Ortho or Polar). At a point prompt you can also type an object-snap keyword (END, MID, CEN, QUA, INT, PER, TAN, NEA, INS, NON) to override the running snaps for the next pick. **Object selection:** click, Shift-click to remove, drag left-to-right for a window (fully enclosed) and right-to-left for a crossing (touching); modify commands ask `Select objects:` when nothing is selected, or use an existing selection.

**Keys:** F1 help, F2 history, F3 object snap, F7 grid, F8 ortho, F9 snap, F10 polar, F12 dynamic input; Ctrl+N/O/S, Ctrl+Shift+S, Ctrl+P, Ctrl+Z/Y, Ctrl+A, Ctrl+C/X/V, Ctrl+1 Properties, Ctrl+9 command line, Ctrl+0 clean screen, Esc cancel, Delete erase. Wheel zooms; middle or right drag pans; a double middle-click zooms to extents.

## Working with 3D meshes

Draw a closed outline (polyline, rectangle, circle, or connected lines that close — JOIN merges selected lines), then **EXTRUDE** with a signed depth and start elevation. When a part already exists, *Add material to an existing part* is the default; you can also create a separate part or cut material. The 2D source stays editable and **UPDATEEXTRUSION** rebuilds linked features. Union, subtract and intersect run in a background worker on the two meshes chosen as Mesh A and B (click a mesh in the 3D view, Shift-click for B). Export OBJ or ASCII STL. The ViewCube and VIEW presets give top, front, side and isometric views; visual styles are 2D Wireframe, Wireframe, Shaded and Shaded with Edges.

## Files and compatibility

- **Project JSON** (`.cadforge.json`) keeps layers, entities, blocks, named views, meshes and extrusion history.
- **DXF** (ASCII): LINE, CIRCLE, TEXT, straight POLYLINE, LWPOLYLINE (bulges tessellated), ARC (tessellated) and supported BLOCK/INSERT geometry. Unsupported entities are omitted with a warning; binary DXF is rejected; nested block expansion is limited; non-uniformly scaled circles become segmented ellipses. Colours export as index 7. Inspect imported and exported drawings before use.
- **SVG / PDF:** the Plot dialog previews A4, A3 or Letter, portrait or landscape, at a fitted or fixed scale (fixed scales treat drawing units as millimetres). The PDF uses Helvetica and replaces non-ASCII text with question marks.
- **OBJ / STL:** triangle meshes only; 3D geometry is not part of DXF output.

## Limits

This is a prototype, not an AutoCAD replacement. Not implemented: DWG, AutoLISP/.NET/ObjectARX plugins, parametric constraints, associative dimensions, true arcs/splines, paper-space viewports and a B-rep solid kernel. Arcs, ellipses and fillets are polylines; hatches are generated line sets; dimensions do not update when geometry changes; text stays horizontal. Layouts are model-view presets plus a paper-sheet view. 3D is a mesh modeler: extrusion accepts simple polygons without holes and circles become 64-sided polygons (cut holes by subtracting a cutter). Booleans use floating-point BSP, accept at most 100000 input triangles, stop after 45 seconds, and do not guarantee manifold results. Full interoperability with AutoCAD has not been tested.

## Building and testing

The HTML is generated from the source modules, so edit the `.js` files and rebuild:

```powershell
powershell -ExecutionPolicy Bypass -File build.ps1          # writes CadForge.html and CadForge-Repaired.html
node Verify.cjs                                             # runs the regression suite against CadForge.html
node Verify.cjs path\to\bundle.html                         # or against another bundle
```

`build.ps1 -Include acad-core,acad-views -Out _dev\test.html` builds a partial bundle (engine plus the named workspace modules) for testing one module. Module checks live in `tests/*.test.cjs` and are run by `Verify.cjs`.

Source layout: the drafting/mesh **engine** is `core.js dialogs.js workspace.js advanced.js mesh.js compatibility.js csg-library.js booleans.js repairs.js profiles.js sheets.js`; the AutoCAD-style **workspace** is `acad-core.js` (command registry, drafting-aid state, shared hooks) plus `acad-icons.js acad-commands.js acad-drafting.js acad-interact.js acad-views.js acad-palettes.js acad-shell.js`. `shell.html` is the page template. `ui.js` is the superseded ribbon from the earlier build and is not bundled.

## Sample files and third-party component

Open **L-Bracket.cadforge.json** for a 2D outline plus a 3D bracket with three cut-through holes; **L-Bracket.dxf** is its 2D drawing and **L-Bracket.obj** its mesh. **Extrusion-After.cadforge.json** is a saved result of adding a closed outline to an existing part (volume 4000 → 5000 drawing units cubed).

Mesh booleans use Evan Wallace's MIT-licensed [csg.js](https://github.com/evanw/csg.js/), bundled locally in the HTML and kept as csg-library.js. Preserve CSG-LICENSE.txt when redistributing. Source snapshot SHA-256: 3E0055252407536964B9A12AD793B6475A9D53C59B088E93794D91F45D4ADFCD. No external network requests are made at runtime.
