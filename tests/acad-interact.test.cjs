// 2D interaction: object snaps, grid snap, ortho/polar, window/crossing selection, PICKADD, grips, shortcut menu, PAN mode.
module.exports=async({run,assert})=>{
 if(!await run("return typeof CF!=='undefined'&&CF.has('interact')")){console.log('SKIP: acad-interact');return}
 await run(`
 const I=CF.interact,near=(a,b,eps=1e-9)=>Math.abs(a-b)<eps;
 view={x:0,y:0,scale:4};W=1000;H=700;
 doc.entities=[{type:'line',layer:'0',points:[{x:0,y:0},{x:80,y:0}]},{type:'line',layer:'0',points:[{x:50,y:-50},{x:50,y:60}]},{type:'circle',layer:'0',center:{x:200,y:0},radius:20},
  {type:'line',layer:'0',hatch:true,group:'h1',points:[{x:-5,y:-5},{x:5,y:5}]}];
 const all=new Set(['endpoint','midpoint','center','intersection','quadrant']);
 // Aperture is 12px = 3 drawing units at scale 4.
 let c=I.osnapCandidates({x:1,y:1},{modes:all,base:null});assert.equal(c[0].type,'endpoint');assert.ok(near(c[0].x,0)&&near(c[0].y,0),'hatch lines are ignored');
 c=I.osnapCandidates({x:40.5,y:.8},{modes:all,base:null});assert.equal(c[0].type,'midpoint');assert.ok(near(c[0].x,40));
 c=I.osnapCandidates({x:51,y:1},{modes:all,base:null});assert.equal(c[0].type,'intersection');assert.ok(near(c[0].x,50)&&near(c[0].y,0));
 c=I.osnapCandidates({x:201,y:1},{modes:all,base:null});assert.equal(c[0].type,'center');
 c=I.osnapCandidates({x:186,y:13},{modes:new Set(['center']),base:null});assert.equal(c[0].type,'center','center snaps from the circle perimeter');
 c=I.osnapCandidates({x:220.5,y:.5},{modes:all,base:null});assert.equal(c[0].type,'quadrant');assert.ok(near(c[0].x,220)&&near(c[0].y,0));
 c=I.osnapCandidates({x:10,y:1},{modes:new Set(['perpendicular','endpoint']),base:{x:30,y:30}});assert.equal(c[0].type,'perpendicular');assert.ok(near(c[0].x,30)&&near(c[0].y,0));
 c=I.osnapCandidates({x:10,y:1},{modes:new Set(['nearest']),base:null});assert.equal(c[0].type,'nearest');assert.ok(near(c[0].x,10)&&near(c[0].y,0));
 assert.equal(I.osnapCandidates({x:120,y:40},{modes:all,base:null}).length,0);
 // Snap pipeline: osnap through the global snap(), grid snap, ortho and polar relative to the last point.
 setTool('line');points=[];CF.set('osnap',true,{quiet:true});let q=snap({x:79,y:1});assert.ok(near(q.x,80)&&near(q.y,0));assert.equal(I.state().mark.type,'endpoint');
 CF.osnapOverride='midpoint';q=snap({x:41,y:1});assert.ok(near(q.x,40));CF.osnapOverride=null;
 CF.set('osnap',false,{quiet:true});CF.set('snap',true,{quiet:true});const s=I.gridStep();assert.ok(s>0);q=snap({x:s*3.3,y:-s*1.6});assert.ok(near(q.x,s*3)&&near(q.y,-s*2));CF.set('snap',false,{quiet:true});
 points=[{x:0,y:0}];CF.set('ortho',true,{quiet:true});q=snap({x:10,y:3});assert.ok(near(q.y,0)&&near(q.x,10));q=snap({x:2,y:-9});assert.ok(near(q.x,0)&&near(q.y,-9));CF.set('ortho',false,{quiet:true});
 let r=I.polarProject({x:0,y:0},{x:10,y:9.5},45);assert.equal(r.angle,45);assert.ok(near(r.x,r.y)&&near(r.x,9.75));
 assert.equal(I.polarProject({x:0,y:0},{x:10,y:5},45),null);r=I.polarProject({x:1,y:1},{x:1.2,y:-20},30);assert.equal(r.angle,270);assert.ok(near(r.x,1));
 CF.polarIncrement=30;CF.set('polar',true,{quiet:true});q=snap({x:10,y:6});assert.ok(near(q.y/q.x,Math.tan(Math.PI/6),1e-9));assert.equal(I.state().track.angle,30);CF.set('polar',false,{quiet:true});delete CF.polarIncrement;
 setTool('select');q=snap({x:79.3,y:1.1});assert.ok(near(q.x,79.3),'pick tools use raw points');
 // Window vs crossing selection.
 doc.entities=[{type:'line',layer:'0',points:[{x:0,y:0},{x:10,y:0}]},{type:'line',layer:'0',points:[{x:5,y:-5},{x:30,y:-5}]},{type:'circle',layer:'0',center:{x:50,y:50},radius:5},
  {type:'polyline',layer:'0',closed:true,points:[{x:100,y:0},{x:110,y:0},{x:110,y:10},{x:100,y:10}]},{type:'text',layer:'0',points:[{x:0,y:20}],text:'AB',height:2}];
 assert.deepEqual(I.windowSelect({x:-1,y:-1},{x:12,y:1},false),[0]);
 assert.deepEqual(I.windowSelect({x:8,y:-6},{x:20,y:1},true).sort(),[0,1]);
 assert.deepEqual(I.windowSelect({x:8,y:-6},{x:20,y:1},false),[]);
 assert.deepEqual(I.windowSelect({x:54,y:49},{x:60,y:51},true),[2],'crossing touches circle edge');
 assert.deepEqual(I.windowSelect({x:48,y:48},{x:52,y:52},true),[],'window inside circle does not touch it');
 assert.deepEqual(I.windowSelect({x:44,y:44},{x:56,y:56},false),[2]);
 assert.deepEqual(I.windowSelect({x:105,y:-1},{x:106,y:20},true),[3],'crossing through polyline edges');
 assert.deepEqual(I.windowSelect({x:-1,y:19},{x:3,y:23},false),[4]);
 doc.entities.push({type:'line',layer:'0',group:'gA',points:[{x:-50,y:0},{x:-40,y:0}]},{type:'line',layer:'0',group:'gA',points:[{x:-50,y:5},{x:-30,y:5}]});
 assert.deepEqual(I.windowSelect({x:-51,y:-1},{x:-35,y:6},false),[],'window needs the whole group');assert.deepEqual(I.windowSelect({x:-51,y:-1},{x:-35,y:6},true).sort(),[5,6]);
 // Pointer-driven implicit window (drag left to right) and crossing (click-move-click right to left).
 CF.deselect();setTool('select');const px=p=>({offsetX:screen(p).x,offsetY:screen(p).y,clientX:screen(p).x,clientY:screen(p).y,pointerId:1,button:0,shiftKey:false});
 canvas.onpointerdown(px({x:-2,y:-2}));assert.ok(I.state().win);canvas.onpointermove({...px({x:12,y:2}),buttons:1});canvas.onpointerup(px({x:12,y:2}));assert.equal(I.state().win,null);assert.deepEqual(CF.selection(),[0]);
 CF.deselect();canvas.onpointerdown(px({x:20,y:2}));canvas.onpointerup(px({x:20,y:2}));assert.ok(I.state().win,'click without drag keeps the window open');canvas.onpointermove(px({x:8,y:-7}));
 assert.equal(CF.prompt(),'Specify opposite corner:');canvas.onpointerdown(px({x:8,y:-7}));assert.deepEqual(CF.selection().sort(),[0,1]);
 canvas.onpointerdown({...px({x:-2,y:-2}),shiftKey:true});canvas.onpointerdown({...px({x:12,y:2}),shiftKey:true});assert.deepEqual(CF.selection(),[1],'shift+window removes');
 // PICKADD: clicks add, Shift+click removes.
 CF.deselect();assert.ok(I.pickAt({x:5,y:0}));assert.deepEqual(CF.selection(),[0]);I.pickAt({x:20,y:-5});assert.deepEqual(CF.selection().sort(),[0,1]);I.pickAt({x:5,y:0},true);assert.deepEqual(CF.selection(),[1]);assert.equal(I.pickAt({x:70,y:70}),false);
 // Grip editing: line endpoint, line midpoint (move) and circle quadrant (radius); one undo step each.
 CF.select([0,2],'replace');let grips=I.gripsFor(I.selectedIndices());assert.equal(grips.filter(g=>g.ei===0).length,3);assert.equal(grips.filter(g=>g.ei===2).length,5);
 const end=grips.find(g=>g.ei===0&&g.kind==='vertex'&&g.vi===1);assert.ok(I.applyGrip(end,{x:12,y:4}));assert.deepEqual(doc.entities[0].points[1],{x:12,y:4});
 undo();assert.deepEqual(doc.entities[0].points[1],{x:10,y:0});
 CF.select([0,2],'replace');grips=I.gripsFor(I.selectedIndices());const quad=grips.find(g=>g.ei===2&&g.kind==='radius');I.applyGrip(quad,{x:50,y:59});assert.ok(near(doc.entities[2].radius,9));
 const mid=grips.find(g=>g.ei===0&&g.kind==='move');I.applyGrip(mid,{x:5,y:3});assert.deepEqual(doc.entities[0].points,[{x:0,y:3},{x:10,y:3}]);
 // Hot grip through the pointer: click grip -> pending point input -> click the destination (snapped).
 CF.set('osnap',false,{quiet:true});CF.select([0],'replace');render();canvas.onpointerdown(px({x:0,y:3}));assert.ok(CF.input?.ixGrip,'grip is hot');assert.match(CF.prompt(),/STRETCH/);
 canvas.onpointerup(px({x:0,y:3}));canvas.onpointermove(px({x:-4,y:8}));canvas.onpointerdown(px({x:-4,y:8}));assert.equal(CF.input,null);assert.ok(near(doc.entities[0].points[0].x,-4)&&near(doc.entities[0].points[0].y,8));
 CF.select([0],'replace');const g2=I.gripsFor([0]).find(g=>g.kind==='vertex'&&g.vi===1);I.startGrip(g2);mouse={x:20,y:3};CF.input.resolve('@5,0');assert.deepEqual(doc.entities[0].points[1],{x:15,y:3});
 I.startGrip(g2);CF.input.resolve(null);assert.equal(CF.input,null);assert.equal(I.state().grip,null);
 assert.deepEqual(I.parsePoint('@3<90',{x:1,y:1}).x,1);assert.ok(near(I.parsePoint('@3<90',{x:1,y:1}).y,4));assert.deepEqual(I.parsePoint('7,8'),{x:7,y:8});
 // Pending distance input resolves from a click (contract), point input resolves with "x,y".
 setTool('select');CF.deselect();let got=null;CF.input={message:'Specify distance:',kind:'distance',base:{x:0,y:0},resolve:v=>{got=v}};canvas.onpointerdown(px({x:3,y:4}));assert.equal(Number(got),5);assert.equal(CF.input,null);
 CF.input={message:'Specify point:',kind:'point',resolve:v=>{got=v}};canvas.onpointerdown(px({x:3,y:4}));assert.equal(got,'3,4');
 CF.input={message:'Specify angle:',kind:'angle',base:{x:0,y:0},resolve:v=>{got=v}};canvas.onpointerdown(px({x:0,y:-4}));assert.equal(Number(got),270);
 // Shortcut menu model.
 CF.deselect();setTool('select');let labels=I.menuModel().map(i=>i.label);assert.ok(labels.includes('Undo')&&labels.includes('Zoom Extents')&&labels.includes('Select All')&&labels.some(l=>/^Repeat/.test(l)));
 CF.select([1]);labels=I.menuModel().map(i=>i.label);assert.ok(labels.includes('Erase')&&labels.includes('Deselect All')&&labels.includes('Mirror'));
 CF.input={message:'LINE Specify next point or [Close/Undo]:',kind:'point',resolve(){}};labels=I.menuModel().map(i=>i.label);assert.ok(labels.includes('Enter')&&labels.includes('Close')&&labels.includes('Snap Overrides'));CF.input=null;
 assert.deepEqual(I.promptOptions('Specify rotation angle or [Copy/Reference] <0>:').map(o=>o.key),['C','R']);assert.equal(I.shortPrompt('LINE Specify next point or [Close/Undo]:').text,'Specify next point or');
 assert.ok(CF.openContextMenu(10,10));
 // PAN mode, middle-drag pan and double-middle-click extents.
 CF.startPan();assert.ok(I.state().panMode);assert.match(CF.prompt(),/Press ESC or ENTER/);const vx=view.x;canvas.onpointerdown({...px({x:0,y:0}),clientX:100,clientY:100});canvas.onpointermove({offsetX:140,offsetY:100,clientX:140,clientY:100});canvas.onpointerup({offsetX:140,offsetY:100,clientX:140,clientY:100});assert.ok(near(view.x,vx-10));I.exitPan();assert.ok(!I.state().panMode);
 canvas.onpointerdown({offsetX:0,offsetY:0,clientX:0,clientY:0,button:1,pointerId:2});canvas.onpointermove({offsetX:-40,offsetY:0,clientX:-40,clientY:0});canvas.onpointerup({offsetX:-40,offsetY:0,clientX:-40,clientY:0,button:1});assert.ok(near(view.x,vx));
 canvas.onpointerdown({offsetX:0,offsetY:0,button:1,pointerId:2});canvas.onpointerup({offsetX:0,offsetY:0,button:1});canvas.onpointerdown({offsetX:0,offsetY:0,button:1,pointerId:2});canvas.onpointerup({offsetX:0,offsetY:0,button:1});assert.notEqual(view.scale,4,'double middle click zooms extents');
 // Rendering with grips, previews and a large hatch must not throw; chosen() keeps engine order.
 doc.entities=[{type:'polyline',layer:'0',closed:true,points:[{x:0,y:0},{x:50,y:0},{x:50,y:50},{x:0,y:50}]}];selected=0;selectionSet=new Set([0]);const cp=cadPrompt;cadPrompt=async()=>'0.25';await hatch();cadPrompt=cp;assert.ok(doc.entities.length>2);selectAll();
 const order=chosen();assert.equal(order.length,doc.entities.length);render();setTool('move');points=[{x:0,y:0}];mouse={x:5,y:5};render();setTool('select');CF.deselect();
 // Verify raw-select scenario.
 mode3D=false;clearSelection();doc.entities=[{type:'line',layer:'0',points:[{x:1.25,y:1.75},{x:3.25,y:1.75}]}];view={x:0,y:0,scale:100};W=1000;H=700;setTool('select');
 canvas.onpointerdown({button:0,offsetX:700,offsetY:175,pointerId:1,shiftKey:false});assert.equal(selected,0);I.resetTransient();CF.deselect();
 `);
 console.log('PASS: acad-interact osnap endpoint/midpoint/intersection/center/quadrant/perpendicular/nearest, grid snap, ortho, polar, window/crossing/group rules, implicit windows, PICKADD/shift-remove, grip stretch/move/radius and hot-grip input, click-resolved inputs, shortcut menu, PAN, middle-button pan/extents, raw select');
};
