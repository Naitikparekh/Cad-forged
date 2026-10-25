# CadForge command reference

Generated from the command registry (`cargo run -q -p cadforge-engine --example commands_doc`): 347 commands. Type a command or alias at the command line, or use the ribbon.

## 3D

| Command | Aliases | Description |
|---|---|---|
| `3DALIGN` |  | Aligns objects with other objects using up to three point pairs. |
| `3DARRAY` | `3A` | Creates a three-dimensional rectangular or polar array. |
| `3DMOVE` | `3M` | Moves 3D solids a specified distance in a specified direction. |
| `3DROTATE` |  | Rotates solids and drawing objects about the X, Y or Z axis through a base point. |
| `3DSCALE` |  | Scales solids and drawing objects, uniformly or along chosen axes. |
| `BOX` |  | Creates a 3D solid box. |
| `CHAMFEREDGE` |  | Bevels the sharp edges of solids. |
| `CONE` |  | Creates a 3D solid cone or frustum. |
| `CONVTOSOLID` |  | Converts closed outlines and regions into solids. |
| `CYLINDER` | `CYL` | Creates a 3D solid cylinder with a circular base. |
| `EXTRUDE` | `EXT` | Extends the dimensions of a 2D closed outline into 3D space, creating a solid. |
| `FILLETEDGE` |  | Rounds the sharp edges of solids. |
| `FLATSHOT` |  | Creates a 2D drawing of the 3D solids as seen in the current view. |
| `INTERFERE` | `INF` | Finds solids that overlap and can create their common volumes. |
| `INTERSECT` | `IN` | Creates a 3D solid or 2D region from the volume or area common to two or more objects. |
| `LOFT` |  | Creates a 3D solid through a set of cross-sections. |
| `MASSPROP` |  | Calculates the mass properties of regions or 3D solids. |
| `OBJOUT` |  | Writes the solids of the drawing as a OBJ mesh file. |
| `POLYSOLID` | `PSOLID` | Creates a 3D wall along a path, like a polyline with width and height. |
| `PRESSPULL` | `PP` | Presses or pulls bounded areas into 3D. |
| `PYRAMID` | `PYR` | Creates a 3D solid pyramid or frustum with a regular base. |
| `REVOLVE` | `REV` | Creates a 3D solid by sweeping a 2D outline about an axis. |
| `SECTION` | `SEC` | Creates a region where a plane cuts 3D solids. |
| `SECTIONPLANE` | `SPLANE` | Draws the outline where a plane cuts the solids, flat in the drawing. |
| `SHELL` |  | Hollows a solid out, leaving openings where faces are removed. |
| `SLICE` | `SL` | Cuts solids with a plane and keeps one or both sides. |
| `SOLIDEDIT` |  | Edits solids: body options shell, separate, clean and check. |
| `SOLIDHIST` |  | Removes the recorded extrusion history from solids. |
| `SPHERE` | `SPH` | Creates a 3D solid sphere. |
| `STLOUT` |  | Writes the solids of the drawing as a STL mesh file. |
| `SUBTRACT` | `SU` `SUB` | Subtracts one set of 3D solids or 2D regions from another. |
| `SWEEP` |  | Creates a 3D solid by sweeping a 2D outline along an open or closed path. |
| `THICKEN` |  | Turns planar surfaces (regions and closed outlines) into solids of a given thickness. |
| `TORUS` | `TOR` | Creates a doughnut-shaped 3D solid. |
| `UNION` | `UNI` | Combines selected 3D solids or 2D regions into a single object. |
| `UPDATEEXTRUSION` | `EXTUPDATE` | Rebuilds solids from their source outlines after the outlines were edited. |
| `VSCURRENT` | `VS` | Sets the visual style in the current viewport. |
| `WEDGE` | `WE` | Creates a 3D solid wedge. |

## Annotate

| Command | Aliases | Description |
|---|---|---|
| `MTEXTEDIT` |  | Edits multiline text in the in-place editor. |
| `QTEXT` |  | Draws text as boxes (QTEXTMODE). |
| `TABLEEXPORT` |  | Exports data from a table object in CSV file format. |
| `TCOUNT` |  | Numbers text objects in selection order with a start and an increment. |
| `TEXTALIGN` |  | Moves text objects onto a line, perpendicular to it. |
| `TXT2MTXT` |  | Converts single-line text objects to multiline text. |

