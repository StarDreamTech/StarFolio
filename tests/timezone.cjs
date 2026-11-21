// Use the shelf and reader's actual event handlers; no network, app storage or real archive.
const assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs');
const saved='2025-02-04T02:33:44.123456+00:00', posted='2025-02-04T02:22:11.123Z';
const reply='2025-07-04T09:10:12+02:00', undated='2025-02-04T02:22:11';
const code=fs.readFileSync('ui/app.js','utf8'), markup=fs.readFileSync('ui/index.html','utf8');
class Element{
  value='';children=[];open=false;
  get options(){return this.children}
  append(...v){this.children.push(...v)} add(v){this.append(v)}
  replaceChildren(...v){this.children=v;this.value=''}
  showModal(){this.open=true} close(){this.open=false}
  querySelectorAll(){return []}
}
const store=new Map();
function app(){
  const elements=new Map();
  const el=id=>{if(!elements.has(id))elements.set(id,new Element());return elements.get(id)};
  const times=[saved,posted,reply,undated].map(dateTime=>({dateTime}));
  const frame={head:new Element(),createElement:()=>new Element(),querySelector:()=>new Element(),addEventListener(){},querySelectorAll:sel=>sel==='time[data-archive-time]'?times:[]};
  el('frame').contentDocument=frame;
  const window={localStorage:{getItem:k=>store.get(k),setItem:(k,v)=>store.set(k,v)},__TAURI__:{core:{invoke:async cmd=>cmd==='catalog'?['/synthetic',[{id:'sample',site:'example.org',title:'Example',saved,posts:3,images:0}]]:{html:'',css:'',site_css:''}}}};
  const document={getElementById:el,createElement:()=>new Element()};
  const ctx={window,document,Option:class{constructor(text,value){this.text=text;this.value=value}},IntersectionObserver:class{disconnect(){}},setTimeout:()=>0,clearTimeout(){}};
  vm.runInNewContext(code,ctx);
  return {el,times,value:()=>el('items').children[0].children[1].textContent,eval:s=>vm.runInNewContext(s,ctx)};
}
(async()=>{
  for(const id of ['libraryZone','readerZone','zonePanel','timeZone','saveZone'])assert.ok(markup.includes(`id="${id}"`));
  const a=app(); await new Promise(setImmediate);
  a.el('libraryZone').onclick(); assert.equal(a.el('zonePanel').open,true);
  assert.equal(a.el('timeZone').value,'');
  a.el('timeZone').value='Asia/Shanghai'; a.el('saveZone').onclick();
  assert.equal(a.el('zonePanel').open,false);
  assert.match(a.value(),/归档于 2025-02-04 10:33:44 UTC\+08:00/);
  await a.el('items').children[0].onclick(); a.el('frame').onload();
  assert.equal(a.times[1].textContent,'2025-02-04 10:22:11 UTC+08:00');
  assert.equal(a.times[1].dateTime,posted);
  assert.equal(a.times[1].title,`原始时间：${posted}`);
  assert.equal(a.times[3].textContent,undated);
  a.el('readerZone').onclick(); assert.equal(a.el('timeZone').value,'Asia/Shanghai');
  a.el('timeZone').value='America/New_York'; a.el('saveZone').onclick();
  assert.equal(a.times[0].textContent,'2025-02-03 21:33:44 UTC-05:00');
  assert.equal(a.times[2].textContent,'2025-07-04 03:10:12 UTC-04:00');
  assert.match(a.value(),/归档于 2025-02-03 21:33:44 UTC-05:00/);
  const b=app(); await new Promise(setImmediate);
  assert.match(b.value(),/2025-02-03 21:33:44 UTC-05:00/);
  b.el('libraryZone').onclick(); assert.equal(b.el('timeZone').value,'America/New_York');
  b.el('timeZone').value='UTC'; b.el('saveZone').onclick();
  assert.match(b.value(),/归档于 2025-02-04 02:33:44 UTC\+00:00/);
  assert.equal(b.eval(`displayTime('2025-02-30T02:22:11Z')`),'2025-02-30T02:22:11Z');
  assert.equal(b.eval(`displayTime('2025-02-04')`),'2025-02-04');
  store.set('starfolio.displayTimeZone','bad/zone'); const c=app(); await new Promise(setImmediate);
  c.el('libraryZone').onclick(); assert.equal(c.el('timeZone').value,'');
  console.log('Display timezone: shelf, reader, UTC+8, UTC, DST, fallback and persistence OK');
})().catch(e=>{console.error(e);process.exitCode=1});
