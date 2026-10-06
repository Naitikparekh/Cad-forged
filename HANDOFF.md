# CadForge 2.0 — developer handoff

## State
* Web application complete and tested: `npm test` (unit) and `npm run test:browser` (real UI in Chromium) pass.
* DXF output validated with `ezdxf` (`tests/dxf/validate.py`); the reader is tested against an `ezdxf`-written fixture
  (`tests/dxf/fixtures`, regenerate with `tests/dxf/make-ezdxf-fixture.py`).
* The previous (v1) implementation is in `legacy/` for reference; v1 JSON projects still open (`Drawing.fromLegacy`).

## Architecture in one page
* **Drawing** (`src/core/drawing.js`): layers, blocks, model-space entities, solids, settings; undo is whole-drawing
  JSON snapshots taken once per command (`beginCommand`/`willChange`/`endCommand`).
* **Runtime** (`src/engine/runtime.js`): a command is an `async run(ctx, args)` function awaiting prompts
  (`ctx.point/corner/number/distance/angle/string/keyword/entity/solid/select`). Clicks, typed text, Enter and Esc
  are routed to whatever prompt is pending. Prompt text gets its `[Option/List]` suffix automatically from `options`
  (capital letters are the typed abbreviation). Commands never touch the DOM; UI services are optional methods on
  `app.ui` (`dialog`, `editText`, `realtime`, `togglePalette`, file operations...) and commands degrade without them.
* **Adding a command**: `registry.define({ name, aliases, title, icon, desc, run })` in a `src/commands/*.js` module;
  add a ribbon button in `src/ui/ribbon-data.js` (labels, icons and tooltips come from the registry) and a test using
  `Sim`. `npm run build` + `node tools/gen-commands.mjs` refresh the artifact and the reference.
* **Rendering**: `renderScene` draws entities with the camera matrix; the same renderer is reused by plotting through
  a recording canvas context (`src/io/recorder.js`) so screen and paper agree.
* **Solids**: `{id, name, layer, color?, vertices, faces, program?}` triangle meshes in `doc.solids`; ids share the
  entity id space so one selection set covers both. Booleans use csg.js in a Blob worker (`src/solid/worker.js`,
  embedded by `build.mjs`). BSP output may contain T-junctions; only volumes are asserted in tests.

## Known gaps / ideas
Layouts and paper-space viewports; DIMSTYLE import from DXF; MLEADER import; attributes; 3D snapping (osnap works on
the z=0 plane only); face-level solid editing; DXF in AC1021+ (UTF-8) form; undo granularity inside long commands.

## Rust port
The DOM-free layers (`core`, `ops`, `engine`, `solid`, `io`) and their tests are the specification for a Rust port.