## Annotation

| Command | Aliases | Description |
|---|---|---|
| `ANNORESET` |  | Resets annotative objects to their size at the current annotation scale. |
| `ANNOUPDATE` |  | Updates annotative objects to the current annotation scale. |
| `BOUNDARY` | `BO` | Creates a region or a polyline from an area enclosed by objects. |
| `DDEDIT` |  | Edits single-line text, multiline text, dimension text, table cells and attribute definitions. |
| `DIM` |  | Creates multiple dimension types with a single command. |
| `DIMALIGNED` | `DAL` | Creates an aligned linear dimension. |
| `DIMANGULAR` | `DAN` | Creates an angular dimension. |
| `DIMARC` | `DAR` | Creates an arc length dimension. |
| `DIMBASELINE` | `DBA` | Creates a linear, angular, or ordinate dimension from the baseline of the previous or selected dimension. |
| `DIMBREAK` |  | Breaks or restores dimension and extension lines where they cross other objects. |
| `DIMCENTER` | `DCE` | Creates the center mark or the centerlines of circles and arcs. |
| `DIMCONTINUE` | `DCO` | Creates a dimension that starts from an extension line of a previously created dimension. |
| `DIMDIAMETER` | `DDI` | Creates a diameter dimension for a circle or an arc. |
| `DIMDISASSOCIATE` | `DDA` | Removes associativity from selected dimensions. |
| `DIMEDIT` | `DED` | Edits dimension text and extension lines. |
| `DIMINSPECT` |  | Adds or removes inspection information for a selected dimension. |
| `DIMJOGGED` | `DJO` | Creates jogged dimensions for circles and arcs. |
| `DIMLINEAR` | `DLI` | Creates a linear dimension. |
| `DIMORDINATE` | `DOR` | Creates ordinate dimensions. |
| `DIMRADIUS` | `DRA` | Creates a radius dimension for a circle or an arc. |
| `DIMREASSOCIATE` | `DRE` | Associates selected dimensions with the geometry at their points, so they follow it. |
| `DIMREGEN` |  | Updates the locations of all associative dimensions. |
| `DIMROTATED` |  | Creates a linear dimension whose dimension line is at a given angle. |
| `DIMSPACE` |  | Adjusts the spacing between parallel linear dimensions. |
| `DIMTEDIT` | `DIMTED` | Moves and rotates dimension text. |
| `FIELD` |  | Creates text with an evaluated field value. |
| `FIND` |  | Finds and replaces text in text, dimensions, leaders, tables and attributes. |
| `HATCH` | `BH` `BHATCH` `H` | Fills an enclosed area or selected objects with a hatch pattern. |
| `HATCHEDIT` | `HE` | Modifies an existing hatch or fill. |
| `MLEADER` | `LE` `LEADER` `MLD` `QLEADER` | Creates a leader line with annotation text. |
| `MLEADEREDIT` |  | Adds leader lines to, or removes them from, a multileader. |
| `MLEADERSTYLE` | `MLS` | Creates and modifies multileader styles. |
| `MTEXT` | `MT` `T` | Creates a multiline text object. |
| `OBJECTSCALE` |  | Adds or deletes supported annotation scales for annotative objects. |
| `QDIM` |  | Quickly creates a series of dimensions from selected objects. |
| `SCALELISTEDIT` | `-SCALELISTEDIT` | Controls the list of scales available for layout viewports, page layouts and annotation. |
| `SPELL` | `SP` | Checks spelling in a drawing against a built-in word list. |
| `TABLEEDIT` | `TABLEDIT` | Edits the text of a table cell and inserts, deletes, merges and unmerges rows, columns and cells. |
| `TABLESTYLE` | `TS` | Creates, modifies or sets the current table style. |
| `TEXT` | `DT` `DTEXT` | Creates a single-line text object. |
| `TEXTEDIT` | `ED` | Edits a dimensional constraint, dimension text, or text object. |
| `TOLERANCE` | `TOL` | Creates geometric tolerances contained in a feature control frame. |

## Block

