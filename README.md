# CadForge 2.0

A browser CAD application with an AutoCAD-style interface and command set: 2D drafting, annotation, hatching,
blocks, layers, 3D solid modelling, DXF import/export and plotting to PDF/SVG/PNG. It builds into **one
self-contained file, `CadForge.html`** (no server, no network, no install) — open it in a browser.

![CadForge drafting](docs/screenshot.png)
![CadForge 3D](docs/screenshot-3d.png)

## What it does

* **Interface modelled on AutoCAD** — application menu, Quick Access Toolbar, InfoCenter search, ribbon with
  tabs/panels/slide-outs/split buttons and contextual *Hatch Creation* tab, document tabs and Start page,
  ViewCube, navigation bar, dynamic input, docked command line with clickable options and autocomplete, status bar
  (Grid, Snap, Dynamic Input, Ortho, Polar, Object Snap, Object Snap Tracking, Lineweight, Quick Properties),
  Properties palette, Layer Properties Manager, Tool Palettes, F2 text window, dark and light themes.
* **160 commands** with AutoCAD names, aliases (`L`, `C`, `TR`, `O`, `M`, `CO`...) and prompts — see
  [docs/COMMANDS.md](docs/COMMANDS.md). Object snaps (END MID CEN NOD QUA INT INS PER TAN NEA) with tracking, polar
  and ortho, FROM / M2P, direct distance entry, pick-first selection, window/crossing/fence/polygon selection, grips,
  transparent commands, command aliases you can edit (`ALIASEDIT`), scripts (`SCRIPT`).
* **2D**: line, polyline (arcs, widths), circle (all variants), arc (all variants), rectangle, polygon, ellipse,
  spline, point, construction line, ray, donut, hatch with island detection, boundary, text/multiline text with an
  in-place editor, linear/aligned/angular/radius/diameter dimensions, leaders, blocks and groups, trim, extend,
  fillet, chamfer, offset, array, mirror, stretch, break, join, lengthen, explode, polyline edit, match properties.
* **3D solids** (triangle meshes): `BOX`, `CYLINDER`, `CONE`, `SPHERE`, `WEDGE`, `EXTRUDE` (holes, taper, start
  elevation, add/subtract with linked history), `PRESSPULL`, `UNION`, `SUBTRACT`, `INTERSECT` (computed on a worker
  thread), `3DMOVE`, `MASSPROP`, orbit and ViewCube, 2D/Wireframe/Hidden/Shaded visual styles, OBJ and STL export.
* **Files**: CadForge JSON, **DXF R2000 export** (validated with `ezdxf`), tolerant **DXF import** (including DXFs written by
  other tools), **plot to PDF / SVG / PNG** with paper size, scale, lineweights and monochrome options and a live preview.

## Build, run, test

```sh
npm install          # esbuild only
npm run build        # -> CadForge.html (add --min for a minified build)
npm test             # unit tests (Node 22+): geometry, commands, DXF, plotting, solids
npm run test:browser # drives the real UI in headless Chromium (needs Playwright + Chromium)
```

`node tools/gen-commands.mjs` regenerates the command reference. A lint configuration is in `eslint.config.mjs`.

## Layout

```
src/core      DOM-free model: geometry, entities, drawing database, styles, dimensions, hatch patterns
src/ops       trim / extend / offset / fillet / boundary detection algorithms
src/engine    command runtime (prompts, snapping, selection, grips), system variables, registry, camera
src/commands  AutoCAD commands, one module per group
src/solid     meshes, extrusion, triangulation, CSG booleans (+ worker), picking
src/io        DXF reader/writer, project files, PDF/SVG writers, plot layout
src/render    canvas renderer, overlay (preview, snaps, grips), solid shading
src/ui        the interface (ribbon, palettes, dialogs, command line...)
tests         unit tests (no browser) and browser tests
legacy        the previous single-file version, kept for reference
```

The core, ops, engine, commands, solid and io layers have no DOM dependency; the unit tests drive them headlessly
with a scripted "user" (`tests/unit/sim.js`).

## Limitations

No DWG, no paper-space layouts/viewports (plotting is of model space), no B-rep solids (meshes only: no fillet or
face editing of solids), no image/OLE/xref entities, limited DXF dimension-style import (dimensions are imported as
their drawn geometry), and a single text font. See `HANDOFF.md`.

Third-party: `csg.js` by Evan Wallace (MIT, `CSG-LICENSE.txt`), vendored in `src/solid/vendor`. Ear-clipping
triangulation follows Mapbox's earcut (ISC).
