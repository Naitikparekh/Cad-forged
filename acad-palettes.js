'use strict';
// Workspace palettes (A2): status bar, Properties palette, ribbon layer dropdown, Layer Properties Manager, Drafting Settings and engine dialog theming.
CF.module('palettes');
const palStore={get(k,d){try{const v=localStorage.getItem('cf.pal.'+k);return v==null?d:JSON.parse(v)}catch{return d}},set(k,v){try{localStorage.setItem('cf.pal.'+k,JSON.stringify(v))}catch{}}};
function palEl(tag,cls,text){const e=document.createElement(tag);if(cls)e.className=cls;if(text!=null)e.textContent=String(text);return e}
function palBtn(cls,text,title,onclick){const b=palEl('button',cls,text);b.type='button';if(title)b.title=title;if(onclick)b.onclick=onclick;return b}
function palOpt(v,l){const o=document.createElement('option');o.value=v;o.textContent=l??v;return o}
const palIcons=()=>CF.has('icons');
function palIcon(name,size=16){try{return CF.icon(name,size)}catch{return ''}}
const palNum=v=>Number.isFinite(v)?(Math.abs(v)<5e-9?0:v).toFixed(4):'';
const palAng=v=>{let d=((v%360)+360)%360;if(d>359.99995)d=0;return String(Number(d.toFixed(4)))};
const palDeg=r=>r*180/Math.PI;
function palParseNum(raw){const s=String(raw??'').trim().replace(/°$|d$/i,'');return /^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i.test(s)?Number(s):NaN}
function palFail(msg){notify(msg);return msg}
function palRun(cmd,fallback){if(CF.resolve(cmd))return CF.run(cmd,{source:'palette'});return fallback?.()}
// Keys typed into palette/dialog fields stay there; F-keys and Ctrl+digit shortcuts still reach the workspace.
// Ctrl+Z/Y in a field that is not being edited is drawing undo/redo, as on the canvas.
function palStop(e){const k=e.key||'',t=e.target;if(/^F\d+$/.test(k)||(e.ctrlKey||e.metaKey)&&/^[0-9]$/.test(k))return;e.stopPropagation?.();
 if((e.ctrlKey||e.metaKey)&&/^[zy]$/i.test(k)&&!(t?.tagName==='INPUT'&&t.value!==t.dataset?.committed)){e.preventDefault?.();const key=t?.closest?.('[data-key]')?.dataset.key;undo(k.toLowerCase()==='y'||e.shiftKey);palUpdate(true);if(key)palFocusKey(key)}}

// ---- Drafting-aid settings read by the interaction module -----------------------------
const palOsnapAll=['endpoint','midpoint','center','quadrant','intersection','perpendicular','nearest'];
const palOsnapNames={endpoint:'Endpoint',midpoint:'Midpoint',center:'Center',quadrant:'Quadrant',intersection:'Intersection',perpendicular:'Perpendicular',nearest:'Nearest'};
const palOsnapMarks={endpoint:'□',midpoint:'△',center:'○',quadrant:'◇',intersection:'×',perpendicular:'⊾',nearest:'⧖'};
const palPolarSteps=[90,45,30,22.5,18,15,10,5];
if(!(CF.osnapModes instanceof Set))CF.osnapModes=new Set(['endpoint','midpoint','center','intersection','quadrant']);
{const saved=palStore.get('osnapModes',null);if(Array.isArray(saved)){CF.osnapModes.clear();for(const m of saved)if(palOsnapAll.includes(m))CF.osnapModes.add(m)}
 const inc=palStore.get('polarIncrement',null);CF.polarIncrement=Number(inc)>0&&Number(inc)<=180?Number(inc):(CF.polarIncrement||45)}
function palSetOsnapMode(mode,on){if(!palOsnapAll.includes(mode))return false;on=on===undefined?!CF.osnapModes.has(mode):!!on;if(on)CF.osnapModes.add(mode);else CF.osnapModes.delete(mode);palStore.set('osnapModes',[...CF.osnapModes]);CF.emit('settings',{name:'osnapModes',value:[...CF.osnapModes]});palStateRev++;render();return true}
function palSetPolarIncrement(v){v=Number(v);if(!(v>0&&v<=180))return false;CF.polarIncrement=v;palStore.set('polarIncrement',v);CF.emit('settings',{name:'polarIncrement',value:v});palStateRev++;render();return true}