| Command | Aliases | Description |
|---|---|---|
| `-RENAME` | `REN` `RENAME` | Changes the names assigned to items such as layers and blocks. |
| `ATTDEF` | `-ATT` `-ATTDEF` `ATT` | Creates an attribute definition for storing data in a block. |
| `ATTDISP` |  | Globally controls attribute visibility. |
| `ATTEDIT` | `-ATE` `-ATTEDIT` `ATE` | Changes attribute information in a block. |
| `ATTSYNC` |  | Updates block references with the attributes currently defined for the block. |
| `BASE` |  | Sets the insertion base point for the current drawing. |
| `BATTMAN` |  | Manages the attributes of a block definition. |
| `BCLOSE` |  | Closes the Block Editor and saves the block definition. |
| `BEDIT` | `BE` | Opens a block definition in the Block Editor. |
| `BLOCK` | `B` | Creates a block definition from selected objects. |
| `BSAVE` |  | Saves the current block definition in the Block Editor. |
| `BSAVEAS` |  | Saves a copy of the current block definition under a new name. |
| `EATTEDIT` |  | Edits the attribute values of a block reference. |
| `GROUP` | `G` | Creates and manages saved sets of objects called groups. |
| `INSERT` | `-INSERT` `I` | Inserts a block or drawing into the current drawing. |
| `PURGE` | `PU` | Removes unused items, such as block definitions and layers, from the drawing. |
| `REFCLOSE` |  | Saves back or discards changes made during in-place editing of a reference. |
| `REFEDIT` |  | Selects a reference for editing in place. |
| `REFSET` |  | Adds or removes objects from the working set during in-place editing of a reference. |
| `UNGROUP` | `UG` | Dissolves a group. |
| `WBLOCK` | `W` | Writes objects or a block to a new drawing file. |
| `XATTACH` | `XA` | Inserts a drawing as an external reference. |
| `XBIND` | `-XB` `-XBIND` `XB` | Binds external references to the current drawing. |
| `XCLIP` | `XC` | Crops the display of a selected external reference or block reference to a specified boundary. |
| `XOPEN` |  | Opens a selected drawing reference (xref) in a new window. |
| `XREF` | `-XR` `-XREF` `XR` | Attaches, detaches, binds and manages external references. |

## Blocks

| Command | Aliases | Description |
|---|---|---|
| `ATTEXT` |  | Extracts block attribute data to a CSV file. |
| `BLOCKPALETTE` |  | Opens the palette that lists the blocks of the drawing; click one to insert it. |
| `GROUPEDIT` |  | Adds objects to and removes objects from a group. |
| `GROUPS` |  | Lists the groups defined in the drawing. |
| `IMAGE` | `IM` | Lists the images in the drawing and detaches them. |
| `IMAGEADJUST` |  | Controls the brightness, contrast and fade of the selected image. |
| `IMAGEATTACH` | `IAT` `IMAGEATT` | Inserts a reference to a raster image file (PNG or JPEG). |
| `IMAGECLIP` | `ICL` | Crops the display of a selected image to a specified boundary. |
| `IMAGEFRAME` |  | Controls whether image frames are shown and plotted. |
| `IMAGEQUALITY` |  | Controls the display quality of images. |

## Drafting

| Command | Aliases | Description |
|---|---|---|
| `ISODRAFT` |  | Turns isometric drafting on or off and sets the current isometric plane. |
| `ISOPLANE` |  | Specifies the current isometric plane (Left, Top or Right) that ortho and the cursor follow. |
| `LIMITS` |  | Sets and controls the grid display limits in the current model space. |

## Draw

| Command | Aliases | Description |
|---|---|---|
| `ARC` | `A` | Creates an arc. |
| `CIRCLE` | `C` | Creates a circle. |
| `DONUT` | `DO` | Creates a filled circle or a wide ring. |
| `ELLIPSE` | `EL` | Creates an ellipse or an elliptical arc. |
| `GRADIENT` |  | Fills an enclosed area or selected objects with a gradient fill. Settings are chosen at the command line. |
| `HATCHGENERATEBOUNDARY` |  | Creates a polyline around each loop of a hatch. |
| `LINE` | `L` | Creates straight line segments. |
| `LOADPAT` |  | Loads hatch patterns from a .pat file (acad.pat format) so HATCH can use them by name. |
| `MLINE` | `ML` | Creates multiple parallel lines as a group of two polylines (STANDARD style; justification and scale are supported, styles and MLEDIT are not). |
| `PLINE` | `PL` | Creates a 2D polyline: a single object made of connected line and arc segments. |
| `POINT` | `PO` | Creates a point object. |
| `POLYGON` | `POL` | Creates an equilateral closed polyline. |
| `RAY` |  | Creates a semi-infinite line. |
| `RECTANG` | `REC` `RECTANGLE` | Creates a rectangular polyline. |
| `REGION` | `REG` | Converts an object that encloses an area into a region object. The source objects are deleted. |
| `REVCLOUD` |  | Creates a polyline of sequential arcs to form a cloud-shaped object. Modify only reverses the arc direction; the Calligraphy style is not available. |
| `SKETCH` |  | Creates a series of freehand line segments, a polyline or a spline from consecutive picked points spaced at least the increment apart. |
| `SPLINE` | `SPL` | Creates a smooth curve through fit points. |
| `TABLE` | `TB` | Creates an empty table object. |
| `TRACE` |  | Creates solid lines of a given width. |
| `WIPEOUT` |  | Creates a polygonal area that masks existing objects with the background color. |
| `XLINE` | `XL` | Creates a line of infinite length. |

