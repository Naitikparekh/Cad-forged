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
  The DXF / DWG IMAGEDEF names the file (other programs look for it); CadForge's own copy of the pixels travels in the
  CADFORGE XRECORD (see DWG below). Render: `Op::Image`, PDF image XObjects.
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

* **Autosave** (`crates/cadforge-engine/src/autosave.rs`): the host calls `App::autosave_tick(force)` every frame; it returns
  recovery files to store (project JSON) and keys to delete. The app keeps them in localStorage (web, key
  `cf.autosave.<key>`) or `<config>/cadforge/autosave/` (native), flushes on `pagehide` / `visibilitychange`, and at start-up
  `recover_autosave` reopens leftovers as unsaved "(recovered)" tabs. `SAVETIME` is in minutes (0 = off).
* **Loaded definitions** (`defs.rs`, `hatch::register_pattern`, `style::register_linetype`): `.pat` / `.lin` text is parsed
  into owned definitions, kept in `Drawing::custom_patterns` / `custom_ltypes` (saved in the JSON) and registered in a
  process-wide table that leaks each definition once so it can be `&'static` like the built-in tables. The DXF reader
  registers simple linetypes of the file's LTYPE table; a pattern defined inside a DXF HATCH is still read as ANSI31.
* **Hidden objects** (`Drawing::hidden_objs`): session-only set for HIDEOBJECTS / ISOLATEOBJECTS; `is_visible` consults it.
* **Polyline widths**: `Vertex { sw, ew }` (start / end width of the edge that starts at the vertex); `pline_outline` builds the
  fill polygons (with bevelled corners) used for drawing, bounds and picking; PLINE has Width / Halfwidth, PEDIT has Width.
* **Multi-page PDF**: `pdf::write_pdf_pages`; `plot::publish_pdf` lays out model space and every layout; PUBLISH hands the file
  to the host as `Event::SaveFile`. The engine now depends on `cadforge-render` for this.
* **Packaging and CI**: `tools/package.sh`, `tests/package/check.sh`, `.github/workflows/{ci,release}.yml`. Windows and macOS
  packages are only ever built by CI. The browser harness can start Chromium with `--force-device-scale-factor` (`launch({ scale })`);
  Playwright's `deviceScaleFactor` emulation does not change `devicePixelContentBoxSize`, so it cannot test HiDPI canvases.
* **Viewport layer overrides** (`ViewportData.layers`, `LayerOverride`): VP freeze plus colour / linetype / lineweight /
  transparency per layout viewport. The renderer applies them in `draw_viewport_content` (the sub-renderer gets
  `vp_over`); plots go through the same path. While a viewport is active (MSPACE) `Drawing.active_vp` names it and
  `touch()` recomputes `active_vp_frozen`, which `is_visible` consults, so frozen layers cannot be picked or snapped
  and undo / redo stay in step. `is_visible_in_sheet` ignores that set (the sheet draws every viewport with its own).
  The freeze list is written to DXF as group 331 (layer handles); the other overrides are not.
* **Plot style tables** (`plotstyle.rs`): `.ctb` / `.stb` in AutoCAD's text layout, plain or inside the zlib container
  (found by scanning for a zlib header after `pmzlibcodec`). Stored in `Drawing.plot_styles` as an `Rc` and carried through
  undo by `Snapshot.plot` (it is not in the JSON snapshot). `Renderer.plot_styles` is set by `render_plot` and decides
  colour / lineweight / screening in `style_of`; `PlotOptions.use_styles` switches it off. **Not tested against a file
  written by AutoCAD** (none was available): lineweight index 0 is assumed to mean "use object lineweight", and the
  container layout is taken from documentation.
* **Binary DXF** (`io/binary_dxf.rs`): `binary_to_text` unpacks a binary file into ordinary DXF text and `text_to_binary`
  packs our writer's text, so there is still one parser and one writer. `decode_dxf_bytes` / `dxf_text_from_bytes` do the
  unpacking, which is why OPEN, IMPORT and drag-and-drop accept binary files without further code. The value type of each
  group comes from its code (same table as ezdxf); R2000+ files use 2-byte codes, R12 1-byte codes (read only). Strings are
  UTF-8 from AC1021 on, otherwise cp1252. Output is always AC1015. `-DXFOUT [ASCII/Binary]` is the command (File > Export >
  Binary DXF in the app). Verified against ezdxf in both directions; never opened in AutoCAD.
* **DWG** (`io/dwg`): the reader (a port of ACadSharp's, MIT) decodes R13–R2018 files (bit streams, R2004 pages and
  LZ77, R2007 Reed-Solomon) into objects, `to_dxf.rs` turns them into DXF groups and `read_dxf` builds the drawing, so
  there is one importer. The writer (`io/dwg/write`) goes the other way: `write_dxf_exact` (numbers in full) is parsed by
  `dxfin.rs` and every record is encoded as a DWG object (`write/mod.rs`, header in `write/header.rs`, the R2004 page
  container and LZ77 encoder in `write/r2004.rs`). Output is AutoCAD 2004 (default) or 2000 (`write_dwg_as`); the drawing
  remembers the version it came from (`Drawing.dwg_version`). Checked by `tests/dwg.rs` (AutoCAD samples of every version
  against their DXF twins), `tests/dwg_write.rs` (open-save-open of every sample, the DXF corpus and drawings with every
  dimension kind, solids, tables, settings; ACadSharp with `DWG_ACADSHARP`, LibreDWG with `LIBREDWG_BIN`, see
  `tools/dwg/README.md`). **Never opened in AutoCAD itself.**
  Round trips: what DXF/DWG cannot express is kept as CadForge data that other programs leave alone — CADFORGE extended
  data on DIMENSIONs (the dimension's own data; used only while its points still match, otherwise the block geometry is
  read), on table / region INSERTs of an anonymous block (only while the insert is unmoved) and on solid polyface meshes
  (their name and id, to join split parts); and one CADFORGE XRECORD in the named object dictionary for drawing-level
  extras (`dxf_write::cadforge_extras` / `dxf_read::apply_extras`). Leaders keep their text as an MTEXT annotation at
  the spot CadForge draws it; groups are GROUP objects; splines CadForge wrote are recognised by their Bezier chain.
  App: OPEN / IMPORT / drop read bytes (`parse_drawing`), Save As has a type list (`SaveFormat` in
  `commands/ext_system/files.rs`, also `-SAVEAS` and `EXPORTDWG`), and a save reports `io::not_kept_in_dxf`.
  `cargo run -p cadforge-core --example rtdiff -- file` lists what changes in a DWG (or `RTDIFF_DXF=1` DXF) round trip.
