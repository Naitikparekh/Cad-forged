# CadForge command reference

Generated from the command registry (157 commands). Type a command or alias at the command line, or use the ribbon.

## 3D

| Command | Aliases | Description |
|---|---|---|
| `3DMOVE` | `3M` | Moves 3D solids a specified distance in a specified direction. |
| `BOX` |  | Creates a 3D solid box. |
| `CONE` |  | Creates a 3D solid cone or frustum. |
| `CYLINDER` | `CYL` | Creates a 3D solid cylinder with a circular base. |
| `EXTRUDE` | `EXT` | Extends the dimensions of a 2D closed outline into 3D space, creating a solid. |
| `INTERSECT` | `IN` | Creates a 3D solid from the volume common to two or more solids. |
| `MASSPROP` |  | Calculates the mass properties of regions or 3D solids. |
| `OBJOUT` |  | Writes the solids of the drawing as a OBJ mesh file. |
| `PRESSPULL` | `PRESSPULL` | Presses or pulls bounded areas into 3D. |
| `SPHERE` | `SPH` | Creates a 3D solid sphere. |
| `STLOUT` |  | Writes the solids of the drawing as a STL mesh file. |
| `SUBTRACT` | `SU` `SUB` | Subtracts one set of 3D solids from another. |
| `UNION` | `UNI` | Combines selected 3D solids into a single solid. |
| `UPDATEEXTRUSION` | `EXTUPDATE` | Rebuilds solids from their source outlines after the outlines were edited. |
| `VSCURRENT` | `VS` | Sets the visual style in the current viewport. |
| `WEDGE` | `WE` | Creates a 3D solid wedge. |

## Annotation

| Command | Aliases | Description |
|---|---|---|
| `BOUNDARY` | `BO` | Creates a region or a polyline from an area enclosed by objects. |
| `DIM` |  | Creates multiple dimension types with a single command. |
| `DIMALIGNED` | `DAL` | Creates an aligned linear dimension. |
| `DIMANGULAR` | `DAN` | Creates an angular dimension. |
| `DIMBASELINE` | `DBA` | Creates a linear, angular, or ordinate dimension from the baseline of the previous or selected dimension. |
| `DIMCONTINUE` | `DCO` | Creates a dimension that starts from an extension line of a previously created dimension. |
| `DIMDIAMETER` | `DDI` | Creates a diameter dimension for a circle or an arc. |
| `DIMLINEAR` | `DLI` | Creates a linear dimension. |
| `DIMRADIUS` | `DRA` | Creates a radius dimension for a circle or an arc. |
| `HATCH` | `BH` `BHATCH` `H` | Fills an enclosed area or selected objects with a hatch pattern. |
| `HATCHEDIT` | `HE` | Modifies an existing hatch or fill. |
| `MLEADER` | `LE` `LEADER` `MLD` `QLEADER` | Creates a leader line with annotation text. |
| `MTEXT` | `MT` `T` | Creates a multiline text object. |
| `TEXT` | `DT` `DTEXT` | Creates a single-line text object. |
| `TEXTEDIT` | `ED` | Edits a dimensional constraint, dimension text, or text object. |

## Block

| Command | Aliases | Description |
|---|---|---|
| `-RENAME` | `REN` `RENAME` | Changes the names assigned to items such as layers and blocks. |
| `BLOCK` | `B` | Creates a block definition from selected objects. |
| `GROUP` | `G` | Creates and manages saved sets of objects called groups. |
| `INSERT` | `-INSERT` `I` | Inserts a block or drawing into the current drawing. |
| `PURGE` | `PU` | Removes unused items, such as block definitions and layers, from the drawing. |
| `UNGROUP` | `UG` | Dissolves a group. |
| `WBLOCK` | `W` | Writes objects or a block to a new drawing file. |

## Draw

| Command | Aliases | Description |
|---|---|---|
| `ARC` | `A` | Creates an arc. |
| `CIRCLE` | `C` | Creates a circle. |
| `DONUT` | `DO` | Creates a filled circle or a wide ring. |
| `ELLIPSE` | `EL` | Creates an ellipse or an elliptical arc. |
| `LINE` | `L` | Creates straight line segments. |
| `PLINE` | `PL` | Creates a 2D polyline: a single object made of connected line and arc segments. |
| `POINT` | `PO` | Creates a point object. |
| `POLYGON` | `POL` | Creates an equilateral closed polyline. |
| `RAY` |  | Creates a semi-infinite line. |
| `RECTANG` | `REC` `RECTANGLE` | Creates a rectangular polyline. |
| `SPLINE` | `SPL` | Creates a smooth curve through fit points. |
| `XLINE` | `XL` | Creates a line of infinite length. |

## Inquiry

| Command | Aliases | Description |
|---|---|---|
| `AREA` | `AA` | Calculates the area and perimeter of objects or of defined areas. |
| `DIST` | `DI` | Measures the distance and angle between two points. |
| `ID` |  | Displays the UCS coordinate values of a specified location. |
| `LIST` | `LI` `LS` | Displays property data for selected objects. |
| `MEASUREGEOM` | `MEA` | Measures distances, radii, angles, areas, and volumes of selected objects. |

## Layers