## File

| Command | Aliases | Description |
|---|---|---|
| `-DXFOUT` |  | Writes the drawing as an ASCII or binary DXF file. |
| `-NEW` |  | Creates a new drawing from the metric or imperial defaults or from a template file. |
| `-SAVEAS` |  | Saves the drawing under a new name as an AutoCAD 2004 or 2000 DWG, a DXF, a drawing template or a CadForge file. |
| `ARCHIVE` |  | Packs the drawing, its pictures and a report into one ZIP file. |
| `AUDIT` |  | Evaluates the integrity of a drawing and corrects some errors. |
| `CLOSEALL` |  | Closes all currently open drawings. |
| `DRAWINGRECOVERY` |  | Shows whether autosave is on and which open drawings have a recovery file. |
| `PASTESPEC` |  | Pastes the clipboard as objects, as a block, or at its original coordinates. |
| `RECOVER` |  | Repairs the open drawing: undefined layers, linetypes and block references. |

## Inquiry

| Command | Aliases | Description |
|---|---|---|
| `AREA` | `AA` | Calculates the area and perimeter of objects or of defined areas. |
| `DBLIST` |  | Lists database information for every object in the drawing. |
| `DIST` | `DI` `DISTANCE` | Measures the distance and angle between two points. |
| `ID` |  | Displays the UCS coordinate values of a specified location. |
| `LIST` | `LI` `LS` | Displays property data for selected objects. |
| `LISTSELECT` |  | Lists the properties of the objects that are currently selected. |
| `MEASUREGEOM` | `MEA` | Measures distances, radii, angles, areas, and volumes of selected objects. |
| `STATUS` |  | Displays drawing statistics, modes, and extents. |
| `TIME` |  | Displays the date and time statistics of a drawing. |

## Layers

| Command | Aliases | Description |
|---|---|---|
| `-LAYER` |  | Manages layers from the command line. |
| `COLOR` | `COL` | Sets the color for new objects. |
| `LAYCUR` |  | Changes the layer of selected objects to the current layer. |
| `LAYDEL` |  | Deletes the layers of selected objects and the objects on them. |
| `LAYER` | `LA` | Manages layers and layer properties. |
| `LAYERP` |  | Undoes the last change or set of changes made to layer settings. |
| `LAYERPALL` |  | Resets the layer property overrides in all layout viewports. |
| `LAYERPMODE` |  | Turns the tracking of layer settings (for LAYERP) on or off. |
| `LAYFRZ` |  | Freezes the layer of selected objects. |
| `LAYISO` |  | Hides or locks all layers except those of the selected objects. |
| `LAYLCK` |  | Locks the layer of a selected object. |
| `LAYMCH` | `LAYMATCH` | Changes the layer of selected objects to match the layer of a destination object. |
| `LAYMCUR` |  | Sets the current layer to that of a selected object. |
| `LAYMRG` |  | Merges selected layers into a target layer, removing the previous layers from the drawing. |
| `LAYOFF` |  | Turns off the layer of selected objects. |
| `LAYON` |  | Turns on all layers in the drawing. |
| `LAYTHW` |  | Thaws all layers in the drawing. |
| `LAYTRANS` |  | Changes layers to the layer names you specify and merges layers that map to the same name. |
| `LAYULK` |  | Unlocks the layer of a selected object. |
| `LAYUNISO` |  | Restores layers hidden or locked by LAYISO. |
| `LAYVPI` |  | Isolates the layer of a selected object in the active layout viewport. |
| `LAYWALK` |  | Displays objects on selected layers and hides objects on all other layers. |
| `LINETYPE` | `LT` `LTYPE` | Loads, sets, and modifies linetypes. |
| `LTSCALE` | `LTS` | Sets the global linetype scale factor. |
| `LWEIGHT` | `LINEWEIGHT` `LW` | Sets the current lineweight, lineweight display options, and lineweight units. |
| `VPLAYER` |  | Freezes, thaws or resets layers in individual layout viewports, and sets defaults for new viewports. |