* **CAD standards** (`standards.rs`): `Drawing::standards` keeps a copy of each standards file's layers, linetypes and
  dimension / multileader / table styles (`Standard::from_drawing`), so checking needs no file access (works in the
  browser) and survives JSON, DXF and DWG (CADFORGE XRECORD key `standards`). `check` lists problems, `fix` / `fix_all`
  apply them; the engine commands are in `commands/ext_system/standards.rs`, the dialogs in `app/src/dialogs/standards.rs`
  (they run the commands, so each fix is one undo step). Named dimension styles are now written to the DIMSTYLE table and
  read back from it; all styles also ride in the XRECORD (`dim_styles`, `mleader_styles`, `table_styles`, `cur_*`).
* **Annotative objects** (`annotative.rs`): `Entity::anno` keeps the paper size (`size_of`: text / leader height, hatch
  pattern scale, block X scale, `DimData::sc`); stored geometry is always sized for CANNOSCALE
  (`Drawing::set_annotation_scale` resizes them), and `draw_viewport_content` draws each through `at_factor` with the
  viewport's factor. DXF / DWG: XRECORD keys `annotative` (by handle), `cannoscale`, `scales`.
* **Associative dimensions** (`dim_assoc.rs`): `DimData::assoc` ties defining points to an object's snap point (index in
  `snap_points_of` plus its kind) or, for radius / diameter, to the circle itself. `Drawing::end_command` runs
  `update_dim_associations`: dimensions made in the command (ids from `cmd_next_id`) are tied when DIMASSOC is 2, and
  tied ones move after their geometry (with their dimension line when every tied point moved the same way). DXF / DWG:
  the links leave the dimension's own JSON and go in the XRECORD (`dim_assoc`, by handle); `W::written_as` records the
  real handle of entities that write other objects first (a dimension's block, a table's block).
* **Isometric drafting**: `SNAPSTYL` 1 switches `snap::snap_to_grid` to the 30° / 150° lattice (`Grid::iso`), ortho to
  `snap::ortho_axes(ISOPLANE)`, the renderer to `grid_iso` lines and the crosshair to the plane's axes.
* **Spatial index** (`spatial.rs`, `Drawing::candidates`): a uniform grid over entity bounds, built per space on first use
  after a change (`Caches` is cleared by every `touch()`), skipped below 512 entities. Callers loop over the returned
  positions in ascending order, so results equal a full scan (`tests/spatial_equiv.rs` proves it against the linear
  path). Used by `pick_at`, `pick_all`, `select_*`, `object_snap` and the renderer's culling; hover, grips, trim / offset
  and boundary search still scan everything.
* **MTEXT formatting** (`mtext.rs`, `text::layout_formatted`): the DXF reader now keeps the codes (`keep_mtext`; only
  `\P` becomes a newline) and the layout turns them into `Layout.runs` / `bars`. Text without codes takes the old path
  unchanged. The renderer draws bold as a double strike, italic as a shear (no new `Op` fields, so PDF / SVG / PNG
  need nothing). `%%d %%p %%c %%u %%o` work in single-line text too. `\c` true colour is read as blue-in-the-high-byte;
  that byte order is from memory of the format, not from a test against AutoCAD.
* **Browser test harness**: `launch()` now proves the input path works before returning (keys typed right after start-up
  were sometimes lost); tests that type into a fresh page should still use `startCommand` / `until` rather than fixed sleeps.
* **Gotcha**: do not run `cargo fmt` — the code base keeps its own longer-line style and has no rustfmt config, so it
  would reformat every file.
* **Gotcha**: `python3 tools/benchmark/score.py --check` prints "EVIDENCE: ... pattern not found" *above* the score table and
  exits 1; do not pipe it through `tail`. Evidence patterns are regexes, so write `\(` for a literal parenthesis (or avoid them).

### Tests
`cargo test --workspace` (Rust), `node tests/web/run.mjs` (wasm build in headless Chromium; rebuild first with
`tools/build-web.sh dev`). DXF tests use files made by `ezdxf` (`tests/dxf/make-corpus.py`; add new corpus files, do not
regenerate old ones). Scenario tests for commands live in `crates/cadforge-engine/tests/` (`commands_smoke.rs`,
`ui_actions.rs`, `ext_*.rs`, `lisp*.rs`); they drive the `Sim` harness and read `Event`s via `take_events()`.

### Next steps (largest score gaps first, see docs/BENCHMARK.md)
Done since the first benchmark: autosave, drag-and-drop, isolate / hide objects, polyline widths, multi-sheet PUBLISH, .lin / .pat
loading, packaging and CI, ten small commands. Still open:
Meshes and surfaces, rendering, sheet sets, dynamic blocks, annotative scaling, MTEXT inline formatting, the full hatch
pattern library, PDF/DGN underlays, DWG output in 2007+ formats and a check of the DWG files in AutoCAD itself.