// ---- Layer operations (all document changes go through mutate, so they are undoable) ----
const palRenameLog=[];
const palLayer=name=>doc.layers.find(l=>l.name===name);
function palLayerObjects(name){let n=0;for(const e of doc.entities)if(e.layer===name)n++;return n}
const palLayerInBlocks=name=>Object.values(doc.blocks||{}).some(b=>(b.items||[]).some(e=>e.layer===name));
function palNextLayerName(){for(let i=1;;i++){const n='Layer'+i;if(!doc.layers.some(l=>l.name.toLowerCase()===n.toLowerCase()))return n}}
function palCheckName(name,except){name=String(name??'').trim();if(!name)return 'Layer name cannot be empty.';if(/[<>\/\\":;?*|=,`]/.test(name))return 'Layer names cannot contain < > / \\ " : ; ? * | = , or `.';if(name.length>255)return 'Layer name is too long.';if(doc.layers.some(l=>l!==except&&l.name.toLowerCase()===name.toLowerCase()))return `A layer named "${name}" already exists.`;return true}
function palSelectCurrent(name){syncLayers();$('layer').value=name;syncLayers()}
function palSetCurrent(name){if(!palLayer(name))return palFail(`Layer "${name}" was not found.`);if($('layer').value!==name){palSelectCurrent(name);CF.emit('layer',name)}render();return true}
function palAddLayer(name=palNextLayerName(),from){name=String(name).trim();const ok=palCheckName(name);if(ok!==true)return palFail(ok);const src=palLayer(from)||palLayer($('layer').value);mutate(()=>doc.layers.push({name,color:src?.color||'#ffffff',visible:true}));palSelectCurrent($('layer').value);return true}
function palRenameLayer(from,to){const l=palLayer(from);if(!l)return palFail(`Layer "${from}" was not found.`);to=String(to??'').trim();if(to===from)return true;if(from==='0')return palFail('Layer 0 cannot be renamed.');const ok=palCheckName(to,l);if(ok!==true)return palFail(ok);const wasCurrent=$('layer').value===from;
 mutate(()=>{palLayer(from).name=to;for(const e of doc.entities)if(e.layer===from)e.layer=to;for(const b of Object.values(doc.blocks||{}))for(const e of b.items||[])if(e.layer===from)e.layer=to});
 palRenameLog.push({from,to});palSelectCurrent(wasCurrent?to:$('layer').value);render();return true}
function palDeleteLayer(name){if(!palLayer(name))return palFail(`Layer "${name}" was not found.`);if(name==='0')return palFail('Layer 0 cannot be deleted.');if(name===$('layer').value)return palFail('The current layer cannot be deleted.');const n=palLayerObjects(name);if(n)return palFail(`Layer "${name}" contains ${n} object${n>1?'s':''} and cannot be deleted.`);if(palLayerInBlocks(name))return palFail(`Layer "${name}" is used by a block definition and cannot be deleted.`);mutate(()=>doc.layers=doc.layers.filter(l=>l.name!==name));palSelectCurrent($('layer').value);return true}
function palSetLayerColor(name,c){c=String(c||'').toLowerCase();if(!/^#[0-9a-f]{6}$/.test(c))return palFail('Invalid colour.');const l=palLayer(name);if(!l)return palFail(`Layer "${name}" was not found.`);if(l.color.toLowerCase()!==c){mutate(()=>palLayer(name).color=c);syncLayers()}return true}
function palSetLayerVisible(name,v){const l=palLayer(name);if(!l)return palFail(`Layer "${name}" was not found.`);if(l.visible!==!!v){mutate(()=>palLayer(name).visible=!!v);syncLayers();if(!v&&name===$('layer').value)notify('The current layer is turned off.')}return true}
// Undo/redo of a rename that touched the current layer keeps the renamed layer current.
{const prev=undo;undo=function(redo=false){const cur=$('layer').value,r=prev(redo);if(!doc.layers.some(l=>l.name===cur)){const hit=[...palRenameLog].reverse().find(x=>redo?x.from===cur:x.to===cur),back=hit&&(redo?hit.to:hit.from);if(back&&doc.layers.some(l=>l.name===back)){palSelectCurrent(back);render()}}return r}}
// Colour names follow the AutoCAD index colours 1-9; anything else is shown as R,G,B.
const palACI=[['#ff0000','red'],['#ffff00','yellow'],['#00ff00','green'],['#00ffff','cyan'],['#0000ff','blue'],['#ff00ff','magenta'],['#ffffff','white'],['#808080','8'],['#c0c0c0','9']];
function palColorName(c){c=String(c||'').toLowerCase();const n=palACI.find(x=>x[0]===c);return n?n[1]:/^#[0-9a-f]{6}$/.test(c)?[1,3,5].map(i=>parseInt(c.slice(i,i+2),16)).join(','):c}
function palSwatch(color,button){const s=palEl(button?'button':'span','cf-swatch');if(button)s.type='button';s.style.background=color||'transparent';if(!color)s.classList.add('none');return s}
function palBulb(on){const s=palEl('span','cf-bulb '+(on?'on':'off'));if(palIcons()){s.classList.add('icon');s.innerHTML=palIcon(on?'layer-on':'layer-off',16)}return s}

// ---- Popups (status menus, colour picker, layer list) ----------------------------------
let palPopups=[];
function palCloseMenus(){for(const p of palPopups){try{p.el.remove()}catch{}p.onclose?.()}palPopups=[]}
function palInsidePopup(t){return palPopups.some(p=>p.el.contains?.(t)||p.anchor?.contains?.(t))}
try{document.addEventListener('pointerdown',e=>{if(palPopups.length&&!palInsidePopup(e.target))palCloseMenus()},true);document.addEventListener('keydown',e=>{if(palPopups.length&&e.key==='Escape'){e.stopPropagation();e.preventDefault?.();palCloseMenus()}},true);window.addEventListener?.('blur',()=>palCloseMenus());window.addEventListener?.('resize',()=>palCloseMenus())}catch{}
// Places el next to anchor: 'up' opens above (status bar), 'down' below (ribbon).
// Popups opened from a modal dialog live inside it, otherwise they would sit below the dialog's top layer.
const palPopupHost=anchor=>anchor?.closest?.('dialog[open]')||document.body;
function palPopup(el,anchor,{dir='up',stack=false,onclose,width}={}){if(!stack)palCloseMenus();el.classList.add('cf-pal-popup');palPopupHost(anchor).append(el);const r=anchor?.getBoundingClientRect?.()||{left:0,top:0,bottom:0,width:0},w=el.offsetWidth||width||200,vw=innerWidth||1280,vh=innerHeight||800;el.style.left=Math.max(2,Math.min(r.left,vw-w-4))+'px';if(width)el.style.minWidth=width+'px';if(dir==='up')el.style.bottom=Math.max(2,vh-r.top+2)+'px';else{el.style.top=r.bottom+2+'px';el.style.maxHeight=Math.max(120,vh-r.bottom-12)+'px'}palPopups.push({el,anchor,onclose});return el}
// items: {label, checked, radio, hint, mark, act, sep, disabled}
function palMenu(items,anchor,opts){const m=palEl('div','cf-pmenu');m.setAttribute('role','menu');for(const it of items){if(it.sep){m.append(palEl('div','cf-pmenu-sep'));continue}if(it.header){m.append(palEl('div','cf-pmenu-head',it.header));continue}const row=palEl('div','cf-pmenu-item'+(it.disabled?' disabled':''));row.setAttribute('role','menuitem');const chk=palEl('span','cf-pmenu-chk'+(it.checked?(it.radio?' radio':' on'):''));const mark=palEl('span','cf-pmenu-mark',it.mark||'');row.append(chk,mark,palEl('span','cf-pmenu-label',it.label),palEl('span','cf-pmenu-hint',it.hint||''));row.onclick=e=>{e.stopPropagation?.();if(it.disabled)return;palCloseMenus();it.act?.()};m.append(row)}return palPopup(m,anchor,opts)}
let palNativeColor=null;
function palPickNative(anchor,current,cb){if(!palNativeColor){palNativeColor=palEl('input','cf-native-color');palNativeColor.type='color'}palPopupHost(anchor).append(palNativeColor);const r=anchor?.getBoundingClientRect?.()||{left:0,top:0};palNativeColor.style.left=r.left+'px';palNativeColor.style.top=r.top+'px';palNativeColor.value=/^#[0-9a-f]{6}$/i.test(current)?current:'#ffffff';palNativeColor.onchange=()=>cb(palNativeColor.value);try{palNativeColor.showPicker()}catch{palNativeColor.click()}}
// Compact "Select Color" popup: index colours 1-9 plus the system colour picker.
function palColorPopup(anchor,current,cb,opts={}){const box=palEl('div','cf-colorpop');box.append(palEl('div','cf-pmenu-head','Index color'));const grid=palEl('div','cf-colorpop-grid');for(const [hex,name]of palACI){const s=palSwatch(hex,true);s.title=/^\d$/.test(name)?'Color '+name:name[0].toUpperCase()+name.slice(1);if(hex===String(current).toLowerCase())s.classList.add('current');s.onclick=e=>{e.stopPropagation?.();palCloseMenus();cb(hex)};grid.append(s)}box.append(grid);const cur=palEl('div','cf-colorpop-cur');cur.append(palSwatch(current),palEl('span','',palColorName(current)));box.append(cur);const more=palBtn('cf-colorpop-more','Select Color…','Choose any true color',e=>{e?.stopPropagation?.();palCloseMenus();palPickNative(anchor,current,cb)});box.append(more);return palPopup(box,anchor,{dir:opts.dir||'down',stack:opts.stack})}

// ---- Properties palette model (plain data, also used by tests) -------------------------
let palFilter='all',palVertex=0,palVertexKey=null,palMarker=null;
const palTypeNames={line:'Line',polyline:'Polyline',circle:'Circle',text:'Text',block:'Block Reference',hatch:'Hatch',group:'Group'};
// Selected entities grouped into objects: block instances, hatches and groups count once, like AutoCAD.
function palObjects(ids){const out=[],groups=new Map();for(const i of ids){const e=doc.entities[i];if(!e)continue;if(e.group){let g=groups.get(e.group);if(!g){g={type:e.blockName?'block':e.hatch?'hatch':'group',ids:[],name:e.blockName||''};groups.set(e.group,g);out.push(g)}g.ids.push(i)}else out.push({type:e.type,ids:[i]})}return out}
function palBBox(ids){let b=null;for(const i of ids){const e=doc.entities[i];if(!e)continue;const q=CF.geom.bbox(e);b=b?{minX:Math.min(b.minX,q.minX),minY:Math.min(b.minY,q.minY),maxX:Math.max(b.maxX,q.maxX),maxY:Math.max(b.maxY,q.maxY)}:q}return b}
const palMove=(o,dx,dy)=>{for(const i of o.ids)transformEntity(doc.entities[i],p=>({x:p.x+dx,y:p.y+dy}))};
const palPolyLen=e=>CF.geom.segments(e).reduce((s,[a,b])=>s+CF.geom.dist(a,b),0);
function palPolyArea(e){let s=0;const p=e.points;for(let i=0;i<p.length;i++){const a=p[i],b=p[(i+1)%p.length];s+=a.x*b.y-b.x*a.y}return Math.abs(s)/2}
const palVtx=e=>Math.max(0,Math.min(palVertex,e.points.length-1));
const palPositive=v=>v>0?null:'Value must be greater than zero.';
const palF=(k,label,get,set,o={})=>({k,label,get,set,kind:'num',cat:'Geometry',...o});
// Hatch spacing is recovered from the distance between parallel 45-degree hatch rows.
function palHatchSpacing(o){const vs=[...new Set(o.ids.map(i=>{const p=doc.entities[i].points[0];return Math.round((p.y-p.x)/Math.SQRT2*1e6)/1e6}))].sort((a,b)=>a-b);let d=Infinity;for(let i=1;i<vs.length;i++)d=Math.min(d,vs[i]-vs[i-1]);return Number.isFinite(d)?d:NaN}
const palGroupPos=[palF('px','Position X',(e,o)=>palBBox(o.ids).minX,(e,v,o)=>palMove(o,v-palBBox(o.ids).minX,0)),palF('py','Position Y',(e,o)=>palBBox(o.ids).minY,(e,v,o)=>palMove(o,0,v-palBBox(o.ids).minY))];
// Points are replaced rather than edited in place: chained engine geometry may share point objects.
const palDefs={
 line:[palF('sx','Start X',e=>e.points[0].x,(e,v)=>e.points[0]={...e.points[0],x:v}),palF('sy','Start Y',e=>e.points[0].y,(e,v)=>e.points[0]={...e.points[0],y:v}),palF('sz','Start Z',()=>0),
  palF('ex','End X',e=>e.points[1].x,(e,v)=>e.points[1]={...e.points[1],x:v}),palF('ey','End Y',e=>e.points[1].y,(e,v)=>e.points[1]={...e.points[1],y:v}),palF('ez','End Z',()=>0),
  palF('dx','Delta X',e=>e.points[1].x-e.points[0].x),palF('dy','Delta Y',e=>e.points[1].y-e.points[0].y),palF('dz','Delta Z',()=>0),
  palF('len','Length',e=>CF.geom.dist(e.points[0],e.points[1]),(e,v)=>{const a=CF.geom.dist(e.points[0],e.points[1])>1e-12?CF.geom.angle(e.points[0],e.points[1]):0;e.points[1]=CF.geom.polar(e.points[0],v,a)},{ok:palPositive}),
  palF('ang','Angle',e=>palDeg(CF.geom.angle(e.points[0],e.points[1])),(e,v)=>{e.points[1]=CF.geom.polar(e.points[0],CF.geom.dist(e.points[0],e.points[1]),v*Math.PI/180)},{kind:'deg'})],
 circle:[palF('cx','Center X',e=>e.center.x,(e,v)=>e.center={...e.center,x:v}),palF('cy','Center Y',e=>e.center.y,(e,v)=>e.center={...e.center,y:v}),palF('cz','Center Z',()=>0),
  palF('r','Radius',e=>e.radius,(e,v)=>e.radius=v,{ok:palPositive}),palF('d','Diameter',e=>e.radius*2,(e,v)=>e.radius=v/2,{ok:palPositive}),
  palF('circ','Circumference',e=>2*Math.PI*e.radius),palF('area','Area',e=>Math.PI*e.radius**2)],
 polyline:[palF('vtx','Current Vertex',null,null,{kind:'spin',single:true}),
  palF('vx','Vertex X',e=>e.points[palVtx(e)].x,(e,v)=>{const i=palVtx(e);e.points[i]={...e.points[i],x:v}},{single:true}),palF('vy','Vertex Y',e=>e.points[palVtx(e)].y,(e,v)=>{const i=palVtx(e);e.points[i]={...e.points[i],y:v}},{single:true}),palF('vz','Vertex Z',()=>0,null,{single:true}),
  palF('len','Length',palPolyLen),palF('area','Area',palPolyArea,null,{when:e=>!!e.closed}),
  palF('closed','Closed',e=>!!e.closed,(e,v)=>e.closed=v,{kind:'bool',cat:'Misc',ok:(v,e)=>v&&e.points.length<3?'A closed polyline needs at least three vertices.':null})],
 text:[palF('txt','Contents',e=>e.text,(e,v)=>e.text=v,{kind:'text',cat:'Text',ok:v=>String(v).trim()?null:'Text contents cannot be empty.'}),palF('just','Justify',()=>'Left',null,{kind:'text',cat:'Text'}),
  palF('h','Height',e=>e.height,(e,v)=>e.height=v,{cat:'Text',ok:palPositive}),palF('rot','Rotation',()=>0,null,{kind:'deg',cat:'Text'}),
  palF('px','Position X',e=>e.points[0].x,(e,v)=>e.points[0]={...e.points[0],x:v}),palF('py','Position Y',e=>e.points[0].y,(e,v)=>e.points[0]={...e.points[0],y:v}),palF('pz','Position Z',()=>0)],
 block:[palF('name','Name',(e,o)=>o.name,null,{kind:'text',cat:'Misc'}),palF('n','Objects',(e,o)=>o.ids.length,null,{kind:'int',cat:'Misc'}),...palGroupPos],
 hatch:[palF('pat','Type',()=>'User defined',null,{kind:'text',cat:'Pattern'}),palF('hang','Angle',()=>45,null,{kind:'deg',cat:'Pattern'}),palF('hsp','Spacing',(e,o)=>palHatchSpacing(o),null,{cat:'Pattern'}),palF('n','Lines',(e,o)=>o.ids.length,null,{kind:'int',cat:'Pattern'}),...palGroupPos],
 group:[palF('n','Objects',(e,o)=>o.ids.length,null,{kind:'int',cat:'Misc'}),...palGroupPos]};
function palFmt(kind,v){return kind==='num'?palNum(v):kind==='deg'?palAng(v):kind==='bool'?(v?'Yes':'No'):String(v)}
function palParse(kind,raw){return kind==='num'||kind==='deg'?palParseNum(raw):kind==='bool'?raw===true||/^(y|yes|true|1|on)$/i.test(String(raw).trim()):kind==='int'?parseInt(raw,10):String(raw??'')}
function palApplyDef(def,objs,raw){const v=palParse(def.kind,raw);if((def.kind==='num'||def.kind==='deg')&&!Number.isFinite(v))return palFail('Requires a numeric value.');for(const o of objs){const err=def.ok?.(v,doc.entities[o.ids[0]],o);if(err)return palFail(err)}mutate(()=>{for(const o of objs)def.set(doc.entities[o.ids[0]],v,o)});return true}
function palDefRow(def,objs){const ents=objs.map(o=>doc.entities[o.ids[0]]);if(def.single&&objs.length>1)return null;if(def.when&&!ents.every(def.when))return null;
 if(def.kind==='spin'){const e=ents[0];return{k:def.k,label:def.label,kind:'spin',cat:def.cat,value:palVtx(e)+1,max:e.points.length,ro:false,apply:d=>palStepVertex(Number(d))}}
 const vals=objs.map((o,j)=>def.get(ents[j],o)),shown=vals.map(v=>palFmt(def.kind,v)),same=shown.every(s=>s===shown[0]);
 const row={k:def.k,label:def.label,kind:def.kind==='int'?'num':def.kind,cat:def.cat,value:same?(def.kind==='bool'?!!vals[0]:shown[0]):'*VARIES*',ro:!def.set};
 if(def.set)row.apply=raw=>palApplyDef(def,objs,raw);return row}
function palSetEntityLayer(ids,name){if(!palLayer(name))return palFail(`Layer "${name}" was not found.`);if(ids.every(i=>doc.entities[i]?.layer===name))return true;mutate(()=>{for(const i of ids)if(doc.entities[i])doc.entities[i].layer=name});return true}
function palStepVertex(d){palVertex+=d;palUpdate(true);render();return true}
const palLayerOptions=()=>doc.layers.map(l=>[l.name,l.name]);
const palMeshCache=new WeakMap();
// Triangle count, vertex count, enclosed volume (divergence theorem) and bounds of a mesh.
function palMeshStats(m){let s=palMeshCache.get(m);if(s&&s.rev===palRev)return s;let vol=0;const b={minX:Infinity,minY:Infinity,minZ:Infinity,maxX:-Infinity,maxY:-Infinity,maxZ:-Infinity};for(const p of m.vertices){b.minX=Math.min(b.minX,p.x);b.minY=Math.min(b.minY,p.y);b.minZ=Math.min(b.minZ,p.z);b.maxX=Math.max(b.maxX,p.x);b.maxY=Math.max(b.maxY,p.y);b.maxZ=Math.max(b.maxZ,p.z)}for(const f of m.faces){const [a,q,c]=f.map(i=>m.vertices[i]);if(a&&q&&c)vol+=(a.x*(q.y*c.z-q.z*c.y)-a.y*(q.x*c.z-q.z*c.x)+a.z*(q.x*c.y-q.y*c.x))/6}s={rev:palRev,vol:Math.abs(vol),b};palMeshCache.set(m,s);return s}
function palPickMesh(i){const a=$('meshA');if(a){a.value=String(i)}render();palUpdate(true)}
function palMeshCats(){const solids=doc.solids||[],cats=[],rows=[];const ai=Number($('meshA')?.value),m=solids[ai];
 rows.push(solids.length?{k:'meshes',label:'Meshes',kind:'list',items:solids.map((s,i)=>({label:`${i+1}: ${s.name}`,sub:`${s.faces.length} tri`,active:i===ai,act:()=>palPickMesh(i)}))}:{k:'meshes',label:'Meshes',kind:'ro',value:'None — use BOX or EXTRUDE'});
 if(m){const st=palMeshStats(m),b=st.b,xyz=(x,y,z)=>`${palNum(x)}, ${palNum(y)}, ${palNum(z)}`;
  rows.push({k:'meshName',label:'Name',kind:'text',value:m.name,apply:v=>{v=String(v).trim();if(!v)return palFail('Mesh name cannot be empty.');mutate(()=>doc.solids[ai].name=v);return true}},
   {k:'meshColor',label:'Color',kind:'color',value:m.color,apply:c=>{if(!/^#[0-9a-f]{6}$/i.test(c))return palFail('Invalid colour.');mutate(()=>doc.solids[ai].color=c.toLowerCase());return true}},
   {k:'tris',label:'Triangles',kind:'ro',value:String(m.faces.length)},{k:'verts',label:'Vertices',kind:'ro',value:String(m.vertices.length)},{k:'volume',label:'Volume',kind:'ro',value:palNum(st.vol)},
   {k:'sizeX',label:'Size X',kind:'ro',value:palNum(b.maxX-b.minX)},{k:'sizeY',label:'Size Y',kind:'ro',value:palNum(b.maxY-b.minY)},{k:'sizeZ',label:'Size Z',kind:'ro',value:palNum(b.maxZ-b.minZ)},
   {k:'bmin',label:'Min point',kind:'ro',value:xyz(b.minX,b.minY,b.minZ)},{k:'bmax',label:'Max point',kind:'ro',value:xyz(b.maxX,b.maxY,b.maxZ)})}
 cats.push({id:'mesh',label:'3D Mesh',rows});
 if(m?.extrusionProgram){const fs=m.extrusionProgram.features||[];cats.push({id:'features',label:'Extrusion features',rows:[{k:'base',label:'Base part',kind:'ro',value:m.extrusionProgram.base?m.extrusionProgram.base.name||'Mesh':'None'},...fs.map((f,i)=>({k:'feat'+i,label:'Feature '+(i+1),kind:'ro',value:`${f.operation==='new'?'New':f.operation[0].toUpperCase()+f.operation.slice(1)} · ${palNum(f.height)} @ Z ${palNum(f.startZ)}`}))]})}
 cats.push({id:'solidedit',label:'Solid editing',rows:[{k:'solidedit',label:'',kind:'mount'}]});return cats}
function palDrawingCats(){const ext=doc.entities.length?palBBox(doc.entities.map((e,i)=>i)):null,cur=palLayer($('layer').value)||doc.layers[0],cats=[];
 cats.push({id:'general',label:'General',rows:[{k:'color',label:'Color',kind:'swatch',value:'ByLayer',swatch:cur?.color},{k:'layer',label:'Layer',kind:'select',value:cur?.name,options:palLayerOptions(),apply:v=>palSetCurrent(v)},{k:'linetype',label:'Linetype',kind:'ro',value:'ByLayer'},{k:'lineweight',label:'Lineweight',kind:'ro',value:'ByLayer'}]});
 if(!mode3D&&CF.space==='model')cats.push({id:'view',label:'View',rows:[
  {k:'viewX',label:'Center X',kind:'num',value:palNum(view.x),live:()=>palNum(view.x),apply:raw=>{const v=palParseNum(raw);if(!Number.isFinite(v))return palFail('Requires a numeric value.');view.x=v;render();return true}},
  {k:'viewY',label:'Center Y',kind:'num',value:palNum(view.y),live:()=>palNum(view.y),apply:raw=>{const v=palParseNum(raw);if(!Number.isFinite(v))return palFail('Requires a numeric value.');view.y=v;render();return true}},
  {k:'viewH',label:'Height',kind:'num',value:palNum(H/view.scale),live:()=>palNum(H/view.scale),apply:raw=>{const v=palParseNum(raw);if(!(v>0))return palFail('Value must be greater than zero.');view.scale=Math.max(.01,Math.min(1000,H/v));render();return true}},
  {k:'viewW',label:'Width',kind:'ro',value:palNum(W/view.scale),live:()=>palNum(W/view.scale)}]});
 const dr=[{k:'name',label:'Name',kind:'ro',value:CF.fileName},{k:'objects',label:'Objects',kind:'ro',value:String(doc.entities.length)},{k:'layers',label:'Layers',kind:'ro',value:String(doc.layers.length)},{k:'blocks',label:'Block definitions',kind:'ro',value:String(Object.keys(doc.blocks||{}).length)},{k:'meshCount',label:'Meshes',kind:'ro',value:String((doc.solids||[]).length)}];
 if(ext)dr.push({k:'extMin',label:'Extents min',kind:'ro',value:`${palNum(ext.minX)}, ${palNum(ext.minY)}`},{k:'extMax',label:'Extents max',kind:'ro',value:`${palNum(ext.maxX)}, ${palNum(ext.maxY)}`});
 cats.push({id:'drawing',label:'Drawing',rows:dr});
 const flag=(k,label)=>({k,label,kind:'bool',value:CF.get(k),apply:v=>{CF.set(k,palParse('bool',v));return true}});
 cats.push({id:'drafting',label:'Drafting settings',rows:[flag('grid','Grid'),flag('snap','Snap mode'),flag('ortho','Ortho'),flag('polar','Polar tracking'),{k:'polarIncrement',label:'Polar angle',kind:'select',value:String(CF.polarIncrement),options:[...new Set([...palPolarSteps,CF.polarIncrement])].map(v=>[String(v),String(v)]),apply:v=>palSetPolarIncrement(v)||palFail('Invalid polar angle.')},flag('osnap','Object snap'),{k:'osnapModes',label:'Snap modes',kind:'ro',value:palOsnapAll.filter(m=>CF.osnapModes.has(m)).map(m=>palOsnapNames[m].slice(0,3)).join(', ')||'None'},flag('dyn','Dynamic input')]});
 cats.push({id:'misc',label:'Misc',rows:[{k:'space',label:'Space',kind:'select',value:CF.space,options:[['model','Model'],['layout','Layout1']],apply:v=>{CF.setSpace(v);return true}},{k:'workspace',label:'Workspace',kind:'select',value:CF.workspace,options:[['drafting','Drafting & Annotation'],['3d','3D Modeling']],apply:v=>{CF.setWorkspace(v);return true}},flag('ucsIcon','UCS icon On'),flag('viewCube','ViewCube'),flag('navBar','Navigation bar')]});
 return cats}
// Builds {options, filter, cats:[{id,label,rows}]} for the current selection.
function palModel(){const m=palModelRaw();for(const c of m.cats)for(const r of c.rows)if(r.kind==='ro')r.ro=true;return m}
function palModelRaw(){const ids=chosen().filter(i=>doc.entities[i]),objs=palObjects(ids),by=new Map();palMarker=null;
 for(const o of objs){if(!by.has(o.type))by.set(o.type,[]);by.get(o.type).push(o)}
 const three=mode3D||CF.workspace==='3d';
 if(!objs.length){palFilter='all';return{options:[['none','No selection']],filter:'none',cats:three?[...palMeshCats(),...palDrawingCats()]:palDrawingCats(),objects:[]}}
 if(palFilter!=='all'&&!by.has(palFilter))palFilter='all';
 const type=by.size===1?[...by.keys()][0]:palFilter==='all'?null:palFilter,targets=type?by.get(type):objs,tIds=targets.flatMap(o=>o.ids);
 const options=by.size===1?[[type,objs.length>1?`${palTypeNames[type]} (${objs.length})`:palTypeNames[type]]]:[['all',`All (${objs.length})`],...[...by].map(([t,l])=>[t,`${palTypeNames[t]} (${l.length})`])];
 const layers=new Set(tIds.map(i=>doc.entities[i].layer)),common=layers.size===1?[...layers][0]:null;
 const cats=[{id:'general',label:'General',rows:[{k:'color',label:'Color',kind:'swatch',value:'ByLayer',swatch:common?palLayer(common)?.color:null},{k:'layer',label:'Layer',kind:'select',value:common??'*VARIES*',options:palLayerOptions(),apply:v=>palSetEntityLayer(tIds,v)},{k:'linetype',label:'Linetype',kind:'ro',value:'ByLayer'},{k:'lineweight',label:'Lineweight',kind:'ro',value:'ByLayer'}]}];
 if(type){const first=doc.entities[targets[0].ids[0]];if(type==='polyline'&&targets.length===1){const key=first.uuid||targets[0].ids[0];if(key!==palVertexKey){palVertexKey=key;palVertex=0}palVertex=((palVertex%first.points.length)+first.points.length)%first.points.length;palMarker={index:targets[0].ids[0],vertex:palVertex}}
  const byCat=new Map();for(const def of palDefs[type]||[]){const r=palDefRow(def,targets);if(!r)continue;if(!byCat.has(def.cat))byCat.set(def.cat,[]);byCat.get(def.cat).push(r)}
  for(const [c,rows]of byCat)cats.push({id:c.toLowerCase(),label:c,rows})}
 else cats.push({id:'selection',label:'Selection',rows:[...by].map(([t,l])=>({k:'count-'+t,label:palTypeNames[t],kind:'ro',value:String(l.length)}))});
 if(three)cats.push(...palMeshCats());
 return{options,filter:type||'all',cats,objects:targets}}
function palEdit(key,raw){const m=palModel();for(const c of m.cats)for(const r of c.rows)if(r.k===key){if(r.ro||!r.apply)return `Property "${r.label}" is read-only.`;return r.apply(raw)}return `No property "${key}".`}

// ---- Properties palette DOM -------------------------------------------------------------
const palHost=CF.hosts.palette;
let palBody=null,palSel=null,palSigLast='',palRev=0,palStateRev=0,palDocRef=null,palDocN=0,palDirty=false,palHold=false,palLive=[];
const palCollapsed=new Set(palStore.get('collapsed',[]));
const palVisible=()=>CF.get('properties')&&!CF.get('clean');
function palFocused(){const a=document.activeElement;return !!a&&!!palHost.contains?.(a)&&/^(INPUT|SELECT|TEXTAREA)$/.test(a.tagName||'')}
function palSignature(){const ids=chosen();if(doc!==palDocRef){palDocRef=doc;palDocN++}return [palRev,palDocN,palStateRev,ids.length,ids.length<=64?ids.join(','):ids[0]+'~'+ids.at(-1),palFilter,palVertex,mode3D?1:0,CF.space,CF.workspace,$('layer').value,doc.layers.length,mode3D||CF.workspace==='3d'?$('meshA')?.value+'/'+(doc.solids||[]).length:''].join('|')}
function palBuildFrame(){const grip=palEl('div','cf-pal-grip');grip.title='Drag to resize';
 grip.onpointerdown=e=>{e.preventDefault?.();try{grip.setPointerCapture(e.pointerId)}catch{}const x0=e.clientX,w0=palHost.getBoundingClientRect().width;grip.onpointermove=ev=>{palHost.style.width=Math.round(Math.max(220,Math.min(600,w0+x0-ev.clientX)))+'px'};grip.onpointerup=()=>{grip.onpointermove=grip.onpointerup=null;palStore.set('width',parseInt(palHost.style.width)||270)}};
 const title=palEl('div','cf-pal-title');title.append(palEl('span','cf-pal-title-text','PROPERTIES'),palBtn('cf-pal-x','×','Close (PROPERTIESCLOSE)',()=>CF.toggle('properties',false)));
 const top=palEl('div','cf-pal-top');palSel=palEl('select','cf-pal-sel');palSel.title='Object types in the current selection';palSel.onchange=()=>{palFilter=palSel.value;palUpdate(true)};palSel.onkeydown=palStop;top.append(palSel);
 palBody=palEl('div','cf-pal-body');palHost.replaceChildren(grip,title,top,palBody);const w=palStore.get('width',0);if(w>=220&&w<=600)palHost.style.width=w+'px';
 try{palHost.addEventListener('focusout',()=>setTimeout(()=>{if(palDirty&&!palFocused())palUpdate()},0))}catch{}}
// The boolean controls (#meshA #meshB #booleanButtons) are engine elements: lent to the palette, returned before each rebuild.
// References are kept because an element moved into a detached subtree is invisible to getElementById.
let palMeshHome=null;const palMeshEls={};
function palPark(){if(!palMeshHome){const a=$('meshA');if(!a)return;palMeshHome=a.parentNode||null;if(!palMeshHome)return;for(const id of ['meshA','meshB','booleanButtons']){const x=$(id);if(x)palMeshEls[id]=x}}for(const id in palMeshEls){const x=palMeshEls[id];if(x.parentNode!==palMeshHome)palMeshHome.append(x)}}
function palMountSolid(v){const wrap=palEl('div','cf-pal-solid');if(palMeshHome&&palMeshEls.meshA){for(const [id,label]of [['meshA','Mesh A'],['meshB','Mesh B']]){const el=palMeshEls[id];if(!el)continue;const r=palEl('label','cf-pal-solid-row');r.append(palEl('span','',label),el);el.onkeydown??=palStop;wrap.append(r)}if(palMeshEls.booleanButtons)wrap.append(palMeshEls.booleanButtons)}
 const more=palEl('div','cf-pal-solid-btns');for(const [cmd,label,fn]of [['3DMOVE','3D Move',()=>translateMesh()],['MESHCOPY','Copy',()=>duplicateMesh()],['UPDATEEXTRUSION','Update',()=>updateExtrusion()]])more.append(palBtn('',label,`${label} (${cmd})`,()=>palRun(cmd,fn)));wrap.append(more);v.append(wrap)}
function palCommit(r,raw,refocus){if(!r.apply)return;const res=r.apply(raw);if(palHold)return res;palUpdate(true);if(refocus)palFocusKey(r.k);return res}
function palFocusKey(k){try{const i=palBody.querySelector(`[data-key="${k}"] input`);i?.focus();i?.select()}catch{}}
function palRow(r){const row=palEl('div','cf-pal-row'+(r.ro?' ro':'')+(r.kind==='mount'||r.kind==='list'?' wide':'')),k=palEl('div','cf-pal-k',r.label),v=palEl('div','cf-pal-v');k.title=r.label;row.dataset.key=r.k;row.setAttribute('data-key',r.k);
 if(r.kind==='mount'){palMountSolid(row);return row}
 if(r.kind==='list'){const list=palEl('div','cf-pal-list');for(const it of r.items){const item=palEl('div','cf-pal-item'+(it.active?' active':''));item.append(palEl('span','',it.label),palEl('span','cf-pal-sub',it.sub||''));item.onclick=it.act;list.append(item)}row.append(k,list);return row}
 row.append(k,v);
 if(r.kind==='select'||r.kind==='bool'){const s=palEl('select');const opts=r.kind==='bool'?[['Yes','Yes'],['No','No']]:r.options||[];const val=r.kind==='bool'?(r.value===true?'Yes':r.value===false?'No':'*VARIES*'):r.value;if(val==='*VARIES*')s.append(palOpt('*VARIES*'));s.append(...opts.map(([a,b])=>palOpt(a,b)));s.value=val;s.disabled=!!r.ro;s.onchange=()=>{if(s.value!=='*VARIES*')palCommit(r,s.value)};s.onkeydown=palStop;v.append(s);return row}
 if(r.kind==='swatch'){v.classList.add('cf-pal-ro');v.append(palSwatch(r.swatch),palEl('span','',r.value));return row}
 if(r.kind==='color'){const b=palSwatch(r.value,true);b.title='Select color';b.onclick=()=>palColorPopup(b,r.value,c=>palCommit(r,c));v.append(b,palEl('span','cf-pal-colortext',palColorName(r.value)));return row}
 if(r.kind==='spin'){const prev=palBtn('cf-pal-spin','◀','Previous vertex',()=>r.apply(-1)),next=palBtn('cf-pal-spin','▶','Next vertex',()=>r.apply(1));v.classList.add('cf-pal-spinbox');v.append(prev,palEl('span','cf-pal-spinval',`${r.value} / ${r.max}`),next);return row}
 if(r.ro){const s=palEl('div','cf-pal-ro',r.value);s.title=r.value;v.append(s);if(r.live)palLive.push({r,el:s});return row}
 const i=palEl('input');i.value=r.value;i.dataset.committed=r.value;i.spellcheck=false;i.setAttribute('autocomplete','off');let done=false;
 i.onkeydown=e=>{palStop(e);if(e.key==='Enter'){e.preventDefault?.();done=true;palCommit(r,i.value,true)}else if(e.key==='Escape'){e.preventDefault?.();done=true;i.value=r.value;i.blur()}};
 i.onfocus=()=>{done=false;try{i.select()}catch{}};
 // Blur commits, but the rebuild waits a tick so a click on another palette field is not swallowed.
 i.onblur=()=>{if(done||i.value===r.value)return;done=true;palHold=true;try{r.apply?.(i.value)}finally{palHold=false}setTimeout(()=>{if(palFocused())palDirty=true;else palUpdate(true)},0)};
 if(r.live)palLive.push({r,el:i,input:true});v.append(i);return row}
function palRenderModel(m){try{palRenderModelInner(m)}catch(err){palPark();throw err}}
function palRenderModelInner(m){palPark();palLive=[];
 palSel.replaceChildren(...m.options.map(([v,l])=>palOpt(v,l)));palSel.value=m.filter;palSel.disabled=m.options.length<2;
 const cats=m.cats.map(c=>{const box=palEl('div','cf-pal-cat'+(palCollapsed.has(c.id)?' collapsed':'')),head=palEl('div','cf-pal-cathead'),rows=palEl('div','cf-pal-rows');head.append(palEl('span','cf-pal-caret'),palEl('span','',c.label));head.onclick=()=>{const on=!palCollapsed.has(c.id);if(on)palCollapsed.add(c.id);else palCollapsed.delete(c.id);box.classList.toggle('collapsed',on);palStore.set('collapsed',[...palCollapsed])};for(const r of c.rows)rows.append(palRow(r));box.append(head,rows);return box});
 palBody.replaceChildren(...cats)}
function palUpdateLive(){for(const {r,el,input}of palLive){const t=r.live();if(input){if(document.activeElement!==el&&el.value!==t){el.value=t;r.value=t;el.dataset.committed=t}}else if(el.textContent!==t)el.textContent=t}}
function palUpdate(force){if(!palBody||!palVisible())return;const sig=palSignature();if(!force&&sig===palSigLast){palUpdateLive();return}if(!force&&(palHold||palFocused())){palDirty=true;return}palDirty=false;palRenderModel(palModel());palSigLast=palSignature()}
// Current polyline vertex marker (AutoCAD draws an X at the vertex being edited).
function palDrawMarker(){if(!palMarker||!palVisible()||mode3D||CF.space!=='model')return;const e=doc.entities[palMarker.index];if(!e||e.type!=='polyline')return;const p=e.points[Math.min(palMarker.vertex,e.points.length-1)];if(!p)return;const s=screen(p);ctx.save();ctx.setLineDash([]);ctx.strokeStyle=CF.colors.gripHot||'#e23c3c';ctx.lineWidth=2;ctx.beginPath();ctx.moveTo(s.x-6,s.y-6);ctx.lineTo(s.x+6,s.y+6);ctx.moveTo(s.x+6,s.y-6);ctx.lineTo(s.x-6,s.y+6);ctx.stroke();ctx.restore()}

// ---- Ribbon layer dropdown ----------------------------------------------------------------
const palDropdowns=new Set();let palDDSig='',palDDOpen=null;
function palCreateLayerDropdown(){const wrap=palEl('div','cf-laydd'),btn=palEl('button','cf-laydd-btn');btn.type='button';btn.title='Layer — click to change the current layer, turn layers on/off or change layer colors';wrap.append(btn);wrap.palBtn=btn;btn.onclick=()=>{if(palDDOpen===wrap)palCloseMenus();else palOpenDD(wrap)};palDropdowns.add(wrap);palFillDD(wrap);return wrap}
function palFillDD(wrap){const l=palLayer($('layer').value)||doc.layers[0];if(!l)return;wrap.palBtn.replaceChildren(palBulb(l.visible),palSwatch(l.color),palEl('span','cf-laydd-name',l.name),palEl('span','cf-caret'))}
function palDDRows(list){const cur=$('layer').value;list.replaceChildren(...doc.layers.map(l=>{const row=palEl('div','cf-laydd-row'+(l.name===cur?' current':'')),bulb=palBulb(l.visible),sw=palSwatch(l.color,true);bulb.title=l.visible?'Turn layer off':'Turn layer on';bulb.onclick=e=>{e.stopPropagation?.();palSetLayerVisible(l.name,!l.visible);palDDRows(list)};sw.title='Layer color: '+palColorName(l.color);sw.onclick=e=>{e.stopPropagation?.();palColorPopup(sw,l.color,c=>{palSetLayerColor(l.name,c);palDDRows(list)},{stack:true})};row.append(bulb,sw,palEl('span','cf-laydd-name',l.name));row.title=`Set "${l.name}" as the current layer`;row.onclick=()=>{palSetCurrent(l.name);palCloseMenus()};return row}))}
function palOpenDD(wrap){const list=palEl('div','cf-laydd-list');palDDRows(list);palDDOpen=wrap;wrap.classList.add('open');palPopup(list,wrap,{dir:'down',width:Math.max(200,wrap.getBoundingClientRect().width||0),onclose:()=>{wrap.classList.remove('open');if(palDDOpen===wrap)palDDOpen=null}})}
function palRefreshDropdowns(){if(palDropdowns.size>1)for(const w of palDropdowns)if(w.isConnected===false&&palDDOpen!==w)palDropdowns.delete(w);const sig=$('layer').value+JSON.stringify(doc.layers)+palIcons();if(sig===palDDSig)return;palDDSig=sig;for(const w of palDropdowns){if(w.isConnected===false&&palDDOpen!==w){palDropdowns.delete(w);continue}palFillDD(w)}}

// ---- Layer Properties Manager -------------------------------------------------------------
let palLpm=null,palLpmSel=null,palLpmEdit=null,palLpmSortDesc=false,palLpmSig='',palLpmRows=new Map(),palLpmParts={};
function palDialogFrame(id,title,cls){const d=palEl('dialog','cf-dlg '+(cls||''));d.id=id;const head=palEl('div','cf-dlg-title');head.append(palEl('span','',title),palBtn('cf-dlg-x','×','Close',()=>d.close()));d.append(head);document.body.append(d);return d}
function palToolBtn(icon,label,tip,fn){const b=palBtn('cf-tool',null,tip,fn);if(palIcons()){const s=palEl('span','cf-tool-ico');s.innerHTML=palIcon(icon,16);b.append(s)}b.append(palEl('span','',label));return b}
function palBuildLpm(){const d=palLpm=palDialogFrame('cf-layer-manager','Layer Properties Manager','cf-lpm');
 const bar=palEl('div','cf-lpm-bar');const cur=palEl('div','cf-lpm-cur'),search=palEl('input','cf-lpm-search');search.placeholder='Search for layer';search.setAttribute('autocomplete','off');search.oninput=()=>palLpmRender(true);
 bar.append(palToolBtn('plus','New Layer','New Layer (Alt+N) — creates a layer with the properties of the selected layer',palLpmNew),palToolBtn('erase','Delete Layer','Delete Layer (Alt+D)',palLpmDelete),palToolBtn('check','Set Current','Set Current (Alt+C)',palLpmCurrent),palEl('span','cf-lpm-gap'),cur,search);
 const grid=palEl('div','cf-lpm-grid'),table=palEl('table','cf-lpm-table'),thead=palEl('thead'),hr=palEl('tr'),tbody=palEl('tbody');
 for(const [key,label,cls]of [['status','Status','c-status'],['name','Name','c-name'],['on','On','c-on'],['color','Color','c-color'],['linetype','Linetype','c-lt'],['objects','Objects','c-obj']]){const th=palEl('th',cls,label);if(key==='name'){th.classList.add('sortable');th.title='Sort by name';th.onclick=()=>{palLpmSortDesc=!palLpmSortDesc;palLpmRender(true)}}hr.append(th)}
 thead.append(hr);table.append(thead,tbody);grid.append(table);
 const msg=palEl('div','cf-lpm-msg'),foot=palEl('div','cf-dlg-foot'),count=palEl('span','cf-lpm-count');foot.append(count,palBtn('cf-primary','Close','Close the Layer Properties Manager',()=>d.close()));
 const body=palEl('div','cf-dlg-body cf-lpm-body');body.append(bar,grid,msg);d.append(body,foot);palLpmParts={cur,search,tbody,msg,count,grid};
 d.onkeydown=e=>{e.stopPropagation?.();if(/^(INPUT|SELECT|TEXTAREA)$/.test(e.target?.tagName||''))return;const k=e.key;if(k==='F2'){e.preventDefault?.();palLpmStartRename(palLpmSel)}else if(k==='Delete')palLpmDelete();else if(e.altKey&&/^[ndc]$/i.test(k)){e.preventDefault?.();({n:palLpmNew,d:palLpmDelete,c:palLpmCurrent})[k.toLowerCase()]()}else if(k==='ArrowDown'||k==='ArrowUp'){e.preventDefault?.();const names=palLpmVisibleNames(),i=names.indexOf(palLpmSel),j=Math.max(0,Math.min(names.length-1,i+(k==='ArrowDown'?1:-1)));if(names[j])palLpmSelect(names[j])}};
 d.oncancel=e=>{if(palLpmEdit){e.preventDefault?.();palLpmFinishRename(false)}};d.onclose=()=>{palLpmEdit=null;palCloseMenus();try{canvas.focus()}catch{}}}
function palLpmMsg(t){if(palLpm)palLpmParts.msg.textContent=t||''}
function palLpmVisibleNames(){const q=String(palLpmParts.search?.value||'').trim();let test=()=>true;if(q){const re=/[*?]/.test(q)?new RegExp('^'+q.replace(/[.+^${}()|[\]\\]/g,'\\$&').replace(/\*/g,'.*').replace(/\?/g,'.')+'$','i'):null;test=n=>re?re.test(n):n.toLowerCase().includes(q.toLowerCase())}
 const names=doc.layers.map(l=>l.name).filter(test).sort((a,b)=>a.localeCompare(b,undefined,{numeric:true,sensitivity:'base'}));return palLpmSortDesc?names.reverse():names}
function palLpmSelect(name){palLpmSel=name;for(const [n,tr]of palLpmRows)tr.classList.toggle('sel',n===name)}
function palLpmRender(force){if(!palLpm)return;if(palLpmEdit&&!force)return;palLpmEdit=null;const cur=$('layer').value,counts=new Map();for(const e of doc.entities)counts.set(e.layer,(counts.get(e.layer)||0)+1);
 if(!palLayer(palLpmSel))palLpmSel=cur;palLpmRows=new Map();const names=palLpmVisibleNames();
 palLpmParts.tbody.replaceChildren(...names.map(name=>{const l=palLayer(name),tr=palEl('tr','cf-lpm-row'+(name===cur?' cur':'')+(name===palLpmSel?' sel':''));
  const st=palEl('td','c-status',name===cur?'✓':'');st.title=name===cur?'Current layer':'Double-click to set current';st.ondblclick=()=>{palLpmSelect(name);palLpmCurrent()};
  const nm=palEl('td','c-name'),ns=palEl('span','',name);nm.append(ns);nm.ondblclick=()=>palLpmStartRename(name);nm.title=name==='0'?'Layer 0 cannot be renamed':'Double-click or press F2 to rename';tr.palName=nm;
  const on=palEl('td','c-on'),bulb=palBulb(l.visible);bulb.title=l.visible?'On — click to turn off':'Off — click to turn on';bulb.onclick=e=>{e.stopPropagation?.();palLpmSelect(name);palSetLayerVisible(name,!palLayer(name).visible);palLpmRender(true)};on.append(bulb);
  const co=palEl('td','c-color'),sw=palSwatch(l.color,true);sw.title='Select color';co.append(sw,palEl('span','',palColorName(l.color)));co.onclick=e=>{e.stopPropagation?.();palLpmSelect(name);palColorPopup(sw,palLayer(name).color,c=>{palSetLayerColor(name,c);palLpmRender(true)})};
  tr.append(st,nm,on,co,palEl('td','c-lt','Continuous'),palEl('td','c-obj',String(counts.get(name)||0)));tr.onclick=()=>palLpmSelect(name);palLpmRows.set(name,tr);return tr}));
 palLpmParts.cur.textContent='Current layer: '+cur;palLpmParts.count.textContent=`All: ${names.length} layer${names.length===1?'':'s'} displayed of ${doc.layers.length} total layer${doc.layers.length===1?'':'s'}`;palLpmSig=palLpmSignature()}
const palLpmSignature=()=>palRev+'|'+palDocN+'|'+$('layer').value+'|'+JSON.stringify(doc.layers)+'|'+doc.entities.length;
function palLpmUpdate(){if(palLpm?.open&&!palLpmEdit&&palLpmSignature()!==palLpmSig)palLpmRender()}
function palLpmNew(){const name=palNextLayerName(),r=palAddLayer(name,palLpmSel);if(r!==true){palLpmMsg(r);return}palLpmSel=name;palLpmMsg('');palLpmRender(true);palLpmStartRename(name)}
function palLpmDelete(){if(!palLpmSel)return;const name=palLpmSel,r=palDeleteLayer(name);palLpmMsg(r===true?`Layer "${name}" deleted.`:r);if(r===true)palLpmSel=$('layer').value;palLpmRender(true)}
function palLpmCurrent(){if(!palLpmSel)return;const r=palSetCurrent(palLpmSel);palLpmMsg(r===true?'':r);palLpmRender(true)}
function palLpmStartRename(name){if(!name||!palLayer(name))return;if(name==='0'){palLpmMsg('Layer 0 cannot be renamed.');return}const tr=palLpmRows.get(name);if(!tr)return;palLpmSelect(name);const inp=palEl('input','cf-lpm-edit');inp.value=name;inp.setAttribute('autocomplete','off');palLpmEdit={name,inp};tr.palName.replaceChildren(inp);
 inp.onkeydown=e=>{e.stopPropagation?.();if(e.key==='Enter'){e.preventDefault?.();palLpmFinishRename(true,true)}else if(e.key==='Escape'){e.preventDefault?.();palLpmFinishRename(false)}};inp.onblur=()=>{if(palLpmEdit?.inp===inp)palLpmFinishRename(true,false)};try{inp.focus();inp.select()}catch{}}
function palLpmFinishRename(commit,keepOnError){const ed=palLpmEdit;if(!ed)return;const to=String(ed.inp.value).trim();if(commit&&to!==ed.name){const r=palRenameLayer(ed.name,to);if(r!==true){palLpmMsg(r);if(keepOnError){try{ed.inp.focus();ed.inp.select()}catch{}return}}else{palLpmSel=to;palLpmMsg('')}}palLpmEdit=null;palLpmRender(true);try{palLpm.focus?.()}catch{}}
function palOpenLayerManager(){if(!palLpm)palBuildLpm();palLpmSel=palLayer(palLpmSel)?palLpmSel:$('layer').value;palLpmMsg('');palLpmRender(true);try{if(!palLpm.open)palLpm.showModal()}catch{palLpm.open=true}}

// ---- Drafting Settings dialog (DSETTINGS) -------------------------------------------------
let palDs=null,palDsParts={};
function palChk(label,checked){const l=palEl('label','cf-chk'),i=palEl('input');i.type='checkbox';i.checked=!!checked;l.append(i,palEl('span','',label));l.input=i;return l}
function palBuildDs(){const d=palDs=palDialogFrame('cf-drafting-settings','Drafting Settings','cf-ds');const tabs=palEl('div','cf-ds-tabs'),pages=palEl('div','cf-ds-pages'),P={};
 const mk=(id,label)=>{const t=palBtn('cf-ds-tab',label,null,()=>palDsTab(id)),p=palEl('div','cf-ds-page');t.dataset.tab=id;p.dataset.tab=id;tabs.append(t);pages.append(p);P[id]={t,p};return p};
 const snap=mk('snap','Snap and Grid'),polar=mk('polar','Polar Tracking'),osnap=mk('osnap','Object Snap'),dyn=mk('dyn','Dynamic Input');
 const c={snap:palChk('Snap On (F9)'),grid:palChk('Grid On (F7)'),ortho:palChk('Ortho On (F8)'),polar:palChk('Polar Tracking On (F10)'),osnap:palChk('Object Snap On (F3)'),dyn:palChk('Enable Dynamic Input (F12)')};
 snap.append(c.snap,c.grid,c.ortho,palEl('p','cf-note','Snap and grid spacing follow the visible grid, which adapts to the zoom level (major lines every 5 minor lines).'));
 const inc=palEl('select');for(const v of palPolarSteps)inc.append(palOpt(String(v)));const incRow=palEl('label','cf-field');incRow.append(palEl('span','','Increment angle:'),inc);polar.append(c.polar,palEl('div','cf-group-title','Polar Angle Settings'),incRow,palEl('p','cf-note','The cursor tracks along multiples of the increment angle from the last point. Ortho and Polar are mutually exclusive.'));
 const modes={},cols=palEl('div','cf-ds-modes');for(const m of palOsnapAll){const l=palChk(palOsnapNames[m]);const mark=palEl('span','cf-osmark',palOsnapMarks[m]);l.insertBefore(mark,l.children?.[1]||null);modes[m]=l;cols.append(l)}
 const all=palBtn('','Select All',null,()=>{for(const m in modes)modes[m].input.checked=true}),none=palBtn('','Clear All',null,()=>{for(const m in modes)modes[m].input.checked=false}),side=palEl('div','cf-ds-side');side.append(all,none);
 const os=palEl('div','cf-ds-osgrid');os.append(cols,side);osnap.append(c.osnap,palEl('div','cf-group-title','Object Snap modes'),os);
 dyn.append(c.dyn,palEl('p','cf-note','Shows the command prompt, coordinates, distance and angle next to the crosshairs while you draw.'));
 const body=palEl('div','cf-dlg-body');body.append(tabs,pages);const foot=palEl('div','cf-dlg-foot');foot.append(palEl('span','cf-lpm-count',''),palBtn('cf-primary','OK',null,palDsApply),palBtn('','Cancel',null,()=>d.close()));d.append(body,foot);
 d.onkeydown=e=>{e.stopPropagation?.();if(e.key==='Enter'&&e.target?.tagName!=='BUTTON'){e.preventDefault?.();palDsApply()}};palDsParts={P,c,inc,modes}}
function palDsTab(id){for(const [k,{t,p}]of Object.entries(palDsParts.P)){t.classList.toggle('active',k===id);p.classList.toggle('active',k===id)}}
function palDsApply(){const {c,inc,modes}=palDsParts;for(const k of ['snap','grid','ortho','polar','osnap','dyn'])if(c[k].input.checked!==CF.get(k)&&!(k==='polar'&&c.ortho.input.checked&&c.polar.input.checked))CF.set(k,c[k].input.checked,{quiet:true});
 for(const m of palOsnapAll)if(modes[m].input.checked!==CF.osnapModes.has(m))palSetOsnapMode(m,modes[m].input.checked);palSetPolarIncrement(inc.value);palDs.close();notify('Drafting settings updated.')}
function palOpenDraftingSettings(tab='snap'){if(!palDs)palBuildDs();const {c,inc,modes}=palDsParts;for(const k in c)c[k].input.checked=CF.get(k);if(!palPolarSteps.includes(CF.polarIncrement)&&![...inc.children].some(o=>o.value===String(CF.polarIncrement)))inc.append(palOpt(String(CF.polarIncrement)));inc.value=String(CF.polarIncrement);for(const m of palOsnapAll)modes[m].input.checked=CF.osnapModes.has(m);palDsTab(tab);try{if(!palDs.open)palDs.showModal()}catch{palDs.open=true}}

// ---- Status bar -----------------------------------------------------------------------------
const palSB={},palSBIconEls=[],palSBItems={};let palCoordsOn=true,palCoordText='',palSBState={};
const palSBHidden=new Set(palStore.get('statusHidden',[]));
const palToggles=[{k:'grid',icon:'grid',text:'GRID',tip:'Grid Display — show the drawing grid (F7)'},{k:'snap',icon:'snap',text:'SNAP',tip:'Snap Mode — snap the cursor to the grid (F9)'},{k:'ortho',icon:'ortho',text:'ORTHO',tip:'Ortho Mode — constrain the cursor to horizontal and vertical (F8)'},{k:'polar',icon:'polar',text:'POLAR',tip:'Polar Tracking — guide the cursor along angle increments (F10). Right-click for angles.',menu:a=>palPolarMenu(a)},{k:'osnap',icon:'osnap',text:'OSNAP',tip:'Object Snap — snap the cursor to points on objects (F3). Right-click for snap modes.',menu:a=>palOsnapMenu(a)},{k:'dyn',icon:'dyn',text:'DYN',tip:'Dynamic Input — show prompts and input near the cursor (F12)'}];
function palSBButton(cls,icon,text,tip,onclick){const b=palBtn('cf-sb-btn '+(cls||''),null,tip,onclick);const ic=palEl('span','cf-sb-ico');ic.dataset.icon=icon;palSBIconEls.push(ic);b.append(ic,palEl('span','cf-sb-txt',text));return b}
function palPolarMenu(a){palMenu([{header:'Polar tracking angle'},...palPolarSteps.map(v=>({label:`${v}, ${[...Array(Math.min(4,Math.floor(360/v)))].map((_,i)=>String(Number((v*(i+2)).toFixed(1)))).slice(0,3).join(', ')}...`,checked:CF.polarIncrement===v,radio:true,act:()=>{palSetPolarIncrement(v);if(!CF.get('polar'))CF.set('polar',true)}})),{sep:true},{label:'Tracking Settings…',act:()=>palOpenDraftingSettings('polar')}],a,{dir:'up'})}
function palOsnapMenu(a){palMenu([...palOsnapAll.map(m=>({label:palOsnapNames[m],mark:palOsnapMarks[m],checked:CF.osnapModes.has(m),act:()=>{const on=!CF.osnapModes.has(m);palSetOsnapMode(m,on);if(on&&!CF.get('osnap'))CF.set('osnap',true)}})),{sep:true},{label:'Object Snap Settings…',act:()=>palOpenDraftingSettings('osnap')}],a,{dir:'up'})}
function palWorkspaceMenu(a){palMenu([{label:'Drafting & Annotation',checked:CF.workspace==='drafting',radio:true,act:()=>CF.setWorkspace('drafting')},{label:'3D Modeling',checked:CF.workspace==='3d',radio:true,act:()=>CF.setWorkspace('3d')},{sep:true},{label:'Drafting Settings…',act:()=>palOpenDraftingSettings()}],a,{dir:'up'})}
const palSBCustom=[['coords','Coordinates'],['model','Model Space'],['grid','Grid'],['snap','Snap Mode'],['ortho','Ortho Mode'],['polar','Polar Tracking'],['osnap','Object Snap'],['dyn','Dynamic Input'],['workspace','Workspace Switching'],['properties','Properties'],['clean','Clean Screen']];
function palCustomizeMenu(a){palMenu([{header:'Customization'},...palSBCustom.map(([k,label])=>({label,checked:!palSBHidden.has(k),act:()=>{if(palSBHidden.has(k))palSBHidden.delete(k);else palSBHidden.add(k);palStore.set('statusHidden',[...palSBHidden]);palApplySBHidden()}}))],a,{dir:'up'})}
function palApplySBHidden(){for(const [k,el]of Object.entries(palSBItems))el.style.display=palSBHidden.has(k)?'none':''}
function palBuildStatus(){const sb=CF.hosts.statusbar;sb.classList.add('cf-sb');const left=palEl('div','cf-sb-left'),right=palEl('div','cf-sb-right'),tabs=palEl('div','cf-sb-tabs');
 const model=palBtn('cf-sb-tab','Model','Model space',()=>CF.setSpace('model')),layout=palBtn('cf-sb-tab','Layout1','Layout1 — paper space sheet (right-click for page setup)',()=>CF.setSpace('layout'));
 layout.oncontextmenu=e=>{e.preventDefault?.();palMenu([{label:'Page Setup Manager…',act:()=>palRun('PAGESETUP',()=>plotSheet())},{label:'Plot…',act:()=>palRun('PLOT',()=>plotSheet())},{sep:true},{label:'Activate Model Tab',act:()=>CF.setSpace('model')}],layout,{dir:'up'})};
 tabs.append(model,layout);palSB.modelTab=model;palSB.layoutTab=layout;palSBItems.tabs=tabs;
 const coords=palEl('div','cf-sb-coords','0.0000, 0.0000, 0.0000');coords.title='Cursor coordinates — click to turn the display on or off';coords.onclick=()=>{palCoordsOn=!palCoordsOn;coords.classList.toggle('off',!palCoordsOn);palCoordText='';palUpdateCoords()};palSB.coords=coords;palSBItems.coords=coords;left.append(tabs,coords);
 const space=palBtn('cf-sb-btn cf-sb-model','MODEL','Model or paper space — click to switch',()=>CF.setSpace(CF.space==='model'?'layout':'model'));palSB.model=space;palSBItems.model=space;right.append(space,palEl('span','cf-sb-sep'));
 for(const t of palToggles){const b=palSBButton('',t.icon,t.text,t.tip,()=>CF.toggle(t.k));b.dataset.k=t.k;b.setAttribute('aria-pressed','false');palSB[t.k]=b;
  if(t.menu){const arrow=palBtn('cf-sb-btn cf-sb-arrow','','Settings for '+t.text.toLowerCase(),()=>t.menu(arrow));arrow.append(palEl('span','cf-caret up'));b.oncontextmenu=e=>{e.preventDefault?.();t.menu(b)};const grp=palEl('span','cf-sb-split');grp.append(b,arrow);palSBItems[t.k]=grp;palSB[t.k+'Menu']=arrow;right.append(grp)}else{palSBItems[t.k]=b;right.append(b)}}
 const s2=palEl('span','cf-sb-sep');right.append(s2);
 const ws=palSBButton('cf-sb-ws','gear','Drafting & Annotation','Workspace Switching — Drafting & Annotation / 3D Modeling',null);ws.onclick=()=>palWorkspaceMenu(ws);ws.append(palEl('span','cf-caret up'));palSB.workspace=ws;palSBItems.workspace=ws;
 const pr=palSBButton('','properties','PROPS','Properties palette (Ctrl+1)',()=>CF.toggle('properties'));palSB.properties=pr;palSBItems.properties=pr;
 const cl=palSBButton('','clean','CLEAN','Clean Screen — maximize the drawing area (Ctrl+0)',()=>CF.toggle('clean'));palSB.clean=cl;palSBItems.clean=cl;
 const cu=palSBButton('cf-sb-custom','menu','≡','Customization — choose which status bar items are shown',null);cu.onclick=()=>palCustomizeMenu(cu);palSB.customize=cu;
 right.append(ws,pr,cl,palEl('span','cf-sb-sep'),cu);sb.replaceChildren(left,right);palSBIcons();palApplySBHidden();palSyncStatus(true)}
function palSBIcons(){const on=palIcons();CF.hosts.statusbar.classList.toggle('cf-sb-textmode',!on);if(on)for(const s of palSBIconEls)s.innerHTML=palIcon(s.dataset.icon,16)}
function palSyncStatus(force){const st={space:CF.space,ws:CF.workspace,tabs:CF.get('layoutTabs'),properties:CF.get('properties'),clean:CF.get('clean')};for(const t of palToggles)st[t.k]=CF.get(t.k);
 if(!force&&Object.keys(st).every(k=>st[k]===palSBState[k]))return;palSBState=st;
 for(const t of palToggles){palSB[t.k].classList.toggle('on',st[t.k]);palSB[t.k].setAttribute('aria-pressed',String(st[t.k]))}
 palSB.modelTab.classList.toggle('active',st.space==='model');palSB.layoutTab.classList.toggle('active',st.space==='layout');palSB.model.textContent=st.space==='model'?'MODEL':'PAPER';palSBItems.tabs.style.display=st.tabs?'':'none';
 palSB.properties.classList.toggle('on',st.properties);palSB.clean.classList.toggle('on',st.clean);const wt=palSB.workspace.querySelector?.('.cf-sb-txt');const wsName=st.ws==='3d'?'3D Modeling':'Drafting & Annotation';if(wt&&wt.textContent!==undefined)wt.textContent=wsName;palSB.workspace.title='Workspace Switching — current: '+wsName;palSB.workspace.dataset.ws=st.ws}
function palUpdateCoords(){if(!palCoordsOn)return;const t=mode3D?'3D':`${palNum(mouse.x)}, ${palNum(mouse.y)}, 0.0000`;if(t!==palCoordText){palCoordText=t;palSB.coords.textContent=t}}

// ---- Engine dialog theming (CSS only) and module styles ---------------------------------------
const palDlg=':is(#extrusionDialog,#plotDialog,#exportDialog,dialog:has(#dialogLabel))';
const palStyle=palEl('style');palStyle.id='cf-palettes-style';palStyle.textContent=`
#cf-statusbar{height:28px;display:flex;align-items:stretch;justify-content:space-between;background:var(--cf-frame);border-top:1px solid #12161c;color:var(--cf-text-dim);user-select:none;font-size:12px;overflow:hidden}
.cf-sb-left,.cf-sb-right{display:flex;align-items:center;min-width:0}.cf-sb-left{flex:1 1 auto;overflow:hidden}.cf-sb-right{flex:none;gap:1px;padding:0 4px}
.cf-sb-tabs{display:flex;align-items:stretch;height:100%;padding-left:2px;border-right:1px solid #2b323c}
.cf-sb-tab{border:0;border-radius:0;background:transparent;color:var(--cf-text-dim);padding:0 14px;height:100%;position:relative}
.cf-sb-tab:hover{background:#2b323c;color:var(--cf-text)}.cf-sb-tab.active{background:#2b323c;color:#fff}.cf-sb-tab.active::after{content:'';position:absolute;left:0;right:0;bottom:0;height:2px;background:var(--cf-accent)}
.cf-sb-coords{font:12px var(--cf-mono);padding:0 12px;white-space:nowrap;color:var(--cf-text);cursor:default;min-width:0;overflow:hidden;text-overflow:ellipsis}.cf-sb-coords.off{color:var(--cf-text-faint)}
.cf-sb-btn{height:22px;min-width:24px;padding:0 3px;border:1px solid transparent;background:transparent;border-radius:2px;display:inline-flex;align-items:center;justify-content:center;gap:4px;color:#8b95a3;font-size:11px}
.cf-sb-btn:hover{background:var(--cf-hover);border-color:var(--cf-hover-border);color:var(--cf-text)}
.cf-sb-btn.on{color:#7fbcff;background:#22354a;border-color:#2f5d8a}.cf-sb-btn.on:hover{background:#2a4561}
.cf-sb-model{font-weight:600;letter-spacing:.6px;padding:0 8px;color:var(--cf-text)}
.cf-sb-split{display:inline-flex;align-items:center}.cf-sb-arrow{min-width:11px;width:11px;padding:0}.cf-sb-split .cf-sb-btn:first-child{margin-right:0}
.cf-sb-sep{width:1px;height:16px;background:#3a424e;margin:0 5px;flex:none}
.cf-sb-ico{display:inline-flex}.cf-sb-ico svg{display:block}.cf-sb-txt{display:none}
.cf-sb-textmode .cf-sb-ico{display:none}.cf-sb-textmode .cf-sb-txt{display:inline;font-weight:600;letter-spacing:.4px;padding:0 3px}
.cf-sb-ws .cf-sb-txt{font-weight:400!important;letter-spacing:0!important}.cf-sb-custom .cf-sb-txt{font-size:14px;font-weight:400!important}
.cf-caret{display:inline-block;width:0;height:0;border-left:4px solid transparent;border-right:4px solid transparent;border-top:4px solid currentColor;flex:none;opacity:.8}.cf-caret.up{border-top:0;border-bottom:4px solid currentColor}
.cf-pal-popup{position:fixed;z-index:2000}
.cf-pmenu,.cf-colorpop{background:#262c35;border:1px solid #4a5260;box-shadow:var(--cf-shadow);padding:3px 0;min-width:190px;font-size:12px;color:var(--cf-text)}
.cf-pmenu-item{display:grid;grid-template-columns:18px 16px 1fr auto;align-items:center;height:24px;padding:0 12px 0 4px;cursor:default;white-space:nowrap}
.cf-pmenu-item:hover{background:var(--cf-active)}.cf-pmenu-item.disabled{color:var(--cf-text-faint)}
.cf-pmenu-chk.on::before{content:'\\2713';color:var(--cf-text);padding-left:4px}.cf-pmenu-chk.radio::before{content:'';display:block;width:6px;height:6px;border-radius:50%;background:var(--cf-text);margin-left:5px}
.cf-pmenu-mark{color:${CF.colors.osnap||'#3ddc84'};font-size:12px;text-align:center}.cf-pmenu-hint{color:var(--cf-text-faint);padding-left:16px}
.cf-pmenu-sep{height:1px;background:#3a424e;margin:3px 0}.cf-pmenu-head{padding:4px 10px 3px;color:var(--cf-text-faint);font-size:11px;text-transform:uppercase;letter-spacing:.5px}
.cf-swatch{width:12px;height:12px;border:1px solid #0b0d10;display:inline-block;flex:none;padding:0;border-radius:0;vertical-align:middle;box-shadow:inset 0 0 0 1px rgba(255,255,255,.12)}button.cf-swatch{cursor:pointer}button.cf-swatch:hover{outline:1px solid var(--cf-hover-border)}.cf-swatch.none{background:repeating-linear-gradient(45deg,#3a424e 0 3px,#2b323c 3px 6px)!important}
.cf-colorpop{padding:4px 8px 8px;min-width:0;width:212px}.cf-colorpop-grid{display:grid;grid-template-columns:repeat(9,18px);gap:4px;padding:4px 0 8px}.cf-colorpop-grid .cf-swatch{width:18px;height:18px}.cf-colorpop-grid .current{outline:2px solid var(--cf-accent);outline-offset:1px}
.cf-colorpop-cur{display:flex;align-items:center;gap:6px;padding:2px 0 8px;color:var(--cf-text-dim)}.cf-colorpop-more{width:100%;height:24px;padding:0 8px}
.cf-native-color{position:fixed;width:1px;height:1px;opacity:0;pointer-events:none;border:0;padding:0}
.cf-bulb{display:inline-block;flex:none;width:10px;height:10px;border-radius:50% 50% 45% 45%;position:relative;margin:0 3px 3px;cursor:pointer;vertical-align:middle}
.cf-bulb.on{background:radial-gradient(circle at 40% 35%,#fff6c2,#f5c84a 60%,#c99a1e);box-shadow:0 0 5px rgba(245,200,74,.55)}.cf-bulb.off{background:radial-gradient(circle at 40% 35%,#8db3d9,#3e6a96 65%,#2a4a6b)}
.cf-bulb::after{content:'';position:absolute;left:3px;bottom:-3px;width:4px;height:3px;background:#9aa4b1;border-radius:0 0 1px 1px}
.cf-bulb.icon{width:auto;height:auto;background:none;box-shadow:none;border-radius:0;margin:0}.cf-bulb.icon::after{display:none}.cf-bulb.icon svg{display:block}
#cf-palette{position:relative;font-size:12px}
.cf-pal-grip{position:absolute;left:-3px;top:0;bottom:0;width:6px;cursor:ew-resize;z-index:2}
.cf-pal-title{height:26px;flex:none;display:flex;align-items:center;justify-content:space-between;padding:0 3px 0 10px;background:#1f252d;border-bottom:1px solid #12161c;color:var(--cf-text-dim);font-size:11px;font-weight:600;letter-spacing:.8px}
.cf-pal-x,.cf-dlg-x{border:0;background:transparent;width:22px;height:22px;padding:0;color:var(--cf-text-dim);font-size:16px;line-height:20px}.cf-pal-x:hover,.cf-dlg-x:hover{background:#c0392b;color:#fff}
.cf-pal-top{flex:none;padding:5px 6px;display:flex;gap:4px;border-bottom:1px solid #12161c;background:var(--cf-panel)}.cf-pal-top select{flex:1;min-width:0;height:24px;padding:2px 4px}
.cf-pal-body{flex:1;overflow:auto;min-height:0;scrollbar-width:thin}
.cf-pal-cathead{height:22px;display:flex;align-items:center;gap:7px;padding:0 6px;background:var(--cf-panel-header);border-top:1px solid #414a57;border-bottom:1px solid #222830;font-weight:600;cursor:pointer;color:var(--cf-text)}.cf-pal-cathead:hover{background:#3b4451}
.cf-pal-caret{width:0;height:0;border-left:4px solid transparent;border-right:4px solid transparent;border-top:5px solid var(--cf-text-dim);transition:transform .1s}.cf-pal-cat.collapsed .cf-pal-caret{transform:rotate(-90deg)}.cf-pal-cat.collapsed .cf-pal-rows{display:none}
.cf-pal-row{display:grid;grid-template-columns:44% 56%;min-height:22px;border-bottom:1px solid #262c35}.cf-pal-row:hover{background:#313945}.cf-pal-row:focus-within{background:#2a3d52}
.cf-pal-row.wide{display:block}
.cf-pal-k{padding:3px 6px 3px 18px;color:var(--cf-text-dim);border-right:1px solid #262c35;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;align-self:center}
.cf-pal-v{display:flex;align-items:center;gap:6px;min-width:0;padding-right:2px}
.cf-pal-v input,.cf-pal-v select{width:100%;min-width:0;height:21px;border:1px solid transparent;background:transparent;padding:1px 5px;border-radius:0}
.cf-pal-v input:hover,.cf-pal-v select:hover{border-color:var(--cf-border);background:var(--cf-input)}.cf-pal-v input:focus,.cf-pal-v select:focus{background:var(--cf-input);border-color:var(--cf-accent);outline:none}
.cf-pal-v select:disabled{opacity:.6}
.cf-pal-ro,.cf-pal-v.cf-pal-ro{padding:3px 6px;color:var(--cf-text-faint);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-variant-numeric:tabular-nums}
.cf-pal-v.cf-pal-ro{padding-left:6px}.cf-pal-v.cf-pal-ro span{color:var(--cf-text)}
.cf-pal-v input{font-variant-numeric:tabular-nums}
.cf-pal-v>.cf-swatch{margin-left:6px}.cf-pal-colortext{color:var(--cf-text)}
.cf-pal-spinbox{padding-left:4px}.cf-pal-spin{width:20px;height:18px;padding:0;font-size:9px;line-height:16px;background:#3a4350}.cf-pal-spinval{min-width:56px;text-align:center;font-variant-numeric:tabular-nums}
.cf-pal-list{padding:2px 4px 4px 18px;max-height:150px;overflow:auto}.cf-pal-item{display:flex;justify-content:space-between;gap:8px;padding:2px 6px;cursor:pointer;border:1px solid transparent}.cf-pal-item:hover{background:var(--cf-hover)}.cf-pal-item.active{background:var(--cf-active);border-color:var(--cf-active-border)}.cf-pal-sub{color:var(--cf-text-faint)}
.cf-pal-solid{padding:6px 8px 8px 18px;display:flex;flex-direction:column;gap:5px}.cf-pal-solid-row{display:grid;grid-template-columns:56px 1fr;align-items:center;gap:6px;margin:0;color:var(--cf-text-dim)}.cf-pal-solid-row select{height:22px;padding:1px 4px;width:100%;margin:0}
#cf-palette #booleanButtons,.cf-pal-solid-btns{display:flex;gap:4px;flex-wrap:wrap}#cf-palette #booleanButtons button,.cf-pal-solid-btns button{flex:1 1 auto;height:24px;padding:0 6px;margin:0;font-size:11px;width:auto;white-space:nowrap}
.cf-laydd{position:relative;display:inline-flex;width:220px;max-width:100%;height:24px}
.cf-laydd-btn{flex:1;min-width:0;display:flex;align-items:center;gap:5px;padding:0 6px 0 4px;height:24px;background:var(--cf-input);border:1px solid var(--cf-border);border-radius:2px;text-align:left}.cf-laydd.open .cf-laydd-btn{border-color:var(--cf-accent)}
.cf-laydd-name{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.cf-laydd-list{background:#262c35;border:1px solid var(--cf-border);box-shadow:var(--cf-shadow);max-height:320px;overflow:auto;padding:2px 0}
.cf-laydd-row{display:flex;align-items:center;gap:6px;height:24px;padding:0 8px 0 4px;cursor:default;white-space:nowrap}.cf-laydd-row:hover{background:var(--cf-hover)}.cf-laydd-row.current{background:var(--cf-active)}
dialog.cf-dlg:focus,${palDlg}:focus{outline:none}
dialog.cf-dlg{padding:0;border:1px solid #12161c;border-radius:2px;background:var(--cf-panel);color:var(--cf-text);font:12px/1.4 var(--cf-font);box-shadow:var(--cf-shadow);width:auto;max-width:95vw}
.cf-dlg-title,${palDlg} h2{display:flex;align-items:center;justify-content:space-between;height:30px;margin:0;padding:0 4px 0 12px;background:var(--cf-frame);border-bottom:1px solid #12161c;font-size:12px;font-weight:600;color:var(--cf-text);letter-spacing:.2px}
.cf-dlg-body{padding:10px 12px}.cf-dlg-foot,${palDlg} .actions{display:flex;align-items:center;justify-content:flex-end;gap:6px;padding:8px 12px;background:#262c35;border-top:1px solid #3a424e}
.cf-dlg-foot>span:first-child{margin-right:auto;color:var(--cf-text-dim)}
.cf-dlg-foot button,${palDlg} .actions button{min-width:76px;height:24px;padding:0 12px}
.cf-primary,#dialogOk,#extrusionApply,#sheetPDF,.exportDownload{background:var(--cf-active)!important;border-color:var(--cf-active-border)!important;color:#fff!important}.cf-primary:hover,#dialogOk:hover,#extrusionApply:hover,#sheetPDF:hover,.exportDownload:hover{background:#3a6fa3!important}
${palDlg}{padding:0;border:1px solid #12161c;border-radius:2px;background:var(--cf-panel);color:var(--cf-text);font:12px/1.4 var(--cf-font);box-shadow:var(--cf-shadow)}
${palDlg} h2{margin:0 0 4px;height:30px;line-height:30px}
${palDlg}>form{margin:0}
${palDlg} .actions{margin:12px 0 0}
${palDlg} label{display:block;margin:8px 12px 3px;color:var(--cf-text-dim);font-size:12px}
${palDlg} [hidden]{display:none!important}
${palDlg} input:not([type=checkbox]):not([hidden]),${palDlg} select,${palDlg} textarea{display:block;width:calc(100% - 24px)!important;margin:0 12px!important;height:24px;padding:2px 6px;box-sizing:border-box}
${palDlg} textarea{height:220px!important;font:11px var(--cf-mono)!important;background:#161b21!important;color:var(--cf-text)!important;border:1px solid var(--cf-border)!important}
${palDlg} p{margin:8px 12px;color:var(--cf-text-dim);line-height:1.45}
dialog:has(#dialogLabel){width:420px}dialog:has(#dialogLabel) .actions{flex-direction:row}#dialogOk{order:-1}
#extrusionDialog{width:460px}#extrusionError{color:#ff9a7a!important;min-height:1em}
#plotDialog{width:min(980px,95vw)}#plotDialog .sheetControls{display:flex;gap:16px;padding:6px 12px 0}#plotDialog .sheetControls label{display:flex;align-items:center;gap:6px;margin:0}#plotDialog .sheetControls select{width:auto!important;margin:0!important;display:inline-block}
#sheetPreview{margin:10px 12px!important;border:1px solid #12161c;background:#5a616d!important;padding:12px;max-height:58vh;overflow:auto}#sheetPreview svg{background:#fff;box-shadow:0 2px 10px rgba(0,0,0,.5);max-height:52vh}
#exportDialog{width:700px}.exportDownload{display:inline-block;margin:4px 12px;padding:5px 16px;border:1px solid;border-radius:2px;text-decoration:none}
.cf-tool{display:inline-flex;align-items:center;gap:5px;height:26px;padding:0 9px}.cf-tool-ico{display:inline-flex}
.cf-lpm{width:780px}.cf-lpm-bar{display:flex;align-items:center;gap:4px;margin-bottom:8px}.cf-lpm-gap{flex:1}.cf-lpm-cur{color:var(--cf-text-dim);margin-right:10px;white-space:nowrap}.cf-lpm-search{width:170px;height:24px}
.cf-lpm-grid{height:300px;overflow:auto;border:1px solid #12161c;background:#1f252d}
.cf-lpm-table{border-collapse:collapse;width:100%;table-layout:fixed}
.cf-lpm-table th{position:sticky;top:0;background:#343c48;color:var(--cf-text-dim);font-weight:600;text-align:left;padding:0 8px;height:24px;border-right:1px solid #262c35;border-bottom:1px solid #12161c;white-space:nowrap;z-index:1}
.cf-lpm-table th.sortable{cursor:pointer}.cf-lpm-table th.sortable:hover{color:var(--cf-text)}
.cf-lpm-table td{height:24px;padding:0 8px;border-right:1px solid #262c35;border-bottom:1px solid #262c35;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.cf-lpm-row{cursor:default}.cf-lpm-row:hover{background:#2b3440}.cf-lpm-row.sel{background:var(--cf-active)}.cf-lpm-row.cur td.c-name{font-weight:600}
.c-status{width:56px;text-align:center!important;color:var(--cf-ok);font-weight:700}.c-name{width:auto}.c-on{width:44px;text-align:center}.c-color{width:140px;cursor:pointer}.c-color span{margin-left:6px}.c-lt{width:110px;color:var(--cf-text-dim)}.c-obj{width:76px;text-align:right!important;color:var(--cf-text-dim)}
.cf-lpm-edit{display:block;margin:0;width:100%;height:20px;padding:0 4px;border:1px solid var(--cf-accent);background:var(--cf-input)}
.cf-lpm-msg{min-height:18px;padding-top:6px;color:#ffb088}
.cf-ds{width:520px}.cf-ds-tabs{display:flex;gap:2px;border-bottom:1px solid var(--cf-border)}.cf-ds-tab{border:1px solid transparent;border-bottom:0;border-radius:2px 2px 0 0;background:transparent;color:var(--cf-text-dim);padding:5px 12px}.cf-ds-tab.active{background:#343c48;border-color:var(--cf-border);color:#fff}
.cf-ds-pages{min-height:220px;padding:12px 4px 4px}.cf-ds-page{display:none}.cf-ds-page.active{display:block}
.cf-chk{display:flex;align-items:center;gap:7px;margin:4px 0;cursor:pointer}.cf-chk input{margin:0}.cf-osmark{display:inline-block;width:14px;text-align:center;color:${CF.colors.osnap||'#3ddc84'}}
.cf-group-title{margin:12px 0 6px;color:var(--cf-text-dim);border-bottom:1px solid #3a424e;padding-bottom:3px}
.cf-field{display:flex;align-items:center;gap:8px}.cf-field select{width:90px}
.cf-ds-osgrid{display:flex;gap:16px}.cf-ds-modes{display:grid;grid-template-columns:1fr 1fr;column-gap:16px;flex:1}.cf-ds-side{display:flex;flex-direction:column;gap:6px}.cf-ds-side button{min-width:84px;height:24px}
.cf-note{color:var(--cf-text-faint);margin:10px 0 0;line-height:1.45}
`;document.head.append(palStyle);

// ---- Wiring --------------------------------------------------------------------------------------
CF.openLayerManager=palOpenLayerManager;CF.createLayerDropdown=palCreateLayerDropdown;CF.openDraftingSettings=palOpenDraftingSettings;
try{palBuildFrame()}catch(err){console.error(err)}
try{palBuildStatus()}catch(err){console.error(err)}
try{$('meshA')?.addEventListener?.('change',()=>render())}catch{}
let palIconsSeen=palIcons(),palErrOnce=false;
function palAfterRender(){if(palIconsSeen!==palIcons()){palIconsSeen=palIcons();palSBIcons()}palUpdateCoords();palSyncStatus();palUpdate();palRefreshDropdowns();palLpmUpdate();palDrawMarker()}
{const prev=render;render=function(...a){const r=prev.apply(this,a);try{palAfterRender()}catch(err){if(!palErrOnce){palErrOnce=true;console.error(err)}}return r}}
CF.on('state',e=>{palStateRev++;palSyncStatus();if(e?.name==='properties'&&e.value||e?.name==='clean'&&!e.value)palUpdate(true);else palUpdate()});
CF.on('space',()=>{palSyncStatus(true);palUpdate()});CF.on('workspace',()=>{palSyncStatus(true);palUpdate()});
CF.on('selection',()=>palUpdate());CF.on('document',()=>{palStateRev++;palUpdate()});
CF.on('modified',()=>{palRev++;palUpdate();palRefreshDropdowns();palLpmUpdate()});
// Commands (registered only when the command module has not supplied them).
function palReg(def){if(CF.resolve(def.name))return;def.aliases=(def.aliases||[]).filter(a=>!CF.resolve(a));CF.register(def)}
palReg({name:'LAYER',aliases:['LA'],label:'Layer Properties',desc:'Manages layers and layer properties',icon:'layer-properties',category:'Layers',run:()=>CF.openLayerManager()});
palReg({name:'PROPERTIES',aliases:['PR','PROPS','CH','MO'],label:'Properties',desc:'Controls properties of existing objects',icon:'properties',category:'Palettes',run:()=>CF.toggle('properties',true)});
palReg({name:'PROPERTIESCLOSE',aliases:['PRCLOSE'],label:'Close Properties',desc:'Closes the Properties palette',icon:'properties',category:'Palettes',run:()=>CF.toggle('properties',false)});
palReg({name:'DSETTINGS',aliases:['DS','SE'],label:'Drafting Settings',desc:'Sets grid and snap, polar tracking, object snap modes and dynamic input',icon:'settings',category:'Settings',run:()=>CF.openDraftingSettings()});
palReg({name:'LAYMCUR',label:'Make Object\'s Layer Current',desc:'Sets the current layer to that of a selected object',icon:'layer',category:'Layers',run:()=>{const i=chosen()[0],e=doc.entities[i];if(!e){notify('Select an object first, then run LAYMCUR.');return}palSetCurrent(e.layer);notify(`${e.layer} is now the current layer.`)}});
palReg({name:'LAYON',label:'Turn All Layers On',desc:'Turns on all layers in the drawing',icon:'layer-on',category:'Layers',run:()=>{if(doc.layers.every(l=>l.visible)){notify('All layers are already on.');return}mutate(()=>doc.layers.forEach(l=>l.visible=true));syncLayers();notify('All layers have been turned on.')}});
CF.palettes={model:palModel,edit:palEdit,refresh:()=>palUpdate(true),objects:()=>palObjects(chosen()),
 filter:{get:()=>palFilter,set:v=>{palFilter=v;palUpdate(true)}},vertex:{get:()=>palVertex,set:v=>{palVertex=v;palUpdate(true)},step:palStepVertex},
 layers:{current:()=>$('layer').value,setCurrent:palSetCurrent,nextName:palNextLayerName,add:palAddLayer,rename:palRenameLayer,remove:palDeleteLayer,setColor:palSetLayerColor,setVisible:palSetLayerVisible,objects:palLayerObjects,checkName:palCheckName,colorName:palColorName},
 setOsnapMode:palSetOsnapMode,setPolarIncrement:palSetPolarIncrement,osnapModes:palOsnapAll,polarSteps:palPolarSteps,
 status:palSB,syncStatus:()=>palSyncStatus(true),menus:{polar:palPolarMenu,osnap:palOsnapMenu,workspace:palWorkspaceMenu,customize:palCustomizeMenu,close:palCloseMenus},
 layerManager:{open:palOpenLayerManager,render:()=>palLpmRender(true),dialog:()=>palLpm,select:palLpmSelect,newLayer:palLpmNew,deleteLayer:palLpmDelete,setCurrent:palLpmCurrent,visibleNames:palLpmVisibleNames},
 draftingSettings:{open:palOpenDraftingSettings,apply:()=>palDsApply(),parts:()=>palDsParts},dropdowns:palDropdowns};
palUpdate(true);
