// Palettes: Properties geometry edits + undo, layer manager rules, osnap/polar settings, status bar toggles, layer dropdown.
module.exports=async({run,assert})=>{
 if(!await run("return typeof CF!=='undefined'&&CF.has('palettes')")){console.log('SKIP: acad-palettes');return}
 await run(`
 const P=CF.palettes,row=(m,k)=>{for(const c of m.cats)for(const r of c.rows)if(r.k===k)return r;return null};
 CF.toggle('properties',true);
 // ---- Line geometry: edits apply through mutate (one undo step each) and are undoable.
 doc.entities=[{type:'line',layer:'0',points:[{x:0,y:0},{x:10,y:0}]},{type:'circle',layer:'0',center:{x:5,y:5},radius:2},{type:'polyline',layer:'0',closed:false,points:[{x:0,y:0},{x:10,y:0},{x:10,y:10}]},{type:'text',layer:'0',points:[{x:1,y:2}],text:'Hi',height:5}];
 history=[];future=[];CF.select([0]);
 let m=P.model();assert.equal(m.options[0][1],'Line');assert.equal(row(m,'ex').value,'10.0000');assert.equal(row(m,'len').value,'10.0000');assert.equal(row(m,'ang').value,'0');assert.ok(row(m,'dx').ro);
 assert.equal(P.edit('ex','30'),true);assert.equal(doc.entities[0].points[1].x,30);assert.equal(history.length,1);
 undo();assert.equal(doc.entities[0].points[1].x,10);undo(true);assert.equal(doc.entities[0].points[1].x,30);
 CF.select([0]);assert.equal(P.edit('len','20'),true);assert.ok(Math.abs(doc.entities[0].points[1].x-20)<1e-9);
 assert.equal(P.edit('ang','90'),true);assert.ok(Math.abs(doc.entities[0].points[1].x)<1e-9&&Math.abs(doc.entities[0].points[1].y-20)<1e-9);assert.deepEqual(doc.entities[0].points[0],{x:0,y:0});
 const h=history.length;assert.notEqual(P.edit('len','-5'),true);assert.notEqual(P.edit('sx','abc'),true);assert.equal(history.length,h);assert.notEqual(P.edit('dx','3'),true);
 // ---- Circle
 CF.select([1]);m=P.model();assert.equal(row(m,'area').value,(Math.PI*4).toFixed(4));
 assert.equal(P.edit('d','10'),true);assert.equal(doc.entities[1].radius,5);assert.equal(P.edit('cx','-3'),true);assert.equal(doc.entities[1].center.x,-3);assert.notEqual(P.edit('r','0'),true);undo();assert.equal(doc.entities[1].center.x,5);
 // ---- Polyline vertex spinner + closed flag
 CF.select([2]);m=P.model();assert.equal(row(m,'vtx').max,3);P.vertex.set(1);assert.equal(P.edit('vy','4'),true);assert.equal(doc.entities[2].points[1].y,4);assert.equal(doc.entities[2].points[0].y,0);
 P.vertex.step(1);m=P.model();assert.equal(row(m,'vtx').value,3);assert.equal(row(m,'area'),null);
 assert.equal(P.edit('closed','Yes'),true);assert.equal(doc.entities[2].closed,true);m=P.model();assert.ok(row(m,'area'));undo();assert.equal(doc.entities[2].closed,false);
 // ---- Text
 CF.select([3]);assert.equal(P.edit('txt','Hello'),true);assert.equal(doc.entities[3].text,'Hello');assert.equal(P.edit('h','2.5'),true);assert.equal(doc.entities[3].height,2.5);assert.notEqual(P.edit('txt','  '),true);
 // ---- Multiple selection: All (n) + per-type filter with *VARIES*; layer applies to every selected object.
 doc.layers.push({name:'A',color:'#ff0000',visible:true});syncLayers();
 doc.entities.push({type:'line',layer:'0',points:[{x:0,y:5},{x:4,y:5}]});CF.select([0,1,4]);m=P.model();
 assert.equal(m.options[0][1],'All (3)');assert.ok(m.options.some(o=>o[1]==='Line (2)'));assert.equal(row(m,'ex'),null);assert.equal(row(m,'layer').value,'0');
 P.filter.set('line');m=P.model();assert.equal(row(m,'ex').value,'*VARIES*');assert.equal(row(m,'sx').value,'0.0000');
 assert.equal(P.edit('layer','A'),true);assert.equal(doc.entities[0].layer,'A');assert.equal(doc.entities[4].layer,'A');assert.equal(doc.entities[1].layer,'0');
 P.filter.set('all');m=P.model();assert.equal(row(m,'layer').value,'*VARIES*');
 // ---- Block references count as one object
 doc.entities.push({type:'line',layer:'0',group:'gB',blockName:'Bolt',points:[{x:0,y:0},{x:1,y:0}]},{type:'circle',layer:'0',group:'gB',blockName:'Bolt',center:{x:0,y:0},radius:1});
 CF.select([5]);m=P.model();assert.equal(m.options[0][1],'Block Reference');assert.equal(row(m,'name').value,'Bolt');assert.equal(P.edit('px','10'),true);assert.equal(doc.entities[5].points[0].x,11);assert.equal(doc.entities[6].center.x,11);
 // ---- Nothing selected: drawing summary and drafting settings; palette DOM builds without errors.
 CF.deselect();m=P.model();assert.equal(m.filter,'none');assert.ok(row(m,'objects'));assert.ok(row(m,'osnap'));P.refresh();render();
 mode3D=true;doc.solids=[makeExtrusion([{x:0,y:0},{x:10,y:0},{x:10,y:10},{x:0,y:10}],10,'Cube')];$('meshA').value='0';m=P.model();assert.equal(row(m,'volume').value,'1000.0000');assert.equal(row(m,'tris').value,'12');assert.ok(row(m,'volume').ro&&row(m,'objects').ro);assert.match(String(P.edit('volume','5')),/read-only/);P.refresh();mode3D=false;doc.solids=[];
 CF.toggle('properties',false);
 `);
 console.log('PASS: properties palette line/circle/polyline/text/block geometry edits with undo, validation, multi-selection filter, 3D mesh stats');
 await run(`
 const L=CF.palettes.layers;history=[];future=[];
 doc={layers:[{name:'0',color:'#ffffff',visible:true},{name:'Walls',color:'#ff0000',visible:true},{name:'Empty',color:'#00ff00',visible:true}],entities:[{type:'line',layer:'Walls',points:[{x:0,y:0},{x:1,y:0}]},{type:'line',layer:'0',points:[{x:0,y:1},{x:1,y:1}]}],blocks:{Door:{items:[{type:'line',layer:'Walls',points:[{x:0,y:0},{x:2,y:0}]}]}}};
 syncLayers();assert.equal(L.setCurrent('Walls'),true);assert.equal($('layer').value,'Walls');
 // Rename updates entities, block definitions and the current layer; undo restores everything.
 assert.equal(L.rename('Walls','A-WALL'),true);assert.equal(doc.entities[0].layer,'A-WALL');assert.equal(doc.blocks.Door.items[0].layer,'A-WALL');assert.equal($('layer').value,'A-WALL');assert.ok(!doc.layers.some(l=>l.name==='Walls'));validate(doc);
 undo();assert.equal(doc.entities[0].layer,'Walls');assert.equal(doc.blocks.Door.items[0].layer,'Walls');assert.equal($('layer').value,'Walls');undo(true);assert.equal($('layer').value,'A-WALL');
 assert.notEqual(L.rename('0','Zero'),true);assert.notEqual(L.rename('Empty','a-wall'),true);assert.notEqual(L.rename('Empty','Bad*Name'),true);assert.notEqual(L.rename('Empty',''),true);
 // Delete rules: 0, current and layers with objects (or block references) are protected.
 assert.notEqual(L.remove('0'),true);assert.notEqual(L.remove('A-WALL'),true);L.setCurrent('0');assert.match(L.remove('A-WALL'),/contains 1 object/);
 doc.entities.shift();assert.match(L.remove('A-WALL'),/block/);
 assert.equal(L.remove('Empty'),true);assert.ok(!doc.layers.some(l=>l.name==='Empty'));undo();assert.ok(doc.layers.some(l=>l.name==='Empty'));
 // New layer naming, colour and visibility (all undoable).
 const n=L.nextName();assert.equal(n,'Layer1');assert.equal(L.add(n),true);assert.ok(doc.layers.some(l=>l.name==='Layer1'));assert.equal(L.nextName(),'Layer2');
 assert.equal(L.setColor('Layer1','#00FFFF'),true);assert.equal(doc.layers.find(l=>l.name==='Layer1').color,'#00ffff');assert.equal(L.colorName('#00ffff'),'cyan');assert.equal(L.colorName('#63d9c0'),'99,217,192');
 assert.equal(L.setVisible('Layer1',false),true);assert.equal(doc.layers.find(l=>l.name==='Layer1').visible,false);undo();assert.equal(doc.layers.find(l=>l.name==='Layer1').visible,true);
 // Layer Properties Manager dialog and the LAYER command.
 CF.openLayerManager();const d=CF.palettes.layerManager.dialog();assert.ok(d.open);assert.equal(d.id,'cf-layer-manager');CF.palettes.layerManager.select('Layer1');CF.palettes.layerManager.setCurrent();assert.equal($('layer').value,'Layer1');
 CF.palettes.layerManager.deleteLayer();assert.ok(doc.layers.some(l=>l.name==='Layer1'));d.close();CF.run('LAYER');assert.ok(d.open);d.close();L.setCurrent('0');
 // Ribbon layer dropdown factory: independent instances that follow the current layer.
 const a=CF.createLayerDropdown(),b=CF.createLayerDropdown();assert.ok(a&&b&&a!==b);assert.ok(CF.palettes.dropdowns.has(a));L.setCurrent('Layer1');render();assert.equal($('layer').value,'Layer1');
 `);
 console.log('PASS: layer rename (entities, blocks, current) with undo, rename/delete rules, new layer, colour/visibility, Layer Properties Manager, layer dropdown');
 await run(`
 const P=CF.palettes,S=P.status;
 // Object snap modes and polar increment (read by the interaction module).
 assert.ok(CF.osnapModes instanceof Set);P.setOsnapMode('nearest',true);assert.ok(CF.osnapModes.has('nearest'));P.setOsnapMode('nearest',false);assert.ok(!CF.osnapModes.has('nearest'));assert.equal(P.setOsnapMode('bogus',true),false);
 assert.equal(P.setPolarIncrement(30),true);assert.equal(CF.polarIncrement,30);assert.equal(P.setPolarIncrement(-5),false);assert.equal(CF.polarIncrement,30);P.setPolarIncrement(45);
 P.menus.osnap(S.osnap);P.menus.polar(S.polar);P.menus.workspace(S.workspace);P.menus.close();
 // Status bar toggles call CF.toggle and mirror the state.
 const orig=CF.toggle,calls=[];CF.toggle=function(n,v,o){calls.push(n);return orig(n,v,o)};
 try{const g=CF.get('grid');S.grid.click();assert.equal(CF.get('grid'),!g);assert.equal(S.grid.classList.contains('on'),!g);S.grid.click();
  S.ortho.click();assert.ok(CF.get('ortho'));assert.ok(S.ortho.classList.contains('on'));S.polar.click();assert.ok(CF.get('polar'));assert.ok(!CF.get('ortho'));assert.ok(!S.ortho.classList.contains('on'));S.polar.click();
  S.snap.click();S.snap.click();S.osnap.click();S.osnap.click();S.dyn.click();S.dyn.click();S.properties.click();assert.ok(CF.get('properties'));S.properties.click();
  assert.deepEqual(calls,['grid','grid','ortho','polar','polar','snap','snap','osnap','osnap','dyn','dyn','properties','properties'])}finally{CF.toggle=orig}
 // Model / Layout tabs and MODEL/PAPER button.
 S.layoutTab.click();assert.equal(CF.space,'layout');assert.equal(S.model.textContent,'PAPER');S.modelTab.click();assert.equal(CF.space,'model');assert.equal(S.model.textContent,'MODEL');
 mouse={x:12.5,y:-3};render();assert.equal(S.coords.textContent,'12.5000, -3.0000, 0.0000');
 // Drafting Settings dialog applies modes.
 CF.openDraftingSettings('osnap');const parts=P.draftingSettings.parts();parts.modes.perpendicular.input.checked=true;parts.inc.value='15';P.draftingSettings.apply();assert.ok(CF.osnapModes.has('perpendicular'));assert.equal(CF.polarIncrement,15);P.setOsnapMode('perpendicular',false);P.setPolarIncrement(45);
 for(const n of ['LAYER','LA','PROPERTIES','PROPERTIESCLOSE','DSETTINGS'])assert.ok(CF.resolve(n),n);
 `);
 console.log('PASS: osnap/polar settings, status bar toggles via CF.toggle, Model/Layout tabs, coordinates readout, Drafting Settings, palette commands');
};