## Layout

| Command | Aliases | Description |
|---|---|---|
| `LAYOUT` |  | Creates and modifies layout tabs. |
| `LAYOUTWIZARD` |  | Creates a new layout tab and specifies page and plot settings. |
| `MSPACE` | `MS` | Switches from paper space to model space in a layout viewport. |
| `MVIEW` | `MV` | Creates and controls layout viewports. |
| `PAGESETUP` |  | Controls the page layout, plotting device, paper size and other settings for each new layout. |
| `PSPACE` | `PS` | Switches from model space in a layout viewport to paper space. |
| `VPCLIP` |  | Clips layout viewport objects and reshapes the viewport border. |
| `VPORTS` |  | Creates multiple viewports in a layout. |

## Modify

| Command | Aliases | Description |
|---|---|---|
| `ALIGN` | `AL` | Aligns objects with other objects in 2D by moving, rotating (and optionally scaling) them between pairs of points. |
| `ARRAY` | `AR` | Creates copies of objects arranged in a pattern. |
| `ARRAYPATH` |  | Evenly distributes copies of the selected object along a path or a portion of a path. |
| `ARRAYPOLAR` |  | Evenly distributes copies of the selected object in a circular pattern around a center point. |
| `ARRAYRECT` |  | Distributes copies of the selected object into any combination of rows and columns. |
| `BLEND` |  | Creates a spline in the gap between two selected lines or open curves (tangent or smooth continuity). |
| `BREAK` | `BR` | Breaks the selected object between two points. |
| `BREAKATPOINT` |  | Breaks the selected object at a single point. |
| `CHAMFER` | `CHA` | Bevels the edges of objects. |
| `CHANGE` |  | Changes the properties of existing objects: moves line ends, resizes circles, edits text and block insertions, or edits properties. |
| `CHPROP` |  | Changes the properties of objects: color, layer, linetype, linetype scale, lineweight, thickness and transparency. |
| `COPY` | `CO` `CP` | Copies objects a specified distance in a specified direction. |
| `COPYCLIP` |  | Copies selected objects to the clipboard. |
| `CUTCLIP` |  | Copies selected objects to the clipboard and removes them from the drawing. |
| `DIVIDE` | `DIV` | Creates evenly spaced point objects or blocks along the length or perimeter of an object. |
| `ERASE` | `DEL` `E` | Removes objects from the drawing. |
| `EXPLODE` | `X` | Breaks a compound object into its component objects. |
| `EXTEND` | `EX` | Extends objects to meet the edges of other objects. |
| `FILLET` | `F` | Rounds and fillets the edges of objects. |
| `FLATTEN` |  | Sets the Z coordinate of objects to zero. Drawing geometry is already 2D, so nothing changes. |
| `JOIN` | `J` | Joins the endpoints of linear and curved objects to create a single object. |
| `JUSTIFYTEXT` |  | Changes the justification point of selected text objects without changing their locations. |
| `LENGTHEN` | `LEN` | Changes the length of objects and the included angle of arcs. |
| `MATCHPROP` | `MA` | Applies the properties of a selected object to other objects. |
| `MEASURE` | `ME` | Creates point objects or blocks at measured intervals along the length or perimeter of an object. |
| `MIRROR` | `MI` | Creates a mirrored copy of selected objects. |
| `MIRROR3D` | `3DMIRROR` | Creates a mirror image of objects about a plane. |
| `MOVE` | `M` | Moves objects a specified distance in a specified direction. |
| `MULTIPLE` |  | Repeats the next command until you cancel it. |
| `OFFSET` | `O` | Creates concentric circles, parallel lines, and parallel curves. |
| `OOPS` |  | Restores objects deleted by the last ERASE command. |
| `OVERKILL` |  | Deletes duplicate objects and combines overlapping or end-to-end collinear lines and arcs of the same circle. Polylines and circles are only checked for duplicates. |
| `PASTECLIP` |  | Inserts data from the clipboard into the drawing. |
| `PEDIT` | `PE` | Edits polylines and 3D polygon meshes. |
| `ROTATE` | `RO` | Rotates objects around a base point. |
| `ROTATE3D` |  | Moves objects about a 3D axis. |
| `SCALE` | `SC` | Enlarges or reduces selected objects, keeping the proportions the same. |
| `SCALETEXT` |  | Enlarges or reduces selected text objects without changing their locations. |
| `SPLINEDIT` | `SPE` | Edits a fit-point spline: move, add or delete fit points, close or open it, reverse it. |
| `STRETCH` | `S` | Stretches objects crossed by a selection window or polygon. |
| `TRIM` | `TR` | Trims objects to meet the edges of other objects. |

