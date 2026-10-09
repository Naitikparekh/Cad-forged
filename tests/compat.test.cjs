// Compatibility: SVG export covers the drawing extents (not the screen); DXF layer colours use AutoCAD Color Index.
module.exports=async({run,assert})=>{
 if(!await run("return typeof dxfRecords==='function'&&typeof drawingSVG==='function'&&typeof hexToAci==='function'")){console.log('SKIP: compatibility');return}
 await run(`
 mode3D=false;W=1000;H=700;
 const shot=()=>{const real=download;let got=null;download=(n,t,ty)=>{got={n,t,ty}};try{$('svg').onclick()}finally{download=real}return got};
 const box=s=>s.match(/viewBox="([^"]+)"/)[1].split(' ').map(Number);
 const sq=(x,y,w,h,layer='0')=>({type:'polyline',closed:true,layer,points:[{x,y},{x:x+w,y},{x:x+w,y:y+h},{x,y:y+h}]});
 doc={layers:[{name:'0',color:'#63d9c0',visible:true},{name:'Off',color:'#ff0000',visible:false}],entities:[sq(0,0,30,20),{type:'circle',layer:'0',center:{x:15,y:10},radius:2},sq(500,500,5,5,'Off')]};
 // Same extents regardless of pan, zoom, canvas size or 3D mode.
 const ref=shot();assert.equal(ref.n,'drawing.svg');assert.equal(ref.ty,'image/svg+xml');
 const [bx,by,bw,bh]=box(ref.t);
 assert.ok(bx<0&&bx>-5&&by<-20&&by>-25,'margin around extents (y flipped): '+box(ref.t));
 assert.ok(Math.abs(bx+bx+bw-30)<1e-6&&Math.abs(2*by+bh+20)<1e-6,'viewBox centred on the drawing');
 assert.ok(bw>30&&bh>20&&bw<40);
 assert.ok(!ref.t.includes('500'),'hidden layer excluded from extents and output');
 for(const [v,w,h,m3]of [[{x:200,y:200,scale:5},1000,700,false],[{x:-9e3,y:4,scale:.01},300,200,false],[{x:0,y:0,scale:100},1000,700,true]]){view=v;W=w;H=h;mode3D=m3;const s=shot();assert.equal(s.t,ref.t)}
 mode3D=false;W=1000;H=700;
 // Stroke width follows the drawing size, not the zoom.
 view={x:0,y:0,scale:1000};const sw=Number(shot().t.match(/stroke-width="([^"]+)"/)[1]);view={x:0,y:0,scale:.1};assert.equal(Number(shot().t.match(/stroke-width="([^"]+)"/)[1]),sw);assert.ok(sw>0.01&&sw<1);
 // Text extents count; tiny drawings keep a non-zero stroke; empty drawing still yields a valid viewBox.
 doc.entities=[{type:'text',layer:'0',points:[{x:10,y:5}],height:2,text:'ABCDEFGHIJ'}];
 let b=box(shot().t);assert.ok(b[0]<=10&&b[0]+b[2]>=10+10*2*.6&&-b[1]>=5+2&&-(b[1]+b[3])<=5);
 doc.entities=[{type:'line',layer:'0',points:[{x:0,y:0},{x:.01,y:.02}]}];
 const tiny=shot().t;assert.ok(Number(tiny.match(/stroke-width="([^"]+)"/)[1])>0);assert.ok(box(tiny)[2]>0.01);
 doc.entities=[];b=box(shot().t);assert.ok(b[2]>0&&b[3]>0&&b.every(Number.isFinite));
 doc.entities=[{type:'text',layer:'0',points:[{x:0,y:0}],height:3,text:'a<b & "c"'}];assert.ok(shot().t.includes('a&lt;b &amp; &quot;c&quot;'));
 `);
 console.log('PASS: SVG export covers drawing extents with margin, independent of pan/zoom/canvas/3D, hidden layers excluded');
 await run(`
 // ACI table anchors (standard values) and nearest-colour mapping.
 const std={1:'#ff0000',2:'#ffff00',3:'#00ff00',4:'#00ffff',5:'#0000ff',6:'#ff00ff',7:'#ffffff',8:'#808080',9:'#c0c0c0',10:'#ff0000',11:'#ff7f7f',12:'#a50000',13:'#a55252',14:'#7f0000',15:'#7f3f3f',16:'#4c0000',17:'#4c2626',18:'#260000',19:'#261313',20:'#ff3f00',30:'#ff7f00',40:'#ffbf00',50:'#ffff00',60:'#bfff00',70:'#7fff00',90:'#00ff00',130:'#00ffff',170:'#0000ff',210:'#ff00ff',250:'#333333',251:'#5b5b5b',252:'#848484',253:'#adadad',254:'#d6d6d6',255:'#ffffff'};
 for(const k in std)assert.equal(aciToHex(+k),std[k],'ACI '+k);
 assert.equal(aciToHex(0),null);assert.equal(aciToHex(256),null);
 assert.equal(new Set(Array.from({length:255},(_,i)=>aciToHex(i+1))).size>=240,true);
 for(let i=1;i<=255;i++){const back=hexToAci(aciToHex(i));assert.equal(aciToHex(back),aciToHex(i),'round trip colour '+i)}
 assert.equal(hexToAci('#ffffff'),7);assert.equal(hexToAci('#FF0000'),1);assert.equal(hexToAci('#00f'),5);assert.equal(hexToAci('garbage'),7);
 assert.equal(hexToAci('#f1bd65'),31);// Markings orange in L-Bracket
 `);
 await run(`
 doc={layers:[{name:'0',color:'#ffffff',visible:true},{name:'Markings',color:'#f1bd65',visible:true},{name:'Red',color:'#ff0000',visible:false},{name:'Blue',color:'#0000ff',visible:true}],entities:[{type:'circle',layer:'Markings',center:{x:1,y:2},radius:3},{type:'line',layer:'Red',points:[{x:0,y:0},{x:5,y:5}]},{type:'text',layer:'Blue',points:[{x:1,y:1}],height:2,text:'hi'}]};
 const text=exportDXF();
 assert.ok(/^[\\x00-\\x7f]*$/.test(text),'ASCII DXF');assert.ok(text.includes('AC1009'));
 const lines=text.split('\\r\\n'),layerTable=lines.slice(lines.indexOf('TABLES'),lines.indexOf('ENTITIES'));
 const colourOf=n=>{const i=layerTable.findIndex((x,j)=>x===n&&layerTable[j-1]==='2'&&layerTable[j-2]==='LAYER');const k=layerTable.indexOf('62',i);return Number(layerTable[k+1])};
 assert.equal(colourOf('0'),7);assert.equal(colourOf('Markings'),31);assert.equal(colourOf('Red'),-1);assert.equal(colourOf('Blue'),5);
 const back=importDXF(text);assert.equal(back.skipped,0);
 const lay=Object.fromEntries(back.doc.layers.map(l=>[l.name,l]));
 assert.equal(lay['Markings'].color,aciToHex(31));assert.equal(lay['Markings'].visible,true);
 assert.equal(lay['Red'].color,'#ff0000');assert.equal(lay['Red'].visible,false);
 assert.equal(lay['Blue'].color,'#0000ff');assert.equal(lay['0'].color,'#ffffff');
 assert.equal(back.doc.entities.length,3);assert.equal(back.doc.entities.find(e=>e.type==='circle').layer,'Markings');
 // Raw ACI layers from other CAD programs map to hex; negative = layer off; missing/invalid colour keeps the default.
 const rec=(n,c)=>({type:'LAYER',pairs:[[2,n],[70,0],...(c===undefined?[]:[[62,c]]),[6,'CONTINUOUS']]});
 const foreign=dxfWrapped([],[{type:'TABLE',pairs:[[2,'LAYER'],[70,5]]},rec('A',1),rec('B',-3),rec('C',140),rec('D'),rec('E',256),{type:'ENDTAB',pairs:[]}]);
 const fl=Object.fromEntries(importDXF(foreign).doc.layers.map(l=>[l.name,l]));
 assert.equal(fl.A.color,'#ff0000');assert.equal(fl.B.color,'#00ff00');assert.equal(fl.B.visible,false);assert.equal(fl.C.color,aciToHex(140));
 assert.equal(fl.D.color,'#ffffff');assert.equal(fl.D.visible,true);assert.equal(fl.E.color,'#63d9c0');
 // Block instances keep layer colours through the compatibility exporter/importer too.
 doc.entities=[{type:'circle',layer:'Markings',group:'g',center:{x:0,y:0},radius:1},{type:'line',layer:'Blue',group:'g',points:[{x:0,y:0},{x:4,y:0}]}];
 const grouped=importDXF(exportDXF());assert.equal(grouped.doc.layers.find(l=>l.name==='Markings').color,aciToHex(31));assert.equal(grouped.doc.entities[0].group,grouped.doc.entities[1].group);
 `);
 console.log('PASS: DXF layer colours export as nearest ACI (negative = off), import maps ACI to hex, standard ACI table anchors, block instances keep colours');
};
