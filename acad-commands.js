'use strict';
// B1: command registry, docked command line (history, prompt, AutoComplete), cadPrompt routing, object picking, ZOOM, clipboard, keyboard map.
(()=>{
CF.module('commands');
const G=CF.geom,ORIGIN={x:0,y:0},DEG=Math.PI/180;
const S={active:null,hold:0,lock:0,lastPoint:null,quiet:0,mutations:0,typed:false,chain:{pts:[]},copy:{base:null,count:0,sel:[]},textHeight:5,lastRadius:0,rectL:10,rectW:10,clip:null,prevSel:[],
 views:[],lastView:null,viewAt:0,zoom:{p1:null},hist:[],recall:-1,ac:[],acIndex:-1,expanded:false,shown:null};
const lines=CF.history=[];
const fnum=n=>String(Number((+n).toFixed(4))),fpt=p=>`${fnum(p.x)},${fnum(p.y)}`,rnd=v=>{const r=Math.round(v*1e9)/1e9;return Object.is(r,-0)?0:r};
const host=CF.hosts.cmdline,input=$('command');
// ---- Tool metadata ---------------------------------------------------------------------------------------------------
names.zoom='Zoom';names.paste='Paste';names.dimension??='Dimension';
const TOOL_LABEL={line:'LINE',polyline:'PLINE',circle:'CIRCLE',arc:'ARC',rectangle:'RECTANG',text:'TEXT',move:'MOVE',copy:'COPY',rotate:'ROTATE',scale:'SCALE',mirror:'MIRROR',offset:'OFFSET',trim:'TRIM',extend:'EXTEND',delete:'ERASE',dimension:'DIMALIGNED',window:'SELECT',insert:'INSERT',zoom:'ZOOM',paste:'PASTECLIP'};
const LABEL_TOOL={DIMLINEAR:'dimension',DIMALIGNED:'dimension',MTEXT:'text'};
const ONE_SHOT=new Set(['circle','rectangle','arc','move','rotate','scale','mirror','dimension','insert']),MODIFY_TOOLS=new Set(['move','rotate','scale','mirror']);
const ECHO_TOOLS=new Set(['line','polyline','circle','arc','rectangle','text','move','copy','rotate','scale','mirror','dimension','insert','paste']);
const labelFor=t=>S.active&&(TOOL_LABEL[t]===S.active||LABEL_TOOL[S.active]===t)?S.active:TOOL_LABEL[t]||String(names[t]||t).toUpperCase();
const curLabel=()=>tool!=='select'?labelFor(tool):S.active;
const ENGINE_PROMPTS={
 line:()=>!points.length?'Specify first point:':S.chain.pts.length>=3?'Specify next point or [Close/Undo]:':'Specify next point or [Undo]:',
 polyline:()=>!points.length?'Specify start point:':'Specify next point or [Close/Undo]:',
 circle:()=>!points.length?'Specify center point for circle:':`Specify radius of circle or [Diameter]${S.lastRadius>0?` <${fnum(S.lastRadius)}>`:''}:`,
 arc:()=>['Specify center point of arc:','Specify start point of arc:','Specify end point of arc:'][Math.min(points.length,2)],
 rectangle:()=>!points.length?'Specify first corner point:':'Specify other corner point or [Dimensions]:',
 text:()=>'Specify start point of text:',
 move:()=>!chosen().length?'Select objects:':!points.length?'Specify base point:':'Specify second point:',
 copy:()=>!chosen().length?'Select objects:':!points.length?'Specify base point:':'Specify second point or [Exit/Undo] <Exit>:',
 rotate:()=>!chosen().length?'Select objects:':'Specify base point:',scale:()=>!chosen().length?'Select objects:':'Specify base point:',
 mirror:()=>!chosen().length?'Select objects:':!points.length?'Specify first point of mirror line:':'Specify second point of mirror line:',
 offset:()=>selected<0?'Select object to offset:':'Specify point on side to offset:',trim:()=>'Select object to trim:',extend:()=>'Select object to extend:',delete:()=>'Select objects:',
 dimension:()=>['Specify first extension line origin:','Specify second extension line origin:','Specify dimension line location:'][Math.min(points.length,2)],
 window:()=>!points.length?'Specify first corner:':'Specify opposite corner:',insert:()=>'Specify insertion point:',paste:()=>'Specify insertion point:',
 zoom:()=>'Specify window corner, enter a scale factor (nX), or [All/Extents/Previous/Window/In/Out] <Extents>:'};
// ---- Parsing helpers -----------------------------------------------------------------------------------------------
const optKey=w=>{const c=String(w).match(/[A-Z]/g);return c?c.join(''):String(w)};
const optionsOf=p=>{const m=String(p).match(/\[([^\]]+)\]/);return m?m[1].split('/').map(s=>s.trim()).filter(Boolean):[]};
function matchOpt(text,list){const t=String(text??'').trim().toUpperCase();if(!t)return null;return list.find(w=>optKey(w).toUpperCase()===t)||list.find(w=>w.toUpperCase().startsWith(t))||null}
const NUM='[+-]?(?:\\d+\\.?\\d*|\\.\\d+)(?:e[+-]?\\d+)?';
const RE={abs:new RegExp(`^#?(${NUM})\\s*,\\s*(${NUM})(?:\\s*,\\s*(${NUM}))?$`,'i'),rel:new RegExp(`^@\\s*(${NUM})\\s*,\\s*(${NUM})(?:\\s*,\\s*(${NUM}))?$`,'i'),polar:new RegExp(`^(@?)\\s*(${NUM})\\s*<\\s*(${NUM})$`,'i'),num:new RegExp(`^(${NUM})$`,'i'),zoomNum:/^([+-]?(?:\d+\.?\d*|\.\d+))(x|xp)?$/i};
// Point text: x,y | @x,y | @d<a | d<a | @ (last point) | bare distance along the direction base -> dir (direct distance entry).
function parsePointText(text,base,dir){
 const t=String(text??'').trim();if(!t)return null;const from=base||S.lastPoint||ORIGIN;let m;
 if(t==='@')return{x:from.x,y:from.y};
 if(m=t.match(RE.rel))return{x:rnd(from.x+ +m[1]),y:rnd(from.y+ +m[2])};
 if(m=t.match(RE.abs))return{x:+m[1],y:+m[2]};
 if(m=t.match(RE.polar)){const o=m[1]?from:ORIGIN,a=+m[3]*DEG;return{x:rnd(o.x+ +m[2]*Math.cos(a)),y:rnd(o.y+ +m[2]*Math.sin(a))}}
 if(base&&(m=t.match(RE.num))){const d=+m[1];let a=0;if(dir&&Math.hypot(dir.x-base.x,dir.y-base.y)>1e-9)a=Math.atan2(dir.y-base.y,dir.x-base.x);return{x:rnd(base.x+d*Math.cos(a)),y:rnd(base.y+d*Math.sin(a))}}
 return null}
// ---- History area, prompt and AutoComplete popup ---------------------------------------------------------------------
let histEl=null,promptEl=null,rowEl=null,acEl=null;
const mk=(tag,cls)=>{const e=document.createElement(tag);if(cls)e.className=cls;return e};
function print(text,cls){
 for(const l of String(text??'').split(/\r?\n/)){
  lines.push(l);if(lines.length>3000)lines.splice(0,1000);
  if(!histEl)continue;
  const d=mk('div',cls||(/^Command:/.test(l)?'cf-cmd-in':/^\*Cancel\*|^Unknown command|failed|^Invalid|^Requires|^Point or option/i.test(l)?'cf-cmd-err':''));d.textContent=l||'\u00a0';histEl.append(d);
  const kids=histEl.children;if(kids&&kids.length>600){try{kids[0].remove()}catch(err){}}}
 if(histEl){try{histEl.scrollTop=histEl.scrollHeight}catch(err){}}}
function renderPrompt(text){
 if(!promptEl)return;const literal=!!CF.input?.literal,nodes=[];
 for(const part of String(text).split(/(\[[^\]]*\])/)){
  if(part.length>1&&part[0]==='['&&part.at(-1)===']'){
   nodes.push('[');
   part.slice(1,-1).split('/').forEach((w,i)=>{if(i)nodes.push('/');const s=mk('span','cf-cmd-opt'),key=literal?w:optKey(w);s.textContent=w;s.title='Enter '+key;s.onmousedown=e=>e?.preventDefault?.();s.onclick=()=>{submit(key);try{input.focus()}catch(err){}};nodes.push(s)});
   nodes.push(']')}
  else if(part)nodes.push(part)}
 promptEl.replaceChildren(...nodes)}
