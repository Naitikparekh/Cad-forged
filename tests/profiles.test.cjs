// Extrude dialog: every outline gets a distinguishing, position-based label and the chosen one is highlighted.
module.exports=async({run,assert})=>{
 if(!await run("return typeof profileLabel==='function'&&typeof extrusionOutlines!=='undefined'")){console.log('SKIP: profiles');return}
 await run(`
 clearSelection();mode3D=false;view={x:0,y:0,scale:4};W=1000;H=700;
 const sq=(x,y,w,h,layer='0')=>({type:'polyline',closed:true,layer,points:[{x,y},{x:x+w,y},{x:x+w,y:y+h},{x,y:y+h}]});
 // Loose lines forming a loop sit after a leading entity: the label must not leak the raw entity index (5).
 doc.entities=[{type:'line',layer:'0',points:[{x:-50,y:-50},{x:-40,y:-50}]},{type:'circle',layer:'0',center:{x:0,y:0},radius:3},{type:'line',layer:'0',points:[{x:0,y:20},{x:10,y:20}]},{type:'line',layer:'0',points:[{x:10,y:20},{x:10,y:40}]},{type:'line',layer:'0',points:[{x:10,y:40},{x:0,y:40}]},{type:'line',layer:'0',points:[{x:0,y:40},{x:0,y:20}]}];
 await extrude();
 let opts=$('extrusionProfile').children;
 assert.equal(opts.length,2);
 assert.equal(opts[0].text,'1 of 2: Circle Ø6 at (-3, -3) · layer 0');
 assert.equal(opts[1].text,'2 of 2: Closed outline 10 × 20 at (0, 20) · layer 0');
 assert.ok(!opts.some(o=>/^[56]:/.test(o.text)));
 extrusionDialog.close();
 // Two closed polylines on the same layer must be told apart by size/position and carry stable 1-based positions.
 doc.entities=[sq(0,0,100,60),sq(200,50,30,30)];clearSelection();
 await extrude();
 opts=$('extrusionProfile').children;
 assert.equal(opts.map(o=>o.text).join('|'),'1 of 2: Closed outline 100 × 60 at (0, 0) · layer 0|2 of 2: Closed outline 30 × 30 at (200, 50) · layer 0');
 assert.equal(new Set(opts.map(o=>o.text)).size,opts.length);
 // The most recently drawn outline is preselected and drawn as the canvas highlight; changing the choice moves it.
 assert.equal($('extrusionProfile').value,opts[1].value);
 assert.equal(extrusionOutlines.get(opts[1].value).length,4);
 $('extrusionProfile').value=opts[0].value;
 assert.equal(extrusionOutlines.get($('extrusionProfile').value)[2].x,100);
 extrusionDialog.close();
 `);
 console.log('PASS: extrude dialog outline labels are unique, position-indexed and describe size/position; highlight geometry follows the choice');
};
