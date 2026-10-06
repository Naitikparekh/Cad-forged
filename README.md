# CadForge — local CAD application

Open **CadForge-Repaired.html** (or CadForge.html in the repaired package) in a modern desktop browser. It is a self-contained application and does not require installation or a server. Use Save project to keep a durable editable copy; browser autosave is best effort. Separate JavaScript files are the editable source modules already bundled into the HTML.

## Available workflows

- **Draft:** lines, straight polylines, rectangles, circles, segmented arcs and text; exact coordinates; relative coordinates; endpoint/grid snapping; orthogonal drafting.
- **Select/edit:** single and Shift-click multiple selection, window selection, Select all, move/copy/rotate/scale/mirror, erase, explode, properties, undo/redo.
- **Modify:** signed line/circle offset; line trim and extend against line boundaries; equal-distance chamfer between two lines sharing an endpoint.
- **Blocks:** define selected entities as a reusable template, insert copies, transform/delete groups, explode them. DXF output uses BLOCK and INSERT records for grouped geometry.
- **Annotate:** aligned dimensions represented by editable lines/text, length/area measurement, diagonal hatching within a circle or simple closed polyline. Hatch lines form an editable group.
- **Layouts:** save and restore model-view presets. Plot / Print sheet opens an in-app monochrome vector preview for A4, A3 or Letter, portrait or landscape, with fit or fixed scale. Download PDF or SVG directly from that preview. Fixed scales treat drawing units as millimetres; the PDF currently uses Helvetica and substitutes non-ASCII text with question marks.
- **3D meshes:** extrude simple closed polygon/circle boundaries or connected closed line outlines with signed depth and a start elevation, create boxes, orbit and zoom, translate/copy meshes, union/subtract/intersect two meshes, and export OBJ or ASCII STL triangle meshes. Choose Home → Extrude profile or View → Extrude. The dialog lists closed profiles and detected closed line loops. When a part exists, Add material to an existing part is the default. You can also create a separate part or cut material. Extrusion retains the 2D source. 3D geometry is saved in project JSON and exported separately from DXF.
- **Files:** project JSON; supported ASCII DXF import/export; visible 2D SVG export; 3D OBJ/STL export.

## Commands and controls

Home contains drawing and modification tools. Insert contains block and drafting tools; Annotate contains dimensions, text, measurement and hatching; View contains layouts and 3D tools; Manage contains project output tools.

Enter `x,y` for an exact point, or `@x,y` relative to the preceding construction point. Enter ends a polyline; Escape cancels. Shift-click adds/removes entities, Ctrl+A selects visible entities, Delete erases the selection, Ctrl+Z/Y undo/redo, F8 toggles Ortho, F9 toggles snapping. Mouse wheel zooms; middle/right drag pans in 2D; drag orbits in 3D.

Aliases: L, PL, C, REC, M, CO, E, RO, SC, MI, O, TR, EX, DIM, A. Commands also include SELECTALL, BLOCK, INSERT, EXPLODE, HATCH, CHAMFER, LAYOUT, RESTORELAYOUT, PLOT, EXTRUDE, BOX, ORBIT, MODEL and OBJ. Tools requiring numeric values or names use in-app dialogs.

Arc construction: center, start point, then end direction, counterclockwise. Dimension construction: first point, second point, then dimension-line position. For Move/Copy: select entities, choose tool, specify base and destination.

## Compatibility and remaining work

This is an independent CAD prototype, not a finished AutoCAD replacement. Native DWG, AutoLISP/.NET/ObjectARX plugins, parametric constraints, associative dimensions, specialist toolsets, a B-rep solid kernel are not implemented. Mesh boolean operations are available, but are not a CAD-grade solid kernel. No DWG engine or Autodesk SDK is included.

DXF support covers LINE, CIRCLE, TEXT, straight POLYLINE, LWPOLYLINE (including tessellated bulges), tessellated ARC, and supported BLOCK/INSERT geometry. Unsupported entities are omitted with a warning; binary DXF is rejected. Nested block expansion is limited. Nonuniformly scaled circles become segmented ellipses. Object-coordinate-system transforms, attributes, text rotation, custom styles and many other DXF attributes are not preserved. Colors export as color index 7. Group instances export with generated block names; original block naming and shared-instance editing semantics are not retained. Inspect imported/exported drawings before use.

