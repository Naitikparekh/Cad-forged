// Command layer: registry/aliases, command-line input parsing, prompts, picking phase, LINE/PLINE/COPY/TEXT/CIRCLE flows, ZOOM, clipboard, keyboard.
module.exports=async({run,assert})=>{
 if(!await run("return typeof CF!=='undefined'&&CF.has('commands')")){console.log('SKIP: acad-commands');return}
 await run(`
 const tick=()=>new Promise(r=>setTimeout(r,0));
 // Verify.cjs replaces cadPrompt with its answer-queue stub after load; route prompts to the command line for these tests.
 globalThis.__stubPrompt=cadPrompt;cadPrompt=CF.commandLine.cadPrompt;
 globalThis.T={tick,eq:(a,b,msg)=>assert.equal(JSON.stringify(a),JSON.stringify(b),msg),
  reset(){CF.cancel();doc={layers:[{name:'0',color:'#63d9c0',visible:true}],entities:[]};history=[];future=[];setTool('select');points=[];selected=-1;selectionSet.clear();CF.input=null;CF.picking=null;$('command').value='';mouse={x:0,y:0};view={x:0,y:0,scale:4};W=1000;H=700;CF.history.length=0;CF.set('ortho',false,{quiet:true});CF.set('polar',false,{quiet:true});CF.set('snap',false,{quiet:true})},
  sub:async s=>{await CF.commandLine.submit(s);await tick()},
  key:(k,extra={})=>{let prevented=false;const e={key:k,target:$('command'),preventDefault(){prevented=true},...extra};$('command').onkeydown(e);if(!e.cfHandled)document.onkeydown(e);return{prevented,e}},
  typeLine:async s=>{const c=$('command');c.value=s;T.key('Enter');await tick();await tick()},
  lines:()=>CF.history.slice(),
  has:t=>CF.history.some(l=>l.includes(t)),
  line:(a,b,c,d)=>({type:'line',layer:'0',points:[{x:a,y:b},{x:c,y:d}]}),
  opts:()=>{const out=[];const walk=n=>{if(!n||typeof n==='string')return;if(n.className==='cf-cmd-opt')out.push(n);(n.children||[]).forEach(walk)};walk(CF.hosts.cmdline);return out}};
 T.reset();
 `);

 // ---- Registry and aliases -------------------------------------------------------------------------------------
 await run(`
 const pairs=[['L','LINE'],['PL','PLINE'],['C','CIRCLE'],['REC','RECTANG'],['RECTANGLE','RECTANG'],['A','ARC'],['M','MOVE'],['CO','COPY'],['CP','COPY'],['RO','ROTATE'],['SC','SCALE'],['E','ERASE'],['X','EXPLODE'],['J','JOIN'],['Z','ZOOM'],['P','PAN'],['U','UNDO'],['DT','TEXT'],['DTEXT','TEXT'],['MT','MTEXT'],['MTEXT','MTEXT'],['T','TEXT'],['H','HATCH'],['BHATCH','HATCH'],['B','BLOCK'],['I','INSERT'],['DIM','DIMLINEAR'],['DLI','DIMLINEAR'],['DAL','DIMALIGNED'],['MEA','MEASUREGEOM'],['PR','PROPERTIES'],['PRCLOSE','PROPERTIESCLOSE'],['LA','LAYER'],['SN','SNAP'],['OS','OSNAP'],['EXT','EXTRUDE'],['UNI','UNION'],['SU','SUBTRACT'],['IN','INTERSECT'],['3M','3DMOVE'],['PRINT','PLOT'],['SAVE','QSAVE'],['VS','VSCURRENT'],['V','VIEW'],['RE','REGEN'],['3DO','3DORBIT'],['AI_SELALL','SELECTALL'],['EXP','EXPORT'],['_line','LINE'],["'zoom",'ZOOM']];
 for(const [a,n] of pairs)assert.equal(CF.resolve(a)?.name,n,a);
 for(const n of ['LINE','PLINE','CIRCLE','ARC','RECTANG','TEXT','MTEXT','HATCH','DIMALIGNED','DIMLINEAR','MOVE','COPY','ROTATE','SCALE','MIRROR','ERASE','EXPLODE','JOIN','BLOCK','INSERT','MEASUREGEOM','SELECTALL','UNDO','REDO','COPYCLIP','CUTCLIP','PASTECLIP','HELP','ZOOM','PAN','REGEN','3DORBIT','PLAN','VIEW','VSCURRENT','MODEL','LAYOUT','GRID','SNAP','ORTHO','OSNAP','POLAR','DYNMODE','CLEANSCREENON','CLEANSCREENOFF','COMMANDLINE','COMMANDLINEHIDE','TEXTSCR','PROPERTIES','PROPERTIESCLOSE','LAYER','RIBBON','RIBBONCLOSE','WSCURRENT','NEW','OPEN','QSAVE','SAVEAS','PLOT','PAGESETUP','EXPORT','DXFOUT','DXFIN','SVGOUT','STLOUT','OBJEXPORT','EXTRUDE','BOX','UNION','SUBTRACT','INTERSECT','UPDATEEXTRUSION','3DMOVE','MESHCOPY','MESHCLEAR','MESHFIT'])assert.ok(CF.commands.has(n),n);
 for(const d of CF.commands.values())if(d.category)assert.ok(d.desc,d.name+' needs a description');
 assert.equal(typeof CF.commandLine.submit,'function');assert.ok(Array.isArray(CF.history));assert.equal(CF.prompt(),'Command:');
 `);
 console.log('PASS: command registry names and aliases');

 // ---- Parsing --------------------------------------------------------------------------------------------------
 await run(`
 const P=CF.commandLine.parsePoint,near=(p,x,y)=>assert.ok(p&&Math.abs(p.x-x)<1e-9&&Math.abs(p.y-y)<1e-9,JSON.stringify(p)+' vs '+x+','+y);
 near(P('10,20'),10,20);near(P(' -3.5 , .5 '),-3.5,.5);near(P('1,2,0'),1,2);near(P('@5,-2',{x:10,y:10}),15,8);near(P('@10<90',{x:1,y:1}),1,11);near(P('@10<0',{x:0,y:0}),10,0);near(P('10<180'),-10,0);
 near(P('25',{x:0,y:0},{x:0,y:50}),0,25);near(P('25',{x:5,y:5},{x:5,y:5}),30,5);near(P('-4',{x:0,y:0},{x:10,y:0}),-4,0);
 assert.equal(P('25'),null);assert.equal(P('abc'),null);assert.equal(P('1,'),null);assert.equal(P(''),null);
 `);
 console.log('PASS: point text parsing (absolute, relative, polar, direct distance)');

 // ---- PLINE: absolute, polar, relative, close ----------------------------------------------------------------
 await run(`
 T.reset();await T.sub('PLINE');assert.equal(tool,'polyline');assert.equal(CF.prompt(),'PLINE Specify start point:');
 await T.sub('0,0');assert.equal(CF.prompt(),'PLINE Specify next point or [Arc/Length/Undo]:');
 await T.sub('@10<0');await T.sub('@0,10');await T.sub('@-10,0');assert.equal(points.length,4);
 await T.sub('C');assert.equal(doc.entities.length,1);const pl=doc.entities[0];assert.equal(pl.type,'polyline');assert.ok(pl.closed);
 T.eq(pl.points.map(p=>[p.x,p.y]),[[0,0],[10,0],[10,10],[0,10]]);assert.equal(tool,'select');assert.equal(CF.prompt(),'Command:');
 assert.ok(T.has('Command: PLINE'));assert.ok(T.has('PLINE Specify start point: 0,0'));assert.ok(T.has('PLINE Specify next point or [Arc/Length/Undo]: @10<0'));
 // Enter finishes an open polyline; U removes the last vertex.
 T.reset();await T.sub('pl');await T.sub('0,0');await T.sub('5,0');await T.sub('5,5');await T.sub('U');assert.equal(points.length,2);await T.sub('9,9');await T.sub('');
 assert.equal(doc.entities.length,1);assert.ok(!doc.entities[0].closed);assert.equal(doc.entities[0].points.length,3);assert.equal(tool,'select');
 `);
 console.log('PASS: PLINE with absolute, polar and relative input, Close, Undo and Enter');

 // ---- RECTANG ---------------------------------------------------------------------------------------------------
 await run(`
 T.reset();await T.typeLine('REC');assert.equal(tool,'rectangle');assert.equal(CF.prompt(),'RECTANG Specify first corner point:');
 await T.typeLine('0,0');assert.equal(CF.prompt(),'RECTANG Specify other corner point or [Dimensions]:');await T.typeLine('@50,30');
 assert.equal(doc.entities.length,1);assert.ok(doc.entities[0].closed);T.eq(doc.entities[0].points.map(p=>[p.x,p.y]),[[0,0],[50,0],[50,30],[0,30]]);assert.equal(tool,'select');
 // Dimensions option
 T.reset();mouse={x:100,y:100};await T.sub('REC');await T.sub('10,10');await T.sub('D');assert.ok(CF.input&&/Specify length for rectangles <10>:$/.test(CF.input.message),CF.prompt());await T.sub('40');assert.ok(/width/.test(CF.prompt()));await T.sub('20');
 T.eq(doc.entities[0].points.map(p=>[p.x,p.y]),[[10,10],[50,10],[50,30],[10,30]]);assert.equal(tool,'select');
 `);
 console.log('PASS: RECTANG corners, relative corner, Dimensions option');

 // ---- LINE chaining ----------------------------------------------------------------------------------------------
 await run(`
 T.reset();await T.sub('L');assert.equal(CF.prompt(),'LINE Specify first point:');await T.sub('0,0');assert.equal(CF.prompt(),'LINE Specify next point or [Undo]:');
 await T.sub('10,0');assert.equal(doc.entities.length,1);assert.equal(tool,'line');assert.equal(points.length,1);assert.equal(points[0].x,10);assert.equal(CF.prompt(),'LINE Specify next point or [Undo]:');
 await T.sub('@0,10');assert.equal(doc.entities.length,2);assert.equal(CF.prompt(),'LINE Specify next point or [Close/Undo]:');T.eq(doc.entities[1].points.map(p=>[p.x,p.y]),[[10,0],[10,10]]);
 // Undo removes the last segment and continues from its start.
 await T.sub('U');assert.equal(doc.entities.length,1);assert.equal(points[0].x,10);assert.equal(points[0].y,0);assert.equal(CF.prompt(),'LINE Specify next point or [Undo]:');
 await T.sub('@0,5');await T.sub('@-10,0');assert.equal(doc.entities.length,3);
 await T.sub('C');assert.equal(doc.entities.length,4);T.eq(doc.entities[3].points.map(p=>[p.x,p.y]),[[0,5],[0,0]]);assert.equal(tool,'select');
 // Enter ends the chain; Enter at Command: repeats LINE.
 T.reset();await T.sub('LINE');await T.sub('0,0');await T.sub('4,0');await T.sub('4,4');await T.sub('');assert.equal(doc.entities.length,2);assert.equal(tool,'select');
 await T.sub('');assert.equal(tool,'line','empty Enter repeats the last command');assert.equal(CF.prompt(),'LINE Specify first point:');
 // Ctrl+Z inside a chain behaves like U.
 await T.sub('0,0');await T.sub('1,1');assert.equal(doc.entities.length,3);document.onkeydown({key:'z',ctrlKey:true,target:document.body,preventDefault(){}});assert.equal(doc.entities.length,2);assert.equal(tool,'line');
 // Clickable [Undo] option submits U.
 await T.sub('2,2');assert.equal(doc.entities.length,3);const undo=T.opts().find(o=>o.textContent==='Undo');assert.ok(undo,'Undo option rendered');undo.onclick();await T.tick();assert.equal(doc.entities.length,2);
 `);
 console.log('PASS: LINE chaining with Undo, Close, Enter, repeat, Ctrl+Z and clickable options');

 // ---- Direct distance entry --------------------------------------------------------------------------------------
 await run(`
 T.reset();await T.sub('L');await T.sub('0,0');mouse={x:50,y:3};await T.sub('25');
 assert.equal(doc.entities.length,1);let e=doc.entities[0];assert.ok(Math.abs(e.points[1].x-25*Math.cos(Math.atan2(3,50)))<1e-6);assert.ok(Math.abs(e.points[1].y-25*Math.sin(Math.atan2(3,50)))<1e-6);
 const lp=doc.entities[0].points[1];mouse={x:lp.x,y:90};await T.sub('10');const a=doc.entities[1].points[0],b=doc.entities[1].points[1];assert.ok(Math.abs(b.x-a.x)<1e-6&&Math.abs(b.y-a.y-10)<1e-6);
 await T.sub('');T.reset();
 CF.set('ortho',true,{quiet:true});await T.sub('L');await T.sub('0,0');mouse={x:30,y:0};await T.sub('12');assert.equal(doc.entities[0].points[1].x,12);assert.equal(doc.entities[0].points[1].y,0);
 await T.sub('');T.reset();
 // Circle radius by bare number.
 await T.sub('C');await T.sub('5,5');assert.equal(CF.prompt(),'CIRCLE Specify radius of circle or [Diameter]:');await T.sub('7');
 assert.equal(doc.entities.length,1);assert.equal(doc.entities[0].type,'circle');assert.equal(doc.entities[0].radius,7);assert.equal(doc.entities[0].center.x,5);assert.equal(tool,'select');
 `);
 console.log('PASS: direct distance entry for LINE, ortho and CIRCLE radius');

 // ---- CIRCLE: radius, diameter, last radius default ----------------------------------------------------------------
 await run(`
 T.reset();await T.sub('CIRCLE');await T.sub('0,0');await T.sub('D');assert.ok(CF.input,'diameter prompt is pending input');assert.equal(CF.input.kind,'distance');assert.equal(CF.input.message,'CIRCLE Specify diameter of circle <14>:','default is twice the previous radius');
 await T.sub('20');assert.equal(doc.entities.length,1);assert.equal(doc.entities[0].radius,10);assert.equal(tool,'select');
 await T.sub('C');await T.sub('50,0');assert.equal(CF.prompt(),'CIRCLE Specify radius of circle or [Diameter] <10>:');await T.sub('');assert.equal(doc.entities.length,2);assert.equal(doc.entities[1].radius,10);
 await T.sub('C');await T.sub('0,0');await T.sub('d');await T.sub('');assert.equal(doc.entities[2].radius,10,'diameter default is twice the last radius');
 await T.sub('C');await T.sub('0,0');await T.sub('3,4');assert.equal(doc.entities[3].radius,5);
 // Clicking resolves the pending distance input like the canvas does.
 await T.sub('C');await T.sub('0,0');await T.sub('D');CF.input.resolve('6');await T.tick();await T.tick();assert.equal(doc.entities[4].radius,3);
 `);
 console.log('PASS: CIRCLE radius, Diameter option, default radius, point on circumference');

 // ---- ARC and DIMALIGNED / DIMLINEAR -------------------------------------------------------------------------------
 await run(`
 T.reset();await T.sub('ARC');assert.equal(CF.prompt(),'ARC Specify start point of arc or [Center]:');await T.sub('C');assert.equal(CF.prompt(),'ARC Specify center point of arc:');await T.sub('0,0');assert.equal(CF.prompt(),'ARC Specify start point of arc:');await T.sub('10,0');assert.equal(CF.prompt(),'ARC Specify end point of arc or [Angle]:');await T.sub('0,10');
 assert.equal(doc.entities.length,1);assert.equal(doc.entities[0].type,'polyline');assert.ok(Math.abs(doc.entities[0].points.at(-1).y-10)<1e-6);assert.equal(tool,'select');
 T.reset();await T.sub('DAL');assert.equal(tool,'dimension');assert.equal(CF.prompt(),'DIMALIGNED Specify first extension line origin:');await T.sub('0,0');assert.equal(CF.prompt(),'DIMALIGNED Specify second extension line origin:');await T.sub('30,40');assert.equal(CF.prompt(),'DIMALIGNED Specify dimension line location:');await T.sub('10,10');
 assert.ok(doc.entities.some(e=>e.type==='text'&&e.text==='50.00'));assert.equal(tool,'select');
 T.reset();await T.sub('DIMLINEAR');assert.ok(['dimension','dimlinear'].includes(tool),'DIMLINEAR starts the dimension tool (B2 may supply its own)');
 T.reset();
 `);
 console.log('PASS: ARC and DIMALIGNED/DIMLINEAR prompts');

 // ---- TEXT ----------------------------------------------------------------------------------------------------------
 await run(`
 T.reset();await T.sub('T');assert.equal(CF.prompt(),'TEXT Specify start point of text:');await T.sub('5,5');
 assert.ok(CF.input,'height prompt pending');assert.equal(CF.input.message,'TEXT Specify height <5>:');assert.equal(CF.input.defaultValue,'5');assert.equal(CF.input.kind,'distance');
 await T.sub('3');assert.equal(CF.input.message,'TEXT Enter text:');assert.equal(CF.input.kind,'text');
 // Space is part of the text, not Enter.
 $('command').value='Hello world';T.key(' ');assert.ok(CF.input,'space does not submit text');assert.equal($('command').value,'Hello world');
 await T.sub('Hello world');
 assert.equal(doc.entities.length,1);const t=doc.entities[0];assert.equal(t.type,'text');assert.equal(t.text,'Hello world');assert.equal(t.height,3);assert.equal(t.points[0].x,5);assert.equal(tool,'select');
 // The last height becomes the default.
 await T.sub('DT');await T.sub('0,0');assert.equal(CF.input.message,'TEXT Specify height <3>:');await T.sub('');await T.sub('Again');assert.equal(doc.entities[1].height,3);assert.equal(doc.entities[1].text,'Again');
 // Esc at the text prompt creates nothing.
 await T.sub('T');await T.sub('1,1');await T.sub('2');document.onkeydown({key:'Escape',target:$('command'),preventDefault(){}});await T.tick();assert.equal(doc.entities.length,2);assert.equal(CF.input,null);assert.equal(tool,'select');
 `);
 console.log('PASS: TEXT start point, height default, text, space handling, cancel');

 // ---- cadPrompt routing ----------------------------------------------------------------------------------------------
 await run(`
 T.reset();
 let pr=cadPrompt('Rotation angle in degrees','90');assert.ok(CF.input);assert.equal(CF.input.message,'Specify rotation angle or [Copy/Reference] <90>:');assert.equal(CF.input.kind,'angle');assert.equal(CF.input.defaultValue,'90');assert.equal(CF.prompt(),CF.input.message);
 await T.sub('');assert.equal(await pr,'90','empty Enter returns the default');assert.equal(CF.input,null);
 pr=cadPrompt('Scale factor','2');assert.equal(CF.input.message,'Specify scale factor or [Copy/Reference] <2>:');assert.equal(CF.input.kind,'factor');await T.sub('0.5');assert.equal(await pr,'0.5');
 pr=cadPrompt('Text');assert.equal(CF.input.message,'Enter text:');assert.equal(CF.input.kind,'text');await T.sub('abc');assert.equal(await pr,'abc');
 pr=cadPrompt('Hatch spacing','5');assert.equal(CF.input.message,'Specify hatch spacing <5>:');assert.equal(CF.input.kind,'distance');await T.sub('');assert.equal(await pr,'5');
 pr=cadPrompt('Block name');assert.equal(CF.input.message,'Enter block name:');await T.sub('Door');assert.equal(await pr,'Door');
 pr=cadPrompt('Block name: a, b','a');assert.equal(CF.input.message,'Enter block name to insert [a/b] <a>:');assert.ok(CF.input.literal);await T.sub('b');assert.equal(await pr,'b');
 pr=cadPrompt('Layer name');assert.equal(CF.input.message,'Enter name for new layer:');CF.input.resolve('Walls');assert.equal(await pr,'Walls');
 pr=cadPrompt('Box width, depth, height','100,60,30');assert.equal(CF.input.message,'Specify box width, depth, height <100,60,30>:');await T.sub('');assert.equal(await pr,'100,60,30');
 pr=cadPrompt('Unmapped engine message','x');assert.equal(CF.input.message,'Unmapped engine message <x>:');CF.input.resolve('');assert.equal(await pr,'x');
 // Already worded prompts (drafting module) pass through with their own default and options.
 pr=cadPrompt('Specify offset distance or [Through] <5.0000>:','',{kind:'distance',defaultValue:'5.0000'});assert.equal(CF.input.message,'Specify offset distance or [Through] <5.0000>:');assert.equal(CF.input.kind,'distance');await T.sub('');assert.equal(await pr,'5.0000');
 // The prompt and history show the answer; options are clickable.
 pr=cadPrompt('Rotation angle in degrees','90');await T.sub('45');assert.equal(await pr,'45');assert.ok(T.has('Specify rotation angle or [Copy/Reference] <90>: 45'));
 pr=cadPrompt('Rotation angle in degrees','90');T.eq(T.opts().map(o=>o.textContent),['Copy','Reference']);T.opts()[0].onclick();await T.tick();assert.ok(CF.input,'Copy keeps the angle prompt open');CF.input.resolve('1');await T.tick();
 // Esc cancels pending input and prints *Cancel*.
 T.reset();pr=cadPrompt('Layer name');document.onkeydown({key:'Escape',target:$('command'),preventDefault(){}});assert.equal(await pr,null);assert.equal(CF.input,null);assert.ok(T.has('*Cancel*'));
 // A second prompt supersedes the first (like the modal dialog).
 const p1=cadPrompt('Text'),p2=cadPrompt('Layer name');assert.equal(await p1,null);CF.input.resolve('z');assert.equal(await p2,'z');
 T.reset();
 `);
 console.log('PASS: cadPrompt routed to the command line with AutoCAD wording, defaults, kinds and cancel');

 // ---- Selection phase + MOVE / COPY / ROTATE / SCALE / ERASE -------------------------------------------------------
 await run(`
 T.reset();doc.entities=[T.line(0,0,10,0),T.line(0,5,10,5),T.line(50,50,60,60)];
 await T.sub('M');assert.ok(CF.picking,'selection phase starts');assert.equal(CF.picking.command,'MOVE');assert.equal(CF.prompt(),'MOVE Select objects:');assert.equal(tool,'select');
 CF.select([0,1],'add');await T.sub('');assert.equal(CF.picking,null);assert.equal(tool,'move');assert.equal(CF.prompt(),'MOVE Specify base point or [Displacement] <Displacement>:');
 await T.sub('0,0');assert.equal(CF.prompt(),'MOVE Specify second point or <use first point as displacement>:');await T.sub('@5,10');
 T.eq(doc.entities[0].points.map(p=>[p.x,p.y]),[[5,10],[15,10]]);T.eq(doc.entities[1].points.map(p=>[p.x,p.y]),[[5,15],[15,15]]);T.eq(doc.entities[2].points[0],{x:50,y:50});
 assert.equal(tool,'select');assert.equal(selected,-1,'selection clears after a modify command');
 // Noun-verb: with a selection the object phase is skipped.
 CF.select([2]);await T.sub('M');assert.equal(CF.picking,null);assert.equal(CF.prompt(),'MOVE Specify base point or [Displacement] <Displacement>:');await T.sub('50,50');await T.sub('0,0');T.eq(doc.entities[2].points[0],{x:0,y:0});
 // Enter with nothing selected cancels the command.
 T.reset();await T.sub('M');await T.sub('');assert.equal(CF.picking,null);assert.equal(tool,'select');assert.equal(CF.prompt(),'Command:');
 // Esc in the selection phase cancels and clears.
 await T.sub('M');assert.ok(CF.picking);document.onkeydown({key:'Escape',target:document.body,preventDefault(){}});assert.equal(CF.picking,null);assert.equal(tool,'select');
 // Typed ALL / Last / point picks.
 doc.entities=[T.line(0,0,10,0),T.line(0,5,10,5),T.line(0,9,10,9)];await T.sub('E');await T.sub('ALL');assert.equal(chosen().length,3);await T.sub('');assert.equal(doc.entities.length,0);
 doc.entities=[T.line(0,0,10,0),T.line(0,5,10,5)];await T.sub('E');await T.sub('L');T.eq(chosen(),[1]);await T.sub('');assert.equal(doc.entities.length,1);
 doc.entities=[T.line(0,0,10,0),T.line(0,5,10,5)];await T.sub('E');await T.sub('5,5');T.eq(chosen(),[1]);await T.sub('x,y');await T.sub('');assert.equal(doc.entities.length,1);
 // ERASE with a selection acts immediately (noun-verb).
 doc.entities=[T.line(0,0,10,0),T.line(0,5,10,5)];CF.select([0]);await T.sub('ERASE');assert.equal(doc.entities.length,1);assert.equal(tool,'select');
 undo();assert.equal(doc.entities.length,2);
 `);
 console.log('PASS: object selection phase, MOVE, noun-verb, ERASE and typed selection options');

 await run(`
 T.reset();doc.entities=[T.line(0,0,10,0),T.line(0,0,0,10)];CF.select([0,1],'add');
 await T.sub('CO');assert.equal(tool,'copy');assert.equal(CF.prompt(),'COPY Specify base point or [Displacement] <Displacement>:');await T.sub('0,0');assert.equal(CF.prompt(),'COPY Specify second point or <use first point as displacement>:');
 await T.sub('@20,0');assert.equal(doc.entities.length,4);assert.equal(points.length,1,'base point is kept for the next copy');assert.equal(CF.prompt(),'COPY Specify second point or [Exit/Undo] <Exit>:','after the first copy Enter exits');await T.sub('@40,0');assert.equal(doc.entities.length,6);
 T.eq(doc.entities[4].points.map(p=>[p.x,p.y]),[[40,0],[50,0]]);
 await T.sub('U');assert.equal(doc.entities.length,4);assert.equal(chosen().length,2,'selection restored after undo');await T.sub('@0,30');assert.equal(doc.entities.length,6);
 await T.sub('');assert.equal(tool,'select');assert.equal(selected,-1);assert.equal(doc.entities.length,6);
 // Exit option
 CF.select([0]);await T.sub('copy');await T.sub('0,0');await T.sub('@1,1');await T.sub('E');assert.equal(tool,'select');assert.equal(doc.entities.length,7);
 `);
 console.log('PASS: COPY multiple with Undo, Exit and Enter');

 await run(`
 T.reset();doc.entities=[T.line(10,0,20,0)];
 await T.sub('RO');assert.equal(CF.picking.command,'ROTATE');CF.select([0]);await T.sub('');assert.equal(tool,'rotate');assert.equal(CF.prompt(),'ROTATE Specify base point:');
 await T.sub('0,0');assert.ok(CF.input,'angle prompt');assert.equal(CF.input.message,'ROTATE Specify rotation angle or [Copy/Reference] <90>:');assert.equal(CF.input.kind,'angle');T.eq(CF.input.base,{x:0,y:0});
 await T.sub('');await T.tick();assert.ok(Math.abs(doc.entities[0].points[0].x)<1e-9&&Math.abs(doc.entities[0].points[0].y-10)<1e-9,JSON.stringify(doc.entities[0].points));assert.equal(tool,'select');
 // Canvas click on the angle prompt: degrees from the base point.
 T.reset();doc.entities=[T.line(10,0,20,0)];CF.select([0]);await T.sub('RO');await T.sub('0,0');CF.input.resolve('180');await T.tick();await T.tick();assert.ok(Math.abs(doc.entities[0].points[0].x+10)<1e-9);
 // Copy option leaves the original in place.
 T.reset();doc.entities=[T.line(10,0,20,0)];CF.select([0]);await T.sub('RO');await T.sub('0,0');await T.sub('C');assert.ok(CF.input,'prompt stays after Copy');await T.sub('90');await T.tick();
 assert.equal(doc.entities.length,2);T.eq(doc.entities[0].points[0],{x:10,y:0});assert.ok(Math.abs(doc.entities[1].points[0].y-10)<1e-9);
 // Reference option: reference angle 30, new angle 75 -> rotates by 45.
 T.reset();doc.entities=[T.line(10,0,20,0)];CF.select([0]);await T.sub('RO');await T.sub('0,0');await T.sub('R');await T.sub('30');await T.sub('75');await T.tick();
 const p=doc.entities[0].points[0];assert.ok(Math.abs(Math.atan2(p.y,p.x)*180/Math.PI-45)<1e-6);
 // SCALE with a factor, then Reference.
 T.reset();doc.entities=[T.line(10,0,20,0)];CF.select([0]);await T.sub('SC');assert.equal(CF.prompt(),'SCALE Specify base point:');await T.sub('0,0');assert.equal(CF.input.message,'SCALE Specify scale factor or [Copy/Reference] <2>:');await T.sub('3');await T.tick();assert.equal(doc.entities[0].points[1].x,60);
 T.reset();doc.entities=[T.line(10,0,20,0)];CF.select([0]);await T.sub('SC');await T.sub('0,0');await T.sub('R');await T.sub('5');await T.sub('10');await T.tick();assert.equal(doc.entities[0].points[1].x,40);
 `);
 console.log('PASS: ROTATE and SCALE through the command line (default, click, Copy, Reference)');

 // ---- Other engine-backed commands ------------------------------------------------------------------------------
 await run(`
 T.reset();doc.entities=[{type:'polyline',layer:'0',closed:true,points:[{x:0,y:0},{x:10,y:0},{x:10,y:10},{x:0,y:10}]}];
 CF.select([0]);await T.sub('X');assert.equal(doc.entities.length,4,'EXPLODE splits the polyline');assert.equal(doc.entities[0].type,'line');
 T.reset();doc.entities=[{type:'polyline',layer:'0',closed:true,points:[{x:0,y:0},{x:10,y:0},{x:10,y:10},{x:0,y:10}]}];
 await T.sub('H');assert.equal(CF.picking.command,'HATCH');CF.select([0]);const hp=T.sub('');await T.tick();assert.ok(CF.input&&CF.input.message==='HATCH Specify hatch spacing <5>:',CF.prompt());await T.sub('2');await T.tick();assert.ok(doc.entities.length>5,'hatch lines created');
 T.reset();doc.entities=[T.line(0,0,3,4)];CF.select([0]);await T.sub('MEA');assert.ok(T.has('Length: 5.0000'));
 T.reset();doc.entities=[T.line(0,0,10,0),T.line(10,0,10,10),T.line(10,10,0,10),T.line(0,10,0,0)];CF.select([0,1,2,3],'add');await T.sub('J');await T.tick();assert.equal(doc.entities.length,1);assert.ok(doc.entities[0].closed);
 T.reset();doc.entities=[T.line(0,0,10,0)];CF.select([0]);const bl=T.sub('B');await T.tick();assert.equal(CF.input.message,'BLOCK Enter block name:');await T.sub('Tag');await T.tick();assert.equal(CF.input.message,'BLOCK Specify insertion base point <0,0,0>:','BLOCK asks for the base point');await T.sub('');await T.tick();assert.ok(doc.blocks.Tag);
 await T.sub('INSERT');await T.tick();assert.equal(CF.input.message,'INSERT Enter block name to insert [Tag] <Tag>:');await T.sub('');await T.tick();assert.equal(tool,'insert');assert.equal(CF.prompt(),'INSERT Specify insertion point or [Scale/Rotate]:');await T.sub('100,0');assert.equal(doc.entities.length,2);assert.equal(tool,'select');
 T.reset();doc.entities=[T.line(0,0,10,0),T.line(5,-5,5,5)];await T.sub('SELECTALL');assert.equal(chosen().length,2);await T.sub('UNDO');
 `);
 console.log('PASS: EXPLODE, HATCH boundary, MEASUREGEOM, JOIN, BLOCK, INSERT, SELECTALL');

 // ---- Esc, repeat, recall ----------------------------------------------------------------------------------------
 await run(`
 T.reset();doc.entities=[T.line(0,0,10,0),T.line(0,5,10,5)];CF.select([0,1],'add');await T.sub('L');await T.sub('0,0');
 let prevented=false;document.onkeydown({key:'Escape',target:$('command'),preventDefault(){prevented=true}});assert.ok(prevented);assert.equal(tool,'select');assert.equal(points.length,0);assert.ok(T.has('*Cancel*'));
 assert.equal(chosen().length,2,'first Esc cancels the command and keeps the selection');
 document.onkeydown({key:'Escape',target:$('command'),preventDefault(){}});assert.equal(selected,-1);assert.equal(chosen().length,0,'second Esc clears the selection');
 // Esc clears typed text and the AutoComplete popup.
 $('command').value='li';$('command').oninput();assert.ok(CF.commandLine.state().ac>0);document.onkeydown({key:'Escape',target:$('command'),preventDefault(){}});assert.equal($('command').value,'');assert.equal(CF.commandLine.state().ac,0);
 // Esc does not leave the 3D view.
 T.reset();show3D();assert.ok(mode3D);document.onkeydown({key:'Escape',target:document.body,preventDefault(){}});assert.ok(mode3D,'Esc keeps the 3D view');showModel();
 // Verify-style: Esc with the select tool and one selected entity.
 T.reset();doc.entities=[T.line(1.25,1.75,3.25,1.75)];view={x:0,y:0,scale:100};setTool('select');canvas.onpointerdown({button:0,offsetX:700,offsetY:175,pointerId:1,shiftKey:false});assert.equal(selected,0);
 prevented=false;document.onkeydown({key:'Escape',target:document.getElementById('command'),preventDefault(){prevented=true}});assert.ok(prevented);assert.equal(selected,-1);
 // Empty Enter repeats; typed input is recalled with the arrow keys.
 T.reset();await T.typeLine('REC');await T.typeLine('0,0');await T.typeLine('@5,5');assert.equal(tool,'select');
 await T.typeLine('');assert.equal(tool,'rectangle','empty Enter repeats RECTANG');assert.ok(T.has('Command: RECTANG'));document.onkeydown({key:'Escape',target:$('command'),preventDefault(){}});
 await T.typeLine('GRID');assert.equal(CF.get('grid'),false);await T.typeLine('GRID');assert.equal(CF.get('grid'),true);
 $('command').value='';let r=T.key('ArrowUp');assert.ok(r.prevented);assert.equal($('command').value,'GRID');T.key('ArrowUp');assert.equal($('command').value,'@5,5');T.key('ArrowUp');assert.equal($('command').value,'0,0');T.key('ArrowDown');assert.equal($('command').value,'@5,5');T.key('ArrowDown');assert.equal($('command').value,'GRID');T.key('ArrowDown');assert.equal($('command').value,'');
 // Space acts as Enter at the command prompt.
 T.reset();$('command').value='LINE';T.key(' ');await T.tick();assert.equal(tool,'line');assert.equal($('command').value,'');document.onkeydown({key:'Escape',target:$('command'),preventDefault(){}});
 // Unknown commands report like AutoCAD.
 await T.sub('FOOBAR');assert.ok(T.has('Unknown command "FOOBAR".  Press F1 for help.'));assert.equal(tool,'select');
 `);
 console.log('PASS: Escape (cancel, second Esc, 3D, Verify scenario), empty-Enter repeat, ArrowUp recall, Space, unknown command');

 // ---- AutoComplete --------------------------------------------------------------------------------------------------
 await run(`
 const ac=CF.commandLine.autocomplete,names=q=>ac(q).map(m=>m.name);
 assert.ok(names('li').slice(0,3).includes('LINE'));assert.equal(names('lin')[0],'LINE');assert.equal(names('l')[0],'LINE');assert.equal(names('pl')[0],'PLINE');assert.equal(names('rect')[0],'RECTANG');assert.equal(names('z')[0],'ZOOM');assert.equal(names('MOV')[0],'MOVE');
 assert.equal(names('ext')[0],'EXTRUDE','exact alias first');assert.ok(names('ext').includes('EXTEND'));assert.ok(names('ext').indexOf('EXTRUDE')<names('ext').indexOf('EXTEND'));
 assert.ok(names('mesh').length>=4&&names('mesh').slice(0,3).every(n=>n.startsWith('MESH')));assert.equal(names('xyzzy').length,0);assert.equal(ac('').length,0);assert.ok(ac('e').length<=10);
 assert.ok(names('unions').length===0);assert.ok(names('wireframe').includes('VSCURRENT'),'contains-match on the description');assert.ok(names('clipboard').includes('COPYCLIP'));
 const m=ac('circ')[0];assert.equal(m.name,'CIRCLE');assert.equal(m.alias,'C');assert.ok(m.desc.length>5);assert.equal(m.icon,'circle');
 // Popup behaviour: typing, arrows, Tab, Enter and click.
 T.reset();const c=$('command');c.value='lin';c.oninput();let st=CF.commandLine.state();assert.ok(st.ac>0);
 assert.equal(T.key('ArrowDown').prevented,true);assert.equal(CF.commandLine.state().acIndex,0);T.key('Tab');assert.equal(c.value,'LINE');assert.equal(CF.commandLine.state().ac,0);c.value='';
 c.value='pl';c.oninput();T.key('ArrowDown');T.key('Enter');await T.tick();assert.equal(tool,'polyline');assert.equal(c.value,'');document.onkeydown({key:'Escape',target:c,preventDefault(){}});
 c.value='zo';c.oninput();T.key('Tab');assert.equal(c.value,'ZOOM');c.value='';
 // No popup while a command is waiting for input.
 await T.sub('L');c.value='li';c.oninput();assert.equal(CF.commandLine.state().ac,0);document.onkeydown({key:'Escape',target:c,preventDefault(){}});
 `);
 console.log('PASS: AutoComplete matching function and popup navigation');

 // ---- ZOOM -------------------------------------------------------------------------------------------------------
 await run(`
 T.reset();doc.entities=[T.line(0,0,100,0),T.line(100,0,100,50)];
 await T.sub('Z');assert.ok(CF.input);assert.ok(/Extents/.test(CF.prompt()));assert.ok(/^ZOOM Specify window corner/.test(CF.prompt()));assert.equal(tool,'zoom');
 await T.sub('E');assert.equal(tool,'select');const ext={x:50,y:25,scale:8};assert.ok(Math.abs(view.x-ext.x)<1e-9&&Math.abs(view.y-ext.y)<1e-9&&Math.abs(view.scale-ext.scale)<1e-9,JSON.stringify(view));
 await T.sub('Z');await T.sub('I');assert.ok(Math.abs(view.scale-16)<1e-9);await T.sub('Z');await T.sub('O');assert.ok(Math.abs(view.scale-8)<1e-9);
 await T.sub('Z');await T.sub('I');await T.sub('Z');await T.sub('P');assert.ok(Math.abs(view.scale-8)<1e-9,'Previous restores the view before the last zoom');
 await T.sub('Z');await T.sub('P');assert.ok(Math.abs(view.scale-16)<1e-9,'Previous walks back through the view stack');
 // Window by typed corners and by click-style resolution.
 await T.sub('Z');await T.sub('W');assert.equal(CF.input.message,'ZOOM Specify first corner:');await T.sub('0,0');assert.equal(CF.input.message,'ZOOM Specify opposite corner:');
 const pv=CF.previews.zoom({x:10,y:10});assert.equal(pv.length,1);assert.equal(pv[0].points.length,4);await T.sub('10,10');assert.ok(Math.abs(view.x-5)<1e-9&&Math.abs(view.y-5)<1e-9&&Math.abs(view.scale-70)<1e-9,JSON.stringify(view));assert.equal(tool,'select');
 await T.sub('Z');CF.input.resolve('0,0');await T.tick();assert.equal(CF.input.message,'ZOOM Specify opposite corner:');CF.input.resolve('20,20');await T.tick();await T.tick();assert.ok(Math.abs(view.scale-35)<1e-9);
 // Scale factors: nX relative to the current view, n relative to the extents.
 await T.sub('Z');await T.sub('2X');assert.ok(Math.abs(view.scale-70)<1e-9);await T.sub('Z');await T.sub('0.5x');assert.ok(Math.abs(view.scale-35)<1e-9);
 await T.sub('Z');await T.sub('2');assert.ok(Math.abs(view.scale-16)<1e-9);
 // All and Enter (default Extents) and the ribbon argument form.
 view={x:0,y:0,scale:4};await T.sub('Z');await T.sub('A');assert.ok(Math.abs(view.scale-8)<1e-9);view={x:0,y:0,scale:4};await T.sub('Z');await T.sub('');assert.ok(Math.abs(view.scale-8)<1e-9);
 view={x:0,y:0,scale:4};CF.run('ZOOM',{args:'E'});await T.tick();assert.ok(Math.abs(view.scale-8)<1e-9);assert.equal(CF.input,null);assert.equal(tool,'select');
 CF.run('ZOOM',{args:'W'});await T.tick();assert.equal(CF.input.message,'ZOOM Specify first corner:');document.onkeydown({key:'Escape',target:document.body,preventDefault(){}});await T.tick();assert.equal(CF.input,null);assert.equal(tool,'select');
 // Bad input re-asks.
 await T.sub('Z');await T.sub('banana');assert.ok(CF.input,'prompt stays after invalid input');await T.sub('E');assert.equal(CF.input,null);
 // Legacy words from the old command bar still work.
 view={x:0,y:0,scale:4};await T.sub('fit');assert.ok(Math.abs(view.scale-8)<1e-9);
 `);
 console.log('PASS: ZOOM Extents/All/Window/Previous/In/Out/scale factors and view stack');

 // ---- Clipboard ---------------------------------------------------------------------------------------------------
 await run(`
 T.reset();doc.entities=[T.line(10,10,20,10),{type:'circle',layer:'0',center:{x:30,y:30},radius:5}];CF.select([0,1],'add');
 const copyKey={key:'c',ctrlKey:true,target:document.body,preventDefault(){}};document.onkeydown(copyKey);assert.equal(CF.commandLine.state().clip,2);assert.equal(doc.entities.length,2);assert.ok(T.has('2 objects copied'));
 document.onkeydown({key:'v',ctrlKey:true,target:document.body,preventDefault(){}});assert.equal(tool,'paste');assert.equal(CF.prompt(),'PASTECLIP Specify insertion point:');
 await T.sub('100,100');assert.equal(doc.entities.length,4);T.eq(doc.entities[2].points.map(p=>[p.x,p.y]),[[100,100],[110,100]]);T.eq(doc.entities[3].center,{x:120,y:120});assert.equal(tool,'select');T.eq(chosen(),[2,3]);
 assert.notEqual(doc.entities[2].uuid,doc.entities[0].uuid);
 // Grouped objects get a new group.
 T.reset();doc.entities=[{...T.line(0,0,5,0),group:'g1'},{...T.line(0,1,5,1),group:'g1'}];CF.select([0]);await T.sub('COPYCLIP');await T.sub('PASTECLIP');await T.sub('0,50');assert.equal(doc.entities.length,4);assert.ok(doc.entities[2].group&&doc.entities[2].group===doc.entities[3].group&&doc.entities[2].group!=='g1');
 // Cut.
 T.reset();doc.entities=[T.line(0,0,5,0),T.line(0,5,5,5)];CF.select([1]);document.onkeydown({key:'x',ctrlKey:true,target:document.body,preventDefault(){}});assert.equal(doc.entities.length,1);document.onkeydown({key:'v',ctrlKey:true,target:document.body,preventDefault(){}});await T.sub('0,0');
 assert.equal(doc.entities.length,2);T.eq(doc.entities[1].points.map(p=>[p.x,p.y]),[[0,0],[5,0]]);
 `);
 console.log('PASS: COPYCLIP, CUTCLIP and PASTECLIP');

 // ---- Keyboard map ---------------------------------------------------------------------------------------------------
 await run(`
 T.reset();const body=document.body,press=(k,extra={})=>{let p=false;document.onkeydown({key:k,target:body,preventDefault(){p=true},...extra});return p};
 let o=$('ortho').checked;assert.ok(press('F8'));assert.equal($('ortho').checked,!o);press('F8');assert.equal($('ortho').checked,o);
 assert.ok(press('F9'));assert.equal(CF.get('snap'),true);press('F9');assert.equal(CF.get('snap'),false);
 const g=CF.get('grid');press('F7');assert.equal(CF.get('grid'),!g);press('F7');
 const osn=CF.get('osnap');press('F3');assert.equal(CF.get('osnap'),!osn);press('F3');
 press('F10');assert.equal(CF.get('polar'),true);press('F10');assert.equal(CF.get('polar'),false);
 const dy=CF.get('dyn');press('F12');assert.equal(CF.get('dyn'),!dy);press('F12');
 press('F2');assert.equal(CF.commandLine.state().expanded,true);press('F2');assert.equal(CF.commandLine.state().expanded,false);
 press('F1');assert.ok(T.has('CadForge commands'));assert.ok(T.has('Draw: '));assert.ok(T.lines().some(l=>/^Modify: .*MOVE \\(M\\)/.test(l)));assert.ok(T.has('Keys: F1'));assert.equal(CF.commandLine.state().expanded,true);CF.commandLine.expand(false);
 press('1',{ctrlKey:true});assert.equal(CF.get('properties'),true);press('1',{ctrlKey:true});assert.equal(CF.get('properties'),false);
 press('9',{ctrlKey:true});assert.equal(CF.get('commandLine'),false);press('9',{ctrlKey:true});assert.equal(CF.get('commandLine'),true);
 press('0',{ctrlKey:true});assert.equal(CF.get('clean'),true);press('0',{ctrlKey:true});assert.equal(CF.get('clean'),false);
 // Ctrl+A, Delete, Ctrl+Z / Ctrl+Y.
 doc.entities=[T.line(0,0,1,0),T.line(0,1,1,1),T.line(0,2,1,2)];assert.ok(press('a',{ctrlKey:true}));assert.equal(chosen().length,3);
 assert.ok(press('Delete'));assert.equal(doc.entities.length,0);assert.ok(press('z',{ctrlKey:true}));assert.equal(doc.entities.length,3);assert.ok(press('y',{ctrlKey:true}));assert.equal(doc.entities.length,0);
 // Ctrl+Z also works from the command input.
 const c=$('command');document.onkeydown({key:'z',ctrlKey:true,target:c,preventDefault(){}});assert.equal(doc.entities.length,3);
 // Ctrl+N/O/S/P run commands (stub the handlers).
 const ran=[];const orig={};for(const n of ['NEW','OPEN','QSAVE','SAVEAS','PLOT']){const d=CF.commands.get(n);orig[n]=d.run;d.run=()=>{ran.push(n)}}
 press('n',{ctrlKey:true});press('o',{ctrlKey:true});press('s',{ctrlKey:true});press('s',{ctrlKey:true,shiftKey:true});press('p',{ctrlKey:true});
 T.eq(ran,['NEW','OPEN','QSAVE','SAVEAS','PLOT']);for(const n in orig)CF.commands.get(n).run=orig[n];
 // Typing anywhere goes to the command line.
 T.reset();assert.ok(press('L'));assert.equal($('command').value,'L');$('command').value='';
 // Enter outside inputs repeats the last command.
 CF.lastCommand='CIRCLE';press('Enter');assert.equal(tool,'circle');document.onkeydown({key:'Escape',target:body,preventDefault(){}});
 // Dialog guard: nothing happens while a dialog is open.
 const o2=$('ortho').checked;inputDialog.open=true;press('F8');press('Escape');assert.equal($('ortho').checked,o2);doc.entities=[T.line(0,0,1,0)];CF.select([0]);press('Delete');assert.equal(doc.entities.length,1);inputDialog.open=false;
 press('Delete');assert.equal(doc.entities.length,0);
 `);
 console.log('PASS: keyboard map (function keys, Ctrl combos, Delete, typing, dialog guard)');

 // ---- Misc commands --------------------------------------------------------------------------------------------------
 await run(`
 T.reset();
 await T.sub('PROPERTIES');assert.equal(CF.get('properties'),true);await T.sub('PR');assert.equal(CF.get('properties'),false);await T.sub('PROPERTIES');await T.sub('PROPERTIESCLOSE');assert.equal(CF.get('properties'),false);
 await T.sub('CLEANSCREENON');assert.equal(CF.get('clean'),true);await T.sub('CLEANSCREENOFF');assert.equal(CF.get('clean'),false);
 await T.sub('COMMANDLINEHIDE');assert.equal(CF.get('commandLine'),false);await T.sub('COMMANDLINE');assert.equal(CF.get('commandLine'),true);
 await T.sub('SNAP');assert.equal(CF.get('snap'),true);await T.sub('SNAP');await T.sub('ORTHO');assert.equal(CF.get('ortho'),true);await T.sub('POLAR');assert.equal(CF.get('ortho'),false);assert.equal(CF.get('polar'),true);await T.sub('POLAR');
 await T.sub('TEXTSCR');assert.equal(CF.commandLine.state().expanded,true);await T.sub('TEXTSCR');assert.equal(CF.commandLine.state().expanded,false);
 await T.sub('MODEL');assert.equal(CF.space,'model');await T.sub('LAYOUT');assert.equal(CF.space,'layout');await T.sub('MODEL');
 await T.sub('RIBBONCLOSE');assert.equal(CF.get('ribbonMin'),true);await T.sub('RIBBON');assert.equal(CF.get('ribbonMin'),false);
 await T.sub('WSCURRENT');assert.ok(/Drafting\\/3dModeling/.test(CF.prompt()));await T.sub('M');assert.equal(CF.workspace,'3d');await T.sub('WSCURRENT');await T.sub('D');assert.equal(CF.workspace,'drafting');
 let vs=null;const ovs=CF.setVisualStyle;CF.setVisualStyle=n=>{vs=n};await T.sub('VS');assert.ok(/2dwireframe\\/Wireframe\\/Shaded\\/shadedEdges/.test(CF.prompt()));await T.sub('E');assert.equal(vs,'shadededges');await T.sub('VS');await T.sub('');assert.equal(vs,'2dwireframe');await T.sub('VS');await T.sub('shaded');assert.equal(vs,'shaded');CF.setVisualStyle=ovs;
 await T.sub('3DORBIT');assert.ok(mode3D);await T.sub('PLAN');assert.ok(!mode3D);
 // VIEW Save / Restore use the engine named views.
 view={x:12,y:3,scale:9};await T.sub('VIEW');assert.ok(/\\[Restore\\/Save\\]/.test(CF.prompt()));await T.sub('S');assert.equal(CF.input.message,'VIEW Enter view name to save <Sheet 1>:');await T.sub('');await T.tick();assert.ok(doc.layouts['Sheet 1']);
 view={x:0,y:0,scale:1};await T.sub('V');await T.sub('R');assert.ok(/^VIEW Enter view name to restore \\[Sheet 1\\] <Sheet 1>:$/.test(CF.input.message));await T.sub('');await T.tick();assert.equal(view.x,12);
 // EXPORT asks for a format; the ribbon passes the option as an argument.
 const clicks=[];for(const id of ['dxf','svg']){const b=$(id);b.__o=b.onclick;b.onclick=()=>clicks.push(id)}
 await T.sub('EXPORT');assert.ok(/\\[Dxf\\/Svg\\/Obj\\/Stl\\/Pdf\\]/.test(CF.prompt()));await T.sub('Dxf');CF.run('EXPORT',{args:'Svg'});await T.tick();T.eq(clicks,['dxf','svg']);for(const id of ['dxf','svg'])$(id).onclick=$(id).__o;
 // Options that do not exist are rejected without leaving a stuck prompt.
 await T.sub('EXPORT');await T.sub('Q');assert.equal(CF.input,null);assert.ok(T.has('Invalid option'));
 // Console noise: engine tool hints are not echoed in the history.
 T.reset();setTool('line');setTool('circle');setTool('select');assert.ok(!T.lines().some(l=>/Specify \\w+ points\\./.test(l)));
 // Commands run from other sources log the command name.
 T.reset();CF.run('GRID',{source:'ribbon'});assert.ok(T.has('Command: GRID'));CF.run('GRID',{source:'ribbon'});
 // Setting the prompt text explicitly.
 CF.commandLine.setPrompt('Custom [One/Two]:');T.eq(T.opts().map(o=>o.textContent),['One','Two']);T.reset();
 `);
 console.log('PASS: settings, view, export and workspace commands');

 // ---- MOVE / COPY displacement ---------------------------------------------------------------------------------------------
 await run(`
 const circ=()=>({type:'circle',layer:'0',center:{x:0,y:0},radius:2});
 // A typed base point is the displacement when Enter is pressed at the second-point prompt.
 T.reset();doc.entities=[circ()];CF.select([0]);await T.sub('M');assert.equal(CF.prompt(),'MOVE Specify base point or [Displacement] <Displacement>:');
 await T.sub('10,5');assert.equal(CF.prompt(),'MOVE Specify second point or <use first point as displacement>:');await T.sub('');await T.tick();
 T.eq(doc.entities[0].center,{x:10,y:5});assert.equal(tool,'select');assert.equal(CF.prompt(),'Command:');assert.equal(selected,-1);assert.equal(doc.entities.length,1);
 undo();T.eq(doc.entities[0].center,{x:0,y:0});
 // COPY the same way: the original stays, one copy is made, the command ends.
 T.reset();doc.entities=[T.line(0,0,10,0)];CF.select([0]);await T.sub('CO');await T.sub('5,5');await T.sub('');await T.tick();
 assert.equal(doc.entities.length,2);T.eq(doc.entities[0].points,[{x:0,y:0},{x:10,y:0}]);T.eq(doc.entities[1].points,[{x:5,y:5},{x:15,y:5}]);assert.equal(tool,'select');
 // Several selected objects all move; a clicked base point works the same way.
 T.reset();doc.entities=[T.line(0,0,1,0),T.line(0,1,1,1),T.line(50,50,60,60)];CF.select([0,1],'add');await T.sub('M');await accept({x:3,y:-2});await T.tick();assert.equal(CF.input,null);CF.enter();await T.tick();
 T.eq(doc.entities[0].points[0],{x:3,y:-2});T.eq(doc.entities[1].points[0],{x:3,y:-1});T.eq(doc.entities[2].points[0],{x:50,y:50});
 // The [Displacement] option (typed, and as a clickable prompt option).
 T.reset();doc.entities=[circ()];CF.select([0]);await T.sub('M');assert.ok(T.opts().some(o=>o.textContent==='Displacement'),'Displacement is a clickable option');
 await T.sub('d');assert.ok(CF.input);assert.equal(CF.input.message,'MOVE Specify displacement <0,0,0>:');assert.equal(CF.input.kind,'point');await T.sub('@4,-3');await T.tick();T.eq(doc.entities[0].center,{x:4,y:-3});assert.equal(tool,'select');
 T.reset();doc.entities=[circ()];CF.select([0]);await T.sub('CO');T.opts().find(o=>o.textContent==='Displacement').onclick();await T.tick();assert.equal(CF.input.message,'COPY Specify displacement <0,0,0>:');await T.sub('0,7');await T.tick();
 assert.equal(doc.entities.length,2);T.eq(doc.entities[1].center,{x:0,y:7});assert.equal(tool,'select');
 // Enter at the base prompt chooses <Displacement>; bad input re-asks; Esc cancels without moving.
 T.reset();doc.entities=[circ()];CF.select([0]);await T.sub('M');await T.sub('');assert.equal(CF.input.message,'MOVE Specify displacement <0,0,0>:');await T.sub('banana');assert.ok(CF.input,'still asking');assert.ok(T.has('Point or option keyword required.'));
 document.onkeydown({key:'Escape',target:$('command'),preventDefault(){}});await T.tick();assert.equal(CF.input,null);assert.equal(tool,'select');T.eq(doc.entities[0].center,{x:0,y:0});
 // After the first copy Enter still exits (no extra displacement copy).
 T.reset();doc.entities=[T.line(0,0,10,0)];CF.select([0]);await T.sub('CO');await T.sub('0,0');await T.sub('@0,5');assert.equal(doc.entities.length,2);await T.sub('');await T.tick();assert.equal(doc.entities.length,2);assert.equal(tool,'select');
 `);
 console.log('PASS: MOVE/COPY treat the base point as a displacement on Enter, [Displacement] option');

 // ---- CIRCLE 3P / 2P / Ttr --------------------------------------------------------------------------------------------------
 await run(`
 const near=(a,b,msg)=>assert.ok(Math.abs(a-b)<1e-6,msg+': '+a+' vs '+b);
 T.reset();await T.sub('C');assert.equal(CF.prompt(),'CIRCLE Specify center point for circle or [3P/2P/Ttr (tan tan radius)]:');
 T.eq(T.opts().map(o=>o.textContent),['3P','2P','Ttr (tan tan radius)']);T.eq(T.opts().map(o=>o.title),['Enter 3P','Enter 2P','Enter T']);
 // 3P: circumscribed circle; relative coordinates continue from the previous point.
 await T.sub('3p');assert.equal(CF.prompt(),'CIRCLE Specify first point on circle:');assert.equal(tool,'circle');await T.sub('0,0');assert.equal(CF.prompt(),'CIRCLE Specify second point on circle:');
 assert.equal(CF.previews.circle({x:5,y:5}).length,1,'3P preview is a line after one point');await T.sub('@10,0');assert.equal(CF.prompt(),'CIRCLE Specify third point on circle:');
 const pv=CF.previews.circle({x:0,y:10});assert.equal(pv[0].type,'circle');near(pv[0].radius,Math.sqrt(50),'preview radius');await T.sub('0,10');
 assert.equal(doc.entities.length,1);let e=doc.entities[0];assert.equal(e.type,'circle');near(e.center.x,5,'cx');near(e.center.y,5,'cy');near(e.radius,Math.sqrt(50),'r');assert.equal(tool,'select');assert.equal(CF.prompt(),'Command:');
 // Clicking the prompt option starts the same flow; Enter at a point prompt cancels it.
 T.reset();await T.sub('C');T.opts()[0].onclick();await T.tick();assert.equal(CF.prompt(),'CIRCLE Specify first point on circle:');await T.sub('');assert.equal(tool,'select');assert.equal(doc.entities.length,0);
 // Collinear points have no circle.
 T.reset();await T.sub('C');await T.sub('3P');await T.sub('0,0');await T.sub('5,0');await T.sub('10,0');assert.equal(doc.entities.length,0);assert.ok(T.has('Circle does not exist.'));assert.equal(tool,'select');
 // 2P: diameter end points.
 T.reset();await T.sub('C');await T.sub('2P');assert.equal(CF.prompt(),"CIRCLE Specify first end point of circle's diameter:");await T.sub('0,0');assert.equal(CF.prompt(),"CIRCLE Specify second end point of circle's diameter:");
 assert.equal(CF.previews.circle({x:6,y:8})[0].radius,5);await T.sub('6,8');e=doc.entities[0];near(e.center.x,3,'2P cx');near(e.center.y,4,'2P cy');near(e.radius,5,'2P r');assert.equal(tool,'select');
 // A canvas click is a point answer, and Esc cancels the flow.
 T.reset();await T.sub('C');await T.sub('2p');await accept({x:0,y:0});await accept({x:10,y:0});await T.tick();assert.equal(doc.entities.length,1);near(doc.entities[0].radius,5,'click 2P');
 T.reset();await T.sub('C');await T.sub('3P');await T.sub('1,1');document.onkeydown({key:'Escape',target:$('command'),preventDefault(){}});await T.tick();assert.equal(tool,'select');assert.equal(CF.prompt(),'Command:');assert.equal(doc.entities.length,0);assert.equal(CF.previews.circle({x:5,y:5}).length,0);
 // Ordinary circles still work afterwards.
 await T.sub('C');await T.sub('0,0');assert.equal(CF.prompt().indexOf('Specify radius of circle or [Diameter]'),7);await T.sub('4');assert.equal(doc.entities[0].radius,4);
 // Pure helpers.
 const L=CF.commandLine;assert.equal(L.circle3P({x:0,y:0},{x:1,y:1},{x:2,y:2}),null);assert.equal(L.circle2P({x:1,y:1},{x:1,y:1}),null);near(L.circle3P({x:-5,y:0},{x:5,y:0},{x:0,y:5}).radius,5,'3P radius');
 assert.equal(L.optKey('3P'),'3P');assert.equal(L.optKey('Ttr (tan tan radius)'),'T');assert.equal(L.optKey('Displacement'),'D');assert.equal(L.optKey('3dModeling'),'M');assert.equal(L.optKey('shadedEdges'),'E');
 // Ttr: two lines, a line and a circle, two circles.
 const X={type:'line',layer:'0',points:[{x:-50,y:0},{x:50,y:0}]},Y={type:'line',layer:'0',points:[{x:0,y:-50},{x:0,y:50}]};
 let r=L.circleTTR(X,{x:10,y:0},Y,{x:0,y:10},5);near(r.center.x,5,'TTR x');near(r.center.y,5,'TTR y');r=L.circleTTR(X,{x:-10,y:0},Y,{x:0,y:-10},5);near(r.center.x,-5,'TTR -x');near(r.center.y,-5,'TTR -y');
 const C0={type:'circle',layer:'0',center:{x:0,y:10},radius:5};r=L.circleTTR(X,{x:10,y:0},C0,{x:4,y:10},3);near(r.center.y,3,'line-circle y');near(r.center.x,Math.sqrt(15),'line-circle x');r=L.circleTTR(X,{x:-10,y:0},C0,{x:-4,y:10},3);near(r.center.x,-Math.sqrt(15),'line-circle left');
 const A={type:'circle',layer:'0',center:{x:0,y:0},radius:5},B={type:'circle',layer:'0',center:{x:20,y:0},radius:5};r=L.circleTTR(A,{x:5,y:0},B,{x:15,y:0},5);near(r.center.x,10,'circle-circle x');near(r.center.y,0,'circle-circle y');
 assert.equal(L.circleTTR(X,{x:1,y:0},{type:'line',layer:'0',points:[{x:0,y:3},{x:10,y:3}]},{x:1,y:3},9),null,'parallel lines 3 apart need r=1.5');
 // Ttr through the command line: pick two objects, then the radius.
 T.reset();doc.entities=[X,Y];await T.sub('C');await T.sub('T');assert.equal(CF.prompt(),'CIRCLE Specify point on object for first tangent of circle:');assert.ok(CF.isPick(),'tangent picks are raw object picks');
 await T.sub('10,0');assert.equal(CF.prompt(),'CIRCLE Specify point on object for second tangent of circle:');await T.sub('30,30');assert.ok(T.has('Select a line, polyline or circle.'),'empty space is rejected');await T.sub('0,10');
 assert.ok(CF.input&&CF.input.message.indexOf('CIRCLE Specify radius of circle')===0);assert.ok(!CF.isPick());await T.sub('5');await T.tick();assert.equal(doc.entities.length,3);e=doc.entities[2];near(e.center.x,5,'cmd TTR x');near(e.center.y,5,'cmd TTR y');near(e.radius,5,'cmd TTR r');assert.equal(tool,'select');
 // No tangent circle: radius too small for two parallel lines.
 T.reset();doc.entities=[{type:'line',layer:'0',points:[{x:-50,y:0},{x:50,y:0}]},{type:'line',layer:'0',points:[{x:-50,y:10},{x:50,y:10}]}];await T.sub('C');await T.sub('TTR');await T.sub('0,0');await T.sub('0,10');await T.sub('2');await T.tick();assert.equal(doc.entities.length,2);assert.ok(T.has('Circle does not exist.'));
 T.reset();
 `);
 console.log('PASS: CIRCLE 3P, 2P and Ttr (prompt options, flows, geometry, cancel)');

 // ---- Select objects: Window / Crossing / polygon / fence keywords --------------------------------------------------------------
 await run(`
 const mk=()=>{doc.entities=[T.line(1,1,5,1),T.line(2,8,20,8),T.line(30,30,40,30),{type:'circle',layer:'0',center:{x:5,y:5},radius:2},{type:'circle',layer:'0',center:{x:10,y:10},radius:3},{type:'text',layer:'0',points:[{x:2,y:20}],text:'Hi',height:2}]};
 const sel=()=>chosen().slice().sort((a,b)=>a-b);
 // Window: fully enclosed objects only.
 T.reset();mk();await T.sub('E');await T.sub('W');assert.equal(CF.prompt(),'ERASE Specify first corner:');assert.equal(CF.input.kind,'point');assert.ok(CF.picking,'still in the selection phase');
 await T.sub('0,0');assert.equal(CF.prompt(),'ERASE Specify opposite corner:');assert.equal(CF.previews.select({x:9,y:9}).length,1,'rubber-band rectangle');await T.sub('10,10');T.eq(sel(),[0,3]);assert.ok(T.has('2 found'));assert.equal(CF.prompt(),'ERASE Select objects:');
 // Crossing adds what touches the rectangle; the total is reported.
 await T.sub('c');await T.sub('0,0');await T.sub('10,10');T.eq(sel(),[0,1,3,4]);assert.ok(T.lines().some(l=>l==='4 found'),T.lines().slice(-4).join('|'));
 await T.sub('');await T.tick();assert.equal(doc.entities.length,2);assert.equal(tool,'select');
 // Right-to-left BOX is a crossing, left-to-right a window.
 T.reset();mk();await T.sub('E');await T.sub('BOX');await T.sub('0,0');await T.sub('10,10');T.eq(sel(),[0,3]);await T.sub('');
 T.reset();mk();await T.sub('E');await T.sub('BOX');await T.sub('10,10');await T.sub('0,0');T.eq(sel(),[0,1,3,4]);await T.sub('');
 // Relative opposite corner and the full keyword names.
 T.reset();mk();await T.sub('E');await T.sub('window');await T.sub('0,0');await T.sub('@10,10');T.eq(sel(),[0,3]);CF.cancel();
 // WPolygon / CPolygon with Undo, Enter to close.
 T.reset();mk();await T.sub('E');await T.sub('WP');assert.equal(CF.prompt(),'ERASE First polygon point:');await T.sub('0,0');assert.equal(CF.prompt(),'ERASE Specify endpoint of line or [Undo]:');await T.sub('14,0');await T.sub('99,99');await T.sub('U');await T.sub('0,14');
 assert.equal(CF.previews.select({x:1,y:1})[0].points.length,4);await T.sub('');T.eq(sel(),[0,3],'WP selects only fully inside');
 T.reset();mk();await T.sub('E');await T.sub('CP');await T.sub('0,0');await T.sub('14,0');await T.sub('0,14');await T.sub('');T.eq(sel(),[0,1,3],'CP also takes the line it crosses');
 T.reset();mk();await T.sub('E');await T.sub('CP');await T.sub('0,0');await T.sub('1,1');await T.sub('');assert.equal(chosen().length,0);assert.ok(T.has('A polygon needs at least three points.'));
 // Fence selects what its path crosses.
 T.reset();mk();await T.sub('E');await T.sub('F');assert.equal(CF.prompt(),'ERASE First fence point:');await T.sub('3,-5');await T.sub('3,3');await T.sub('');T.eq(sel(),[0],'fence crosses only the first line');
 T.reset();mk();await T.sub('E');await T.sub('F');await T.sub('6,0');await T.sub('6,12');await T.sub('');T.eq(sel(),[1,3],'a vertical fence crosses the long line and the first circle');
 // Objects already selected make the total differ from the found count.
 T.reset();mk();await T.sub('E');await T.sub('3,1');T.eq(sel(),[0]);await T.sub('F');await T.sub('6,0');await T.sub('6,12');await T.sub('');T.eq(sel(),[0,1,3]);assert.ok(T.lines().some(l=>l==='2 found, 3 total'),T.lines().slice(-4).join('|'));
 // Other commands share the phase: window select, then the base point prompt.
 T.reset();mk();await T.sub('M');await T.sub('W');await T.sub('0,0');await T.sub('10,10');await T.sub('');assert.equal(tool,'move');assert.equal(CF.prompt(),'MOVE Specify base point or [Displacement] <Displacement>:');CF.cancel();
 // Invalid keywords are reported with the real option list; single picks echo "1 found"; Esc inside a keyword cancels the command.
 T.reset();mk();await T.sub('E');await T.sub('zzz');assert.ok(T.has('Invalid selection. Expects a point or Window/Crossing'));await T.sub('1,1');assert.ok(T.lines().some(l=>l==='1 found'),T.lines().slice(-3).join('|'));
 await T.sub('W');document.onkeydown({key:'Escape',target:$('command'),preventDefault(){}});await T.tick();assert.equal(CF.picking,null);assert.equal(CF.input,null);assert.equal(tool,'select');assert.equal(CF.previews.select({x:1,y:1}).length,0);
 // Pure geometry: groups are not split by a window, hidden layers are skipped, text boxes count.
 T.reset();doc.entities=[{...T.line(0,0,4,0),group:'g'},{...T.line(0,5,40,5),group:'g'},T.line(2,2,3,3),{...T.line(1,1,2,2),layer:'Hidden'}];doc.layers.push({name:'Hidden',color:'#fff',visible:false});
 T.eq(CF.commandLine.selectByShape('window',[{x:-1,y:-1},{x:10,y:-1},{x:10,y:10},{x:-1,y:10}]),[2],'partially enclosed group stays out of a window');T.eq(CF.commandLine.selectByShape('crossing',[{x:-1,y:-1},{x:10,y:-1},{x:10,y:10},{x:-1,y:10}]),[0,1,2]);
 T.reset();
 `);
 console.log('PASS: select-objects keywords W, C, BOX, WP, CP and F with N found echo');

 // ---- Layout tab cancels the running command ----------------------------------------------------------------------------------
 await run(`
 T.reset();await T.sub('L');assert.equal(tool,'line');assert.equal(CF.prompt(),'LINE Specify first point:');CF.setSpace('layout');
 assert.equal(tool,'select','the running command is cancelled');assert.equal(CF.prompt(),'Command:');assert.ok(T.has('*Cancel*'));
 await T.sub('GRID');assert.equal(CF.get('grid'),false,'typed words are commands in paper space, not LINE points');await T.sub('GRID');assert.equal(CF.get('grid'),true);CF.setSpace('model');
 // Picking phase and pending prompts are cancelled as well.
 T.reset();doc.entities=[T.line(0,0,1,0)];await T.sub('M');assert.ok(CF.picking);CF.setSpace('layout');assert.equal(CF.picking,null);assert.equal(tool,'select');CF.setSpace('model');
 T.reset();const pr=cadPrompt('Layer name');assert.ok(CF.input);CF.setSpace('layout');assert.equal(await pr,null);assert.equal(CF.input,null);CF.setSpace('model');
 // An idle switch prints nothing and starting a drawing tool still returns to model space.
 T.reset();const n0=CF.history.length;CF.setSpace('layout');assert.equal(CF.history.length,n0,'nothing to cancel');await T.sub('L');assert.equal(CF.space,'model');assert.equal(tool,'line');CF.cancel();T.reset();
 `);
 console.log('PASS: switching to the Layout tab cancels the running command');

 // ---- UNDO / REDO echo is the same from every route -----------------------------------------------------------------------------
 await run(`
 const add2=()=>{mutate(()=>doc.entities.push(T.line(0,0,5,5)));mutate(()=>doc.entities.push(T.line(1,1,6,6)))};
 const last=(from)=>T.lines().slice(from);const kz=(k,extra)=>document.onkeydown({key:k,ctrlKey:true,target:document.body,preventDefault(){},...extra});
 T.reset();add2();let n=CF.history.length;kz('z');const viaKey=last(n);n=CF.history.length;CF.run('REDO',{source:'ribbon'});const redoRibbon=last(n);
 n=CF.history.length;CF.run('UNDO',{source:'qat'});const viaQat=last(n);n=CF.history.length;kz('y');const redoKey=last(n);
 T.eq(viaKey,['Command: UNDO']);T.eq(viaQat,['Command: UNDO']);T.eq(redoRibbon,['Command: REDO']);T.eq(redoKey,['Command: REDO']);assert.equal(doc.entities.length,2);
 // Typed U / UNDO / REDO echo the typed word like every other command and do the work.
 n=CF.history.length;await T.sub('UNDO');T.eq(last(n),['Command: UNDO']);assert.equal(doc.entities.length,1);n=CF.history.length;await T.sub('REDO');T.eq(last(n),['Command: REDO']);assert.equal(doc.entities.length,2);
 // Nothing to undo / redo is reported, not silent.
 T.reset();n=CF.history.length;kz('z');T.eq(last(n),['Command: UNDO','Nothing to undo.']);n=CF.history.length;CF.run('REDO',{source:'ribbon'});T.eq(last(n),['Command: REDO','Nothing to redo.']);
 // Inside LINE the key undoes the last segment and echoes it like typing U.
 T.reset();await T.sub('L');await T.sub('0,0');await T.sub('5,0');await T.sub('5,5');n=CF.history.length;kz('z');T.eq(last(n),['LINE Specify next point or [Close/Undo]: U']);assert.equal(doc.entities.length,1);CF.cancel();T.reset();
 `);
 console.log('PASS: UNDO/REDO echo is consistent for Ctrl+Z/Y, ribbon/QAT commands and typed commands');

 // ---- BLOCK base point and INSERT scale/rotation ------------------------------------------------------------------------------------
 await run(`
 T.reset();doc.entities=[T.line(10,10,20,10)];CF.select([0]);await T.sub('B');await T.sub('Q');assert.equal(CF.input.message,'BLOCK Specify insertion base point <0,0,0>:');await T.sub('10,10');await T.tick();
 T.eq(doc.blocks.Q.items[0].points,[{x:0,y:0},{x:10,y:0}],'the base point becomes the block origin');assert.equal(selected,-1);assert.equal(chosen().length,0,'BLOCK releases the selection');assert.equal(tool,'select');assert.ok(T.has('Block "Q" defined.'));assert.ok(!T.lines().some(l=>/Click Insert block/.test(l)));
 // Default base point is 0,0,0; an existing name is refused; Esc at the base prompt defines nothing.
 T.reset();doc.entities=[T.line(10,10,20,10)];CF.select([0]);await T.sub('B');await T.sub('R');await T.sub('');await T.tick();T.eq(doc.blocks.R.items[0].points,[{x:10,y:10},{x:20,y:10}]);
 doc.entities=[T.line(10,10,20,10)];CF.select([0]);await T.sub('B');await T.sub('R');await T.tick();assert.ok(T.has('A block with that name already exists.'));assert.equal(CF.input,null);
 CF.select([0]);await T.sub('B');await T.sub('Z');document.onkeydown({key:'Escape',target:$('command'),preventDefault(){}});await T.tick();assert.ok(!doc.blocks.Z);
 // INSERT: scale and rotation options, accurate wording, the command ends after the point.
 T.reset();doc.entities=[];doc.blocks={Q:{items:[T.line(0,0,10,0)]}};await T.sub('I');await T.sub('Q');await T.tick();assert.equal(CF.prompt(),'INSERT Specify insertion point or [Scale/Rotate]:');T.eq(T.opts().map(o=>o.textContent),['Scale','Rotate']);
 await T.sub('S');assert.equal(CF.input.message,'INSERT Specify scale factor <1>:');await T.sub('2');await T.tick();assert.equal(tool,'insert');await T.sub('R');assert.equal(CF.input.message,'INSERT Specify rotation angle <0>:');await T.sub('90');await T.tick();assert.equal(tool,'insert');
 await T.sub('100,0');await T.tick();assert.equal(doc.entities.length,1);const p=doc.entities[0].points;assert.ok(Math.abs(p[0].x-100)<1e-9&&Math.abs(p[0].y)<1e-9&&Math.abs(p[1].x-100)<1e-9&&Math.abs(p[1].y-20)<1e-9,JSON.stringify(p));assert.equal(doc.entities[0].blockName,'Q');
 assert.equal(tool,'select','INSERT ends after the insertion point');assert.ok(T.has('Inserted Q.'));assert.ok(!T.lines().some(l=>/Escape ends insertion/.test(l)));undo();assert.equal(doc.entities.length,0,'one undo step');
 // A new INSERT starts again at scale 1 / rotation 0; the unscaled path uses the engine insert.
 await T.sub('I');await T.sub('Q');await T.tick();await T.sub('10,10');await T.tick();T.eq(doc.entities[0].points,[{x:10,y:10},{x:20,y:10}]);
 T.reset();
 `);
 console.log('PASS: BLOCK asks for the base point and releases the selection; INSERT Scale/Rotate and wording');

 // ---- Real canvas clicks through acad-interact (only when that module is bundled) -------------------------------------------
 if(await run("return CF.has('interact')")){
 await run(`
 const click=(x,y,extra={})=>{const s=screen({x,y});canvas.onpointerdown({button:0,offsetX:s.x,offsetY:s.y,clientX:s.x,clientY:s.y,pointerId:1,shiftKey:false,...extra});return T.tick()};
 const near=(a,b,msg)=>assert.ok(Math.abs(a-b)<1e-6,msg+': '+a+' vs '+b);
 // 3P by clicking three points.
 T.reset();await T.sub('C');await T.sub('3P');await click(0,0);await click(10,0);await click(0,10);assert.equal(doc.entities.length,1);near(doc.entities[0].center.x,5,'3P click x');near(doc.entities[0].center.y,5,'3P click y');assert.equal(tool,'select');
 // Ttr: clicks pick the objects (raw points), then the radius is typed.
 T.reset();doc.entities=[{type:'line',layer:'0',points:[{x:-50,y:0},{x:50,y:0}]},{type:'line',layer:'0',points:[{x:0,y:-50},{x:0,y:50}]}];
 await T.sub('C');await T.sub('T');await click(10,0.2);await click(0.2,10);assert.ok(CF.input&&/Specify radius of circle/.test(CF.input.message));await T.sub('5');await T.tick();
 assert.equal(doc.entities.length,3);near(doc.entities[2].center.x,5,'Ttr click x');near(doc.entities[2].center.y,5,'Ttr click y');
 // Window keyword corners by clicking; the right button ends the selection like Enter.
 T.reset();doc.entities=[T.line(1,1,5,1),T.line(8,8,20,8)];await T.sub('E');await T.sub('W');await click(0,0);await click(10,10);T.eq(chosen(),[0]);assert.equal(CF.prompt(),'ERASE Select objects:');
 // MOVE: a clicked base point, then Enter, uses that point as the displacement.
 T.reset();doc.entities=[{type:'circle',layer:'0',center:{x:0,y:0},radius:2}];CF.select([0]);await T.sub('M');await click(12,-4);assert.equal(CF.prompt(),'MOVE Specify second point or <use first point as displacement>:');CF.enter();await T.tick();
 assert.equal(doc.entities[0].center.x,12);assert.equal(doc.entities[0].center.y,-4);assert.equal(tool,'select');
 T.reset();
 `);
 console.log('PASS: CIRCLE 3P/Ttr, window keyword and MOVE displacement driven by real canvas clicks');
 }

 // ---- Compatibility with replaced cadPrompt ------------------------------------------------------------------------------
 await run(`
 T.reset();const savedPrompt=cadPrompt;
 cadPrompt=async(m)=>{ if(m==='Text')return 'Plain';return null};
 setTool('text');await accept({x:1,y:2});assert.equal(doc.entities.length,1);assert.equal(doc.entities[0].text,'Plain');assert.ok(doc.entities[0].height>0);assert.equal(tool,'select');
 // Restore the stub that Verify.cjs installed so later modules see what they expect.
 cadPrompt=globalThis.__stubPrompt;T.reset();
 `);
 console.log('PASS: engine flows still work with a replaced cadPrompt');
};
