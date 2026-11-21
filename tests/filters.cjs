// Test the same DOM-wired controls loaded by Tauri, without a network or model.
const assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs');
class Element{value='';children=[];get childElementCount(){return this.children.length}get options(){return this.children}replaceChildren(...v){this.children=v;this.value=''}append(...v){this.children.push(...v)}add(v){this.append(v)}focus(){this.focused=true}scrollIntoView(){this.scrolled=true}}
const els=new Map(),el=id=>{if(!els.has(id))els.set(id,new Element());return els.get(id)};
const rows=[
  ['X','x.com','社交帖子',['科学'],'x.com:account:one','Shared (@one)'],
  ['Paper','else.example','随笔',['科学','艺术'],'else.example:byline:Shared','Shared'],
  ['Older','x.com','随笔',[],'x.com:account:one','Shared (@one)'],
  ['Second','x.com','社交帖子',['艺术'],'x.com:account:two','Shared (@two)'],
  ['No byline','x.com','随笔',[],null],
].map(([title,site,category,tags,key,label],id)=>({title,site,category,tags,author:key?{key,label}:null,id,saved:'2025-01-01'}));
const markup=fs.readFileSync('ui/index.html','utf8');
for(const id of ['author','site','category','tag','find','clearFilters','pager','prevPage','nextPage','pageNumber'])assert.ok(markup.includes(`id="${id}"`));
const window={scrollY:320,scrollTo(x,y){this.scrollY=y},__TAURI__:{core:{invoke:async cmd=>cmd==='catalog'?['library',rows]:{html:'',css:'',site_css:''}}}};
const ctx=vm.createContext({document:{getElementById:el,createElement:()=>new Element()},Option:class{constructor(text,value){this.text=text;this.value=value}},window,clearTimeout,setTimeout});
vm.runInContext(fs.readFileSync('ui/app.js','utf8'),ctx);
setImmediate(async()=>{
  const count=n=>assert.equal(el('items').childElementCount,n);
  const options=()=>el('author').options.map(o=>o.text);
  const set=(id,value)=>{el(id).value=value;(el(id).onchange||el(id).oninput)()};
  count(5);
  assert.equal(options()[1],'Shared (@one) · 2 篇');
  set('site','x.com'); count(4);
  assert.deepEqual(options(),['全部作者','Shared (@one) · 2 篇','Shared (@two) · 1 篇','未记录作者 · 1 篇']);
  set('author','x.com:account:two'); count(1);
  assert.equal(el('items').children[0].children[0].textContent,'Second');
  set('tag','科学'); count(0);
  assert.deepEqual(options(),['全部作者','Shared (@one) · 1 篇','Shared (@two) · 0 篇']);
  set('author','x.com:account:one'); count(1);
  set('category','随笔'); count(0);
  set('tag','__untagged'); count(1);
  set('author','__unknown_author'); count(1);
  assert.equal(el('items').children[0].children[0].textContent,'No byline');
  set('find','@one'); count(0);
  el('clearFilters').onclick(); count(5);
  set('find','@one'); count(2);
  set('author','x.com:account:one'); count(2);
  set('site','else.example'); count(0);
  assert.equal(el('author').value,'');
  set('find',''); count(1);
  assert.deepEqual(options(),['全部作者','Shared · 1 篇']);
  assert.equal(el('pager').hidden,true);
  el('clearFilters').onclick();
  rows.push(...Array.from({length:38},(_,i)=>({...rows[0],id:5+i,title:`Extra ${i+1}`})));
  await vm.runInContext('refresh()',ctx);
  count(20);
  assert.equal(el('pager').hidden,false);
  assert.equal(el('prevPage').disabled,true);
  assert.equal(options()[1],'Shared (@one) · 40 篇');
  el('prevPage').onclick();
  assert.equal(el('pageNumber').textContent,'第 1 / 3 页');
  el('nextPage').onclick(); count(20);
  assert.equal(el('shelfTitle').scrolled,true);
  assert.equal(el('count').textContent,'显示第 21–40 篇 · 匹配 43 篇 / 全部 43 篇');
  el('nextPage').onclick(); count(3);
  assert.equal(el('nextPage').disabled,true);
  el('nextPage').onclick();
  await el('items').children[0].onclick();
  window.scrollY=0;
  el('back').onclick(); count(3);
  assert.equal(window.scrollY,320);
  assert.equal(el('pageNumber').textContent,'第 3 / 3 页');
  assert.equal(el('items').children[0].children[0].textContent,'Extra 36');
  el('prevPage').onclick(); count(20);
  set('site','x.com');
  assert.equal(el('pageNumber').textContent,'第 1 / 3 页');
  el('nextPage').onclick();
  set('author','x.com:account:one');
  assert.equal(el('pageNumber').textContent,'第 1 / 2 页');
  el('nextPage').onclick(); count(20);
  set('find','Extra');
  assert.equal(el('pageNumber').textContent,'第 1 / 2 页');
  assert.equal(options()[1],'Shared (@one) · 38 篇');
  set('category','随笔'); count(0);
  assert.equal(el('pager').hidden,true);
  el('clearFilters').onclick(); count(20);
  assert.equal(el('pageNumber').textContent,'第 1 / 3 页');
  console.log('Shelf: author counts, combined filters, pagination, boundaries and reader return OK');
});