## Output

| Command | Aliases | Description |
|---|---|---|
| `DATAEXTRACTION` | `EATTEXT` | Extracts drawing data (block counts with attributes, or object counts) to a CSV file. |
| `DATALINK` |  | Links a table to a CSV file: the file's cells fill the table. |
| `DATALINKUPDATE` |  | Reads linked data files into their tables again, or writes the tables back to the files. |
| `ETRANSMIT` |  | Lists the files needed to send a drawing to someone else. |
| `PLOTSTYLE` |  | Loads, lists or removes the plot style table (.ctb) that decides colours, lineweights and screening when plotting. |
| `PREVIEW` | `PRE` | Displays the drawing as it will be plotted. |
| `PUBLISH` |  | Publishes drawings to PDF or plotters. |

## Properties

| Command | Aliases | Description |
|---|---|---|
| `-LINETYPE` |  | Lists, sets or loads linetypes (acad.lin format) from the command line. |
| `CELTSCALE` |  | Sets the linetype scale for new objects drawn from now on. |
| `SETBYLAYER` |  | Changes the properties of selected objects to ByLayer. |

## Select

| Command | Aliases | Description |
|---|---|---|
| `HIDEOBJECTS` | `OBJECTHIDE` | Hides the selected objects until UNISOLATEOBJECTS. |
| `ISOLATEOBJECTS` | `ISOLATE` | Hides all objects except the selected ones until UNISOLATEOBJECTS. |
| `UNISOLATEOBJECTS` | `ENDOBJECTISOLATION` `UNISOLATE` | Displays the objects hidden by ISOLATEOBJECTS or HIDEOBJECTS. |

## System

