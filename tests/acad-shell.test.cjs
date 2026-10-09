// A1: icon library coverage, ribbon model per workspace, button -> CF.run mapping, workspace switch, title/modified marker, Start page.
module.exports=async({run,assert})=>{
 if(!await run("return typeof CF!=='undefined'&&CF.has('icons')&&CF.has('shell')")){console.log('SKIP: acad-shell');return}
 // ---- icons -------------------------------------------------------------------------------------------------
 await run(`
 const keys='app menu new open save saveas plot undo redo search close chevron-down chevron-right check layer layer-on layer-off layer-freeze layer-properties color properties settings gear workspace grid snap ortho polar osnap dyn clean pan zoom zoom-extents zoom-window zoom-in zoom-out orbit viewcube home model layout plus minus export import block insert explode match measure dist id list fillet chamfer polygon ellipse arc circle rectangle hatch text dimlinear dimaligned offset trim extend mirror rotate scale move copy erase join extrude box union subtract intersect update mesh-move mesh-copy mesh-clear visual-style ucs line polyline mtext purge selectall copyclip cutclip pasteclip array help regen plan view'.split(' ');
 const missing=keys.filter(k=>!CF.hasIcon(k));assert.deepEqual(missing,[]);
 const commands='LINE PLINE CIRCLE ARC RECTANG TEXT HATCH DIMALIGNED POLYGON ELLIPSE DIMLINEAR DIST ID LIST MATCHPROP ARRAYRECT PURGE MOVE COPY ROTATE SCALE ERASE EXPLODE JOIN MIRROR OFFSET TRIM EXTEND FILLET CHAMFER BLOCK INSERT MEASUREGEOM SELECTALL UNDO REDO COPYCLIP CUTCLIP PASTECLIP HELP ZOOM PAN REGEN 3DORBIT PLAN VIEW VSCURRENT MODEL LAYOUT GRID SNAP ORTHO OSNAP POLAR DYNMODE CLEANSCREENON COMMANDLINE COMMANDLINEHIDE TEXTSCR PROPERTIES PROPERTIESCLOSE LAYER RIBBON RIBBONCLOSE WSCURRENT NEW OPEN QSAVE SAVEAS PLOT PAGESETUP EXPORT DXFOUT DXFIN SVGOUT STLOUT OBJEXPORT EXTRUDE BOX UNION SUBTRACT INTERSECT UPDATEEXTRUSION 3DMOVE MESHCOPY MESHCLEAR MESHFIT'.split(' ');
 assert.deepEqual(commands.filter(c=>!CF.hasIcon(c)),[]);
 const svg=CF.icon('line',16);assert.ok(svg.startsWith('<svg')&&svg.includes('width="16"')&&svg.includes('viewBox="0 0 24 24"')&&svg.includes('cf-icon'));
 assert.ok(CF.icon('does-not-exist',20).includes('<svg'));assert.ok(!CF.hasIcon('does-not-exist'));
 assert.ok(Array.isArray(CF.iconNames)&&CF.iconNames.length>=keys.length);assert.ok(CF.iconNames.includes('line')&&CF.iconNames.includes('mesh-clear'));
 assert.ok(!/autocad|autodesk/i.test(CF.icon('app',24)));
 // every icon is well formed: balanced tags and no NaN
 for(const n of CF.iconNames){const s=CF.icon(n,24);assert.ok(!s.includes('NaN')&&!s.includes('undefined'),n);assert.equal((s.match(/<svg/g)||[]).length,1,n);assert.ok(s.endsWith('</svg>'),n)}
 `);
 console.log('PASS: icon library covers every spec key and command, falls back for unknown names, exports iconNames');
 // ---- ribbon model ------------------------------------------------------------------------------------------
 await run(`
 const S=CF.shell;
 for(const ws of ['drafting','3d']){const model=S.ribbonModel(ws),names=model.map(t=>t.name);
  assert.deepEqual(names,ws==='3d'?['Home','Solid','Mesh','Insert','Annotate','View','Manage','Output']:['Home','Insert','Annotate','View','Manage','Output']);
  for(const t of model){assert.ok(t.panels.length>0,t.name);const titles=t.panels.map(p=>p.title);assert.equal(new Set(titles).size,titles.length,t.name+' panel titles unique')}
  for(const it of S.items(model)){assert.ok(it.label,'label');assert.ok(it.icon,'icon of '+it.label);assert.ok(CF.hasIcon(it.icon),'icon '+it.icon);
   assert.ok(it.cmd||it.run||it.flag,'action of '+it.label);if(it.cmd)assert.ok(S.knownCommands.includes(it.cmd),'known command '+it.cmd)}}
 const home=S.ribbonModel('drafting')[0],panels=home.panels.map(p=>p.title);
 assert.deepEqual(panels,['Draw','Modify','Annotation','Layers','Block','Properties','Utilities','Clipboard','View']);
 const cmds=p=>S.items([{panels:[home.panels.find(x=>x.title===p)]}]).map(i=>i.cmd).filter(Boolean);
 for(const c of ['LINE','PLINE','CIRCLE','ARC','RECTANG','HATCH','TEXT'])assert.ok(cmds('Draw').includes(c),'Draw has '+c);
 for(const c of ['MOVE','ROTATE','TRIM','EXTEND','COPY','MIRROR','FILLET','CHAMFER','SCALE','ARRAYRECT','OFFSET','ERASE','EXPLODE','JOIN'])assert.ok(cmds('Modify').includes(c),'Modify has '+c);
 for(const c of ['MTEXT','TEXT','DIMLINEAR','DIMALIGNED'])assert.ok(cmds('Annotation').includes(c),'Annotation has '+c);
 assert.ok(cmds('Layers').includes('LAYER'));assert.ok(home.panels.find(p=>p.title==='Layers').items.some(i=>i.kind==='col'&&i.items.some(x=>x.kind==='widget')));
 assert.ok(cmds('Block').includes('INSERT')&&cmds('Block').includes('BLOCK'));assert.ok(cmds('Properties').includes('MATCHPROP'));
 assert.ok(cmds('Utilities').includes('SELECTALL')&&cmds('Utilities').includes('ID'));assert.ok(['PASTECLIP','COPYCLIP','CUTCLIP'].every(c=>cmds('Clipboard').includes(c)));
 assert.ok(cmds('View').includes('ZOOM')&&cmds('View').includes('PAN'));
 // split buttons
 const modify=home.panels.find(p=>p.title==='Modify'),splits=S.items([{panels:[modify]}]);assert.ok(modify.items.some(c=>c.items&&c.items.some(x=>x.kind==='split'&&x.id==='trim'&&x.items.map(i=>i.cmd).join()==='TRIM,EXTEND')));
 assert.ok(modify.items.some(c=>c.items&&c.items.some(x=>x.kind==='split'&&x.id==='fillet'&&x.items.map(i=>i.cmd).join()==='FILLET,CHAMFER')));
 const d3=S.ribbonModel('3d');const d3cmds=S.items(d3).map(i=>i.cmd).filter(Boolean);
 for(const c of ['BOX','EXTRUDE','UPDATEEXTRUSION','JOIN','UNION','SUBTRACT','INTERSECT','3DMOVE','MESHCOPY','MESHCLEAR','STLOUT','OBJEXPORT'])assert.ok(d3cmds.includes(c),'3D has '+c);
 const outCmds=S.items(S.ribbonModel('drafting').filter(t=>t.name==='Output')).map(i=>i.cmd);for(const c of ['PLOT','PAGESETUP','DXFOUT','SVGOUT','OBJEXPORT','STLOUT','EXPORT'])assert.ok(outCmds.includes(c),'Output has '+c);
 const insCmds=S.items(S.ribbonModel('drafting').filter(t=>t.name==='Insert')).map(i=>i.cmd);for(const c of ['INSERT','BLOCK','OPEN','DXFIN'])assert.ok(insCmds.includes(c),'Insert has '+c);
 const viewItems=S.items(S.ribbonModel('drafting').filter(t=>t.name==='View'));for(const f of ['ucsIcon','viewCube','navBar','fileTabs','layoutTabs','commandLine','clean'])assert.ok(viewItems.some(i=>i.flag===f||i.cmd&&CF.get&&false),'View toggles '+f);
 assert.equal(viewItems.filter(i=>i.style).length>=4,true);
 `);
 console.log('PASS: ribbon model for both workspaces (tabs, panels, split buttons, icons, known commands)');
 // ---- DOM build, button -> CF.run mapping, active highlight ------------------------------------------------
 await run(`
 const S=CF.shell,calls=[],stubbed=[];
 CF.workspace='drafting';
 for(const ws of ['drafting','3d'])for(const n of S.commandNames(ws))if(!CF.resolve(n)){CF.register({name:n,run:()=>{}});stubbed.push(n)}
 CF.on('command',e=>calls.push(e));
 const seen=new Set();
 for(const ws of ['drafting','3d']){CF.workspace=ws;S.buildRibbon();assert.deepEqual(S.tabs,S.ribbonModel(ws).map(t=>t.name));assert.equal(S.tabButtons.length,S.tabs.length);
  for(const name of S.tabs){assert.ok(S.selectTab(name));assert.equal(S.activeTab,name);assert.ok(S.buttons.length>0,ws+'/'+name+' has buttons');
   const last=S.tabButtons.find(b=>b.dataset.tab===name);assert.ok(last.classList.contains('active'));
   for(const b of S.buttons){const items=b.items?b.items:[b.item];for(const it of items){if(!it.cmd||it.flag||it.run||seen.has(it.cmd+'|'+it.opt))continue;seen.add(it.cmd+'|'+it.opt);
     calls.length=0;S.exec(it);assert.equal(calls.length,1,'exec '+it.cmd);assert.equal(calls[0].def.name,it.cmd);assert.equal(calls[0].source,'ribbon')}}}}
 assert.ok(seen.size>60,'ran '+seen.size+' distinct ribbon commands');
 // clicking the real button element routes through CF.run too
 CF.workspace='drafting';S.buildRibbon();S.selectTab('Home');calls.length=0;
 const line=S.buttons.find(b=>b.item&&b.item.cmd==='LINE');assert.ok(line);line.el.onclick();assert.equal(calls.length,1);assert.equal(calls[0].def.name,'LINE');
 // ZOOM macros carry the option keyword as args
 let zoomArgs;CF.register({name:'ZOOM',aliases:['Z'],run:a=>{zoomArgs=a}});const z=S.buttons.find(b=>b.item&&b.item.cmd==='ZOOM'&&b.item.opt==='E');assert.ok(z);z.el.onclick();assert.equal(zoomArgs,'E');
 // active tool highlighting (LINE command <-> engine 'line' tool)
 setTool('line');S.state.sig='';render();assert.ok(line.el.classList.contains('active'));
 const circle=S.buttons.find(b=>b.item&&b.item.cmd==='CIRCLE');assert.ok(!circle.el.classList.contains('active'));
 setTool('circle');assert.ok(circle.el.classList.contains('active'));assert.ok(!line.el.classList.contains('active'));setTool('select');assert.ok(!circle.el.classList.contains('active'));
 // interface toggle buttons reflect and change flags
 S.selectTab('View');const ucs=S.buttons.find(b=>b.item&&b.item.flag==='ucsIcon');assert.ok(ucs);assert.ok(ucs.el.classList.contains('active'));ucs.el.onclick();assert.equal(CF.get('ucsIcon'),false);assert.ok(!ucs.el.classList.contains('active'));ucs.el.onclick();assert.equal(CF.get('ucsIcon'),true);
 // visual style / view preset buttons call the CF hooks
 const styles=[],presets=[];const oldS=CF.setVisualStyle,oldP=CF.setViewPreset;CF.setVisualStyle=n=>styles.push(n);CF.setViewPreset=n=>presets.push(n);
 for(const b of S.buttons){const items=b.items?b.items:[b.item];for(const it of items){if(it.style||it.preset)S.exec(it)}}
 assert.deepEqual(styles,['2dwireframe','wireframe','shaded','shadededges']);assert.deepEqual(presets,['top','bottom','left','right','front','back','sw','se','ne','nw']);CF.setVisualStyle=oldS;CF.setViewPreset=oldP;
 // double-click on the active tab toggles ribbonMin
 S.selectTab('Home');const tb=S.tabButtons.find(b=>b.dataset.tab==='Home');assert.equal(CF.get('ribbonMin'),false);tb.ondblclick();assert.equal(CF.get('ribbonMin'),true);tb.ondblclick();assert.equal(CF.get('ribbonMin'),false);
 for(const n of stubbed){CF.commands.delete(n)}CF.commands.delete('ZOOM');CF.aliases.delete('Z');
 `);
 console.log('PASS: ribbon builds per tab, every button runs CF.run with its command name (source ribbon), options as args, active-tool and toggle highlighting, double-click minimise');
 // ---- workspace switch ------------------------------------------------------------------------------------
 await run(`
 const S=CF.shell;CF.setWorkspace('3d');assert.equal(CF.workspace,'3d');assert.ok(S.tabs.includes('Solid')&&S.tabs.includes('Mesh'));
 assert.ok(S.ribbonModel('3d')[0].panels.some(p=>p.title==='Modeling'));
 CF.setWorkspace('drafting');assert.ok(!S.tabs.includes('Solid'));assert.equal(S.tabs[0],'Home');
 assert.equal(S.setWorkspace('3D Modeling'),true);assert.equal(CF.workspace,'3d');assert.equal(S.setWorkspace('Drafting & Annotation'),true);assert.equal(CF.workspace,'drafting');assert.equal(S.setWorkspace('nonsense'),false);
 // WSCURRENT registered by the shell when absent
 assert.ok(CF.resolve('WSCURRENT')&&CF.resolve('RIBBON')&&CF.resolve('RIBBONCLOSE'));
 CF.run('WSCURRENT',{args:'3D'});assert.equal(CF.workspace,'3d');CF.run('WSCURRENT',{args:'2D'});assert.equal(CF.workspace,'drafting');
 // a workspace switch always lands on the Home tab, even when the target workspace remembered another tab
 S.selectTab('Output');assert.equal(S.activeTab,'Output');CF.setWorkspace('3d');assert.equal(S.activeTab,'Home');
 S.selectTab('Solid');assert.equal(S.activeTab,'Solid');CF.setWorkspace('drafting');assert.equal(S.activeTab,'Home');
 S.selectTab('Output');CF.setWorkspace('3d');CF.setWorkspace('drafting');assert.equal(S.activeTab,'Home');assert.ok(S.tabButtons.find(b=>b.dataset.tab==='Home').classList.contains('active'));
 CF.setWorkspace('3d');S.selectTab('Output');CF.setWorkspace('drafting');CF.setWorkspace('3d');assert.equal(S.activeTab,'Home');CF.setWorkspace('drafting');
 `);
 console.log('PASS: workspace switch rebuilds the ribbon tabs (Drafting 6 tabs, 3D Modeling 8), WSCURRENT/RIBBON/RIBBONCLOSE registered');
 // ---- title / modified marker -----------------------------------------------------------------------------
 await run(`
 const S=CF.shell;CF.setFileName('Bracket.cadforge.json');assert.equal(CF.fileName,'Bracket');CF.markSaved();
 assert.equal(S.titleText(),'CadForge  Bracket.cadforge.json');assert.equal(document.title,'CadForge - Bracket');
 mutate(()=>doc.entities.push({type:'line',layer:'0',points:[{x:0,y:0},{x:1,y:1}]}));assert.equal(CF.modified,true);assert.equal(S.titleText(),'CadForge  Bracket.cadforge.json*');
 assert.ok(S.state.ftLbl.textContent==='Bracket*');
 CF.markSaved();assert.equal(S.titleText(),'CadForge  Bracket.cadforge.json');assert.equal(S.state.ftLbl.textContent,'Bracket');
 CF.setFileName('Drawing1');assert.equal(document.title,'CadForge - Drawing1');
 `);
 console.log('PASS: document title and file-tab label follow file name and modified marker');
 // ---- Start page --------------------------------------------------------------------------------------------
 await run(`
 const S=CF.shell;S.showStart(false);assert.equal(S.startVisible(),false);assert.ok(!document.body.classList.contains('cf-start'));
 S.showStart(true);assert.equal(S.startVisible(),true);assert.ok(document.body.classList.contains('cf-start'));assert.ok(S.state.ftStart.classList.contains('active'));assert.ok(!S.state.ftDoc.classList.contains('active'));
 S.showStart(false);assert.equal(S.startVisible(),false);assert.ok(S.state.ftDoc.classList.contains('active'));
 // running any ribbon command from the Start page leaves it
 S.showStart(true);S.exec({cmd:'ID',label:'ID'});assert.equal(S.startVisible(),false);
 // New drawing button on the file tab bar runs NEW
 `);
 console.log('PASS: Start page toggles with file tabs and body class; commands leave it');
 // ---- search ------------------------------------------------------------------------------------------------
 await run(`
 const S=CF.shell;let ran=0;CF.register({name:'cfshelltest',aliases:['cfst'],label:'Shell Test',desc:'Searchable test command',run:()=>{ran++}});
 let r=S.search('cfst');assert.equal(r[0].name,'CFSHELLTEST');r=S.search('searchable');assert.ok(r.some(x=>x.name==='CFSHELLTEST'));
 r[0].run();assert.equal(ran,1);
 assert.equal(S.search('').length,0);const l=S.search('line');assert.ok(l.length>0&&l[0].name==='LINE'||l.some(x=>x.name==='LINE'));
 const ex=S.search('trim');assert.ok(ex.some(x=>x.name==='TRIM'));
 CF.commands.delete('CFSHELLTEST');CF.aliases.delete('CFST');
 `);
 console.log('PASS: command search ranks name/alias/description matches and runs the chosen command');
 // ---- application menu + quick access toolbar -------------------------------------------------------------
 await run(`
 const S=CF.shell;assert.deepEqual(S.state.qat.map(q=>q.cmd),['NEW','OPEN','QSAVE','SAVEAS','PLOT','UNDO','REDO']);
 S.openAppMenu();assert.ok(S.state.menus.length===1);S.closeMenus();assert.equal(S.state.menus.length,0);
 `);
 console.log('PASS: Quick Access Toolbar commands (New, Open, Save, Save As, Plot, Undo, Redo); application menu opens and closes');
 // ---- title-bar search hands the keyboard back to the command line ----------------------------------------
 await run(`
 const S=CF.shell,input=S.state.searchInput;let blurs=0;input.blur=()=>{blurs++};
 for(const via of ['enter','click']){
  input.value='line';input.oninput();assert.ok(S.state.menus.length===1,'search popup open');const pop=S.state.menus[0].el;
  if(via==='enter')input.onkeydown({key:'Enter',stopPropagation(){},preventDefault(){}});else pop.children[0].onclick();
  assert.equal(S.state.menus.length,0,via+': popup closed');assert.equal(input.value,'',via+': field cleared');assert.equal(blurs,via==='enter'?1:2,via+': field blurred');
  CF.cancel?.();CF.commandLine?.cancel?.();}
 setTool('select');
 `);
 console.log('PASS: running a command from the title-bar search clears and blurs the field so typing reaches the command line');
 // ---- Model/Layout round trip keeps the 3D view; ribbon View / Visual Style combos follow the viewport -------
 if(!await run("return CF.has('views')")){console.log('SKIP: shell view-state checks need the views module');return}
 await run(`
 const S=CF.shell,savedSolids=doc.solids;doc.solids=[makeExtrusion([{x:0,y:0},{x:10,y:0},{x:10,y:20},{x:0,y:20}],30,'T')];
 try{
  CF.setSpace('model');CF.setVisualStyle('2dwireframe');CF.setViewPreset('top');
  CF.setVisualStyle('shaded');CF.setViewPreset('ne');assert.equal(mode3D,true);const cam={yaw:camera3.yaw,pitch:camera3.pitch,scale:camera3.scale};
  CF.setSpace('layout');assert.equal(mode3D,false);assert.equal(CF.space,'layout');
  CF.setSpace('model');assert.equal(CF.space,'model');assert.equal(mode3D,true,'3D view restored');
  assert.equal(camera3.yaw,cam.yaw);assert.equal(camera3.pitch,cam.pitch);assert.equal(camera3.scale,cam.scale);assert.equal(CF.visualStyle(),'shaded');
  // a plain 2D model never gains a 3D view from the round trip
  CF.setViewPreset('top');assert.equal(mode3D,false);CF.setSpace('layout');CF.setSpace('model');assert.equal(mode3D,false);
  // a new drawing forgets the remembered 3D view
  CF.setViewPreset('se');CF.setSpace('layout');CF.emit('document',{name:'X'});CF.setSpace('model');assert.equal(mode3D,false);
 }finally{CF.setSpace('model');CF.setViewPreset('top');CF.setVisualStyle('2dwireframe');doc.solids=savedSolids}
 `);
 console.log('PASS: returning from Layout to Model restores the 3D camera and visual style; 2D models and new drawings are untouched');
 await run(`
 const S=CF.shell,savedSolids=doc.solids;doc.solids=[makeExtrusion([{x:0,y:0},{x:10,y:0},{x:10,y:20},{x:0,y:20}],30,'T')];
 try{
  CF.setWorkspace('3d');S.selectTab('Home');
  const combo=id=>S.buttons.filter(b=>b.split===id),show=(id)=>combo(id).map(b=>b.current());
  CF.setVisualStyle('2dwireframe');CF.setViewPreset('top');
  assert.ok(combo('vstyle').length>=1&&combo('preset').length>=1);
  assert.ok(show('vstyle').every(i=>i.style==='2dwireframe')&&show('preset').every(i=>i.preset==='top'),'fresh state');
  CF.setVisualStyle('shaded');CF.setViewPreset('se');
  assert.ok(show('vstyle').every(i=>i.style==='shaded'),'style follows CF.setVisualStyle');assert.ok(show('preset').every(i=>i.preset==='se'),'view follows CF.setViewPreset');
  for(const b of combo('vstyle'))assert.ok(b.main.innerHTML.includes(CF.icon('vs-shaded',16))||b.main.innerHTML.includes(CF.icon('vs-shaded',32)),'style icon');
  for(const b of combo('preset'))assert.ok(b.main.innerHTML.includes('Shaded')===false&&(b.main.innerHTML.includes('SE Isometric')||b.arrow.innerHTML.includes('SE Isometric')),'view label');
  // a ribbon click and a viewport change both end up reflected
  S.exec(S.buttons.find(b=>b.split==='preset').items.find(i=>i.preset==='left'),S.buttons.find(b=>b.split==='preset'),4);assert.ok(show('preset').every(i=>i.preset==='left'));
  CF.setViewPreset('nw');assert.ok(show('preset').every(i=>i.preset==='nw'),'viewport change overrides the last click');
  // the combos survive a ribbon rebuild
  CF.setWorkspace('drafting');CF.setWorkspace('3d');S.selectTab('Home');assert.ok(show('preset').every(i=>i.preset==='nw')&&show('vstyle').every(i=>i.style==='shaded'));
  CF.setViewPreset('top');CF.setVisualStyle('2dwireframe');assert.ok(show('preset').every(i=>i.preset==='top')&&show('vstyle').every(i=>i.style==='2dwireframe'));
 }finally{CF.setViewPreset('top');CF.setVisualStyle('2dwireframe');CF.setWorkspace('drafting');doc.solids=savedSolids}
 `);
 console.log('PASS: ribbon View and Visual Style combos track the real view and style, not the last clicked item');};