function syncPrompt(){
 if(!S.hold&&tool==='select'&&!CF.input&&!CF.picking)S.active=null;
 let p;try{p=String(CF.prompt())}catch(err){p='Command:'}
 const key=p+'|'+(CF.input?.literal?1:0);if(key!==S.shown){S.shown=key;renderPrompt(p)}}
function expand(on){S.expanded=on===undefined?!S.expanded:!!on;try{document.body.classList.toggle('cf-cmd-expanded',S.expanded)}catch(err){}
 if(histEl){try{histEl.scrollTop=histEl.scrollHeight}catch(err){}}return S.expanded}
// AutoComplete: prefix matches on names and aliases first, then contains-match on name/label/description.
function acMatches(text){
 const q=String(text??'').trim().toUpperCase().replace(/^[_.'-]+/,'');if(!q)return[];const out=[];
 for(const def of CF.commands.values()){
  let score=99,via='';
  if(def.name===q)score=0;else if(def.aliases.includes(q)){score=1;via=q}else if(def.name.startsWith(q))score=2;
  else{const a=def.aliases.find(a=>a.startsWith(q));if(a){score=3;via=a}else if(def.name.includes(q))score=4;else if(`${def.label} ${def.desc}`.toUpperCase().includes(q))score=5}
  if(score<99)out.push({def,score,via})}
 out.sort((a,b)=>a.score-b.score||a.def.name.length-b.def.name.length||(a.def.name<b.def.name?-1:1));
 return out.slice(0,10).map(({def,via})=>({name:def.name,alias:via||def.aliases[0]||'',desc:def.desc||def.label,icon:def.icon}))}
function hideAc(){S.ac=[];S.acIndex=-1;if(acEl){acEl.style.display='none'}}
function showAc(items){
 if(!acEl)return;acEl.replaceChildren();
 items.forEach((it,i)=>{const r=mk('div','cf-ac-row'+(i===S.acIndex?' sel':'')),ic=mk('span','cf-ac-ico'),nm=mk('span','cf-ac-name'),al=mk('span','cf-ac-alias'),ds=mk('span','cf-ac-desc');
  try{ic.innerHTML=CF.icon(it.icon,16)}catch(err){}nm.textContent=it.name;al.textContent=it.alias;ds.textContent=it.desc;r.append(ic,nm,al,ds);
  r.onmousedown=e=>{e?.preventDefault?.();chooseAc(i,true)};acEl.append(r)});
 acEl.style.display='block'}
function moveAc(d){if(!S.ac.length)return;S.acIndex=(S.acIndex+d+S.ac.length+(d<0&&S.acIndex<0?1:0))%S.ac.length;showAc(S.ac)}
function chooseAc(i,run){const it=S.ac[i];if(!it)return;hideAc();if(run){input.value='';submit(it.name)}else{input.value=it.name;try{input.focus()}catch(err){}}}
const isIdle=()=>!CF.input&&!CF.picking&&tool==='select';
function onInput(){S.recall=-1;const v=input.value;if(!isIdle()||!v.trim()){hideAc();return}S.ac=acMatches(v);S.acIndex=-1;S.ac.length?showAc(S.ac):hideAc()}
function remember(t){if(t&&S.hist.at(-1)!==t){S.hist.push(t);if(S.hist.length>100)S.hist.shift()}}
function recall(dir){
 if(!S.hist.length)return;
 if(dir<0)S.recall=S.recall<0?S.hist.length-1:Math.max(0,S.recall-1);
 else{if(S.recall<0)return;S.recall++;if(S.recall>=S.hist.length){S.recall=-1;input.value='';return}}
 input.value=S.hist[S.recall]}
function buildUI(){
 const css=document.createElement('style');css.id='cf-cmd-style';css.textContent=`
#cf-cmdline{background:#262c35;border-top:1px solid #12161c;font:12px/16px var(--cf-mono);color:var(--cf-text)}
.cf-cmd-hist{height:52px;overflow-y:auto;overflow-x:hidden;padding:3px 30px 2px 10px;color:var(--cf-text-dim);white-space:pre-wrap;word-break:break-word;scrollbar-width:thin;scrollbar-color:#4a5260 transparent}
body.cf-cmd-expanded .cf-cmd-hist{height:208px}
.cf-cmd-hist div{min-height:16px}.cf-cmd-hist .cf-cmd-in{color:var(--cf-text)}.cf-cmd-hist .cf-cmd-err{color:#e8a0a0}
.cf-cmd-toggle{position:absolute;top:2px;right:6px;width:20px;height:16px;padding:0;line-height:12px;font-size:10px;background:transparent;border:1px solid transparent;color:var(--cf-text-faint)}
.cf-cmd-toggle:hover{background:var(--cf-hover);border-color:var(--cf-hover-border);color:var(--cf-text)}
.cf-cmd-row{display:flex;align-items:center;gap:6px;min-height:26px;padding:0 10px;border-top:1px solid var(--cf-border-soft);background:#222831}
.cf-cmd-row:focus-within{background:#1d232b}
.cf-cmd-glyph{display:flex;flex:none;color:var(--cf-accent)}
.cf-cmd-prompt{flex:0 1 auto;max-width:72%;color:#dfe6ee;white-space:pre-wrap;word-break:break-word}
.cf-cmd-opt{color:#79bbff;cursor:pointer}.cf-cmd-opt:hover{color:#fff;text-decoration:underline}
#command{flex:1 1 120px;min-width:100px;height:22px;padding:0 2px;background:transparent;border:0;border-radius:0;outline:0;font:12px var(--cf-mono);color:#fff;caret-color:#fff}
#command:focus-visible{outline:0}
.cf-cmd-ac{display:none;position:absolute;left:10px;bottom:100%;width:min(560px,calc(100% - 20px));background:var(--cf-panel);border:1px solid var(--cf-border);box-shadow:var(--cf-shadow);z-index:80;font:12px var(--cf-font)}
.cf-ac-row{display:flex;align-items:center;gap:10px;padding:3px 10px;cursor:pointer;color:var(--cf-text)}.cf-ac-row.sel,.cf-ac-row:hover{background:var(--cf-active)}
.cf-ac-ico{display:flex;width:16px;flex:none;color:#dfe5ec}.cf-ac-name{font:600 12px var(--cf-mono);min-width:120px}.cf-ac-alias{font:11px var(--cf-mono);color:var(--cf-text-faint);min-width:52px}
.cf-ac-desc{color:var(--cf-text-dim);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}`;
 document.head.append(css);
 if(!host)return;
 histEl=mk('div','cf-cmd-hist');promptEl=mk('span','cf-cmd-prompt');rowEl=mk('div','cf-cmd-row');acEl=mk('div','cf-cmd-ac');
 const glyph=mk('span','cf-cmd-glyph'),toggle=mk('button','cf-cmd-toggle');
 glyph.innerHTML='<svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M2.5 3.5 7 7l-4.5 3.5"/><path d="M8.5 11.5h4"/></svg>';
 toggle.textContent='\u25B4';toggle.title='Command history (F2)';toggle.onmousedown=e=>e?.preventDefault?.();toggle.onclick=()=>{expand()};
 rowEl.append(glyph,promptEl,input);host.append(histEl,toggle,rowEl,acEl);
 rowEl.onmousedown=e=>{if(e&&e.target===rowEl){e.preventDefault?.();try{input.focus()}catch(err){}}};
 input.placeholder='';input.setAttribute('autocomplete','off');input.setAttribute('spellcheck','false');input.setAttribute('aria-label','Command line');input.value='';
 input.oninput=onInput;input.onblur=()=>setTimeout(hideAc,150)}
// ---- Pending input: cadPrompt routed into the command line ----------------------------------------------------------
// spec {message, def, kind, base, literal, adapt}. Resolves a string, or null when cancelled. CF.input follows the contract.
function askLine(spec){
 return new Promise(resolve=>{
  if(CF.input){const old=CF.input;CF.input=null;try{old.resolve(null,true)}catch(err){}}
  let done=false;
  const inp={message:spec.message,defaultValue:spec.def==null?'':String(spec.def),kind:spec.kind||'text',base:spec.base||null,literal:!!spec.literal,
   resolve(value,quiet){
    if(done)return;done=true;if(CF.input===inp)CF.input=null;let out=null;
    if(value!==null&&value!==undefined){out=String(value);if(!quiet)print(`${inp.message} ${out}`.trimEnd());if(out.trim()==='')out=inp.defaultValue}
    let next=out;if(out!==null&&spec.adapt){try{next=spec.adapt(out,spec)}catch(err){next=null}}
    Promise.resolve(next).then(resolve,()=>resolve(null));syncPrompt();try{render()}catch(err){}}};
  CF.input=inp;syncPrompt();try{render()}catch(err){}})}
async function askNum(message,def,o={}){
 for(;;){const raw=await askLine({message,def:def==null?'':String(def),kind:o.kind||'distance',base:o.base});if(raw===null)return null;
  const v=Number(raw);if(raw.trim()!==''&&Number.isFinite(v)&&(!o.positive||v>0))return v;notify(o.positive?'Value must be positive and nonzero.':'Requires a numeric value.')}}
// Rotate/Scale "Copy": the engine would transform the originals, so the transformed copies are added here and the engine step is cancelled.
function copyTransformed(kind,value){
 const base=points[0]||ORIGIN,v=Number(value);
 if(!Number.isFinite(v)||(kind==='factor'&&v<=0)){notify(kind==='factor'?'Positive factor required.':'Requires a numeric angle.');return}
 const ids=chosen(),groups={},t=v*DEG,c=Math.cos(t),s=Math.sin(t);
 const fn=kind==='angle'?q=>({x:base.x+(q.x-base.x)*c-(q.y-base.y)*s,y:base.y+(q.x-base.x)*s+(q.y-base.y)*c}):q=>({x:base.x+(q.x-base.x)*v,y:base.y+(q.y-base.y)*v});
 const clones=ids.map(i=>{const e=structuredClone(doc.entities[i]);delete e.uuid;if(e.group)e.group=groups[e.group]??=gid();transformEntity(e,fn);if(kind==='factor'){if(e.type==='circle')e.radius*=v;if(e.type==='text')e.height*=v}return e});
 mutate(()=>doc.entities.push(...clones))}
// ROTATE / SCALE answers: Copy keeps the originals, Reference asks for two values.
const transformAdapt=kind=>async(v,spec)=>{
 let value=v,copy=false;const lab=curLabel()||(kind==='angle'?'ROTATE':'SCALE');
 for(;;){
  const opt=matchOpt(value,['Copy','Reference']);
  if(opt==='Copy'&&!copy){copy=true;print(kind==='angle'?'Rotating a copy of the selected objects.':'Scaling a copy of the selected objects.');const r=await askLine({...spec,adapt:null});if(r===null)return null;value=r;continue}
  if(opt==='Reference'){
   if(kind==='angle'){const ref=await askNum(`${lab} Specify the reference angle <0>:`,0,{kind:'angle',base:spec.base});if(ref===null)return null;const nw=await askNum(`${lab} Specify the new angle:`,null,{kind:'angle',base:spec.base});if(nw===null)return null;value=String(nw-ref)}
   else{const ref=await askNum(`${lab} Specify reference length <1>:`,1,{kind:'distance',base:spec.base,positive:true});if(ref===null)return null;const nw=await askNum(`${lab} Specify new length:`,null,{kind:'distance',base:spec.base,positive:true});if(nw===null)return null;value=String(nw/ref)}}
  break}
 if(copy){copyTransformed(kind,value);return null}
 return value};
const rotateAdapt=transformAdapt('angle'),scaleAdapt=transformAdapt('factor');
const csv=s=>s.split(/,\s*/).join('/');
const PROMPT_MAP=[
 [/^Text$/,()=>({stem:'Enter text',kind:'text'})],
 [/^Rotation angle in degrees$/,()=>({stem:'Specify rotation angle or [Copy/Reference]',kind:'angle',adapt:rotateAdapt})],
 [/^Scale factor$/,()=>({stem:'Specify scale factor or [Copy/Reference]',kind:'factor',adapt:scaleAdapt})],
 [/^Hatch spacing$/,()=>({stem:'Specify hatch spacing',kind:'distance'})],
 [/^Block name$/,()=>({stem:'Enter block name',kind:'text'})],
 [/^Block name: (.*)$/,m=>({stem:`Enter block name to insert [${csv(m[1])}]`,kind:'text',literal:true})],
 [/^Layer name$/,()=>({stem:'Enter name for new layer',kind:'text'})],
 [/^Layout name$/,()=>({stem:'Enter view name to save',kind:'text'})],
 [/^Layout: (.*)$/,m=>({stem:`Enter view name to restore [${csv(m[1])}]`,kind:'text',literal:true})],
 [/^Box width, depth, height$/,()=>({stem:'Specify box width, depth, height',kind:'text'})],
 [/^Mesh displacement dx,dy,dz$/,()=>({stem:'Specify displacement dx,dy,dz',kind:'text'})],
 [/^Offset distance/,()=>({stem:'Specify offset distance',kind:'distance'})],
 [/^Equal chamfer distance$/,()=>({stem:'Specify chamfer distance',kind:'distance'})],
 [/^Extrusion height$/,()=>({stem:'Specify height of extrusion',kind:'distance'})],
 [/^Paper size/,()=>({stem:'Enter paper size [A4/A3/LETTER]',kind:'text',literal:true})],
 [/^Save drawing as/,()=>({stem:'Enter file name',kind:'text'})]];
// Engine message -> AutoCAD wording, <default>, input kind and base point.
function mapPrompt(msg,initial,o){
 let def=o.defaultValue!=null?String(o.defaultValue):initial,kind=o.kind,base=o.base,stem=msg,adapt=null,literal=false,final=/[:?]\s*$/.test(msg);
 for(const [re,fn] of PROMPT_MAP){const m=msg.match(re);if(m){const r=fn(m);stem=r.stem;kind=kind||r.kind;adapt=r.adapt||null;literal=!!r.literal;final=false;break}}
 kind=kind||'text';if(!base&&(kind==='angle'||kind==='factor')&&points.length)base=points[0];
 let text=stem;if(!final)text=`${stem}${def!==''&&!/<[^>]*>/.test(stem)?` <${def}>`:''}:`;
 const pre=curLabel();if(pre&&!text.startsWith(pre+' '))text=`${pre} ${text}`;
 return{message:text,def,kind,base,literal,adapt}}
const modalPrompt=cadPrompt;
function cmdPrompt(message,initial='',opts){return askLine(mapPrompt(String(message??''),initial==null?'':String(initial),opts||{}))}
cadPrompt=cmdPrompt;
cadConfirm=function(message){const result=modalPrompt(message);$('dialogValue').hidden=true;$('dialogOk').focus();return result.then(v=>v!==null)};
// ---- Command lifecycle ----------------------------------------------------------------------------------------------
function abortPending(){
 if(CF.input){const i=CF.input;CF.input=null;try{i.resolve(null,true)}catch(err){}}
 if(CF.picking){const p=CF.picking;CF.picking=null;try{p.cancel?.()}catch(err){}}}
function endCmd(clearSel){
 S.active=null;CF.picking=null;
 if(clearSel){S.prevSel=chosen();selectionSet.clear();selected=-1;CF.emit('selection',[])}
 if(tool!=='select')setTool('select');else points=[];
 syncPrompt();render()}
function begin(label,toolName){abortPending();S.active=label;setTool(toolName);print(CF.prompt());render()}
// Keeps the running command's name for prompts while an async command waits between its questions.
async function hold(label,fn){S.hold++;S.active=label;try{return await fn()}finally{S.hold--}}
// "Select objects:" phase; Enter / Space / right-click calls done().
function pick(label,then){
 abortPending();if(tool!=='select')setTool('select');S.active=label;
 const pk={command:label,message:`${label} Select objects:`,done(){if(CF.picking!==pk)return;CF.picking=null;then()},cancel(){if(CF.picking===pk)CF.picking=null}};
 CF.picking=pk;print(pk.message);syncPrompt();render();return pk}
// Modify command: use the current selection (noun-verb) or ask for objects first; then start an engine tool or run an action.
function modify(label,toolName,action,opts={}){
 abortPending();S.active=label;
 const go=()=>{
  const ids=chosen();if(!ids.length){endCmd();return}
  if(selected<0||!ids.includes(selected))selected=ids[0];
  if(toolName){setTool(toolName);print(CF.prompt());render();return}
  return hold(label,()=>action(ids)).catch(err=>notify(`${label} failed: ${err?.message||err}`)).finally(()=>{if(S.active===label)endCmd(!opts.keep)})};
 return chosen().length?go():void pick(label,go)}
function eraseIds(ids){const set=new Set(ids);clearSelection();mutate(()=>{doc.entities=doc.entities.filter((e,i)=>!set.has(i))});CF.emit('selection',[]);render();print(`${set.size} object${set.size===1?'':'s'} erased.`)}
// ---- Tool behaviour: LINE chaining, COPY multiple, TEXT height, CIRCLE diameter, one-shot commands ---------------------
const prevAccept=accept;
function track(fn){
 const t0=tool,m0=S.mutations,post=()=>{if(tool===t0&&ONE_SHOT.has(t0)&&S.mutations!==m0&&!points.length&&!CF.input)endCmd(MODIFY_TOOLS.has(t0))};
 const r=fn();post();return r&&typeof r.then==='function'?r.then(v=>{post();return v}):r}
function acceptLine(p,rest){
 const first=!points.length,prev=points[0],m0=S.mutations,r=prevAccept(p,...rest);
 if(first){S.chain.pts=[{x:p.x,y:p.y}];return r}
 if(S.mutations!==m0){S.chain.pts.push({x:p.x,y:p.y});points=[{x:p.x,y:p.y}]}else points=[prev];
 render();return r}
function acceptCopy(p,rest){
 if(!chosen().length||selected<0)return prevAccept(p,...rest);
 const had=points.length,m0=S.mutations;if(!had)S.copy={base:{x:p.x,y:p.y},count:0,sel:chosen()};
 const base=S.copy.base,r=prevAccept(p,...rest);
 if(had&&S.mutations!==m0){points=[{...base}];S.copy.count++;render()}
 return r}
function acceptCircle(p,rest){const c=points[0],m0=S.mutations,r=track(()=>prevAccept(p,...rest));if(c&&S.mutations!==m0)S.lastRadius=distance(c,p);return r}
async function acceptText(p){
 // With a replaced cadPrompt (tests, other hosts) fall back to the engine flow: text only, default height.
 const h=cadPrompt===cmdPrompt?await askNum(`${labelFor('text')} Specify height <${fnum(S.textHeight)}>:`,S.textHeight,{kind:'distance',base:p,positive:true}):S.textHeight;
 if(h===null){endCmd();return}
 const text=await cadPrompt('Text');
 if(text){S.textHeight=h;add({type:'text',points:[p],text,height:h})}
 endCmd()}
function lineUndo(){const c=S.chain;if(c.pts.length<2){print('Nothing to undo.');return}undo();c.pts.pop();points=[{...c.pts.at(-1)}];render()}
function lineClose(){
 const c=S.chain;if(c.pts.length<3){notify('Close needs at least two segments.');return}
 const a=c.pts.at(-1),b=c.pts[0];if(distance(a,b)>1e-9)add({type:'line',points:[{...a},{...b}]});endCmd()}
function plineClose(){if(points.length<3){notify('At least three points are required to close a polyline.');return}add({type:'polyline',points:structuredClone(points),closed:true});points=[];endCmd()}
function plineUndo(){if(points.length){points.pop();render()}else print('Nothing to undo.')}
function copyUndo(){
 if(S.copy.count<=0){print('Nothing to undo.');return}
 undo();S.copy.count--;selectionSet=new Set(S.copy.sel);selected=S.copy.sel[0]??-1;points=[{...S.copy.base}];render()}
async function circleDiameter(){
 const c=points[0],d=await askNum(`${labelFor('circle')} Specify diameter of circle${S.lastRadius>0?` <${fnum(S.lastRadius*2)}>`:''}:`,S.lastRadius>0?S.lastRadius*2:null,{kind:'distance',base:c,positive:true});
 if(d===null){endCmd();return}S.lastRadius=d/2;add({type:'circle',center:{...c},radius:d/2});endCmd()}
async function rectDims(){
 const a=points[0],L=await askNum(`${labelFor('rectangle')} Specify length for rectangles <${fnum(S.rectL)}>:`,S.rectL,{base:a,positive:true});if(L===null){endCmd();return}
 const Wd=await askNum(`${labelFor('rectangle')} Specify width for rectangles <${fnum(S.rectW)}>:`,S.rectW,{base:a,positive:true});if(Wd===null){endCmd();return}
 S.rectL=L;S.rectW=Wd;const sx=mouse.x<a.x?-1:1,sy=mouse.y<a.y?-1:1,b={x:a.x+sx*L,y:a.y+sy*Wd};
 add({type:'polyline',points:[{...a},{x:b.x,y:a.y},b,{x:a.x,y:b.y}],closed:true});endCmd()}
function clickInput(p){
 const inp=CF.input;if(!inp)return;
 if(inp.kind==='point')return inp.resolve(fpt(p));if(!inp.base)return;
 if(inp.kind==='angle'){let a=Math.atan2(p.y-inp.base.y,p.x-inp.base.x)/DEG;if(a<0)a+=360;return inp.resolve(fnum(a))}
 if(inp.kind==='distance'||inp.kind==='factor')return inp.resolve(fnum(distance(inp.base,p)))}
function pickAt(p){const i=hit(p);if(i>=0){CF.select([i],typeof shiftSelection!=='undefined'&&shiftSelection?'remove':'add');print(`1 found, ${chosen().length} total`)}return Promise.resolve()}
accept=function(p,...rest){
 if(CF.input){clickInput(p);return Promise.resolve()}
 if(CF.picking)return pickAt(p);
 const t=tool;
 if(t!=='select'){if(!CF.isPick())S.lastPoint={x:p.x,y:p.y};if(!S.typed&&ECHO_TOOLS.has(t))print(`${CF.prompt()} ${fpt(p)}`)}
 if(t==='line')return acceptLine(p,rest);
 if(t==='copy')return acceptCopy(p,rest);
 if(t==='text')return acceptText(p);
 if(t==='circle')return acceptCircle(p,rest);
 if(t==='paste'){pasteAt(p);return Promise.resolve()}
 return track(()=>prevAccept(p,...rest))};
{const prevSetTool=setTool;setTool=function(t,...rest){S.chain={pts:[]};S.copy={base:null,count:0,sel:[]};S.zoom.p1=null;S.quiet++;try{prevSetTool(t,...rest)}finally{S.quiet--}syncPrompt()}}
{const prevNotify=notify,NOISE=/^(Select an entity|Specify [^:]*\.$|Ready|Workflow repair|Drag to orbit)|entities selected\. Shift-click/i;let autosaveShown=false;
 notify=function(s){prevNotify(s);if(S.quiet||NOISE.test(String(s)))return;if(/^Autosave unavailable/.test(String(s))){if(autosaveShown)return;autosaveShown=true}print(s)}}
{const prevCheckpoint=checkpoint;checkpoint=function(){S.mutations++;return prevCheckpoint()}}
{const prevRender=render;render=function(){prevRender();noteView();syncPrompt()}}
{const prevPreview=drawPreview;drawPreview=function(...a){
 prevPreview(...a);if(CF.has('interact')||mode3D||(tool!=='zoom'&&tool!=='paste'))return;
 let ents=[];try{ents=CF.previews[tool]?.(mouse)||[]}catch(err){}for(const e of ents)drawEntity(e,CF.colors.preview,true)}}
// ---- Input dispatch ------------------------------------------------------------------------------------------------
function answerInput(raw){const inp=CF.input,text=inp.kind==='text'?raw:raw.trim();inp.resolve(text.trim()===''?'':text)}
function answerPicking(text){
 const pk=CF.picking,echo=pk.message;let m;
 if(text===''){CF.enter();return}
 const visible=i=>CF.visible(doc.entities[i]);
 if(/^all$/i.test(text)){const ids=doc.entities.map((e,i)=>i).filter(visible);CF.select(ids,'add');print(`${echo} ${text}`);print(`${ids.length} found`);return}
 if(/^l(ast)?$/i.test(text)){let i=doc.entities.length-1;while(i>=0&&!visible(i))i--;print(`${echo} ${text}`);if(i>=0){CF.select([i],'add');print('1 found')}return}
 if(/^p(revious)?$/i.test(text)){const ids=S.prevSel.filter(i=>i<doc.entities.length);print(`${echo} ${text}`);CF.select(ids,'add');print(`${ids.length} found`);return}
 const p=parsePointText(text,null,null);
 if(p&&!(m=text.match(RE.num))){print(`${echo} ${text}`);pickAt(p);return}
 print(`${echo} ${text}`);notify('Invalid selection. Expects a point or All/Last/Previous.')}
function toolOption(text){
 const t=tool,pickOpt=list=>matchOpt(text,list);
 if(t==='line'&&points.length){const o=pickOpt(S.chain.pts.length>=3?['Close','Undo']:['Undo']);if(o==='Undo')return lineUndo;if(o==='Close')return lineClose}
 if(t==='polyline'&&points.length){const o=pickOpt(['Close','Undo']);if(o==='Undo')return plineUndo;if(o==='Close')return plineClose}
 if(t==='circle'&&points.length===1&&pickOpt(['Diameter']))return circleDiameter;
 if(t==='copy'&&points.length){const o=pickOpt(['Exit','Undo']);if(o==='Undo')return copyUndo;if(o==='Exit')return()=>endCmd(true)}
 if(t==='rectangle'&&points.length===1&&pickOpt(['Dimensions']))return rectDims;
 return null}
function answerTool(text){
 const echo=CF.prompt();
 if(text===''){CF.enter();return}
 const opt=toolOption(text);
 if(opt){print(`${echo} ${text}`);return opt()}
 if(tool==='circle'&&points.length===1&&RE.num.test(text)){
  print(`${echo} ${text}`);const r=+text;if(!(r>0)){notify('Requires a positive, nonzero radius.');return}
  add({type:'circle',center:{...points[0]},radius:r});S.lastRadius=r;endCmd();return}
 const base=points.length?points[points.length-1]:null,p=parsePointText(text,base,mouse);
 if(p){print(`${echo} ${text}`);S.typed=true;try{return accept(p)}finally{S.typed=false}}
 print(`${echo} ${text}`);
 const def=CF.resolve(text);
 if(def&&def.name===text.trim().toUpperCase().replace(/^[_.'-]+/,'')){CF.run(text,{source:'command'});return}
 notify('Point or option keyword required.')}
const LEGACY=new Set(['fit','obj','stl','movemesh','copymesh','joinprofile','restorelayout','rect','select','close']);
const prevCmdKey=input.onkeydown;
function answerIdle(text){
 if(text===''){repeatLast();return}
 print(`Command: ${text}`);
 if(!CF.resolve(text)){
  if(LEGACY.has(text.toLowerCase())&&prevCmdKey){input.value=text;try{prevCmdKey.call(input,{key:'Enter',target:input,preventDefault(){},stopPropagation(){}})}catch(err){notify(String(err?.message||err))}return}
  print(`Unknown command "${text.toUpperCase()}".  Press F1 for help.`);return}
 CF.run(text,{source:'command'})}
function repeatLast(){const n=CF.lastCommand;if(!n){print('Command:');return}print(`Command: ${n}`);CF.run(n,{source:'repeat'})}
function submit(text){
 const raw=String(text??''),t=raw.trim();
 if(t)remember(t);S.recall=-1;
 try{
  let r;
  if(CF.input)answerInput(raw);else if(CF.picking)answerPicking(t);
  else if(t&&tool!=='select'&&CF.drafting&&CF.drafting.state&&CF.drafting.state()&&CF.drafting.state().prompt!=null&&CF.drafting.input){const echo=CF.prompt();r=CF.drafting.input(t);if(r)print(`${echo} ${t}`);else r=answerTool(t)} // option keyword for a drafting command (clickable options, macros)
  else if(tool!=='select'||points.length)r=answerTool(t);else answerIdle(t);
  if(r&&typeof r.then==='function')r.catch(err=>notify(`Command failed: ${err?.message||err}`))}
 catch(err){notify(`Command failed: ${err?.message||err}`)}
 syncPrompt();return new Promise(res=>setTimeout(()=>{syncPrompt();res()},0))}
// ---- CF.enter / CF.cancel / CF.prompt -------------------------------------------------------------------------------
CF.prompt=function(){
 if(CF.input)return CF.input.message;
 if(CF.picking)return CF.picking.message||'Select objects:';
 const p=CF.prompts[tool];if(p){const s=typeof p==='function'?p():p;if(s)return s}
 const e=ENGINE_PROMPTS[tool];if(e)return`${labelFor(tool)} ${e()}`;
 return tool==='select'?'Command:':labelFor(tool)};
CF.enter=function(){
 if(CF.input){answerInput('');return}
 if(CF.picking){const pk=CF.picking;print(pk.message);pk.done();syncPrompt();render();return}
 if(tool==='select'&&!points.length){repeatLast();return}
 print(CF.prompt());const t=tool;
 if(t==='polyline')finish();
 else if(t==='circle'&&points.length===1&&S.lastRadius>0)add({type:'circle',center:{...points[0]},radius:S.lastRadius});
 endCmd(t==='copy'&&S.copy.count>0)};
CF.cancel=function(){
 const busy=!!(CF.input||CF.picking||tool!=='select'||points.length),wasPicking=!!CF.picking;
 input.value='';hideAc();S.recall=-1;abortPending();S.active=null;S.zoom.p1=null;
 if(tool!=='select')setTool('select');else points=[];
 print('*Cancel*');
 if(!busy||wasPicking){selectionSet.clear();selected=-1;CF.emit('selection',[])}
 syncPrompt();render()};
// ---- ZOOM with a view stack ---------------------------------------------------------------------------------------
const snapView=()=>({x:view.x,y:view.y,scale:view.scale}),clampScale=s=>Math.max(.01,Math.min(1000,s));
function pushView(v){S.views.push(v);if(S.views.length>30)S.views.shift()}
function commitView(fn){const b=snapView();S.lock++;try{fn()}finally{S.lock--}const a=snapView();if(Math.abs(a.x-b.x)>1e-9||Math.abs(a.y-b.y)>1e-9||Math.abs(a.scale-b.scale)>1e-9)pushView(b);S.lastView=snapView();S.viewAt=0}
function noteView(){
 if(mode3D||!view||S.lock)return;const c=snapView(),l=S.lastView;if(!l){S.lastView=c;return}
 if(Math.abs(c.x-l.x)<1e-9&&Math.abs(c.y-l.y)<1e-9&&Math.abs(c.scale-l.scale)<1e-9)return;
 const now=Date.now();if(now-S.viewAt>700)pushView(l);S.viewAt=now;S.lastView=c}
function extentsView(){
 const bs=doc.entities.map(e=>G.bbox(e));if(!bs.length)return{x:0,y:0,scale:4};
 const a=Math.min(...bs.map(b=>b.minX)),b=Math.max(...bs.map(b=>b.maxX)),c=Math.min(...bs.map(b=>b.minY)),d=Math.max(...bs.map(b=>b.maxY));
 return{x:(a+b)/2,y:(c+d)/2,scale:clampScale(Math.min(W/Math.max(20,b-a),H/Math.max(20,d-c))*.8)}}
function zoomTo(v){commitView(()=>{view={x:v.x,y:v.y,scale:clampScale(v.scale)};render()})}
function zoomWindow(a,b){
 const dx=Math.abs(a.x-b.x),dy=Math.abs(a.y-b.y);
 if(dx<1e-9||dy<1e-9){notify('Invalid window; the corners must differ in both directions.');return}
 if(mode3D){notify('Zoom Window works in the 2D model view.');return}
 zoomTo({x:(a.x+b.x)/2,y:(a.y+b.y)/2,scale:Math.min(W/dx,H/dy)})}
function zoomScale(f){if(mode3D){camera3.scale=Math.max(.01,Math.min(1000,camera3.scale*f));render();return}zoomTo({x:view.x,y:view.y,scale:view.scale*f})}
async function zoomCorner(label){const raw=await askLine({message:label,kind:'point'});return raw===null?null:parsePointText(raw,S.lastPoint,null)}
async function zoomAnswer(raw){
 const t=raw.trim(),opt=matchOpt(t,['All','Extents','Previous','Window','In','Out']);let m;
 if(opt==='All'||opt==='Extents'){if(mode3D)fit();else commitView(()=>fit());return true}
 if(opt==='Previous'){const v=S.views.pop();if(!v)notify('No previous view saved.');else{view=v;S.lastView=snapView();S.viewAt=0;render()}return true}
 if(opt==='In'){zoomScale(2);return true}if(opt==='Out'){zoomScale(.5);return true}
 if(opt==='Window'){const a=await zoomCorner('ZOOM Specify first corner:');if(!a)return true;S.zoom.p1=a;points=[a];const b=await zoomCorner('ZOOM Specify opposite corner:');if(b)zoomWindow(a,b);return true}
 if(m=t.match(RE.zoomNum)){const n=+m[1];if(!(n>0)){notify('Requires a positive scale factor.');return false}
  if(mode3D)zoomScale(n);else if(m[2])zoomScale(n);else{const e=extentsView();zoomTo({x:view.x,y:view.y,scale:e.scale*n})}return true}
 const a=parsePointText(t,S.lastPoint,null);
 if(a){S.zoom.p1=a;points=[a];const b=await zoomCorner('ZOOM Specify opposite corner:');if(b)zoomWindow(a,b);return true}
 notify('Requires a point, a scale factor or an option keyword.');return false}
async function zoomCmd(arg){
 abortPending();S.active='ZOOM';setTool('zoom');S.hold++;const message=ENGINE_PROMPTS.zoom();
 try{
  let first=typeof arg==='string'?arg:null;
  for(;;){
   let raw=first;first=null;
   if(raw===null||raw===undefined){raw=await askLine({message:`ZOOM ${message}`,def:'Extents',kind:'point'});if(raw===null)return}
   else print(`ZOOM ${message} ${raw}`);
   if(await zoomAnswer(raw))return}}
 finally{S.hold--;S.zoom.p1=null;if(tool==='zoom')endCmd()}}
CF.previews.zoom=m=>{const a=S.zoom.p1;return a?[{type:'polyline',closed:true,points:[a,{x:m.x,y:a.y},m,{x:a.x,y:m.y}]}]:[]};
// ---- Clipboard -----------------------------------------------------------------------------------------------------
function copyClip(cut){
 const ids=chosen();if(!ids.length){notify('Select objects first.');return}
 const items=ids.map(i=>structuredClone(doc.entities[i])),bs=items.map(e=>G.bbox(e));
 S.clip={items,base:{x:Math.min(...bs.map(b=>b.minX)),y:Math.min(...bs.map(b=>b.minY))}};
 print(`${items.length} object${items.length===1?'':'s'} copied to the clipboard.`);if(cut)eraseIds(ids)}
function pasteAt(p){
 if(!S.clip)return;const dx=p.x-S.clip.base.x,dy=p.y-S.clip.base.y,groups={},start=doc.entities.length;
 const ents=S.clip.items.map(e=>{const c=structuredClone(e);delete c.uuid;transformEntity(c,q=>({x:q.x+dx,y:q.y+dy}));if(c.group)c.group=groups[c.group]??=gid();if(!doc.layers.some(l=>l.name===c.layer))c.layer=layer().name;return c});
 mutate(()=>doc.entities.push(...ents));CF.select(ents.map((_,k)=>start+k),'replace');endCmd()}
CF.previews.paste=m=>{
 if(!S.clip)return[];const dx=m.x-S.clip.base.x,dy=m.y-S.clip.base.y;
 return S.clip.items.slice(0,500).map(e=>{const c=structuredClone(e);transformEntity(c,q=>({x:q.x+dx,y:q.y+dy}));return c})};
// ---- Command registry ----------------------------------------------------------------------------------------------
const reg=(name,aliases,label,desc,category,run,o={})=>CF.register({name,aliases,label,desc,category,run,...o});
const flagCmd=(name,flag,aliases,label,desc,icon)=>reg(name,aliases,label,desc,'Settings',()=>{CF.toggle(flag)},{icon:icon||flag});
const stub=(name,hint)=>()=>notify(`${name}: ${hint}`);
function helpCmd(){
 const cats=['Draw','Modify','Block','Utilities','View','Settings','Files','3D','Help'],by={};
 for(const d of CF.commands.values())(by[d.category||'Other']??=[]).push(d);
 print('CadForge commands: type a name or alias at the Command prompt, then press Enter or Space.');
 for(const c of [...cats,...Object.keys(by).filter(c=>!cats.includes(c))])if(by[c])print(`${c}: `+by[c].sort((a,b)=>a.name<b.name?-1:1).map(d=>d.aliases.length?`${d.name} (${d.aliases.slice(0,2).join(', ')})`:d.name).join(', '));
 print('Keys: F1 help, F2 history, F3 osnap, F7 grid, F8 ortho, F9 snap, F10 polar, F12 dynamic input, Ctrl+Z/Y undo/redo, Ctrl+C/X/V clipboard, Esc cancel.');
 expand(true)}
function startPaste(){if(!S.clip){notify('The clipboard is empty. Use COPYCLIP first.');return}begin('PASTECLIP','paste')}
function askChoice(label,message,options,def,apply,arg){
 abortPending();
 return hold(label,async()=>{
  let raw=typeof arg==='string'?arg:null;
  if(raw===null)raw=await askLine({message,def,kind:'text'});else print(`${message} ${raw}`);
  if(raw===null||raw==='')return;const o=matchOpt(raw,options);if(!o){notify(`Invalid option "${raw}".`);return}return apply(o)})}
function hatchCmd(){
 const go=()=>{const ids=chosen(),idx=ids.find(i=>{const e=doc.entities[i];return e&&(e.type==='circle'||e.type==='polyline'&&e.closed)});
  if(idx===undefined){notify('Select a closed polyline or circle boundary.');endCmd();return}
  selected=idx;selectionSet=new Set([idx]);render();return hold('HATCH',()=>hatch()).finally(()=>endCmd(true))};
 abortPending();S.active='HATCH';return chosen().length?go():void pick('HATCH',go)}
// Draw
reg('LINE',['L'],'Line','Creates straight line segments.','Draw',()=>begin('LINE','line'));
reg('PLINE',['PL','POLYLINE'],'Polyline','Creates a 2D polyline.','Draw',()=>begin('PLINE','polyline'));
reg('CIRCLE',['C'],'Circle','Creates a circle from a center point and radius or diameter.','Draw',()=>begin('CIRCLE','circle'));
reg('ARC',['A'],'Arc','Creates an arc from center, start and end points.','Draw',()=>begin('ARC','arc'));
reg('RECTANG',['REC','RECTANGLE'],'Rectangle','Creates a rectangular polyline.','Draw',()=>begin('RECTANG','rectangle'));
reg('TEXT',['DT','DTEXT','T'],'Text','Creates single-line text.','Draw',()=>begin('TEXT','text'));
reg('MTEXT',['MT'],'Multiline Text','Creates text (each line is a single-line text object).','Draw',()=>begin('MTEXT','text'),{icon:'mtext'});
reg('HATCH',['H','BHATCH'],'Hatch','Fills a closed boundary with hatch lines.','Draw',hatchCmd);
reg('DIMALIGNED',['DAL'],'Aligned Dimension','Creates an aligned linear dimension.','Draw',()=>begin('DIMALIGNED','dimension'));
reg('DIMLINEAR',['DLI','DIM'],'Linear Dimension','Creates a linear dimension.','Draw',()=>begin('DIMLINEAR','dimension'));
// Modify
reg('MOVE',['M'],'Move','Moves objects a specified distance in a specified direction.','Modify',()=>modify('MOVE','move'));
reg('COPY',['CO','CP'],'Copy','Copies objects; repeats until Enter.','Modify',()=>modify('COPY','copy'));
reg('ROTATE',['RO'],'Rotate','Rotates objects around a base point.','Modify',()=>modify('ROTATE','rotate'));
reg('SCALE',['SC'],'Scale','Enlarges or reduces objects.','Modify',()=>modify('SCALE','scale'));
reg('MIRROR',['MI'],'Mirror','Creates a mirror image of objects.','Modify',()=>modify('MIRROR','mirror'));
reg('ERASE',['E','DELETE'],'Erase','Removes objects from the drawing.','Modify',()=>modify('ERASE',null,eraseIds));
reg('EXPLODE',['X'],'Explode','Breaks blocks, groups and polylines into simple objects.','Modify',()=>modify('EXPLODE',null,()=>explode()));
reg('JOIN',['J'],'Join','Joins connected lines into a closed profile.','Modify',()=>modify('JOIN',null,()=>joinProfile()));
reg('OFFSET',['O'],'Offset','Creates parallel lines or concentric circles.','Modify',()=>begin('OFFSET','offset'));
reg('TRIM',['TR'],'Trim','Trims objects to meet the edges of other objects.','Modify',()=>begin('TRIM','trim'));
reg('EXTEND',['EX'],'Extend','Extends objects to meet the edges of other objects.','Modify',()=>begin('EXTEND','extend'));
reg('CHAMFER',['CHA'],'Chamfer','Bevels the corner between two lines.','Modify',()=>modify('CHAMFER',null,()=>chamfer(),{keep:true}));
// Blocks
reg('BLOCK',['B'],'Create Block','Creates a block definition from the selected objects.','Block',()=>modify('BLOCK',null,()=>createBlock()));
reg('INSERT',['I'],'Insert','Inserts a block into the drawing.','Block',()=>{abortPending();return hold('INSERT',()=>startInsert())});
// Utilities
reg('MEASUREGEOM',['MEA','MEASURE'],'Measure','Reports the length and area of an object.','Utilities',()=>modify('MEASUREGEOM',null,()=>measure(),{keep:true}));
reg('SELECTALL',['AI_SELALL'],'Select All','Selects all visible objects.','Utilities',()=>{abortPending();selectAll()});
reg('UNDO',['U'],'Undo','Reverses the last action.','Utilities',()=>undoKey(false,true));
reg('REDO',[],'Redo','Reverses the effect of the last UNDO.','Utilities',()=>undoKey(true,true));
reg('COPYCLIP',[],'Copy to Clipboard','Copies the selected objects to the clipboard.','Utilities',()=>modify('COPYCLIP',null,()=>copyClip(false),{keep:true}));
reg('CUTCLIP',[],'Cut','Copies the selected objects to the clipboard and erases them.','Utilities',()=>modify('CUTCLIP',null,()=>copyClip(true)));
reg('PASTECLIP',[],'Paste','Pastes the clipboard objects at an insertion point.','Utilities',startPaste);
reg('HELP',['?'],'Command List','Lists the available commands and keys.','Help',helpCmd);
// View
reg('ZOOM',['Z'],'Zoom','Zooms to a window, extents, a scale factor or the previous view.','View',zoomCmd);
reg('PAN',['P'],'Pan','Pans the view; press Esc or Enter to exit.','View',()=>{abortPending();CF.startPan()});
reg('REGEN',['RE','REGENALL'],'Regen','Regenerates the drawing and redraws the view.','View',()=>{render();print('Regenerating model.')});
reg('3DORBIT',['3DO','ORBIT'],'3D Orbit','Opens the 3D mesh view.','View',()=>{abortPending();show3D()});
reg('PLAN',[],'Plan View','Returns to the 2D plan (top) view.','View',()=>{abortPending();showModel()});
reg('VIEW',['V'],'Named Views','Saves or restores a named view.','View',arg=>askChoice('VIEW','VIEW Enter an option [Restore/Save]:',['Restore','Save'],'',o=>o==='Save'?saveLayout():restoreLayout(),arg));
reg('VSCURRENT',['VS'],'Visual Style','Sets the visual style: 2dwireframe, wireframe, shaded or shaded with edges.','View',arg=>askChoice('VSCURRENT','VSCURRENT Enter an option [2dwireframe/Wireframe/Shaded/shadedEdges] <2dwireframe>:',['2dwireframe','Wireframe','Shaded','shadedEdges'],'2dwireframe',o=>CF.setVisualStyle(o.toLowerCase()),arg));
reg('MODEL',[],'Model','Switches to model space.','View',()=>CF.setSpace('model'));
reg('LAYOUT',[],'Layout','Switches to a paper-space layout.','View',()=>CF.setSpace('layout'));
// Settings and interface
flagCmd('GRID','grid',[],'Grid','Turns the grid display on or off (F7).','grid');
flagCmd('SNAP','snap',['SN'],'Snap Mode','Turns grid snap on or off (F9).','snap');
flagCmd('ORTHO','ortho',[],'Ortho Mode','Constrains cursor movement to horizontal and vertical (F8).','ortho');
flagCmd('OSNAP','osnap',['OS'],'Object Snap','Turns running object snaps on or off (F3).','osnap');
flagCmd('POLAR','polar',[],'Polar Tracking','Turns polar tracking on or off (F10).','polar');
flagCmd('DYNMODE','dyn',[],'Dynamic Input','Turns dynamic input on or off (F12).','dyn');
reg('CLEANSCREENON',[],'Clean Screen','Hides the ribbon and palettes (Ctrl+0).','Settings',()=>CF.set('clean',true),{icon:'clean'});
reg('CLEANSCREENOFF',[],'Restore Screen','Restores the ribbon and palettes.','Settings',()=>CF.set('clean',false),{icon:'clean'});
reg('COMMANDLINE',[],'Command Line','Shows the command line (Ctrl+9).','Settings',()=>{CF.set('commandLine',true);try{input.focus()}catch(err){}},{icon:'commandline'});
reg('COMMANDLINEHIDE',[],'Hide Command Line','Hides the command line.','Settings',()=>CF.set('commandLine',false),{icon:'commandline'});
reg('TEXTSCR',[],'Text Window','Expands or collapses the command history (F2).','Settings',()=>{expand()},{icon:'commandline',noRepeat:true});
reg('PROPERTIES',['PR','PROPS','CH','MO'],'Properties','Shows or hides the Properties palette (Ctrl+1).','Settings',()=>CF.toggle('properties'));
reg('PROPERTIESCLOSE',['PRCLOSE'],'Close Properties','Closes the Properties palette.','Settings',()=>CF.set('properties',false),{icon:'properties'});
reg('LAYER',['LA'],'Layer Properties','Opens the Layer Properties Manager.','Settings',()=>CF.openLayerManager());
reg('RIBBON',[],'Ribbon','Shows the ribbon.','Settings',()=>CF.set('ribbonMin',false),{icon:'ribbon'});
reg('RIBBONCLOSE',[],'Close Ribbon','Minimizes the ribbon.','Settings',()=>CF.set('ribbonMin',true),{icon:'ribbon'});
reg('WSCURRENT',[],'Workspace','Sets the current workspace.','Settings',arg=>askChoice('WSCURRENT','WSCURRENT Enter name of workspace to make current [Drafting/3dModeling] <Drafting>:',['Drafting','3dModeling'],'Drafting',o=>CF.setWorkspace(o==='Drafting'?'drafting':'3d'),typeof arg==='string'&&/^2d/i.test(arg.trim())?'Drafting':arg),{icon:'workspace'});
// Files
reg('NEW',[],'New','Creates a new drawing (Ctrl+N).','Files',()=>{abortPending();return CF.newDrawing()});
reg('OPEN',[],'Open','Opens a drawing or DXF file (Ctrl+O).','Files',()=>CF.openFile());
reg('QSAVE',['SAVE'],'Save','Saves the drawing (Ctrl+S).','Files',()=>CF.saveProject());
reg('SAVEAS',[],'Save As','Saves the drawing under a new name (Ctrl+Shift+S).','Files',()=>{abortPending();return hold('SAVEAS',()=>CF.saveAs())});
reg('PLOT',['PRINT'],'Plot','Plots the drawing to a PDF or SVG sheet (Ctrl+P).','Files',()=>plotSheet());
reg('PAGESETUP',[],'Page Setup','Sets paper size, orientation and scale.','Files',()=>plotSheet());
reg('EXPORT',['EXP'],'Export','Exports the drawing to DXF, SVG, OBJ, STL or PDF.','Files',arg=>askChoice('EXPORT','EXPORT Enter file format [Dxf/Svg/Obj/Stl/Pdf]:',['Dxf','Svg','Obj','Stl','Pdf'],'',o=>({Dxf:()=>$('dxf').click(),Svg:()=>$('svg').click(),Obj:()=>exportOBJ(),Stl:()=>exportSTL(),Pdf:()=>plotSheet()})[o](),arg));
reg('DXFOUT',[],'Export DXF','Exports the drawing as an ASCII DXF file.','Files',()=>$('dxf').click(),{icon:'export'});
reg('DXFIN',[],'Import DXF','Opens a DXF or project file.','Files',()=>CF.openFile(),{icon:'import'});
reg('SVGOUT',[],'Export SVG','Exports the current view as SVG.','Files',()=>$('svg').click(),{icon:'export'});
reg('STLOUT',[],'Export STL','Exports the meshes as an STL file.','Files',()=>exportSTL(),{icon:'export'});
reg('OBJEXPORT',[],'Export OBJ','Exports the meshes as a Wavefront OBJ file.','Files',()=>exportOBJ(),{icon:'export'});
// 3D mesh modeling
reg('EXTRUDE',['EXT'],'Extrude','Extrudes a closed profile into a mesh part.','3D',()=>{abortPending();return hold('EXTRUDE',()=>extrude())});
reg('BOX',[],'Box','Creates a box mesh from width, depth and height.','3D',()=>{abortPending();return hold('BOX',()=>box3D())});
reg('UNION',['UNI'],'Union','Combines Mesh A and Mesh B.','3D',()=>applyBoolean('union'));
reg('SUBTRACT',['SU'],'Subtract','Subtracts Mesh B from Mesh A.','3D',()=>applyBoolean('subtract'));
reg('INTERSECT',['IN'],'Intersect','Keeps the volume shared by Mesh A and Mesh B.','3D',()=>applyBoolean('intersect'));
reg('UPDATEEXTRUSION',[],'Update Extrusion','Rebuilds an extrusion from its source outlines.','3D',()=>updateExtrusion(),{icon:'update'});
reg('3DMOVE',['3M'],'3D Move','Moves Mesh A by a displacement.','3D',()=>{abortPending();return hold('3DMOVE',()=>translateMesh())},{icon:'mesh-move'});
reg('MESHCOPY',[],'Copy Mesh','Duplicates Mesh A.','3D',()=>duplicateMesh(),{icon:'mesh-copy'});
reg('MESHCLEAR',[],'Clear Meshes','Removes all meshes.','3D',()=>clearMeshes(),{icon:'mesh-clear'});
reg('MESHFIT',[],'Fit Meshes','Fits the 3D view to the meshes.','3D',()=>{if(mode3D){fit3D();render()}else show3D()},{icon:'mesh-fit'});
// ---- Keyboard --------------------------------------------------------------------------------------------------------
function undoKey(redo,quiet){
 if(!redo&&CF.input)return;
 if(!redo&&tool==='line'&&S.chain.pts.length>1){lineUndo();return}
 if(!redo&&tool==='polyline'&&points.length){plineUndo();return}
 undo(redo);if(!quiet)print(redo?'REDO':'UNDO')}
function dialogOpen(){
 try{
  if(typeof inputDialog!=='undefined'&&inputDialog.open)return true;if(typeof plotDialog!=='undefined'&&plotDialog.open)return true;
  if(typeof exportDialog!=='undefined'&&exportDialog.open)return true;if(typeof extrusionDialog!=='undefined'&&extrusionDialog.open)return true;
  for(const d of document.querySelectorAll('dialog'))if(d.open&&String(d.id||'').startsWith('cf-'))return true}catch(err){}
 return false}
const isField=t=>{try{return!!t&&(!!t.matches?.('input,select,textarea,[contenteditable=""],[contenteditable="true"]')||/^(INPUT|SELECT|TEXTAREA)$/.test(t.tagName||''))}catch(err){return false}};
const isButton=t=>/^(BUTTON|A|SUMMARY)$/.test(t?.tagName||'');
const stop=e=>{e.cfHandled=true;e.preventDefault?.();e.stopPropagation?.()};
const FKEYS={F3:'osnap',F7:'grid',F8:'ortho',F9:'snap',F10:'polar',F12:'dyn'};
function cmdKey(e){
 if(!e||e.cfHandled||e.ctrlKey||e.metaKey||e.altKey)return;
 const key=e.key,v=input.value;
 if(key==='Enter'||key==='NumpadEnter'||(key===' '&&CF.input?.kind!=='text')){
  if(key!==' '&&S.ac.length&&S.acIndex>=0){stop(e);chooseAc(S.acIndex,true);return}
  stop(e);const raw=input.value;input.value='';hideAc();submit(raw);return}
 if(key==='ArrowUp'||key==='ArrowDown'){
  if(S.ac.length){stop(e);moveAc(key==='ArrowDown'?1:-1);return}
  if(v===''||S.recall>=0){stop(e);recall(key==='ArrowUp'?-1:1)}return}
 if(key==='Tab'&&S.ac.length){stop(e);chooseAc(S.acIndex>=0?S.acIndex:0,false)}}
input.onkeydown=cmdKey;
document.onkeydown=function(e){
 if(!e||dialogOpen())return;
 const key=e.key||'',k=key.toLowerCase(),ctrl=!!(e.ctrlKey||e.metaKey),shift=!!e.shiftKey,alt=!!e.altKey,t=e.target,inCmd=t===input,inField=!inCmd&&isField(t),typing=inCmd||inField;
 if(key==='F1'){e.preventDefault?.();CF.run('HELP',{source:'key'});return}
 if(key==='F2'){e.preventDefault?.();expand();return}
 if(FKEYS[key]){e.preventDefault?.();CF.toggle(FKEYS[key]);return}
 if(key==='Escape'){if(inField)return;e.preventDefault?.();CF.cancel();return}
 if(ctrl){
  if(alt)return;
  if(k==='n'||k==='o'||k==='s'||k==='p'){
   const map={n:'NEW',o:'OPEN',s:shift?'SAVEAS':'QSAVE',p:'PLOT'};e.preventDefault?.();CF.run(map[k],{source:'key'});return}
  if(k==='1'||k==='9'||k==='0'){e.preventDefault?.();CF.toggle(k==='1'?'properties':k==='9'?'commandLine':'clean');return} // interface toggles work from palette fields too
  if(inField)return;
  if(k==='z'||k==='y'){e.preventDefault?.();undoKey(k==='y'||shift);return}
  if(k==='a'&&!inCmd){e.preventDefault?.();CF.run('SELECTALL',{source:'key'});return}
  if((k==='c'||k==='x'||k==='v')&&(!inCmd||input.value==='')){e.preventDefault?.();CF.run(k==='c'?'COPYCLIP':k==='x'?'CUTCLIP':'PASTECLIP',{source:'key'});return}
  if(k==='1'){e.preventDefault?.();CF.toggle('properties');return}
  if(k==='9'){e.preventDefault?.();CF.toggle('commandLine');return}
  if(k==='0'){e.preventDefault?.();CF.toggle('clean');return}
  return}
 if(alt||inField)return;
 if(inCmd){
  if(key==='Delete'&&input.value===''){e.preventDefault?.();const ids=chosen();if(ids.length)eraseIds(ids)}
  else if(!e.cfHandled&&(key==='Enter'||key==='ArrowUp'||key==='ArrowDown'||key==='Tab'||key===' '))cmdKey(e);
  return}
 if(key==='Delete'){e.preventDefault?.();const ids=chosen();if(ids.length)eraseIds(ids);return}
 if(key==='Enter'||(key===' '&&CF.input?.kind!=='text')){if(isButton(t))return;e.preventDefault?.();submit('');return}
 if(key==='ArrowUp'||key==='ArrowDown'){try{input.focus()}catch(err){}e.preventDefault?.();cmdKey(e);return}
 if(key.length===1){e.preventDefault?.();try{input.focus()}catch(err){}input.value+=key;onInput()}};
// ---- Wiring --------------------------------------------------------------------------------------------------------
CF.on('command',({def,source})=>{S.active=def.name;if(source!=='command'&&source!=='repeat')print(`Command: ${def.name}`,'cf-cmd-in')});
CF.on('tool',()=>syncPrompt());
CF.on('document',()=>{S.lastPoint=null;S.views=[];S.lastView=null});
CF.commandLine={submit,print,ask:askLine,cadPrompt:cmdPrompt,autocomplete:acMatches,parsePoint:parsePointText,expand,focus(){try{input.focus()}catch(err){}},setPrompt(text){S.shown=String(text)+'|0';renderPrompt(String(text))},
 state:()=>({active:S.active,chain:S.chain.pts.length,copy:S.copy.count,views:S.views.length,clip:S.clip?S.clip.items.length:0,history:S.hist.slice(),expanded:S.expanded,ac:S.ac.length,acIndex:S.acIndex})};
buildUI();
print('CadForge command line. Type a command (LINE, CIRCLE, MOVE, ZOOM) and press Enter, or press F1 for the command list.');
syncPrompt();
})();
