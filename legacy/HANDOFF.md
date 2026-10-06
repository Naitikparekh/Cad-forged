# CadForge handoff

The current deliverable is `CadForge.html` and `CadForge-Rebuilt.zip`.

The interface now follows the AutoCAD layout pattern: quick access header, tabbed ribbon panels, model-space drawing canvas, command line and history, dockable Properties, ViewCube-style view control, and status toggles for Ortho, Snap and Clean Screen. Supported aliases include LINE/L, PLINE/PL, CIRCLE/C, RECTANG/REC, MOVE/M, COPY/CO, ERASE/E, TRIM/TR, EXTEND/EX, OFFSET/O, ROTATE/RO, SCALE/SC, MIRROR/MI, DIM/DIMLINEAR, HATCH/H, BLOCK/B, INSERT/I, EXPLODE/X, EXTRUDE/EXT, UNION, SUBTRACT, INTERSECT, and Z then E for extents.

Closed outlines drawn after an existing part can be extruded as additive material. The source outline remains editable and Update extrusion rebuilds the linked feature history.

Verification: `node outputs/Verify.cjs` passes all geometry, mesh, extrusion, import/export, UI-command, and PDF checks. This is a local browser CAD prototype; native DWG, full AutoCAD command coverage, B-rep constraints, and associative drafting are outside the current implementation.