| Command | Aliases | Description |
|---|---|---|
| `ABOUT` |  | Displays information about the application. |
| `ALIASEDIT` | `AE` | Defines and modifies command aliases. |
| `CLEANSCREENON` |  | Clears the screen of toolbars and dockable windows. |
| `CLOSE` |  | Closes the current drawing. |
| `COMMANDLINE` |  | Displays the command line. |
| `DIMSTYLE` | `D` `DDIM` `DST` | Creates and modifies dimension styles. |
| `DRAWORDER` | `DR` | Changes the draw order of images and other objects. |
| `DSETTINGS` | `DS` `RM` `SE` | Sets grid and snap, polar and object snap tracking, object snap modes, dynamic input, and quick properties. |
| `DXFOUT` |  | Writes the drawing as an ASCII DXF file. |
| `DYN` |  | Turns dynamic input on or off. |
| `EXPORT` | `EXP` | Saves the objects in a drawing to a different file format. |
| `EXPORTDWG` | `DWGOUT` | Writes a copy of the drawing as an AutoCAD 2004 DWG file. |
| `EXPORTPDF` |  | Exports the layout to a PDF file. |
| `FILTER` | `FI` | Creates a list of requirements that an object must meet to be included in a selection set (command-line form of the Object Selection Filters dialog). |
| `GRID` |  | Displays a grid pattern in the current viewport. |
| `HELP` | `?` | Displays command and keyboard help. |
| `IMPORT` | `DXFIN` `IMP` | Imports files of various formats into the current drawing. |
| `LAYERSTATE` | `LAS` | Saves, restores, and manages named layer states. |
| `LWDISPLAY` |  | Displays or hides lineweights. |
| `MENUBAR` |  | Controls the display of the menu bar. |
| `NEW` | `QNEW` | Creates a new drawing file. |
| `OPEN` |  | Opens an existing drawing file. |
| `OPTIONS` | `OP` `PREFERENCES` | Customizes the program settings. |
| `ORTHO` |  | Constrains cursor movement to the horizontal or vertical direction. |
| `OSNAP` | `OS` | Sets running object snap modes. |
| `OSNAPTOGGLE` |  | Turns running object snaps on or off. |
| `OTRACK` |  | Tracks from acquired object snap points. |
| `PLOT` | `PRINT` | Plots a drawing to a plotter, printer, or file. |
| `POLAR` |  | Restricts cursor movement to specified angle increments. |
| `PROPERTIES` | `CH` `DDMODIFY` `MO` `PR` `PROPS` | Controls properties of existing objects. |
| `PURGEDIALOG` |  | Purge unused named objects. |
| `QSAVE` | `SAVE` | Saves the current drawing. |
| `QSELECT` |  | Creates a selection set based on filtering criteria. |
| `QUICKCALC` | `CAL` `QC` | Opens the QuickCalc calculator. |
| `QUICKPROPERTIES` | `QP` | Displays the Quick Properties panel. |
| `QUIT` | `EXIT` | Exits the program. |
| `REDO` |  | Reverses the effects of the previous UNDO or U command. |
| `SAVEAS` |  | Saves a copy of the current drawing under a new file name. |
| `SCRIPT` | `SCR` | Executes a sequence of commands from a script file. |
| `SELECT` |  | Places selected objects in the Previous selection set and keeps them selected for the next command. |
| `SELECTALL` |  | Selects all objects except those on locked or frozen layers. |
| `SELECTSIMILAR` | `SELSIM` | Adds similar objects to the selection set based on selected objects. |
| `SETVAR` | `SET` | Lists or changes the values of system variables. |
| `SNAP` | `SN` | Restricts cursor movement to specified intervals. |
| `STYLE` | `DDSTYLE` `ST` | Creates, modifies, or specifies text styles. |
| `TOBACK` |  | Sends objects to the back. |
| `TOFRONT` |  | Brings objects to the front. |
| `TOOLPALETTES` | `TP` | Opens the Tool Palettes window. |
| `U` |  | Reverses the most recent action. |
| `UNDO` |  | Reverses the effect of commands. |
| `UNITS` | `DDUNITS` `UN` | Controls coordinate and angle display formats and precision. |

## Utilities

| Command | Aliases | Description |
|---|---|---|
| `ACTRECORD` | `RECORD` | Starts the Action Recorder: records the commands you run so they can be replayed. |
| `ACTSTOP` |  | Stops the Action Recorder and saves the macro as a script file. |
| `APPLOAD` | `AP` | Loads applications and script files. |
| `CHECKSTANDARDS` |  | Checks layers, linetypes and dimension, multileader and table styles against the standards files and fixes them. |
| `COPYBASE` |  | Copies objects to the clipboard along with a specified base point. |
| `COPYHIST` |  | Copies the text in the command line history to the clipboard. |
| `GRAPHSCR` |  | Switches from the text window to the drawing area. |
| `HIDEPALETTES` |  | Hides all currently displayed palettes (including the command line). |
| `LAYERCLOSE` |  | Closes the Layer Properties Manager. |
| `PASTEBLOCK` |  | Pastes copied objects as a block. |
| `PASTEORIG` |  | Pastes copied objects at the coordinates they were copied from. |
| `PROPERTIESCLOSE` |  | Closes the Properties palette. |
| `REINIT` |  | Reinitializes the digitizer, the digitizer input/output port, and the program parameters file. |
| `RIBBON` |  | Opens the ribbon. |
| `RIBBONCLOSE` |  | Closes the ribbon. |
| `SAVEALL` |  | Saves all open drawings. |
| `SHOWPALETTES` |  | Restores the display of hidden palettes. |
| `STANDARDS` |  | Associates standards files (.dws, .dwg or .dxf) with the current drawing. |
| `TEXTSCR` |  | Displays the text window. |
| `TOOLPALETTESCLOSE` |  | Closes the Tool Palettes window. |
| `VLISP` | `VLIDE` | Opens the AutoLISP console where routines can be typed, run and loaded. |
| `WORKSPACE` |  | Creates, modifies, and saves workspaces and makes a workspace current. |

## View

