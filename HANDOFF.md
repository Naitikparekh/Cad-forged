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
  `Sim`. `npm run build` refreshes the artifact. (The JS app is frozen; new work goes into the Rust crates.)
* **Rendering**: `renderScene` draws entities with the camera matrix; the same renderer is reused by plotting through
  a recording canvas context (`src/io/recorder.js`) so screen and paper agree.
* **Solids**: `{id, name, layer, color?, vertices, faces, program?}` triangle meshes in `doc.solids`; ids share the
  entity id space so one selection set covers both. Booleans use csg.js in a Blob worker (`src/solid/worker.js`,
  embedded by `build.mjs`). BSP output may contain T-junctions; only volumes are asserted in tests.

## Known gaps / ideas
Layouts and paper-space viewports; DIMSTYLE import from DXF; MLEADER import; attributes; 3D snapping (osnap works on
the z=0 plane only); face-level solid editing; DXF in AC1021+ (UTF-8) form; undo granularity inside long commands.

## Rust port (done)
The DOM-free layers were ported crate by crate and keep the same names and behaviour:

| JS | Rust |
| --- | --- |
| `src/core`, `src/ops`, `src/solid`, `src/io` (DXF/project) | `cadforge-core` (`geom`, `entities`, `drawing`, `ops`, `solid`, `io`) |
| `src/io` (plot, PDF/SVG), `src/render` | `cadforge-render` (`Op` list in device space -> egui shapes / PDF / SVG / tiny-skia PNG) |
| `src/engine`, `src/commands` | `cadforge-engine` (`runtime.rs` async prompts via `Waker::noop` pump, `commands/*`, `Sim` test harness) |
| `src/ui` | `cadforge-app` (egui; custom ribbon, command line, menus, dialogs are hand-drawn with painters) |

Things worth knowing:
* The command runtime has no async executor dependency: each command is a future polled by a tiny pump; prompts park
  the future until input arrives. Dialog-style commands expose `pending_dialog()` and are answered with
  `Handle::answer_dialog(DialogReply)` (the egui app mirrors those into `Dialog` modals).
* Rendering produces `Op::{Stroke,Fill,Text}` lists; the egui painter, PDF/SVG writers and PNG rasteriser all consume
  the same list, so screen and paper agree. The app caches ops keyed by (editor id, revision, view, selection, prefs).
* egui 0.36 specifics: `eframe::App::ui` (not `update`), `Panel::show`, `Modal` for dialogs; a `ScrollArea` inherits the
  parent layout direction, so wrap table bodies in `with_layout(top_down)`.
* The ribbon model is generated: `node tools/gen-app-data.mjs` reads `src/ui/ribbon-data.js` and the command registry
  into `crates/cadforge-app/assets/{icons,ribbon}.json`; rerun it after changing ribbon contents.
* wasm test hook: `window.cf.state_json()` and `window.cf.world_to_page(x, y)` (see `crates/cadforge-app/src/lib.rs`)
  are what `tests/web/harness.mjs` uses to assert on state and aim clicks. Headless Chromium needs
  `--use-angle=swiftshader --enable-unsafe-swiftshader` for WebGL.

Rust gaps relative to the JS app: none known in commands; solids' booleans run synchronously (no worker thread).

## Rust app: what was added after the port
* **Command breadth**: 314 registered commands in `crates/cadforge-engine/src/commands/` (`ext_*` modules are the later
  batches; `ext_tail` is registered just before `ext_system`, which must stay last). Reference: `docs/COMMANDS.md`,
  regenerated with `cargo run -q -p cadforge-engine --example commands_doc > docs/COMMANDS.md`.
* **Layouts**: per-sheet paper-space blocks, `LAYOUT` objects and viewports in DXF (`dxf_write.rs`/`dxf_read.rs`).
* **Raster images**: `Kind::Image` + `Drawing.images` (pixels live in the drawing and are left out of undo snapshots).
  DXF `IMAGE`/`IMAGEDEF` read and write with the pixel convention origin top-left, y down, pixel centres at integers.
  The DXF references the file; pixels are not embedded. Render: `Op::Image`, PDF image XObjects.
* **Block references**: `REFEDIT`/`REFSET`/`REFCLOSE`, `XCLIP` (`InsertData.clip`, display-only, convex clipping in the
  renderer via `clip_ops_to_convex`), `BSAVE`/`BSAVEAS`, `XOPEN`, block palette.
* **AutoLISP** (`crates/cadforge-engine/src/lisp/`): explicit-stack machine (no native recursion, so deep recursion cannot
  overflow), shallow dynamic binding, `(command ...)` feeds `App.lisp_input` (it has priority over the script queue;
  `None` means a `PAUSE` is held until the prompt is answered), `<ID:n,...>` input token for entity names and
  selection sets, `c:` commands land in `Registry.user_commands`, `*error*` handler.
* **Selection**: entity prompts take `allow_locked()` (the layer commands pick objects on locked layers; ordinary
  selection never does).
* **Benchmark**: `tools/benchmark/` holds the AutoCAD command inventories, `rubric.tsv` (checklist credits, each with an
  evidence pointer), `stubs.tsv` (commands that only report "not supported", with partial credit) and `score.py`.
  `score.py --check` fails when an evidence pointer no longer exists; `--write` regenerates `docs/BENCHMARK.md`.
  Only raise a credit when there is code or a test to point at.

### Tests
`cargo test --workspace` (Rust), `node tests/web/run.mjs` (wasm build in headless Chromium; rebuild first with
`tools/build-web.sh dev`). DXF tests use files made by `ezdxf` (`tests/dxf/make-corpus.py`; add new corpus files, do not
regenerate old ones). Scenario tests for commands live in `crates/cadforge-engine/tests/` (`commands_smoke.rs`,
`ui_actions.rs`, `ext_*.rs`, `lisp*.rs`); they drive the `Sim` harness and read `Event`s via `take_events()`.

### Next steps (largest score gaps first, see docs/BENCHMARK.md)
Meshes and surfaces, rendering, sheet sets, dynamic blocks, annotative scaling, MTEXT inline formatting, the full hatch
pattern library, plot styles, PUBLISH, PDF/DGN underlays, DWG, autosave, a spatial index for very large drawings.
