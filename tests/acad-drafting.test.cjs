// B2 drafting commands: exact geometry of OFFSET/TRIM/EXTEND/FILLET/CHAMFER/POLYGON/ELLIPSE/DIMLINEAR and the command flows.
module.exports=async({run,assert,setAnswers})=>{
 if(!await run("return typeof CF!=='undefined'&&CF.has('drafting')")){console.log('SKIP: acad-drafting');return}
 await run(`globalThis.DT={
  P:(x,y)=>({x,y}),L:(x1,y1,x2,y2,layer='0')=>({type:'line',layer,points:[{x:x1,y:y1},{x:x2,y:y2}]}),
  near:(a,b,e=1e-6)=>Math.abs(a-b)<=e,
  pt:(p,x,y,e=1e-6)=>{assert.ok(Math.abs(p.x-x)<=e&&Math.abs(p.y-y)<=e,'expected ('+x+','+y+') got ('+p.x+','+p.y+')')},
  pts:(ps,exp,e=1e-6)=>{assert.equal(ps.length,exp.length,'point count');exp.forEach(([x,y],i)=>DT.pt(ps[i],x,y,e))},
  box:ps=>({minX:Math.min(...ps.map(p=>p.x)),maxX:Math.max(...ps.map(p=>p.x)),minY:Math.min(...ps.map(p=>p.y)),maxY:Math.max(...ps.map(p=>p.y))}),
  reset:(ents=[])=>{setTool('select');clearSelection();CF.input=null;CF.picking=null;mode3D=false;W=1000;H=700;view={x:0,y:0,scale:4};doc={layers:[{name:'0',color:'#63d9c0',visible:true}],entities:ents};history=[];future=[];$('layer').value='0';syncLayers()},
  idle:()=>CF.drafting.idle(),
  flags:{pline:CF.drafting.settings.acadPline,arc:CF.drafting.settings.acadArc},setFlags:(pline,arc)=>{CF.drafting.settings.acadPline=pline;CF.drafting.settings.acadArc=arc},restoreFlags:()=>DT.setFlags(DT.flags.pline,DT.flags.arc)};`);
 const go=async(code,answers=[])=>{setAnswers(answers);await run(code)};

 // ---- registry ----
 await go(`for(const [a,n]of [['O','OFFSET'],['TR','TRIM'],['EX','EXTEND'],['F','FILLET'],['CHA','CHAMFER'],['MI','MIRROR'],['POL','POLYGON'],['EL','ELLIPSE'],['DLI','DIMLINEAR'],['DIM','DIMLINEAR'],['DI','DIST'],['ID','ID'],['LI','LIST'],['MA','MATCHPROP'],['AR','ARRAYRECT'],['ARRAY','ARRAYRECT'],['PU','PURGE']])assert.equal(CF.resolve(a).name,n,a);
  assert.equal(typeof chamfer,'function');assert.equal(typeof extendAt,'function');assert.equal(typeof trimAt,'function');
  assert.deepEqual(CF.drafting.optionsOf('Select first object or [Undo/Polyline/Radius/Multiple]:').map(o=>o.key),['U','P','R','M']);
  assert.equal(CF.drafting.matchOption('la','[Blocks/LAyers/All]'),'LAyers');assert.equal(CF.drafting.matchOption('r','[cuTting edges/eRase]'),'eRase');assert.equal(CF.drafting.matchOption('q','[Yes/No]'),null);`);

 // ---- OFFSET geometry ----
 await go(`const {P,L}=DT,D=CF.drafting,rect={type:'polyline',layer:'0',closed:true,uuid:'r1',group:'g',points:[P(0,0),P(100,0),P(100,60),P(0,60)]};
  let o=D.offsetEntity(rect,5,P(-10,30));DT.pts(o.points,[[-5,-5],[105,-5],[105,65],[-5,65]]);assert.ok(DT.near(D.area(o.points),7700));assert.ok(o.closed);assert.equal(o.uuid,undefined);assert.equal(o.group,undefined);
  o=D.offsetEntity(rect,5,P(50,30));DT.pts(o.points,[[5,5],[95,5],[95,55],[5,55]]);assert.ok(DT.near(D.area(o.points),4500));
  assert.equal(D.offsetEntity(rect,30,P(50,30)),null,'inward offset past the centre collapses');
  o=D.offsetEntity(rect,null,P(50,-3));assert.ok(DT.near(D.area(o.points),106*66),'through point');
  const cw={...rect,points:[...rect.points].reverse()};o=D.offsetEntity(cw,5,P(-10,30));assert.ok(DT.near(Math.abs(D.area(o.points)),7700),'clockwise outline grows outward');
  DT.pts(D.offsetEntity(L(0,0,10,0),2,P(5,-1)).points,[[0,-2],[10,-2]]);DT.pts(D.offsetEntity(L(0,0,10,10),Math.SQRT2,P(0,10)).points,[[-1,1],[9,11]]);
  DT.pts(D.offsetEntity(L(0,0,10,0),null,P(3,7)).points,[[0,7],[10,7]]);
  const c={type:'circle',layer:'0',center:P(0,0),radius:5};assert.equal(D.offsetEntity(c,2,P(0,1)).radius,3);assert.equal(D.offsetEntity(c,2,P(0,9)).radius,7);assert.equal(D.offsetEntity(c,6,P(0,1)),null);
  const open={type:'polyline',layer:'0',closed:false,points:[P(0,0),P(10,0),P(10,10)]};DT.pts(D.offsetEntity(open,1,P(5,1)).points,[[0,1],[9,1],[9,10]]);DT.pts(D.offsetEntity(open,1,P(5,-1)).points,[[0,-1],[11,-1],[11,10]]);`);
 // OFFSET command flow: distance, pick, side, repeat, Enter; one undo step per copy.
 await go(`const {P}=DT;DT.reset([{type:'polyline',layer:'0',closed:true,points:[P(0,0),P(100,0),P(100,60),P(0,60)]}]);
  CF.run('OFFSET');await DT.idle();assert.equal(tool,'offset');assert.equal(CF.prompt(),'OFFSET Select object to offset or [Exit/Undo] <Exit>:');
  await accept(P(50,0));assert.ok(CF.prompt().includes('Specify point on side to offset'));assert.equal(CF.previews.offset(P(50,-10)).length,1);
  await accept(P(50,-10));assert.equal(doc.entities.length,2);assert.ok(DT.near(CF.drafting.area(doc.entities[1].points),7700));
  await accept(P(50,0));await accept(P(50,30));assert.equal(doc.entities.length,3);assert.ok(DT.near(CF.drafting.area(doc.entities[2].points),4500));
  CF.enter();await DT.idle();assert.equal(tool,'select');assert.equal(CF.drafting.state(),null);assert.equal(CF.drafting.settings.offsetDist,5);undo();assert.equal(doc.entities.length,2);`,['5']);
 await go(`const {P,L}=DT;DT.reset([L(0,0,10,0)]);CF.run('O');await DT.idle();await accept(P(5,0));await accept(P(4,3));DT.pts(doc.entities[1].points,[[0,5],[10,5]]);CF.enter();await DT.idle();
  CF.run('O');await DT.idle();await accept(P(5,0));await accept(P(5,-2.5));DT.pts(doc.entities[2].points,[[0,-2.5],[10,-2.5]]);assert.equal(CF.drafting.settings.offsetDist,null);CF.enter();await DT.idle()`,['','T']);

 // ---- TRIM ----
 await go(`const {P,L}=DT;DT.reset([L(0,0,100,0),L(30,-10,30,10),L(70,-10,70,10)]);
  CF.run('TRIM');await DT.idle();assert.ok(CF.isPick());assert.ok(CF.prompt().startsWith('TRIM Select object to trim or shift-select to extend'));
  const pv=CF.previews.trim(P(50,0));DT.pts(pv[0].points,[[30,0],[70,0]]);
  await accept(P(50,0));assert.equal(doc.entities.length,4);const h=doc.entities.filter(e=>e.points[0].y===0&&e.points[1].y===0);assert.equal(h.length,2);DT.pts(h[0].points,[[0,0],[30,0]]);DT.pts(h[1].points,[[70,0],[100,0]]);
  await accept(P(15,0));assert.equal(doc.entities.length,4,'cut only at its own endpoint: nothing trimmed');
  await accept(P(30,6));const v=doc.entities.find(e=>e.points[0].x===30&&e.points[1].x===30);DT.pts(v.points,[[30,-10],[30,0]]);
  CF.enter();await DT.idle();assert.equal(tool,'select');undo();assert.equal(doc.entities.find(e=>e.points[0].x===30&&e.points[1].x===30).points[1].y,10);undo();assert.equal(doc.entities.length,3);`);
 await go(`const {P,L}=DT;DT.reset([{type:'circle',layer:'0',center:P(0,0),radius:10},L(-20,0,20,0)]);CF.run('TR');await DT.idle();await accept(P(0,10));
  const a=doc.entities[0];assert.equal(a.type,'polyline');assert.ok(!a.closed);DT.pt(a.points[0],-10,0);DT.pt(a.points.at(-1),10,0);assert.ok(a.points.length>=33);
  for(const p of a.points){assert.ok(p.y<=1e-9);assert.ok(DT.near(Math.hypot(p.x,p.y),10))}assert.ok(a.points.some(p=>DT.near(p.y,-10)));
  DT.reset([{type:'circle',layer:'0',center:P(0,0),radius:10},L(-20,20,20,20)]);CF.run('TR');await DT.idle();await accept(P(0,10));assert.equal(doc.entities[0].type,'circle','no cutting edges');CF.enter();await DT.idle()`);
 await go(`const {P,L}=DT,D=CF.drafting;let pl={type:'polyline',layer:'0',closed:false,points:[P(0,0),P(50,0),P(50,50)]};
  let r=D.trimEntity(pl,P(50,20),[L(25,-5,25,5),L(45,40,55,40)]);assert.equal(r.add.length,2);DT.pts(r.add[0].points,[[0,0],[25,0]]);DT.pts(r.add[1].points,[[50,40],[50,50]]);DT.pts(r.removed,[[25,0],[50,0],[50,40]]);
  const sq={type:'polyline',layer:'0',closed:true,points:[P(0,0),P(10,0),P(10,10),P(0,10)]};r=D.trimEntity(sq,P(5,10),[L(-5,5,15,5)]);assert.equal(r.add.length,1);assert.equal(r.add[0].closed,false);DT.pts(r.add[0].points,[[0,5],[0,0],[10,0],[10,5]]);
  r=D.trimEntity(sq,P(5,0),[L(-5,5,15,5)]);DT.pts(r.add[0].points,[[10,5],[10,10],[0,10],[0,5]]);
  assert.ok(D.trimEntity(sq,P(5,0),[L(-5,5,5,5)]).error,'one cut cannot trim a closed polyline');
  r=D.trimEntity(L(0,0,10,0),P(2,0),[L(5,-1,5,1)]);DT.pts(r.add[0].points,[[5,0],[10,0]]);assert.equal(r.add.length,1);
  r=D.trimEntity(L(0,0,10,0),P(2,0),[{type:'circle',center:P(5,0),radius:2}]);DT.pts(r.add[0].points,[[3,0],[10,0]]);`);
 // Erase option and shift-select-to-extend inside TRIM.
 await go(`const {P,L}=DT;DT.reset([L(0,0,4,0),L(10,-10,10,10),L(20,-5,20,5)]);CF.run('TRIM');await DT.idle();shiftSelection=true;await accept(P(3,0));shiftSelection=false;DT.pts(doc.entities[0].points,[[0,0],[10,0]]);
  await CF.drafting.input('R');assert.ok(CF.prompt().includes('Select objects to erase'));await accept(P(20,0));assert.equal(doc.entities.length,2);CF.enter();await DT.idle();assert.ok(CF.prompt().includes('Select object to trim'));
  await CF.drafting.input('U');assert.equal(doc.entities.length,3);CF.enter();await DT.idle();assert.equal(tool,'select')`);

 // ---- EXTEND ----
 await go(`const {P,L}=DT;DT.reset([L(0,0,5,0),{type:'circle',layer:'0',center:P(20,0),radius:5}]);CF.run('EX');await DT.idle();DT.pts(CF.previews.extend(P(4,0))[0].points,[[5,0],[15,0]]);
  await accept(P(4,0));DT.pts(doc.entities[0].points,[[0,0],[15,0]]);await accept(P(14,0));DT.pts(doc.entities[0].points,[[0,0],[25,0]],1e-6);CF.enter();await DT.idle();
  DT.reset([L(0,0,4,0),L(10,-10,10,10),L(-6,-1,-6,1)]);CF.run('EXTEND');await DT.idle();await accept(P(1,0));DT.pts(doc.entities[0].points,[[-6,0],[4,0]]);await accept(P(3.5,0));DT.pts(doc.entities[0].points,[[-6,0],[10,0]]);CF.enter();await DT.idle();
  const D=CF.drafting,pl={type:'polyline',layer:'0',closed:false,points:[P(0,0),P(0,10),P(5,10)]};DT.pts(D.extendEntity(pl,P(4,10),[L(12,0,12,20)]).points,[[0,0],[0,10],[12,10]]);DT.pts(D.extendEntity(pl,P(0,1),[L(-5,-3,5,-3)]).points,[[0,-3],[0,10],[5,10]]);
  assert.ok(D.extendEntity(L(0,0,4,0),P(3,0),[L(10,5,10,10)]).error);`);

 // ---- FILLET ----
 await go(`const {P,L}=DT;CF.drafting.settings.filletRad=0;DT.reset([L(0,0,8,0),L(10,2,10,20)]);CF.run('FILLET');await DT.idle();assert.equal(CF.prompt(),'FILLET Select first object or [Undo/Polyline/Radius/Multiple]:');
  await accept(P(4,0));assert.ok(CF.prompt().startsWith('FILLET Select second object'));await accept(P(10,15));DT.pts(doc.entities[0].points,[[0,0],[10,0]]);DT.pts(doc.entities[1].points,[[10,0],[10,20]]);assert.equal(doc.entities.length,2);assert.equal(tool,'select');
  DT.reset([L(-10,0,10,0),L(0,-10,0,10)]);CF.run('F');await DT.idle();await accept(P(5,0));await accept(P(0,5));DT.pts(doc.entities[0].points,[[0,0],[10,0]]);DT.pts(doc.entities[1].points,[[0,0],[0,10]]);`);
 await go(`const {P,L}=DT;DT.reset([L(0,0,20,0),L(20,0,20,20)]);CF.run('FILLET');await DT.idle();CF.run('R');await DT.idle();assert.equal(CF.drafting.settings.filletRad,5);
  await accept(P(5,0));assert.equal(CF.previews.fillet(P(20,15)).length,3);await accept(P(20,15));DT.pts(doc.entities[0].points,[[0,0],[15,0]]);DT.pts(doc.entities[1].points,[[20,5],[20,20]]);
  const arc=doc.entities[2];assert.equal(arc.type,'polyline');assert.equal(arc.points.length,9);DT.pt(arc.points[0],15,0);DT.pt(arc.points.at(-1),20,5);for(const p of arc.points)assert.ok(DT.near(Math.hypot(p.x-15,p.y-5),5));
  undo();assert.equal(doc.entities.length,2);DT.pts(doc.entities[0].points,[[0,0],[20,0]]);`,['5']);
 // Integration with the command line (clickable options / macros call CF.commandLine.submit): the option reaches the waiting flow.
 await go(`if(!(CF.has('commands')&&CF.commandLine&&CF.commandLine.submit))return;const {P,L}=DT;DT.reset([L(0,0,20,0),L(20,0,20,20)]);CF.run('FILLET');await DT.idle();
  await CF.commandLine.submit('R');await DT.idle();assert.equal(CF.drafting.settings.filletRad,3);assert.equal(CF.drafting.state().prompt,'Select first object or [Undo/Polyline/Radius/Multiple]:');CF.cancel();`,['3']);
 await go(`const {P,L}=DT,D=CF.drafting;let r=D.filletLines([P(0,0),P(10,0)],P(5,0),[P(0,5),P(10,5)],P(5,5),2);assert.equal(r.error,'Lines are parallel.');
  r=D.filletLines([P(0,0),P(20,0)],P(5,0),[P(20,0),P(20,20)],P(20,15),50);assert.equal(r.error,'Radius is too large.');
  r=D.filletLines([P(0,0),P(20,0)],P(5,0),[P(20,0),P(10,10)],P(15,5),3);const t=3/Math.tan(Math.PI/8);DT.pt(r.line1[1],20-t,0);for(const p of r.arc)assert.ok(DT.near(Math.hypot(p.x-r.center.x,p.y-r.center.y),3));DT.pt(r.center,20-t,3);assert.ok(r.arc.length>=4);
  CF.drafting.settings.filletRad=5;const rect={type:'polyline',layer:'0',closed:true,points:[P(0,0),P(100,0),P(100,60),P(0,60)]};const fr=D.cornerPolyline(rect,{kind:'fillet',r:5});assert.equal(fr.count,4);assert.equal(fr.points.length,36);
  const b=DT.box(fr.points);assert.ok(DT.near(b.minX,0)&&DT.near(b.maxX,100)&&DT.near(b.minY,0)&&DT.near(b.maxY,60));assert.ok(Math.abs(Math.abs(D.area(fr.points))-(6000-(4-Math.PI)*25))<0.6);
  DT.reset([rect]);CF.run('F');await DT.idle();await CF.drafting.input('P');await accept(P(50,0));assert.equal(doc.entities[0].points.length,36);assert.ok(doc.entities[0].closed);assert.equal(tool,'select');
  DT.reset([{type:'polyline',layer:'0',closed:false,points:[P(0,0),P(20,0),P(20,20)]}]);CF.run('F');await DT.idle();await accept(P(5,0));await accept(P(20,15));const ps=doc.entities[0].points;assert.equal(ps.length,11);DT.pt(ps[1],15,0);DT.pt(ps[9],20,5);DT.pt(ps[10],20,20);`);

 // ---- CHAMFER ----
 await go(`const {P,L}=DT;DT.reset([L(3,0,20,0),L(0,4,0,20)]);CF.run('CHA');await DT.idle();assert.equal(CF.prompt(),'CHAMFER Select first line or [Undo/Polyline/Distance/Angle/Multiple]:');
  await CF.drafting.input('D');await accept(P(10,0));await accept(P(0,10));DT.pts(doc.entities[0].points,[[5,0],[20,0]]);DT.pts(doc.entities[1].points,[[0,3],[0,20]]);DT.pts(doc.entities[2].points,[[5,0],[0,3]]);
  assert.equal(CF.drafting.settings.chamferA,5);assert.equal(CF.drafting.settings.chamferB,3);undo();assert.equal(doc.entities.length,2);`,['5','3']);
 await go(`const {P,L}=DT;DT.reset([L(0,0,20,0),L(0,0,0,20)]);CF.run('CHAMFER');await DT.idle();await CF.drafting.input('A');await accept(P(10,0));await accept(P(0,10));
  DT.pts(doc.entities[0].points,[[4,0],[20,0]]);DT.pts(doc.entities[1].points,[[0,4],[0,20]]);DT.pts(doc.entities[2].points,[[4,0],[0,4]]);
  const r=CF.drafting.chamferLines([P(0,0),P(20,0)],P(10,0),[P(0,0),P(0,20)],P(0,10),3,0,30);DT.pt(r.chamfer[1],0,Math.sqrt(3));
  CF.drafting.settings.chamferMethod='distance';CF.drafting.settings.chamferA=2;CF.drafting.settings.chamferB=2;DT.reset([L(0,0,20,0),L(0,0,0,20)]);CF.run('CHA');await DT.idle();shiftSelection=false;await accept(P(10,0));shiftSelection=true;await accept(P(0,10));shiftSelection=false;
  assert.equal(doc.entities.length,2,'shift-select applies a sharp corner');assert.ok(CF.drafting.chamferLines([P(0,0),P(20,0)],P(10,0),[P(0,0),P(0,20)],P(0,10),30,1).error);`,['4','45']);

 // ---- MIRROR ----
 await go(`const {P,L}=DT;DT.reset([L(0,0,10,5),{type:'text',layer:'0',points:[P(2,1)],text:'AB',height:2}]);CF.select([0,1]);CF.run('MI');await DT.idle();assert.equal(CF.prompt(),'MIRROR Specify first point of mirror line:');
  await accept(P(0,0));assert.equal(CF.previews.mirror(P(0,10)).length,2);await accept(P(0,10));assert.equal(doc.entities.length,4);DT.pts(doc.entities[2].points,[[0,0],[-10,5]]);DT.pt(doc.entities[3].points[0],-4.4,1);assert.equal(doc.entities[3].text,'AB');assert.equal(tool,'select');`,['']);
 await go(`const {P,L}=DT;DT.reset([L(0,0,10,5)]);CF.run('MIRROR');await DT.idle();assert.ok(CF.picking);assert.equal(CF.prompt(),'MIRROR Select objects:');await accept(P(5,2.5));CF.enter();await DT.idle();
  await accept(P(0,0));await accept(P(10,0));assert.equal(doc.entities.length,1);DT.pts(doc.entities[0].points,[[0,0],[10,-5]]);undo();DT.pts(doc.entities[0].points,[[0,0],[10,5]]);`,['Y']);

 // ---- POLYGON / ELLIPSE ----
 await go(`const {P}=DT;DT.reset();CF.run('POL');await DT.idle();await accept(P(0,0));await accept(P(10,0));const e=doc.entities[0];assert.equal(e.type,'polyline');assert.ok(e.closed);assert.equal(e.points.length,6);
  DT.pt(e.points[0],10,0);for(const p of e.points)assert.ok(DT.near(Math.hypot(p.x,p.y),10));DT.pt(e.points[1],5,10*Math.sin(Math.PI/3));assert.equal(CF.drafting.settings.sides,6);assert.equal(e.layer,'0');`,['6','I']);
 await go(`const {P}=DT;DT.reset();CF.run('POLYGON');await DT.idle();await accept(P(0,0));await CF.drafting.input('10');DT.pts(doc.entities[0].points,[[-10,-10],[10,-10],[10,10],[-10,10]]);
  CF.run('POLYGON');await DT.idle();await accept(P(0,0));await accept(P(0,5));const e=doc.entities[1];for(const p of e.points)assert.ok(DT.near(Math.hypot(p.x,p.y),5*Math.SQRT2));
  CF.run('POLYGON');await DT.idle();await CF.drafting.input('E');await accept(P(0,0));await accept(P(10,0));DT.pts(doc.entities[2].points,[[0,0],[10,0],[10,10],[0,10]]);`,['4','C','','','4']);
 await go(`const {P}=DT;DT.reset([]);await go2();async function go2(){CF.run('EL');await DT.idle();await accept(P(-10,0));await accept(P(10,0));await accept(P(0,4));}
  let e=doc.entities[0],b=DT.box(e.points);assert.ok(e.closed);assert.equal(e.points.length,72);assert.ok(DT.near(b.minX,-10)&&DT.near(b.maxX,10)&&DT.near(b.minY,-4)&&DT.near(b.maxY,4));
  for(const p of e.points)assert.ok(DT.near((p.x/10)**2+(p.y/4)**2,1));
  CF.run('ELLIPSE');await DT.idle();await CF.drafting.input('C');await accept(P(5,5));await accept(P(5,15));await CF.drafting.input('3');e=doc.entities[1];b=DT.box(e.points);assert.ok(DT.near(b.minX,2)&&DT.near(b.maxX,8)&&DT.near(b.minY,-5)&&DT.near(b.maxY,15));
  CF.run('ELLIPSE');await DT.idle();await accept(P(0,0));await accept(P(20,0));await CF.drafting.input('Rotation');b=DT.box(doc.entities[2].points);assert.ok(DT.near(b.maxY,5)&&DT.near(b.minY,-5));`,['60']);

 // ---- DIMLINEAR ----
 await go(`const {P,L}=DT,D=CF.drafting;DT.reset();CF.run('DLI');await DT.idle();await accept(P(0,0));await accept(P(30,40));assert.ok(CF.previews.dimlinear(P(15,50)).length===8);await accept(P(15,50));
  let ents=doc.entities;assert.equal(ents.length,8);assert.ok(ents.every(e=>e.group&&e.group===ents[0].group&&e.layer==='0'));const t=ents.find(e=>e.type==='text');assert.equal(t.text,'30.00');assert.ok(t.points[0].y>50);
  const dl=ents.find(e=>e.type==='line'&&e.points[0].y===50&&e.points[1].y===50&&DT.near(Math.abs(e.points[1].x-e.points[0].x),30));assert.ok(dl,'horizontal dimension line at y=50');
  assert.equal(D.dimLinear(P(0,0),P(30,40),P(40,20)).text,'40.00');assert.equal(D.dimLinear(P(0,0),P(30,40),P(40,20)).orient,'v');assert.equal(D.dimLinear(P(0,0),P(30,40),P(15,-8)).orient,'h');assert.equal(D.dimLinear(P(0,0),P(30,0),P(15,0),'v'),null);
  CF.run('DIMLINEAR');await DT.idle();await accept(P(0,0));await accept(P(30,40));await CF.drafting.input('V');await accept(P(15,50));const t2=doc.entities.filter(e=>e.type==='text');assert.equal(t2[1].text,'40.00');assert.notEqual(doc.entities[8].group,doc.entities[0].group);
  DT.reset([L(0,0,25,0)]);CF.run('DIM');await DT.idle();CF.enter();await DT.idle();assert.ok(CF.prompt().includes('Select object to dimension'));await accept(P(10,0));await accept(P(10,-10));assert.equal(doc.entities.find(e=>e.type==='text').text,'25.00');
  DT.reset([{type:'circle',layer:'0',center:P(0,0),radius:4}]);CF.run('DIM');await DT.idle();CF.enter();await DT.idle();await accept(P(4,0));await accept(P(12,1));assert.equal(doc.entities.find(e=>e.type==='text').text,'8.00');`);

 // ---- DIST / ID / LIST ----
 await go(`const {P,L}=DT;DT.reset([L(0,0,10,0),{type:'circle',layer:'0',center:P(20,0),radius:5}]);CF.run('DI');await DT.idle();await accept(P(0,0));await accept(P(3,4));const r=CF.drafting.settings.lastReport;
  assert.equal(r[0],'Distance = 5.0000,  Angle in XY Plane = 53.1301°,  Angle from XY Plane = 0°');assert.equal(r[1],'Delta X = 3.0000,  Delta Y = 4.0000,  Delta Z = 0.0000');assert.equal(tool,'select');
  assert.ok(CF.drafting.distReport(P(0,0),P(0,-2))[0].includes('Angle in XY Plane = 270°'));
  CF.run('ID');await DT.idle();await accept(P(1.5,-2));assert.deepEqual(CF.drafting.settings.lastReport,['X = 1.5000     Y = -2.0000     Z = 0.0000']);
  CF.select([0]);CF.run('LIST');await DT.idle();let rep=CF.drafting.settings.lastReport.join('|');assert.ok(rep.includes('LINE  Layer: "0"'));assert.ok(rep.includes('Length = 10.0000'));assert.equal(selected,-1);
  CF.run('LI');await DT.idle();assert.ok(CF.picking);await accept(P(25,0));CF.enter();await DT.idle();rep=CF.drafting.settings.lastReport.join('|');assert.ok(rep.includes('CIRCLE'));assert.ok(rep.includes('radius 5.0000'));assert.ok(rep.includes('area 78.5398'));
  CF.run('LIST');await DT.idle();CF.cancel();await DT.idle();assert.equal(CF.picking,null);assert.equal(CF.drafting.state(),null);assert.equal(tool,'select');`);

 // ---- MATCHPROP ----
 await go(`const {P,L}=DT;DT.reset([L(0,0,10,0,'A'),L(0,10,10,10),{type:'text',layer:'A',points:[P(0,20)],text:'Src',height:7},{type:'text',layer:'0',points:[P(0,30)],text:'Dst',height:2}]);doc.layers.push({name:'A',color:'#ff0000',visible:true});
  CF.run('MA');await DT.idle();assert.equal(CF.prompt(),'MATCHPROP Select source object:');await accept(P(5,0));assert.ok(CF.prompt().includes('Select destination object(s) or [Settings]'));await accept(P(5,10));assert.equal(doc.entities[1].layer,'A');CF.enter();await DT.idle();
  CF.run('MATCHPROP');await DT.idle();await accept(P(1,21));await accept(P(1,31));assert.equal(doc.entities[3].height,7);assert.equal(doc.entities[3].layer,'A');CF.enter();await DT.idle();assert.equal(tool,'select');undo();assert.equal(doc.entities[3].height,2);`);

 // ---- ARRAYRECT ----
 await go(`const {P}=DT;DT.reset([{type:'circle',layer:'0',center:P(0,0),radius:1}]);CF.select([0]);CF.run('AR');await DT.idle();assert.equal(doc.entities.length,12);const xs=new Set(doc.entities.map(e=>e.center.x)),ys=new Set(doc.entities.map(e=>e.center.y));
  assert.deepEqual([...xs].sort((a,b)=>a-b),[0,3,6,9]);assert.deepEqual([...ys].sort((a,b)=>a-b),[0,3,6]);undo();assert.equal(doc.entities.length,1);`,['','','','']);
 await go(`const {P,L}=DT;DT.reset([{...L(0,0,2,0),group:'g1'},{...L(0,0,0,2),group:'g1'}]);CF.select([0]);CF.run('ARRAY');await DT.idle();assert.equal(doc.entities.length,12);
  assert.equal(new Set(doc.entities.map(e=>e.group)).size,6,'each copy is its own group');const c=doc.entities.filter(e=>e.points[0].x===40&&e.points[0].y===10);assert.equal(c.length,2);assert.equal(c[0].group,c[1].group);`,['2','3','10','20']);

 // ---- PURGE ----
 await go(`const {P,L}=DT;DT.reset([{...L(0,0,1,1),blockName:'Used',group:'g'}]);doc.layers.push(...['A','B','C','D'].map(name=>({name,color:'#ffffff',visible:true})));
  doc.blocks={Used:{items:[L(0,0,1,0,'B')]},Unused:{items:[L(0,0,1,0,'C')]}};syncLayers();$('layer').value='D';
  assert.deepEqual(CF.drafting.purgeAnalysis(),{blocks:['Unused'],layers:['A']});CF.run('PU');await new Promise(r=>setTimeout(r,0));
  assert.deepEqual(Object.keys(doc.blocks),['Used']);assert.deepEqual(doc.layers.map(l=>l.name),['0','B','D']);assert.ok(CF.drafting.settings.lastReport.includes('2 layers deleted.'));
  undo();assert.equal(doc.layers.length,5);CF.run('PURGE');await new Promise(r=>setTimeout(r,0));assert.deepEqual(Object.keys(doc.blocks),['Used']);assert.equal(doc.layers.length,5);
  DT.reset();CF.run('PURGE');await new Promise(r=>setTimeout(r,0));assert.deepEqual(CF.drafting.settings.lastReport,['No unreferenced blocks or layers found.']);`,['','B']);

 // ---- cancellation and tool hand-off ----
 await go(`const {P,L}=DT;DT.reset([L(0,0,10,0)]);CF.run('FILLET');await DT.idle();await accept(P(5,0));setTool('select');await DT.idle();assert.equal(CF.drafting.state(),null);assert.equal(selected,-1);
  CF.run('TRIM');await DT.idle();setTool('line');assert.equal(CF.drafting.state(),null);setTool('trim');await DT.idle();assert.equal(CF.drafting.state().tool,'trim');setTool('select');
  CF.run('POLYGON');await DT.idle();assert.equal(CF.drafting.state(),null,'Esc at the sides prompt cancels');assert.equal(tool,'select');`);
 // ---- PLINE keeps the segments drawn so far when ended with Esc (or when another command starts) ----
 await go(`const {P}=DT;DT.reset();DT.setFlags(false,false);setTool('polyline');await accept(P(0,0));await accept(P(10,0));await accept(P(10,10));
  if(CF.has('commands'))document.onkeydown({key:'Escape',target:$('command'),preventDefault(){}});else CF.cancel();
  assert.equal(doc.entities.length,1,'Esc keeps the polyline');const pl=doc.entities[0];assert.equal(pl.type,'polyline');assert.ok(!pl.closed);DT.pts(pl.points,[[0,0],[10,0],[10,10]]);assert.equal(tool,'select');assert.equal(points.length,0);
  undo();assert.equal(doc.entities.length,0,'one undo step');
  setTool('polyline');await accept(P(0,0));CF.cancel();assert.equal(doc.entities.length,0,'a lone start point leaves nothing');
  setTool('polyline');await accept(P(0,0));await accept(P(5,5));CF.cancel();assert.equal(doc.entities.length,1);DT.pts(doc.entities[0].points,[[0,0],[5,5]]);
  if(CF.resolve('CIRCLE')){setTool('polyline');await accept(P(1,1));await accept(P(2,1));await accept(P(2,2));CF.run('CIRCLE');assert.equal(doc.entities.length,2,'starting another command also keeps the polyline');assert.equal(doc.entities[1].points.length,3);CF.cancel()}DT.restoreFlags()`);
 await go(`if(!CF.has('commands')||!CF.resolve('GRID'))return;const {P}=DT;DT.reset();DT.setFlags(false,false);setTool('polyline');await accept(P(0,0));await accept(P(4,0));CF.run('GRID');CF.run('GRID');assert.equal(tool,'polyline','a settings toggle does not end PLINE');assert.equal(points.length,2);finish();assert.equal(doc.entities.length,1);DT.restoreFlags()`);

 // ---- PLINE (Arc/Length/Close/Undo) and ARC (3-point, Center) in AutoCAD prompt order; switched on via settings ----
 await go(`const {P}=DT,D=CF.drafting,PI=Math.PI;
  let a=D.tangentArc(P(0,0),P(0,1),P(10,10));assert.ok(a&&!a.straight);DT.pt(a.pts[0],0,0);DT.pt(a.pts.at(-1),10,10);for(const p of a.pts)assert.ok(DT.near(Math.hypot(p.x-10,p.y),10,1e-9),'quarter circle about (10,0)');DT.pt(a.tan,1,0,1e-9);DT.pt(a.center,10,0,1e-9);
  a=D.tangentArc(P(0,0),P(1,0),P(10,10));DT.pt(a.center,0,10,1e-9);DT.pt(a.tan,0,1,1e-9);a=D.tangentArc(P(0,0),P(1,0),P(10,-10));DT.pt(a.center,0,-10,1e-9);DT.pt(a.tan,0,-1,1e-9);
  a=D.tangentArc(P(0,0),P(1,0),P(5,0));assert.ok(a.straight);assert.equal(D.tangentArc(P(0,0),P(1,0),P(-5,0)),null,'directly behind the tangent');assert.equal(D.tangentArc(P(0,0),P(1,0),P(0,0)),null);
  a=D.tangentArc(P(0,0),P(1,0),P(-1,0.2));assert.ok(a&&Math.abs(a.radius)>0,'nearly a full turn is allowed');
  a=D.arcThrough(P(10,0),P(0,10),P(-10,0));DT.pt(a.center,0,0,1e-9);assert.ok(DT.near(a.sweep,PI,1e-9));DT.pt(a.pts[0],10,0);DT.pt(a.pts.at(-1),-10,0);assert.ok(a.pts.some(p=>DT.near(p.x,0,1e-9)&&DT.near(p.y,10,1e-9)),'passes through the second point');DT.pt(a.tan,0,-1,1e-9);
  a=D.arcThrough(P(10,0),P(0,-10),P(-10,0));assert.ok(DT.near(a.sweep,-PI,1e-9));assert.ok(a.pts.every(p=>p.y<=1e-9),'clockwise through the bottom');
  assert.equal(D.arcThrough(P(0,0),P(5,5),P(10,10)),null,'collinear');
  a=D.arcCenterStart(P(0,0),P(10,0),P(0,5));assert.ok(DT.near(a.sweep,PI/2,1e-9));DT.pt(a.pts.at(-1),0,10,1e-9);a=D.arcCenterStart(P(0,0),P(10,0),P(0,-5));assert.ok(DT.near(a.sweep,1.5*PI,1e-9),'counterclockwise all the way round');
  a=D.arcCenterStart(P(0,0),P(10,0),null,-PI/2);DT.pt(a.pts.at(-1),0,-10,1e-9);assert.equal(D.arcCenterStart(P(0,0),P(10,0),P(20,0)),null,'end on the start ray');assert.equal(D.arcCenterStart(P(0,0),P(0,0),P(5,5)),null)`);
 await go(`const {P}=DT,D=CF.drafting;DT.reset();DT.setFlags(true,false);
  D.start('polyline');await DT.idle();assert.equal(tool,'polyline');assert.equal(CF.prompt(),'PLINE Specify start point:');
  await accept(P(0,0));assert.equal(CF.prompt(),'PLINE Specify next point or [Arc/Length/Undo]:');await accept(P(10,0));assert.equal(CF.prompt(),'PLINE Specify next point or [Arc/Length/Undo]:','Close is not offered after one segment');
  await accept(P(10,10));assert.equal(CF.prompt(),'PLINE Specify next point or [Arc/Close/Length/Undo]:','Close appears after two segments');assert.equal(CF.previews.polyline(P(0,10))[0].points.length,4);
  await D.input('U');assert.equal(CF.prompt(),'PLINE Specify next point or [Arc/Length/Undo]:');assert.equal(CF.previews.polyline(P(0,10))[0].points.length,3);await D.input('U');await D.input('U');assert.equal(D.state().prompt,'Specify next point or [Arc/Length/Undo]:','extra undo is harmless');
  await accept(P(10,0));await accept(P(10,10));{const pr=D.input('close');assert.ok(pr);await pr}await DT.idle();assert.equal(doc.entities.length,1);assert.ok(doc.entities[0].closed);DT.pts(doc.entities[0].points,[[0,0],[10,0],[10,10]]);assert.equal(tool,'select');assert.equal(D.state(),null);undo();assert.equal(doc.entities.length,0);
  // Esc and Enter keep the segments; a lone start point leaves nothing
  D.start('polyline');await DT.idle();await accept(P(0,0));await accept(P(5,0));await accept(P(5,5));CF.cancel();assert.equal(doc.entities.length,1,'Esc keeps the polyline');assert.ok(!doc.entities[0].closed);DT.pts(doc.entities[0].points,[[0,0],[5,0],[5,5]]);assert.equal(D.state(),null);assert.equal(tool,'select');
  D.start('polyline');await DT.idle();await accept(P(0,0));CF.cancel();assert.equal(doc.entities.length,1);D.start('polyline');await DT.idle();await accept(P(1,1));await accept(P(2,2));CF.enter();await DT.idle();assert.equal(doc.entities.length,2);D.start('polyline');await DT.idle();await accept(P(7,7));await accept(P(8,8));D.start('offset');assert.equal(doc.entities.length,3,'starting another command keeps it too');CF.cancel();
  // Width is not supported: said so, command continues
  DT.reset();D.start('polyline');await DT.idle();await accept(P(0,0));{const w=D.input('W');assert.ok(w);await w}assert.equal(D.state().prompt,'Specify next point or [Arc/Length/Undo]:');CF.cancel();assert.equal(doc.entities.length,0);DT.restoreFlags()`);
 await go(`const {P}=DT,D=CF.drafting;DT.reset();DT.setFlags(true,false);
  // Arc mode: Direction, tangent continuation, back to Line; Undo removes a whole arc segment
  D.start('polyline');await DT.idle();await accept(P(0,0));await D.input('A');assert.equal(CF.prompt(),'PLINE Specify endpoint of arc or [Close/Direction/Line/Second pt/Undo]:');await D.input('D');assert.ok(CF.prompt().includes('tangent direction'));await D.input('90');assert.equal(D.state().prompt,'Specify endpoint of arc or [Close/Direction/Line/Second pt/Undo]:');
  await accept(P(10,10));assert.equal(CF.previews.polyline(P(20,0)).length,1);await accept(P(20,0));await D.input('L');await accept(P(30,0));await D.input('U');await D.input('U');await D.input('A');await accept(P(20,0));CF.enter();await DT.idle();
  const pl=doc.entities[0];assert.equal(pl.type,'polyline');DT.pt(pl.points[0],0,0);DT.pt(pl.points.at(-1),20,0);for(const p of pl.points)assert.ok(DT.near(Math.hypot(p.x-10,p.y),10,1e-9)&&p.y>=-1e-9,'the tangent continuation stays on the circle about (10,0): '+JSON.stringify(p));assert.ok(pl.points.some(p=>DT.near(p.x,10,1e-9)&&DT.near(p.y,10,1e-9)),'passes the first arc end point');
  // Second pt: three-point arc segment, then Close with an arc
  DT.reset();D.start('polyline');await DT.idle();await accept(P(10,0));await D.input('A');await D.input('S');assert.ok(CF.prompt().includes('Specify second point on arc'));await accept(P(0,10));assert.ok(CF.prompt().includes('Specify end point of arc'));await accept(P(-10,0));await DT.idle();
  assert.ok(CF.prompt().includes('Specify endpoint of arc'));await D.input('C');await DT.idle();let e=doc.entities[0];assert.ok(e.closed,'closed with an arc');DT.pt(e.points[0],10,0);for(const p of e.points)assert.ok(DT.near(Math.hypot(p.x,p.y),10,1e-9));assert.ok(e.points.some(p=>DT.near(p.y,-10,1e-6)||DT.near(p.y,10,1e-9)));assert.ok(Math.abs(CF.drafting.area(e.points))>300,'area '+CF.drafting.area(e.points));
  // Length repeats the previous direction
  DT.reset();D.start('polyline');await DT.idle();await accept(P(0,0));await D.input('L');assert.equal(D.state().prompt,'Specify next point or [Arc/Length/Undo]:','Length needs a first segment');await accept(P(0,5));await D.input('L');await DT.idle();CF.enter();await DT.idle();DT.pts(doc.entities[0].points,[[0,0],[0,5],[0,12]]);
  DT.restoreFlags()`,['7']);
 await go(`const {P}=DT,D=CF.drafting,PI=Math.PI;DT.reset();DT.setFlags(false,true);D.start('arc');await DT.idle();assert.equal(tool,'arc');assert.equal(CF.prompt(),'ARC Specify start point of arc or [Center]:');
  await accept(P(10,0));assert.equal(CF.prompt(),'ARC Specify second point of arc or [Center]:');await accept(P(0,10));assert.equal(CF.prompt(),'ARC Specify end point of arc:');assert.ok(CF.previews.arc(P(-10,0)).length===1);assert.equal(CF.previews.arc(P(-10,20)).length,0,'collinear: no preview');
  await accept(P(-10,0));let a=doc.entities[0];assert.equal(a.type,'polyline');assert.ok(!a.closed);DT.pt(a.points[0],10,0);DT.pt(a.points.at(-1),-10,0);for(const p of a.points)assert.ok(DT.near(Math.hypot(p.x,p.y),10,1e-9));assert.ok(a.points.every(p=>p.y>=-1e-9),'goes through (0,10)');assert.equal(tool,'select');assert.equal(D.state(),null);undo();assert.equal(doc.entities.length,0);
  // a collinear third point is rejected and asked again
  D.start('arc');await DT.idle();await accept(P(0,0));await accept(P(5,5));await accept(P(10,10));assert.equal(CF.prompt(),'ARC Specify end point of arc:');assert.equal(doc.entities.length,0);await accept(P(10,0));assert.equal(doc.entities.length,1);
  // [Center] first: center, start, end (counterclockwise) and the Angle option
  DT.reset();D.start('arc');await DT.idle();await D.input('C');assert.equal(CF.prompt(),'ARC Specify center point of arc:');await accept(P(0,0));assert.equal(CF.prompt(),'ARC Specify start point of arc:');await accept(P(10,0));assert.equal(CF.prompt(),'ARC Specify end point of arc or [Angle]:');await accept(P(0,3));
  a=doc.entities[0];DT.pt(a.points[0],10,0);DT.pt(a.points.at(-1),0,10,1e-9);for(const p of a.points)assert.ok(DT.near(Math.hypot(p.x,p.y),10,1e-9));assert.ok(a.points.length>=9);
  D.start('arc');await DT.idle();await D.input('C');await accept(P(0,0));await accept(P(0,10));await D.input('A');await DT.idle();a=doc.entities[1];assert.ok(a.points.every(p=>p.x>=-1e-9),'-90 degrees is clockwise from (0,10) to (10,0)');DT.pt(a.points.at(-1),10,0,1e-9);
  // Start, Center, End
  D.start('arc');await DT.idle();await accept(P(10,0));await D.input('C');assert.equal(CF.prompt(),'ARC Specify center point of arc:');await accept(P(0,0));await accept(P(-4,0));a=doc.entities[2];assert.ok(DT.near(D.area([...a.points,P(0,0)])*2,Math.PI*100,4),'half disc');DT.pt(a.points.at(-1),-10,0,1e-9);
  assert.equal(doc.entities.length,3);D.start('arc');await DT.idle();CF.cancel();assert.equal(D.state(),null);DT.restoreFlags()`,['-90']);
 await go(`if(!CF.has('commands'))return;const D=CF.drafting;DT.reset();DT.setFlags(false,false);
  CF.run('PL');await DT.idle();assert.equal(tool,'polyline');assert.equal(D.state(),null,'engine PLINE while the flag is off');assert.equal(CF.prompt(),'PLINE Specify start point:');assert.ok(!('polyline' in CF.previews),'no preview hook left behind');CF.cancel();CF.run('ARC');await DT.idle();assert.equal(tool,'arc');assert.equal(D.state(),null);assert.equal(CF.prompt(),'ARC Specify center point of arc:');CF.cancel();
  DT.setFlags(true,true);CF.run('PL');await DT.idle();assert.equal(tool,'polyline');assert.equal(D.state().tool,'polyline');assert.equal(CF.prompt(),'PLINE Specify start point:');assert.ok('polyline' in CF.previews);CF.cancel();assert.ok(!('polyline' in CF.previews)&&!('polyline' in CF.prompts)&&!('polyline' in CF.pickTools),'hooks removed when the flow ends');CF.run('A');await DT.idle();assert.equal(D.state().tool,'arc');assert.equal(CF.prompt(),'ARC Specify start point of arc or [Center]:');CF.cancel();setTool('polyline');assert.equal(D.state().tool,'polyline','a ribbon/legacy setTool(polyline) gets the same flow');CF.cancel();DT.setFlags(false,true);setTool('polyline');assert.equal(D.state(),null);CF.cancel();DT.setFlags(true,true);assert.equal(CF.resolve('PL').name,'PLINE');assert.equal(CF.resolve('A').name,'ARC');assert.equal(CF.resolve('POLYLINE').name,'PLINE');DT.restoreFlags()`);

 // ---- HATCH: boundary detection from an internal point ----
 await go(`const {P,L}=DT,D=CF.drafting,rect=(x0,y0,x1,y1)=>({type:'polyline',layer:'0',closed:true,points:[P(x0,y0),P(x1,y0),P(x1,y1),P(x0,y1)]});
  const area=l=>Math.abs(D.area(l)),total=r=>r.loops[0]&&r.loops.reduce((s,l,k)=>s+(k?-1:1)*area(l),0);
  // closed polyline, and the same rectangle drawn as four loose lines
  DT.reset([rect(0,0,100,60)]);let r=D.boundaryAt(P(50,30));assert.ok(!r.error);assert.equal(r.loops.length,1);assert.ok(DT.near(total(r),6000,1e-6));assert.ok(D.boundaryAt(P(150,30)).error,'outside everything');assert.equal(D.boundaryAt(P(150,30)).error,'Valid hatch boundary not found.');
  DT.reset([L(0,0,100,0),L(100,0,100,60),L(100,60,0,60),L(0,60,0,0)]);r=D.boundaryAt(P(20,20));assert.ok(DT.near(total(r),6000,1e-6),'four lines form a boundary');
  // a # of crossing lines: the smallest cell around the click, not the whole figure
  DT.reset([L(-10,0,50,0),L(-10,20,50,20),L(10,-10,10,40),L(30,-10,30,40)]);r=D.boundaryAt(P(20,10));assert.ok(DT.near(total(r),400,1e-6),'centre cell of the #');r=D.boundaryAt(P(40,10));assert.equal(r.error,'Valid hatch boundary not found.','the open end cell is not enclosed');
  // T junction splits a rectangle; a dangling stub is ignored
  DT.reset([rect(0,0,100,60),L(50,0,50,60),L(100,30,130,30)]);r=D.boundaryAt(P(20,30));assert.ok(DT.near(total(r),3000,1e-6),'half of the divided rectangle');r=D.boundaryAt(P(80,30));assert.ok(DT.near(total(r),3000,1e-6));
  // partly shared edges
  DT.reset([rect(0,0,10,10),rect(10,5,20,15)]);assert.ok(DT.near(total(D.boundaryAt(P(5,5))),100,1e-6));assert.ok(DT.near(total(D.boundaryAt(P(15,10))),100,1e-6));
  // islands: click between the outer and inner shapes, then inside the inner one
  DT.reset([rect(0,0,100,100),rect(30,30,70,70),{type:'circle',layer:'0',center:P(15,85),radius:5}]);r=D.boundaryAt(P(10,10));assert.equal(r.loops.length,3);assert.ok(DT.near(total(r),10000-1600-Math.PI*25*(128/(2*Math.PI))*Math.sin(2*Math.PI/128),1e-6),'outer minus both islands');
  r=D.boundaryAt(P(50,50));assert.equal(r.loops.length,1);assert.ok(DT.near(total(r),1600,1e-6));
  // a circle crossing a rectangle: the lens is the smaller region
  DT.reset([rect(0,0,20,20),{type:'circle',layer:'0',center:P(0,10),radius:6}]);r=D.boundaryAt(P(3,10));assert.ok(Math.abs(total(r)-Math.PI*18)<0.3,'half disc inside the rectangle: '+total(r));
  // hidden layers and hatch lines are not boundaries
  DT.reset([rect(0,0,10,10),{...rect(-5,-5,15,15),layer:'H'},{...L(0,0,10,10),hatch:true,group:'x'}]);doc.layers.push({name:'H',color:'#fff',visible:false});r=D.boundaryAt(P(5,5));assert.ok(DT.near(total(r),100,1e-6),'hidden outline and hatch line ignored');
  // too many segments is reported, not frozen
  DT.reset(Array.from({length:60},(_,k)=>({type:'circle',layer:'0',center:P(k*3,0),radius:1})));r=D.boundaryAt(P(0,0));assert.ok(r.loops||r.error)`);
 await go(`const {P}=DT,D=CF.drafting;
  const sq=[[P(0,0),P(10,0),P(10,10),P(0,10)]];let f=D.hatchFill(sq,2,45);assert.ok(!f.error);const len=f.lines.reduce((s,[a,b])=>s+Math.hypot(b.x-a.x,b.y-a.y),0);assert.ok(Math.abs(len*2-100)<4,'line length x spacing ~ area, got '+len*2);
  for(const [a,b]of f.lines){assert.ok(DT.near(Math.atan2(b.y-a.y,b.x-a.x)*180/Math.PI,45,1e-6)||DT.near(Math.atan2(b.y-a.y,b.x-a.x)*180/Math.PI,-135,1e-6));for(const p of [a,b])assert.ok(p.x>=-1e-9&&p.x<=10+1e-9&&p.y>=-1e-9&&p.y<=10+1e-9)}
  f=D.hatchFill(sq,2,0);assert.ok(f.lines.every(([a,b])=>DT.near(a.y,b.y)&&Number.isInteger(Math.round(a.y/2))),'horizontal rows on the global spacing grid');
  const ring=[[P(0,0),P(20,0),P(20,20),P(0,20)],[P(5,5),P(15,5),P(15,15),P(5,15)]];f=D.hatchFill(ring,1,45);const l2=f.lines.reduce((s,[a,b])=>s+Math.hypot(b.x-a.x,b.y-a.y),0);assert.ok(Math.abs(l2-300)<8,'island is left empty, got '+l2);
  assert.equal(D.hatchFill(sq,0,45).error,'Enter positive spacing.');assert.ok(D.hatchFill([[P(0,0),P(1e6,0),P(1e6,1e6)]],0.1,45).error.startsWith('Spacing too small'));`);

 // ---- HATCH command flow: click inside, settings, undo, select-objects fallback, typed point ----
 await go(`const {P,L}=DT,D=CF.drafting;DT.reset([{type:'polyline',layer:'0',closed:true,points:[P(0,0),P(100,0),P(100,60),P(0,60)]},{type:'polyline',layer:'0',closed:true,points:[P(200,0),P(260,0),P(260,40),P(200,40)]}]);
  D.settings.hatchSpacing=5;D.settings.hatchAngle=45;CF.run('HATCH');await DT.idle();assert.equal(tool,'hatch');assert.ok(CF.picking&&CF.picking.command==='HATCH');assert.ok(CF.prompt().startsWith('HATCH Pick internal point'));assert.ok(CF.pickTools.hatch());
  assert.equal(CF.previews.hatch(P(50,30)).length,1,'hover previews the region');assert.equal(CF.previews.hatch(P(150,30)).length,0);
  await accept(P(150,30));await DT.idle();assert.equal(doc.entities.length,2,'a click outside any boundary creates nothing');assert.ok(CF.picking,'still waiting for a point');
  await accept(P(50,30));await DT.idle();const n1=doc.entities.length;assert.ok(n1>2+8);const hs=doc.entities.slice(2);assert.ok(hs.every(e=>e.hatch&&e.type==='line'&&e.group===hs[0].group&&e.layer==='0'));assert.equal(D.settings.hatchSpacing,10);
  for(const e of hs)for(const p of e.points)assert.ok(p.x>=-1e-9&&p.x<=100+1e-9&&p.y>=-1e-9&&p.y<=60+1e-9);
  await accept(P(230,20));await DT.idle();assert.ok(doc.entities.length>n1,'second region needs no new question');assert.notEqual(doc.entities.at(-1).group,hs[0].group);
  assert.equal(tool,'hatch');await CF.drafting.input('U');await DT.idle();assert.equal(doc.entities.length,n1,'U undoes the last hatch');
  CF.enter();await DT.idle();assert.equal(tool,'select');assert.equal(D.state(),null);assert.equal(CF.picking,null);undo();assert.equal(doc.entities.length,2);`,['10']);
 await go(`const {P}=DT,D=CF.drafting;DT.reset([{type:'polyline',layer:'0',closed:true,points:[P(0,0),P(100,0),P(100,60),P(0,60)]}]);CF.run('H');await DT.idle();
  await CF.drafting.input('T');await DT.idle();
  assert.equal(D.settings.hatchSpacing,8);assert.equal(D.settings.hatchAngle,90);await CF.drafting.input('50,30');await DT.idle();const hs=doc.entities.slice(1);assert.ok(hs.length>5);assert.ok(hs.every(e=>DT.near(e.points[0].x,e.points[1].x)),'90 degree hatch is vertical');
  assert.equal(await CF.drafting.input('xyz'),false);CF.cancel();await DT.idle();assert.equal(D.state(),null);`,['8','90']);
 // Select objects path: pick the boundary, Enter, spacing question; loose lines work too.
 await go(`const {P,L}=DT,D=CF.drafting;DT.reset([L(0,0,50,0),L(50,0,50,30),L(50,30,0,30),L(0,30,0,0)]);D.settings.hatchAngle=45;CF.run('HATCH');await DT.idle();
  await accept(P(25,0));await accept(P(50,15));assert.equal(CF.selection().length,2);await accept(P(25,30));await accept(P(0,15));CF.enter();await DT.idle();
  assert.equal(tool,'select');assert.ok(doc.entities.length>4+3);assert.equal(selected,-1);assert.ok(doc.entities.slice(4).every(e=>e.hatch));
  DT.reset([{type:'polyline',layer:'0',closed:true,points:[P(0,0),P(30,0),P(30,30),P(0,30)]}]);CF.select([0]);CF.run('HATCH');await DT.idle();CF.enter();await DT.idle();assert.ok(doc.entities.length>5,'a noun-verb selection hatches on Enter');
  DT.reset([L(0,0,10,0)]);CF.select([0]);CF.run('HATCH');await DT.idle();CF.enter();await DT.idle();assert.equal(doc.entities.length,1);assert.ok(CF.picking,'an open line is no boundary: still prompting');CF.cancel();assert.equal(D.state(),null)`,['5','5']);
 // The canvas hook: a click on empty space becomes the internal point, a click on a boundary object stays a selection, hatch lines never block it.
 await go(`const {P}=DT,D=CF.drafting;DT.reset([{type:'polyline',layer:'0',closed:true,points:[P(0,0),P(100,0),P(100,60),P(0,60)]}]);D.settings.hatchSpacing=2;D.settings.hatchAngle=45;CF.run('HATCH');await DT.idle();
  const ev=(x,y,o={})=>({button:0,target:canvas,offsetX:500+x*4,offsetY:350-y*4,consumed:0,preventDefault(){this.consumed++},stopPropagation(){this.consumed++},stopImmediatePropagation(){this.consumed++},...o});
  let e=ev(0,30);assert.equal(D.onHatchClick(e),false,'on the boundary edge: left to the selection code');assert.equal(e.consumed,0);assert.equal(D.onHatchClick(ev(50,30,{shiftKey:true})),false);assert.equal(D.onHatchClick(ev(50,30,{button:2})),false);assert.equal(D.onHatchClick(ev(50,30,{target:{}})),false);
  e=ev(50,30);assert.equal(D.onHatchClick(e),true);assert.ok(e.consumed>=3);await DT.idle();const n=doc.entities.length;assert.ok(n>20,'hatched');
  e=ev(50.3,30.2);assert.equal(D.onHatchClick(e),true,'dense hatch lines under the pickbox do not turn the click into a selection');await DT.idle();assert.ok(doc.entities.length>n);assert.equal(CF.selection().length,0);
  mode3D=true;assert.equal(D.onHatchClick(ev(50,30)),false);mode3D=false;CF.cancel();assert.equal(D.onHatchClick(ev(50,30)),false,'no command running')`,['2']);
 // Without B1 (or when its picking does not apply) a click routed through accept() reaches the same code.
 await go(`const {P}=DT,D=CF.drafting;DT.reset([{type:'circle',layer:'0',center:P(0,0),radius:20}]);CF.run('HATCH');await DT.idle();
  D.settings.hatchSpacing=1;D.settings.hatchAngle=45;await accept(P(1,1));await DT.idle();const hs=doc.entities.slice(1);assert.ok(hs.length>=35);
  let len=0;for(const e of hs)len+=Math.hypot(e.points[1].x-e.points[0].x,e.points[1].y-e.points[0].y);assert.ok(Math.abs(len-Math.PI*400)<20,'filled disc area '+len);CF.cancel()`,['']);

 // ---- Esc during a selection window cancels the whole command ----
 await go(`if(!CF.has('interact'))return;const {P,L}=DT,D=CF.drafting;DT.reset([{type:'polyline',layer:'0',closed:true,points:[P(0,0),P(100,0),P(100,60),P(0,60)]}]);const st=CF.interact.state();
  const ev=()=>({key:'Escape',consumed:0,preventDefault(){this.consumed++},stopPropagation(){this.consumed++},stopImmediatePropagation(){this.consumed++}});
  CF.run('HATCH');await DT.idle();st.win={a:P(1,1),cur:P(5,5),down:false};let e=ev();assert.equal(D.onEscape(e),true);assert.ok(e.consumed>=3);assert.equal(st.win,null);assert.equal(D.state(),null);assert.equal(tool,'select');assert.equal(CF.picking,null);
  CF.run('TRIM');await DT.idle();st.win={a:P(1,1),cur:P(5,5),down:false};assert.equal(D.onEscape(ev()),true);assert.equal(D.state(),null);
  setTool('select');st.win={a:P(1,1),cur:P(5,5),down:false};e=ev();assert.equal(D.onEscape(e),false,'idle window: left to the interaction module');assert.equal(e.consumed,0);st.win=null;assert.equal(D.onEscape(ev()),false);assert.equal(D.onEscape({key:'a'}),false)`);

 // ---- MATCHPROP: colour, layer, text height, feedback ----
 await go(`const {P,L}=DT,D=CF.drafting;DT.reset([{type:'circle',layer:'0',center:P(0,0),radius:5,color:'#ff3030'},{type:'circle',layer:'0',center:P(30,0),radius:5},{type:'circle',layer:'A',center:P(60,0),radius:5,color:'#00ff00'}]);doc.layers.push({name:'A',color:'#ff0000',visible:true});
  CF.run('MA');await DT.idle();await accept(P(5,0));await accept(P(35,0));assert.equal(doc.entities[1].color,'#ff3030','colour is matched');assert.ok(/Matched Color, Layer to 1 object/.test(D.settings.lastReport[0]),D.settings.lastReport[0]);
  assert.ok(CF.selection().includes(0)&&CF.selection().includes(1),'source and destination stay highlighted');
  await accept(P(65,0));assert.equal(doc.entities[2].color,'#ff3030');assert.equal(doc.entities[2].layer,'0');assert.ok(/2 so far/.test(D.settings.lastReport[0]));CF.enter();await DT.idle();assert.equal(CF.selection().length,0);
  // a ByLayer source clears an explicit colour
  CF.run('MATCHPROP');await DT.idle();await accept(P(35,0));doc.entities[1].color=undefined;delete doc.entities[1].color;await accept(P(65,0));assert.ok(!('color' in doc.entities[2]),'ByLayer source resets colour');CF.enter();await DT.idle();undo();assert.equal(doc.entities[2].color,'#ff3030');`);
 await go(`const {P}=DT,D=CF.drafting;DT.reset([{type:'circle',layer:'A',center:P(0,0),radius:5,color:'#ff3030'},{type:'circle',layer:'0',center:P(30,0),radius:5,color:'#00ff00'}]);doc.layers.push({name:'A',color:'#ff0000',visible:true});
  CF.run('MA');await DT.idle();await accept(P(5,0));await CF.drafting.input('S');await DT.idle();assert.equal(D.settings.matchColor,false);assert.equal(D.settings.matchLayer,true);assert.equal(D.settings.matchText,false);
  await accept(P(35,0));assert.equal(doc.entities[1].layer,'A');assert.equal(doc.entities[1].color,'#00ff00','colour untouched when only Layer is matched');assert.ok(/Matched Layer to 1 object/.test(D.settings.lastReport[0]));
  Object.assign(D.settings,{matchColor:true,matchLayer:true,matchText:true});CF.cancel()`,['Layer']);
 console.log('PASS: drafting OFFSET (lines/circles/closed+open polylines, through), quick TRIM (lines/circles/polylines, erase, shift-extend), EXTEND, FILLET (r=0, tangent arc, polyline), CHAMFER (distance/angle), MIRROR, POLYGON, ELLIPSE, DIMLINEAR, DIST/ID/LIST, MATCHPROP (colour/layer/text, feedback), ARRAYRECT, PURGE, cancellation, PLINE kept on Esc, HATCH internal-point boundaries/islands/flow, Esc during a selection window');
};
