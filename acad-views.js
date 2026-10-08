'use strict';
// C2 views: viewport controls, ViewCube, navigation bar, UCS icon, 3D visual styles and picking, layout (paper) space.
CF.module('views');
{
const PI=Math.PI,TAU=PI*2,HALF=PI/2,ISO=Math.asin(1/Math.sqrt(3)); // ISO = 0.6155 rad (35.26 deg)
// Camera presets for project3 (yaw about Z, pitch above the XY plane). d = direction from the target toward the viewer.
const PRESETS={top:{yaw:0,pitch:HALF,label:'Top',d:[0,0,1]},bottom:{yaw:0,pitch:-HALF+1e-3,label:'Bottom',d:[0,0,-1]},left:{yaw:HALF,pitch:0,label:'Left',d:[-1,0,0]},right:{yaw:-HALF,pitch:0,label:'Right',d:[1,0,0]},front:{yaw:0,pitch:0,label:'Front',d:[0,-1,0]},back:{yaw:PI,pitch:0,label:'Back',d:[0,1,0]},sw:{yaw:PI/4,pitch:ISO,label:'SW Isometric',d:[-1,-1,1]},se:{yaw:-PI/4,pitch:ISO,label:'SE Isometric',d:[1,-1,1]},ne:{yaw:-3*PI/4,pitch:ISO,label:'NE Isometric',d:[1,1,1]},nw:{yaw:3*PI/4,pitch:ISO,label:'NW Isometric',d:[-1,1,1]}};
const STYLES={'2dwireframe':'2D Wireframe',wireframe:'Wireframe',shaded:'Shaded',shadededges:'Shaded with Edges'};
const S={style:'shadededges',pick:{a:null,b:null},panMode:false,drag:null,cubeDrag:null,hover:null,tween:null,lastMid:null,menu:null,menuAnchor:null,mode:'',chrome:'',cubeSig:'',layout:{x:148.5,y:105,s:2,fitted:false,sig:''}};
const CUBE={size:128,h:21,cx:64,cy:60};
const dot=(a,b)=>a.x*b.x+a.y*b.y+a.z*b.z,v3=(x,y,z)=>({x,y,z}),wrapA=a=>Math.atan2(Math.sin(a),Math.cos(a)),clampPitch=p=>Math.max(-HALF+1e-3,Math.min(HALF-1e-3,p));
const isLayout=()=>CF.space==='layout'&&!mode3D;
// Orthonormal camera basis matching project3: R = screen right, U = screen up, V = toward the viewer.
function basis(yaw,pitch){const cy=Math.cos(yaw),sy=Math.sin(yaw),cp=Math.cos(pitch),sp=Math.sin(pitch);return{R:v3(cy,-sy,0),U:v3(sy*sp,cy*sp,cp),V:v3(-sy*cp,-cy*cp,sp)}}
function hexRgb(c){const m=/^#?([0-9a-f]{6})$/i.exec(String(c||''));const n=m?parseInt(m[1],16):0x63d9c0;return[n>>16&255,n>>8&255,n&255]}

// ---- View presets and visual styles ---------------------------------------------------
function presetKey(name){const k=String(name||'').toLowerCase().replace(/isometric|iso|view|[\s_.-]/g,'');return PRESETS[k]?k:({southeast:'se',southwest:'sw',northeast:'ne',northwest:'nw',plan:'top'})[k]||null}
function presetOf(yaw,pitch){if(pitch>HALF-2e-3)return'top';for(const [k,p]of Object.entries(PRESETS))if(k!=='top'&&Math.abs(wrapA(yaw-p.yaw))<1e-3&&Math.abs(pitch-p.pitch)<1e-3)return k;return null}
function viewName(){if(isLayout())return'Paper';if(!mode3D)return'Top';const k=presetOf(camera3.yaw,camera3.pitch);return k?PRESETS[k].label:'Custom View'}
function styleLabel(){return mode3D?STYLES[S.style]:'2D Wireframe'}
// A direction matches a preset when its non-zero components share one magnitude and the preset's signs.
function presetFromDirection(d){const m=Math.max(...d.map(Math.abs));if(!m)return null;const k=d.map(v=>Math.abs(v)/m<1e-6?0:Math.abs(Math.abs(v)/m-1)<1e-6?Math.sign(v):NaN).join(',');for(const [n,p]of Object.entries(PRESETS))if(p.d.join(',')===k)return n;return null}
function cameraFromDirection(d){const len=Math.hypot(...d)||1;return{yaw:Math.abs(d[0])+Math.abs(d[1])<1e-9?0:Math.atan2(-d[0],-d[1]),pitch:Math.asin(Math.max(-1,Math.min(1,d[2]/len)))}}
const canAnimate=()=>typeof requestAnimationFrame==='function'&&!S.noAnimate;
function emitView(){CF.emit('view',{view:viewName(),style:S.style,mode3D})}
// Move the 3D camera (entering 3D when needed); the change is animated by a short time-based tween.
function goCamera(cam,animate=true){if(isLayout())CF.setSpace('model');const was3D=mode3D,from=was3D?{yaw:camera3.yaw,pitch:camera3.pitch}:{yaw:0,pitch:HALF-1e-3};camera3.yaw=cam.yaw;camera3.pitch=cam.pitch;if(!was3D)show3D();if(animate&&canAnimate()&&(Math.abs(wrapA(from.yaw-cam.yaw))>1e-3||Math.abs(from.pitch-cam.pitch)>1e-3)){S.tween={from,to:{yaw:cam.yaw,pitch:cam.pitch},t0:performance.now(),dur:400};const step=()=>{if(!S.tween)return;if(performance.now()-S.tween.t0>=S.tween.dur)S.tween=null;render();if(S.tween)requestAnimationFrame(step)};requestAnimationFrame(step)}render();emitView()}
// Draw with the interpolated camera while a tween runs; camera3 always holds the target.
function withTween(fn){const t=S.tween;if(!t||!mode3D)return fn();const k=Math.min(1,(performance.now()-t.t0)/t.dur);if(k>=1){S.tween=null;return fn()}const e=k<.5?2*k*k:1-(-2*k+2)**2/2,yaw=camera3.yaw,pitch=camera3.pitch;camera3.yaw=t.from.yaw+wrapA(t.to.yaw-t.from.yaw)*e;camera3.pitch=t.from.pitch+(t.to.pitch-t.from.pitch)*e;try{return fn()}finally{camera3.yaw=yaw;camera3.pitch=pitch}}
CF.setViewPreset=function(name){const k=presetKey(name);if(!k){notify(`Unknown view "${name}".`);return false}S.tween=null;if(k==='top'){if(isLayout())CF.setSpace('model');if(mode3D)showModel();else render();emitView();return true}goCamera(PRESETS[k]);return true};
function styleKey(name){const s=String(name||'').toLowerCase().replace(/with|[\s_-]/g,'');return STYLES[s]?s:({'2d':'2dwireframe','2dw':'2dwireframe',w:'wireframe','3dwireframe':'wireframe',s:'shaded',realistic:'shaded',shadesofgray:'shaded',shadesofgrey:'shaded',e:'shadededges',shadededge:'shadededges',conceptual:'shadededges'})[s]||null}
CF.setVisualStyle=function(name){const k=styleKey(name);if(!k){notify(`Unknown visual style "${name}".`);return false}S.style=k;if(!isLayout()&&!mode3D&&k!=='2dwireframe')show3D();else render();notify(`Visual style: ${STYLES[k]}`);emitView();return true};
CF.visualStyle=()=>S.style;
try{camera3.yaw=PRESETS.se.yaw;camera3.pitch=PRESETS.se.pitch}catch(err){}

// ---- Mesh edges (feature edges, silhouettes) -------------------------------------------
// Edges whose adjacent face normals differ by more than `angle` are feature edges; gently curved edges are kept as
// silhouette candidates. Unpaired edges that are covered by collinear coplanar edges (CSG T-junctions) are hidden.
function meshEdges(mesh,angle=20){
 const V=mesh.vertices,F=mesh.faces,nf=F.length,nv=V.length,N=new Float64Array(nf*3),ok=new Uint8Array(nf),cosT=Math.cos(angle*PI/180);
 let x0=Infinity,x1=-Infinity,y0=Infinity,y1=-Infinity,z0=Infinity,z1=-Infinity;for(const p of V){if(p.x<x0)x0=p.x;if(p.x>x1)x1=p.x;if(p.y<y0)y0=p.y;if(p.y>y1)y1=p.y;if(p.z<z0)z0=p.z;if(p.z>z1)z1=p.z}
 const size=Math.hypot(x1-x0,y1-y0,z1-z0)||1,tol=size*1e-7;
 const id=new Int32Array(nv),seen=new Map();for(let i=0;i<nv;i++){const p=V[i],k=Math.round(p.x/tol)+','+Math.round(p.y/tol)+','+Math.round(p.z/tol);let j=seen.get(k);if(j===undefined){j=i;seen.set(k,i)}id[i]=j}
 for(let f=0;f<nf;f++){const [a,b,c]=F[f].map(i=>V[i]);if(!a||!b||!c)continue;const ux=b.x-a.x,uy=b.y-a.y,uz=b.z-a.z,wx=c.x-a.x,wy=c.y-a.y,wz=c.z-a.z,nx=uy*wz-uz*wy,ny=uz*wx-ux*wz,nz=ux*wy-uy*wx,l=Math.hypot(nx,ny,nz);if(l>size*size*1e-14){N[f*3]=nx/l;N[f*3+1]=ny/l;N[f*3+2]=nz/l;ok[f]=1}}
 const nd=(f,g)=>N[f*3]*N[g*3]+N[f*3+1]*N[g*3+1]+N[f*3+2]*N[g*3+2],map=new Map();
 for(let f=0;f<nf;f++){if(!ok[f])continue;for(let i=0;i<3;i++){const ra=F[f][i],rb=F[f][(i+1)%3],a=id[ra],b=id[rb];if(a===b)continue;const key=a<b?a*nv+b:b*nv+a;let r=map.get(key);if(!r)map.set(key,r={a:ra,b:rb,fs:[]});r.fs.push(f)}}
 // CSG T-junctions: split every unpaired edge at the welded vertices lying on it so the pieces pair up with the neighbours.
 {const reps=[];for(let i=0;i<nv;i++)if(id[i]===i)reps.push(i);let nOpen=0;for(const r of map.values())if(r.fs.length===1)nOpen++;
  if(nOpen&&nOpen*reps.length<4e7){const lt=size*2e-6,lt2=lt*lt,jobs=[];
   for(const [key,r] of map){if(r.fs.length!==1)continue;const A=V[r.a],B=V[r.b],dx=B.x-A.x,dy=B.y-A.y,dz=B.z-A.z,L2=dx*dx+dy*dy+dz*dz;if(L2<lt2*4)continue;
    const x0=Math.min(A.x,B.x)-lt,x1=Math.max(A.x,B.x)+lt,y0=Math.min(A.y,B.y)-lt,y1=Math.max(A.y,B.y)+lt,z0=Math.min(A.z,B.z)-lt,z1=Math.max(A.z,B.z)+lt,ia=id[r.a],ib=id[r.b],on=[];
    for(const i of reps){if(i===ia||i===ib)continue;const P=V[i];if(P.x<x0||P.x>x1||P.y<y0||P.y>y1||P.z<z0||P.z>z1)continue;const t=((P.x-A.x)*dx+(P.y-A.y)*dy+(P.z-A.z)*dz)/L2;if(t<=0||t>=1)continue;const ex=A.x+t*dx-P.x,ey=A.y+t*dy-P.y,ez=A.z+t*dz-P.z;if(ex*ex+ey*ey+ez*ez<=lt2)on.push({t,i})}
    if(on.length)jobs.push([key,r,on.sort((p,q)=>p.t-q.t)])}
   for(const [key,r,on] of jobs){map.delete(key);const chain=[r.a,...on.map(o=>o.i),r.b];for(let k=0;k+1<chain.length;k++){const ra=chain[k],rb=chain[k+1],a=id[ra],b=id[rb];if(a===b)continue;const k2=a<b?a*nv+b:b*nv+a;let r2=map.get(k2);if(!r2)map.set(k2,r2={a:ra,b:rb,fs:[]});r2.fs.push(r.fs[0])}}}}
 const edges=[],open=[];
 for(const r of map.values()){const fs=r.fs;if(fs.length===2){const d=nd(fs[0],fs[1]);if(d<cosT)edges.push({a:r.a,b:r.b,f1:fs[0],f2:fs[1],feature:true});else if(d<.99999)edges.push({a:r.a,b:r.b,f1:fs[0],f2:fs[1],feature:false})}else if(fs.length===1)open.push({a:r.a,b:r.b,f:fs[0]});else edges.push({a:r.a,b:r.b,f1:fs[0],f2:fs[1],feature:true})}
 // Group unpaired edges by supporting line and test interval coverage by edges of similarly oriented faces.
 // Every edge lying on such a line (paired ones too, e.g. between slivers) counts toward the coverage.
 const groups=new Map(),q=size*1e-5;
 const lineOf=(ia,ib,e)=>{const A=V[ia],B=V[ib];let dx=B.x-A.x,dy=B.y-A.y,dz=B.z-A.z;const l=Math.hypot(dx,dy,dz);if(l<tol)return null;dx/=l;dy/=l;dz/=l;if(dx<-1e-9||(Math.abs(dx)<=1e-9&&(dy<-1e-9||(Math.abs(dy)<=1e-9&&dz<0)))){dx=-dx;dy=-dy;dz=-dz}const ta=A.x*dx+A.y*dy+A.z*dz,tb=B.x*dx+B.y*dy+B.z*dz;e.t0=Math.min(ta,tb);e.t1=Math.max(ta,tb);return[dx,dy,dz].map(v=>Math.round(v*1e4)).join()+'|'+[A.x-ta*dx,A.y-ta*dy,A.z-ta*dz].map(v=>Math.round(v/q)).join()};
 for(const e of open){const key=lineOf(e.a,e.b,e);if(key)(groups.get(key)||groups.set(key,[]).get(key)).push(e)}
 if(groups.size)for(const r of map.values()){if(r.fs.length<2)continue;const t={},key=lineOf(r.a,r.b,t),g=key&&groups.get(key);if(g)for(const f of r.fs)g.push({f,t0:t.t0,t1:t.t1,cover:true})}
 for(const g of groups.values())for(const e of g){if(e.cover)continue;const iv=[];for(const o of g){if(o===e||o.f===e.f||nd(o.f,e.f)<cosT)continue;const s=Math.max(e.t0,o.t0),t=Math.min(e.t1,o.t1);if(t>s)iv.push([s,t])}iv.sort((p,r)=>p[0]-r[0]);let covered=0,end=e.t0;for(const [s,t]of iv){const a=Math.max(s,end);if(t>a){covered+=t-a;end=t}}if(covered<(e.t1-e.t0)*(1-1e-6)-tol)edges.push({a:e.a,b:e.b,f1:e.f,f2:-1,feature:true})}
 return{normals:N,ok,edges}}
const infoCache=new WeakMap();
function meshInfo(s){let c=infoCache.get(s);if(c&&c.faces===s.faces&&c.nf===s.faces.length&&c.nv===s.vertices.length)return c;c={faces:s.faces,nf:s.faces.length,nv:s.vertices.length,...meshEdges(s)};infoCache.set(s,c);return c}

// ---- 3D picking: Mesh A (click) / Mesh B (Shift+click) ---------------------------------
function pickMesh(sx,sy){let best=null;(doc.solids||[]).forEach((s,si)=>{const P=s.vertices.map(project3);let x0=Infinity,x1=-Infinity,y0=Infinity,y1=-Infinity;for(const p of P){if(p.x<x0)x0=p.x;if(p.x>x1)x1=p.x;if(p.y<y0)y0=p.y;if(p.y>y1)y1=p.y}if(sx<x0||sx>x1||sy<y0||sy>y1)return;s.faces.forEach((f,fi)=>{const a=P[f[0]],b=P[f[1]],c=P[f[2]];if(!a||!b||!c)return;const d=(b.x-a.x)*(c.y-a.y)-(b.y-a.y)*(c.x-a.x);if(d>=0)return;const w1=((b.x-sx)*(c.y-sy)-(b.y-sy)*(c.x-sx))/d,w2=((c.x-sx)*(a.y-sy)-(c.y-sy)*(a.x-sx))/d,w3=1-w1-w2;if(w1<-1e-9||w2<-1e-9||w3<-1e-9)return;const depth=w1*a.depth+w2*b.depth+w3*c.depth;if(!best||depth<best.depth)best={index:si,face:fi,depth}})});return best}
function pickIndex(slot){const s=S.pick[slot];if(!s)return -1;const i=(doc.solids||[]).indexOf(s),sel=$(slot==='b'?'meshB':'meshA');return i>=0&&String(sel.value)===String(i)?i:-1}
function setMesh(slot,index){const s=doc.solids?.[index];if(!s)return false;if(typeof syncMeshChoices==='function'){try{syncMeshChoices()}catch(err){}}const sel=$(slot==='b'?'meshB':'meshA'),other=$(slot==='b'?'meshA':'meshB'),prev=sel.value;sel.value=String(index);S.pick[slot]=s;if(String(other.value)===String(index)&&prev!==String(index)&&doc.solids[Number(prev)]){other.value=prev;S.pick[slot==='b'?'a':'b']=null}for(const el of [sel,other])try{if(typeof Event==='function'&&el.dispatchEvent)el.dispatchEvent(new Event('change',{bubbles:true}))}catch(err){}notify(`Mesh ${slot.toUpperCase()}: ${index+1}: ${s.name}${slot==='a'?'   (Shift+click selects Mesh B)':''}`);CF.emit('meshpick',{slot,index});render();return true}
function clearPick(){if(S.pick.a||S.pick.b){S.pick.a=S.pick.b=null;if(mode3D)render()}}

// ---- 3D presentation (replaces the engine's render3D) -----------------------------------
function draw3DGrid(){if(Math.abs(Math.sin(camera3.pitch))<.08)return;const span=Math.max(W,H)/camera3.scale*.8,raw=span/10,p=10**Math.floor(Math.log10(raw)),step=raw/p<2?p:raw/p<5?2*p:5*p,n=Math.ceil(span/step),cx=Math.round(meshCenter.x/step)*step,cy=Math.round(meshCenter.y/step)*step,lo=-n*step,hi=n*step;
 const line=(a,b)=>{const p=project3(a),q=project3(b);ctx.moveTo(p.x,p.y);ctx.lineTo(q.x,q.y)};ctx.lineWidth=1;
 for(const major of [false,true]){ctx.beginPath();for(let i=-n;i<=n;i++){const x=cx+i*step,y=cy+i*step,mx=Math.round(x/step)%5===0,my=Math.round(y/step)%5===0;if(mx===major&&Math.abs(x)>step*1e-6)line(v3(x,cy+lo,0),v3(x,cy+hi,0));if(my===major&&Math.abs(y)>step*1e-6)line(v3(cx+lo,y,0),v3(cx+hi,y,0))}ctx.strokeStyle=major?'rgba(150,170,195,.16)':'rgba(150,170,195,.07)';ctx.stroke()}
 if(Math.abs(cy)<=hi){ctx.beginPath();line(v3(cx+lo,0,0),v3(cx+hi,0,0));ctx.strokeStyle='rgba(190,90,90,.55)';ctx.stroke()}if(Math.abs(cx)<=hi){ctx.beginPath();line(v3(0,cy+lo,0),v3(0,cy+hi,0));ctx.strokeStyle='rgba(90,170,100,.55)';ctx.stroke()}}
// 2D drawing entities are shown in the XY plane (z = 0) so sketches and profiles stay visible under parts.
function draw3DEntities(){const layers=new Map(doc.layers.map(l=>[l.name,l])),B=basis(camera3.yaw,camera3.pitch),sc=camera3.scale;ctx.lineWidth=1;
 for(const e of doc.entities){const l=layers.get(e.layer);if(!l?.visible)continue;ctx.strokeStyle=ctx.fillStyle=l.color;ctx.globalAlpha=e.hatch?.45:.9;
  if(e.type==='text'){const p=project3(v3(e.points[0].x,e.points[0].y,0)),k=e.height/100*sc,a=B.R.x*k,b=-B.U.x*k,c=-B.R.y*k,d=B.U.y*k;if(Math.abs(a*d-b*c)<1e-4)continue;ctx.save();ctx.transform(a,b,c,d,p.x,p.y);ctx.font='100px Segoe UI';ctx.fillText(e.text,0,0);ctx.restore();continue}
  ctx.beginPath();if(e.type==='circle'){const n=Math.max(16,Math.min(96,Math.ceil(e.radius*sc/3)));for(let i=0;i<=n;i++){const t=i/n*TAU,p=project3(v3(e.center.x+e.radius*Math.cos(t),e.center.y+e.radius*Math.sin(t),0));i?ctx.lineTo(p.x,p.y):ctx.moveTo(p.x,p.y)}}else{(e.points||[]).forEach((q,i)=>{const p=project3(v3(q.x,q.y,0));i?ctx.lineTo(p.x,p.y):ctx.moveTo(p.x,p.y)});if(e.closed)ctx.closePath()}ctx.stroke()}
 ctx.globalAlpha=1}
// Coarse depth buffer (one sample per DS pixels) of every front face; used to hide the edges that lie behind surfaces.
const DS=2;
function depthBuffer(per){const w=Math.ceil(W/DS)+2,h=Math.ceil(H/DS)+2,Z=new Float32Array(w*h).fill(Infinity),G=new Float32Array(w*h);let zmin=Infinity,zmax=-Infinity;
 for(const m of per){const F=m.s.faces,nf=F.length;for(let f=0;f<nf;f++){if(!m.front[f])continue;const t=F[f],a=m.P[t[0]],b=m.P[t[1]],c=m.P[t[2]],d=(b.x-a.x)*(c.y-a.y)-(b.y-a.y)*(c.x-a.x);if(d>-1e-9)continue;
  const i0=Math.max(0,Math.ceil(Math.min(a.x,b.x,c.x)/DS)),i1=Math.min(w-1,Math.floor(Math.max(a.x,b.x,c.x)/DS)),j0=Math.max(0,Math.ceil(Math.min(a.y,b.y,c.y)/DS)),j1=Math.min(h-1,Math.floor(Math.max(a.y,b.y,c.y)/DS));if(i1<i0||j1<j0)continue;
  const n1c=b.x*c.y-b.y*c.x,n1x=b.y-c.y,n1y=c.x-b.x,n2c=c.x*a.y-c.y*a.x,n2x=c.y-a.y,n2y=a.x-c.x,za=a.depth-c.depth,zb=b.depth-c.depth;
  const gr=Math.hypot(n1x*za+n2x*zb,n1y*za+n2y*zb)/-d;if(c.depth<zmin)zmin=c.depth;if(c.depth>zmax)zmax=c.depth;
  for(let j=j0;j<=j1;j++){const sy=j*DS;for(let i=i0;i<=i1;i++){const sx=i*DS,u=(n1c+n1x*sx+n1y*sy)/d,v=(n2c+n2x*sx+n2y*sy)/d;if(u<-1e-6||v<-1e-6||u+v>1+1e-6)continue;const z=c.depth+u*za+v*zb,k=j*w+i;if(z<Z[k]){Z[k]=z;G[k]=gr}}}}}
 Z.w=w;Z.h=h;Z.G=G;Z.span=Number.isFinite(zmin)?zmax-zmin:1;return Z}
// Appends the visible parts of the screen-space edge p-q (with depths) to the current canvas path.
function visibleSegments(p,q,Z,eps,g=ctx){const w=Z.w,h=Z.h,dx=q.x-p.x,dy=q.y-p.y;let t0=0,t1=1;
 for(const [pp,dd,lo,hi]of [[p.x,dx,0,W],[p.y,dy,0,H]]){if(Math.abs(dd)<1e-9){if(pp<lo||pp>hi)return}else{let a=(lo-pp)/dd,b=(hi-pp)/dd;if(a>b)[a,b]=[b,a];t0=Math.max(t0,a);t1=Math.min(t1,b);if(t0>t1)return}}
 const len=Math.hypot(dx,dy)*(t1-t0),n=Math.max(1,Math.ceil(len/DS));let start=null,last=null;
 for(let s=0;s<=n;s++){const t=t0+(t1-t0)*s/n,x=p.x+dx*t,y=p.y+dy*t,z=p.depth+(q.depth-p.depth)*t,ci=Math.round(x/DS),cj=Math.round(y/DS);let m=Infinity;for(let j=Math.max(0,cj-1);j<=Math.min(h-1,cj+1);j++)for(let i=Math.max(0,ci-1);i<=Math.min(w-1,ci+1);i++){const k=j*w+i,v=Z[k]+Z.G[k]*Math.hypot(x-i*DS,y-j*DS);if(v<m)m=v}
  if(z<=m+eps){const pt={x,y};if(!start)start=pt;last=pt}else if(start){if(last!==start){g.moveTo(start.x,start.y);g.lineTo(last.x,last.y)}start=null}}
 if(start&&last!==start){g.moveTo(start.x,start.y);g.lineTo(last.x,last.y)}}
render3D=function(){
 const B=basis(camera3.yaw,camera3.pitch),st=S.style,solids=doc.solids||[],shaded=st==='shaded'||st==='shadededges';
 ctx.save();ctx.setLineDash([]);ctx.globalAlpha=1;ctx.lineJoin='round';ctx.lineCap='round';
 const g=ctx.createLinearGradient(0,0,0,H);g.addColorStop(0,'#2a3240');g.addColorStop(1,'#1c222b');ctx.fillStyle=g;ctx.fillRect(0,0,W,H);
 if(CF.get('grid'))draw3DGrid();
 draw3DEntities();
 // Light: mostly a headlight, raised and from the upper left, so the three iso faces read differently.
 let L=v3(B.V.x*.75+B.U.x*.5-B.R.x*.3,B.V.y*.75+B.U.y*.5-B.R.y*.3,B.V.z*.75+B.U.z*.5-B.R.z*.3);const ll=Math.hypot(L.x,L.y,L.z);L=v3(L.x/ll,L.y/ll,L.z/ll);
 const ia=pickIndex('a'),ib=pickIndex('b'),per=[],faces=[];let tris=0;
 solids.forEach((s,si)=>{const info=meshInfo(s),P=s.vertices.map(project3),nf=s.faces.length,front=new Uint8Array(nf);tris+=nf;for(let f=0;f<nf;f++){const t=s.faces[f],a=P[t[0]],b=P[t[1]],c=P[t[2]];if(!a||!b||!c||!info.ok[f])continue;if((b.x-a.x)*(c.y-a.y)-(b.y-a.y)*(c.x-a.x)<0){front[f]=1;if(shaded)faces.push({si,f,a,b,c,depth:a.depth+b.depth+c.depth})}}per.push({s,info,P,front,rgb:hexRgb(s.color),hl:si===ia?1:si===ib?2:0})});
 const edgeColor=m=>m.hl===1?CF.colors.selection:m.hl===2?'#e8b84f':null;
 if(shaded){
  faces.sort((p,q)=>q.depth-p.depth);
  for(let k=0;k<faces.length;k++){const F=faces[k],m=per[F.si],n=m.info.normals,i=F.f*3,I=.36+.64*Math.max(0,n[i]*L.x+n[i+1]*L.y+n[i+2]*L.z);let [r,gg,b]=m.rgb.map(v=>v*I);if(m.hl===1){r=r*.7+120*.3;gg=gg*.7+180*.3;b=b*.7+255*.3}else if(m.hl===2){r=r*.75+232*.25;gg=gg*.75+184*.25;b=b*.75+79*.25}const col=`rgb(${r|0},${gg|0},${b|0})`;ctx.beginPath();ctx.moveTo(F.a.x,F.a.y);ctx.lineTo(F.b.x,F.b.y);ctx.lineTo(F.c.x,F.c.y);ctx.closePath();ctx.fillStyle=col;ctx.fill();ctx.strokeStyle=col;ctx.lineWidth=1.1;ctx.stroke()}
  // Edges are drawn after all faces and clipped by a coarse depth buffer, so hidden edges stay hidden.
  if(st==='shadededges'){const Z=depthBuffer(per),eps=2/camera3.scale+1e-4*Z.span;for(const m of per){ctx.beginPath();for(const e of m.info.edges){const A=e.f1>=0&&m.front[e.f1]===1,Bf=e.f2>=0&&m.front[e.f2]===1;if(e.feature?!(A||Bf):A===Bf)continue;visibleSegments(m.P[e.a],m.P[e.b],Z,eps)}ctx.strokeStyle=edgeColor(m)||'rgba(8,11,16,.95)';ctx.lineWidth=m.hl?1.6:1.1;ctx.stroke()}}
 }else for(const m of per){ // Wireframe / 2D Wireframe: every feature edge (hidden ones too) plus silhouettes.
  ctx.beginPath();for(const e of m.info.edges){if(!e.feature&&m.front[e.f1]===m.front[e.f2])continue;const p=m.P[e.a],q=m.P[e.b];ctx.moveTo(p.x,p.y);ctx.lineTo(q.x,q.y)}ctx.strokeStyle=edgeColor(m)||m.s.color;ctx.lineWidth=m.hl?1.6:1;ctx.stroke()}
 ctx.restore();
 $('status').textContent=`3D | ${solids.length} mesh${solids.length===1?'':'es'} | ${tris} triangles | ${STYLES[st]}`;$('entityKind').textContent='3D mesh workspace'};
// Zoom extents in 3D covers meshes and the 2D drawing (at z = 0).
fit3D=function(){let x0=Infinity,y0=Infinity,z0=Infinity,x1=-Infinity,y1=-Infinity,z1=-Infinity;const add=(x,y,z)=>{if(x<x0)x0=x;if(x>x1)x1=x;if(y<y0)y0=y;if(y>y1)y1=y;if(z<z0)z0=z;if(z>z1)z1=z};
 for(const s of doc.solids||[])for(const p of s.vertices)add(p.x,p.y,p.z);
 for(const e of doc.entities){if(!CF.visible(e))continue;if(e.type==='circle'){add(e.center.x-e.radius,e.center.y-e.radius,0);add(e.center.x+e.radius,e.center.y+e.radius,0)}else for(const p of e.points||[])add(p.x,p.y,0)}
 if(!Number.isFinite(x0))return;meshCenter={x:(x0+x1)/2,y:(y0+y1)/2,z:(z0+z1)/2};camera3.scale=Math.min(W,H)*.7/Math.max(1,Math.hypot(x1-x0,y1-y0,z1-z0))};
function pan3D(dx,dy){const B=basis(camera3.yaw,camera3.pitch),s=camera3.scale;meshCenter={x:meshCenter.x-B.R.x*dx/s+B.U.x*dy/s,y:meshCenter.y-B.R.y*dx/s+B.U.y*dy/s,z:meshCenter.z-B.R.z*dx/s+B.U.z*dy/s}}
// Zoom about a screen point: the model point under the cursor stays put.
function zoom3DAt(sx,sy,f){const B=basis(camera3.yaw,camera3.pitch),s1=camera3.scale,s2=Math.max(1e-6,Math.min(1e6,s1*f)),cx=sx-W/2,cy=sy-H/2,a=cx/s1-cx/s2,b=cy/s2-cy/s1;meshCenter={x:meshCenter.x+B.R.x*a+B.U.x*b,y:meshCenter.y+B.R.y*a+B.U.y*b,z:meshCenter.z+B.R.z*a+B.U.z*b};camera3.scale=s2}

// ---- Layout (paper) space ---------------------------------------------------------------
// Sheet settings come from the Plot dialog (sheetConfig); fall back to A4 landscape, fit, when they are unusable.
function sheet(){let c=null;try{if(typeof sheetConfig==='function')c=sheetConfig()}catch(err){c=null}if(c&&[c.width,c.height,c.scale,c.offsetX,c.offsetY].every(Number.isFinite)&&c.scale>0)return c;const width=297,height=210,margin=12,entities=doc.entities.filter(e=>CF.visible(e)),b=typeof entityBounds==='function'?entityBounds(entities):{minX:0,minY:0,maxX:100,maxY:100},scale=Math.min((width-margin*2)/Math.max(1,b.maxX-b.minX),(height-margin*2)/Math.max(1,b.maxY-b.minY));return{width,height,scale,offsetX:(width-(b.maxX-b.minX)*scale)/2-b.minX*scale,offsetY:(height-(b.maxY-b.minY)*scale)/2-b.minY*scale,entities,overflow:false}}
const modelToPaper=(p,c=sheet())=>({x:c.offsetX+p.x*c.scale,y:c.height-c.offsetY-p.y*c.scale}); // mm, Y down (like sheetSVG)
const paperToScreen=p=>({x:W/2+(p.x-S.layout.x)*S.layout.s,y:H/2+(p.y-S.layout.y)*S.layout.s});
const screenToPaper=p=>({x:S.layout.x+(p.x-W/2)/S.layout.s,y:S.layout.y+(p.y-H/2)/S.layout.s});
function fitLayout(c=sheet()){const L=S.layout;if(W<20||H<20)return;L.s=Math.max(.02,Math.min((W-56)/c.width,(H-56)/c.height));L.x=c.width/2;L.y=c.height/2;L.fitted=true;L.sig=c.width+'x'+c.height}
function zoomLayoutAt(sx,sy,f){const L=S.layout,p=screenToPaper({x:sx,y:sy});L.s=Math.max(.02,Math.min(500,L.s*f));L.x=p.x-(sx-W/2)/L.s;L.y=p.y-(sy-H/2)/L.s}
function renderLayout(){
 const c=sheet(),L=S.layout;if(!L.fitted||L.sig!==c.width+'x'+c.height)fitLayout(c);
 ctx.save();ctx.setLineDash([]);ctx.globalAlpha=1;ctx.fillStyle=CF.colors.paperBackground;ctx.fillRect(0,0,W,H);
 const a=paperToScreen({x:0,y:0}),b=paperToScreen({x:c.width,y:c.height}),w=b.x-a.x,h=b.y-a.y;
 ctx.fillStyle='rgba(0,0,0,.18)';ctx.fillRect(a.x+3,a.y+3,w+3,h+3);ctx.fillStyle='rgba(0,0,0,.32)';ctx.fillRect(a.x+5,a.y+5,w,h);
 ctx.fillStyle=CF.colors.paper;ctx.fillRect(a.x,a.y,w,h);
 const pm=5,p1=paperToScreen({x:pm,y:pm}),p2=paperToScreen({x:c.width-pm,y:c.height-pm});ctx.lineWidth=1;ctx.strokeStyle='#9aa3ae';ctx.setLineDash([6,4]);ctx.strokeRect(p1.x+.5,p1.y+.5,p2.x-p1.x,p2.y-p1.y);ctx.setLineDash([]);
 const vm=12,v1=paperToScreen({x:vm,y:vm}),v2=paperToScreen({x:c.width-vm,y:c.height-vm});ctx.strokeStyle='#6b7584';ctx.strokeRect(v1.x+.5,v1.y+.5,v2.x-v1.x,v2.y-v1.y);
 ctx.beginPath();ctx.rect(v1.x,v1.y,v2.x-v1.x,v2.y-v1.y);ctx.clip();
 const k=L.s,P=q=>paperToScreen(modelToPaper(q,c));ctx.lineWidth=Math.max(.75,.25*k);
 for(const e of c.entities){ctx.strokeStyle=ctx.fillStyle=e.hatch?'#8c8c8c':'#141414';
  if(e.type==='text'){const px=e.height*c.scale*k;if(px<.6)continue;const p=P(e.points[0]);ctx.font=`${px}px Arial`;ctx.fillText(e.text,p.x,p.y);continue}
  ctx.beginPath();if(e.type==='circle'){const p=P(e.center);ctx.arc(p.x,p.y,Math.max(.1,e.radius*c.scale*k),0,TAU)}else{e.points.forEach((q,i)=>{const p=P(q);i?ctx.lineTo(p.x,p.y):ctx.moveTo(p.x,p.y)});if(e.closed)ctx.closePath()}ctx.stroke()}
 ctx.restore();
 ctx.save();ctx.fillStyle='#c4cbd4';ctx.font='11px Segoe UI';const paper=String($('sheetPaper')?.value||'A4'),fixed=Number($('sheetScale')?.value),scaleText=Number.isFinite(fixed)&&fixed>0?`1:${fixed}`:'Scaled to fit';if(b.y+20<H)ctx.fillText(`${/^(A3|A4|LETTER)$/i.test(paper)?paper:'A4'} ${c.width>c.height?'Landscape':'Portrait'}  |  ${scaleText}  |  Double-click the sheet for Page Setup / Plot`,a.x,b.y+18);ctx.restore()}

// ---- UCS icon (lower-left of the drawing area) --------------------------------------------
function arrow(x0,y0,x1,y1,color,label){const l=Math.hypot(x1-x0,y1-y0);ctx.strokeStyle=ctx.fillStyle=color;if(l<3){ctx.beginPath();ctx.arc(x0,y0,2.5,0,TAU);ctx.fill();return}const ux=(x1-x0)/l,uy=(y1-y0)/l;ctx.beginPath();ctx.moveTo(x0,y0);ctx.lineTo(x1-ux*7,y1-uy*7);ctx.stroke();ctx.beginPath();ctx.moveTo(x1,y1);ctx.lineTo(x1-ux*9-uy*3.5,y1-uy*9+ux*3.5);ctx.lineTo(x1-ux*9+uy*3.5,y1-uy*9-ux*3.5);ctx.closePath();ctx.fill();if(label){ctx.font='bold 11px Segoe UI';ctx.textAlign='center';ctx.textBaseline='middle';ctx.fillText(label,x1+ux*9,y1+uy*9)}}
function drawUCS(kind){ctx.save();ctx.setLineDash([]);ctx.globalAlpha=1;ctx.lineWidth=1.6;ctx.lineCap='butt';const ox=30,oy=H-30,L=40;
 if(kind==='layout'){ctx.strokeStyle='#e3e8ee';ctx.beginPath();ctx.moveTo(ox,oy);ctx.lineTo(ox+L,oy);ctx.lineTo(ox,oy-L);ctx.closePath();ctx.stroke();ctx.fillStyle='#e3e8ee';ctx.font='bold 11px Segoe UI';ctx.textAlign='center';ctx.textBaseline='middle';ctx.fillText('X',ox+L+9,oy);ctx.fillText('Y',ox,oy-L-9)}
 else if(kind==='2d'){arrow(ox,oy,ox+L,oy,'#e0605a','X');arrow(ox,oy,ox,oy-L,'#5cc87a','Y');ctx.strokeStyle='#dfe5ec';ctx.lineWidth=1.2;ctx.strokeRect(ox-.5,oy-6.5,7,7)}
 else{const B=basis(camera3.yaw,camera3.pitch),axes=[[v3(1,0,0),'#e0605a','X'],[v3(0,1,0),'#5cc87a','Y'],[v3(0,0,1),'#4fa3e8','Z']].map(([a,c,n])=>({x:ox+dot(B.R,a)*L,y:oy-dot(B.U,a)*L,depth:-dot(B.V,a),c,n}));axes.sort((p,q)=>q.depth-p.depth);for(const a of axes)arrow(ox,oy,a.x,a.y,a.c,a.n);ctx.fillStyle='#dfe5ec';ctx.fillRect(ox-2,oy-2,4,4)}
 ctx.restore()}

// ---- DOM chrome: viewport controls, ViewCube, navigation bar --------------------------------
const ui={};
const mk=(tag,props={})=>Object.assign(document.createElement(tag),props);
const svg=(body,size=18)=>`<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${body}</svg>`;
const ICONS={pan:svg('<path d="M8 12V6.5a1.5 1.5 0 0 1 3 0V11M11 10.5V5a1.5 1.5 0 0 1 3 0v5.5M14 10V6.5a1.5 1.5 0 0 1 3 0v6.5c0 4-2.5 7-6 7-2.5 0-3.8-1.2-5.2-3.4L4.2 13.8a1.4 1.4 0 0 1 2.3-1.6L8 14"/>'),'zoom-extents':svg('<circle cx="10.5" cy="10.5" r="5.5"/><path d="M15 15l5 5"/><path d="M8 8.5h5v4H8z" stroke="#4fa3e8"/><path d="M3 6V3h3M18 3h3v3M3 18v3h3"/>'),orbit:svg('<circle cx="12" cy="12" r="4.5" stroke="#4fa3e8"/><ellipse cx="12" cy="12" rx="9.5" ry="4" transform="rotate(-20 12 12)"/><path d="M19.6 6.2l1.6 1.4-2 .9"/>'),home:svg('<path d="M4 11.5L12 4.5l8 7"/><path d="M6.5 10v9h11v-9"/><path d="M10.5 19v-5h3v5" stroke="#4fa3e8"/>',16),check:svg('<path d="M5 12.5l4.5 4.5L19 7.5"/>',14),drop:'<svg width="8" height="8" viewBox="0 0 8 8" aria-hidden="true"><path d="M1 2.5h6L4 6z" fill="currentColor"/></svg>'};
const icon=(name,size=18)=>CF.has('icons')?CF.icon(name,size):ICONS[name]||CF.icon(name,size);
const CSS=`
#cf-vpctl{position:absolute;left:6px;top:4px;display:flex;font:12px var(--cf-font);color:#c9d1db;pointer-events:auto;user-select:none;z-index:5;text-shadow:0 1px 1px rgba(0,0,0,.6)}
#cf-vpctl span{padding:1px 1px;cursor:pointer;border-radius:2px;white-space:nowrap}
#cf-vpctl span:hover,#cf-vpctl span.open{color:#fff;background:rgba(74,144,217,.32)}
#cf-viewcube{position:absolute;top:4px;right:8px;width:${CUBE.size}px;pointer-events:auto;opacity:.72;transition:opacity .12s;user-select:none;z-index:4}
#cf-viewcube:hover,#cf-viewcube.drag{opacity:1}
#cf-viewcube canvas{display:block;width:${CUBE.size}px;height:${CUBE.size}px;cursor:default;touch-action:none}
#cf-viewcube .cf-vc-home{position:absolute;left:10px;top:4px;width:22px;height:22px;padding:2px;border:1px solid transparent;background:transparent;color:#c9d1db;display:flex;align-items:center;justify-content:center;border-radius:2px}
#cf-viewcube .cf-vc-home:hover{border-color:var(--cf-hover-border);background:rgba(69,81,95,.85);color:#fff}
#cf-viewcube .cf-vc-wcs{margin:-4px auto 0;width:max-content;font:11px var(--cf-font);color:#c9d1db;background:rgba(43,50,60,.9);border:1px solid #4a5260;padding:0 9px;border-radius:2px;line-height:16px}
#cf-navbar{position:absolute;right:16px;top:158px;display:flex;flex-direction:column;background:rgba(43,50,60,.9);border:1px solid #4a5260;border-radius:3px;pointer-events:auto;padding:2px 0;z-index:4;box-shadow:0 2px 8px rgba(0,0,0,.3)}
#cf-navbar .cf-nb-row{display:flex;align-items:stretch}
#cf-navbar button{border:0;background:transparent;border-radius:0;padding:4px 2px 4px 5px;color:#dfe5ec;display:flex;align-items:center;justify-content:center;min-width:28px;height:28px}
#cf-navbar button.cf-nb-drop{min-width:11px;width:11px;padding:0;color:#a9b3bf}
#cf-navbar button:hover{background:var(--cf-hover)}#cf-navbar button.active{background:var(--cf-active)}
#cf-navbar .cf-nb-sep{height:1px;background:#4a5260;margin:2px 4px}
.cf-vw-menu{position:absolute;min-width:178px;background:#2b323c;border:1px solid #4a5260;box-shadow:var(--cf-shadow);padding:3px 0;pointer-events:auto;z-index:30;font:12px var(--cf-font);color:var(--cf-text);user-select:none}
.cf-vw-menu .cf-vw-item{display:flex;align-items:center;gap:4px;padding:3px 20px 3px 4px;white-space:nowrap;cursor:default}
.cf-vw-menu .cf-vw-item:hover{background:var(--cf-active)}
.cf-vw-menu .cf-vw-ck{width:18px;display:flex;justify-content:center;color:var(--cf-accent)}
.cf-vw-menu .cf-vw-sep{height:1px;background:#4a5260;margin:3px 2px}
.cf-vw-menu .cf-vw-head{padding:3px 8px 2px;color:var(--cf-text-faint);font-size:11px}
`;
function closeMenu(){if(S.menu){S.menu.remove();S.menu=null}S.menuAnchor?.classList.remove('open');S.menuAnchor=null}
// items: {label, run, checked} | '-' | {head}. align 'below' (default) or 'left' (opens to the left of the anchor).
function openMenu(anchor,items,align='below'){const same=S.menuAnchor===anchor;closeMenu();if(same)return;const m=mk('div',{className:'cf-vw-menu'});for(const it of items){if(it==='-'){m.append(mk('div',{className:'cf-vw-sep'}));continue}if(it.head){m.append(mk('div',{className:'cf-vw-head',textContent:it.head}));continue}const row=mk('div',{className:'cf-vw-item'}),ck=mk('span',{className:'cf-vw-ck'});ck.innerHTML=it.checked?ICONS.check:'';row.append(ck,mk('span',{textContent:it.label}));row.onclick=e=>{e?.stopPropagation?.();closeMenu();it.run()};m.append(row)}CF.hosts.overlay.append(m);try{const r=anchor.getBoundingClientRect(),o=CF.hosts.overlay.getBoundingClientRect();if(align==='left'){m.style.right=(o.right-r.left+4)+'px';m.style.top=Math.max(0,r.top-o.top)+'px'}else{m.style.left=(r.left-o.left)+'px';m.style.top=(r.bottom-o.top+2)+'px'}}catch(err){}S.menu=m;S.menuAnchor=anchor;anchor.classList.add('open')}
const flagItem=(label,flag)=>({label,checked:CF.get(flag),run:()=>CF.toggle(flag)});
function viewMenuItems(){const cur=mode3D?presetOf(camera3.yaw,camera3.pitch):isLayout()?null:'top',it=k=>({label:PRESETS[k].label,checked:cur===k,run:()=>CF.setViewPreset(k)}),items=[{head:'Preset views'},...['top','bottom','left','right','front','back'].map(it),'-',...['sw','se','ne','nw'].map(it)];if(CF.resolve('VIEW'))items.push('-',{label:'View Manager...',run:()=>CF.run('VIEW',{source:'viewport'})});return items}
function styleMenuItems(){const cur=mode3D?S.style:'2dwireframe';return Object.entries(STYLES).map(([k,label])=>({label,checked:cur===k,run:()=>CF.setVisualStyle(k)}))}
function zoomCmd(opt){if(!CF.resolve('ZOOM')||!CF.commandLine?.submit)return false;CF.run('ZOOM',{source:'navbar'});const asking=()=>!!CF.input||/Extents/i.test(String(CF.prompt?.()||''));if(asking())CF.commandLine.submit(opt);else Promise.resolve().then(()=>{if(asking())CF.commandLine.submit(opt)});return true}
function zoomBy(f){if(isLayout())zoomLayoutAt(W/2,H/2,f);else if(mode3D)zoom3DAt(W/2,H/2,f);else{view.scale=Math.max(.01,Math.min(1000,view.scale*f))}render()}
function zoom(kind){if(kind==='in')return zoomBy(2);if(kind==='out')return zoomBy(.5);if(kind==='extents'){if(isLayout()){fitLayout();render()}else if(mode3D){fit3D();render()}else if(!zoomCmd('E'))fit();return}if(isLayout()||mode3D){notify(`Zoom ${kind==='window'?'Window':'Previous'} is available in 2D model space.`);return}if(!zoomCmd(kind==='window'?'W':'P')){if(kind==='window')fit();else notify('Zoom Previous needs the command line.')}}
function enterPan(){S.panMode=true;canvas.style.cursor='grab';notify('Press ESC or ENTER to exit, or right-click to display shortcut menu.');syncNav()}
function exitPan(){if(!S.panMode)return;S.panMode=false;canvas.style.cursor=mode3D?'default':isLayout()?'crosshair':'';syncNav()}
function pan(){if(mode3D||isLayout())enterPan();else CF.startPan()}
function orbit(){if(isLayout())CF.setSpace('model');if(!mode3D)goCamera({yaw:camera3.yaw,pitch:camera3.pitch});exitPan();notify('Drag to orbit. Press ESC or ENTER to exit, or right-click to display shortcut menu.')}
function syncNav(){ui.panBtn?.classList.toggle('active',S.panMode)}
function buildChrome(){
 const st=mk('style',{id:'cf-views-style',textContent:CSS});document.head.append(st);const ov=CF.hosts.overlay;
 // Viewport controls [-][Top][2D Wireframe]
 ui.vp=mk('div',{id:'cf-vpctl'});ui.vpMin=mk('span',{textContent:'[\u2013]',title:'Viewport controls'});ui.vpView=mk('span',{textContent:'[Top]',title:'View controls'});ui.vpStyle=mk('span',{textContent:'[2D Wireframe]',title:'Visual Style controls'});ui.vp.append(ui.vpMin,ui.vpView,ui.vpStyle);
 ui.vpMin.onclick=()=>openMenu(ui.vpMin,[flagItem('ViewCube','viewCube'),flagItem('Navigation Bar','navBar'),flagItem('UCS Icon','ucsIcon'),'-',{label:'Zoom Extents',run:()=>zoom('extents')},{label:'Home (SE Isometric)',run:goHome}]);
 ui.vpView.onclick=()=>openMenu(ui.vpView,viewMenuItems());ui.vpStyle.onclick=()=>openMenu(ui.vpStyle,styleMenuItems());
 // ViewCube
 ui.cube=mk('div',{id:'cf-viewcube'});ui.cubeCanvas=mk('canvas',{width:CUBE.size,height:CUBE.size,title:'ViewCube: click a face, edge or corner; drag to orbit'});ui.cubeCtx=ui.cubeCanvas.getContext('2d');ui.home=mk('button',{className:'cf-vc-home',title:'Home (SE Isometric)'});ui.home.innerHTML=ICONS.home;ui.home.onclick=goHome;ui.wcs=mk('div',{className:'cf-vc-wcs',textContent:'WCS',title:'World Coordinate System'});ui.cube.append(ui.cubeCanvas,ui.home,ui.wcs);
 const cc=ui.cubeCanvas;
 cc.onpointerdown=e=>{if(e.button!==0)return;e.preventDefault?.();const hit=cubeHit(e.offsetX,e.offsetY);S.cubeDrag={x:e.clientX,y:e.clientY,hit,moved:false};try{cc.setPointerCapture(e.pointerId)}catch(err){}ui.cube.classList.add('drag')};
 cc.onpointermove=e=>{const d=S.cubeDrag;if(!d){const hit=cubeHit(e.offsetX,e.offsetY),key=hit?hit.kind+(hit.d||hit.dir||''):'';if(key!==(S.hover?S.hover.key:'')){S.hover=hit?{...hit,key}:null;drawCube()}cc.style.cursor=hit?'pointer':'default';return}
  if(!d.moved&&Math.hypot(e.clientX-d.x,e.clientY-d.y)>3){d.moved=true;S.tween=null;if(!mode3D){camera3.yaw=0;camera3.pitch=HALF-.02;if(isLayout())CF.setSpace('model');show3D()}d.yaw=camera3.yaw;d.pitch=camera3.pitch;d.x=e.clientX;d.y=e.clientY}
  if(d.moved){camera3.yaw=d.yaw+(e.clientX-d.x)*.012;if(d.hit?.kind!=='ring'&&d.hit?.kind!=='letter')camera3.pitch=clampPitch(d.pitch+(e.clientY-d.y)*.012);render()}};
 cc.onpointerup=e=>{const d=S.cubeDrag;S.cubeDrag=null;ui.cube.classList.remove('drag');try{cc.releasePointerCapture(e.pointerId)}catch(err){}if(d&&!d.moved)cubeClick(d.hit);else if(d)emitView()};
 cc.onpointercancel=()=>{S.cubeDrag=null;ui.cube.classList.remove('drag')};
 cc.onpointerleave=()=>{if(S.hover&&!S.cubeDrag){S.hover=null;drawCube()}};
 cc.oncontextmenu=e=>e.preventDefault?.();
 // Navigation bar
 ui.nav=mk('div',{id:'cf-navbar'});const row=(main,dropItems)=>{const r=mk('div',{className:'cf-nb-row'});r.append(main);if(dropItems){const dd=mk('button',{className:'cf-nb-drop',title:main.title+' options'});dd.innerHTML=ICONS.drop;dd.onclick=()=>openMenu(dd,dropItems(),'left');r.append(dd)}else main.style.paddingRight='13px';return r};
 const btn=(iconName,title,run)=>{const b=mk('button',{title});b.innerHTML=icon(iconName);b.onclick=run;return b};
 ui.panBtn=btn('pan','Pan (PAN)',pan);ui.zoomBtn=btn('zoom-extents','Zoom Extents (ZOOM E)',()=>zoom('extents'));ui.orbitBtn=btn('orbit','Orbit (3DORBIT)',orbit);
 ui.nav.append(row(ui.panBtn),mk('div',{className:'cf-nb-sep'}),row(ui.zoomBtn,()=>[{label:'Zoom Extents',run:()=>zoom('extents')},{label:'Zoom Window',run:()=>zoom('window')},{label:'Zoom Previous',run:()=>zoom('previous')},'-',{label:'Zoom In',run:()=>zoom('in')},{label:'Zoom Out',run:()=>zoom('out')}]),mk('div',{className:'cf-nb-sep'}),row(ui.orbitBtn,()=>[{label:'Orbit',run:orbit},{label:'Home (SE Isometric)',run:goHome},{label:'Plan View (Top)',run:()=>CF.setViewPreset('top')}]));
 ov.append(ui.vp,ui.cube,ui.nav);
 document.addEventListener('pointerdown',e=>{if(!S.menu)return;const t=e.target;if(S.menu.contains?.(t)||S.menuAnchor?.contains?.(t))return;closeMenu()},true);
 document.addEventListener('keydown',e=>{if(e.key==='Escape'||e.key==='Enter'){if(S.menu)closeMenu();if(S.panMode&&!(e.key==='Enter'&&e.target?.matches?.('input,textarea')&&e.target.value))exitPan()}});
 window.addEventListener?.('blur',closeMenu)}
function goHome(){CF.setViewPreset('se');fit3D();render()}
function updateChrome(){if(!ui.vp)return;const layout=isLayout(),vn=viewName(),sl=styleLabel(),sig=[layout,mode3D,CF.get('viewCube'),CF.get('navBar'),vn,sl].join('|');if(sig!==S.chrome){S.chrome=sig;ui.vp.style.display=layout?'none':'';ui.vpView.textContent=`[${vn}]`;ui.vpStyle.textContent=`[${sl}]`;const cubeOn=CF.get('viewCube')&&!layout;ui.cube.style.display=cubeOn?'':'none';ui.nav.style.display=CF.get('navBar')?'':'none';ui.nav.style.top=cubeOn?'158px':'8px';ui.orbitBtn.style.opacity=layout?'.45':'';if(S.menu)closeMenu();emitView()}drawCube()}

// ---- ViewCube geometry -------------------------------------------------------------------
const cross=(a,b)=>v3(a.y*b.z-a.z*b.y,a.z*b.x-a.x*b.z,a.x*b.y-a.y*b.x);
// Face label "up" vectors follow the ViewCube convention: side faces read upright, TOP reads with FRONT at the bottom.
const FACES=[['TOP',[0,0,1],[0,1,0]],['BOTTOM',[0,0,-1],[0,-1,0]],['FRONT',[0,-1,0],[0,0,1]],['BACK',[0,1,0],[0,0,1]],['RIGHT',[1,0,0],[0,0,1]],['LEFT',[-1,0,0],[0,0,1]]].map(([label,n,u])=>{n=v3(...n);u=v3(...u);return{label,n,u,r:cross(u,n)}});
const cubeCam=()=>mode3D?{yaw:camera3.yaw,pitch:camera3.pitch}:{yaw:0,pitch:HALF};
const zone=v=>v>.6?1:v<-.6?-1:0;
function faceFrame(F,B){const h=CUBE.h;return{cx:CUBE.cx+dot(B.R,F.n)*h,cy:CUBE.cy-dot(B.U,F.n)*h,ax:dot(B.R,F.r)*h,ay:-dot(B.U,F.r)*h,bx:dot(B.R,F.u)*h,by:-dot(B.U,F.u)*h}}
const RING={inner:1.62,outer:2.02,z:-1.02};
function letterPos(B){const h=CUBE.h,r=(RING.inner+RING.outer)/2*h,z=RING.z*h;return[['N',0,1],['E',1,0],['S',0,-1],['W',-1,0]].map(([dir,x,y])=>{const p=v3(x*r,y*r,z);return{dir,x:CUBE.cx+dot(B.R,p),y:CUBE.cy-dot(B.U,p),depth:-dot(B.V,p)}})}
// Hit-test the cube (3x3 zones per face: faces, edges, corners) and the compass ring.
function cubeHit(x,y,cam=cubeCam()){const B=basis(cam.yaw,cam.pitch);
 for(const F of FACES){if(dot(F.n,B.V)<=1e-3)continue;const f=faceFrame(F,B),det=f.ax*f.by-f.ay*f.bx;if(Math.abs(det)<1e-6)continue;const dx=x-f.cx,dy=y-f.cy,s=(dx*f.by-dy*f.bx)/det,t=(f.ax*dy-f.ay*dx)/det;if(Math.abs(s)>1.0001||Math.abs(t)>1.0001)continue;const rs=zone(s),rt=zone(t);return{kind:'zone',face:F.label,d:['x','y','z'].map(k=>F.n[k]+rs*F.r[k]+rt*F.u[k]+0)}}
 for(const l of letterPos(B))if(Math.hypot(x-l.x,y-l.y)<8)return{kind:'letter',dir:l.dir};
 const sp=Math.sin(cam.pitch);if(Math.abs(sp)>.15){const h=CUBE.h,a=x-CUBE.cx,b=-(y-CUBE.cy),cp=Math.cos(cam.pitch),c2=(b-RING.z*h*cp)/sp,cy=Math.cos(cam.yaw),sy=Math.sin(cam.yaw),wx=a*cy+c2*sy,wy=-a*sy+c2*cy,r=Math.hypot(wx,wy)/h;if(r>RING.inner*.92&&r<RING.outer*1.12)return{kind:'ring'}}
 return null}
function cubeClick(hit){if(!hit)return;if(hit.kind==='zone'){const k=presetFromDirection(hit.d);if(k)CF.setViewPreset(k);else goCamera(cameraFromDirection(hit.d))}else if(hit.kind==='letter'){goCamera({yaw:{N:PI,S:0,E:-HALF,W:HALF}[hit.dir],pitch:mode3D?camera3.pitch:ISO})}}
function drawCube(){const c=ui.cubeCtx;if(!c||ui.cube.style.display==='none')return;const cam=cubeCam(),hv=S.hover,sig=[cam.yaw.toFixed(4),cam.pitch.toFixed(4),hv?.key||''].join('|');if(sig===S.cubeSig)return;S.cubeSig=sig;
 const B=basis(cam.yaw,cam.pitch),N=CUBE.size,dpr=typeof devicePixelRatio==='number'&&devicePixelRatio>0?devicePixelRatio:1,h=CUBE.h,P=p=>({x:CUBE.cx+dot(B.R,p),y:CUBE.cy-dot(B.U,p),depth:-dot(B.V,p)});
 if(ui.cubeCanvas.width!==Math.round(N*dpr)){ui.cubeCanvas.width=Math.round(N*dpr);ui.cubeCanvas.height=Math.round(N*dpr)}c.setTransform(dpr,0,0,dpr,0,0);c.clearRect(0,0,N,N);c.lineJoin='round';
 // Compass ring split into far and near halves so the cube sits "inside" it.
 const segs=[],n=48,z=RING.z*h,center=P(v3(0,0,z)).depth;for(let i=0;i<n;i++){const a0=i/n*TAU,a1=(i+1)/n*TAU,q=[[a0,RING.inner],[a1,RING.inner],[a1,RING.outer],[a0,RING.outer]].map(([a,r])=>P(v3(Math.cos(a)*r*h,Math.sin(a)*r*h,z)));segs.push({q,depth:P(v3(Math.cos((a0+a1)/2)*1.8*h,Math.sin((a0+a1)/2)*1.8*h,z)).depth})}
 const ringHot=hv&&(hv.kind==='ring'||hv.kind==='letter'),drawRing=list=>{if(!list.length)return;c.beginPath();for(const s of list){c.moveTo(s.q[0].x,s.q[0].y);for(const p of s.q.slice(1))c.lineTo(p.x,p.y);c.closePath()}c.fillStyle=ringHot?'#6f8299':'#59636f';c.fill();c.strokeStyle=c.fillStyle;c.lineWidth=.6;c.stroke()};
 const letters=letterPos(B),drawLetters=list=>{c.font='bold 9px Segoe UI';c.textAlign='center';c.textBaseline='middle';for(const l of list){c.fillStyle=hv?.kind==='letter'&&hv.dir===l.dir?'#ffffff':'#d7dde5';c.fillText(l.dir,l.x,l.y+.5)}};
 drawRing(segs.filter(s=>s.depth>=center-1e-9));drawLetters(letters.filter(l=>l.depth>=center-1e-9));
 const L0=v3(B.V.x*.8+B.U.x*.5-B.R.x*.2,B.V.y*.8+B.U.y*.5-B.R.y*.2,B.V.z*.8+B.U.z*.5-B.R.z*.2),ll=Math.hypot(L0.x,L0.y,L0.z);
 for(const F of FACES){const vis=dot(F.n,B.V);if(vis<=1e-3)continue;const f=faceFrame(F,B),pt=(s,t)=>({x:f.cx+s*f.ax+t*f.bx,y:f.cy+s*f.ay+t*f.by}),quad=(s0,s1,t0,t1)=>{const ps=[pt(s0,t0),pt(s1,t0),pt(s1,t1),pt(s0,t1)];c.beginPath();ps.forEach((p,i)=>i?c.lineTo(p.x,p.y):c.moveTo(p.x,p.y));c.closePath()};
  const I=.74+.26*Math.max(0,dot(F.n,L0)/ll);quad(-1,1,-1,1);c.fillStyle=`rgb(${214*I|0},${220*I|0},${228*I|0})`;c.fill();
  if(hv?.kind==='zone'){const d=v3(...hv.d);if(dot(d,F.n)===1){const rs=dot(d,F.r),rt=dot(d,F.u),rg=v=>v<0?[-1,-.6]:v>0?[.6,1]:[-.6,.6],[s0,s1]=rg(rs),[t0,t1]=rg(rt);quad(s0,s1,t0,t1);c.fillStyle='rgba(74,144,217,.62)';c.fill()}}
  quad(-1,1,-1,1);c.strokeStyle='#5a6574';c.lineWidth=1;c.stroke();
  if(vis>.12){const fs=F.label.length>4?8.5:10;c.save();c.transform(f.ax/h,f.ay/h,-f.bx/h,-f.by/h,f.cx,f.cy);c.font=`600 ${fs}px Segoe UI`;c.textAlign='center';c.textBaseline='middle';c.fillStyle='#2b323c';c.fillText(F.label,0,.5);c.restore()}}
 drawRing(segs.filter(s=>s.depth<center-1e-9));drawLetters(letters.filter(l=>l.depth<center-1e-9))}

// ---- Render wrapper: layout space, tweened camera, UCS icon, chrome refresh ------------------
// In paper space the model-space drawing passes are skipped (the sheet is painted over the whole canvas instead).
{const g=drawGrid,p=drawPreview,c=drawCursor,d=drawEntity;drawGrid=function(...a){if(!S.suppress)return g.apply(this,a)};drawPreview=function(...a){if(!S.suppress)return p.apply(this,a)};drawCursor=function(...a){if(!S.suppress)return c.apply(this,a)};drawEntity=function(...a){if(!S.suppress)return d.apply(this,a)}}
function syncMode(){const m=isLayout()?'layout':mode3D?'3d':'2d';if(m===S.mode)return;const was=S.mode;S.mode=m;if(was)S.panMode=false;try{canvas.style.cursor=m==='layout'?'crosshair':m==='3d'?'default':''}catch(err){}syncNav()}
const vwPrevRender=render;
render=function(){if(S.hold)return;syncMode();if(isLayout()){S.suppress=true;try{vwPrevRender()}finally{S.suppress=false}renderLayout();if(CF.get('ucsIcon'))drawUCS('layout');updateChrome();return}
 withTween(()=>{vwPrevRender();if(CF.get('ucsIcon'))drawUCS(mode3D?'3d':'2d');updateChrome()})};
// Mode changes refresh the chrome; leaving layout for a 3D view goes through model space.
{const prev=show3D;show3D=function(...a){if(CF.space==='layout'){CF.space='model';CF.emit('space','model')}const r=prev.apply(this,a);emitView();return r}}
{const prev=showModel;showModel=function(...a){S.tween=null;const r=prev.apply(this,a);emitView();return r}}
// Selecting the Select tool (Esc / CF.cancel) keeps the 3D view; drawing tools still return to 2D model space.
{const prev=setTool;setTool=function(t){if(mode3D&&t==='select'){S.hold=true;mode3D=false;try{prev(t)}finally{mode3D=true;S.hold=false}render();return}return prev(t)}}
// Zoom extents while in paper space also fits the sheet.
{const prev=fit;fit=function(...a){const r=prev.apply(this,a);if(isLayout()){fitLayout();render()}return r}}
{const prev=CF.startPan;CF.startPan=function(...a){if(mode3D||isLayout())return enterPan();return prev.apply(CF,a)}}
{const prev=CF.cancel;CF.cancel=function(...a){exitPan();closeMenu();if(mode3D)clearPick();return prev.apply(CF,a)}}
CF.on('space',s=>{if(s==='layout'&&!S.layout.fitted)fitLayout();closeMenu()});
CF.on('state',e=>{if(['viewCube','navBar','ucsIcon'].includes(e?.name)){S.chrome='';S.cubeSig=''}});
for(const id of ['meshA','meshB'])try{$(id).addEventListener('change',()=>{const i=Number($(id).value);S.pick[id==='meshA'?'a':'b']=doc.solids?.[i]||null;if(mode3D)render()})}catch(err){}

// ---- Canvas input in 3D and layout space (2D model input goes to the handlers installed earlier) ----
const H0={down:canvas.onpointerdown,move:canvas.onpointermove,up:canvas.onpointerup,cancel:canvas.onpointercancel,wheel:canvas.onwheel,menu:canvas.oncontextmenu,dbl:canvas.ondblclick};
const call=(fn,e)=>typeof fn==='function'?fn.call(canvas,e):undefined,mine=()=>mode3D||CF.space==='layout';
canvas.onpointerdown=e=>{if(!mine())return call(H0.down,e);closeMenu();try{canvas.setPointerCapture(e.pointerId)}catch(err){}S.tween=null;const now=performance.now();
 if(e.button===1){const m=S.lastMid;if(m&&now-m.t<350&&Math.hypot(e.clientX-m.x,e.clientY-m.y)<6){S.lastMid=null;zoom('extents');return}S.lastMid={t:now,x:e.clientX,y:e.clientY}} // double middle click = extents
 const kind=e.button===0?(S.panMode?'pan':mode3D?'orbit':'none'):e.button===1?'pan':'right';
 S.drag={kind,button:e.button,x:e.clientX,y:e.clientY,lx:e.clientX,ly:e.clientY,ox:e.offsetX,oy:e.offsetY,yaw:camera3.yaw,pitch:camera3.pitch,moved:false,shift:!!e.shiftKey};if(kind==='pan')canvas.style.cursor='grabbing'};
canvas.onpointermove=e=>{const d=S.drag;if(!d){if(!mine())return call(H0.move,e);return}
 if(!d.moved&&Math.hypot(e.clientX-d.x,e.clientY-d.y)>3)d.moved=true;if(!d.moved)return;
 if(d.kind==='orbit'){camera3.yaw=d.yaw+(e.clientX-d.x)*.008;camera3.pitch=clampPitch(d.pitch+(e.clientY-d.y)*.008);render()}
 else if(d.kind==='pan'||d.kind==='right'){const mx=e.clientX-d.lx,my=e.clientY-d.ly;d.lx=e.clientX;d.ly=e.clientY;if(isLayout()){S.layout.x-=mx/S.layout.s;S.layout.y-=my/S.layout.s}else pan3D(mx,my);render()}};
canvas.onpointerup=e=>{const d=S.drag;if(!d)return call(H0.up,e);S.drag=null;try{canvas.releasePointerCapture(e.pointerId)}catch(err){}if(d.kind==='pan')canvas.style.cursor=S.panMode?'grab':mode3D?'default':'crosshair';
 if(!d.moved){if(d.kind==='orbit'&&mode3D){const hit=pickMesh(d.ox,d.oy);if(hit)setMesh(d.shift?'b':'a',hit.index)}else if(d.button===2){exitPan();CF.openContextMenu(e.clientX,e.clientY)}}else if(d.kind==='orbit')emitView()};
canvas.onpointercancel=e=>{if(S.drag){S.drag=null;return}return call(H0.cancel,e)};
canvas.onwheel=e=>{if(!mine())return call(H0.wheel,e);e.preventDefault?.();const f=Math.exp(-(e.deltaY||0)*.001);if(isLayout())zoomLayoutAt(e.offsetX,e.offsetY,f);else zoom3DAt(e.offsetX,e.offsetY,f);render()};
canvas.oncontextmenu=e=>{if(mine()){e.preventDefault?.();return}return call(H0.menu,e)};
canvas.ondblclick=e=>{if(isLayout()){const c=sheet(),p=screenToPaper({x:e.offsetX,y:e.offsetY});if(p.x>=0&&p.y>=0&&p.x<=c.width&&p.y<=c.height&&typeof plotSheet==='function')plotSheet();return}if(mode3D)return;return call(H0.dbl,e)};

// ---- Commands (display toggles; fallbacks when the command module is absent) -------------------
async function onOff(flag){const v=await cadPrompt('Enter an option [ON/OFF]',CF.get(flag)?'ON':'OFF');if(v===null)return;const s=String(v).trim().toUpperCase();if(/^ON/.test(s))CF.set(flag,true,{quiet:true});else if(/^OF/.test(s))CF.set(flag,false,{quiet:true});else notify('Invalid option keyword.')}
CF.register({name:'NAVVCUBE',label:'ViewCube',desc:'Controls the display of the ViewCube',category:'View',icon:'viewcube',run:()=>onOff('viewCube')});
CF.register({name:'NAVBAR',label:'Navigation Bar',desc:'Controls the display of the navigation bar',category:'View',icon:'pan',run:()=>onOff('navBar')});
CF.register({name:'UCSICON',label:'UCS Icon',desc:'Controls the display of the UCS icon',category:'View',icon:'ucs',run:()=>onOff('ucsIcon')});
if(!CF.resolve('VSCURRENT'))CF.register({name:'VSCURRENT',aliases:['VS'],label:'Visual Styles',desc:'Sets the visual style of the current viewport',category:'View',icon:'visual-style',async run(){const v=await cadPrompt('Enter an option [2dwireframe/Wireframe/Shaded/shaded with Edges]',mode3D?S.style:'2dwireframe');if(v===null)return;const s=String(v).trim().toLowerCase();CF.setVisualStyle(s.startsWith('2')?'2dwireframe':s.startsWith('w')?'wireframe':s==='e'||s.includes('edge')?'shadededges':s.startsWith('s')?'shaded':s)}});
if(!CF.resolve('3DORBIT'))CF.register({name:'3DORBIT',aliases:['3DO','ORBIT'],label:'Orbit',desc:'Rotates the view in 3D space',category:'View',icon:'orbit',run:orbit});
if(!CF.resolve('PLAN'))CF.register({name:'PLAN',label:'Plan View',desc:'Displays the plan view of the XY plane',category:'View',icon:'model',run:()=>CF.setViewPreset('top')});

// ---- Public API (tests and other modules) ---------------------------------------------------
CF.views={presets:PRESETS,styles:STYLES,style:()=>S.style,viewName,styleLabel,presetOf,presetFromDirection,cameraFromDirection,basis,goCamera,cubeHit,cubeClick,depthBuffer,visibleSegments,featureEdges:m=>meshEdges(m).edges.filter(e=>e.feature),meshEdges,pickMesh,setMesh,picked:()=>({a:pickIndex('a'),b:pickIndex('b')}),pan3D,zoom3DAt,zoom,pan,orbit,home:goHome,enterPan,exitPan,
 layout:()=>({sheet:sheet(),modelToPaper,paperToScreen,screenToPaper,state:S.layout}),fitLayout,zoomLayoutAt,label:()=>[ui.vpMin?.textContent,ui.vpView?.textContent,ui.vpStyle?.textContent],state:S};
try{buildChrome()}catch(err){console.error('CadForge views: chrome unavailable',err)}
try{render()}catch(err){console.error(err)}
}
