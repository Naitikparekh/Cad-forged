'use strict';
// B2 drafting commands with AutoCAD prompts: OFFSET TRIM EXTEND FILLET CHAMFER MIRROR POLYGON ELLIPSE DIMLINEAR DIST ID LIST MATCHPROP ARRAYRECT PURGE.
// Each command runs as an async "flow" on its own engine tool. Clicks reach the flow through accept(), typed option keywords
// and numbers through the command input / CF.run, Enter through CF.enter()/finish(), and any setTool() aborts it.
CF.drafting=(()=>{
const G=CF.geom,TAU=Math.PI*2;
const P=(x,y)=>({x,y}),sub=(a,b)=>P(a.x-b.x,a.y-b.y),plus=(a,b)=>P(a.x+b.x,a.y+b.y),mul=(a,s)=>P(a.x*s,a.y*s),dot=(a,b)=>a.x*b.x+a.y*b.y;
const unit=a=>{const l=Math.hypot(a.x,a.y);return l<1e-15?null:P(a.x/l,a.y/l)},lerp=(a,b,t)=>P(a.x+(b.x-a.x)*t,a.y+(b.y-a.y)*t),cross=(a,b,p)=>(b.x-a.x)*(p.y-a.y)-(b.y-a.y)*(p.x-a.x);
const clamp1=v=>Math.max(-1,Math.min(1,v)),normAng=a=>((a%TAU)+TAU)%TAU;
const fmt=n=>(Math.abs(n)<5e-5?0:n).toFixed(4),deg=n=>String(Number((Math.abs(n)<5e-5?0:n).toFixed(4)));
const area=ps=>{let s=0;for(let i=0;i<ps.length;i++){const a=ps[i],b=ps[(i+1)%ps.length];s+=a.x*b.y-b.x*a.y}return s/2};
const inside=(p,ps)=>{let c=false;for(let i=0,j=ps.length-1;i<ps.length;j=i++){const a=ps[i],b=ps[j];if((a.y>p.y)!==(b.y>p.y)&&p.x<(b.x-a.x)*(p.y-a.y)/(b.y-a.y)+a.x)c=!c}return c};
const overlap=(a,b,m=1e-7)=>a.minX<=b.maxX+m&&b.minX<=a.maxX+m&&a.minY<=b.maxY+m&&b.minY<=a.maxY+m;
const segBox=(a,b)=>({minX:Math.min(a.x,b.x),minY:Math.min(a.y,b.y),maxX:Math.max(a.x,b.x),maxY:Math.max(a.y,b.y)});
const S={offsetDist:null,filletRad:0,chamferA:0,chamferB:0,chamferLen:0,chamferAng:0,chamferMethod:'distance',sides:4,polyMode:'I',matchLayer:true,matchText:true,rows:3,cols:4,lastReport:[]};

// ---- Pure geometry ----------------------------------------------------------------------------------
// Arc polyline from angle a0 sweeping sw (signed), at least `per` segments per 90 degrees.
function arcPoints(c,r,a0,sw,per=8){const n=Math.max(2,Math.ceil(Math.abs(sw)/(Math.PI/2)*per-1e-9));return Array.from({length:n+1},(_,i)=>P(c.x+r*Math.cos(a0+sw*i/n),c.y+r*Math.sin(a0+sw*i/n)))}
// Offset a path by d (d>0 = left of travel direction); joins are mitred by intersecting neighbouring offset segments.
function offsetPath(pts,closed,d){let ps=pts.filter((p,i)=>!i||G.dist(p,pts[i-1])>1e-9);if(closed&&ps.length>2&&G.dist(ps[0],ps.at(-1))<1e-9)ps=ps.slice(0,-1);const n=ps.length;if(n<2)return null;const segs=[];for(let i=0;i<(closed?n:n-1);i++){const a=ps[i],b=ps[(i+1)%n],L=G.dist(a,b),o=P(-(b.y-a.y)/L*d,(b.x-a.x)/L*d);segs.push([plus(a,o),plus(b,o)])}
 const join=(s,t)=>{const q=G.segmentIntersection(s[0],s[1],t[0],t[1],true);return q?P(q.x,q.y):{...s[1]}};const out=[];
 if(closed&&n>2)for(let i=0;i<n;i++)out.push(join(segs[(i-1+n)%n],segs[i]));else{out.push({...segs[0][0]});for(let i=1;i<segs.length;i++)out.push(join(segs[i-1],segs[i]));out.push({...segs.at(-1)[1]})}return out}
// Which side of a polyline a point is on (+1 left of travel) and its distance to the path.
function polySide(e,p){let best=Infinity,cand=[];for(const [a,b]of G.segments(e)){const q=G.closestOnSegment(p,a,b);if(q.d<best-1e-9){best=q.d;cand=[[a,b]]}else if(Math.abs(q.d-best)<=1e-9)cand.push([a,b])}
 if(e.closed&&e.points.length>2)return{sign:inside(p,e.points)===(area(e.points)>0)?1:-1,dist:best};let s=1,m=-1;for(const [a,b]of cand){const c=cross(a,b,p)/(G.dist(a,b)||1);if(Math.abs(c)>m){m=Math.abs(c);s=Math.sign(c)||1}}return{sign:s,dist:best}}
// OFFSET one object: dist null = through point p. Returns the new entity or null when the offset is impossible.
function offsetEntity(e,dist,p){const out=structuredClone(e);for(const k of ['group','blockName','uuid','hatch'])delete out[k];
 if(e.type==='circle'){const dp=G.dist(p,e.center),r=dist==null?dp:dp<e.radius?e.radius-dist:e.radius+dist;if(!(r>1e-9))return null;out.radius=r;return out}
 if(e.type==='line'){const [a,b]=e.points,L=G.dist(a,b);if(L<1e-12)return null;const s=cross(a,b,p)/L,d=dist==null?s:(s<0?-1:1)*dist;if(Math.abs(d)<1e-12)return null;const o=P(-(b.y-a.y)/L*d,(b.x-a.x)/L*d);out.points=e.points.map(q=>plus(q,o));return out}
 if(e.type==='polyline'){const side=polySide(e,p),d=side.sign*(dist==null?side.dist:dist);if(Math.abs(d)<1e-12)return null;const ps=offsetPath(e.points,e.closed,d);if(!ps)return null;if(e.closed&&ps.length>2){const a0=area(e.points),a1=area(ps);if(Math.sign(a0)!==Math.sign(a1)||Math.abs(a1)<1e-9)return null}out.points=ps;return out}
 return null}
// Cutting edges / boundaries: every other visible line, polyline and circle (hatch lines and text excluded).
function edges(skip){return doc.entities.filter(e=>e!==skip&&e.type!=='text'&&!e.hatch&&CF.visible(e))}
function segHits(a,b,c){if(c.type==='circle')return G.lineCircle(a,b,c.center,c.radius).map(q=>q.t);const out=[];for(const [p,q]of G.segments(c)){const x=G.segmentIntersection(a,b,p,q);if(x)out.push(x.t)}return out}
// Sorted path parameters (segment index + t) where entity e is crossed by the cutters.
function pathCuts(e,cs){const boxes=cs.map(c=>[c,G.bbox(c)]),out=[];G.segments(e).forEach(([a,b],k)=>{const sb=segBox(a,b);for(const [c,bx]of boxes)if(overlap(sb,bx))for(const t of segHits(a,b,c))out.push(k+Math.max(0,Math.min(1,t)))});out.sort((x,y)=>x-y);return out.filter((s,i)=>!i||s-out[i-1]>1e-9)}
function nearestSeg(e,p){let best={d:Infinity,k:-1,t:0};G.segments(e).forEach(([a,b],k)=>{const q=G.closestOnSegment(p,a,b);if(q.d<best.d-1e-12)best={d:q.d,k,t:q.t}});return best}
// Points of the path between parameters s1 < s2 (closed paths wrap modulo the vertex count).
function subPath(ps,closed,s1,s2){const N=ps.length,at=s=>{let k=Math.floor(s+1e-12),t=Math.max(0,s-k);if(!closed&&k>=N-1){k=N-2;t=1}return lerp(ps[((k%N)+N)%N],ps[(((k+1)%N)+N)%N],t)};const out=[at(s1)];for(let k=Math.floor(s1+1e-12)+1;k<s2-1e-9;k++)out.push({...ps[k%N]});out.push(at(s2));return out.filter((q,i)=>!i||G.dist(q,out[i-1])>1e-12)}
// Quick TRIM of the piece of e around p. Returns {add:[replacement entities], removed:[points]} or {error}.
function trimEntity(e,p,cs=edges(e)){
 const base=()=>{const c=structuredClone(e);delete c.uuid;return c};
 if(e.type==='circle'){const c=e.center,bx=G.bbox(e),angs=[];for(const k of cs)if(overlap(bx,G.bbox(k)))for(const q of G.intersections(e,k))angs.push(normAng(Math.atan2(q.y-c.y,q.x-c.x)));angs.sort((a,b)=>a-b);const u=angs.filter((a,i)=>!i||a-angs[i-1]>1e-9);if(u.length>1&&u[0]+TAU-u.at(-1)<1e-9)u.pop();
  if(u.length<2)return{error:u.length?'A circle must cross two cutting edges to be trimmed.':'No cutting edges'};const ap=normAng(Math.atan2(p.y-c.y,p.x-c.x));let lo=null,hi=null;for(const a of u){if(a<=ap)lo=a;else if(hi==null)hi=a}if(lo==null)lo=u.at(-1);if(hi==null)hi=u[0];
  let end=lo;while(end<=hi+1e-12)end+=TAU;const keep=arcPoints(c,e.radius,hi,end-hi,16),rem=(hi-lo+TAU)%TAU||TAU,arc={type:'polyline',layer:e.layer,closed:false,points:keep};if(e.group)arc.group=e.group;return{add:[arc],removed:arcPoints(c,e.radius,lo,rem,16)}}
 if(e.type!=='line'&&e.type!=='polyline')return{error:'Cannot trim that object.'};
 const ps=e.points,closed=e.type==='polyline'&&!!e.closed&&ps.length>2,n=closed?ps.length:ps.length-1;if(n<1)return{error:'Cannot trim that object.'};
 const cuts=[...new Set(pathCuts(e,cs).map(s=>closed&&s>n-1e-9?0:s))].filter(s=>closed||s>1e-9&&s<n-1e-9).sort((a,b)=>a-b);if(!cuts.length)return{error:'No cutting edges'};
 const q=nearestSeg(e,p),sp=q.k+q.t,piece=pts=>{const c=base();c.points=pts;if(c.type==='polyline')c.closed=false;return c};
 if(!closed){let lo=null,hi=null;for(const s of cuts){if(s<sp-1e-12)lo=s;else if(s>sp+1e-12&&hi==null)hi=s}const add=[];if(lo!=null)add.push(piece(subPath(ps,false,0,lo)));if(hi!=null)add.push(piece(subPath(ps,false,hi,n)));if(add[0]&&e.uuid)add[0].uuid=e.uuid;return{add:add.filter(c=>c.points.length>1),removed:subPath(ps,false,lo??0,hi??n)}}
 if(cuts.length<2)return{error:'A closed polyline must cross two cutting edges to be trimmed.'};let lo=null,hi=null;for(const s of cuts){if(s<sp-1e-12)lo=s;else if(s>sp+1e-12&&hi==null)hi=s}if(lo==null)lo=cuts.at(-1);if(hi==null)hi=cuts[0];
 const keep=piece(subPath(ps,true,hi,lo>hi?lo:lo+n));if(e.uuid)keep.uuid=e.uuid;return{add:[keep],removed:subPath(ps,true,lo,hi>lo?hi:hi+n)}}
// EXTEND the end of a line/open polyline nearest p to the closest boundary along its direction.
function extendEntity(e,p,cs=edges(e)){if(!(e.type==='line'||e.type==='polyline'&&!e.closed))return{error:'Cannot extend that object.'};const ps=e.points,N=ps.length;if(N<2)return{error:'Cannot extend that object.'};let atStart;
 if(e.type==='line')atStart=G.closestOnSegment(p,ps[0],ps[1]).t<.5;else{const q=nearestSeg(e,p);let total=0,upto=0;G.segments(e).forEach(([a,b],k)=>{const L=G.dist(a,b);if(k<q.k)upto+=L;if(k===q.k)upto+=L*q.t;total+=L});atStart=upto<total/2}
 const a=atStart?ps[1]:ps[N-2],b=atStart?ps[0]:ps[N-1];if(G.dist(a,b)<1e-12)return{error:'Cannot extend that object.'};let best=Infinity;
 for(const c of cs){if(c.type==='circle'){for(const q of G.lineCircle(a,b,c.center,c.radius,true))if(q.t>1+1e-9&&q.t<best)best=q.t}else for(const [u,v]of G.segments(c)){const x=G.segmentIntersection(a,b,u,v,true);if(x&&x.u>=-1e-9&&x.u<=1+1e-9&&x.t>1+1e-9&&x.t<best)best=x.t}}
 if(!Number.isFinite(best))return{error:'Object does not intersect an edge.'};const q=lerp(a,b,best),points=ps.map(v=>({...v}));points[atStart?0:N-1]=q;return{points,from:{...b},to:q}}
// Corner of two lines picked at p1/p2: intersection I, unit directions d1/d2 towards the kept (picked) sides,
// index of the endpoint to move on each line, and the kept lengths L1/L2 measured from I.
function corner(l1,p1,l2,p2){const [a1,b1]=l1,[a2,b2]=l2,X=G.segmentIntersection(a1,b1,a2,b2,true);if(!X)return{error:'Lines are parallel.'};const I=P(X.x,X.y);
 const side=(a,b,p)=>{const u=unit(sub(b,a));if(!u)return null;const s=dot(sub(p,I),u);let d;if(Math.abs(s)>1e-9)d=mul(u,Math.sign(s));else{const sa=dot(sub(a,I),u),sb=dot(sub(b,I),u);d=mul(u,Math.sign(Math.abs(sa)>Math.abs(sb)?sa:sb)||1)}const sa=dot(sub(a,I),d),sb=dot(sub(b,I),d);return{d,near:sa<sb?0:1,far:Math.max(sa,sb)}};
 const s1=side(a1,b1,p1),s2=side(a2,b2,p2);if(!s1||!s2)return{error:'Zero-length line.'};if(s1.far<=1e-9||s2.far<=1e-9)return{error:'Pick the lines on the sides to keep.'};return{I,d1:s1.d,d2:s2.d,n1:s1.near,n2:s2.near,L1:s1.far,L2:s2.far,theta:Math.acos(clamp1(dot(s1.d,s2.d)))}}
const moveEnd=(pts,i,q)=>{const r=pts.map(p=>({...p}));r[i]={...q};return G.dist(r[0],r[1])>1e-9?r:null};
// FILLET two lines with radius r (0 = sharp corner). Returns {line1,line2,arc} (null line = fully consumed) or {error}.
function filletLines(l1,p1,l2,p2,r){const c=corner(l1,p1,l2,p2);if(c.error)return c;if(!(r>1e-12))return{line1:moveEnd(l1,c.n1,c.I),line2:moveEnd(l2,c.n2,c.I),arc:null};
 if(c.theta<1e-9||Math.PI-c.theta<1e-9)return{error:'Lines are parallel.'};const t=r/Math.tan(c.theta/2);if(t>c.L1+1e-9||t>c.L2+1e-9)return{error:'Radius is too large.'};
 const T1=plus(c.I,mul(c.d1,t)),T2=plus(c.I,mul(c.d2,t)),C=plus(c.I,mul(unit(plus(c.d1,c.d2)),r/Math.sin(c.theta/2))),a0=Math.atan2(T1.y-C.y,T1.x-C.x);let sw=Math.atan2(T2.y-C.y,T2.x-C.x)-a0;while(sw>Math.PI)sw-=TAU;while(sw<-Math.PI)sw+=TAU;
 const arc=arcPoints(C,r,a0,sw);arc[0]={...T1};arc[arc.length-1]={...T2};return{line1:moveEnd(l1,c.n1,T1),line2:moveEnd(l2,c.n2,T2),arc,center:C}}
// Second chamfer distance for the angle method (length d1 on the first line, chamfer at `angle` degrees from it).
const angleDist=(d1,angle,theta)=>{const a=angle*Math.PI/180,den=Math.sin(Math.PI-theta-a);return den>1e-9&&a>0?d1*Math.sin(a)/den:NaN};
// CHAMFER two lines: d1 on the first line, d2 on the second (or angle method when angle != null).
function chamferLines(l1,p1,l2,p2,d1,d2,angle=null){const c=corner(l1,p1,l2,p2);if(c.error)return c;if(angle!=null){d2=angleDist(d1,angle,c.theta);if(!Number.isFinite(d2))return{error:'Invalid chamfer angle for these lines.'}}
 if(d1>c.L1+1e-9||d2>c.L2+1e-9)return{error:'Distance is too large.'};const C1=plus(c.I,mul(c.d1,d1)),C2=plus(c.I,mul(c.d2,d2));return{line1:moveEnd(l1,c.n1,C1),line2:moveEnd(l2,c.n2,C2),chamfer:G.dist(C1,C2)>1e-9?[C1,C2]:null}}
// Replacement points for polyline vertex k. spec {kind:'fillet',r} or {kind:'chamfer',d1,d2,angle}; firstPrev: the first
// distance applies to the segment before k. half: limit each cut to half a segment (whole-polyline mode).
function vertexCorner(ps,k,spec,firstPrev=true,half=false){const N=ps.length,V=ps[k];let A=ps[(k-1+N)%N],B=ps[(k+1)%N];if(!firstPrev)[A,B]=[B,A];const u1=unit(sub(A,V)),u2=unit(sub(B,V));if(!u1||!u2)return{skip:'zero'};
 const L1=G.dist(A,V)*(half?.5:1),L2=G.dist(B,V)*(half?.5:1),th=Math.acos(clamp1(dot(u1,u2)));if(th<1e-9||Math.PI-th<1e-7)return{skip:'parallel'};let out;
 if(spec.kind==='fillet'){if(!(spec.r>1e-12))return{skip:'zero'};const t=spec.r/Math.tan(th/2);if(t>L1+1e-9||t>L2+1e-9)return{skip:'short'};const T1=plus(V,mul(u1,t)),T2=plus(V,mul(u2,t)),C=plus(V,mul(unit(plus(u1,u2)),spec.r/Math.sin(th/2))),a0=Math.atan2(T1.y-C.y,T1.x-C.x);let sw=Math.atan2(T2.y-C.y,T2.x-C.x)-a0;while(sw>Math.PI)sw-=TAU;while(sw<-Math.PI)sw+=TAU;out=arcPoints(C,spec.r,a0,sw);out[0]=T1;out[out.length-1]=T2}
 else{const d1=spec.d1,d2=spec.angle!=null?angleDist(d1,spec.angle,th):spec.d2;if(!Number.isFinite(d2))return{skip:'angle'};if(d1>L1+1e-9||d2>L2+1e-9)return{skip:'short'};if(d1<1e-12&&d2<1e-12)return{skip:'zero'};const C1=plus(V,mul(u1,d1)),C2=plus(V,mul(u2,d2));out=G.dist(C1,C2)>1e-9?[C1,C2]:[C1]}
 return{points:firstPrev?out:out.reverse()}}
// FILLET/CHAMFER every vertex of a polyline (the Polyline option).
function cornerPolyline(e,spec){const ps=e.points.filter((p,i)=>!i||G.dist(p,e.points[i-1])>1e-9);if(e.closed&&ps.length>2&&G.dist(ps[0],ps.at(-1))<1e-9)ps.pop();const closed=!!e.closed&&ps.length>2,out=[];let count=0,short=0;
 ps.forEach((V,k)=>{if(!closed&&(k===0||k===ps.length-1)){out.push({...V});return}const r=vertexCorner(ps,k,spec,true,true);if(r.points){out.push(...r.points);count++}else{out.push({...V});if(r.skip==='short')short++}});return{points:out,count,short}}
// POLYGON vertices: mode 'I' = radius to a vertex at `angle`; 'C' = radius to an edge midpoint at `angle`.
function polygonPoints(n,c,r,angle=0,mode='I'){if(!(n>=3)||!(r>1e-12))return null;const R=mode==='C'?r/Math.cos(Math.PI/n):r,a0=mode==='C'?angle+Math.PI/n:angle;return Array.from({length:n},(_,k)=>P(c.x+R*Math.cos(a0+TAU*k/n),c.y+R*Math.sin(a0+TAU*k/n)))}
// POLYGON from an edge: counterclockwise, the polygon lies to the left of a->b.
function polygonEdge(n,a,b){const L=G.dist(a,b);if(!(n>=3)||L<1e-12)return null;const out=[{...a}],ang0=G.angle(a,b);let p={...a};for(let k=0;k<n-1;k++){p=G.polar(p,L,ang0+TAU*k/n);out.push(p)}return out}
// ELLIPSE as a closed 72-segment polyline: center c, axis endpoint e (first semi-axis), b = other semi-axis length.
function ellipsePoints(c,e,b,count=72){const a=G.dist(c,e),u=unit(sub(e,c));if(!u||!(b>1e-12))return null;const v=P(-u.y,u.x);return Array.from({length:count},(_,k)=>{const t=TAU*k/count,ca=Math.cos(t),sa=Math.sin(t);return P(c.x+u.x*a*ca+v.x*b*sa,c.y+u.y*a*ca+v.y*b*sa)})}
// DIMLINEAR geometry: horizontal ('h') or vertical ('v'), chosen from the location like AutoCAD when orient is null.
function dimOrient(a,b,pos){const x0=Math.min(a.x,b.x),x1=Math.max(a.x,b.x),y0=Math.min(a.y,b.y),y1=Math.max(a.y,b.y),ox=Math.max(0,x0-pos.x,pos.x-x1),oy=Math.max(0,y0-pos.y,pos.y-y1);if(ox<=0&&oy>0)return'h';if(oy<=0&&ox>0)return'v';if(ox<=0&&oy<=0)return x1-x0>=y1-y0?'h':'v';return ox>oy?'v':'h'}
function dimLinear(a,b,pos,orient=null){const o=orient||dimOrient(a,b,pos),A=o==='h'?P(a.x,pos.y):P(pos.x,a.y),B=o==='h'?P(b.x,pos.y):P(pos.x,b.y),L=G.dist(A,B);if(L<1e-9)return null;
 const u=unit(sub(B,A)),n=P(-u.y,u.x),size=Math.min(3,L/8),h=3,text=L.toFixed(2),w=text.length*h*.6,ents=[];
 for(const [p,q]of [[a,A],[b,B]]){const v=unit(sub(q,p));if(v)ents.push({type:'line',points:[{...p},plus(q,mul(v,1.25))]})} // extension lines overshoot the dimension line (DIMEXE)
 ents.push({type:'line',points:[{...A},{...B}]});for(const [p,dir]of [[A,1],[B,-1]])for(const side of [-1,1])ents.push({type:'line',points:[{...p},P(p.x+dir*u.x*size+n.x*side*size*.35,p.y+dir*u.y*size+n.y*side*size*.35)]});
 const m=G.mid(A,B),at=o==='h'?P(m.x-w/2,pos.y+1):P(pos.x>=(a.x+b.x)/2?pos.x+1:pos.x-1-w,m.y-h/2);ents.push({type:'text',points:[at],text,height:h});return{entities:ents,text,value:L,orient:o}}
function mirrorEntity(e,a,b){const u=unit(sub(b,a));if(!u)return null;const m=q=>{const v=sub(q,a),d=dot(v,u);return P(a.x+2*d*u.x-v.x,a.y+2*d*u.y-v.y)},c=structuredClone(e);
 if(e.type==='circle')c.center=m(e.center);else if(e.type==='text'){const w=String(e.text).length*e.height*.6,ctr=m(P(e.points[0].x+w/2,e.points[0].y+e.height/2));c.points=[P(ctr.x-w/2,ctr.y-e.height/2)]}else c.points=e.points.map(m);return c} // text stays readable (MIRRTEXT=0)
const pt=p=>`X=${fmt(p.x)}  Y=${fmt(p.y)}  Z=0.0000`;
function distReport(a,b){const dx=b.x-a.x,dy=b.y-a.y;let ang=Math.atan2(dy,dx)*180/Math.PI;if(ang<0)ang+=360;if(Math.hypot(dx,dy)<1e-12)ang=0;return[`Distance = ${fmt(Math.hypot(dx,dy))},  Angle in XY Plane = ${deg(ang)}°,  Angle from XY Plane = 0°`,`Delta X = ${fmt(dx)},  Delta Y = ${fmt(dy)},  Delta Z = 0.0000`]}
function idReport(p){return[`X = ${fmt(p.x)}     Y = ${fmt(p.y)}     Z = 0.0000`]}
function listReport(e){const head=(t)=>`${t}  Layer: "${e.layer}"  Space: Model space${e.blockName?`  Block: "${e.blockName}"`:e.group?'  (grouped)':''}`;
 if(e.type==='line'){const [a,b]=e.points,dx=b.x-a.x,dy=b.y-a.y;let ang=Math.atan2(dy,dx)*180/Math.PI;if(ang<0)ang+=360;return[head(e.hatch?'LINE (hatch)':'LINE'),`  from point, ${pt(a)}`,`  to point,   ${pt(b)}`,`  Length = ${fmt(Math.hypot(dx,dy))},  Angle in XY Plane = ${deg(ang)}°`,`  Delta X = ${fmt(dx)},  Delta Y = ${fmt(dy)},  Delta Z = 0.0000`]}
 if(e.type==='circle')return[head('CIRCLE'),`  center point, ${pt(e.center)}`,`  radius ${fmt(e.radius)}`,`  circumference ${fmt(TAU*e.radius)}`,`  area ${fmt(Math.PI*e.radius**2)}`];
 if(e.type==='text')return[head('TEXT'),'  Style = "Standard"',`  start point, ${pt(e.points[0])}`,`  height ${fmt(e.height)}`,`  text "${e.text}"`,'  rotation angle 0°'];
 const ps=e.points,L=G.segments(e).reduce((s,[a,b])=>s+G.dist(a,b),0),out=[head('LWPOLYLINE'),`  ${e.closed?'Closed':'Open'}`];if(e.closed)out.push(`  area ${fmt(Math.abs(area(ps)))}`);out.push(`  ${e.closed?'perimeter':'length'} ${fmt(L)}`);
 ps.slice(0,40).forEach(p=>out.push(`  at point  ${pt(p)}`));if(ps.length>40)out.push(`  ... ${ps.length-40} more vertices`);return out}
// Unreferenced blocks, and layers used by no entity and no block definition (purging blocks first can free more layers).
function purgeAnalysis(d=doc,current=$('layer').value){const used=new Set(d.entities.map(e=>e.blockName).filter(Boolean)),blocks=Object.keys(d.blocks||{}).filter(n=>!used.has(n)),items=Object.values(d.blocks||{}).flatMap(b=>b.items||[]),usedLayers=new Set([...d.entities,...items].map(e=>e.layer));
 return{blocks,layers:d.layers.map(l=>l.name).filter(n=>n!=='0'&&n!==current&&!usedLayers.has(n))}}
function arrayCopies(ents,rows,cols,dy,dx){const out=[];for(let r=0;r<rows;r++)for(let c=0;c<cols;c++){if(!r&&!c)continue;const groups={};for(const e of ents){const k=structuredClone(e);delete k.uuid;if(k.group)k.group=groups[k.group]??=gid();transformEntity(k,q=>P(q.x+c*dx,q.y+r*dy));out.push(k)}}return out}

// ---- Command flow runtime -------------------------------------------------------------------------------
const TOOLS={};let flow=null,serial=0,starting=false;const idleQ=[];
// Waiters are released when the command is waiting/ended; force releases them while a command-line question is pending.
const idleFlush=force=>{if(force||!flow||flow.waiter||flow.picking)while(idleQ.length)idleQ.shift()()};
// Resolves once the active command waits for input again (or ends); tests await it after each step.
const idle=()=>new Promise(r=>{if(!flow||flow.waiter||flow.picking)r();else idleQ.push(r)});
const hasB1=()=>CF.has('commands');
function optionsOf(prompt){const m=String(prompt).match(/\[([^\]]+)\]/);return m?m[1].split('/').map(w=>({word:w,key:(w.match(/[A-Z]/g)||[w[0]]).join('').toUpperCase()})):[]}
// Option keyword match: the capitalised shortcut letters or any abbreviation of the whole word.
function matchOption(text,opts){const t=String(text??'').trim().toUpperCase();if(!t)return null;const list=typeof opts==='string'?optionsOf(opts):opts.map(o=>typeof o==='string'?{word:o,key:(o.match(/[A-Z]/g)||[o[0]]).join('').toUpperCase()}:o);const o=list.find(o=>o.key===t)||list.find(o=>t.length>1&&o.word.toUpperCase().startsWith(t));return o?o.word:null}
function hl(f,ids){if(ids.length){CF.select(ids,'replace');f.hl=true}else if(f.hl){f.hl=false;selectionSet.clear();selected=-1;CF.emit('selection',[]);render()}}
function end(f){f.dead=true;if(f.hl)hl(f,[]);if(flow===f){flow=null;if(TOOLS[tool])setTool('select')}idleFlush()}
function abort(){const f=flow;if(!f)return;flow=null;f.dead=true;if(f.input&&CF.input===f.input){try{CF.input.resolve?.(null)}catch(err){}if(CF.input===f.input)CF.input=null}if(f.picking){const pk=f.picking;f.picking=null;if(CF.picking===pk.obj)CF.picking=null;pk.resolve(null)}const w=f.waiter;if(w)w.resolve({cancel:true});if(f.hl)hl(f,[]);idleFlush()}
function start(name){const spec=TOOLS[name];abort();const f={name,cmd:spec.cmd,id:++serial,waiter:null,picking:null,count:0,dead:false,hl:false,ghost:null};flow=f;starting=true;try{setTool(name)}finally{starting=false}
 return(async()=>{try{await spec.run(f)}catch(err){console.error(err);notify(`${spec.cmd} failed: ${err?.message||err}`)}finally{end(f)}})()}
// Wait for a click / option / number / Enter. phase: {prompt, pick, base, number, preview(mouse)}.
function wait(f,phase){if(f.dead||flow!==f)return Promise.resolve({cancel:true});phase.options=optionsOf(phase.prompt);points=phase.base?[{...phase.base}]:[];
 return new Promise(res=>{const w={phase,resolve:v=>{if(f.waiter===w)f.waiter=null;res(v)}};f.waiter=w;if(!hasB1())notify(`${f.cmd} ${phase.prompt}`);render();idleFlush()})}
function respond(r){const w=flow?.waiter;if(!w)return false;const p=new Promise(res=>idleQ.push(res));w.resolve(r);return p}
// Command-line question through cadPrompt (B1 routes it to the command line). Empty input returns the default.
async function ask(f,message,def='',o={}){if(f.dead)return null;points=o.base?[{...o.base}]:[];const pr=Promise.resolve(cadPrompt(message,hasB1()?'':String(def),{kind:o.kind||'text',base:o.base,defaultValue:String(def)}));
 f.input=CF.input||null;try{if(CF.input&&o.kind){CF.input.kind=o.kind;if(o.base)CF.input.base=o.base}}catch(err){}let settled=false;pr.then(()=>settled=true,()=>settled=true);setTimeout(()=>{if(!settled)idleFlush(true)},0);
 const v=await pr;if(f.dead||v==null)return null;const s=String(v).trim();return s===''?String(def):s}
async function askNum(f,message,def,o={}){for(;;){const raw=await ask(f,message,def,o);if(raw==null)return null;const v=Number(raw);if(raw!==''&&Number.isFinite(v)&&(!o.int||Number.isInteger(v))&&(o.min==null||v>=o.min)&&(o.max==null||v<=o.max)&&(!o.positive||v>0))return v;
 notify(o.int?`Requires an integer between ${o.min} and ${o.max}.`:o.positive?'Value must be positive and nonzero.':o.min!=null?`Requires a value between ${o.min} and ${o.max}.`:'Requires a numeric value.')}}
async function askOption(f,message,def){for(;;){const raw=await ask(f,message,def);if(raw==null)return null;const o=matchOption(raw,message);if(o)return o;notify('Invalid option keyword.')}}
// "Select objects:" phase: uses the current selection (noun-verb) or opens CF.picking until Enter.
function selectObjects(f,cmd){const ids=chosen();if(ids.length)return Promise.resolve(ids);if(f.dead)return Promise.resolve(null);return new Promise(res=>{const obj={command:cmd,message:`${cmd} Select objects:`,draft:true,
 done(){if(f.picking?.obj!==obj)return;f.picking=null;if(CF.picking===obj)CF.picking=null;const ids=chosen();if(!ids.length)notify('No objects selected.');res(ids)},cancel(){if(f.picking?.obj!==obj)return;f.picking=null;if(CF.picking===obj)CF.picking=null;res(null)}};
 f.picking={obj,resolve:res};CF.picking=obj;points=[];if(!hasB1())notify(obj.message);render();idleFlush()})}
function report(lines){S.lastReport=lines;for(const l of lines)notify(l)}
function undoOne(f){if(f.count>0){undo();f.count--}else notify('Everything has been undone.')}
const live=ents=>ents.filter(e=>doc.entities.includes(e));
const pickAt=p=>{const i=hit(p);return i>=0?{i,e:doc.entities[i]}:null};

// ---- Commands ----------------------------------------------------------------------------------------------------
async function runOffset(f){notify('Current settings: Erase source=No  Layer=Source  OFFSETGAPTYPE=0');
 for(;;){const def=S.offsetDist==null?'Through':fmt(S.offsetDist),raw=await ask(f,`Specify offset distance or [Through] <${def}>:`,def,{kind:'distance'});if(raw==null)return;if(matchOption(raw,['Through'])){S.offsetDist=null;break}const v=Number(raw);if(Number.isFinite(v)&&v>0){S.offsetDist=v;break}notify('Value must be positive and nonzero.')}
 for(;;){hl(f,[]);const r=await wait(f,{prompt:'Select object to offset or [Exit/Undo] <Exit>:',pick:true});if(r.cancel||r.enter||r.option==='Exit')return;if(r.option==='Undo'){undoOne(f);continue}if(!r.point)continue;
  const hit0=pickAt(r.point);if(!hit0)continue;if(!['line','circle','polyline'].includes(hit0.e.type)){notify('Cannot offset that object.');continue}hl(f,[hit0.i]);let src=hit0.e,multiple=false;
  for(;;){const dist=S.offsetDist,r2=await wait(f,{prompt:`${dist==null?'Specify through point':'Specify point on side to offset'} or [Exit/Multiple/Undo] <Exit>:`,preview:m=>{const e=offsetEntity(src,dist,m);return e?[e]:[]}});
   if(r2.cancel||r2.option==='Exit')return;if(r2.enter){if(multiple)break;return}if(r2.option==='Undo'){undoOne(f);break}if(r2.option==='Multiple'){multiple=true;continue}if(!r2.point)continue;
   const e=offsetEntity(src,dist,r2.point);if(!e){notify('Cannot offset that object.');if(multiple)continue;break}mutate(()=>doc.entities.push(e));f.count++;if(!multiple)break;src=e}}}
function applyTrim(p){const h=pickAt(p);if(!h)return'';if(h.e.blockName)return'Cannot trim a block reference; explode it first.';const r=trimEntity(h.e,p);if(r.error)return r.error;mutate(()=>{const k=doc.entities.indexOf(h.e);doc.entities.splice(k,1,...r.add)});return true}
function applyExtend(p){const h=pickAt(p);if(!h)return'';if(h.e.blockName)return'Cannot extend a block reference; explode it first.';const r=extendEntity(h.e,p);if(r.error)return r.error;mutate(()=>h.e.points=r.points);return true}
function previewTrimExtend(m,ext){const h=pickAt(m);if(!h||h.e.blockName)return[];if(ext){const r=extendEntity(h.e,m);return r.error?[]:[{type:'line',points:[r.from,r.to]}]}const r=trimEntity(h.e,m);return r.error?[]:[{type:'polyline',points:r.removed}]}
async function runTrimExtend(f,ext){notify('Current settings: Projection=UCS, Edge=None, Mode=Quick');
 for(;;){const r=await wait(f,{prompt:ext?'Select object to extend or shift-select to trim or [Undo]:':'Select object to trim or shift-select to extend or [eRase/Undo]:',pick:true,preview:m=>previewTrimExtend(m,ext)});
  if(r.cancel||r.enter)return;if(r.option==='Undo'){undoOne(f);continue}
  if(r.option==='eRase'){for(;;){const r2=await wait(f,{prompt:'Select objects to erase or <exit>:',pick:true});if(r2.cancel)return;if(!r2.point)break;const h=pickAt(r2.point);if(!h)continue;const g=h.e.group;mutate(()=>doc.entities=doc.entities.filter(e=>e!==h.e&&!(g&&e.group===g)));f.count++}continue}
  if(!r.point)continue;const res=ext!==!!r.shift?applyExtend(r.point):applyTrim(r.point);if(res===true)f.count++;else if(res)notify(res)}}
// Dry-run of a fillet/chamfer between picked objects: {set:[[entity,points|null]], add:entity|null} or {error}.
function cornerResult(e1,p1,e2,p2,spec){const kind=spec.kind;
 if(e1===e2){if(e1.type!=='polyline')return{error:`Cannot ${kind} an object to itself.`};const ps=e1.points,N=ps.length,segN=e1.closed&&N>2?N:N-1,s1=nearestSeg(e1,p1).k,s2=nearestSeg(e1,p2).k;let k=-1,firstPrev=true;
  if(s2===s1+1)k=s1+1;else if(s1===s2+1){k=s2+1;firstPrev=false}else if(e1.closed&&s1===segN-1&&s2===0)k=0;else if(e1.closed&&s2===segN-1&&s1===0){k=0;firstPrev=false}if(k<0)return{error:'Select two adjacent segments of the polyline.'};
  const r=vertexCorner(ps,k%N,spec,firstPrev);if(!r.points)return{error:r.skip==='short'?kind==='fillet'?'Radius is too large.':'Distance is too large.':r.skip==='zero'?'Nothing to do with zero radius/distance on a polyline vertex.':'Segments are parallel.'};const np=ps.map(p=>({...p}));np.splice(k%N,1,...r.points);return{set:[[e1,np]],add:null}}
 if(e1.type!=='line'||e2.type!=='line')return{error:`Select lines, or two adjacent segments of one polyline, to ${kind}.`};
 const r=kind==='fillet'?filletLines(e1.points,p1,e2.points,p2,spec.r):chamferLines(e1.points,p1,e2.points,p2,spec.d1,spec.d2,spec.angle);if(r.error)return r;
 const lay=e1.layer===e2.layer?e1.layer:layer().name,add=r.arc?{type:'polyline',closed:false,layer:lay,points:r.arc}:r.chamfer?{type:'line',layer:lay,points:r.chamfer}:null;return{set:[[e1,r.line1],[e2,r.line2]],add}}
function applyCorner(res){mutate(()=>{for(const [e,ps]of res.set)if(ps)e.points=ps;const gone=new Set(res.set.filter(([,ps])=>!ps).map(([e])=>e));if(gone.size)doc.entities=doc.entities.filter(e=>!gone.has(e));if(res.add)doc.entities.push(res.add)})}
function previewCorner(e1,p1,m,spec){const h=pickAt(m);if(!h||!doc.entities.includes(e1))return[];const r=cornerResult(e1,p1,h.e,m,spec);if(r.error)return[];return[...r.set.filter(([,ps])=>ps).map(([e,ps])=>({type:e.type,closed:e.closed,points:ps})),...(r.add?[r.add]:[])]}
async function runCorner(f,kind){const fillet=kind==='fillet',spec=zero=>fillet?{kind,r:zero?0:S.filletRad}:zero?{kind,d1:0,d2:0,angle:null}:S.chamferMethod==='angle'?{kind,d1:S.chamferLen,angle:S.chamferAng}:{kind,d1:S.chamferA,d2:S.chamferB,angle:null};
 const settings=()=>notify(fillet?`Current settings: Mode = TRIM, Radius = ${fmt(S.filletRad)}`:S.chamferMethod==='angle'?`(TRIM mode) Current chamfer Length = ${fmt(S.chamferLen)}, Angle = ${deg(S.chamferAng)}`:`(TRIM mode) Current chamfer Dist1 = ${fmt(S.chamferA)}, Dist2 = ${fmt(S.chamferB)}`);
 // Options shared by both selection prompts; returns false when the command was cancelled.
 const option=async o=>{if(o==='Radius'){const v=await askNum(f,`Specify fillet radius <${fmt(S.filletRad)}>:`,fmt(S.filletRad),{kind:'distance',min:0});if(v==null)return false;S.filletRad=v}
  else if(o==='Distance'){const a=await askNum(f,`Specify first chamfer distance <${fmt(S.chamferA)}>:`,fmt(S.chamferA),{kind:'distance',min:0});if(a==null)return false;const b=await askNum(f,`Specify second chamfer distance <${fmt(a)}>:`,fmt(a),{kind:'distance',min:0});if(b==null)return false;S.chamferA=a;S.chamferB=b;S.chamferMethod='distance'}
  else if(o==='Angle'){const a=await askNum(f,`Specify chamfer length on the first line <${fmt(S.chamferLen)}>:`,fmt(S.chamferLen),{kind:'distance',min:0});if(a==null)return false;const g=await askNum(f,`Specify chamfer angle from the first line <${deg(S.chamferAng)}>:`,deg(S.chamferAng),{kind:'angle',min:0,max:180});if(g==null)return false;S.chamferLen=a;S.chamferAng=g;S.chamferMethod='angle'}return true};
 const noun=fillet?'object':'line',first=`Select first ${noun} or [Undo/Polyline/${fillet?'Radius':'Distance/Angle'}/Multiple]:`,second=`Select second ${noun} or shift-select to apply corner or [${fillet?'Radius':'Distance/Angle'}]:`;let multiple=false;settings();
 for(;;){hl(f,[]);const r=await wait(f,{prompt:first,pick:true});if(r.cancel||r.enter)return;
  if(r.option==='Multiple'){multiple=true;continue}if(r.option==='Undo'){undoOne(f);continue}if(r.option&&r.option!=='Polyline'){if(!await option(r.option))return;continue}
  if(r.option==='Polyline'){const r2=await wait(f,{prompt:'Select 2D polyline:',pick:true});if(r2.cancel)return;const h=r2.point&&pickAt(r2.point);if(!h)continue;if(h.e.type!=='polyline'){notify('Object selected is not a polyline.');continue}
   const res=cornerPolyline(h.e,spec(false));if(res.count){mutate(()=>h.e.points=res.points);f.count++}notify(`${res.count} line${res.count===1?' was':'s were'} ${fillet?'filleted':'chamfered'}`+(res.short?`; ${res.short} ${res.short===1?'was':'were'} too short`:''));if(!multiple)return;continue}
  if(!r.point)continue;const h1=pickAt(r.point);if(!h1)continue;if(!['line','polyline'].includes(h1.e.type)){notify(`Cannot ${kind} that object.`);continue}hl(f,[h1.i]);const e1=h1.e,p1=r.point;let done=false;
  for(;;){const r2=await wait(f,{prompt:second,pick:true,preview:m=>previewCorner(e1,p1,m,spec(false))});if(r2.cancel||r2.enter)return;if(r2.option){if(!await option(r2.option))return;continue}if(!r2.point)continue;const h2=pickAt(r2.point);if(!h2)continue;
   if(!doc.entities.includes(e1))break;const res=cornerResult(e1,p1,h2.e,r2.point,spec(!!r2.shift));if(res.error){notify(res.error);break}hl(f,[]);applyCorner(res);f.count++;done=true;break}
  if(done&&!multiple)return}}
async function runMirror(f){const ids=await selectObjects(f,'MIRROR');if(!ids?.length)return;const ents=ids.map(i=>doc.entities[i]);
 const r1=await wait(f,{prompt:'Specify first point of mirror line:'});if(!r1.point)return;const a=r1.point;let b;
 for(;;){const r2=await wait(f,{prompt:'Specify second point of mirror line:',base:a,preview:m=>G.dist(a,m)<1e-9?[]:ents.slice(0,2000).map(e=>mirrorEntity(e,a,m)).filter(Boolean)});if(!r2.point)return;if(G.dist(a,r2.point)>1e-9){b=r2.point;break}notify('Invalid point: the mirror line needs two different points.')}
 f.ghost=ents.slice(0,2000).map(e=>mirrorEntity(e,a,b));const ans=await askOption(f,'Erase source objects? [Yes/No] <No>:','No');f.ghost=null;if(ans==null)return;const src=live(ents);
 mutate(()=>{if(ans==='Yes'){for(const e of src){const m=mirrorEntity(e,a,b);if(e.center)e.center=m.center;else e.points=m.points}}else{const groups={};for(const e of src){const m=mirrorEntity(e,a,b);delete m.uuid;if(m.group)m.group=groups[m.group]??=gid();doc.entities.push(m)}}});end(f);clearSelection();render()}
const polyEnt=ps=>({type:'polyline',closed:true,points:ps});
function addEntities(ents,extra={}){const lay=layer().name;mutate(()=>doc.entities.push(...ents.map(e=>({...e,layer:lay,...extra}))))}
async function runPolygon(f){const n=await askNum(f,`Enter number of sides <${S.sides}>:`,S.sides,{int:true,min:3,max:1024});if(n==null)return;S.sides=n;
 const r=await wait(f,{prompt:'Specify center of polygon or [Edge]:'});
 if(r.option==='Edge'){const a=await wait(f,{prompt:'Specify first endpoint of edge:'});if(!a.point)return;const b=await wait(f,{prompt:'Specify second endpoint of edge:',base:a.point,preview:m=>{const ps=polygonEdge(n,a.point,m);return ps?[polyEnt(ps)]:[]}});if(!b.point)return;const ps=polygonEdge(n,a.point,b.point);if(!ps){notify('Edge length must be nonzero.');return}addEntities([polyEnt(ps)]);return}
 if(!r.point)return;const c=r.point,mode=await askOption(f,`Enter an option [Inscribed in circle/Circumscribed about circle] <${S.polyMode}>:`,S.polyMode);if(mode==null)return;S.polyMode=mode[0];
 for(;;){const r3=await wait(f,{prompt:'Specify radius of circle:',base:c,number:true,preview:m=>{const ps=polygonPoints(n,c,G.dist(c,m),G.angle(c,m),S.polyMode);return ps?[polyEnt(ps)]:[]}});let ps;
  if(r3.point)ps=polygonPoints(n,c,G.dist(c,r3.point),G.angle(c,r3.point),S.polyMode);
  else if(r3.number!=null)ps=polygonPoints(n,c,r3.number,-Math.PI/2-(S.polyMode==='C'?TAU/n:Math.PI/n),S.polyMode); // typed radius: bottom edge horizontal
  else return;if(!ps){notify('Value must be positive and nonzero.');continue}addEntities([polyEnt(ps)]);return}}
async function runEllipse(f){const r=await wait(f,{prompt:'Specify axis endpoint of ellipse or [Center]:'});let c,e1;const line=a=>m=>[{type:'line',points:[a,m]}];
 if(r.option==='Center'){const a=await wait(f,{prompt:'Specify center of ellipse:'});if(!a.point)return;c=a.point;const b=await wait(f,{prompt:'Specify endpoint of axis:',base:c,preview:line(c)});if(!b.point)return;e1=b.point}
 else if(r.point){const p1=r.point,b=await wait(f,{prompt:'Specify other endpoint of axis:',base:p1,preview:line(p1)});if(!b.point)return;c=G.mid(p1,b.point);e1=b.point}else return;
 if(G.dist(c,e1)<1e-9){notify('Invalid axis: the axis length must be nonzero.');return}
 for(;;){const r3=await wait(f,{prompt:'Specify distance to other axis or [Rotation]:',base:c,number:true,preview:m=>{const ps=ellipsePoints(c,e1,G.dist(c,m));return ps?[polyEnt(ps)]:[]}});let b;
  if(r3.point)b=G.dist(c,r3.point);else if(r3.number!=null)b=r3.number;else if(r3.option==='Rotation'){const ang=await askNum(f,'Specify rotation around major axis <0>:',0,{kind:'angle',min:0,max:89.4});if(ang==null)return;b=G.dist(c,e1)*Math.cos(ang*Math.PI/180)}else return;
  const ps=ellipsePoints(c,e1,b);if(!ps){notify('Value must be positive and nonzero.');continue}addEntities([polyEnt(ps)]);return}}
async function runDimLinear(f){let a,b,circ=null;const r=await wait(f,{prompt:'Specify first extension line origin or <select object>:'});
 if(r.enter){for(;;){const s=await wait(f,{prompt:'Select object to dimension:',pick:true});if(!s.point)return;const h=pickAt(s.point);if(!h)continue;if(h.e.type==='line'){[a,b]=h.e.points;break}if(h.e.type==='polyline'){[a,b]=G.segments(h.e)[nearestSeg(h.e,s.point).k];break}if(h.e.type==='circle'){circ=h.e;break}notify('Object selected is not a line, polyline segment or circle.')}}
 else if(r.point){a=r.point;const r2=await wait(f,{prompt:'Specify second extension line origin:',base:a,preview:m=>[{type:'line',points:[a,m]}]});if(!r2.point)return;b=r2.point}else return;
 // A circle is dimensioned across its quadrants in the chosen direction (its diameter).
 const make=(pos,o)=>{if(!circ)return dimLinear(a,b,pos,o);const c=circ.center,R=circ.radius,oo=o||(pos.x>=c.x-R&&pos.x<=c.x+R?'h':'v');return oo==='h'?dimLinear(P(c.x-R,c.y),P(c.x+R,c.y),pos,'h'):dimLinear(P(c.x,c.y-R),P(c.x,c.y+R),pos,'v')};let orient=null;
 for(;;){const r3=await wait(f,{prompt:'Specify dimension line location or [Horizontal/Vertical]:',preview:m=>make(m,orient)?.entities||[]});if(r3.option==='Horizontal'){orient='h';continue}if(r3.option==='Vertical'){orient='v';continue}if(!r3.point)return;
  const d=make(r3.point,orient);if(!d){notify('The measured distance is zero; specify another location or orientation.');continue}addEntities(d.entities,{group:gid()});end(f);report([`Dimension text = ${d.text}`]);return}}
async function runDist(f){const a=await wait(f,{prompt:'Specify first point:'});if(!a.point)return;const b=await wait(f,{prompt:'Specify second point:',base:a.point,preview:m=>[{type:'line',points:[a.point,m]}]});if(!b.point)return;end(f);report(distReport(a.point,b.point))}
async function runId(f){const a=await wait(f,{prompt:'Specify point:'});if(!a.point)return;end(f);report(idReport(a.point))}
async function runList(f){const ids=await selectObjects(f,'LIST');if(!ids?.length)return;const lines=ids.sort((x,y)=>x-y).flatMap(i=>listReport(doc.entities[i]));end(f);clearSelection();render();report(lines)}
async function runMatch(f){let src;for(;;){const r=await wait(f,{prompt:'Select source object:',pick:true});if(!r.point)return;const h=pickAt(r.point);if(h){src=h.e;hl(f,[h.i]);break}}
 const active=()=>notify(`Current active settings: ${[S.matchLayer&&'Layer',S.matchText&&'Text'].filter(Boolean).join(' ')||'None'}`);active();
 for(;;){const r=await wait(f,{prompt:'Select destination object(s) or [Settings]:',pick:true});if(r.cancel||r.enter)return;
  if(r.option==='Settings'){const o=await askOption(f,'Enter properties to match [Layer/Text/Both] <Both>:','Both');if(o==null)return;S.matchLayer=o!=='Text';S.matchText=o!=='Layer';active();continue}
  if(!r.point)continue;const h=pickAt(r.point);if(!h||h.e===src)continue;if(!doc.entities.includes(src)){notify('The source object no longer exists.');return}const targets=h.e.group?doc.entities.filter(e=>e.group===h.e.group):[h.e];
  mutate(()=>{for(const e of targets){if(S.matchLayer)e.layer=src.layer;if(S.matchText&&src.type==='text'&&e.type==='text')e.height=src.height}});f.count++}}
async function runArray(f){const ids=await selectObjects(f,'ARRAYRECT');if(!ids?.length)return;const ents=ids.map(i=>doc.entities[i]),bs=ents.map(G.bbox),w=Math.max(...bs.map(b=>b.maxX))-Math.min(...bs.map(b=>b.minX)),h=Math.max(...bs.map(b=>b.maxY))-Math.min(...bs.map(b=>b.minY));
 notify('Type = Rectangular  Associative = No');const rows=await askNum(f,`Enter the number of rows (---) <${S.rows}>:`,S.rows,{int:true,min:1,max:1000});if(rows==null)return;const cols=await askNum(f,`Enter the number of columns (|||) <${S.cols}>:`,S.cols,{int:true,min:1,max:1000});if(cols==null)return;
 if(rows*cols<2){notify('An array needs more than one item.');return}if((rows*cols-1)*ents.length>200000){notify('The array would create too many objects.');return}S.rows=rows;S.cols=cols;
 const defR=fmt(((h>1e-9?h:w)||10/1.5)*1.5),defC=fmt(((w>1e-9?w:h)||10/1.5)*1.5);let dy=0,dx=0;
 if(rows>1){dy=await askNum(f,`Specify the distance between rows (---) <${defR}>:`,defR,{kind:'distance'});if(dy==null)return}if(cols>1){dx=await askNum(f,`Specify the distance between columns (|||) <${defC}>:`,defC,{kind:'distance'});if(dx==null)return}
 const copies=arrayCopies(live(ents),rows,cols,dy,dx);mutate(()=>doc.entities.push(...copies));end(f);clearSelection();render();report([`${rows*cols} items in ${rows} row${rows>1?'s':''} x ${cols} column${cols>1?'s':''}: ${copies.length} objects created.`])}
async function runPurge(){const f={dead:false},a=purgeAnalysis();if(!a.blocks.length&&!a.layers.length){report(['No unreferenced blocks or layers found.']);return}
 const o=await askOption(f,'Enter type of unused objects to purge [Blocks/LAyers/All] <All>:','All');if(o==null)return;const blocks=o!=='LAyers'?a.blocks:[];let layers=o!=='Blocks'?a.layers:[];
 if(o==='All'){const after=structuredClone({...doc,solids:undefined});for(const n of blocks)delete after.blocks[n];layers=purgeAnalysis(after).layers} // layers used only by purged blocks go too
 if(!blocks.length&&!layers.length){report([`No unreferenced ${o==='Blocks'?'blocks':'layers'} found.`]);return}
 mutate(()=>{for(const n of blocks)delete doc.blocks[n];doc.layers=doc.layers.filter(l=>!layers.includes(l.name))});syncLayers();
 report([...blocks.map(n=>`Deleting block "${n}".`),...layers.map(n=>`Deleting layer "${n}".`),`${blocks.length} block${blocks.length===1?'':'s'} deleted.`,`${layers.length} layer${layers.length===1?'':'s'} deleted.`])}

// ---- Engine / workspace integration ------------------------------------------------------------------------
function defineTool(name,cmd,run){TOOLS[name]={cmd,run};names[name]??=cmd[0]+cmd.slice(1).toLowerCase();const mine=()=>flow&&flow.name===name;
 CF.prompts[name]=()=>mine()&&flow.waiter?`${cmd} ${flow.waiter.phase.prompt}`:cmd;
 CF.previews[name]=m=>{if(!mine())return[];const w=flow.waiter;try{return(w?w.phase.preview?.(m):flow.ghost)||[]}catch(err){return[]}};
 CF.pickTools[name]=()=>!!(mine()&&(flow.picking||flow.waiter?.phase.pick))}
// Typed text for the waiting command: an option keyword, or a number where the prompt accepts one.
function parseInput(text){const w=flow?.waiter;if(!w||CF.input)return null;const t=String(text??'').trim();if(!t)return null;const o=matchOption(t,w.phase.options);if(o)return{option:o};if(w.phase.number&&/^[+-]?(\d+\.?\d*|\.\d+)(e[+-]?\d+)?$/i.test(t))return{number:Number(t)};return null}
function input(text){const r=parseInput(text);return r?respond(r):false}
{const prevSetTool=setTool;setTool=function(t,...rest){if(!starting){if(flow)abort();if(TOOLS[t]){start(t);return}}return prevSetTool(t,...rest)}}
{const prevAccept=accept;accept=async function(p,...rest){const f=flow;if(!f||!TOOLS[tool]||f.name!==tool)return prevAccept(p,...rest);
 if(f.picking){const i=hit(p);if(i>=0)CF.select([i],typeof shiftSelection!=='undefined'&&shiftSelection?'remove':'add');return}
 if(!f.waiter)return;return respond({point:{x:p.x,y:p.y},shift:typeof shiftSelection!=='undefined'&&!!shiftSelection})}}
{const prevFinish=finish;finish=function(...a){const f=flow;if(f&&TOOLS[tool]){if(f.picking)f.picking.obj.done();else if(f.waiter)respond({enter:true});return}return prevFinish(...a)}}
{const prevEnter=CF.enter;CF.enter=function(...a){if(flow&&TOOLS[tool]&&flow.waiter&&!CF.input&&!CF.picking){respond({enter:true});return}return prevEnter.apply(CF,a)}}
// Typed option keywords reach the command even when the command line hands them to CF.run.
{const prevRun=CF.run;CF.run=function(text,options={}){if(flow?.waiter&&!['ribbon','menu','appmenu','qat','toolbar','search'].includes(options?.source)){const r=parseInput(text);if(r){respond(r);return true}}return prevRun.call(CF,text,options)}}
try{const el=$('command'),prevKey=el.onkeydown;el.onkeydown=function(e){const v=String(e?.target?.value??'').trim();if((e.key==='Enter'||e.key===' '&&v)&&flow?.waiter&&!CF.input){const r=parseInput(v);if(r){e.preventDefault?.();e.stopImmediatePropagation?.();const line=`${CF.prompt()} ${v}`;e.target.value='';try{CF.commandLine?.print?.(line)}catch(err){}respond(r);return}}return prevKey?.call(this,e)}}catch(err){}
// Without the interaction module the rubber-band previews are drawn here (dashed, preview colour).
{const prevPreview=drawPreview;drawPreview=function(...a){prevPreview(...a);if(CF.has('interact')||!flow||mode3D)return;let ents=[];try{ents=CF.previews[tool]?.(mouse)||[]}catch(err){}for(const e of ents)drawEntity(e,CF.colors.preview,true)}}

defineTool('offset','OFFSET',runOffset);defineTool('trim','TRIM',f=>runTrimExtend(f,false));defineTool('extend','EXTEND',f=>runTrimExtend(f,true));
defineTool('fillet','FILLET',f=>runCorner(f,'fillet'));defineTool('chamfer','CHAMFER',f=>runCorner(f,'chamfer'));defineTool('mirror','MIRROR',runMirror);
defineTool('polygon','POLYGON',runPolygon);defineTool('ellipse','ELLIPSE',runEllipse);defineTool('dimlinear','DIMLINEAR',runDimLinear);defineTool('dist','DIST',runDist);
defineTool('id','ID',runId);defineTool('list','LIST',runList);defineTool('matchprop','MATCHPROP',runMatch);defineTool('arrayrect','ARRAYRECT',runArray);
for(const [name,aliases,label,desc,category,tool_]of [
 ['OFFSET',['O'],'Offset','Creates concentric circles, parallel lines and parallel polylines','Modify','offset'],
 ['TRIM',['TR'],'Trim','Trims objects to meet the edges of other objects (quick mode)','Modify','trim'],
 ['EXTEND',['EX'],'Extend','Extends objects to meet the edges of other objects','Modify','extend'],
 ['FILLET',['F'],'Fillet','Rounds and fillets the corner between two lines or polyline segments','Modify','fillet'],
 ['CHAMFER',['CHA'],'Chamfer','Bevels the corner between two lines or polyline segments','Modify','chamfer'],
 ['MIRROR',['MI'],'Mirror','Creates a mirrored copy of selected objects','Modify','mirror'],
 ['POLYGON',['POL'],'Polygon','Creates an equilateral closed polyline','Draw','polygon'],
 ['ELLIPSE',['EL'],'Ellipse','Creates an ellipse (closed polyline)','Draw','ellipse'],
 ['DIMLINEAR',['DLI','DIM'],'Linear','Creates a horizontal or vertical linear dimension','Annotate','dimlinear'],
 ['DIST',['DI'],'Distance','Measures the distance and angle between two points','Utilities','dist'],
 ['ID',[],'ID Point','Displays the coordinates of a location','Utilities','id'],
 ['LIST',['LI','LS'],'List','Displays property data for selected objects','Utilities','list'],
 ['MATCHPROP',['MA','PAINTER'],'Match Properties','Applies the properties of a selected object to other objects','Properties','matchprop'],
 ['ARRAYRECT',['AR','ARRAY'],'Rectangular Array','Distributes object copies into rows and columns','Modify','arrayrect']])
 CF.register({name,aliases,label,desc,category,icon:name.toLowerCase(),tool:tool_,run:()=>start(tool_)});
CF.register({name:'PURGE',aliases:['PU'],label:'Purge',desc:'Removes unused block definitions and empty layers',category:'Manage',icon:'purge',run:()=>{if(flow)setTool('select');return runPurge()}});
CF.module('drafting');
return{settings:S,idle,input,start,state:()=>flow&&{tool:flow.name,prompt:flow.waiter?.phase.prompt??null,picking:!!flow.picking},
 offsetPath,offsetEntity,trimEntity,extendEntity,filletLines,chamferLines,vertexCorner,cornerPolyline,polygonPoints,polygonEdge,ellipsePoints,dimLinear,dimOrient,mirrorEntity,distReport,idReport,listReport,purgeAnalysis,arrayCopies,matchOption,optionsOf,area,subPath,pathCuts}})();
