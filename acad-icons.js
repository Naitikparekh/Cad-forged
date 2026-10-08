'use strict';
// CadForge original icon library: CF.icon(name,size) returns an inline 24x24-grid SVG string (currentColor + one accent).
{
const B='#4fa3e8',Y='#e8b84f',G='#5cc87a',R='#e05a5a',AX='#e0605a',AY='#5cc87a';
// Drawing helpers (all coordinates on the 24-unit grid; stroke 1.5 inherited from the <svg>).
const p=(d,c,w)=>`<path d="${d}"${c?` stroke="${c}"`:''}${w?` stroke-width="${w}"`:''}/>`;
const f=(d,c='currentColor',o)=>`<path d="${d}" fill="${c}" stroke="none"${o?` fill-opacity="${o}"`:''}/>`;
const pf=(d,c=B,o=.3)=>`<path d="${d}" stroke="${c}" fill="${c}" fill-opacity="${o}"/>`;
const dash=(d,c)=>`<path d="${d}" stroke-dasharray="2 2"${c?` stroke="${c}"`:''}/>`;
const c=(x,y,r,col)=>`<circle cx="${x}" cy="${y}" r="${r}"${col?` stroke="${col}"`:''}/>`;
const cf=(x,y,r,col=B,o=.3)=>`<circle cx="${x}" cy="${y}" r="${r}" stroke="${col}" fill="${col}" fill-opacity="${o}"/>`;
const dot=(x,y,r=1.2,col='currentColor')=>`<circle cx="${x}" cy="${y}" r="${r}" fill="${col}" stroke="none"/>`;
const g=(x,y,col='currentColor')=>`<rect x="${x-1.75}" y="${y-1.75}" width="3.5" height="3.5" fill="${col}" stroke="none"/>`; // grip / defining point
const txt=(x,y,s,t,col='#fff')=>`<text x="${x}" y="${y}" font-size="${s}" font-family="Segoe UI,Arial,sans-serif" font-weight="700" text-anchor="middle" fill="${col}" stroke="none">${t}</text>`;
// Isometric cube centred at (x,y) with "radius" s: [outline+inner edges, top face, left face, right face].
const cube=(x,y,s)=>{const h=s*.87,q=s/2,n=v=>+v.toFixed(2),P=(a,b)=>`${n(a)} ${n(b)}`;return{
 edges:`M${P(x,y-s)}L${P(x+h,y-q)}V${n(y+q)}L${P(x,y+s)}L${P(x-h,y+q)}V${n(y-q)}ZM${P(x-h,y-q)}L${P(x,y)}L${P(x+h,y-q)}M${P(x,y)}V${n(y+s)}`,
 top:`M${P(x,y-s)}L${P(x+h,y-q)}L${P(x,y)}L${P(x-h,y-q)}Z`,left:`M${P(x-h,y-q)}L${P(x,y)}V${n(y+s)}L${P(x-h,y+q)}Z`,right:`M${P(x+h,y-q)}L${P(x,y)}V${n(y+s)}L${P(x+h,y+q)}Z`,
 hidden:`M${P(x,y-s)}V${n(y)}M${P(x-h,y+q)}L${P(x,y)}L${P(x+h,y+q)}`}};
const K=cube(12,12,9);
// Gear outline generated once (8 teeth).
const gear=(()=>{let d='';for(let i=0;i<8;i++){const a=i*Math.PI/4;for(const [da,r]of [[-.24,6.6],[-.14,9.2],[.14,9.2],[.24,6.6]])d+=(d?'L':'M')+(12+r*Math.cos(a+da)).toFixed(2)+' '+(12+r*Math.sin(a+da)).toFixed(2)}return d+'Z'})();
const magnifier=(x=10.5,y=10.5,r=6.5)=>c(x,y,r)+p(`M${x+r*.72} ${y+r*.72}L${x+r*.72+5} ${y+r*.72+5}`,undefined,2.2);
const docShape='M6 3h8l4 4v14H6z',docFold='M14 3v4h4';
const badge=(t,col)=>p(docShape)+p(docFold)+`<rect x="2.5" y="12" width="17" height="8" rx="1" fill="${col}" stroke="none"/>`+txt(11,18.4,6.6,t);
const bulb='M12 3a6 6 0 0 0-3.6 10.8c.7.6 1.1 1.4 1.1 2.2h5c0-.8.4-1.6 1.1-2.2A6 6 0 0 0 12 3z';
const hexagon='M20 12l-4 6.93H8L4 12l4-6.93h8z';
const I={
 // ---- application / files ----------------------------------------------------------
 app:`<path d="M22.5 12l-5.25 9.09H6.75L1.5 12l5.25-9.09h10.5z" fill="#2f6fb0" stroke="none"/>`+p('M11.5 8.54A4 4 0 1 0 11.5 15.46','#fff',2)+p('M14 16V8h4.3M14 12h3.3','#fff',2),
 menu:p('M4 7h16M4 12h16M4 17h16'),
 new:p('M13 21H6V3h8l4 4v6')+p('M14 3v4h4')+p('M18 15v6M15 18h6',G,2),
 open:p('M3 19V5h6l2 2.5h8V10')+pf('M3 19l3-8.5h16L19 19z',Y,.35),
 save:p('M4 4h12.5L20 7.5V20H4z')+p('M8 4v5h7V4')+pf('M7.5 20v-6.5h9V20',B,.3),
 saveas:p('M11 20H4V4h12.5L20 7.5V11')+p('M8 4v4.5h6.5V4')+pf('M13 21v-2.6l6.2-6.2 2.6 2.6-6.2 6.2z',Y,.3),
 plot:p('M7 9V3.5h10V9')+p('M7 17H4.5V9h15v8H17')+pf('M7 13.5h10v7H7z',B,.2)+p('M9.5 16.5h5M9.5 18.5h3')+dot(16.6,11.3,.9,G),
 pagesetup:p('M5 3h14v18H5z')+dash('M8 6h8v12H8z',B)+p('M10.5 15h3',B),
 export:p('M11 4H4v16h16v-7')+p('M14 4h6v6M20 4l-9 9',B),
 import:p('M13 4h7v16H4v-7')+p('M4 4l9 9M13 7v6H7',B),
 'file-dxf':badge('DXF','#3f86c9'),'file-svg':badge('SVG','#c9842f'),'file-obj':badge('OBJ','#3f9a5c'),'file-stl':badge('STL','#7d63c9'),'file-pdf':badge('PDF','#c94b48'),
 close:p('M6 6l12 12M18 6L6 18'),
 undo:p('M8.5 4.5L4 9l4.5 4.5',Y)+p('M4 9h10.5a5 5 0 0 1 0 10H10'),
 redo:p('M15.5 4.5L20 9l-4.5 4.5',Y)+p('M20 9H9.5a5 5 0 0 0 0 10H14'),
 search:c(10.5,10.5,6)+p('M15 15l5.5 5.5',undefined,2),
 help:c(12,12,9)+p('M9.5 9.3a2.6 2.6 0 1 1 3.6 2.4c-.7.3-1.1 1-1.1 1.7v.8',B)+dot(12,17.2,1.1,B),
 home:p('M3.5 11.5L12 4l8.5 7.5')+p('M6 9.5V20h12V9.5')+pf('M10 20v-5.5h4V20',B,.3),
 'chevron-down':p('M7 9.5l5 5 5-5'),'chevron-up':p('M7 14.5l5-5 5 5'),'chevron-right':p('M9.5 7l5 5-5 5'),'chevron-left':p('M14.5 7l-5 5 5 5'),
 check:p('M5 12.5l4.5 4.5L19 7.5',G,2),plus:p('M12 5v14M5 12h14'),minus:p('M5 12h14'),
 // ---- interface ---------------------------------------------------------------------
 layer:pf('M12 3.5l8.5 4.25L12 12 3.5 7.75z',B,.3)+p('M3.5 12L12 16.25 20.5 12')+p('M3.5 16.25L12 20.5l8.5-4.25'),
 'layer-properties':pf('M9 3.5l7 3.5-7 3.5L2 7z',B,.3)+p('M2 11l7 3.5 7-3.5')+p('M2 15l7 3.5 7-3.5')+p('M18.5 5h3M18.5 9.5h3M18.5 14h3M18.5 18.5h3',Y),
 'layer-on':pf(bulb,Y,.85)+p('M9.5 18.5h5M10.5 21h3'),
 'layer-off':`<g opacity=".6">${p(bulb)}${p('M9.5 18.5h5M10.5 21h3')}</g>`,
 'layer-freeze':p('M12 3v18M4.2 7.5l15.6 9M4.2 16.5l15.6-9',B)+p('M10 4.5l2 2 2-2M10 19.5l2-2 2 2',B),
 'layer-current':pf('M12 6l8.5 4.25L12 14.5 3.5 10.25z',B,.3)+p('M3.5 14.5L12 18.75l8.5-4.25')+p('M15 4.5l2 2 3.5-4',G,1.8),
 'layer-new':pf('M10 5l7.5 3.75L10 12.5 2.5 8.75z',B,.3)+p('M2.5 12.5L10 16.25l4-2')+p('M18.5 13v8M14.5 17h8',G,2),
 'layer-allon':pf('M12 3.5l8.5 4.25L12 12 3.5 7.75z',Y,.45)+p('M3.5 12L12 16.25 20.5 12')+p('M3.5 16.25L12 20.5l8.5-4.25'),
 color:p('M12 3a9 9 0 1 0 0 18c1.2 0 1.8-.8 1.8-1.7 0-1.3-1.3-1.8-1.3-3 0-1 .8-1.6 1.8-1.6H17a4 4 0 0 0 4-4C21 6.4 17 3 12 3z')+dot(7.5,11,1.4,R)+dot(9.8,7,1.4,Y)+dot(14.6,7,1.4,G)+dot(8.3,15.2,1.4,B),
 properties:p('M4 3h16v18H4z')+p('M4 7h16')+p('M7 10.5h3M7 14h3M7 17.5h3')+p('M12.5 10.5h4.5M12.5 14h4.5M12.5 17.5h4.5',B),
 settings:p('M4 7h16M4 12h16M4 17h16')+cf(8,7,1.9,B,1)+cf(15.5,12,1.9,B,1)+cf(10,17,1.9,B,1),
 gear:p(gear)+c(12,12,3,B),
 workspace:p('M3 4h8v7H3zM13 4h8v7h-8zM3 13h8v7H3z')+pf('M13 13h8v7h-8z',B,.35),
 grid:p('M4 4h16v16H4z')+p('M9.33 4v16M14.67 4v16M4 9.33h16M4 14.67h16',undefined,1),
 snap:[[6,6],[12,6],[18,6],[6,12],[18,12],[6,18],[12,18],[18,18]].map(([x,y])=>dot(x,y,1.2)).join('')+p('M10 10h4v4h-4z',B)+p('M12 7.5V10M12 14v2.5M7.5 12H10M14 12h2.5',B,1),
 ortho:p('M5 3.5V19h15.5')+p('M5 15h4v4',B)+p('M3 6l2-2.5L7 6M18 17l2.5 2-2.5 2',undefined,1.2),
 polar:p('M4 20h16')+p('M4 20L17.5 6.5',B)+dash('M17.5 6.5l3-3',B)+p('M11 20A7 7 0 0 0 8.95 15.05',Y),
 osnap:p('M8 8h8v8H8z',B)+p('M12 2.5v4M12 17.5v4M2.5 12h4M17.5 12h4'),
 dyn:p('M8 3v10M3 8h10')+pf('M11.5 13.5h9.5v6h-9.5z',B,.2)+p('M13.5 16.5h4',B),
 clean:p('M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5')+pf('M8.5 8.5h7v7h-7z',B,.25),
 ribbon:p('M3 4h18v16H3z')+pf('M3 4h18v5H3z',B,.3)+p('M6 14h5M6 17h3',undefined,1.2),
 'ribbon-min':p('M3 4h18v16H3z')+pf('M3 4h18v5H3z',B,.3)+p('M9 16l3-3 3 3',Y),
 filetabs:p('M3 9h18v11H3z')+p('M3 9V5h7v4')+pf('M10 9V5.5h7V9',B,.3),
 layouttabs:p('M3 4h18v12H3z')+p('M3 20h5.5v-4M8.5 20h6v-4')+pf('M5.5 6.5h13v7h-13z',B,.15),
 commandline:p('M3 5h18v14H3z')+p('M6.5 10l3 2.5-3 2.5',B)+p('M11.5 15.5h5'),
 textscr:p('M3 4h18v16H3z')+p('M6 8h12M6 11.5h12M6 15h7',B),
 // ---- navigation / views ------------------------------------------------------------
 pan:p('M8.5 12V5.75a1.25 1.25 0 0 1 2.5 0V11M11 10.5V4.25a1.25 1.25 0 0 1 2.5 0V11M13.5 10.75V5.75a1.25 1.25 0 0 1 2.5 0V12M16 11.5V8.25a1.25 1.25 0 0 1 2.5 0V14c0 4-2.7 7-6.5 7-2.6 0-4-1.1-5.3-3l-2.6-4a1.3 1.3 0 0 1 2.1-1.5l2.1 2'),
 zoom:magnifier()+`<circle cx="10.5" cy="10.5" r="4.5" fill="${B}" fill-opacity=".3" stroke="none"/>`,
 'zoom-in':magnifier()+p('M7.5 10.5h6M10.5 7.5v6',B),
 'zoom-out':magnifier()+p('M7.5 10.5h6',B),
 'zoom-extents':p('M3 8V3h5M16 3h5v5M21 16v5h-5M8 21H3v-5',B)+c(11,11,4)+p('M14 14l3.5 3.5',undefined,2),
 'zoom-window':dash('M3 3h13v10H3z',B)+c(15,15,4)+p('M18 18l3 3',undefined,2),
 'zoom-previous':magnifier()+p('M13 10.5H8.2M10.2 8.3L8 10.5l2.2 2.2',Y),
 orbit:c(12,12,5)+`<ellipse cx="12" cy="12" rx="10" ry="4" transform="rotate(-20 12 12)" stroke="${B}"/>`+dot(20.6,8.2,1.4,Y),
 regen:p('M19.5 9.5A8 8 0 0 0 5 8',B)+p('M4.5 14.5A8 8 0 0 0 19 16',B)+p('M19.5 4.5v5h-5M4.5 19.5v-5h5'),
 plan:p('M12 2v5',Y)+p('M10 5l2 2 2-2',Y)+pf('M12 9l8 4-8 4-8-4z',B,.45)+p('M4 13v4l8 4 8-4v-4M12 17v4'),
 view:p('M3 7.5h12v9H3z')+pf('M15 10.5l6-3v9l-6-3',B,.3),
 'view-save':p('M3 7.5h12v9H3z')+pf('M15 10.5l6-3v9l-6-3',B,.3)+p('M9 9.5v5M6.5 12h5',G,1.8),
 'view-restore':p('M3 7.5h12v9H3z')+pf('M15 10.5l6-3v9l-6-3',B,.3)+p('M11.5 12H6.5M8.5 10l-2 2 2 2',Y,1.6),
 navbar:p('M8 2.5h8v19H8z')+p('M10.5 6h3M10.5 10h3M10.5 14h3M10.5 18h3',B),
 viewcube:pf(K.top,B,.45)+p(K.edges),
 'view-top':pf(K.top,B,.55)+p(K.edges),'view-front':pf(K.left,B,.55)+p(K.edges),'view-right':pf(K.right,B,.55)+p(K.edges),
 'view-bottom':p(K.edges)+dash(K.hidden,B)+f('M4.17 16.5L12 12l7.83 4.5L12 21z',B,.3),
 'view-back':p(K.edges)+dash(K.hidden,B)+f('M12 3V12l-7.83 4.5V7.5z',B,.3),
 'view-left':p(K.edges)+dash(K.hidden,B)+f('M12 3V12l7.83 4.5V7.5z',B,.3),
 'view-iso':f(K.top,B,.55)+f(K.left,B,.3)+f(K.right,B,.15)+p(K.edges),
 'visual-style':c(12,12,8.5)+pf('M12 3.5A8.5 8.5 0 0 1 12 20.5A4 8.5 0 0 0 12 3.5z',B,.55),
 'vs-2dwireframe':p('M4 4h16v16H4z')+c(12,12,5,B),
 'vs-wireframe':p(K.edges,B)+dash(K.hidden),
 'vs-shaded':f(K.top,'#86c2f2')+f(K.left,'#3d86c6')+f(K.right,'#2a6aa3'),
 'vs-shadededges':f(K.top,'#86c2f2')+f(K.left,'#3d86c6')+f(K.right,'#2a6aa3')+p(K.edges),
 ucs:p('M6 18V5',AY)+p('M6 18h13',AX)+p('M3.8 7.5L6 5l2.2 2.5',AY)+p('M16.5 15.8L19 18l-2.5 2.2',AX)+p('M6 15h3v3'),
 model:p('M4 20V4l16 16z')+p('M7.5 16.5v-4.6l4.6 4.6z',B),
 layout:p('M4 3h16v18H4z')+pf('M6.5 5.5h11v9h-11z',B,.2)+p('M6.5 17.5h11'),
 // ---- draw --------------------------------------------------------------------------
 line:p('M5 19L19 5',B)+g(5,19)+g(19,5),
 polyline:p('M4 18L9 7.5l6 9 5-11',B)+g(4,18)+g(9,7.5)+g(15,16.5)+g(20,5.5),
 circle:c(12,12,8,B)+p('M12 12h8',undefined,1)+g(12,12),
 arc:p('M4 17A8 8 0 0 1 20 17',B)+g(4,17)+g(12,9)+g(20,17),
 rectangle:p('M4 6h16v12H4z',B)+g(4,18)+g(20,6),
 polygon:pf(hexagon,B,.12)+g(12,12)+p('M12 12l8 0',undefined,1),
 ellipse:`<ellipse cx="12" cy="12" rx="9" ry="5.5" stroke="${B}"/>`+dash('M3 12h18')+g(12,12),
 hatch:p('M4 4h16v16H4z')+p('M4 10l6-6M4 16L16 4M8 20L20 8M14 20l6-6',B,1.2),
 text:p('M5.5 20L12 4l6.5 16M8 14h8',B,1.8),
 mtext:p('M3.5 15L8 4l4.5 11M5.2 11h5.6',B,1.6)+p('M14.5 7h6.5M14.5 11h6.5M14.5 15h6.5M3.5 19.5h17.5'),
 dimlinear:p('M5 6.5V20M19 6.5V20')+p('M5 10.5h14',B)+f('M5 10.5l3.5-2v4z',B)+f('M19 10.5l-3.5-2v4z',B)+p('M9.5 6.5h5',Y),
 dimaligned:p('M7.3 20.3L3 16M20.3 7.3L16 3')+p('M4 17L17 4',B)+f('M4 17l2.97-1.27-1.7-1.7z',B)+f('M17 4l-2.97 1.27 1.7 1.7z',B)+p('M10.5 16.5l3-3',Y),
 block:dash('M3 3h18v18H3z')+c(9,9,3.2,B)+p('M11.5 12.8h6.5v5.7h-6.5z',B)+p('M5.5 18.5l2.5-4 2.5 4z',B),
 insert:pf('M11 11h9.5v9.5H11z',B,.2)+c(15.75,15.75,2.4,B)+p('M3.5 3.5L11 11',Y)+p('M6.5 11H11V6.5',Y),
 // ---- modify ------------------------------------------------------------------------
 move:p('M12 3v18M3 12h18')+p('M9.5 5.5L12 3l2.5 2.5M9.5 18.5L12 21l2.5-2.5M5.5 9.5L3 12l2.5 2.5M18.5 9.5L21 12l-2.5 2.5',Y),
 copy:p('M14 7.5V4H4v10h3.5')+pf('M9 9h11v11H9z',B,.15),
 rotate:`<rect x="9" y="9" width="6" height="6" transform="rotate(20 12 12)" stroke="${B}"/>`+p('M20 12A8 8 0 1 1 17.66 6.34',Y)+p('M14.66 6.34h3v-3',Y),
 scale:pf('M4 13h7v7H4z',B,.2)+dash('M4 10V4h16v16h-6')+p('M11 13l7-7',Y)+p('M13.5 6H18v4.5',Y),
 mirror:dash('M12 2.5v19')+p('M9.5 6L3.5 18h6z')+pf('M14.5 6l6 12h-6z',B,.25),
 offset:p('M4 20C4 11 11 4 20 4')+p('M9.5 20c0-5.5 5-10.5 10.5-10.5',B)+p('M6.6 11.6l2.8 2.8',Y,1.2),
 trim:p('M14 3v18')+p('M3 12h11',B)+dash('M14 12h7',R),
 extend:p('M20 3v18')+p('M4 12h7',B)+dash('M11 12h8.5',B)+p('M16 9l3 3-3 3',Y),
 fillet:dash('M4 11V4h7',undefined)+p('M4 21V11M11 4h9')+p('M4 11A7 7 0 0 1 11 4',B,2),
 chamfer:dash('M4 10V4h6')+p('M4 21V10M10 4h10')+p('M4 10l6-6',B,2),
 erase:`<g transform="rotate(-45 12 12)">${p('M5 8.5h14v7H5z')}${pf('M5 8.5h5v7H5z',R,.55)}</g>`+p('M9 20.5h11'),
 explode:p('M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5')+p('M12 7v2.5M12 14.5V17M7 12h2.5M14.5 12H17',Y),
 join:p('M3 18l6-6M15 12l6-6')+p('M9 12h6',G,2)+g(9,12)+g(15,12),
 array:p('M3.5 5h4.5v4.5H3.5z',B)+p('M9.75 5h4.5v4.5h-4.5zM16 5h4.5v4.5H16zM3.5 14.5h4.5V19H3.5zM9.75 14.5h4.5V19h-4.5zM16 14.5h4.5V19H16z'),
 match:p('M19.5 4.5l-6 6',undefined,2.2)+p('M11.5 9.5l3 3',undefined,1.8)+pf('M10.5 11.5l2 2-3.2 3.7c-1.4 1.4-3.3 2.3-5.3 2.8.5-2 1.4-3.9 2.8-5.3z',Y,.85),
 measure:`<g transform="rotate(-35 12 12)">${p('M2.5 9h19v6.5h-19z')}${p('M5.5 9v3M8.5 9v2M11.5 9v3M14.5 9v2M17.5 9v3',Y)}</g>`,
 'measure-area':pf('M4 18l3-12 9-2 4 9-7 7z',B,.25)+g(4,18)+g(7,6)+g(16,4)+g(20,13)+g(13,20),
 dist:p('M5 12.5v6M19 12.5v6',undefined,1.2)+dash('M7 17h10',B)+g(5,17)+g(19,17)+p('M5 8h14M7.5 5.5L5 8l2.5 2.5M16.5 5.5L19 8l-2.5 2.5',Y),
 id:p('M12 3v6M12 15v6M3 12h6M15 12h6')+cf(12,12,2.5,Y,.4),
 list:p('M5 3h14v18H5z')+p('M8 8h8M8 12h8M8 16h5',B),
 purge:p('M4 6.5h16M9.5 6.5V4h5v2.5')+p('M6 6.5l1 14h10l1-14')+p('M10 10.5v6.5M14 10.5v6.5',R),
 selectall:dash('M3 3h18v14H3z',B)+`<path d="M11 9v10.5l2.8-2.6 1.9 4 1.7-.8-1.9-3.9h3.8z" fill="currentColor" stroke="#1b1f26" stroke-width="1"/>`,
 copyclip:p('M8.5 7V3.5H17l3 3v10h-3')+pf('M4 7h11v13.5H4z',B,.15),
 cutclip:c(7,17.5,2.8,R)+c(17,17.5,2.8,R)+p('M8.8 15.4L17 4M15.2 15.4L7 4'),
 pasteclip:p('M8 4.5H5.5v16.5h13V4.5H16')+pf('M9 3h6v3.5H9z',Y,.5)+p('M8.5 11h7M8.5 14.5h7M8.5 18h4',B),
 // ---- 3D ----------------------------------------------------------------------------
 box:pf(K.top,B,.35)+p(K.edges),
 mesh:p(K.edges)+p('M4.17 7.5L12 21M12 12l7.83-4.5L12 3',B,1),
 extrude:pf('M12 13l8 4-8 4-8-4z',B,.35)+dash('M12 3l8 4-8 4-8-4zM4 7v10M20 7v10')+p('M12 17V7.5',Y,1.8)+p('M9.5 10L12 7.5l2.5 2.5',Y,1.8),
 union:pf('M12 6.8A6 6 0 1 0 12 17.2A6 6 0 1 0 12 6.8z',B,.35),
 subtract:pf('M12 6.8A6 6 0 1 0 12 17.2A6 6 0 0 1 12 6.8z',B,.35)+dash('M12 6.8A6 6 0 1 1 12 17.2'),
 intersect:dash('M12 6.8A6 6 0 1 0 12 17.2M12 6.8A6 6 0 1 1 12 17.2')+pf('M12 6.8A6 6 0 0 1 12 17.2A6 6 0 0 1 12 6.8z',B,.4),
 update:p(cube(8,12,6).edges)+f(cube(8,12,6).top,B,.35)+p('M21.5 13A4 4 0 1 1 20.33 10.17',G)+p('M17.33 10.17h3v-3',G),
 'mesh-move':p('M12 13V4',B)+p('M12 13l7.5 4.3',AX)+p('M12 13l-7.5 4.3',AY)+p('M10 6l2-2.5L14 6',B)+p('M16.55 17.21L19.5 17.3l-1.55-2.51',AX)+p('M7.45 17.21L4.5 17.3l1.55-2.51',AY)+dot(12,13,1.6),
 'mesh-copy':p(cube(7.5,8,4.5).edges)+pf(cube(16.5,16,4.5).top,B,.35)+p(cube(16.5,16,4.5).edges,B),
 'mesh-clear':p(cube(10,11,6.5).edges)+p('M16 15l5 5M21 15l-5 5',R,1.8),
 'mesh-fit':p(cube(12,12,5.5).edges)+p('M3 8V3h5M16 3h5v5M21 16v5h-5M8 21H3v-5',B),
 fallback:dash('M5 5h14v14H5z')+dot(12,12,1.6)
};
// Command names and synonyms that reuse a drawn glyph.
const ALIAS={pline:'polyline',pl:'polyline',rectang:'rectangle',rec:'rectangle',dtext:'text',mt:'mtext',bhatch:'hatch',
 measuregeom:'measure',mea:'measure',area:'measure-area',matchprop:'match',arrayrect:'array',ar:'array',
 '3dorbit':'orbit',vscurrent:'visual-style',dynmode:'dyn',cleanscreenon:'clean',cleanscreenoff:'clean',commandlinehide:'commandline',
 propertiesclose:'properties',props:'properties','layer-props':'layer-properties',la:'layer-properties',
 ribbonclose:'ribbon-min',wscurrent:'workspace',qsave:'save',print:'plot',dxfout:'file-dxf',dxfin:'import',svgout:'file-svg',
 stlout:'file-stl',objexport:'file-obj',pdf:'file-pdf',exportpdf:'file-pdf',updateextrusion:'update','3dmove':'mesh-move',meshcopy:'mesh-copy',
 meshclear:'mesh-clear',meshfit:'mesh-fit',delete:'erase',dim:'dimlinear',ai_selall:'selectall',settings2:'gear',start:'home',
 'vs-2dwire':'vs-2dwireframe',filetab:'filetabs',layouttab:'layouttabs',undo2:'undo',zoomextents:'zoom-extents',
 'view-sw':'view-iso','view-se':'view-iso','view-ne':'view-iso','view-nw':'view-iso',cad:'app',logo:'app'};
CF.icon=function(name,size=20,extra=''){const key=String(name??'').toLowerCase(),body=I[key]||I[ALIAS[key]]||I.fallback;
 return `<svg class="cf-icon${extra?' '+extra:''}" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">${body}</svg>`};
CF.hasIcon=name=>{const key=String(name??'').toLowerCase();return !!(I[key]||I[ALIAS[key]])};
CF.iconNames=[...Object.keys(I).filter(k=>k!=='fallback'),...Object.keys(ALIAS)].sort();
CF.module('icons');
}
