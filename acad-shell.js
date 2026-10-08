'use strict';
// CadForge application frame: title bar (application menu, Quick Access Toolbar, workspace, search), ribbon, file tabs, Start page.
{
const sh={tab:{drafting:'Home','3d':'Home'},split:{},buttons:[],qat:[],tabButtons:[],panels:[],menus:[],recent:[],start:false,sig:'',suppress:null};
const WS_LABEL={drafting:'Drafting & Annotation','3d':'3D Modeling'};
const ce=(tag,cls,text)=>{const e=document.createElement(tag);if(cls)e.className=cls;if(text!=null)e.textContent=text;return e};
const esc=s=>String(s??'').replace(/[&<>"]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]));
const ico=(n,s=16,cls)=>CF.icon(n,s,cls);
const plain=l=>String(l||'').replace(/\n/g,' ');
const btn=(cls,html,label)=>{const b=ce('button',cls);b.type='button';if(html)b.innerHTML=html;if(label)b.setAttribute('aria-label',label);return b};
const shellNotify=m=>{try{notify(m)}catch{}};
// ---- Command catalog used for tooltips/search when a command is not registered yet: [aliases, description] ----------
const INFO={LINE:['L','Creates straight line segments.'],PLINE:['PL','Creates a 2D polyline, a single object made of line segments.'],CIRCLE:['C','Creates a circle from a center point and a radius or diameter.'],
 ARC:['A','Creates an arc from a center, start and end point.'],RECTANG:['REC, RECTANGLE','Creates a rectangular polyline.'],POLYGON:['POL','Creates an equilateral closed polyline.'],
 ELLIPSE:['EL','Creates an ellipse from axis endpoints or a center point.'],HATCH:['H, BHATCH','Fills an enclosed area with a hatch pattern.'],TEXT:['DT, DTEXT, T','Creates single-line text objects.'],
 MTEXT:['MT','Creates text objects with a start point, height and contents.'],DIMLINEAR:['DLI, DIM','Creates a horizontal or vertical linear dimension.'],DIMALIGNED:['DAL','Creates a dimension aligned to the measured points.'],
 MOVE:['M','Moves objects a specified distance in a specified direction.'],COPY:['CO, CP','Copies objects a specified distance in a specified direction.'],ROTATE:['RO','Rotates objects around a base point.'],
 SCALE:['SC','Enlarges or reduces selected objects, keeping their proportions.'],MIRROR:['MI','Creates a mirrored copy of selected objects.'],OFFSET:['O','Creates concentric circles, parallel lines and parallel polylines.'],
 TRIM:['TR','Trims objects to meet the edges of other objects.'],EXTEND:['EX','Extends objects to meet the edges of other objects.'],FILLET:['F','Rounds or sharpens the corner between two lines.'],
 CHAMFER:['CHA','Bevels the corner between two lines.'],ARRAYRECT:['AR, ARRAY','Distributes copies of objects into rows and columns.'],ERASE:['E','Removes objects from the drawing.'],
 EXPLODE:['X','Breaks a compound object into its component objects.'],JOIN:['J','Joins connected lines into a single closed profile.'],BLOCK:['B','Creates a block definition from selected objects.'],
 INSERT:['I','Inserts a block into the drawing.'],MATCHPROP:['MA','Applies the properties of a selected object to other objects.'],PROPERTIES:['PR, PROPS, CH, MO','Controls properties of existing objects.'],
 LAYER:['LA','Manages layers and layer properties.'],LIST:['LI','Displays property data for selected objects.'],DIST:['DI','Measures the distance and angle between two points.'],ID:['','Displays the coordinate values of a location.'],
 MEASUREGEOM:['MEA','Measures the length and area of selected objects.'],SELECTALL:['AI_SELALL','Selects all objects on visible layers.'],PASTECLIP:['','Pastes objects from the clipboard at an insertion point.'],
 COPYCLIP:['','Copies selected objects to the clipboard.'],CUTCLIP:['','Copies selected objects to the clipboard and removes them from the drawing.'],ZOOM:['Z','Increases or decreases the magnification of the view.'],
 PAN:['P','Moves the view in the drawing area.'],REGEN:['RE','Regenerates the drawing display.'],'3DORBIT':['3DO, ORBIT','Rotates the view in 3D space.'],PLAN:['','Displays the plan (top) view of the drawing.'],
 VIEW:['V','Saves and restores named views.'],VSCURRENT:['VS','Sets the visual style of the current viewport.'],NEW:['','Creates a new drawing.'],OPEN:['','Opens a CadForge project or a DXF file.'],
 QSAVE:['SAVE','Saves the current drawing.'],SAVEAS:['','Saves a copy of the drawing under a new file name.'],PLOT:['PRINT','Plots the drawing to a PDF or SVG sheet.'],PAGESETUP:['','Controls paper size, orientation and plot scale.'],
 EXPORT:['EXP','Saves the drawing in another file format.'],DXFOUT:['','Exports the drawing to a DXF file.'],DXFIN:['','Imports a DXF file.'],SVGOUT:['','Exports the drawing to an SVG file.'],
 STLOUT:['','Exports meshes to an STL file.'],OBJEXPORT:['','Exports meshes to an OBJ file.'],UNDO:['U','Reverses the most recent action.'],REDO:['','Reverses the effects of the previous UNDO.'],
 HELP:['','Lists the available commands.'],PURGE:['PU','Removes unused block definitions and empty layers.'],EXTRUDE:['EXT','Creates a 3D solid by extruding a closed profile.'],BOX:['','Creates a 3D solid box.'],
 UNION:['UNI','Combines two solids into one.'],SUBTRACT:['SU','Subtracts one solid from another.'],INTERSECT:['IN','Keeps only the volume common to two solids.'],UPDATEEXTRUSION:['','Rebuilds a linked extrusion after its profile changed.'],
 '3DMOVE':['3M','Moves the selected mesh in 3D.'],MESHCOPY:['','Duplicates the selected mesh.'],MESHCLEAR:['','Removes all meshes from the drawing.'],MESHFIT:['','Zooms the 3D view to fit all meshes.'],
 COMMANDLINE:['','Displays the command line.'],CLEANSCREENON:['','Clears the ribbon, tabs and palettes from the screen.'],RIBBON:['','Opens the ribbon.'],RIBBONCLOSE:['','Closes the ribbon.'],
 WSCURRENT:['','Sets the current workspace.'],MODEL:['','Switches to model space.'],LAYOUT:['','Switches to the layout (paper space).']};
const KEYS={NEW:'Ctrl+N',OPEN:'Ctrl+O',QSAVE:'Ctrl+S',SAVEAS:'Ctrl+Shift+S',PLOT:'Ctrl+P',UNDO:'Ctrl+Z',REDO:'Ctrl+Y',COPYCLIP:'Ctrl+C',CUTCLIP:'Ctrl+X',PASTECLIP:'Ctrl+V',PROPERTIES:'Ctrl+1',HELP:'F1',SELECTALL:'Ctrl+A',COMMANDLINE:'Ctrl+9',CLEANSCREENON:'Ctrl+0'};
const FLAGKEYS={properties:'Ctrl+1',commandLine:'Ctrl+9',clean:'Ctrl+0'};
// Engine tool behind a command (active-button highlighting + standalone fallback).
const TOOLS={LINE:'line',PLINE:'polyline',CIRCLE:'circle',ARC:'arc',RECTANG:'rectangle',TEXT:'text',MTEXT:'text',MOVE:'move',COPY:'copy',ROTATE:'rotate',SCALE:'scale',MIRROR:'mirror',OFFSET:'offset',TRIM:'trim',EXTEND:'extend',ERASE:'delete',DIMALIGNED:'dimension',INSERT:'insert'};
// Fallbacks used only while a command is not registered (e.g. a bundle without the command-line module).
const FALLBACK={NEW:()=>CF.newDrawing(),OPEN:()=>CF.openFile(),DXFIN:()=>CF.openFile(),QSAVE:()=>CF.saveProject(),SAVEAS:()=>CF.saveAs(),PLOT:()=>plotSheet(),PAGESETUP:()=>plotSheet(),EXPORT:()=>plotSheet(),
 UNDO:()=>undo(),REDO:()=>undo(true),DXFOUT:()=>$('dxf').onclick(),SVGOUT:()=>$('svg').onclick(),STLOUT:()=>exportSTL(),OBJEXPORT:()=>exportOBJ(),LAYER:()=>CF.openLayerManager(),PROPERTIES:()=>CF.toggle('properties'),
 HATCH:()=>hatch(),EXPLODE:()=>explode(),BLOCK:()=>createBlock(),JOIN:()=>joinProfile(),EXTRUDE:()=>extrude(),BOX:()=>box3D(),UNION:()=>applyBoolean('union'),SUBTRACT:()=>applyBoolean('subtract'),
 INTERSECT:()=>applyBoolean('intersect'),UPDATEEXTRUSION:()=>updateExtrusion(),'3DMOVE':()=>translateMesh(),MESHCOPY:()=>duplicateMesh(),MESHCLEAR:()=>clearMeshes(),MESHFIT:()=>fit3D(),'3DORBIT':()=>show3D(),
 PLAN:()=>showModel(),PAN:()=>CF.startPan(),REGEN:()=>render(),MEASUREGEOM:()=>measure(),SELECTALL:()=>selectAll(),CHAMFER:()=>chamfer(),MODEL:()=>CF.setSpace('model'),LAYOUT:()=>CF.setSpace('layout'),
 COMMANDLINE:()=>CF.toggle('commandLine',true),CLEANSCREENON:()=>CF.toggle('clean'),INSERT:()=>startInsert(),
 ZOOM:it=>{const o=String(it?.opt||'E')[0].toUpperCase();if(mode3D){fit3D();return}if(o==='I'||o==='O'){view.scale=Math.max(.01,Math.min(1000,view.scale*(o==='I'?2:.5)));render()}else fit()}};
for(const [c,t]of Object.entries(TOOLS))FALLBACK[c]??=()=>setTool(t);

// ---- Ribbon model (plain data; run/fallback functions only where no command exists) -------------------------------
function shellRibbonModel(ws=CF.workspace){
 const L=(cmd,label,icon,o)=>({kind:'button',size:'large',cmd,label,icon:icon||cmd.toLowerCase(),...o}),
  S=(cmd,label,icon,o)=>({kind:'button',size:'small',cmd,label,icon:icon||cmd.toLowerCase(),...o}),
  A=(size,label,icon,run,o)=>({kind:'button',size,label,icon,run,...o}),
  T=(size,flag,label,icon,o)=>({kind:'button',size,flag,label,icon,...o}),
  SP=(size,id,items,o)=>({kind:'split',size,id,items,...o}),COL=(...items)=>({kind:'col',items}),ROW=(...items)=>({kind:'row',items}),
  P=(title,items,priority=5,more)=>({title,items,priority,...(more?{more}:{})}),tab=(name,panels)=>({name,panels});
 const vs=[['2dwireframe','2D Wireframe'],['wireframe','Wireframe'],['shaded','Shaded'],['shadededges','Shaded with Edges']].map(([n,l])=>({kind:'button',size:'large',label:l,icon:'vs-'+n,hint:'VSCURRENT',style:n,run:()=>CF.setVisualStyle(n),desc:`Sets the ${l} visual style.`}));
 const views=[['top','Top'],['bottom','Bottom'],['left','Left'],['right','Right'],['front','Front'],['back','Back'],['sw','SW Isometric'],['se','SE Isometric'],['ne','NE Isometric'],['nw','NW Isometric']].map(([n,l])=>({kind:'button',size:'large',label:l,icon:'view-'+(n.length===2?'iso':n),hint:'VIEW',preset:n,run:()=>CF.setViewPreset(n),desc:`Sets the ${l} view.`}));
 const zoom=size=>[['E','Extents','zoom-extents'],['W','Window','zoom-window'],['P','Previous','zoom-previous'],['I','Zoom In','zoom-in'],['O','Zoom Out','zoom-out']].map(([o,l,i])=>({kind:'button',size,cmd:'ZOOM',opt:o,label:l,icon:i}));
 const rect=()=>SP('small','rect',[S('RECTANG','Rectangle','rectangle'),S('POLYGON','Polygon'),S('ELLIPSE','Ellipse')]);
 const measure=()=>SP('large','measure',[L('DIST','Distance','dist'),L('MEASUREGEOM','Area / Length','measure-area'),L('LIST','List'),L('ID','ID Point','id')],{label:'Measure'});
 const layers=priority=>P('Layers',[L('LAYER','Layer\nProperties','layer-properties'),COL(
  ROW(A('icon','Make Object\'s Layer Current','layer-current',shellLayerFromSelection,{hint:'LAYMCUR',desc:'Makes the layer of the selected object the current layer.'}),
   A('icon','New Layer','layer-new',()=>$('addLayer').onclick(),{desc:'Creates a new layer and makes it current.'})),
  {kind:'widget',id:'layer'},
  ROW(A('icon','Current Layer On/Off','layer-off',shellToggleLayer,{desc:'Turns the current layer on or off.'}),A('icon','Turn All Layers On','layer-allon',shellAllLayersOn,{hint:'LAYON',desc:'Turns on all layers in the drawing.'})))],priority);
 const common={
  Insert:[P('Block',[L('INSERT','Insert'),COL(S('EXPLODE','Explode'))],6),P('Block Definition',[L('BLOCK','Create\nBlock','block')],5),
   P('Import',[L('OPEN','Open','open'),L('DXFIN','Import\nDXF','import')],5),
   P('Export',[L('DXFOUT','DXF','file-dxf'),COL(S('SVGOUT','SVG','file-svg'),S('EXPORT','PDF','file-pdf',{opt:'Pdf',fallback:()=>plotSheet()})),COL(S('OBJEXPORT','OBJ','file-obj'),S('STLOUT','STL','file-stl'))],4)],
  Annotate:[P('Text',[SP('large','atext',[L('MTEXT','Multiline\nText','mtext'),L('TEXT','Single Line','text')])],8),
   P('Dimensions',[L('DIMLINEAR','Dimension','dimlinear'),COL(SP('small','adim',[S('DIMLINEAR','Linear'),S('DIMALIGNED','Aligned')]),S('DIMALIGNED','Aligned'))],8),
   P('Hatch',[L('HATCH','Hatch')],6),P('Measure',[measure(),COL(S('DIST','Distance'),S('LIST','List'),S('ID','ID Point'))],5)],
  View:[P('Viewport Tools',[T('large','ucsIcon','UCS\nIcon','ucs',{desc:'Shows or hides the UCS icon.'}),T('large','viewCube','View\nCube','viewcube',{desc:'Shows or hides the ViewCube.'}),T('large','navBar','Navigation\nBar','navbar',{desc:'Shows or hides the navigation bar.'})],7),
   P('Named Views',[L('VIEW','Save\nView','view-save',{opt:'Save',fallback:()=>saveLayout(),desc:'Saves the current view under a name.'}),L('VIEW','Restore\nView','view-restore',{opt:'Restore',fallback:()=>restoreLayout(),desc:'Restores a named view.'})],5),
   P('Views',[SP('large','preset',views)],6),P('Visual Styles',[SP('large','vstyle',vs)],6),
   P('Navigate',[SP('large','zoom',zoom('large')),COL(S('PAN','Pan'),S('3DORBIT','Orbit','orbit'),S('REGEN','Regen'))],8),
   P('Palettes',[L('PROPERTIES','Properties','properties',{flag:'properties'}),L('LAYER','Layer\nProperties','layer-properties')],4),
   P('Interface',[T('large','fileTabs','File\nTabs','filetabs',{desc:'Shows or hides the drawing file tabs.'}),T('large','layoutTabs','Layout\nTabs','layouttabs',{desc:'Shows or hides the Model and Layout tabs.'}),
    COL(T('small','commandLine','Command Line','commandline',{cmd:'COMMANDLINE'}),T('small','clean','Clean Screen','clean',{cmd:'CLEANSCREENON'}),T('small','ribbonMin','Minimize Ribbon','ribbon-min',{desc:'Minimizes the ribbon to tab titles.'}))],3)],
  Manage:[P('Cleanup',[L('PURGE','Purge')],5),P('Help',[L('HELP','Command\nList','help')],5),
   P('Workspaces',[A('large','Drafting &\nAnnotation','workspace',()=>CF.setWorkspace('drafting'),{hint:'WSCURRENT',isActive:()=>CF.workspace==='drafting',desc:'2D drafting ribbon and tools.'}),
    A('large','3D\nModeling','box',()=>CF.setWorkspace('3d'),{hint:'WSCURRENT',isActive:()=>CF.workspace==='3d',desc:'Solid and mesh modeling ribbon and tools.'})],5)],
  Output:[P('Plot',[L('PLOT','Plot'),COL(S('PAGESETUP','Page Setup'))],6),
   P('Export',[L('EXPORT','Export\nPDF','file-pdf',{opt:'Pdf',fallback:()=>plotSheet(),desc:'Plots the drawing to a vector PDF sheet.'}),COL(S('DXFOUT','DXF','file-dxf'),S('SVGOUT','SVG','file-svg')),COL(S('OBJEXPORT','OBJ','file-obj'),S('STLOUT','STL','file-stl'))],5)]};
 const tail=['Insert','Annotate','View','Manage','Output'].map(n=>tab(n,common[n]));
 if(ws==='3d')return[
  tab('Home',[P('Modeling',[L('BOX','Box'),L('EXTRUDE','Extrude'),COL(S('UPDATEEXTRUSION','Update','update'),S('JOIN','Join Profile','join'),S('MESHFIT','Fit Meshes','mesh-fit'))],10),
   P('Solid Editing',[COL(S('UNION','Union'),S('SUBTRACT','Subtract'),S('INTERSECT','Intersect'))],9),
   P('Mesh',[COL(S('3DMOVE','3D Move','mesh-move'),S('MESHCOPY','Copy Mesh','mesh-copy'),S('MESHCLEAR','Clear Meshes','mesh-clear'))],8),
   P('Draw',[L('LINE','Line'),COL(S('PLINE','Polyline','polyline'),S('CIRCLE','Circle'),rect())],7),
   P('Modify',[COL(S('MOVE','Move'),S('COPY','Copy'),S('ROTATE','Rotate')),COL(S('MIRROR','Mirror'),S('ERASE','Erase'),S('EXPLODE','Explode'))],6),
   P('View',[COL(SP('small','vstyle',vs.map(v=>({...v,size:'small'}))),SP('small','preset',views.map(v=>({...v,size:'small'}))),S('3DORBIT','Orbit','orbit')),COL(S('ZOOM','Extents','zoom-extents',{opt:'E'}),S('PLAN','Plan View','plan'),S('PAN','Pan'))],5),
   layers(4)]),
  tab('Solid',[P('Primitive',[L('BOX','Box'),L('EXTRUDE','Extrude')],9),P('Boolean',[L('UNION','Union'),L('SUBTRACT','Subtract'),L('INTERSECT','Intersect')],9),
   P('Solid Editing',[L('UPDATEEXTRUSION','Update\nExtrusion','update'),COL(S('JOIN','Join Profile','join'),S('3DMOVE','3D Move','mesh-move'))],7),P('View',[SP('large','vstyle',vs),SP('large','preset',views)],5)]),
  tab('Mesh',[P('Mesh',[L('3DMOVE','3D Move','mesh-move'),L('MESHCOPY','Copy','mesh-copy'),L('MESHCLEAR','Clear','mesh-clear'),L('MESHFIT','Fit','mesh-fit')],8),
   P('Export',[L('STLOUT','STL','file-stl'),L('OBJEXPORT','OBJ','file-obj')],6)]),...tail];
 return[tab('Home',[
  P('Draw',[L('LINE','Line'),L('PLINE','Polyline','polyline'),L('CIRCLE','Circle'),L('ARC','Arc'),COL(rect(),S('HATCH','Hatch'),S('TEXT','Text'))],10,
   [S('POLYGON','Polygon'),S('ELLIPSE','Ellipse'),S('DIMALIGNED','Aligned Dimension'),S('ID','Point Location','id')]),
  P('Modify',[COL(S('MOVE','Move'),S('COPY','Copy'),S('SCALE','Scale')),COL(S('ROTATE','Rotate'),S('MIRROR','Mirror'),S('ARRAYRECT','Array','array')),
   COL(SP('small','trim',[S('TRIM','Trim'),S('EXTEND','Extend')]),SP('small','fillet',[S('FILLET','Fillet'),S('CHAMFER','Chamfer')]),S('OFFSET','Offset')),
   COL(S('ERASE','Erase'),S('EXPLODE','Explode'),S('JOIN','Join'))],10),
  P('Annotation',[SP('large','text',[L('MTEXT','Multiline\nText','mtext'),L('TEXT','Single Line','text')],{label:'Text'}),SP('large','dim',[L('DIMLINEAR','Linear'),L('DIMALIGNED','Aligned')],{label:'Dimension'}),
   COL(S('DIMLINEAR','Linear'),S('DIMALIGNED','Aligned'),S('MEASUREGEOM','Measure','measure'))],8),
  layers(9),
  P('Block',[L('INSERT','Insert'),COL(S('BLOCK','Create'),S('EXPLODE','Explode'))],5),
  P('Properties',[L('MATCHPROP','Match\nProperties','match'),COL(S('PROPERTIES','Properties','properties',{flag:'properties'}),S('LIST','List'))],4),
  P('Utilities',[measure(),COL(S('SELECTALL','Select All'),S('ID','ID Point'))],3),
  P('Clipboard',[L('PASTECLIP','Paste'),COL(S('COPYCLIP','Copy Clip'),S('CUTCLIP','Cut'))],2),
  P('View',[COL(S('ZOOM','Extents','zoom-extents',{opt:'E'}),S('ZOOM','Window','zoom-window',{opt:'W'}),S('PAN','Pan'))],1)]),...tail];
}
// Every item of a model, flattened (splits, columns, rows and panel slide-outs included).
function shellItems(model){const out=[],walk=it=>{if(!it)return;if(it.items)it.items.forEach(walk);if(it.kind==='button')out.push(it)};for(const t of model)for(const p of t.panels){p.items.forEach(walk);(p.more||[]).forEach(walk)}return out}
const shellCommandNames=ws=>[...new Set(shellItems(shellRibbonModel(ws)).map(i=>i.cmd).filter(Boolean))];

// ---- Execution ------------------------------------------------------------------------------------------------
function shellItemActive(it){try{if(it.isActive)return !!it.isActive();if(it.flag)return CF.get(it.flag);if(!it.cmd)return false;const t=TOOLS[it.cmd]||it.cmd.toLowerCase();return (tool!=='select'&&tool===t)||String(CF.picking?.command||'').toUpperCase()===it.cmd}catch{return false}}
function shellRecent(name){if(!name)return;sh.recent=[name,...sh.recent.filter(n=>n!==name)].slice(0,8)}
// Ribbon macros pass an option keyword (ZOOM -> E). CF.run receives it as args; if the command still waits at an
// option prompt that offers it, the keyword is submitted through the command line like a typed response.
function shellFollow(opt){setTimeout(()=>{try{const i=CF.input,m=String(i?.message||'').match(/\[([^\]]*)\]/);if(i&&m&&CF.commandLine?.submit&&m[1].split('/').some(o=>o.trim().toUpperCase().startsWith(opt[0].toUpperCase())))CF.commandLine.submit(opt)}catch{}},0)}
function shellExec(it,entry,index){
 if(entry&&index!=null){sh.split[entry.id]=index;entry.render()}
 closeMenus();hideTip();
 if(sh.start&&!['OPEN','DXFIN'].includes(it.cmd))showStart(false);
 try{let r;
  if(it.flag)r=CF.toggle(it.flag);
  else if(it.run)r=it.run(it);
  else if(it.cmd){if(CF.resolve(it.cmd)){CF.run(it.cmd,{source:'ribbon',args:it.opt});if(it.opt)shellFollow(it.opt)}
   else{const fb=typeof it.fallback==='function'?it.fallback:FALLBACK[it.cmd];if(fb){shellRecent(it.cmd);r=fb(it)}else CF.run(it.cmd,{source:'ribbon'})}}
  if(r&&typeof r.then==='function')r.catch(err=>shellNotify(`${it.cmd||plain(it.label)} failed: ${err?.message||err}`));
 }catch(err){shellNotify(`${it.cmd||plain(it.label)} failed: ${err?.message||err}`)}
 shellRefresh();
}
function shellLayerFromSelection(){const e=doc.entities[CF.selection()[0]];if(!e){shellNotify('Select an object whose layer will become current.');return}$('layer').value=e.layer;syncLayers();render();shellNotify(`${e.layer} is now the current layer.`)}
function shellToggleLayer(){const v=$('visible');v.checked=!v.checked;v.onchange?.();syncLayers();render()}
function shellAllLayersOn(){mutate(()=>doc.layers.forEach(l=>l.visible=true));syncLayers();shellNotify('All layers have been turned on.')}

// ---- Tooltips, menus and popups ----------------------------------------------------------------------------------
const tipEl=ce('div','cf-tip');
function shellTipData(it){const cmd=it.cmd||it.hint,def=cmd?CF.resolve(cmd):null,info=INFO[cmd]||[],aliases=def?def.aliases.filter(a=>a!==cmd):(info[0]?info[0].split(', '):[]);
 return{title:plain(it.label),desc:it.desc||def?.desc||info[1]||'',command:cmd?`${cmd}${aliases.length?' ('+aliases.slice(0,3).join(', ')+')':''}`:'',key:KEYS[cmd]||FLAGKEYS[it.flag]||''}}
function tip(el,fn){try{el.addEventListener('mouseenter',()=>{clearTimeout(sh.tipT);sh.tipT=setTimeout(()=>showTip(el,fn()),sh.tipFast?150:650)});el.addEventListener('mouseleave',hideTip);el.addEventListener('mousedown',hideTip)}catch{}}
function showTip(el,d){if(!d||!el.isConnected)return;tipEl.innerHTML=`<b>${esc(d.title)}${d.key?`<kbd>${esc(d.key)}</kbd>`:''}</b>${d.desc?`<p>${esc(d.desc)}</p>`:''}${d.command?`<div class="cf-tip-cmd">${ico('commandline',14)}<span>${esc(d.command)}</span></div>`:''}`;
 tipEl.style.display='block';const r=el.getBoundingClientRect(),w=tipEl.offsetWidth||260;let x=r.left;if(x+w>innerWidth-4)x=Math.max(4,innerWidth-w-4);tipEl.style.left=x+'px';tipEl.style.top=(r.bottom+6)+'px';sh.tipFast=true}
function hideTip(){clearTimeout(sh.tipT);tipEl.style.display='none';clearTimeout(sh.fastT);sh.fastT=setTimeout(()=>sh.tipFast=false,500)}
function place(el,anchor,align){if(!el.isConnected)document.body.append(el);const r=anchor.getBoundingClientRect(),w=el.offsetWidth||0,h=el.offsetHeight||0;let x=align==='right'?r.right-w:r.left,y=r.bottom+1;
 if(x+w>innerWidth-2)x=Math.max(2,innerWidth-w-2);if(y+h>innerHeight-2)y=Math.max(2,r.top-h-1);el.style.left=Math.max(0,x)+'px';el.style.top=y+'px'}
function closeMenus(){for(const m of sh.menus.splice(0)){try{m.onClose?.()}catch{}if(!m.keep)m.el.remove()}}
function openMenu(items,anchor,o={}){closeMenus();hideTip();const m=ce('div','cf-menu'+(o.cls?' '+o.cls:''));
 for(const it of items){if(it.sep){m.append(ce('div','cf-msep'));continue}if(it.header){m.append(ce('div','cf-mhead',it.header));continue}
  const b=btn('cf-mi'+(it.big?' big':'')+(it.checked?' checked':''));b.innerHTML=`<span class="cf-mi-ico">${it.icon?ico(it.icon,it.big?24:16):it.checked?ico('check',14):''}</span><span class="cf-mi-lbl">${esc(it.label)}</span>${it.key?`<span class="cf-mi-key">${esc(it.key)}</span>`:''}`;
  if(it.disabled)b.disabled=true;b.onclick=()=>{closeMenus();it.run?.()};m.append(b)}
 place(m,anchor,o.align);sh.menus.push({el:m,owner:o.owner||anchor});return m}
// Menu openers ignore the click that follows a pointerdown which just closed their own menu (toggle behaviour).
const opener=(el,fn)=>{el.onclick=e=>{if(sh.suppress===el){sh.suppress=null;return}fn(e)}};

// ---- Ribbon ------------------------------------------------------------------------------------------------------
const lbl=l=>esc(l).replace('\n','<br>');
function makeButton(it,size=it.size){const b=btn(`cf-rb cf-rb-${size}`,null,plain(it.label));
 b.innerHTML=size==='large'?ico(it.icon,32)+`<span class="cf-lbl">${lbl(it.label)}</span>`:size==='icon'?ico(it.icon,16):ico(it.icon,16)+`<span class="cf-lbl">${esc(plain(it.label))}</span>`;
 b.onclick=()=>shellExec(it);tip(b,()=>shellTipData(it));sh.buttons.push({el:b,item:it});return b}
function makeSplit(sp){const wrap=ce('div',`cf-split cf-split-${sp.size}`),main=btn('cf-rb cf-split-main'),arrow=btn('cf-rb cf-split-arrow'),items=sp.items;
 const entry={el:wrap,item:sp,split:sp.id,id:sp.id,items,main,arrow,current:()=>items[sh.split[sp.id]??0]||items[0]};
 entry.render=()=>{const it=entry.current(),label=sp.label||it.label;main.setAttribute('aria-label',plain(it.label));
  if(sp.size==='large'){main.innerHTML=ico(it.icon,32);arrow.innerHTML=`<span class="cf-lbl">${lbl(label)}${label.includes('\n')?' ':'<br>'}${ico('chevron-down',10,'cf-chev')}</span>`}
  else{main.innerHTML=ico(it.icon,16)+`<span class="cf-lbl">${esc(plain(label))}</span>`;arrow.innerHTML=ico('chevron-down',10,'cf-chev')}};
 main.onclick=()=>shellExec(entry.current());
 opener(arrow,()=>openMenu(items.map((it,i)=>({label:plain(it.label),icon:it.icon,big:sp.size==='large',checked:i===(sh.split[sp.id]??0),run:()=>shellExec(it,entry,i)})),wrap,{owner:arrow}));
 tip(main,()=>shellTipData(entry.current()));entry.render();wrap.append(main,arrow);sh.buttons.push(entry);return wrap}
function makeItem(it){
 if(it.kind==='button')return makeButton(it);
 if(it.kind==='split')return makeSplit(it);
 if(it.kind==='col'||it.kind==='row'){const d=ce('div',it.kind==='col'?'cf-col':'cf-row');for(const x of it.items)d.append(it.kind==='row'&&x.kind==='button'?makeButton(x,'icon'):makeItem(x));return d}
 if(it.kind==='widget'){const w=ce('div','cf-widget cf-widget-'+it.id);try{const el=CF.createLayerDropdown();if(el)w.append(el)}catch(err){console.error(err)}return w}
 return ce('span')}
const panelIcon=p=>{const first=shellItems([{panels:[p]}])[0];return first?.icon||'fallback'};
function makePanel(p,index){const el=ce('div','cf-panel'),body=ce('div','cf-panel-body'),title=ce('div','cf-panel-title'),cbtn=btn('cf-rb cf-rb-large cf-panel-cbtn',null,p.title);
 for(const it of p.items)body.append(makeItem(it));
 title.innerHTML=`<span>${esc(p.title)}</span>`+(p.more?ico('chevron-down',10,'cf-chev'):'');
 const entry={el,body,title,cbtn,priority:p.priority,index,model:p};
 if(p.more){title.classList.add('cf-has-more');title.setAttribute('title','Show more tools');opener(title,()=>openMore(entry))}
 cbtn.innerHTML=ico(panelIcon(p),32)+`<span class="cf-lbl">${esc(p.title)}<br>${ico('chevron-down',10,'cf-chev')}</span>`;opener(cbtn,()=>openCollapsed(entry));
 el.append(cbtn,body,title);sh.panels.push(entry);return el}
function openMore(entry){closeMenus();const pop=ce('div','cf-pop-panel cf-more'),col=ce('div','cf-more-col');for(const it of entry.model.more)col.append(makeButton(it,'small'));pop.append(col);
 place(pop,entry.el);sh.menus.push({el:pop,owner:entry.title})}
function openCollapsed(entry){closeMenus();const pop=ce('div','cf-pop-panel');pop.append(entry.body,entry.title);place(pop,entry.cbtn);
 sh.menus.push({el:pop,owner:entry.cbtn,onClose:()=>entry.el.append(entry.body,entry.title)})}
function shellTabs(){return(sh.model||[]).map(t=>t.name)}
function activeTab(){const ws=CF.workspace,names=shellTabs();if(!names.includes(sh.tab[ws]))sh.tab[ws]=names[0];return sh.tab[ws]}
// The engine reads #layer through getElementById, so the select must never be left detached when its panel is rebuilt.
const layerHome=(()=>{try{return $('layer')?.parentElement||null}catch{return null}})();
function shellRescueLayer(){try{const l=document.getElementById('layer');if(l&&layerHome&&CF.hosts.ribbon.contains(l))layerHome.append(l)}catch{}}
function buildRibbon(){sh.model=shellRibbonModel(CF.workspace);const host=CF.hosts.ribbonTabs;host.replaceChildren();sh.tabButtons=[];
 for(const t of sh.model){const b=btn('cf-rtab',null,t.name);b.textContent=t.name;b.dataset.tab=t.name;
  opener(b,()=>selectTab(t.name,true));b.ondblclick=()=>{if(t.name===activeTab()){closeMenus();CF.toggle('ribbonMin')}};sh.tabButtons.push(b);host.append(b)}
 const fill=ce('div','cf-rtab-fill'),min=btn('cf-rmin',null,'Minimize the ribbon');sh.minBtn=min;min.onclick=()=>{closeMenus();CF.toggle('ribbonMin')};
 tip(min,()=>({title:CF.get('ribbonMin')?'Show Full Ribbon':'Minimize to Tabs',desc:'Double-click a ribbon tab to toggle as well.',command:'RIBBON / RIBBONCLOSE'}));host.append(fill,min);
 buildPanels()}
function buildPanels(){const host=CF.hosts.ribbon,name=activeTab();closeMenus();shellRescueLayer();host.replaceChildren();sh.buttons=[];sh.panels=[];
 const t=sh.model.find(x=>x.name===name);t.panels.forEach((p,i)=>host.append(makePanel(p,i)));
 for(const b of sh.tabButtons)b.classList.toggle('active',b.dataset.tab===name);shellRefresh(true);fitRibbon()}
function selectTab(name,user){if(!shellTabs().includes(name))return false;sh.tab[CF.workspace]=name;buildPanels();if(user&&CF.get('ribbonMin'))showRibbonPop(sh.tabButtons.find(b=>b.dataset.tab===name));return true}
function showRibbonPop(owner){const host=CF.hosts.ribbon;host.classList.add('cf-pop');try{host.style.top=CF.hosts.ribbonTabs.getBoundingClientRect().bottom+'px'}catch{}
 sh.menus.push({el:host,owner,keep:true,onClose:()=>{host.classList.remove('cf-pop');host.style.top=''}});fitRibbon()}
// Shrink to fit: low-priority panels lose small-button labels then collapse to a single button; Draw/Modify go last.
function fitRibbon(){const host=CF.hosts.ribbon;if(!host||!(host.clientWidth>0)||!sh.panels.length)return;
 for(const p of sh.panels)p.el.classList.remove('cf-compact','cf-collapsed');
 const over=()=>host.scrollWidth>host.clientWidth+1;if(!over())return;
 const order=[...sh.panels].sort((a,b)=>a.priority-b.priority||b.index-a.index),low=order.filter(p=>p.priority<8),high=order.filter(p=>p.priority>=8);
 for(const [p,c]of [...low.flatMap(p=>[[p,'cf-compact'],[p,'cf-collapsed']]),...high.map(p=>[p,'cf-compact']),...high.map(p=>[p,'cf-collapsed'])]){
  const w=p.el.offsetWidth;p.el.classList.add(c);if(c==='cf-collapsed'&&p.el.offsetWidth>=w){p.el.classList.remove(c);continue} // never collapse into something wider
  if(!over())return}}
function shellRefresh(force){for(const b of sh.buttons){const on=b.split?b.items.some(shellItemActive):shellItemActive(b.item);b.el.classList.toggle('active',!!on)}
 if(sh.minBtn)sh.minBtn.innerHTML=ico(CF.get('ribbonMin')?'chevron-down':'chevron-up',14);
 if(sh.wsLbl)sh.wsLbl.textContent=WS_LABEL[CF.workspace]||CF.workspace}

// ---- Search (application menu + title bar) -------------------------------------------------------------------
function shellSearch(q,limit=12){const Q=String(q??'').trim().toUpperCase().replace(/^[_.'-]+/,'');if(!Q)return[];const out=[],seen=new Set();
 // Short queries only match the start of names, aliases and label words; substring and description hits need 3+ characters.
 const wordStart=s=>s.split(/[^A-Z0-9]+/).some(w=>w.startsWith(Q)),long=Q.length>=3;
 const score=(name,aliases,label,desc)=>name===Q?0:aliases.includes(Q)?1:name.startsWith(Q)?2:aliases.some(a=>a.startsWith(Q))?3:label.startsWith(Q)?4:wordStart(label)||wordStart(name)?5:long&&(name.includes(Q)||label.includes(Q))?6:long&&wordStart(desc)?7:99;
 for(const def of CF.commands.values()){const s=score(def.name,def.aliases||[],String(def.label||'').toUpperCase(),String(def.desc||'').toUpperCase());if(s<99){seen.add(def.name);out.push({s,name:def.name,label:def.label,desc:def.desc||INFO[def.name]?.[1]||'',icon:def.icon,aliases:def.aliases||[],run:()=>shellExec({cmd:def.name})})}}
 // Ribbon tools whose command is not registered (or that have no command, e.g. interface toggles) stay searchable.
 for(const ws of ['drafting','3d'])for(const it of shellItems(shellRibbonModel(ws))){const name=it.cmd&&!it.opt?it.cmd:plain(it.label).toUpperCase(),key=name+'|'+(it.opt||'');if(seen.has(name)||seen.has(key))continue;seen.add(key);
  const info=INFO[it.cmd]||[],aliases=info[0]?info[0].split(', '):[],s=score(name,aliases,plain(it.label).toUpperCase(),String(it.desc||info[1]||'').toUpperCase());
  if(s<99)out.push({s:s+.5,name:it.cmd&&!it.opt?it.cmd:plain(it.label),label:plain(it.label),desc:it.desc||info[1]||'',icon:it.icon,aliases,run:()=>shellExec(it)})}
 return out.sort((a,b)=>a.s-b.s||a.name.localeCompare(b.name)).slice(0,limit)}
function renderResults(list,box,state){box.replaceChildren();if(!list.length){box.append(ce('div','cf-sr-none','No matching commands'));return}
 list.forEach((r,i)=>{const b=btn('cf-sr'+(i===state.sel?' sel':''));b.innerHTML=ico(r.icon,20)+`<b>${esc(r.name)}</b><i>${esc(r.aliases.slice(0,2).join(', '))}</i><span>${esc(r.desc||r.label)}</span>`;b.onclick=()=>{closeMenus();r.run()};box.append(b)})}
function searchKeys(e,input,state,box,onEsc){e.stopPropagation();const list=state.list||[];
 if(e.key==='ArrowDown'||e.key==='ArrowUp'){e.preventDefault();if(!list.length)return;state.sel=(state.sel+(e.key==='ArrowDown'?1:-1)+list.length)%list.length;renderResults(list,box,state)}
 else if(e.key==='Enter'){e.preventDefault();const r=list[state.sel]||list[0];if(r){closeMenus();input.value='';r.run()}}
 else if(e.key==='Escape'){e.preventDefault();onEsc()}}

// ---- Title bar: application button/menu, QAT, workspace, title, search -------------------------------------------
const APP=()=>[
 {label:'New',icon:'new',cmd:'NEW',head:'Create a new drawing',sub:[{label:'Drawing',icon:'new',cmd:'NEW',desc:'Creates a new, empty drawing.'}]},
 {label:'Open',icon:'open',cmd:'OPEN',head:'Open a file',sub:[{label:'Drawing',icon:'open',cmd:'OPEN',desc:'Opens a CadForge project (.cadforge.json) or a DXF file.'},{label:'DXF',icon:'file-dxf',cmd:'DXFIN',desc:'Imports geometry from a DXF file.'}]},
 {label:'Save',icon:'save',cmd:'QSAVE'},
 {label:'Save As',icon:'saveas',cmd:'SAVEAS',head:'Save a copy of the drawing',sub:[{label:'Drawing',icon:'saveas',cmd:'SAVEAS',desc:'Saves the drawing as a CadForge project under a new name.'},{label:'DXF',icon:'file-dxf',cmd:'DXFOUT',desc:'Saves the drawing in DXF R12 format.'}]},
 {label:'Import',icon:'import',cmd:'DXFIN',head:'Import a file',sub:[{label:'DXF',icon:'file-dxf',cmd:'DXFIN',desc:'Imports lines, arcs, circles, polylines, text and blocks from a DXF file.'}]},
 {label:'Export',icon:'export',head:'Export a copy of the drawing in another file format',sub:[{label:'DXF',icon:'file-dxf',cmd:'DXFOUT',desc:'Drawing Exchange Format for other CAD applications.'},
  {label:'SVG',icon:'file-svg',cmd:'SVGOUT',desc:'Scalable vector graphics of the current view.'},{label:'OBJ',icon:'file-obj',cmd:'OBJEXPORT',desc:'Wavefront OBJ triangle meshes.'},
  {label:'STL',icon:'file-stl',cmd:'STLOUT',desc:'Stereolithography meshes for 3D printing.'},{label:'PDF',icon:'file-pdf',cmd:'EXPORT',opt:'Pdf',fallback:()=>plotSheet(),desc:'Vector PDF sheet through the Plot dialog.'}]},
 {label:'Print',icon:'plot',cmd:'PLOT',head:'Plot the drawing',sub:[{label:'Plot',icon:'plot',cmd:'PLOT',desc:'Plots the drawing to a PDF or SVG sheet.'},{label:'Page Setup',icon:'pagesetup',cmd:'PAGESETUP',desc:'Controls paper size, orientation and scale.'}]},
 {label:'Drawing Utilities',icon:'purge',head:'Tools to maintain the drawing',sub:[{label:'Purge',icon:'purge',cmd:'PURGE',desc:'Removes unused block definitions and empty layers.'},{label:'Command List',icon:'help',cmd:'HELP',desc:'Lists every available command in the command history.'}].filter(s=>CF.resolve(s.cmd))},
 {label:'Close',icon:'close',head:'Close',run:shellCloseDrawing,sub:[{label:'Current Drawing',icon:'close',run:shellCloseDrawing,desc:'Closes the current drawing and returns to the Start tab.'}]}
].filter(e=>!e.sub||e.sub.length);
async function shellCloseDrawing(){const before=doc;await CF.newDrawing();if(doc!==before)showStart(true)}
function openAppMenu(){closeMenus();hideTip();const m=ce('div','cf-appmenu'),head=ce('div','cf-am-head'),input=ce('input','cf-am-search'),body=ce('div','cf-am-body'),left=ce('div','cf-am-left'),right=ce('div','cf-am-right'),results=ce('div','cf-am-results');
 input.placeholder='Search commands';input.setAttribute('aria-label','Search commands');const glass=ce('span','cf-am-glass');glass.innerHTML=ico('search',16);head.append(glass,input);
 const entries=APP(),items=[];
 const pane=e=>{right.replaceChildren();for(const b of items)b.classList.toggle('sel',b._e===e);
  if(!e||!e.sub){right.append(ce('div','cf-am-h','Recent Commands'));if(!sh.recent.length)right.append(ce('div','cf-am-empty','No recent commands. Type a command name above or at the command line.'));
   for(const n of sh.recent){const def=CF.resolve(n),r=btn('cf-am-entry');r.innerHTML=ico(def?.icon||n.toLowerCase(),20)+`<div><b>${esc(n)}</b><span>${esc(def?.desc||INFO[n]?.[1]||'')}</span></div>`;r.onclick=()=>shellExec({cmd:n});right.append(r)}
   right.append(ce('div','cf-am-h cf-am-h2','Open Drawings'));const d=btn('cf-am-entry');d.innerHTML=ico('file-dxf',20).replace('DXF','')+`<div><b>${esc(CF.fileName)}${CF.modified?'*':''}</b><span>${doc.entities.length} objects, ${doc.layers.length} layer${doc.layers.length===1?'':'s'}${doc.solids?.length?`, ${doc.solids.length} mesh${doc.solids.length===1?'':'es'}`:''}</span></div>`;d.onclick=()=>{closeMenus();showStart(false)};right.append(d);return}
  right.append(ce('div','cf-am-h',e.head));for(const s of e.sub){const r=btn('cf-am-entry');r.innerHTML=ico(s.icon,32)+`<div><b>${esc(s.label)}</b><span>${esc(s.desc||'')}</span></div>`;r.onclick=()=>shellExec(s);tip(r,()=>s.cmd?{...shellTipData(s),title:s.label}:null);right.append(r)}};
 for(const e of entries){const b=btn('cf-am-item');b._e=e;b.innerHTML=ico(e.icon,28)+`<span>${esc(e.label)}</span>`+(e.sub?ico('chevron-right',12,'cf-chev'):'');b.onmouseenter=()=>pane(e);b.onclick=()=>{if(e.cmd||e.run)shellExec(e);else pane(e)};items.push(b);left.append(b)}
 body.append(left,right);pane(null);
 const state={sel:0,list:[]};
 input.oninput=()=>{state.list=shellSearch(input.value,14);state.sel=0;if(input.value.trim()){body.style.display='none';results.style.display='block';renderResults(state.list,results,state)}else{body.style.display='';results.style.display='none'}};
 input.onkeydown=e=>searchKeys(e,input,state,results,closeMenus);
 const foot=ce('div','cf-am-foot'),wsb=btn('cf-am-fbtn',null,'Workspace');wsb.innerHTML=ico('workspace',14)+`<span>${esc(WS_LABEL[CF.workspace])}</span>`;wsb.onclick=()=>{closeMenus();CF.setWorkspace(CF.workspace==='3d'?'drafting':'3d')};
 const help=btn('cf-am-fbtn',null,'Commands');help.innerHTML=ico('help',14)+'<span>Command List</span>';help.onclick=()=>shellExec({cmd:'HELP',fallback:()=>shellNotify('Type a command name or alias at the command line; F1 lists commands when the command line is available.')});
 foot.append(wsb,help);results.style.display='none';m.append(head,body,results,foot);document.body.append(m);sh.appBtn.classList.add('open');
 sh.menus.push({el:m,owner:sh.appBtn,onClose:()=>sh.appBtn.classList.remove('open')});try{input.focus()}catch{}}
const QAT=[['NEW','new','New'],['OPEN','open','Open'],['QSAVE','save','Save'],['SAVEAS','saveas','Save As'],['PLOT','plot','Plot'],['UNDO','undo','Undo'],['REDO','redo','Redo']];
function buildTitlebar(){const tb=CF.hosts.titlebar;tb.replaceChildren();
 const app=btn('cf-appbtn',ico('app',24),'Application menu');sh.appBtn=app;opener(app,openAppMenu);tip(app,()=>({title:'Application Menu',desc:'Create, open, save, import, export and plot drawings; search commands.'}));
 const qat=ce('div','cf-qat');sh.qat=[];
 for(const [cmd,icon,label]of QAT){const b=btn('cf-qbtn',ico(icon,16),label);b.onclick=()=>shellExec({cmd,label});tip(b,()=>shellTipData({cmd,label}));sh.qat.push({el:b,cmd,label,icon,shown:true});qat.append(b)}
 const ws=btn('cf-wsbtn',null,'Workspace');ws.innerHTML=ico('workspace',16)+'<span class="cf-wslbl"></span>'+ico('chevron-down',10,'cf-chev');sh.wsLbl=ws.querySelector?.('.cf-wslbl');
 // innerHTML children are not queryable in every environment: keep a real label element.
 const wsl=ce('span','cf-wslbl');ws.innerHTML='';ws.append(wsl);ws.insertAdjacentHTML?.('afterbegin',ico('workspace',16));ws.insertAdjacentHTML?.('beforeend',ico('chevron-down',10,'cf-chev'));sh.wsLbl=wsl;
 opener(ws,()=>openMenu([{header:'Workspaces'},...Object.entries(WS_LABEL).map(([k,l])=>({label:l,checked:CF.workspace===k,run:()=>CF.setWorkspace(k)}))],ws,{owner:ws}));
 tip(ws,()=>({title:'Workspace Switching',desc:'Switches between the Drafting & Annotation and 3D Modeling ribbons.',command:'WSCURRENT'}));
 const cust=btn('cf-qbtn cf-qcust',ico('chevron-down',12),'Customize Quick Access Toolbar');
 opener(cust,()=>openMenu([{header:'Customize Quick Access Toolbar'},...sh.qat.map(q=>({label:q.label,checked:q.shown,run:()=>{q.shown=!q.shown;q.el.style.display=q.shown?'':'none'}})),{sep:true},
  {label:'Show File Tabs',checked:CF.get('fileTabs'),run:()=>CF.toggle('fileTabs')},{label:'Minimize the Ribbon',checked:CF.get('ribbonMin'),run:()=>CF.toggle('ribbonMin')},{label:sh.ribbonClosed?'Show the Ribbon':'Close the Ribbon',run:()=>shellRibbon(!!sh.ribbonClosed)}],cust,{owner:cust}));
 qat.append(ce('div','cf-tsep'),ws,cust);
 const title=ce('div','cf-title');sh.titleEl=title;
 const right=ce('div','cf-tright'),sw=ce('div','cf-search'),input=ce('input');input.placeholder='Type a keyword or phrase';input.setAttribute('aria-label','Search commands');sw.innerHTML=ico('search',14);sw.append(input);sh.searchInput=input;
 const state={sel:0,list:[]},pop=ce('div','cf-menu cf-search-pop');
 const show=()=>{state.list=shellSearch(input.value,10);state.sel=0;if(!input.value.trim()){closeMenus();return}renderResults(state.list,pop,state);if(!sh.menus.some(m=>m.el===pop)){closeMenus();sh.menus.push({el:pop,owner:sw,stay:true})}place(pop,sw,'right')};
 input.oninput=show;input.onfocus=()=>{if(input.value.trim())show()};input.onkeydown=e=>searchKeys(e,input,state,pop,()=>{closeMenus();input.value='';input.blur()});
 const help=btn('cf-qbtn cf-help',ico('help',16),'Help');help.onclick=()=>shellExec({cmd:'HELP',fallback:()=>shellNotify('Type a command name or alias at the command line; F1 lists commands when the command line is available.')});tip(help,()=>shellTipData({cmd:'HELP',label:'Help'}));
 right.append(sw,help);tb.append(app,qat,title,right)}

// ---- File tabs and Start page ----------------------------------------------------------------------------------
function buildFileTabs(){const h=CF.hosts.filetabs;h.replaceChildren();
 const st=ce('div','cf-ftab cf-ftab-start');st.setAttribute('role','tab');st.append(ce('span','cf-lbl','Start'));st.onclick=()=>showStart(true);
 const dt=ce('div','cf-ftab cf-ftab-doc'),dl=ce('span','cf-lbl'),x=btn('cf-ftab-x',ico('close',12),'Close drawing');dt.setAttribute('role','tab');dt.insertAdjacentHTML?.('afterbegin',ico('model',14));dt.append(dl,x);
 dt.onclick=()=>showStart(false);x.onclick=e=>{e?.stopPropagation?.();shellCloseDrawing()};tip(x,()=>({title:'Close',desc:'Closes the drawing (starts a new one) and shows the Start tab.'}));
 const plus=btn('cf-ftab-new',ico('plus',14),'New drawing');plus.onclick=()=>shellExec({cmd:'NEW'});tip(plus,()=>({title:'New Drawing',command:'NEW',key:'Ctrl+N'}));
 sh.ftStart=st;sh.ftDoc=dt;sh.ftLbl=dl;h.append(st,dt,plus);updateFileTabs()}
function updateFileTabs(){if(!sh.ftDoc)return;sh.ftStart.classList.toggle('active',sh.start);sh.ftDoc.classList.toggle('active',!sh.start);sh.ftLbl.textContent=CF.fileName+(CF.modified?'*':'');sh.ftDoc.setAttribute('title',CF.fileName+'.cadforge.json')}
const GETTING=[['LINE','Draw connected straight segments'],['PLINE','Draw a single multi-segment polyline'],['CIRCLE','Circle by center and radius'],['RECTANG','Rectangle by two corners'],['MOVE','Move objects from a base point'],
 ['TRIM','Trim objects at cutting edges'],['OFFSET','Parallel copies at a distance'],['FILLET','Round or sharpen a corner'],['DIMLINEAR','Horizontal or vertical dimension'],['LAYER','Layer Properties Manager'],['EXTRUDE','Extrude a closed profile to a solid'],['PLOT','Plot to a PDF sheet']];
const SHORTCUTS=[['Esc','Cancel the current command'],['Enter / Space','Repeat the last command'],['F1','Command list'],['F2','Expand command history'],['F3','Object snap'],['F7','Grid display'],['F8','Ortho mode'],['F9','Snap mode'],
 ['F10','Polar tracking'],['F12','Dynamic input'],['Ctrl+Z / Ctrl+Y','Undo / Redo'],['Ctrl+1','Properties palette'],['Ctrl+9','Command line'],['Ctrl+0','Clean screen'],['Ctrl+N / O / S','New / Open / Save'],['Ctrl+P','Plot']];
function buildStart(){const st=ce('div');st.id='cf-start';sh.startEl=st;
 const side=ce('div','cf-st-side'),brand=ce('div','cf-st-brand');brand.innerHTML=ico('app',46)+'<div><b>CadForge</b><span>2D drafting &amp; 3D modeling</span></div>';
 const sbtn=(icon,label,sub,fn,primary)=>{const b=btn('cf-st-btn'+(primary?' primary':''));b.innerHTML=ico(icon,24)+`<div><b>${esc(label)}</b><span>${esc(sub)}</span></div>`;b.onclick=fn;return b};
 const empty=()=>!doc.entities.length&&!doc.solids?.length;
 const bNew=sbtn('new','New','Start a new drawing',()=>{if(empty()&&!CF.modified)showStart(false);else shellExec({cmd:'NEW'})},true);
 const bOpen=sbtn('open','Open...','Open a project or DXF file',()=>shellExec({cmd:'OPEN'}));
 sh.stCont=sbtn('model','Continue','',()=>showStart(false));
 const wsHead=ce('div','cf-st-h','Workspace'),wsBox=ce('div','cf-st-ws');sh.stWs=Object.entries(WS_LABEL).map(([k,l])=>{const b=btn('cf-st-wsbtn');b.innerHTML=ico(k==='3d'?'box':'workspace',16)+`<span>${esc(l)}</span>`;b.onclick=()=>{CF.setWorkspace(k);refreshStart()};b._ws=k;wsBox.append(b);return b});
 const foot=ce('div','cf-st-foot','Runs from a single file, offline. Type a command name at any time, or press F1 for the command list.');
 side.append(brand,ce('div','cf-st-h','Get Started'),bNew,bOpen,sh.stCont,wsHead,wsBox,foot);
 const main=ce('div','cf-st-main'),recentH=ce('div','cf-st-h','Current Drawing'),recent=ce('div','cf-st-recent'),card=btn('cf-st-card cf-st-doc');
 const cv=ce('canvas');cv.width=264;cv.height=160;sh.stThumb=cv;sh.stCap=ce('div','cf-st-cap');card.append(cv,sh.stCap);card.onclick=()=>showStart(false);recent.append(card);
 const tips=ce('div','cf-st-card cf-st-tips');tips.innerHTML='<b>Working like a desktop CAD application</b><ul><li>Type a command or its alias (L, C, TR, O) and press <kbd>Enter</kbd>.</li><li>Options in [brackets] at the command line can be clicked or typed by their capital letter.</li><li><kbd>Enter</kbd>, <kbd>Space</kbd> or right-click ends a command; <kbd>Enter</kbd> at <i>Command:</i> repeats the last one.</li><li>Drag left-to-right for a window selection, right-to-left for a crossing selection.</li><li>Switch to <i>3D Modeling</i> in the Quick Access Toolbar for extrusions and solids.</li></ul>';recent.append(tips);
 const gsH=ce('div','cf-st-h','Getting Started'),grid=ce('div','cf-st-grid');
 for(const [cmd,desc]of GETTING){const b=btn('cf-st-cmd'),al=(INFO[cmd]?.[0]||'').split(', ')[0];b.innerHTML=ico(cmd.toLowerCase(),24)+`<div><b>${cmd}${al?` <i>${esc(al)}</i>`:''}</b><span>${esc(desc)}</span></div>`;b.onclick=()=>{showStart(false);shellExec({cmd})};tip(b,()=>shellTipData({cmd,label:cmd}));grid.append(b)}
 const kH=ce('div','cf-st-h','Keyboard Shortcuts'),keys=ce('div','cf-st-card cf-st-keys');keys.innerHTML=SHORTCUTS.map(([k,d])=>`<div><span>${esc(d)}</span><span>${k.split(' / ').map(x=>`<kbd>${esc(x)}</kbd>`).join(' ')}</span></div>`).join('');
 main.append(recentH,recent,gsH,grid,kH,keys);st.append(side,main);(document.querySelector('main')||document.body).append(st)}
function drawThumb(cv){try{const c=cv.getContext('2d'),w=cv.width,h=cv.height;c.fillStyle=CF.colors.canvas;c.fillRect(0,0,w,h);const es=doc.entities.filter(e=>CF.visible(e)).slice(0,6000);
 if(!es.length){c.fillStyle='#6b7684';c.font='12px Segoe UI';c.textAlign='center';c.fillText(doc.solids?.length?`${doc.solids.length} mesh${doc.solids.length===1?'':'es'}`:'Empty drawing',w/2,h/2+4);return}
 let x0=Infinity,y0=Infinity,x1=-Infinity,y1=-Infinity;for(const e of es){const b=CF.geom.bbox(e);x0=Math.min(x0,b.minX);y0=Math.min(y0,b.minY);x1=Math.max(x1,b.maxX);y1=Math.max(y1,b.maxY)}
 const s=Math.min((w-24)/Math.max(1e-9,x1-x0),(h-24)/Math.max(1e-9,y1-y0)),cx=(x0+x1)/2,cy=(y0+y1)/2,P=p=>[w/2+(p.x-cx)*s,h/2-(p.y-cy)*s];c.lineWidth=1;
 for(const e of es){c.strokeStyle=c.fillStyle=doc.layers.find(l=>l.name===e.layer)?.color||'#ccc';c.beginPath();
  if(e.type==='circle'){const [x,y]=P(e.center);c.arc(x,y,Math.max(.5,e.radius*s),0,Math.PI*2)}else if(e.type==='text'){const [x,y]=P(e.points[0]);c.font=`${Math.max(2,e.height*s)}px Segoe UI`;c.fillText(e.text,x,y);continue}
  else{e.points.forEach((p,i)=>{const [x,y]=P(p);i?c.lineTo(x,y):c.moveTo(x,y)});if(e.closed)c.closePath()}c.stroke()}}catch(err){console.error(err)}}
function refreshStart(){if(!sh.startEl)return;const b=sh.stCont;b.querySelector?.('b');b.innerHTML=ico('model',24)+`<div><b>Continue</b><span>${esc(CF.fileName)}${CF.modified?'*':''}</span></div>`;
 for(const w of sh.stWs||[])w.classList.toggle('active',w._ws===CF.workspace);
 sh.stCap.innerHTML=`<b>${esc(CF.fileName)}${CF.modified?'*':''}</b><span>${doc.entities.length} object${doc.entities.length===1?'':'s'} &middot; ${doc.layers.length} layer${doc.layers.length===1?'':'s'}${doc.solids?.length?` &middot; ${doc.solids.length} mesh${doc.solids.length===1?'':'es'}`:''}</span>`;drawThumb(sh.stThumb)}
function showStart(v=true){sh.start=!!v;document.body.classList.toggle('cf-start',sh.start);if(sh.start){closeMenus();refreshStart()}updateFileTabs();if(!sh.start){try{render()}catch{}setTimeout(fitRibbon,0)}}

// ---- Document title -------------------------------------------------------------------------------------------
const shellTitleText=()=>`CadForge  ${CF.fileName}.cadforge.json${CF.modified?'*':''}`;
function updateTitle(){if(sh.titleEl)sh.titleEl.innerHTML=`<span>CadForge</span><b>${esc(CF.fileName)}.cadforge.json${CF.modified?'*':''}</b>`;try{document.title=`CadForge - ${CF.fileName}`}catch{}updateFileTabs();if(sh.start)refreshStart()}
function shellRibbon(show){sh.ribbonClosed=!show;document.body.classList.toggle('cf-ribbon-closed',!show);if(show){if(CF.get('ribbonMin'))CF.set('ribbonMin',false);setTimeout(fitRibbon,0)}}

// ---- Styles ---------------------------------------------------------------------------------------------------
const shellStyle=ce('style');shellStyle.id='cf-shell-style';shellStyle.textContent=`
#cf-titlebar{height:32px;display:flex;align-items:center;gap:2px;padding:0 6px 0 0;background:var(--cf-titlebar);border-bottom:1px solid #12161c;user-select:none;z-index:40}
.cf-icon{display:block;flex-shrink:0;color:#dfe5ec}
.cf-appbtn{width:46px;height:31px;display:flex;align-items:center;justify-content:center;padding:0;border:0;border-radius:0;background:transparent}
.cf-appbtn:hover,.cf-appbtn.open{background:#2b323c;border:0}.cf-appbtn.open{box-shadow:inset 0 -2px 0 var(--cf-accent)}
.cf-qat{display:flex;align-items:center;gap:1px;padding-left:2px;min-width:0}
.cf-qbtn{width:26px;height:24px;display:flex;align-items:center;justify-content:center;padding:0;background:transparent;border:1px solid transparent;border-radius:2px}
.cf-qcust{width:16px}.cf-qcust .cf-icon{color:var(--cf-text-dim)}
.cf-tsep{width:1px;height:16px;background:#3a424e;margin:0 6px}
.cf-wsbtn{display:flex;align-items:center;gap:6px;height:24px;padding:0 6px 0 5px;background:transparent;border:1px solid transparent;color:#c3cad4;white-space:nowrap}
.cf-wsbtn .cf-icon{color:#c3cad4}.cf-chev{color:var(--cf-text-dim)!important;opacity:.9}
.cf-title{flex:1;min-width:0;text-align:center;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;color:var(--cf-text-faint);pointer-events:none;padding:0 12px}
.cf-title b{font-weight:400;color:#d7dde5;margin-left:10px}
.cf-tright{display:flex;align-items:center;gap:4px}
.cf-search{position:relative;display:flex;align-items:center}.cf-search>.cf-icon{position:absolute;left:7px;color:var(--cf-text-faint);pointer-events:none}
.cf-search input{width:230px;height:22px;padding:2px 8px 2px 26px;background:#262c35;border:1px solid #3a424e;border-radius:2px;font-size:12px}
.cf-search input::placeholder,.cf-am-search::placeholder{color:var(--cf-text-faint)}.cf-search input:focus{outline:0;border-color:var(--cf-accent);background:#1f252d}
#cf-ribbon-tabs{height:27px;display:flex;align-items:flex-end;padding:0 4px 0 6px;background:var(--cf-tabbar);user-select:none}
.cf-rtab{height:26px;padding:0 14px;border:0;border-radius:0;background:transparent;color:#c3cad4;font-size:12px}
.cf-rtab:hover{background:#2b323c;color:#fff;border:0}.cf-rtab.active{background:var(--cf-ribbon);color:#fff;box-shadow:inset 0 2px 0 var(--cf-accent)}
body.cf-ribbon-min .cf-rtab.active{background:transparent;box-shadow:none}body.cf-ribbon-min .cf-rtab.active:hover{background:#2b323c}
.cf-rtab-fill{flex:1}.cf-rmin{width:24px;height:22px;margin:0 0 2px;padding:0;display:flex;align-items:center;justify-content:center;background:transparent;border:1px solid transparent}
#cf-ribbon{display:flex;align-items:stretch;height:96px;background:var(--cf-ribbon);border-bottom:1px solid #12161c;overflow:hidden;user-select:none}
body.cf-ribbon-min #cf-ribbon.cf-pop{display:flex;position:fixed;left:0;right:0;z-index:60;box-shadow:0 10px 24px rgba(0,0,0,.5)}
body.cf-ribbon-closed #cf-ribbon-tabs,body.cf-ribbon-closed #cf-ribbon{display:none!important}
.cf-panel{display:flex;flex-direction:column;flex-shrink:0;border-right:1px solid #4a5260}
.cf-panel-body{flex:1;display:flex;align-items:flex-start;gap:2px;padding:3px 5px 0}
.cf-panel-title{height:18px;display:flex;align-items:center;justify-content:center;gap:4px;padding:0 8px;background:var(--cf-ribbon-title);color:var(--cf-text-dim);font-size:11px;white-space:nowrap;border-top:1px solid #303844}
.cf-panel-title.cf-has-more{cursor:pointer}.cf-panel-title.cf-has-more:hover{background:var(--cf-hover);color:#fff}
.cf-panel-cbtn{display:none!important}.cf-collapsed>.cf-panel-cbtn{display:flex!important;margin:3px 4px;height:86px!important}
.cf-collapsed>.cf-panel-body,.cf-collapsed>.cf-panel-title{display:none}
.cf-rb{display:flex;align-items:center;padding:0;background:transparent;border:1px solid transparent;border-radius:2px;color:var(--cf-text);white-space:nowrap}
.cf-rb-large{flex-direction:column;justify-content:flex-start;min-width:46px;height:72px;padding:4px 4px 0}
.cf-rb-large .cf-lbl,.cf-split-large .cf-lbl{margin-top:3px;min-height:26px;font-size:12px;line-height:13px;text-align:center}
.cf-rb-small{height:22px;gap:5px;padding:0 7px 0 3px}.cf-rb-icon{width:24px;height:22px;justify-content:center}
.cf-compact .cf-rb-small .cf-lbl,.cf-compact .cf-split-small .cf-lbl{display:none}.cf-compact .cf-rb-small{padding:0 3px}
.cf-compact>.cf-panel-body{padding:3px 3px 0}.cf-compact .cf-rb-large,.cf-compact .cf-split-large{min-width:42px}.cf-compact .cf-rb-large{padding:4px 2px 0}
.cf-col{display:flex;flex-direction:column;gap:1px}.cf-row{display:flex;gap:1px;height:22px}
.cf-split{display:flex;border:1px solid transparent;border-radius:2px}.cf-split .cf-rb{border:0;border-radius:0}
.cf-split-large{flex-direction:column;height:72px;min-width:46px}.cf-split-large .cf-split-main{justify-content:center;height:40px;padding:4px 6px 0}
.cf-split-large .cf-split-arrow{flex:1;flex-direction:column;justify-content:flex-start;padding:0 4px}.cf-split-large .cf-split-arrow .cf-lbl{margin-top:0}
.cf-split-large .cf-chev{display:inline-block;vertical-align:middle}
.cf-split-small{height:22px}.cf-split-small .cf-split-main{gap:5px;padding:0 4px 0 3px}.cf-split-small .cf-split-arrow{width:14px;justify-content:center}
.cf-split:hover{border-color:var(--cf-hover-border)}.cf-split-large:hover .cf-split-main{box-shadow:inset 0 -1px 0 var(--cf-hover-border)}.cf-split-small:hover .cf-split-main{box-shadow:inset -1px 0 0 var(--cf-hover-border)}
.cf-split .cf-rb:hover{background:var(--cf-hover)}
.cf-rb.active,.cf-split.active{background:var(--cf-active);border-color:var(--cf-active-border)}
.cf-rb:active:not(:disabled),.cf-qbtn:active{background:#264d73}
.cf-widget{display:flex;align-items:center;min-height:24px}.cf-widget-layer{width:196px}.cf-widget-layer>*{flex:1;min-width:0}.cf-compact .cf-widget-layer{width:132px}
.cf-widget select{width:100%;height:22px;padding:1px 4px;font-size:12px}
.cf-pop-panel{position:fixed;z-index:900;display:flex;flex-direction:column;background:var(--cf-ribbon);border:1px solid #5d6876;box-shadow:var(--cf-shadow)}
.cf-pop-panel>.cf-panel-body{min-height:78px;padding:4px 6px}.cf-more{padding:4px}.cf-more-col{display:flex;flex-direction:column;gap:1px;min-width:170px}
.cf-menu{position:fixed;z-index:1000;min-width:180px;padding:3px;background:#2b323c;border:1px solid #556070;box-shadow:var(--cf-shadow);color:var(--cf-text)}
.cf-mi{display:flex;align-items:center;gap:8px;width:100%;height:26px;padding:0 20px 0 4px;background:transparent;border:1px solid transparent;border-radius:2px;text-align:left;white-space:nowrap}
.cf-mi.big{height:36px}.cf-mi:hover{background:var(--cf-active);border-color:var(--cf-active-border)}.cf-mi:disabled{opacity:.45}
.cf-mi-ico{width:24px;display:flex;align-items:center;justify-content:center;flex-shrink:0;height:100%}.cf-mi.big .cf-mi-ico{width:30px}
.cf-mi.checked .cf-mi-ico{background:rgba(74,144,217,.22);box-shadow:inset 0 0 0 1px var(--cf-active-border);border-radius:2px}
.cf-mi-key{margin-left:auto;padding-left:24px;color:var(--cf-text-faint)}
.cf-msep{height:1px;background:#4a5260;margin:3px 2px}.cf-mhead{padding:4px 8px 5px;color:var(--cf-text-dim);font-size:11px;border-bottom:1px solid #3a424e;margin-bottom:3px}
.cf-appmenu{position:fixed;left:0;top:32px;z-index:950;width:620px;max-width:100vw;display:flex;flex-direction:column;background:#2b323c;border:1px solid #556070;border-top:0;box-shadow:var(--cf-shadow)}
.cf-am-head{position:relative;display:flex;align-items:center;padding:8px 10px;background:#232932;border-bottom:1px solid #3a424e}
.cf-am-glass{position:absolute;left:18px;pointer-events:none}.cf-am-glass .cf-icon{color:var(--cf-text-faint)}
.cf-am-search{flex:1;height:28px;padding:2px 10px 2px 30px;background:#1f252d;border:1px solid #3a424e;font-size:12px}.cf-am-search:focus{outline:0;border-color:var(--cf-accent)}
.cf-am-body{display:flex;min-height:420px}
.cf-am-left{width:214px;flex-shrink:0;padding:4px 0;background:#262c35;border-right:1px solid #3a424e}
.cf-am-item{display:flex;align-items:center;gap:12px;width:100%;height:46px;padding:0 10px 0 14px;background:transparent;border:0;border-radius:0;font-size:13px;text-align:left}
.cf-am-item .cf-chev{margin-left:auto}.cf-am-item:hover,.cf-am-item.sel{background:#343c48;border:0}.cf-am-item.sel{box-shadow:inset 3px 0 0 var(--cf-accent)}
.cf-am-right{flex:1;min-width:0;padding:10px 12px;overflow:auto}
.cf-am-h{padding:2px 4px 7px;margin-bottom:6px;border-bottom:1px solid #3a424e;color:var(--cf-text-dim);font-size:11px;text-transform:uppercase;letter-spacing:.6px}.cf-am-h2{margin-top:14px}
.cf-am-empty{padding:6px 4px;color:var(--cf-text-faint)}
.cf-am-entry{display:flex;align-items:flex-start;gap:12px;width:100%;padding:7px 8px;background:transparent;border:1px solid transparent;text-align:left;white-space:normal}
.cf-am-entry b{display:block;font-weight:600;color:#fff}.cf-am-entry span{display:block;color:var(--cf-text-dim);margin-top:1px}
.cf-am-results{min-height:420px;padding:6px;overflow:auto}
.cf-am-foot{display:flex;justify-content:flex-end;gap:6px;padding:7px 10px;background:#232932;border-top:1px solid #3a424e}
.cf-am-fbtn{display:flex;align-items:center;gap:6px;height:24px;padding:0 10px;background:#343c48}
.cf-sr{display:flex;align-items:center;gap:9px;width:100%;height:30px;padding:0 8px;background:transparent;border:1px solid transparent;text-align:left;white-space:nowrap}
.cf-sr b{min-width:104px;font-weight:600;font-family:var(--cf-mono)}.cf-sr i{min-width:52px;color:var(--cf-text-faint);font-style:normal;font-family:var(--cf-mono);font-size:11px}
.cf-sr span{color:var(--cf-text-dim);overflow:hidden;text-overflow:ellipsis}.cf-sr:hover,.cf-sr.sel{background:var(--cf-active);border-color:var(--cf-active-border)}.cf-sr-none{padding:8px;color:var(--cf-text-faint)}
.cf-search-pop{width:420px;max-height:340px;overflow:auto}
.cf-tip{position:fixed;z-index:2000;display:none;max-width:320px;padding:8px 10px;background:#3a424e;border:1px solid #66717f;box-shadow:0 6px 18px rgba(0,0,0,.45);color:var(--cf-text);pointer-events:none}
.cf-tip b{display:flex;align-items:center;gap:10px;justify-content:space-between;font-weight:600}.cf-tip p{margin:5px 0 0;color:#c9d0d9;line-height:1.4}
.cf-tip-cmd{display:flex;align-items:center;gap:6px;margin-top:7px;padding-top:6px;border-top:1px solid #56606d;color:#c9d0d9;font-family:var(--cf-mono);font-size:11px}
.cf-tip kbd,#cf-start kbd{font:11px var(--cf-mono);font-weight:400;padding:0 5px;background:#1f252d;border:1px solid #4a5260;border-bottom-width:2px;border-radius:2px;color:#e3e8ee}
#cf-filetabs{height:29px;display:flex;align-items:flex-end;gap:1px;padding:0 4px;background:var(--cf-frame);user-select:none}
.cf-ftab{display:flex;align-items:center;gap:7px;height:25px;max-width:260px;padding:0 5px 0 10px;background:#2b323c;color:#c3cad4;border-top:2px solid transparent;cursor:default}
.cf-ftab .cf-lbl{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.cf-ftab .cf-icon{color:#9fb0c4}
.cf-ftab:hover{background:#343c48;color:#fff}.cf-ftab.active{background:var(--cf-canvas);color:#fff;border-top-color:var(--cf-accent)}
.cf-ftab-start{padding:0 16px}
.cf-ftab-x{width:18px;height:18px;padding:0;display:flex;align-items:center;justify-content:center;background:transparent;border:0;opacity:0}
.cf-ftab:hover .cf-ftab-x,.cf-ftab.active .cf-ftab-x{opacity:1}.cf-ftab-x:hover{background:#b8433f;border:0}
.cf-ftab-new{width:28px;height:25px;padding:0;display:flex;align-items:center;justify-content:center;background:transparent;border:0;border-radius:0}.cf-ftab-new:hover{background:#2b323c;border:0}
main{position:relative}
#cf-start{position:absolute;inset:0;z-index:30;display:none;background:#1d2229;overflow:auto}
body.cf-start #cf-start{display:flex}
body.cf-start #cf-ribbon-tabs,body.cf-start #cf-ribbon,body.cf-start #cf-cmdline,body.cf-start #cf-statusbar{display:none!important}
.cf-st-side{width:300px;flex-shrink:0;display:flex;flex-direction:column;gap:8px;padding:26px 22px;background:#232932;border-right:1px solid #12161c}
.cf-st-brand{display:flex;align-items:center;gap:12px;margin-bottom:18px}.cf-st-brand b{display:block;font-size:22px;font-weight:600;letter-spacing:.2px}.cf-st-brand span{color:var(--cf-text-dim)}
.cf-st-h{margin:14px 0 4px;color:var(--cf-text-dim);font-size:11px;text-transform:uppercase;letter-spacing:.8px}
.cf-st-btn{display:flex;align-items:center;gap:12px;padding:8px 12px;background:#2b323c;border:1px solid #3a424e;text-align:left}
.cf-st-btn b{display:block;font-size:13px;font-weight:600}.cf-st-btn span{display:block;color:var(--cf-text-dim)}
.cf-st-btn.primary{background:var(--cf-active);border-color:var(--cf-active-border)}.cf-st-btn.primary span{color:#d5e4f3}
.cf-st-ws{display:flex;flex-direction:column;gap:2px}.cf-st-wsbtn{display:flex;align-items:center;gap:8px;height:28px;padding:0 10px;background:transparent;border:1px solid transparent;text-align:left}
.cf-st-wsbtn.active{background:#2b323c;border-color:var(--cf-active-border)}
.cf-st-foot{margin-top:auto;padding-top:14px;color:var(--cf-text-faint);line-height:1.5}
.cf-st-main{flex:1;min-width:0;display:flex;flex-direction:column;gap:8px;padding:26px 34px 30px}.cf-st-main>.cf-st-h{margin-top:12px}.cf-st-main>.cf-st-h:first-child{margin-top:0}
.cf-st-recent{display:flex;gap:14px;align-items:stretch;flex-wrap:wrap}
.cf-st-card{background:#2b323c;border:1px solid #3a424e;border-radius:2px}
.cf-st-doc{display:flex;flex-direction:column;width:268px;padding:1px;text-align:left}.cf-st-doc canvas{display:block;width:264px;height:160px}
.cf-st-cap{padding:7px 9px}.cf-st-cap b{display:block;font-weight:600}.cf-st-cap span{color:var(--cf-text-dim)}
.cf-st-tips{flex:1;min-width:min(300px,100%);max-width:620px;padding:12px 16px;line-height:1.5}.cf-st-tips ul{margin:6px 0 0;padding-left:18px;color:#c9d0d9}.cf-st-tips li{margin:3px 0}
.cf-st-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:6px}
.cf-st-cmd{display:flex;align-items:flex-start;gap:10px;padding:9px 10px;background:#2b323c;border:1px solid #3a424e;text-align:left;white-space:normal}
.cf-st-cmd b{display:block;font-family:var(--cf-mono);font-weight:600}.cf-st-cmd i{color:var(--cf-text-faint);font-style:normal;font-weight:400}.cf-st-cmd span{display:block;color:var(--cf-text-dim)}
.cf-st-keys{display:grid;grid-template-columns:repeat(auto-fill,minmax(250px,1fr));gap:0 26px;padding:8px 16px}
.cf-st-keys>div{display:flex;justify-content:space-between;align-items:center;gap:10px;padding:5px 0;border-bottom:1px solid #343c48}
@media (max-width:1200px){.cf-search input{width:170px}.cf-title span{display:none}}
`;
document.head.append(shellStyle);

// ---- Commands owned by the frame (only when no other module registered them) --------------------------------
function shellWs(value){const v=String(value??'').trim();if(!v)return false;const ws=/^3d|model/i.test(v)?'3d':/^(2d|draft|annot)/i.test(v)?'drafting':null;if(!ws){shellNotify(`Workspace "${v}" not found.`);return false}CF.setWorkspace(ws);return true}
if(!CF.resolve('RIBBON'))CF.register({name:'RIBBON',label:'Ribbon',category:'Interface',desc:INFO.RIBBON[1],icon:'ribbon',noRepeat:true,run:()=>shellRibbon(true)});
if(!CF.resolve('RIBBONCLOSE'))CF.register({name:'RIBBONCLOSE',label:'Close Ribbon',category:'Interface',desc:INFO.RIBBONCLOSE[1],icon:'ribbonclose',noRepeat:true,run:()=>shellRibbon(false)});
if(!CF.resolve('WSCURRENT'))CF.register({name:'WSCURRENT',label:'Workspace',category:'Interface',desc:INFO.WSCURRENT[1],icon:'workspace',noRepeat:true,
 async run(args){const v=args??await cadPrompt('Enter new value for WSCURRENT',WS_LABEL[CF.workspace]);if(v!=null)shellWs(v)}});

// ---- Public API (testable without a real DOM) ----------------------------------------------------------------
CF.shell={state:sh,ribbonModel:shellRibbonModel,commandNames:shellCommandNames,items:shellItems,knownCommands:Object.keys(INFO),info:INFO,
 buildRibbon,buildPanels,selectTab,fitRibbon,exec:shellExec,search:shellSearch,openAppMenu,closeMenus,showStart,startVisible:()=>sh.start,
 titleText:shellTitleText,updateTitle,setWorkspace:shellWs,ribbon:shellRibbon,
 get buttons(){return sh.buttons},get tabButtons(){return sh.tabButtons},get tabs(){return shellTabs()},get activeTab(){return activeTab()}};

// ---- Build and wire ---------------------------------------------------------------------------------------------
try{
 document.body.append(tipEl);buildTitlebar();buildRibbon();buildFileTabs();buildStart();
 CF.on('workspace',ws=>{buildRibbon();if(sh.start)refreshStart();try{localStorage.setItem('cadforge.workspace',ws)}catch{}});
 CF.on('tool',()=>shellRefresh());
 CF.on('state',({name,value})=>{if(name==='ribbonMin'&&!value)closeMenus();shellRefresh();if(name==='ribbonMin'||name==='clean'||name==='fileTabs')setTimeout(fitRibbon,0)});
 CF.on('command',({def})=>{shellRecent(def?.name);if(sh.start&&!['OPEN','DXFIN'].includes(def?.name))showStart(false);shellRefresh()});
 CF.on('modified',updateTitle);CF.on('document',()=>{updateTitle();if(sh.start)showStart(false)});
 // Engine code changes tool/picking/modified state without events: re-check a cheap signature after each render.
 const shellPrevRender=render;render=function(...a){const r=shellPrevRender(...a);const sig=`${tool}|${CF.picking?.command||''}|${CF.modified}|${CF.fileName}`;if(sig!==sh.sig){sh.sig=sig;shellRefresh();updateTitle()}return r};
 document.addEventListener('pointerdown',e=>{sh.suppress=null;if(!sh.menus.length)return;const t=e.target;if(sh.menus.some(m=>m.el.contains(t)||(m.stay&&m.owner?.contains(t))))return;const own=sh.menus.find(m=>m.owner?.contains(t));closeMenus();if(own)sh.suppress=own.owner},true);
 window.addEventListener('keydown',e=>{if(e.key==='Escape'&&sh.menus.length){closeMenus();e.preventDefault();e.stopPropagation()}},true);
 window.addEventListener('resize',()=>{closeMenus();hideTip();fitRibbon()});
 window.addEventListener('blur',()=>{hideTip()});
 let saved=null;try{saved=localStorage.getItem('cadforge.workspace')}catch{}if(saved==='3d'||saved==='drafting'){if(saved!==CF.workspace)CF.setWorkspace(saved)}
 updateTitle();shellRefresh();
 if(!doc.entities.length&&!doc.solids?.length)showStart(true);
 setTimeout(fitRibbon,0);setTimeout(fitRibbon,400);
}catch(err){console.error('CadForge shell failed to build',err)}
CF.module('shell');
}