| Command | Aliases | Description |
|---|---|---|
| `-PAN` |  | Shifts the view by a specified displacement. |
| `-VIEW` | `V` `VIEW` | Saves and restores named views. |
| `3DORBIT` | `3DFORBIT` `3DO` `ORBIT` | Rotates the view in 3D space, but constrained to horizontal and vertical orbit only. |
| `CLEANSCREENOFF` |  | Restores the display state that was in effect before CLEANSCREENON. |
| `DVIEW` | `DV` | Defines parallel projection or perspective views by using a camera and target. |
| `HIDE` | `HI` | Regenerates a three-dimensional model with hidden lines suppressed. |
| `NAVSWHEEL` |  | Displays a wheel that contains a collection of view tools. |
| `NAVVCUBE` |  | Controls the visibility and display properties of the ViewCube. |
| `PAN` | `3DPAN` `P` | Shifts the view in the current viewport. |
| `PLAN` |  | Displays the plan view of a specified user coordinate system. |
| `REDRAW` | `R` | Refreshes the display in the current viewport. |
| `REGEN` | `RE` | Regenerates the drawing and redraws the current viewport. |
| `REGENALL` | `REA` | Regenerates the drawing and redraws all viewports. |
| `SHADEMODE` | `SHA` | Starts the VSCURRENT command: sets the visual style in the current viewport. |
| `UCS` |  | Manages user coordinate systems. |
| `UCSICON` |  | Controls the visibility and placement of the UCS icon. |
| `VIEWCUBE` |  | Turns the ViewCube on or off. |
| `VIEWGO` |  | Restores a named view. |
| `VISUALSTYLES` | `-VISUALSTYLES` `VSM` | Creates and modifies visual styles and applies a visual style to a viewport (command-line form). |
| `VPOINT` | `-VP` | Sets the viewing direction for a 3D visualization of the drawing. |
| `ZOOM` | `3DZOOM` `Z` | Increases or decreases the magnification of the view in the current viewport. |

## System variables

Type a variable name at the command line to view or change it.

| Variable | Default |
|---|---|
| `APERTURE` | `10` |
| `AUTOCOMPLETE` | `1` |
| `AUTOSNAP` | `63` |
| `BACKGROUND` | `""` |
| `CURSORSIZE` | `5` |
| `DYNMODE` | `3` |
| `FILLMODE` | `1` |
| `GRIDMAJOR` | `5` |
| `GRIDMODE` | `1` |
| `GRIDUNIT_X` | `10` |
| `GRIDUNIT_Y` | `10` |
| `GRIPOBJLIMIT` | `100` |
| `GRIPS` | `2` |
| `GRIPSIZE` | `5` |
| `HIGHLIGHT` | `1` |
| `INPUTHISTORYMODE` | `15` |
| `INPUTSEARCHDELAY` | `300` |
| `ISAVEBAK` | `1` |
| `ISOPLANE` | `0` |
| `LAYERPMODE` | `1` |
| `LAYLOCKFADECTL` | `50` |
| `LWDISPLAY` | `0` |
| `MAXHATCH` | `60000` |
| `MENUBAR` | `0` |
| `NAVBAR` | `1` |
| `NAVVCUBE` | `1` |
| `ORTHOMODE` | `0` |
| `OSMODE` | `4133` |
| `OTRACKMODE` | `1` |
| `PICKADD` | `2` |
| `PICKAUTO` | `1` |
| `PICKBOX` | `3` |
| `PICKDRAG` | `0` |
| `PICKFIRST` | `1` |
| `PLINEWID` | `0` |
| `POLARADDANG` | `""` |
| `POLARANG` | `90` |
| `POLARMODE` | `0` |
| `QPMODE` | `-1` |
| `QTEXTMODE` | `0` |
| `SAVETIME` | `10` |
| `SELECTIONPREVIEW` | `3` |
| `SHORTCUTMENU` | `11` |
| `SNAPANG` | `0` |
| `SNAPMODE` | `0` |
| `SNAPSTYL` | `0` |
| `SNAPUNIT_X` | `10` |
| `SNAPUNIT_Y` | `10` |
| `THEME` | `dark` |
| `TOOLTIPS` | `1` |
| `TRACEWID` | `0.05` |
| `TRANSPARENCYDISPLAY` | `1` |
| `UCSICON` | `1` |
| `UCSORG` | `0` |
| `VPCONTROL` | `1` |