| Command | Aliases | Description |
|---|---|---|
| `-LAYER` |  | Manages layers from the command line. |
| `COLOR` | `COL` | Sets the color for new objects. |
| `LAYCUR` |  | Changes the layer of selected objects to the current layer. |
| `LAYDEL` |  | Deletes the layers of selected objects and the objects on them. |
| `LAYER` | `LA` | Manages layers and layer properties. |
| `LAYFRZ` |  | Freezes the layer of selected objects. |
| `LAYISO` |  | Hides or locks all layers except those of the selected objects. |
| `LAYLCK` |  | Locks the layer of a selected object. |
| `LAYMCH` | `LAYMATCH` | Changes the layer of selected objects to match the layer of a destination object. |
| `LAYMCUR` |  | Sets the current layer to that of a selected object. |
| `LAYOFF` |  | Turns off the layer of selected objects. |
| `LAYON` |  | Turns on all layers in the drawing. |
| `LAYTHW` |  | Thaws all layers in the drawing. |
| `LAYULK` |  | Unlocks the layer of a selected object. |
| `LAYUNISO` |  | Restores layers hidden or locked by LAYISO. |
| `LINETYPE` | `LT` `LTYPE` | Loads, sets, and modifies linetypes. |
| `LTSCALE` | `LTS` | Sets the global linetype scale factor. |
| `LWEIGHT` | `LINEWEIGHT` `LW` | Sets the current lineweight, lineweight display options, and lineweight units. |

## Modify

| Command | Aliases | Description |
|---|---|---|
| `ARRAY` | `AR` | Creates copies of objects arranged in a pattern. |
| `ARRAYPOLAR` |  | Evenly distributes copies of the selected object in a circular pattern around a center point. |
| `ARRAYRECT` |  | Distributes copies of the selected object into any combination of rows and columns. |
| `BREAK` | `BR` | Breaks the selected object between two points. |
| `BREAKATPOINT` |  | Breaks the selected object at a single point. |
| `CHAMFER` | `CHA` | Bevels the edges of objects. |
| `COPY` | `CO` `CP` | Copies objects a specified distance in a specified direction. |
| `COPYCLIP` |  | Copies selected objects to the clipboard. |
| `CUTCLIP` |  | Copies selected objects to the clipboard and removes them from the drawing. |
| `DIVIDE` | `DIV` | Creates evenly spaced point objects or blocks along the length or perimeter of an object. |
| `ERASE` | `DEL` `E` | Removes objects from the drawing. |
| `EXPLODE` | `X` | Breaks a compound object into its component objects. |
| `EXTEND` | `EX` | Extends objects to meet the edges of other objects. |
| `FILLET` | `F` | Rounds and fillets the edges of objects. |
| `JOIN` | `J` | Joins the endpoints of linear and curved objects to create a single object. |
| `LENGTHEN` | `LEN` | Changes the length of objects and the included angle of arcs. |
| `MATCHPROP` | `MA` | Applies the properties of a selected object to other objects. |
| `MEASURE` | `ME` | Creates point objects or blocks at measured intervals along the length or perimeter of an object. |
| `MIRROR` | `MI` | Creates a mirrored copy of selected objects. |
| `MOVE` | `M` | Moves objects a specified distance in a specified direction. |
| `OFFSET` | `O` | Creates concentric circles, parallel lines, and parallel curves. |
| `OOPS` |  | Restores objects deleted by the last ERASE command. |
| `PASTECLIP` |  | Inserts data from the clipboard into the drawing. |
| `PEDIT` | `PE` | Edits polylines and 3D polygon meshes. |
| `ROTATE` | `RO` | Rotates objects around a base point. |
| `SCALE` | `SC` | Enlarges or reduces selected objects, keeping the proportions the same. |
| `STRETCH` | `S` | Stretches objects crossed by a selection window or polygon. |
| `TRIM` | `TR` | Trims objects to meet the edges of other objects. |

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
| `EXPORTPDF` |  | Exports the layout to a PDF file. |
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

## View

| Command | Aliases | Description |
|---|---|---|
| `-PAN` |  | Shifts the view by a specified displacement. |
| `-VIEW` | `V` `VIEW` | Saves and restores named views. |
| `3DORBIT` | `3DO` `ORBIT` | Rotates the view in 3D space, but constrained to horizontal and vertical orbit only. |
| `PAN` | `P` | Shifts the view in the current viewport. |
| `REDRAW` | `R` | Refreshes the display in the current viewport. |
| `REGEN` | `RE` | Regenerates the drawing and redraws the current viewport. |
| `REGENALL` | `REA` | Regenerates the drawing and redraws all viewports. |
| `ZOOM` | `Z` | Increases or decreases the magnification of the view in the current viewport. |

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
| `POLARANG` | `90` |
| `POLARMODE` | `0` |
| `QPMODE` | `-1` |
| `SELECTIONPREVIEW` | `3` |
| `SHORTCUTMENU` | `11` |
| `SNAPANG` | `0` |
| `SNAPMODE` | `0` |
| `SNAPUNIT_X` | `10` |
| `SNAPUNIT_Y` | `10` |
| `THEME` | `dark` |
| `TOOLTIPS` | `1` |
| `TRANSPARENCYDISPLAY` | `1` |
| `UCSICON` | `1` |
| `VPCONTROL` | `1` |
