// Shared workspace contract: registry, flags, selection helpers and geometry.
module.exports=async({run,assert})=>{
 if(!await run("return typeof CF!=='undefined'&&CF.has('core')")){console.log('SKIP: acad-core');return}
 await run(`
 let ran=0;CF.register({name:'cftest',aliases:['ct'],run:()=>{ran++}});
 assert.equal(CF.resolve('CT').name,'CFTEST');assert.equal(CF.resolve('_cftest').name,'CFTEST');assert.equal(CF.resolve('nope'),null);
 assert.ok(CF.run('ct'));assert.equal(ran,1);assert.equal(CF.lastCommand,'CFTEST');assert.equal(CF.run('nope'),false);
 CF.register({name:'cftest',aliases:['c2'],run:()=>{ran+=10}});assert.equal(CF.resolve('ct'),null);CF.run('c2');assert.equal(ran,11);CF.commands.delete('CFTEST');CF.aliases.delete('C2');
 assert.equal(CF.get('snap'),false);CF.toggle('snap');assert.equal($('snap').checked,true);CF.toggle('snap',false);
 CF.set('polar',true);CF.set('ortho',true);assert.equal(CF.get('polar'),false);CF.set('polar',true);assert.equal(CF.get('ortho'),false);CF.set('polar',false);
 CF.toggle('properties',true);assert.ok(document.body.classList.contains('cf-palette-open'));CF.toggle('properties',false);
 doc.entities=[{type:'line',layer:'0',points:[{x:0,y:0},{x:10,y:0}]},{type:'line',layer:'0',group:'g1',points:[{x:0,y:5},{x:10,y:5}]},{type:'line',layer:'0',group:'g1',points:[{x:0,y:6},{x:10,y:6}]}];
 CF.select([0]);assert.deepEqual(CF.selection(),[0]);CF.select([1],'add');assert.deepEqual(CF.selection().sort(),[0,1,2]);
 CF.select([2],'remove');assert.deepEqual(CF.selection(),[0]);CF.deselect();assert.equal(selected,-1);
 const g=CF.geom;const x=g.segmentIntersection({x:0,y:0},{x:10,y:10},{x:0,y:10},{x:10,y:0});assert.ok(Math.abs(x.x-5)<1e-9&&Math.abs(x.t-.5)<1e-9);
 assert.equal(g.segmentIntersection({x:0,y:0},{x:1,y:0},{x:5,y:-1},{x:5,y:1}),null);assert.ok(g.segmentIntersection({x:0,y:0},{x:1,y:0},{x:5,y:-1},{x:5,y:1},true));
 assert.equal(g.lineCircle({x:-10,y:0},{x:10,y:0},{x:0,y:0},5).length,2);assert.equal(g.circleCircle({x:0,y:0},5,{x:8,y:0},5).length,2);
 const rect={type:'polyline',closed:true,layer:'0',points:[{x:0,y:0},{x:10,y:0},{x:10,y:10},{x:0,y:10}]};assert.equal(g.segments(rect).length,4);
 assert.equal(g.intersections(rect,{type:'line',points:[{x:-5,y:5},{x:15,y:5}]}).length,2);assert.equal(g.intersections(rect,{type:'circle',center:{x:0,y:0},radius:3}).length,2);
 const b=g.bbox({type:'circle',center:{x:1,y:2},radius:3});assert.equal(b.minX,-2);assert.equal(b.maxY,5);
 CF.setSpace('layout');assert.equal(CF.space,'layout');setTool('line');assert.equal(CF.space,'model');setTool('select');
 doc.entities=[];mutate(()=>doc.entities.push({type:'line',layer:'0',points:[{x:0,y:0},{x:1,y:1}]}));assert.ok(CF.modified);CF.markSaved();assert.ok(!CF.modified);
 const css=cfStyle.textContent,root=css.match(/:root\\{[^}]*\\}/)[0];
 assert.ok(/color-scheme:dark/.test(root),'root declares color-scheme:dark');assert.ok(/scrollbar-color:var\\(--cf-scroll-thumb\\)/.test(root),'root themes scrollbars');
 assert.ok(/--cf-scroll-thumb:#/.test(root)&&/--cf-scroll-track:#/.test(root),'scrollbar tokens defined');assert.ok(/::-webkit-scrollbar-thumb/.test(css),'webkit scrollbar fallback present');
 `);
 console.log('PASS: workspace contract registry, aliases, toggles, exclusive ortho/polar, selection helpers, geometry helpers, spaces and modified state');
};
