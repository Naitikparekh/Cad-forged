'use strict';
// 2D model-space interaction: grid, crosshair, object snap/ortho/polar, selection windows, grips, previews, shortcut menu, dynamic input.
(()=>{
CF.module('interact');
const G=CF.geom,TAU=Math.PI*2,DEG=180/Math.PI,NO_DASH=[],DASH=[6,4],RUBBER=[4,3],DOTS=[2,3];
const POINT_KINDS={point:1,angle:1,distance:1,factor:1},LEGACY_PICK=new Set(['move','copy','rotate','scale','mirror','offset']);
const DEFAULT_MODES=new Set(['endpoint','midpoint','center','intersection','quadrant']);
const SNAP_LABEL={endpoint:'Endpoint',midpoint:'Midpoint',center:'Center',quadrant:'Quadrant',intersection:'Intersection',perpendicular:'Perpendicular',tangent:'Tangent',nearest:'Nearest',insertion:'Insertion'};
const SNAP_ABBR={endpoint:'endp',midpoint:'mid',center:'cen',quadrant:'qua',intersection:'int',perpendicular:'per',tangent:'tan',nearest:'nea',insertion:'ins'};
// Typed one-shot overrides: any prefix of at least 3 letters of these words (AutoCAD: END, MID, CEN, QUA, INT, PER, TAN, NEA, INS, NON).
const SNAP_WORDS=['endpoint','midpoint','center','quadrant','intersection','perpendicular','tangent','nearest','insertion','none'];
const SEP={sep:true},GRIP_CAP=400,GHOST_CAP=4000;
// Interaction state. sx/sy = cursor in canvas pixels; raw = unsnapped world point; mark = active osnap; track = polar tracking.
const st={over:false,sx:0,sy:0,raw:{x:0,y:0},mark:null,track:null,win:null,pan:null,panMode:false,grip:null,gripDrag:null,hover:-1,hoverSet:null,hoverGrip:null,grips:[],menu:null,menuIndex:-1,midClick:0,recent:[]};
const is2D=()=>!mode3D&&CF.space==='model';
const pointInput=()=>CF.input&&POINT_KINDS[CF.input.kind]?CF.input:null;
const fmt=v=>String(Number((+v).toFixed(8)));
const normDeg=a=>((a%360)+360)%360;
const esc=s=>String(s).replace(/[&<>"]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]));
const print=line=>{try{CF.commandLine?.print?.(line)}catch(err){}};

// ---- Per-frame layer lookups --------------------------------------------------------------
const layerColor=new Map(),hidden=new Set();
function syncLayerMaps(){layerColor.clear();hidden.clear();for(const l of doc.layers){layerColor.set(l.name,l.color);if(!l.visible)hidden.add(l.name)}}
// Selected indices including whole groups, in O(n) (chosen() is quadratic for large groups such as hatches).
function selectedIndices(){const ids=new Set(),n=doc.entities.length;for(const i of selectionSet)if(i>=0&&i<n)ids.add(i);if(selected>=0&&selected<n)ids.add(selected);let groups=null;for(const i of ids){const g=doc.entities[i].group;if(g)(groups??=new Set()).add(g)}if(groups)for(let i=0;i<n;i++){const g=doc.entities[i].group;if(g&&groups.has(g))ids.add(i)}return ids}
// Same result and order as the engine's chosen(), without its quadratic includes() scans (it runs every frame).
chosen=function(){const n=doc.entities.length,valid=[],seen=new Set();for(const i of selectionSet)if(i>=0&&i<n){valid.push(i);seen.add(i)}if(selected>=0&&!seen.has(selected)){valid.push(selected);seen.add(selected)}const groups=new Set();for(const i of valid){const g=doc.entities[i]?.group;if(g)groups.add(g)}if(groups.size)for(let i=0;i<n;i++){const g=doc.entities[i].group;if(g&&groups.has(g)&&!seen.has(i)){valid.push(i);seen.add(i)}}return valid};
function groupMembers(i){const e=doc.entities[i],out=new Set([e]);if(e?.group)for(const o of doc.entities)if(o.group===e.group)out.add(o);return out}

// ---- Modes -----------------------------------------------------------------------------------
// Pick mode: clicks select objects with raw (unsnapped) points and the cursor shows a pickbox.
function pickMode(){if(pointInput())return false;if(CF.picking)return true;if(CF.input)return false;return CF.isPick()||(LEGACY_PICK.has(tool)&&selected<0)}
function basePoint(){const inp=pointInput();if(inp&&inp.base)return inp.base;if(points.length)return points[points.length-1];return null}
function activeModes(){const m=CF.osnapModes;if(m instanceof Set)return m;if(Array.isArray(m))return new Set(m);return DEFAULT_MODES}
function gridStep(){const s=view.scale;if(!(s>0)||!Number.isFinite(s))return 10;let step=10,guard=0;while(step*s<12&&guard++<40)step*=5;while(step*s>=60&&guard++<80)step/=5;return step}

// ---- Object snap candidates --------------------------------------------------------------------
// Returns candidates sorted by rank; rank is distance in world units with penalties so that nearest/perpendicular and
// center-by-perimeter only win when no exact point is inside the aperture. Hatch lines and hidden layers are ignored.
function osnapCandidates(p,opt={}){
 syncLayerMaps();const ap=(opt.aperture??12)/view.scale,modes=opt.modes||activeModes(),base=opt.base===undefined?basePoint():opt.base,out=[],segs=[],circles=[];
 const has=m=>modes.has(m),add=(type,x,y,d)=>out.push({type,x,y,d,label:SNAP_LABEL[type]});
 const ents=doc.entities;
 for(let i=0;i<ents.length;i++){const e=ents[i];if(e.hatch||hidden.has(e.layer))continue;
  if(e.type==='circle'){const c=e.center,dc=Math.hypot(p.x-c.x,p.y-c.y);if(dc<=ap||Math.abs(dc-e.radius)<=ap)circles.push({e,dc,dp:Math.abs(dc-e.radius)});continue}
  if(e.type==='text'){const q=e.points[0],d=Math.hypot(p.x-q.x,p.y-q.y);if(d<=ap&&(has('insertion')||has('endpoint')))add('insertion',q.x,q.y,d);continue}
  const ps=e.points,n=ps?.length||0;if(n<2)continue;const last=e.closed&&n>2?n:n-1;
  for(let j=0;j<last;j++){const a=ps[j],b=ps[(j+1)%n];
   if((a.x<b.x?a.x:b.x)-ap>p.x||(a.x>b.x?a.x:b.x)+ap<p.x||(a.y<b.y?a.y:b.y)-ap>p.y||(a.y>b.y?a.y:b.y)+ap<p.y)continue;
   const q=G.closestOnSegment(p,a,b);if(q.d<=ap&&segs.length<300)segs.push({a,b,e,q})}}
 for(const s of segs){const {a,b,q}=s;
  if(has('endpoint')){const da=Math.hypot(p.x-a.x,p.y-a.y),db=Math.hypot(p.x-b.x,p.y-b.y);if(da<=ap)add('endpoint',a.x,a.y,da);if(db<=ap)add('endpoint',b.x,b.y,db)}
  if(has('midpoint')){const mx=(a.x+b.x)/2,my=(a.y+b.y)/2,d=Math.hypot(p.x-mx,p.y-my);if(d<=ap)add('midpoint',mx,my,d)}
  if(has('nearest'))add('nearest',q.x,q.y,q.d+ap);
  if(has('perpendicular')&&base){const dx=b.x-a.x,dy=b.y-a.y,l2=dx*dx+dy*dy;if(l2>1e-18){const t=((base.x-a.x)*dx+(base.y-a.y)*dy)/l2;if(t>=-1e-9&&t<=1+1e-9){const fx=a.x+t*dx,fy=a.y+t*dy;if(Math.hypot(fx-base.x,fy-base.y)>1e-9)add('perpendicular',fx,fy,q.d+ap*.6)}}}}
 for(const {e,dc,dp}of circles){const c=e.center,r=e.radius;
  if(has('center'))add('center',c.x,c.y,Math.min(dc,dp+ap*.5));
  if(dp<=ap){
   if(has('quadrant'))for(let k=0;k<4;k++){const x=c.x+(k===0?r:k===2?-r:0),y=c.y+(k===1?r:k===3?-r:0),d=Math.hypot(p.x-x,p.y-y);if(d<=ap)add('quadrant',x,y,d)}
   if(has('nearest')&&dc>1e-12)add('nearest',c.x+(p.x-c.x)*r/dc,c.y+(p.y-c.y)*r/dc,dp+ap);
   if(has('perpendicular')&&base){const db=Math.hypot(base.x-c.x,base.y-c.y);if(db>1e-12)add('perpendicular',c.x+(base.x-c.x)*r/db,c.y+(base.y-c.y)*r/db,dp+ap*.6)}
   if(has('tangent')&&base){const db=Math.hypot(base.x-c.x,base.y-c.y);if(db>r*(1+1e-9)){ // two tangent points seen from the base point; take the one nearer the cursor
    const b=Math.atan2(base.y-c.y,base.x-c.x),f=Math.acos(r/db);let best=null;for(const s of[-1,1]){const x=c.x+r*Math.cos(b+s*f),y=c.y+r*Math.sin(b+s*f),d=Math.hypot(p.x-x,p.y-y);if(!best||d<best.d)best={x,y,d}}add('tangent',best.x,best.y,dp+ap*.6)}}}}
 if(has('intersection')){const near=(x,y)=>{const d=Math.hypot(p.x-x,p.y-y);if(d<=ap)add('intersection',x,y,d)};
  for(let i=0;i<segs.length;i++)for(let j=i+1;j<segs.length;j++){const s=segs[i],t=segs[j];if(s.e===t.e)continue;const x=G.segmentIntersection(s.a,s.b,t.a,t.b);if(x)near(x.x,x.y)}
  for(const c of circles){if(c.dp>ap)continue;for(const s of segs)for(const x of G.lineCircle(s.a,s.b,c.e.center,c.e.radius))near(x.x,x.y);for(const o of circles)if(o!==c&&o.dp<=ap)for(const x of G.circleCircle(c.e.center,c.e.radius,o.e.center,o.e.radius))near(x.x,x.y)}}
 out.sort((a,b)=>a.d-b.d);return out}

// ---- Ortho / polar ---------------------------------------------------------------------------
function orthoProject(base,p){return Math.abs(p.x-base.x)>=Math.abs(p.y-base.y)?{x:p.x,y:base.y}:{x:base.x,y:p.y}}
function polarProject(base,p,inc=CF.polarIncrement||45,tol=4){const dx=p.x-base.x,dy=p.y-base.y;if(Math.hypot(dx,dy)<1e-12)return null;const a=Math.atan2(dy,dx)*DEG,snapA=Math.round(a/inc)*inc;if(Math.abs(a-snapA)>tol)return null;const r=snapA/DEG;let ux=Math.cos(r),uy=Math.sin(r);if(Math.abs(ux)<1e-12)ux=0;if(Math.abs(uy)<1e-12)uy=0;const d=dx*ux+dy*uy;if(d<=0)return null;return{x:base.x+ux*d,y:base.y+uy*d,angle:normDeg(Math.round(snapA*1e9)/1e9),dist:d,base}}

// Global snap pipeline: object snap > grid snap > ortho/polar. Pick tools and the window tool use raw points.
snap=function(p){st.mark=null;st.track=null;const q={x:p.x,y:p.y};if(mode3D||CF.space!=='model'||tool==='window'||pickMode())return q;
 const base=basePoint(),ov=CF.osnapOverride;let snapped=false;
 if(ov!=='none'&&(ov||CF.get('osnap'))){const c=osnapCandidates(p,{modes:ov?new Set([ov]):activeModes(),base})[0];if(c){q.x=c.x;q.y=c.y;st.mark=c;snapped=true}}
 if(!snapped&&CF.get('snap')){const s=gridStep();q.x=Math.round(q.x/s)*s;q.y=Math.round(q.y/s)*s}
 if(!snapped&&base){if(CF.get('ortho')){const o=orthoProject(base,q);q.x=o.x;q.y=o.y}else if(CF.get('polar')){const r=polarProject(base,q);if(r){q.x=r.x;q.y=r.y;st.track=r}}}
 return q};

// ---- Selection ------------------------------------------------------------------------------
// Liang-Barsky clip test of segment against an axis-aligned rectangle.
function segRect(ax,ay,bx,by,x0,y0,x1,y1){let t0=0,t1=1;const dx=bx-ax,dy=by-ay;const clip=(p,q)=>{if(p===0)return q>=0;const r=q/p;if(p<0){if(r>t1)return false;if(r>t0)t0=r}else{if(r<t0)return false;if(r<t1)t1=r}return true};return clip(-dx,ax-x0)&&clip(dx,x1-ax)&&clip(-dy,ay-y0)&&clip(dy,y1-ay)}
// Window: objects fully inside. Crossing: objects inside or touching the rectangle (exact for segments and circles).
function windowSelect(a,b,crossing){syncLayerMaps();const x0=Math.min(a.x,b.x),x1=Math.max(a.x,b.x),y0=Math.min(a.y,b.y),y1=Math.max(a.y,b.y),inR=(x,y)=>x>=x0&&x<=x1&&y>=y0&&y<=y1,out=[];
 doc.entities.forEach((e,i)=>{if(hidden.has(e.layer))return;let ok=false;
  if(e.type==='circle'){const c=e.center,r=e.radius;if(!crossing)ok=c.x-r>=x0&&c.x+r<=x1&&c.y-r>=y0&&c.y+r<=y1;else{const nx=Math.max(x0-c.x,0,c.x-x1),ny=Math.max(y0-c.y,0,c.y-y1),fx=Math.max(Math.abs(c.x-x0),Math.abs(c.x-x1)),fy=Math.max(Math.abs(c.y-y0),Math.abs(c.y-y1));ok=nx*nx+ny*ny<=r*r&&fx*fx+fy*fy>=r*r}}
  else if(e.type==='text'){const bb=G.bbox(e);ok=crossing?bb.maxX>=x0&&bb.minX<=x1&&bb.maxY>=y0&&bb.minY<=y1:bb.minX>=x0&&bb.maxX<=x1&&bb.minY>=y0&&bb.maxY<=y1}
  else{const ps=e.points||[],n=ps.length;if(!crossing)ok=n>0&&ps.every(q=>inR(q.x,q.y));else{ok=ps.some(q=>inR(q.x,q.y));const last=e.closed&&n>2?n:n-1;for(let j=0;!ok&&j<last;j++){const s=ps[j],t=ps[(j+1)%n];ok=segRect(s.x,s.y,t.x,t.y,x0,y0,x1,y1)}}}
  if(ok)out.push(i)});
 if(!crossing&&out.some(i=>doc.entities[i].group)){const inside=new Set(out),bad=new Set();doc.entities.forEach((e,i)=>{if(e.group&&!inside.has(i)&&!hidden.has(e.layer))bad.add(e.group)});return out.filter(i=>!bad.has(doc.entities[i].group))}
 return out}
function reportFound(before){if(!CF.picking)return;const after=CF.selection().length;print(`${Math.max(0,after-before)} found, ${after} total`)}
// AutoCAD PICKADD: clicking adds to the selection, Shift+click removes.
function pickAt(p,shift=false){const i=hit(p);if(i<0)return false;const before=CF.selection().length;CF.select([i],shift?'remove':'add');reportFound(before);return true}
function finishWindow(b,shift=false){const w=st.win;st.win=null;if(!w){render();return[]}const crossing=b.x<w.a.x,ids=windowSelect(w.a,b,crossing),before=CF.selection().length;if(ids.length)CF.select(ids,shift||w.shift?'remove':'add');else render();reportFound(before);return ids}

// ---- Grips ----------------------------------------------------------------------------------
// Grip kinds: vertex (stretch one vertex), segment (stretch a polyline segment), move (whole entity), group (whole block
// or group), radius (circle quadrant). Hatch lines never get grips.
function gripsFor(ids){syncLayerMaps();const out=[],groups=new Set();
 for(const i of ids){if(out.length>=GRIP_CAP)break;const e=doc.entities[i];if(!e||e.hatch||hidden.has(e.layer))continue;
  if(e.group){if(groups.has(e.group))continue;groups.add(e.group);const v=e.type==='circle'?e.center:e.points[0];out.push({x:v.x,y:v.y,kind:'group',ei:i,group:e.group});continue}
  if(e.type==='circle'){const c=e.center,r=e.radius;out.push({x:c.x,y:c.y,kind:'move',ei:i});for(let k=0;k<4;k++)out.push({x:c.x+r*Math.round(Math.cos(k*Math.PI/2)),y:c.y+r*Math.round(Math.sin(k*Math.PI/2)),kind:'radius',ei:i});continue}
  if(e.type==='text'){out.push({x:e.points[0].x,y:e.points[0].y,kind:'move',ei:i});continue}
  const ps=e.points;ps.forEach((v,j)=>out.push({x:v.x,y:v.y,kind:'vertex',ei:i,vi:j}));
  if(e.type==='line')out.push({x:(ps[0].x+ps[1].x)/2,y:(ps[0].y+ps[1].y)/2,kind:'move',ei:i});
  else if(ps.length<=32){const n=ps.length,last=e.closed&&n>2?n:n-1;for(let j=0;j<last;j++){const a=ps[j],b=ps[(j+1)%n];out.push({x:(a.x+b.x)/2,y:(a.y+b.y)/2,kind:'segment',ei:i,vi:j})}}}
 return out.length>GRIP_CAP?out.slice(0,GRIP_CAP):out}
function gripAt(p,list=st.grips){const tol=6/view.scale;let best=null,bd=tol;for(const g of list){const d=Math.max(Math.abs(g.x-p.x),Math.abs(g.y-p.y));if(d<=bd){bd=d;best=g}}return best}
function gripTransform(e,g,p){const dx=p.x-g.x,dy=p.y-g.y,shift=q=>({x:q.x+dx,y:q.y+dy});
 if(g.kind==='vertex')e.points[g.vi]={x:p.x,y:p.y};
 else if(g.kind==='segment'){const n=e.points.length,j=g.vi,k=(j+1)%n;e.points[j]=shift(e.points[j]);e.points[k]=shift(e.points[k])}
 else if(g.kind==='radius'){const r=Math.hypot(p.x-e.center.x,p.y-e.center.y);if(r>1e-9)e.radius=r}
 else transformEntity(e,shift)}
function applyGrip(g,p){const e=doc.entities[g.ei];if(!e||!Number.isFinite(p?.x)||!Number.isFinite(p?.y))return false;if(g.kind==='radius'&&!(Math.hypot(p.x-e.center.x,p.y-e.center.y)>1e-9))return false;
 mutate(()=>{if(g.kind==='group'){for(const o of doc.entities)if(o.group===g.group)gripTransform(o,g,p)}else gripTransform(e,g,p)});return true}
function gripGhost(g,p){const e=doc.entities[g.ei];if(!e)return[];const src=g.kind==='group'?doc.entities.filter(o=>o.group===g.group).slice(0,GHOST_CAP):[e];return src.map(o=>{const c=structuredClone(o);gripTransform(c,g,p);return c})}
function parsePoint(v,base){if(v&&typeof v==='object'&&Number.isFinite(v.x)&&Number.isFinite(v.y))return{x:+v.x,y:+v.y};const s=String(v??'').trim(),N='(-?(?:\\d+\\.?\\d*|\\.\\d+)(?:e[-+]?\\d+)?)',o=base||{x:0,y:0};let m;
 if((m=s.match(new RegExp('^(@)?\\s*'+N+'\\s*,\\s*'+N+'(?:\\s*,\\s*'+N+')?$','i'))))return m[1]?{x:o.x+ +m[2],y:o.y+ +m[3]}:{x:+m[2],y:+m[3]};
 if((m=s.match(new RegExp('^(@)?\\s*'+N+'\\s*<\\s*'+N+'$','i')))){const d=+m[2],a=+m[3]/DEG,c=m[1]?o:{x:0,y:0},cs=Math.abs(Math.cos(a))<1e-12?0:Math.cos(a),sn=Math.abs(Math.sin(a))<1e-12?0:Math.sin(a);return{x:c.x+d*cs,y:c.y+d*sn}}
 if((m=s.match(new RegExp('^'+N+'$','i')))&&base){const d=+m[1],a=Math.atan2(mouse.y-base.y,mouse.x-base.x);return{x:base.x+d*Math.cos(a),y:base.y+d*Math.sin(a)}}
 return null}
// A hot grip is a pending point input, so typed coordinates, Esc and dynamic input all work through CF.input.
function startGrip(g){cancelGrip();st.grip=g;const msg=g.kind==='move'||g.kind==='group'?'** MOVE ** Specify move point or [eXit]:':g.kind==='radius'?'** STRETCH ** Specify radius of circle or [eXit]:':'** STRETCH ** Specify stretch point or [eXit]:';
 const inp={message:msg,defaultValue:'',kind:'point',base:{x:g.x,y:g.y},ixGrip:true,resolve(v){if(CF.input===inp)CF.input=null;if(st.grip!==g)return;st.grip=null;st.gripDrag=null;if(v===null||v===undefined||/^\s*(x|exit)?\s*$/i.test(String(v))){render();return}const p=parsePoint(v,inp.base);if(p)applyGrip(g,p);else notify('Point or option keyword required.');render()}};
 CF.input=inp;render();return inp}
function cancelGrip(){const had=!!st.grip;st.grip=null;st.gripDrag=null;if(CF.input?.ixGrip)CF.input=null;return had}

// ---- Canvas drawing -------------------------------------------------------------------------
let batching=false,batchColor=null,frameSel=new Set();const selList=[],hovList=[];
const sxf=x=>W/2+(x-view.x)*view.scale,syf=y=>H/2-(y-view.y)*view.scale;
function flush(){if(batchColor===null)return;ctx.strokeStyle=batchColor;ctx.lineWidth=1;ctx.setLineDash(NO_DASH);ctx.stroke();batchColor=null}
// Adds an entity outline to the current path (screen space) through an optional affine transform m=[a,b,c,d,tx,ty,k];
// returns false when it is off screen.
function addPath(e,m){const s=view.scale,ox=W/2-view.x*s,oy=H/2+view.y*s;
 if(e.type==='circle'){let x=e.center.x,y=e.center.y,r=e.radius;if(m){const X=m[0]*x+m[1]*y+m[4];y=m[2]*x+m[3]*y+m[5];x=X;r*=m[6]}const cx=ox+x*s,cy=oy-y*s,R=r*s;if(!(R>0)||cx+R<-2||cx-R>W+2||cy+R<-2||cy-R>H+2)return false;ctx.moveTo(cx+R,cy);ctx.arc(cx,cy,R,0,TAU);return true}
 const ps=e.points,n=ps?.length||0;if(n<2)return false;let x0=Infinity,y0=Infinity,x1=-Infinity,y1=-Infinity;
 for(let i=0;i<n;i++){let x=ps[i].x,y=ps[i].y;if(m){const X=m[0]*x+m[1]*y+m[4];y=m[2]*x+m[3]*y+m[5];x=X}x=ox+x*s;y=oy-y*s;if(x<x0)x0=x;if(x>x1)x1=x;if(y<y0)y0=y;if(y>y1)y1=y}
 if(x1<-2||x0>W+2||y1<-2||y0>H+2)return false;
 for(let i=0;i<n;i++){let x=ps[i].x,y=ps[i].y;if(m){const X=m[0]*x+m[1]*y+m[4];y=m[2]*x+m[3]*y+m[5];x=X}i?ctx.lineTo(ox+x*s,oy-y*s):ctx.moveTo(ox+x*s,oy-y*s)}
 if(e.closed)ctx.closePath();return true}
function drawText(e,color,m,glow){const p=e.points[0];let x=p.x,y=p.y,h=e.height;if(m){const X=m[0]*x+m[1]*y+m[4];y=m[2]*x+m[3]*y+m[5];x=X;h*=m[6]}const X=sxf(x),Y=syf(y),px=h*view.scale;if(!(px>=.5)||X>W+2||Y<-2||Y-px>H+2||X+String(e.text).length*px*.6<-2)return;
 ctx.font=`${px}px "Segoe UI",Arial,sans-serif`;ctx.textAlign='left';ctx.textBaseline='alphabetic';ctx.fillStyle=color;if(glow){ctx.save();ctx.shadowColor=CF.colors.selection;ctx.shadowBlur=8}ctx.fillText(e.text,X,Y);if(glow)ctx.restore()}
// Draws a list of entities in their layer colours (or one colour), batching paths per colour.
function drawList(list,color,m,width=1,dash=NO_DASH){let cur=null;const done=()=>{if(cur!==null){ctx.strokeStyle=cur;ctx.lineWidth=width;ctx.setLineDash(dash);ctx.stroke();cur=null}};
 for(const e of list){const col=color||e.color||layerColor.get(e.layer)||CF.colors.preview;if(e.type==='text'){drawText(e,col,m);continue}if(col!==cur){done();cur=col;ctx.beginPath()}addPath(e,m)}done();ctx.setLineDash(NO_DASH)}
const baseDrawEntity=drawEntity;
// During the render entity loop, plain entities are batched by colour; selected and hovered ones are deferred and drawn
// on top with the selection effect (glow + dashes) or the hover thickening.
drawEntity=function(e,color,dashed=false){if(mode3D)return baseDrawEntity(e,color,dashed);
 if(batching&&!dashed){if(frameSel.has(e)){selList.push(e);if(st.hoverSet?.has(e))hovList.push(e);return}if(st.hoverSet?.has(e)){hovList.push(e);return}const col=layerColor.get(e.layer)||color;if(e.type==='text'){drawText(e,col);return}if(col!==batchColor){flush();batchColor=col;ctx.beginPath()}addPath(e);return}
 flush();if(e.type==='text'){drawText(e,color);return}ctx.beginPath();if(addPath(e)){ctx.strokeStyle=color;ctx.lineWidth=1;ctx.setLineDash(dashed?DASH:NO_DASH);ctx.stroke();ctx.setLineDash(NO_DASH)}};
function endEntityPhase(){flush();batching=false;if(hovList.length){ctx.globalAlpha=.22;drawList(hovList.filter(e=>e.type!=='text'),CF.colors.selection,null,5);ctx.globalAlpha=1;drawList(hovList,null,null,2.25)}
 if(selList.length){ctx.lineJoin='round';ctx.globalAlpha=.32;drawList(selList.filter(e=>e.type!=='text'),CF.colors.selection,null,5);ctx.globalAlpha=1;ctx.lineJoin='miter';for(const e of selList)if(e.type==='text')drawText(e,layerColor.get(e.layer)||CF.colors.selection,null,true);drawList(selList.filter(e=>e.type!=='text'),null,null,1.25,DASH)}
 selList.length=0;hovList.length=0}

const baseDrawGrid=drawGrid;
// AutoCAD-style adaptive grid: minor lines fade in, every 5th line is major, axes are tinted.
drawGrid=function(){if(mode3D)return baseDrawGrid();const C=CF.colors;ctx.globalAlpha=1;ctx.setLineDash(NO_DASH);ctx.fillStyle=C.canvas;ctx.fillRect(0,0,W,H);if(!CF.get('grid'))return;const s=view.scale;if(!(s>0)||!Number.isFinite(s)||!(W>0)||!(H>0))return;
 const step=gridStep(),px=step*s,wx0=view.x-W/2/s,wx1=view.x+W/2/s,wy0=view.y-H/2/s,wy1=view.y+H/2/s,i0=Math.ceil(wx0/step),i1=Math.floor(wx1/step),j0=Math.ceil(wy0/step),j1=Math.floor(wy1/step);
 if(i1-i0>4000||j1-j0>4000)return;const X=i=>Math.round(sxf(i*step))+.5,Y=j=>Math.round(syf(j*step))+.5,major=k=>((k%5)+5)%5===0;
 ctx.lineWidth=1;ctx.strokeStyle=C.gridMinor;ctx.globalAlpha=Math.max(.3,Math.min(1,.3+(px-12)/40));ctx.beginPath();
 for(let i=i0;i<=i1;i++)if(!major(i)){const x=X(i);ctx.moveTo(x,0);ctx.lineTo(x,H)}for(let j=j0;j<=j1;j++)if(!major(j)){const y=Y(j);ctx.moveTo(0,y);ctx.lineTo(W,y)}ctx.stroke();ctx.globalAlpha=1;
 ctx.strokeStyle=C.gridMajor;ctx.beginPath();for(let i=i0;i<=i1;i++)if(major(i)&&i!==0){const x=X(i);ctx.moveTo(x,0);ctx.lineTo(x,H)}for(let j=j0;j<=j1;j++)if(major(j)&&j!==0){const y=Y(j);ctx.moveTo(0,y);ctx.lineTo(W,y)}ctx.stroke();
 const ox=Math.round(sxf(0))+.5,oy=Math.round(syf(0))+.5;if(oy>=0&&oy<=H){ctx.strokeStyle=C.axisX;ctx.beginPath();ctx.moveTo(0,oy);ctx.lineTo(W,oy);ctx.stroke()}if(ox>=0&&ox<=W){ctx.strokeStyle=C.axisY;ctx.beginPath();ctx.moveTo(ox,0);ctx.lineTo(ox,H);ctx.stroke()}};

function drawGrips(){st.grips=[];if(tool!=='select'||CF.picking||(CF.input&&!CF.input.ixGrip)||points.length||!frameSel.size)return;
 const ids=selectedIndices();st.grips=gripsFor(ids);const C=CF.colors,hot=st.grip;
 for(const g of st.grips){const x=Math.round(sxf(g.x)),y=Math.round(syf(g.y));if(x<-6||x>W+6||y<-6||y>H+6)continue;const isHot=hot&&hot.ei===g.ei&&hot.kind===g.kind&&hot.vi===g.vi&&hot.x===g.x&&hot.y===g.y;
  ctx.fillStyle=isHot?C.gripHot:g===st.hoverGrip?C.gripHover:C.grip;ctx.fillRect(x-3.5,y-3.5,8,8);ctx.strokeStyle='rgba(8,16,28,.85)';ctx.lineWidth=1;ctx.strokeRect(x-3.5,y-3.5,8,8)}}
function rubber(a,b){ctx.beginPath();ctx.moveTo(sxf(a.x),syf(a.y));ctx.lineTo(sxf(b.x),syf(b.y));ctx.strokeStyle=CF.colors.preview;ctx.lineWidth=1;ctx.setLineDash(RUBBER);ctx.stroke();ctx.setLineDash(NO_DASH)}
function shapeList(list){drawList(list,CF.colors.preview)}
function ghostSelection(m){const ids=[...selectedIndices()].slice(0,GHOST_CAP);if(!ids.length)return false;drawList(ids.map(i=>doc.entities[i]),null,m);return true}
// Affine helpers: [a,b,c,d,tx,ty,k] with x'=a x+b y+tx, y'=c x+d y+ty, k = radius/height factor.
const mTranslate=(dx,dy)=>[1,0,0,1,dx,dy,1];
const mRotate=(o,t)=>{const c=Math.cos(t),s=Math.sin(t);return[c,-s,s,c,o.x-c*o.x+s*o.y,o.y-s*o.x-c*o.y,1]};
const mScale=(o,f)=>[f,0,0,f,o.x*(1-f),o.y*(1-f),f];
const mMirror=(a,b)=>{const l=Math.hypot(b.x-a.x,b.y-a.y);if(l<1e-12)return null;const ux=(b.x-a.x)/l,uy=(b.y-a.y)/l,r00=2*ux*ux-1,r01=2*ux*uy,r11=2*uy*uy-1;return[r00,r01,r01,r11,a.x-(r00*a.x+r01*a.y),a.y-(r01*a.x+r11*a.y),1]};
function dimensionPreview(a,b,pos){const len=G.dist(a,b);if(len<1e-9)return[];const ux=(b.x-a.x)/len,uy=(b.y-a.y)/len,nx=-uy,ny=ux,off=(pos.x-a.x)*nx+(pos.y-a.y)*ny,A={x:a.x+nx*off,y:a.y+ny*off},B={x:b.x+nx*off,y:b.y+ny*off},size=Math.min(3,len/8),ents=[{type:'line',points:[a,A]},{type:'line',points:[b,B]},{type:'line',points:[A,B]},{type:'text',points:[{x:(A.x+B.x)/2+nx*2,y:(A.y+B.y)/2+ny*2}],text:len.toFixed(2),height:3}];for(const [p,dir]of [[A,1],[B,-1]])for(const side of [-1,1])ents.push({type:'line',points:[p,{x:p.x+dir*ux*size+nx*side*size*.35,y:p.y+dir*uy*size+ny*side*size*.35}]});return ents}
function arcPoints(c,a,b){const r=G.dist(c,a);if(r<1e-9)return null;const s=Math.atan2(a.y-c.y,a.x-c.x);let e=Math.atan2(b.y-c.y,b.x-c.x);while(e<=s)e+=TAU;const n=Math.max(16,Math.ceil((e-s)*32)),ps=[];for(let i=0;i<=n;i++){const t=s+(e-s)*i/n;ps.push({x:c.x+r*Math.cos(t),y:c.y+r*Math.sin(t)})}return ps}
function builtinPreview(){const n=points.length,m=mouse,inp=CF.input;let band=false;
 switch(tool){
  case 'line':case 'polyline':if(n){shapeList([{type:'polyline',points:tool==='line'?[points[n-1],m]:[...points,m]}]);band=true}break;
  case 'rectangle':if(n){shapeList([{type:'polyline',closed:true,points:[points[0],{x:m.x,y:points[0].y},m,{x:points[0].x,y:m.y}]}]);band=true}break;
  case 'circle':if(n){const r=G.dist(points[0],m);if(r>0)shapeList([{type:'circle',center:points[0],radius:r}]);rubber(points[0],m);band=true}break;
  case 'arc':if(n===1){rubber(points[0],m);band=true}else if(n===2){const ps=arcPoints(points[0],points[1],m);if(ps)shapeList([{type:'polyline',points:ps}]);rubber(points[0],m);band=true}break;
  case 'dimension':if(n===1){shapeList([{type:'line',points:[points[0],m]}]);band=true}else if(n===2){shapeList(dimensionPreview(points[0],points[1],m));band=true}break;
  case 'move':case 'copy':if(n&&ghostSelection(mTranslate(m.x-points[0].x,m.y-points[0].y))){rubber(points[0],m);band=true}break;
  case 'rotate':if(inp?.kind==='angle'&&inp.base){ghostSelection(mRotate(inp.base,G.angle(inp.base,m)));rubber(inp.base,m);band=true}break;
  case 'scale':if(inp?.kind==='factor'&&inp.base){const f=G.dist(inp.base,m);if(f>1e-9)ghostSelection(mScale(inp.base,f));rubber(inp.base,m);band=true}break;
  case 'mirror':if(n===1){const M=mMirror(points[0],m);if(M)ghostSelection(M);shapeList([{type:'line',points:[points[0],m]}]);band=true}break;
  case 'insert':{const b=typeof insertName!=='undefined'&&insertName?doc.blocks?.[insertName]:null;if(b)drawList(b.items.slice(0,GHOST_CAP).map(e=>e.layer==='0'?{...e,layer:layer().name}:e),null,mTranslate(m.x,m.y))}break}
 return band}
const baseDrawPreview=drawPreview;
drawPreview=function(){if(mode3D)return baseDrawPreview();endEntityPhase();drawGrips();let band=false;
 if(st.grip&&CF.input?.ixGrip){drawList(gripGhost(st.grip,mouse));rubber(st.grip,mouse);band=true}
 const custom=CF.previews[tool];
 if(custom){let ents=null;try{ents=custom(mouse)}catch(err){}if(ents?.length){const solid=ents.filter(e=>!e.dashed),dashed=ents.filter(e=>e.dashed);if(solid.length)drawList(solid,null);if(dashed.length)drawList(dashed,null,null,1,RUBBER)}}
 else if(!band)band=builtinPreview();
 const base=pointInput()?.base;if(!band&&!custom&&base)rubber(base,mouse)};

function drawMarker(m){const x=Math.round(sxf(m.x))+.5,y=Math.round(syf(m.y))+.5,s=6;ctx.strokeStyle=CF.colors.osnap;ctx.lineWidth=2;ctx.setLineDash(NO_DASH);ctx.beginPath();
 switch(m.type){case 'endpoint':ctx.rect(x-s,y-s,2*s,2*s);break;case 'midpoint':ctx.moveTo(x,y-s-1);ctx.lineTo(x+s+1,y+s);ctx.lineTo(x-s-1,y+s);ctx.closePath();break;case 'center':ctx.arc(x,y,s+1,0,TAU);break;
  case 'quadrant':ctx.moveTo(x,y-s-1);ctx.lineTo(x+s+1,y);ctx.lineTo(x,y+s+1);ctx.lineTo(x-s-1,y);ctx.closePath();break;case 'intersection':ctx.moveTo(x-s,y-s);ctx.lineTo(x+s,y+s);ctx.moveTo(x+s,y-s);ctx.lineTo(x-s,y+s);break;
  case 'perpendicular':ctx.moveTo(x-s,y-s);ctx.lineTo(x-s,y+s);ctx.lineTo(x+s,y+s);ctx.moveTo(x-s,y);ctx.lineTo(x,y);ctx.lineTo(x,y+s);break;case 'tangent':ctx.arc(x,y+2,s-1,0,TAU);ctx.moveTo(x-s-1,y-3);ctx.lineTo(x+s+1,y-3);break;case 'nearest':ctx.moveTo(x-s,y-s);ctx.lineTo(x+s,y-s);ctx.lineTo(x-s,y+s);ctx.lineTo(x+s,y+s);ctx.closePath();break;
  default:ctx.moveTo(x-s,y-s);ctx.lineTo(x+1,y-s);ctx.lineTo(x+1,y-1);ctx.lineTo(x+s,y-1);ctx.lineTo(x+s,y+s);ctx.lineTo(x-s,y+s);ctx.closePath()}ctx.stroke();ctx.lineWidth=1}
function canvasTip(text,x,y){ctx.font='11px "Segoe UI",Arial,sans-serif';const w=Math.ceil(ctx.measureText(text).width||text.length*6)+12,h=19;let X=Math.round(x),Y=Math.round(y);if(X+w>W-2)X=W-w-2;if(Y<2)Y=2;if(Y+h>H-2)Y=H-h-2;ctx.fillStyle=CF.colors.tooltip;ctx.fillRect(X,Y,w,h);ctx.strokeStyle='#5a6676';ctx.lineWidth=1;ctx.strokeRect(X+.5,Y+.5,w-1,h-1);ctx.fillStyle=CF.colors.tooltipText;ctx.textAlign='left';ctx.textBaseline='middle';ctx.fillText(text,X+6,Y+h/2+.5);ctx.textBaseline='alphabetic'}
function drawWindowRect(){const w=st.win;if(!w)return;const a={x:sxf(w.a.x),y:syf(w.a.y)},b={x:sxf(w.cur.x),y:syf(w.cur.y)},crossing=w.cur.x<w.a.x,C=CF.colors,x=Math.round(Math.min(a.x,b.x))+.5,y=Math.round(Math.min(a.y,b.y))+.5,ww=Math.round(Math.abs(b.x-a.x)),hh=Math.round(Math.abs(b.y-a.y));
 ctx.fillStyle=crossing?C.crossingFill:C.windowFill;ctx.fillRect(x,y,ww,hh);ctx.strokeStyle=crossing?C.crossingStroke:C.windowStroke;ctx.lineWidth=1;ctx.setLineDash(crossing?DASH:NO_DASH);ctx.strokeRect(x,y,ww,hh);ctx.setLineDash(NO_DASH)}
function drawCrosshair(){const C=CF.colors,pick=pickMode(),useSnap=!pick&&CF.get('snap')&&!st.mark&&!st.track,cx=Math.round(useSnap?sxf(mouse.x):st.sx)+.5,cy=Math.round(useSnap?syf(mouse.y):st.sy)+.5;
 // AutoCAD shows only the pickbox while selecting inside a command, crosshair + pickbox at the idle prompt, crosshair alone for points.
 const idle=tool==='select'&&!CF.picking&&!CF.input,box=pick?5:0,crossOn=!pick||idle,pct=CF.crosshairSize||6,L=pct>=100?Math.max(W,H)*2:Math.max(18,Math.round(Math.max(W,H)*pct/100));
 ctx.strokeStyle=C.crosshair;ctx.lineWidth=1;ctx.setLineDash(NO_DASH);ctx.beginPath();
 if(crossOn){ctx.moveTo(cx-L,cy);ctx.lineTo(cx-box,cy);ctx.moveTo(cx+box,cy);ctx.lineTo(cx+L,cy);ctx.moveTo(cx,cy-L);ctx.lineTo(cx,cy-box);ctx.moveTo(cx,cy+box);ctx.lineTo(cx,cy+L)}
 if(box)ctx.rect(cx-box,cy-box,box*2,box*2);ctx.stroke()}
const baseDrawCursor=drawCursor;
drawCursor=function(){if(mode3D)return baseDrawCursor();drawWindowRect();
 if(st.track&&!basePoint()&&!st.gripDrag&&!st.hot)st.track=null; // a finished command leaves no tracking ray behind
 if(st.track){const b=st.track.base,x=sxf(b.x),y=syf(b.y),r=st.track.angle/DEG,L=(W+H)*2;ctx.strokeStyle=CF.colors.osnap;ctx.lineWidth=1;ctx.setLineDash(DOTS);ctx.beginPath();ctx.moveTo(x,y);ctx.lineTo(x+Math.cos(r)*L,y-Math.sin(r)*L);ctx.stroke();ctx.setLineDash(NO_DASH)}
 if(!st.over||st.panMode||st.pan)return;
 if(st.mark)drawMarker(st.mark);drawCrosshair();
 if(st.mark)canvasTip(st.mark.label||SNAP_LABEL[st.mark.type]||st.mark.type,sxf(st.mark.x)+12,syf(st.mark.y)-32);
 else if(st.track)canvasTip(`Polar: ${st.track.dist.toFixed(4)} < ${fmt(st.track.angle)}°`,st.sx+16,st.sy-32)};

// ---- Render wrapper: per-frame state, cursor style and dynamic input ---------------------------
const baseRender=render;
render=function(...a){const twoD=is2D();if(twoD){syncLayerMaps();const ids=selectedIndices();frameSel=new Set();for(const i of ids)frameSel.add(doc.entities[i]);selList.length=0;hovList.length=0;batchColor=null;batching=true}
 try{return baseRender.apply(this,a)}finally{if(batching){flush();batching=false;selList.length=0;hovList.length=0}syncCursorStyle();updateDyn()}};
function syncCursorStyle(){const s=canvas.style;if(!s)return;const want=is2D()?(st.panMode||st.pan&&st.pan.moved?(st.pan?'grabbing':'grab'):'none'):null;if(want){if(s.cursor!==want)s.cursor=want}else if(['none','grab','grabbing'].includes(s.cursor))s.cursor='default'}

// Dynamic input tooltip (F12): prompt + distance/angle or X/Y fields next to the cursor, mirroring command-line typing.
const dyn=document.createElement('div');dyn.className='cf-dyn';dyn.setAttribute('aria-hidden','true');try{CF.hosts.overlay?.append(dyn)}catch(err){}
let dynHtml='',dynShown=false;
function shortPrompt(p){let s=String(p||'').trim(),opts=false,def='';s=s.replace(/^(\*\*[^*]*\*\*|[A-Z0-9_]*[A-Z][A-Z0-9_]+)\s+(?=\S)/,'').replace(/\s*\[[^\]]*\]/,()=>{opts=true;return''}).replace(/\s*<([^>]*)>/,(_,d)=>{def=d;return''}).replace(/:\s*$/,'');return{text:s,opts,def}}
function dynFields(){const typed=String($('command')?.value||''),prompt=String(CF.prompt?.()||''),idle=/^command:?$/i.test(prompt.trim());
 if(idle&&!st.win)return typed?`<span class="f on">${esc(typed)}</span>`:'';
 const sp=shortPrompt(prompt),field=(v,on)=>`<span class="f${on?' on':''}">${esc(v)}</span>`;let html=`<span class="p">${esc(sp.text)}${sp.opts?' <span class="o">▾</span>':''}</span>`;
 const inp=CF.input,base=basePoint();
 if(inp&&!POINT_KINDS[inp.kind])return html+field(typed||sp.def||inp.defaultValue||'',true);
 if(st.win||pickMode())return typed?html+field(typed,true):html;
 if(inp?.ixGrip&&st.grip?.kind==='radius'){const c=doc.entities[st.grip.ei]?.center;if(c)return html+field(typed||G.dist(c,mouse).toFixed(4),true)}
 if(inp&&inp.kind!=='point'&&inp.base){const v=inp.kind==='angle'?`${fmt(normDeg(G.angle(inp.base,mouse)*DEG).toFixed(0))}°`:G.dist(inp.base,mouse).toFixed(4);return html+field(typed||v,true)}
 if(base){const d=G.dist(base,mouse).toFixed(4),a=Math.round(normDeg(G.angle(base,mouse)*DEG))+'°';return html+field(typed||d,true)+field(a,false)}
 return html+field(typed||mouse.x.toFixed(4),true)+field(mouse.y.toFixed(4),false)}
function updateDyn(){let html='';try{if(CF.get('dyn')&&st.over&&is2D()&&!st.panMode&&!st.pan)html=dynFields()}catch(err){html=''}
 if(!html){if(dynShown){dyn.style.display='none';dynShown=false}return}
 if(html!==dynHtml){dyn.innerHTML=html;dynHtml=html}if(!dynShown){dyn.style.display='flex';dynShown=true}
 const w=dyn.offsetWidth||160,h=dyn.offsetHeight||22;let x=st.sx+18,y=st.sy+18;if(x+w>W-2)x=st.sx-18-w;if(y+h>H-2)y=st.sy-18-h;dyn.style.transform=`translate(${Math.round(Math.max(0,x))}px,${Math.round(Math.max(0,y))}px)`}
try{const cmd=$('command');cmd?.addEventListener?.('input',updateDyn);cmd?.addEventListener?.('keyup',updateDyn)}catch(err){}

// ---- Hover (selection preview) ---------------------------------------------------------------
function updateHover(raw){let g=null;if(tool==='select'&&!CF.picking&&!CF.input&&st.grips.length)g=gripAt(raw);st.hoverGrip=g;const i=g?-1:hit(raw);if(i!==st.hover){st.hover=i;st.hoverSet=i>=0?groupMembers(i):null}}
function clearHover(){st.hover=-1;st.hoverSet=null;st.hoverGrip=null}

// ---- Pointer handling -------------------------------------------------------------------------
const prev={down:canvas.onpointerdown,move:canvas.onpointermove,up:canvas.onpointerup,cancel:canvas.onpointercancel,menu:canvas.oncontextmenu,dbl:canvas.ondblclick,leave:canvas.onpointerleave};
const runAccept=p=>{try{Promise.resolve(accept(p)).catch(err=>notify('Action failed: '+(err?.message||err)))}catch(err){notify('Action failed: '+(err?.message||err))}};
function clickInput(raw){const inp=CF.input;if(!inp||!POINT_KINDS[inp.kind])return;const p=snap(raw);mouse=p;CF.osnapOverride=null;let v;
 if(inp.kind==='point')v=fmt(p.x)+','+fmt(p.y);else{if(!inp.base)return;v=inp.kind==='angle'?fmt(normDeg(G.angle(inp.base,p)*DEG)):fmt(G.dist(inp.base,p))}
 try{inp.resolve(v)}finally{if(CF.input===inp)CF.input=null}render()}
function selectClick(raw,e){if(tool==='select'&&!CF.picking&&!e.shiftKey){const ids=selectedIndices();if(ids.size){const g=gripAt(raw,gripsFor(ids));if(g){startGrip(g);st.gripDrag={x:e.offsetX,y:e.offsetY,moved:false};return}}}
 if(pickAt(raw,!!e.shiftKey))return;st.win={a:raw,cur:raw,sx:e.offsetX,sy:e.offsetY,down:true,drag:false,shift:!!e.shiftKey};render()}
function startPanDrag(e,btn){st.pan={btn,cx:e.clientX??e.offsetX,cy:e.clientY??e.offsetY,vx:view.x,vy:view.y,moved:btn!=='right'}}
canvas.onpointerdown=function(e){if(!is2D())return prev.down?.call(this,e);closeMenu();try{canvas.setPointerCapture(e.pointerId)}catch(err){}try{canvas.focus({preventScroll:true})}catch(err){}
 st.sx=e.offsetX;st.sy=e.offsetY;st.over=true;const raw=world({x:e.offsetX,y:e.offsetY});st.raw=raw;
 if(e.button===1){e.preventDefault?.();const now=performance.now();if(now-st.midClick<400){st.midClick=0;st.pan=null;fit();return}st.midClick=now;startPanDrag(e,'middle');return}
 if(e.button===2){startPanDrag(e,'right');return}
 if(e.button!==0)return;
 if(st.panMode){startPanDrag(e,'left');render();return}
 if(st.win){finishWindow(raw,!!e.shiftKey);return}
 if(CF.input){clickInput(raw);return}
 if(CF.picking||tool==='select'){selectClick(raw,e);return}
 if(pickMode()){mouse=raw;shiftSelection=!!e.shiftKey;runAccept(raw);shiftSelection=false;return}
 const p=snap(raw);mouse=p;CF.osnapOverride=null;runAccept(p);render()};
canvas.onpointermove=function(e){if(!is2D()&&!st.pan)return prev.move?.call(this,e);return movePointer(e)};
function movePointer(e){st.sx=e.offsetX;st.sy=e.offsetY;st.over=true;
 if(st.pan){const p=st.pan,dx=(e.clientX??e.offsetX)-p.cx,dy=(e.clientY??e.offsetY)-p.cy;if(!p.moved&&Math.hypot(dx,dy)>4)p.moved=true;if(p.moved){view.x=p.vx-dx/view.scale;view.y=p.vy+dy/view.scale}st.raw=world({x:e.offsetX,y:e.offsetY});render();return}
 const raw=world({x:e.offsetX,y:e.offsetY});st.raw=raw;
 if(st.win){st.win.cur=raw;if(st.win.down&&!st.win.drag&&Math.hypot(e.offsetX-st.win.sx,e.offsetY-st.win.sy)>4)st.win.drag=true;mouse=raw;st.mark=st.track=null;render();return}
 if(st.gripDrag&&!st.gripDrag.moved&&Math.hypot(e.offsetX-st.gripDrag.x,e.offsetY-st.gripDrag.y)>4)st.gripDrag.moved=true;
 if(CF.input&&!POINT_KINDS[CF.input.kind]){mouse=raw;st.mark=st.track=null;clearHover()}
 else if(pickMode()){mouse=raw;st.mark=st.track=null;if(!st.panMode)updateHover(raw)}
 else{mouse=snap(raw);clearHover()}
 render()}
canvas.onpointerup=function(e){if(!is2D()&&!st.pan&&!st.win)return prev.up?.call(this,e);try{canvas.releasePointerCapture(e.pointerId)}catch(err){}
 if(st.pan){const p=st.pan;st.pan=null;if(p.btn==='right'&&!p.moved){if(CF.picking&&!e.shiftKey)CF.enter();else CF.openContextMenu(e.clientX??e.offsetX,e.clientY??e.offsetY,e.shiftKey?'snap':undefined)}render();return}
 if(st.win&&st.win.down){st.win.down=false;if(st.win.drag)finishWindow(world({x:e.offsetX,y:e.offsetY}),!!e.shiftKey);return}
 if(st.gripDrag){const g=st.gripDrag;st.gripDrag=null;if(g.moved&&CF.input?.ixGrip)clickInput(world({x:e.offsetX,y:e.offsetY}))}
 prev.up?.call(this,e)};
canvas.onpointercancel=function(e){st.pan=null;if(st.win)st.win.down=false;st.gripDrag=null;return prev.cancel?.call(this,e)};
canvas.oncontextmenu=function(e){if(!is2D())return prev.menu?.call(this,e);e.preventDefault?.();return false};
canvas.onpointerleave=function(e){st.over=false;if(is2D()){clearHover();render()}return prev.leave?.call(this,e)};
// The cursor is only "over" the canvas once its position is known: entering without coordinates (or a synthetic enter when
// the Start page closes under a resting pointer) must not leave the crosshair parked at the stale 0,0 position.
canvas.onpointerenter=function(e){if(is2D()&&Number.isFinite(e?.offsetX)&&Number.isFinite(e?.offsetY))movePointer(e)};
canvas.ondblclick=function(e){if(!is2D()||e.button!==0||tool!=='select'||CF.picking||st.panMode)return prev.dbl?.call(this,e);
 if(CF.input?.ixGrip)cancelGrip();const i=hit(world({x:e.offsetX,y:e.offsetY})),ent=doc.entities[i];if(!ent){render();return}
 if(ent.type==='text'&&!ent.group){Promise.resolve(cadPrompt('Text',ent.text)).then(v=>{if(v!=null&&v!==''&&v!==ent.text&&doc.entities.includes(ent))mutate(()=>ent.text=v)})}else{CF.select([i],'replace');CF.toggle('properties',true)}render()};
try{canvas.addEventListener('mousedown',e=>{if(e.button===1)e.preventDefault()});canvas.style.outline='none'}catch(err){}

// ---- PAN mode ---------------------------------------------------------------------------------
CF.startPan=function(){if(mode3D)showModel();if(CF.space!=='model')CF.setSpace?.('model');cancelGrip();st.win=null;st.panMode=true;notify('Press ESC or ENTER to exit, or right-click to display shortcut menu.');render()};
function exitPan(){if(!st.panMode)return false;st.panMode=false;st.pan=null;render();return true}
{const basePrompt=CF.prompt;CF.prompt=function(...a){if(st.panMode)return 'PAN Press ESC or ENTER to exit, or right-click to display shortcut menu.';if(st.win&&!CF.input)return(CF.picking?'Select objects: ':'')+'Specify opposite corner:';return basePrompt.apply(this,a)}}
// Esc/Enter consume our own transient states before the command line sees them.
function resetTransient(){const busy=!!(st.win||st.grip||st.panMode);st.win=null;cancelGrip();st.panMode=false;st.pan=null;CF.osnapOverride=null;clearHover();return busy}
{const baseCancel=CF.cancel;CF.cancel=function(...a){closeMenu();resetTransient();return baseCancel.apply(this,a)}}
CF.on('tool',()=>{st.win=null;cancelGrip();st.panMode=false;CF.osnapOverride=null;clearHover();syncCursorStyle()});
const consume=e=>{e.preventDefault?.();e.stopPropagation?.();e.stopImmediatePropagation?.()};
// ---- One-shot object snap overrides (right-click Snap Overrides menu and typed END/MID/CEN/QUA/INT/PER/TAN/NEA/INS/NON) ------
// The override applies to the next point only: snap() honours it even with Osnap off, and every pick/typed point clears it.
function setOverride(mode){CF.osnapOverride=mode;if(mode!=='none')print(`_${SNAP_ABBR[mode]} of`);if(st.over&&is2D())mouse=snap(st.raw);render()}
function snapKeyword(text){const t=String(text??'').trim().replace(/^_/,'').toLowerCase();return t.length>=3?SNAP_WORDS.find(w=>w.startsWith(t))||null:null}
// A point prompt is any pending point/angle/distance/factor input, or a drawing tool waiting for a point (not object picking).
function pointPrompt(){if(!is2D()||st.win||st.panMode)return false;if(CF.input)return!!pointInput();if(CF.picking)return false;return tool==='select'?points.length>0:!pickMode()}
// Returns true when `text` was consumed as an osnap keyword. A keyword the current prompt also offers as an option (e.g. ELLIPSE
// [Center]) stays an option unless written with the leading underscore (_cen).
function typedSnap(text){const raw=String(text??'').trim(),mode=snapKeyword(raw);if(!mode||!pointPrompt())return false;
 const prompt=String(CF.prompt?.()||''),t=raw.replace(/^_/,'').toLowerCase();if(raw[0]!=='_'&&promptOptions(prompt).some(o=>o.label.toLowerCase().startsWith(t)))return false;
 print(`${prompt} ${raw}`);setOverride(mode);return true}
try{const cl=CF.commandLine;if(cl&&typeof cl.submit==='function'){const baseSubmit=cl.submit;cl.submit=function(text,...a){if(typedSnap(text))return Promise.resolve();return baseSubmit.call(this,text,...a)}}}catch(err){}
try{document.addEventListener('keydown',e=>{
 if(st.menu){if(e.key==='Escape'){closeMenu();consume(e)}else if(e.key==='ArrowDown'||e.key==='ArrowUp'){menuMove(e.key==='ArrowDown'?1:-1);consume(e)}else if(e.key==='Enter'){menuActivate();consume(e)}return}
 if((e.key==='Enter'||e.key==='NumpadEnter'||e.key===' ')&&!e.ctrlKey&&!e.altKey&&!e.metaKey&&e.target&&e.target===$('command')){const c=e.target,v=String(c.value||'').trim();
  if(v&&typedSnap(v)){c.value='';try{c.dispatchEvent(new Event('input'))}catch(err){}consume(e);return}
  if(v&&CF.osnapOverride&&pointPrompt()&&parsePoint(v,basePoint()))CF.osnapOverride=null} // a typed point replaces the pending override
 const field=e.target?.matches?.('input,textarea,select')&&e.target.id!=='command',typing=field||!!$('command')?.value;
 if(st.panMode&&(e.key==='Escape'||(!typing&&(e.key==='Enter'||e.key===' ')))){exitPan();consume(e);return}
 if(e.key==='Escape'&&st.win){st.win=null;print('*Cancel*');render();consume(e);return}
 if(e.key==='Escape'&&st.grip&&!CF.has('commands')){cancelGrip();render();consume(e)}},true)}catch(err){}

// ---- Shortcut (context) menu --------------------------------------------------------------------
CF.on('command',ev=>{const n=ev?.def?.name;if(!n)return;st.recent=[n,...st.recent.filter(x=>x!==n)].slice(0,8)});
const cmd=(name,fallback)=>()=>{if(CF.resolve(name))CF.run(name,{source:'menu'});else fallback?.()};
function promptOptions(text){const m=String(text||'').match(/\[([^\]]+)\]/);if(!m)return[];return m[1].split('/').map(s=>s.trim()).filter(Boolean).map(label=>({label,key:(label.match(/[A-Z]+/)||[label])[0]}))}
function submitOption(key){if(CF.commandLine?.submit){CF.commandLine.submit(key);return}if(CF.input){const inp=CF.input;inp.resolve(key);if(CF.input===inp)CF.input=null;render();return}const c=$('command');if(c){c.value=key;c.onkeydown?.({key:'Enter',target:c,preventDefault(){},stopPropagation(){}})}}
function snapItems(){const set=mode=>()=>setOverride(mode);
 return[...['endpoint','midpoint','intersection'].map(m=>({label:SNAP_LABEL[m],run:set(m),checked:CF.osnapOverride===m})),SEP,...['center','quadrant','perpendicular','tangent','nearest'].map(m=>({label:SNAP_LABEL[m],run:set(m),checked:CF.osnapOverride===m})),SEP,{label:'None',run:set('none'),checked:CF.osnapOverride==='none'},SEP,{label:'Object Snap',checked:CF.get('osnap'),shortcut:'F3',run:()=>CF.toggle('osnap')}]}
function menuModel(kind){
 if(kind==='snap')return snapItems();
 const zoomExt={label:'Zoom Extents',icon:'zoom-extents',run:()=>fit()},panItem={label:'Pan',icon:'pan',run:()=>CF.startPan()};
 if(st.panMode)return[{label:'Exit',run:exitPan},SEP,{label:'Pan',icon:'pan',checked:true,run:()=>{}},zoomExt];
 if(CF.input||CF.picking||tool!=='select'||points.length){const items=[{label:'Enter',run:()=>CF.enter()},{label:'Cancel',run:()=>CF.cancel()}],opts=promptOptions(CF.prompt());
  if(opts.length)items.push(SEP,...opts.map(o=>({label:o.label,run:()=>submitOption(o.key)})));
  items.push(SEP,{label:'Snap Overrides',sub:snapItems()},SEP,panItem,zoomExt);return items}
 const last=CF.lastCommand,repeat=last?{label:`Repeat ${last}`,run:()=>CF.run(last,{source:'menu'})}:{label:'Repeat',disabled:true};
 const recent={label:'Recent Input',sub:st.recent.length?st.recent.map(n=>({label:n,run:()=>CF.run(n,{source:'menu'})})):[{label:'(none)',disabled:true}]};
 const props={label:'Properties',icon:'properties',shortcut:'Ctrl+1',run:cmd('PROPERTIES',()=>CF.toggle('properties',true))};
 if(CF.selection().length)return[repeat,recent,SEP,{label:'Erase',icon:'erase',shortcut:'Del',run:cmd('ERASE',()=>eraseSelection())},{label:'Move',icon:'move',run:cmd('MOVE',()=>setTool('move'))},{label:'Copy Selection',icon:'copy',run:cmd('COPY',()=>setTool('copy'))},{label:'Scale',icon:'scale',run:cmd('SCALE',()=>setTool('scale'))},{label:'Rotate',icon:'rotate',run:cmd('ROTATE',()=>setTool('rotate'))},{label:'Mirror',icon:'mirror',run:cmd('MIRROR',()=>setTool('mirror'))},{label:'Explode',icon:'explode',run:cmd('EXPLODE',()=>explode())},SEP,{label:'Deselect All',run:()=>CF.deselect()},SEP,props];
 return[repeat,recent,SEP,{label:'Undo',icon:'undo',shortcut:'Ctrl+Z',run:cmd('UNDO',()=>undo())},{label:'Redo',icon:'redo',shortcut:'Ctrl+Y',run:cmd('REDO',()=>undo(true))},SEP,panItem,zoomExt,SEP,{label:'Select All',shortcut:'Ctrl+A',run:cmd('SELECTALL',()=>selectAll())},SEP,props]}
const CHEVRON='<svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.4"><path d="M3.5 1.5 7 5l-3.5 3.5"/></svg>',CHECK='<svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.6"><path d="m3 7.5 2.6 2.6L11 4.6"/></svg>';
function buildMenu(items,sub){const m=document.createElement('div');m.className=sub?'cf-ctxmenu cf-ctx-sub':'cf-ctxmenu';m.setAttribute('role','menu');
 for(const it of items){if(it.sep){const s=document.createElement('div');s.className='cf-ctx-sep';m.append(s);continue}
  const row=document.createElement('div');row.className='cf-ctx-item'+(it.disabled?' disabled':'');row.setAttribute('role','menuitem');
  const ic=document.createElement('span');ic.className='ic';if(it.checked)ic.innerHTML=CHECK;else if(it.icon&&CF.has('icons'))ic.innerHTML=CF.icon(it.icon,16);
  const lb=document.createElement('span');lb.className='lb';lb.textContent=it.label;const sc=document.createElement('span');sc.className='sc';sc.textContent=it.shortcut||'';row.append(ic,lb,sc);
  if(it.sub){const ar=document.createElement('span');ar.className='ar';ar.innerHTML=CHEVRON;const subMenu=buildMenu(it.sub,true);row.append(ar,subMenu);row.classList.add('has-sub');row.onmouseenter=()=>placeSub(row,subMenu)}
  else if(!it.disabled)row.onclick=ev=>{ev?.stopPropagation?.();closeMenu();try{it.run()}catch(err){notify(`${it.label} failed: ${err?.message||err}`)}};
  row.onmousemove=()=>{const rows=menuRows();st.menuIndex=rows.indexOf(row);rows.forEach(r=>r.classList.toggle('hot',r===row))};
  m.append(row)}
 m.oncontextmenu=e=>e.preventDefault?.();m.onpointerdown=e=>e.stopPropagation?.();return m}
function placeSub(row,subMenu){try{subMenu.style.left='100%';subMenu.style.right='auto';subMenu.style.top='-4px';const r=subMenu.getBoundingClientRect?.();if(r&&r.width&&r.right>innerWidth-4){subMenu.style.left='auto';subMenu.style.right='100%'}if(r&&r.height&&r.bottom>innerHeight-4)subMenu.style.top=`${Math.round(innerHeight-4-r.bottom-4)}px`}catch(err){}}
const menuRows=()=>st.menu?[...st.menu.children].filter(r=>r.classList?.contains('cf-ctx-item')&&!r.classList.contains('disabled')):[];
function menuMove(d){const rows=menuRows();if(!rows.length)return;st.menuIndex=(st.menuIndex+d+rows.length)%rows.length;rows.forEach((r,i)=>r.classList.toggle('hot',i===st.menuIndex))}
function menuActivate(){const row=menuRows()[st.menuIndex];row?.onclick?.()}
function closeMenu(){if(!st.menu)return;try{st.menu.remove()}catch(err){}st.menu=null;st.menuIndex=-1}
function openMenu(x,y,items){closeMenu();const m=buildMenu(items,false);m.style.left=`${Math.round(x)}px`;m.style.top=`${Math.round(y)}px`;document.body.append(m);st.menu=m;
 try{const r=m.getBoundingClientRect();if(r.width&&r.right>innerWidth-2)m.style.left=`${Math.max(2,Math.round(x-r.width))}px`;if(r.height&&r.bottom>innerHeight-2)m.style.top=`${Math.max(2,Math.round(innerHeight-r.height-2))}px`}catch(err){}return m}
CF.openContextMenu=function(x,y,kind){return openMenu(x,y,menuModel(kind))};
try{document.addEventListener('pointerdown',e=>{if(st.menu&&!st.menu.contains(e.target))closeMenu()},true);window.addEventListener('blur',closeMenu);window.addEventListener('resize',closeMenu)}catch(err){}

// ---- Styles -------------------------------------------------------------------------------------
const style=document.createElement('style');style.id='cf-interact-style';style.textContent=`
.cf-ctxmenu{position:fixed;z-index:1000;min-width:210px;padding:3px 0;background:#2b323c;border:1px solid #4a5260;box-shadow:0 6px 20px rgba(0,0,0,.5);font:12px var(--cf-font);color:var(--cf-text);user-select:none;pointer-events:auto}
.cf-ctx-item{position:relative;display:flex;align-items:center;height:24px;padding-right:10px;white-space:nowrap;cursor:default}
.cf-ctx-item .ic{flex:0 0 30px;display:flex;align-items:center;justify-content:center;color:#dfe5ec}
.cf-ctx-item .lb{flex:1;padding-right:16px}.cf-ctx-item .sc{color:var(--cf-text-faint);font-size:11px}.cf-ctx-item .ar{display:flex;margin-left:6px;color:var(--cf-text-dim)}
.cf-ctx-item:hover,.cf-ctx-item.hot{background:var(--cf-active);outline:1px solid var(--cf-active-border);outline-offset:-1px}
.cf-ctx-item.disabled{color:var(--cf-text-faint)}.cf-ctx-item.disabled:hover{background:none;outline:0}
.cf-ctx-sep{height:1px;margin:3px 4px 3px 30px;background:#4a5260}
.cf-ctx-sub{position:absolute;left:100%;top:-4px;display:none}.cf-ctx-item.has-sub:hover>.cf-ctx-sub{display:block}
.cf-dyn{position:absolute;left:0;top:0;display:none;align-items:center;gap:4px;padding:2px 3px 2px 6px;background:rgba(46,53,64,.96);border:1px solid #5a6676;box-shadow:0 2px 8px rgba(0,0,0,.4);color:var(--cf-text);font:11px var(--cf-font);white-space:nowrap;pointer-events:none;z-index:6}
.cf-dyn .p{color:#d6dde6;padding-right:2px}.cf-dyn .o{color:var(--cf-text-dim)}
.cf-dyn .f{min-width:62px;padding:1px 5px;background:#1c2128;border:1px solid #4a5260;font:11px var(--cf-mono);color:#e6edf5}
.cf-dyn .f.on{border-color:var(--cf-accent);background:#173352}
#canvas:focus{outline:none}`;
try{document.head.append(style)}catch(err){}

// ---- Testable surface --------------------------------------------------------------------------
CF.interact={typedSnap,snapKeyword,pointPrompt,setOverride,osnapCandidates,gridStep,orthoProject,polarProject,windowSelect,pickAt,finishWindow,gripsFor,gripAt,applyGrip,startGrip,cancelGrip,parsePoint,menuModel,promptOptions,shortPrompt,exitPan,resetTransient,selectedIndices,state:()=>st};
})();