Arcs and hatches are approximations or primitives. Dimensions do not update when measured geometry changes. Text stays horizontal through transforms. Saved layouts are model-view presets, not paper space with independently editable viewports. 3D is a mesh modeler; extrusion accepts simple polygons without holes, uses circles approximated by 64-sided polygons. Internal circles are not automatically treated as holes; extrude a cutter mesh and use Subtract A − B. Booleans run in a background worker, accept at most 100000 input triangles, and stop after 45 seconds. They use floating-point BSP operations; complex geometry may fail or fragment. Mesh results are not guaranteed to have a manifold indexed topology.

## Verification

Automated checks cover bundled initialization, blocks and grouped transforms, hatch undo/redo, explode, extend, chamfer, layout restoration, DXF BLOCK/INSERT round trips, arc and bulge endpoints, invalid-file rejection, convex/concave extrusion, closed mesh edges, signed extrusion volume, project validation and 3D rendering. Boolean checks cover known volumes for overlapping boxes, coplanar faces, identical subtraction and disjoint results. The drilled L-bracket sample has its volume checked independently. Browser checks cover drawing, ribbon tabs, in-app extrusion input, project import, mesh translation and union. Full interoperability with AutoCAD has not been tested.

## Sample and third-party component

Open **L-Bracket.cadforge.json** for a 2D outline plus a 3D bracket with three cut-through holes. **L-Bracket.dxf** contains its 2D drawing; **L-Bracket.obj** contains its 3D triangle mesh. Drawing units are arbitrary.

Mesh boolean operations use Evan Wallace's MIT-licensed [csg.js](https://github.com/evanw/csg.js/), bundled locally in the HTML and retained as csg-library.js. Preserve CSG-LICENSE.txt when redistributing. Source snapshot SHA-256: 3E0055252407536964B9A12AD793B6475A9D53C59B088E93794D91F45D4ADFCD. No external network requests are made at runtime.

## Repair build: adding material to an existing part

1. Draw the added outline with connected lines, or create a closed polyline/rectangle/circle. The outline must enclose an area.
2. Click **Extrude profile**. Newly drawn closed line loops appear automatically in Source outline; **Join profile** is available when you want to join selected lines explicitly.
3. Select the new source outline, enter depth and Start elevation Z, and keep **Add material to an existing part**. Choose the target part and click **Extrude**. Depth extends from Start Z; the profile must touch/overlap the existing geometry to form one connected body. Disconnected additions remain separate shells in the same mesh.
4. The target part changes in place. Undo/redo restores both geometry and source outlines. Save project retains the extrusion history. **Update extrusion** rebuilds linked features after source-profile edits; legacy meshes only acquire this history when a feature is added. Moving an entire mesh after creating linked features does not move its source sketch or feature history.

The repair also adds a visible Select tool, fixes picking off-grid objects, enables Escape/Delete/undo from the command bar, draws window-selection feedback, fixes current Model/Extents/Plot handlers, prevents offset copies from staying in their source group, and makes exports visible instead of silently initiating a download. Browser downloads are not captured by the Codex embedded preview used for testing; use Chrome or Edge to save via Download file. File contents remain inspectable in the export dialog.

**Extrusion-After.cadforge.json** is the browser-generated regression result: four new lines were detected as a closed outline and added to a 20×20×10 part. Its signed volume changes from 4000 to 5000 drawing units cubed while the part count stays one. Automated tests verify this saved result, the executable worker script, source retention and PDF file structure. Browser checks verify dialog defaults, additive geometry, undo/redo and sheet preview/PDF generation. These checks do not establish complete AutoCAD interoperability.

UI refresh (October 6): compact icon ribbon with Home, Insert, Annotate, View, 3D Modeling and Output tabs; layer controls in the ribbon; collapsible Properties; command history, Up/Down recall, Enter repeat from idle selection, relative polar input (@distance<angle), Z then E extents, Ctrl+S/O and Ctrl+0 clean screen. Familiar aliases map only to supported CadForge tools. This remains a limited CAD prototype, not full AutoCAD command or DWG compatibility.
