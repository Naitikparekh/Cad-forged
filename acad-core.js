'use strict';
// CAD workspace foundation: command registry, drafting-aid state, shared hooks, geometry helpers and layout hosts.
// Later acad-* modules fill the hosts and replace the default hooks; the drafting engine above stays unchanged.
const CF=(()=>{
const listeners={},modules=new Set();
const flags={grid:true,osnap:true,polar:false,dyn:true,properties:false,clean:false,commandLine:true,ribbonMin:false,fileTabs:true,layoutTabs:true,ucsIcon:true,viewCube:true,navBar:true};
const toggleLabels={grid:'Grid',snap:'Snap',ortho:'Ortho',polar:'Polar',osnap:'Osnap',dyn:'Dynamic input'};
const api={
 version:'1.0',
 modules,
 module(name){modules.add(name)},
 has(name){return modules.has(name)},
 on(event,fn){(listeners[event]??=[]).push(fn);return fn},
 off(event,fn){listeners[event]=(listeners[event]||[]).filter(f=>f!==fn)},
 emit(event,data){for(const fn of [...(listeners[event]||[])]){try{fn(data)}catch(err){console.error(err)}}},
 // ---- Commands -------------------------------------------------------------
 commands:new Map(),aliases:new Map(),lastCommand:null,
 register(def){if(!def||!def.name||typeof def.run!=='function')throw Error('A command needs a name and run().');const name=String(def.name).toUpperCase(),entry={desc:'',category:'',...def,name,label:def.label||name[0]+name.slice(1).toLowerCase(),icon:def.icon||name.toLowerCase(),aliases:(def.aliases||[]).map(a=>String(a).toUpperCase())};const old=api.commands.get(name);if(old)for(const a of old.aliases)if(api.aliases.get(a)===name)api.aliases.delete(a);api.commands.set(name,entry);for(const a of entry.aliases)if(!api.commands.has(a)||a===name)api.aliases.set(a,name);return entry},
 resolve(text){const key=String(text??'').trim().toUpperCase().replace(/^[_.'-]+/,'');if(!key)return null;const name=api.commands.has(key)?key:api.aliases.get(key);return name?api.commands.get(name):null},
 run(text,options={}){const raw=String(text??'').trim();if(!raw)return false;const def=api.resolve(raw);if(!def){notify(`Unknown command "${raw.toUpperCase()}". Press F1 for a command list.`);return false}api.emit('command',{def,source:options.source||'command',text:raw});if(!def.noRepeat)api.lastCommand=def.name;try{const result=def.run(options.args);if(result&&typeof result.then==='function')result.catch(err=>notify(`${def.name} failed: ${err?.message||err}`))}catch(err){notify(`${def.name} failed: ${err?.message||err}`)}return true},
 // ---- Drafting aids and interface flags -------------------------------------
 // snap (grid snap) and ortho stay backed by the engine checkboxes #snap and #ortho.
 get(name){return name==='snap'||name==='ortho'?!!$(name).checked:!!flags[name]},
 set(name,value,options={}){value=!!value;if(name==='snap'||name==='ortho')$(name).checked=value;else flags[name]=value;if(value&&name==='ortho'&&flags.polar){flags.polar=false;api.emit('state',{name:'polar',value:false})}if(value&&name==='polar'&&$('ortho').checked){$('ortho').checked=false;api.emit('state',{name:'ortho',value:false})}if(toggleLabels[name]&&!options.quiet)notify(`<${toggleLabels[name]} ${value?'on':'off'}>`);api.emit('state',{name,value});render();return value},
 toggle(name,value,options){return api.set(name,value===undefined?!api.get(name):value,options)},
 // ---- Prompts, pending input and object picking ------------------------------
 prompts:{},      // tool -> string | () => string, e.g. prompts.fillet=()=>'FILLET Select first object:'
 previews:{},     // tool -> (mouse) => temporary entities drawn as the rubber-band preview
 pickTools:{select:true,delete:true,trim:true,extend:true}, // tool -> true | () => boolean: clicks pick objects (raw, unsnapped)
 input:null,      // pending command-line input: {message, defaultValue, kind:'text'|'number'|'angle'|'distance'|'factor'|'point', base, resolve(value|null)}
 picking:null,    // pending "Select objects:" phase: {command, message, done(), cancel()}
 isPick(){if(api.picking)return true;const v=api.pickTools[tool];return typeof v==='function'?!!v():!!v},
 prompt(){if(api.input)return api.input.message;if(api.picking)return api.picking.message||'Select objects:';const p=api.prompts[tool];if(p)return typeof p==='function'?p():p;return tool==='select'?'Command:':(names[tool]||tool).toUpperCase()},
 enter(){finish()},
 cancel(){clearSelection();setTool('select');render()},
 // ---- Selection ---------------------------------------------------------------
 selection(){return chosen()},
 select(ids,mode='replace'){const valid=[...ids].filter(i=>Number.isInteger(i)&&i>=0&&i<doc.entities.length);if(mode==='replace')selectionSet=new Set(valid);else if(mode==='remove'){const groups=new Set(valid.map(i=>doc.entities[i].group).filter(Boolean));for(const i of valid)selectionSet.delete(i);doc.entities.forEach((e,i)=>{if(e.group&&groups.has(e.group))selectionSet.delete(i)});if(valid.includes(selected)||groups.has(doc.entities[selected]?.group))selected=-1}else for(const i of valid)selectionSet.add(i);if(selected<0||!selectionSet.has(selected))selected=selectionSet.size?selectionSet.values().next().value:-1;api.emit('selection',chosen());render()},
 deselect(){selectionSet.clear();selected=-1;api.emit('selection',[]);render()},
 visible(e){return !!doc.layers.find(l=>l.name===e.layer)?.visible},
 // ---- Document / file state ---------------------------------------------------
 fileName:'Drawing1',drawingCount:1,modified:false,
 setFileName(name){api.fileName=String(name||'Drawing1').replace(/\.(cadforge\.json|json|dxf)$/i,'')||'Drawing1';api.emit('document',{name:api.fileName})},
 markSaved(){api.modified=false;api.emit('modified',false)},
 saveProject(name){const n=name||api.fileName||'Drawing1';download(n+'.cadforge.json',JSON.stringify(doc,null,2),'application/json');api.markSaved()},
 async saveAs(){const n=await cadPrompt('Save drawing as (file name)',api.fileName);if(!n?.trim())return;api.setFileName(n.trim());api.saveProject()},
 async newDrawing(){const before=doc;await $('new').onclick();if(doc!==before){api.drawingCount++;api.setFileName('Drawing'+api.drawingCount);api.markSaved()}},
 openFile(){$('file').click()},
 // ---- Spaces and workspaces ---------------------------------------------------
 space:'model',
 setSpace(space){if(space!=='model'&&space!=='layout')return;if(space==='layout'&&mode3D)showModel();api.space=space;api.emit('space',space);render()},
 workspace:'drafting', // 'drafting' (Drafting & Annotation) or '3d' (3D Modeling)
 setWorkspace(name){if(name!=='drafting'&&name!=='3d')return;api.workspace=name;api.emit('workspace',name)},
 // ---- Hooks replaced by later modules (defaults keep every command usable) -----
 openLayerManager(){notify('Layer Properties Manager is not available in this build.')},
 openContextMenu(){},
 setViewPreset(name){if(name==='top')showModel();else show3D()},
 setVisualStyle(){},
 startPan(){notify('Pan: drag with the middle mouse button; the wheel zooms.')},
 icon(name,size=20){return `<svg class="cf-icon" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><rect x="4" y="4" width="16" height="16" rx="2"/></svg>`},
 createLayerDropdown(){const wrap=document.createElement('div');wrap.className='cf-layer-fallback';wrap.append($('layer'));return wrap},
 // ---- Colours used on the canvas ---------------------------------------------
 colors:{canvas:'#212830',gridMinor:'#28303a',gridMajor:'#323c49',axisX:'#8f4747',axisY:'#47834f',crosshair:'#e6e6e6',preview:'#e6e6e6',selection:'#4aa3ff',grip:'#3c7fe0',gripHover:'#ff6f9a',gripHot:'#e23c3c',osnap:'#3ddc84',windowFill:'rgba(70,130,255,.16)',windowStroke:'#6aa6ff',crossingFill:'rgba(70,200,110,.16)',crossingStroke:'#62d38a',paper:'#ffffff',paperBackground:'#4b5260',tooltip:'#2e3540',tooltipText:'#e6edf5'},
 hosts:{},
 // ---- Geometry helpers --------------------------------------------------------
 geom:{
  eps:1e-9,
  dist(a,b){return Math.hypot(a.x-b.x,a.y-b.y)},
  angle(a,b){return Math.atan2(b.y-a.y,b.x-a.x)},
  polar(p,d,ang){return{x:p.x+d*Math.cos(ang),y:p.y+d*Math.sin(ang)}},
  mid(a,b){return{x:(a.x+b.x)/2,y:(a.y+b.y)/2}},
  segments(e){if(e.type==='line')return[[e.points[0],e.points[1]]];if(e.type==='polyline'){const s=[];for(let i=1;i<e.points.length;i++)s.push([e.points[i-1],e.points[i]]);if(e.closed&&e.points.length>2)s.push([e.points.at(-1),e.points[0]]);return s}return[]},
  bbox(e){let ps;if(e.type==='circle')ps=[{x:e.center.x-e.radius,y:e.center.y-e.radius},{x:e.center.x+e.radius,y:e.center.y+e.radius}];else if(e.type==='text'){const p=e.points[0];ps=[p,{x:p.x+String(e.text).length*e.height*.6,y:p.y+e.height}]}else ps=e.points;const xs=ps.map(p=>p.x),ys=ps.map(p=>p.y);return{minX:Math.min(...xs),minY:Math.min(...ys),maxX:Math.max(...xs),maxY:Math.max(...ys)}},
  closestOnSegment(p,a,b){const dx=b.x-a.x,dy=b.y-a.y,len2=dx*dx+dy*dy,t=len2?Math.max(0,Math.min(1,((p.x-a.x)*dx+(p.y-a.y)*dy)/len2)):0,q={x:a.x+t*dx,y:a.y+t*dy};return{x:q.x,y:q.y,t,d:Math.hypot(p.x-q.x,p.y-q.y)}},
  // Intersection of segments ab and cd; t along ab, u along cd. infinite:true treats both as lines.
  segmentIntersection(a,b,c,d,infinite=false){const rx=b.x-a.x,ry=b.y-a.y,sx=d.x-c.x,sy=d.y-c.y,den=rx*sy-ry*sx;if(Math.abs(den)<1e-12)return null;const t=((c.x-a.x)*sy-(c.y-a.y)*sx)/den,u=((c.x-a.x)*ry-(c.y-a.y)*rx)/den,e=1e-9;if(!infinite&&(t<-e||t>1+e||u<-e||u>1+e))return null;return{x:a.x+t*rx,y:a.y+t*ry,t,u}},
  // Intersections of segment ab with circle (c, r); t along ab.
  lineCircle(a,b,c,r,infinite=false){const dx=b.x-a.x,dy=b.y-a.y,fx=a.x-c.x,fy=a.y-c.y,A=dx*dx+dy*dy,B=2*(fx*dx+fy*dy),C=fx*fx+fy*fy-r*r,disc=B*B-4*A*C;if(A<1e-18||disc<0)return[];const s=Math.sqrt(disc),ts=disc===0?[-B/(2*A)]:[(-B-s)/(2*A),(-B+s)/(2*A)];return ts.filter(t=>infinite||(t>=-1e-9&&t<=1+1e-9)).map(t=>({x:a.x+t*dx,y:a.y+t*dy,t}))},
  circleCircle(c1,r1,c2,r2){const d=Math.hypot(c2.x-c1.x,c2.y-c1.y);if(d<1e-12||d>r1+r2+1e-9||d<Math.abs(r1-r2)-1e-9)return[];const a=(r1*r1-r2*r2+d*d)/(2*d),h=Math.sqrt(Math.max(0,r1*r1-a*a)),mx=c1.x+a*(c2.x-c1.x)/d,my=c1.y+a*(c2.y-c1.y)/d,ox=-(c2.y-c1.y)/d*h,oy=(c2.x-c1.x)/d*h;return h<1e-12?[{x:mx,y:my}]:[{x:mx+ox,y:my+oy},{x:mx-ox,y:my-oy}]},
  // All intersection points between two line/polyline/circle entities (text has none).
  intersections(e1,e2){const g=api.geom,out=[];if(e1.type==='text'||e2.type==='text')return out;if(e1.type==='circle'&&e2.type==='circle')return g.circleCircle(e1.center,e1.radius,e2.center,e2.radius);if(e1.type==='circle'||e2.type==='circle'){const c=e1.type==='circle'?e1:e2,o=c===e1?e2:e1;for(const [a,b]of g.segments(o))out.push(...g.lineCircle(a,b,c.center,c.radius).map(p=>({x:p.x,y:p.y})));return out}for(const [a,b]of g.segments(e1))for(const [c,d]of g.segments(e2)){const p=g.segmentIntersection(a,b,c,d);if(p)out.push({x:p.x,y:p.y})}return out}
 }
};
return api})();
CF.module('core');
// Drafting defaults: grid snap off, object snap on (the engine's #snap checkbox now means grid snap only).
$('snap').checked=false;$('ortho').checked=false;
// ---- Layout hosts -------------------------------------------------------------------
{const host=(id,tag='div')=>{const e=document.createElement(tag);e.id=id;CF.hosts[id.replace(/^cf-/,'').replace(/-(\w)/g,(_,c)=>c.toUpperCase())]=e;return e};
 const titlebar=host('cf-titlebar'),ribbonTabs=host('cf-ribbon-tabs'),ribbonHost=host('cf-ribbon'),fileTabs=host('cf-filetabs'),palette=host('cf-palette','aside'),cmdline=host('cf-cmdline'),statusbar=host('cf-statusbar'),overlay=host('cf-overlay');
 document.body.prepend(titlebar,ribbonTabs,ribbonHost,fileTabs);
 document.querySelector('main').append(palette);
 document.querySelector('main').after(cmdline,statusbar);
 $('viewport').append(overlay);}
// ---- Theme tokens and frame layout ------------------------------------------------------
const cfStyle=document.createElement('style');cfStyle.id='cf-core-style';cfStyle.textContent=`
:root{--cf-font:'Segoe UI',system-ui,-apple-system,Arial,sans-serif;--cf-mono:Consolas,'Cascadia Mono','Courier New',monospace;--cf-frame:#1b1f26;--cf-titlebar:#1b1f26;--cf-tabbar:#232932;--cf-ribbon:#343c48;--cf-ribbon-title:#2b323c;--cf-panel:#2b323c;--cf-panel-header:#343c48;--cf-input:#1f252d;--cf-border:#4a5260;--cf-border-soft:#3a424e;--cf-text:#e3e8ee;--cf-text-dim:#a9b3bf;--cf-text-faint:#7d8896;--cf-hover:#45515f;--cf-hover-border:#5d7088;--cf-active:#2f5d8a;--cf-active-border:#4a90d9;--cf-accent:#4a90d9;--cf-accent-2:#e8b84f;--cf-canvas:#212830;--cf-danger:#e05a5a;--cf-ok:#5cc87a;--cf-shadow:0 8px 30px rgba(0,0,0,.45)}
html,body{background:var(--cf-frame);color:var(--cf-text);font:12px/1.35 var(--cf-font)}
body{height:100vh;margin:0;display:flex;flex-direction:column;overflow:hidden}
header,.tabs,#ribbon,.drawingtab,main>aside:not(#cf-palette),#properties,#historylog,footer,.statusbar,#hint,.viewbadge{display:none!important}
#cf-titlebar,#cf-ribbon-tabs,#cf-ribbon,#cf-filetabs,#cf-cmdline,#cf-statusbar{flex-shrink:0;position:relative}
main{display:flex;flex:1;min-height:120px;height:auto!important}
#viewport{flex:1;min-width:0;position:relative;overflow:hidden;background:var(--cf-canvas)}
#cf-overlay{position:absolute;inset:0;pointer-events:none;z-index:3}
#cf-palette{display:none;flex-direction:column;width:270px;flex-shrink:0;background:var(--cf-panel);border-left:1px solid #12161c;overflow:hidden}
body.cf-palette-open #cf-palette{display:flex}
body.cf-clean #cf-ribbon-tabs,body.cf-clean #cf-ribbon,body.cf-clean #cf-filetabs,body.cf-clean #cf-palette{display:none!important}
body.cf-ribbon-min #cf-ribbon{display:none}
body.cf-no-cmdline #cf-cmdline{display:none}
body.cf-no-filetabs #cf-filetabs{display:none}
button,input,select,textarea{font:inherit;color:var(--cf-text);background:var(--cf-input);border:1px solid var(--cf-border);border-radius:2px;padding:4px 8px}
button{background:#3a4350;cursor:pointer}button:hover{background:var(--cf-hover);border-color:var(--cf-hover-border)}button.active{background:var(--cf-active);border-color:var(--cf-active-border)}
button:focus-visible,input:focus-visible,select:focus-visible{outline:1px solid var(--cf-accent);outline-offset:1px}
dialog{background:var(--cf-panel);color:var(--cf-text);border:1px solid var(--cf-border);border-radius:3px;box-shadow:var(--cf-shadow)}dialog::backdrop{background:rgba(0,0,0,.45)}
`;document.head.append(cfStyle);
// Body classes mirror the interface flags so modules only need CF.toggle().
function cfSyncBodyFlags(){const b=document.body.classList;b.toggle('cf-palette-open',CF.get('properties'));b.toggle('cf-clean',CF.get('clean'));b.toggle('cf-ribbon-min',CF.get('ribbonMin'));b.toggle('cf-no-cmdline',!CF.get('commandLine'));b.toggle('cf-no-filetabs',!CF.get('fileTabs'))}
CF.on('state',cfSyncBodyFlags);cfSyncBodyFlags();
// ---- Engine integration ------------------------------------------------------------------
{const cfSetTool=setTool;setTool=function(t){if(CF.space==='layout'&&t!=='select'){CF.space='model';CF.emit('space','model')}cfSetTool(t);CF.emit('tool',t)}}
{const cfMutate=mutate;mutate=function(fn){const result=cfMutate(fn);CF.modified=true;CF.emit('modified',true);return result}}
{const cfUndo=undo;undo=function(redo=false){cfUndo(redo);CF.modified=true;CF.emit('modified',true);CF.emit('selection',chosen())}}
{const cfOpen=$('file').onchange;$('file').onchange=async e=>{const f=e.target.files?.[0],before=doc;await cfOpen(e);if(f&&doc!==before){CF.setFileName(f.name);CF.markSaved()}}}
$('save').onclick=()=>CF.saveProject();
