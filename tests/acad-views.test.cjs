// C2 views: presets and orientation, visual styles, ViewCube zones, feature edges, 3D picking/pan/zoom, paper-space layout.
module.exports=async({run,assert})=>{
 if(!await run("return typeof CF!=='undefined'&&CF.has('views')")){console.log('SKIP: acad-views');return}
 await run(`
 const V=CF.views,near=(a,b,t=1e-6)=>Math.abs(a-b)<t,ang=(a,b)=>Math.abs(Math.atan2(Math.sin(a-b),Math.cos(a-b)));
 const sq=(x,y,s)=>[{x,y},{x:x+s,y},{x:x+s,y:y+s},{x,y:y+s}];
 doc.solids=[makeExtrusion(sq(0,0,10),10,'A')];
 // Presets: Front looks from -Y (X right, Z up); SE Isometric looks from +X,-Y,+Z.
 CF.setViewPreset('front');assert.ok(mode3D);assert.ok(near(camera3.yaw,0)&&near(camera3.pitch,0));assert.equal(V.viewName(),'Front');
 assert.ok(project3({x:5,y:-50,z:5}).depth<project3({x:5,y:50,z:5}).depth);assert.ok(project3({x:50,y:5,z:5}).x>project3({x:-50,y:5,z:5}).x);assert.ok(project3({x:5,y:5,z:50}).y<project3({x:5,y:5,z:-50}).y);
 CF.setViewPreset('SE Isometric');assert.ok(near(camera3.yaw,-Math.PI/4)&&near(camera3.pitch,.6155,1e-3));assert.equal(V.viewName(),'SE Isometric');
 const eye=p=>project3(p).depth;assert.ok(eye({x:105,y:-95,z:105})<eye({x:-95,y:105,z:-95}));
 for(const [n,y,p] of [['back',Math.PI,0],['right',-Math.PI/2,0],['left',Math.PI/2,0],['sw',Math.PI/4,.6155],['ne',-3*Math.PI/4,.6155],['nw',3*Math.PI/4,.6155]]){CF.setViewPreset(n);assert.ok(ang(camera3.yaw,y)<1e-9,n);assert.ok(near(camera3.pitch,p,1e-3),n)}
 CF.setViewPreset('right');assert.ok(eye({x:100,y:5,z:5})<eye({x:-100,y:5,z:5}));assert.ok(project3({x:5,y:50,z:5}).x>project3({x:5,y:-50,z:5}).x);
 CF.setViewPreset('bottom');assert.ok(camera3.pitch<-1.5);assert.ok(eye({x:5,y:5,z:-100})<eye({x:5,y:5,z:100}));
 camera3.yaw+=.2;assert.equal(V.viewName(),'Custom View');
 assert.equal(CF.setViewPreset('nowhere'),false);
 // Select tool (Esc) keeps 3D; drawing tools return to 2D model space; Top returns to the 2D plan view.
 CF.setViewPreset('se');setTool('select');assert.ok(mode3D);setTool('line');assert.ok(!mode3D);setTool('select');
 CF.setViewPreset('se');CF.setViewPreset('top');assert.ok(!mode3D);assert.equal(V.viewName(),'Top');assert.equal(V.styleLabel(),'2D Wireframe');
 // ViewCube direction mapping and zone hit-testing (2D model = looking down at TOP).
 assert.equal(V.presetFromDirection([1,-1,1]),'se');assert.equal(V.presetFromDirection([0,-2,0]),'front');assert.equal(V.presetFromDirection([0,-1,1]),null);
 const edge=V.cameraFromDirection([0,-1,1]);assert.ok(near(edge.yaw,0)&&near(edge.pitch,Math.PI/4));
 for(const k of Object.keys(V.presets))if(k!=='top'&&k!=='bottom'){const c=V.cameraFromDirection(V.presets[k].d);assert.ok(ang(c.yaw,V.presets[k].yaw)<1e-9&&near(c.pitch,V.presets[k].pitch,1e-9),k)}
 const top={yaw:0,pitch:Math.PI/2},C=V.state,cx=64,cy=60,h=21;
 assert.deepEqual(V.cubeHit(cx,cy,top).d,[0,0,1]);assert.deepEqual(V.cubeHit(cx+.9*h,cy+.9*h,top).d,[1,-1,1]);assert.deepEqual(V.cubeHit(cx,cy+.9*h,top).d,[0,-1,1]);
 V.cubeClick(V.cubeHit(cx+.9*h,cy+.9*h,top));assert.ok(mode3D);assert.equal(V.viewName(),'SE Isometric');
 const se=V.presets.se,front=V.cubeHit(cx-12,cy+18,se);assert.equal(front.face,'FRONT');
 V.cubeClick(V.cubeHit(cx,cy,{yaw:0,pitch:Math.PI/2}));assert.ok(!mode3D);
 // Visual styles.
 assert.equal(CF.setVisualStyle('Wireframe'),true);assert.ok(mode3D);assert.equal(V.style(),'wireframe');assert.equal(V.styleLabel(),'Wireframe');
 assert.equal(CF.setVisualStyle('Shaded with Edges'),true);assert.equal(V.style(),'shadededges');assert.equal(CF.setVisualStyle('bogus'),false);assert.equal(V.style(),'shadededges');
 doc.entities=[{type:'line',layer:'0',points:[{x:0,y:0},{x:30,y:0}]},{type:'circle',layer:'0',center:{x:5,y:5},radius:3},{type:'text',layer:'0',points:[{x:0,y:-5}],text:'Note',height:2.5},{type:'polyline',layer:'0',closed:true,points:sq(-5,-5,20)}];
 for(const s of ['2dwireframe','wireframe','shaded','shadededges']){CF.setVisualStyle(s);render()}
 assert.ok($('status').textContent.startsWith('3D | 1 mesh'));
 // Feature edges: a box shows its 12 edges (diagonals hidden); a 64-gon cylinder shows only its two rims.
 const box=makeExtrusion(sq(0,0,10),30,'Box');assert.equal(box.faces.length,12);assert.equal(V.featureEdges(box).length,12);
 const cyl=makeExtrusion(Array.from({length:64},(_,i)=>({x:10*Math.cos(i*Math.PI/32),y:10*Math.sin(i*Math.PI/32)})),20,'Cyl');assert.equal(V.featureEdges(cyl).length,128);assert.ok(V.meshEdges(cyl).edges.filter(e=>!e.feature).length>=64);
 // CSG-style T-junction between coplanar triangles is not drawn; the open outline is.
 const P=[[0,0],[2,0],[2,2],[0,2],[4,0],[4,2],[2,1]].map(([x,y])=>({x,y,z:0}));assert.equal(V.featureEdges({vertices:P,faces:[[0,1,2],[0,2,3],[1,4,6],[6,4,5],[6,5,2]]}).length,6);
 // T-junction between perpendicular faces: the long front-bottom edge is met by two split pieces and is still a feature edge.
 {const t=makeExtrusion(sq(0,0,10),10,'T');t.vertices.push({x:5,y:0,z:0});const M=t.vertices.length-1;const fi=t.faces.findIndex(f=>f.every(i=>i<4)&&f.includes(0)&&f.includes(1)),f=t.faces[fi];let k=0;for(;k<3;k++)if([0,1].includes(f[k])&&[0,1].includes(f[(k+1)%3]))break;const p=f[k],q=f[(k+1)%3],r=f[(k+2)%3];t.faces.splice(fi,1,[p,M,r],[M,q,r]);
  const fe=V.featureEdges(t);assert.equal(fe.length,13,'split edge = 2 pieces');assert.equal(fe.filter(e=>e.f2<0).length,0,'every piece pairs with the long edge')}
 // Hidden-edge removal: the back box's edges behind the front box are hidden, the front box's own edges stay visible.
 {const A=makeExtrusion(sq(0,0,10),10,'A'),B=makeExtrusion(sq(3,20,4),4,'B');B.vertices.forEach(v=>v.z+=3);CF.setViewPreset('front');meshCenter={x:5,y:10,z:5};camera3.scale=20;
  const mkm=s=>{const P=s.vertices.map(project3),front=new Uint8Array(s.faces.length);s.faces.forEach((t,i)=>{const [a,b,c]=t.map(k=>P[k]);if((b.x-a.x)*(c.y-a.y)-(b.y-a.y)*(c.x-a.x)<0)front[i]=1});return{s,P,front}};
  const per=[mkm(A),mkm(B)],Z=V.depthBuffer(per);let segs=0;const g={moveTo(){segs++},lineTo(){}};
  {const e=m=>({a:m.s.vertices.findIndex(v=>v.x===(m.s===A?0:3)&&v.y===(m.s===A?0:20)&&v.z===(m.s===A?0:3)),b:m.s.vertices.findIndex(v=>v.x===(m.s===A?10:7)&&v.y===(m.s===A?0:20)&&v.z===(m.s===A?0:3))});
   const ea=e(per[0]),eb=e(per[1]);V.visibleSegments(per[0].P[ea.a],per[0].P[ea.b],Z,.1,g);assert.equal(segs,1);segs=0;V.visibleSegments(per[1].P[eb.a],per[1].P[eb.b],Z,.1,g);assert.equal(segs,0)}}
 // 3D picking: click picks Mesh A, Shift+click Mesh B; empty space does nothing.
 const A=makeExtrusion(sq(0,0,10),10,'A'),B=makeExtrusion(sq(40,0,10),10,'B');doc.solids=[A,B];doc.entities=[];CF.setVisualStyle('shadededges');CF.setViewPreset('front');
 const pb=project3({x:45,y:0,z:5}),pa=project3({x:5,y:0,z:5});assert.equal(V.pickMesh(pb.x,pb.y).index,1);assert.equal(V.pickMesh(pa.x,pa.y).index,0);assert.equal(V.pickMesh(pb.x,5),null);
 const click=(p,shiftKey=false,button=0)=>{const e={button,clientX:p.x,clientY:p.y,offsetX:p.x,offsetY:p.y,pointerId:1,shiftKey,preventDefault(){}};canvas.onpointerdown(e);canvas.onpointerup(e)};
 click(pb);assert.equal($('meshA').value,'1');click(pa,true);assert.equal($('meshB').value,'0');assert.deepEqual(V.picked(),{a:1,b:0});render();
 click({x:pb.x,y:5});assert.equal($('meshA').value,'1');
 // Middle-drag pans in the screen plane; the wheel zooms about the cursor.
 const q={x:45,y:0,z:5},before=project3(q);canvas.onpointerdown({button:1,clientX:100,clientY:100,offsetX:100,offsetY:100,pointerId:2});canvas.onpointermove({clientX:130,clientY:120,offsetX:130,offsetY:120});canvas.onpointerup({button:1,clientX:130,clientY:120,pointerId:2});
 const after=project3(q);assert.ok(near(after.x-before.x,30,1e-6)&&near(after.y-before.y,20,1e-6));
 const s0=camera3.scale;canvas.onwheel({deltaY:-200,offsetX:after.x,offsetY:after.y,preventDefault(){}});assert.ok(camera3.scale>s0);const z=project3(q);assert.ok(near(z.x,after.x,1e-6)&&near(z.y,after.y,1e-6));
 // Left-drag orbits.
 canvas.onpointerdown({button:0,clientX:10,clientY:10,offsetX:10,offsetY:10,pointerId:3});canvas.onpointermove({clientX:60,clientY:10});canvas.onpointerup({button:0,clientX:60,clientY:10,pointerId:3});assert.ok(near(camera3.yaw,.4,1e-9));assert.equal(V.viewName(),'Custom View');
 // Layout (paper) space: model extents map inside the sheet's viewport and onto the sheet on screen.
 const saved=['sheetPaper','sheetOrientation','sheetScale'].map(id=>$(id).value);
 CF.setViewPreset('top');doc.solids=[];doc.entities=[{type:'polyline',layer:'0',closed:true,points:[{x:-50,y:-20},{x:150,y:-20},{x:150,y:80},{x:-50,y:80}]},{type:'circle',layer:'0',center:{x:0,y:0},radius:10}];
 for(const [paper,orient,scale,w] of [['A3','landscape','fit',420],['A4','portrait','fit',210],['bogus','bogus','bogus',297]]){
  $('sheetPaper').value=paper;$('sheetOrientation').value=orient;$('sheetScale').value=scale;CF.setSpace('layout');V.fitLayout();render();
  const L=V.layout(),s=L.sheet;assert.equal(s.width,w);const a=L.paperToScreen({x:0,y:0}),b=L.paperToScreen({x:s.width,y:s.height});assert.ok(a.x>=0&&a.y>=0&&b.x<=W&&b.y<=H);
  for(const p of [{x:-50,y:-20},{x:150,y:80},{x:-50,y:80},{x:150,y:-20}]){const m=L.modelToPaper(p,s);assert.ok(m.x>=12-1e-6&&m.x<=s.width-12+1e-6&&m.y>=12-1e-6&&m.y<=s.height-12+1e-6,JSON.stringify(m));const sp=L.paperToScreen(m);assert.ok(sp.x>=a.x&&sp.x<=b.x&&sp.y>=a.y&&sp.y<=b.y)}
  const r=L.screenToPaper(L.paperToScreen({x:33,y:44}));assert.ok(near(r.x,33)&&near(r.y,44))}
 const L=V.layout(),m={x:100,y:80},sp=L.paperToScreen(m);canvas.onwheel({deltaY:300,offsetX:sp.x,offsetY:sp.y,preventDefault(){}});const sp2=V.layout().paperToScreen(m);assert.ok(near(sp.x,sp2.x,1e-6)&&near(sp.y,sp2.y,1e-6));
 canvas.onpointerdown({button:1,clientX:0,clientY:0,offsetX:0,offsetY:0,pointerId:4});canvas.onpointermove({clientX:-25,clientY:10});canvas.onpointerup({button:1,clientX:-25,clientY:10,pointerId:4});const sp3=V.layout().paperToScreen(m);assert.ok(near(sp3.x-sp2.x,-25,1e-6)&&near(sp3.y-sp2.y,10,1e-6));
 assert.equal(V.viewName(),'Paper');setTool('line');assert.equal(CF.space,'model');setTool('select');
 ['sheetPaper','sheetOrientation','sheetScale'].forEach((id,i)=>$(id).value=saved[i]);
 // Display toggles, viewport label and 2D-model delegation (raw select still reaches the engine handlers).
 CF.toggle('viewCube',false);CF.toggle('ucsIcon',false);render();CF.toggle('viewCube',true);CF.toggle('ucsIcon',true);
 render();assert.deepEqual(V.label(),['[\\u2013]','[Top]','[2D Wireframe]']);
 doc.entities=[{type:'line',layer:'0',points:[{x:1.25,y:1.75},{x:3.25,y:1.75}]}];view={x:0,y:0,scale:100};clearSelection();canvas.onpointerdown({button:0,offsetX:700,offsetY:175,clientX:700,clientY:175,pointerId:1,shiftKey:false});assert.equal(selected,0);clearSelection();
 doc.solids=[];
 // ---- Regression: solids in the Top / plan view, stale camera on style change, UCS icon in every view, wireframe styles ----
 {const sq2=(x,y,w,d)=>[{x,y},{x:x+w,y},{x:x+w,y:y+d},{x,y:y+d}],sol=(x,y,w,d,h,z=0,n='S')=>{const m=makeExtrusion(sq2(x,y,w,d),h,n);m.vertices.forEach(v=>v.z+=z);return m};
  // A bare solid (no source sketch) is visible when looking straight down, and zoom extents frames it.
  doc.solids=[sol(100,200,50,40,30,0,'Box')];doc.entities=[];CF.setViewPreset('se');CF.setViewPreset('top');assert.ok(!mode3D);render();
  let P=V.planInfo();assert.equal(P.faces.length,2,'top face = 2 triangles');assert.equal(P.edges.length,1);assert.equal(P.edges[0].segs.length/4,4,'top rim = 4 visible edges');
  assert.ok(near(view.x,125,1e-6)&&near(view.y,220,1e-6),'extents frame the solid: '+JSON.stringify(view));
  for(const F of P.faces)for(let i=0;i<3;i++)assert.ok(F.x[i]>=0&&F.x[i]<=W&&F.y[i]>=0&&F.y[i]<=H,'plan faces land on the canvas');
  // The plan geometry is drawn before (beneath) the 2D entities.
  doc.entities=[{type:'line',layer:'0',points:[{x:100,y:200},{x:150,y:240}]}];let under=null;{const d0=drawEntity;drawEntity=function(...a){if(under===null)under=V.planInfo().sig!=='stale';return d0.apply(this,a)};V.planInfo().sig='stale';render();drawEntity=d0}assert.equal(under,true,'solids are painted under the entities');
  // Zoom Extents (fit) in 2D covers entities and solids together.
  doc.entities=[{type:'line',layer:'0',points:[{x:0,y:0},{x:10,y:0}]}];fit();assert.ok(near(view.x,75,1e-6)&&near(view.y,120,1e-6),'fit covers drawing + solid: '+JSON.stringify(view));
  // Hidden-surface: a small solid completely under a bigger one contributes no edges and its top face is painted first.
  doc.entities=[];doc.solids=[sol(0,0,40,40,10,0,'Big'),sol(10,10,10,10,5,0,'Under')];fit();render();P=V.planInfo();assert.equal(P.faces.length,4);assert.equal(P.edges.length,1,'hidden solid has no visible edges');
  doc.solids=[sol(0,0,40,40,10,0,'Low'),sol(10,10,10,10,5,10,'Up')];render();P=V.planInfo();assert.equal(P.edges.length,2,'a solid standing on another shows its own rim');
  doc.solids=[];render();
  // Choosing a 3D visual style from the plan view goes to a predictable SE Isometric, never the last orbit camera.
  doc.solids=[sol(0,0,10,10,10)];camera3.yaw=1.1;camera3.pitch=.3;CF.setViewPreset('top');assert.ok(!mode3D);
  assert.equal(CF.setVisualStyle('Shaded'),true);assert.ok(mode3D);assert.equal(V.viewName(),'SE Isometric');assert.ok(near(camera3.yaw,-Math.PI/4,1e-9)&&near(camera3.pitch,.6155,1e-3));
  camera3.yaw=-.4;camera3.pitch=.2;CF.setViewPreset('top');assert.equal(CF.setVisualStyle('2D Wireframe'),true);assert.ok(!mode3D,'2D Wireframe keeps the plan view');assert.ok(near(camera3.yaw,-.4,1e-9));
  CF.setVisualStyle('shadededges');
  // UCS icon: every axis tip and its label stays on the canvas in all ten preset views (Bottom pointed Y off the canvas).
  for(const k of Object.keys(V.presets)){if(k==='top')continue;CF.setViewPreset(k);const u=V.ucs3D(),b=u.box;assert.ok(u.gx+b.x0>=8-1e-6&&u.gx+b.x1<=W-8+1e-6&&u.gy+b.y0>=8-1e-6&&u.gy+b.y1<=H-8+1e-6,'UCS icon inside canvas in '+k+' '+JSON.stringify(u.box));for(const a of u.axes){assert.ok(u.gx+a.dx>=0&&u.gx+a.dx<=W&&u.gy+a.dy>=0&&u.gy+a.dy<=H,k+' '+a.n)}}
  CF.setViewPreset('bottom');{const u=V.ucs3D(),y=u.axes.find(a=>a.n==='Y');assert.ok(y.dy>30,'Bottom: Y points down');assert.ok(u.gy+y.dy+16<=H-8+1e-6,'Bottom: Y arrow and label inside');assert.ok(u.gy<H-30,'origin moved up')}
  CF.setViewPreset('front');{const u=V.ucs3D();assert.equal(u.gx,30);assert.equal(u.gy,H-30,'normal views keep the corner placement')}
  // 2D Wireframe is flat; Wireframe is depth-cued (several opacities, hidden edges fainter) over the same edges.
  CF.setViewPreset('se');{const mm=makeExtrusion(sq2(0,0,10,10),10,'W'),P3=mm.vertices.map(project3),front=new Uint8Array(mm.faces.length);mm.faces.forEach((t,i)=>{const [a,b,c]=t.map(k=>P3[k]);if((b.x-a.x)*(c.y-a.y)-(b.y-a.y)*(c.x-a.x)<0)front[i]=1});
   const per=[{s:mm,info:V.meshEdges(mm),P:P3,front,rgb:[1,1,1],hl:0}],flat=V.wireGroups(per,'2dwireframe')[0],cued=V.wireGroups(per,'wireframe')[0];
   assert.equal(new Set(flat.map(g=>g.alpha)).size,1);assert.equal(flat[0].alpha,1);assert.ok(new Set(cued.map(g=>g.alpha)).size>=3,'depth cue: several opacities');assert.ok(Math.min(...cued.map(g=>g.alpha))<.5&&Math.max(...cued.map(g=>g.alpha))===1);
   assert.equal(flat.reduce((n,g)=>n+g.list.length/2,0),cued.reduce((n,g)=>n+g.list.length/2,0),'same edge set');}
  for(const s of ['2dwireframe','wireframe'])CF.setVisualStyle(s),render();
  doc.solids=[];doc.entities=[];}
 `);
 console.log('PASS: acad-views presets/orientation, Select keeps 3D, ViewCube zones, visual styles, feature edges (box/cylinder/T-junction), Mesh A/B picking, 3D pan/zoom/orbit, layout sheet transform, paper pan/zoom, chrome toggles, 2D delegation, plan-view solids (visible/hidden-surface/extents), style-from-Top camera, UCS icon on canvas, wireframe vs 2D wireframe');
};
