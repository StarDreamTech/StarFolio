// Test the same DOM-wired controls loaded by Tauri, without a network or model.
const assert=require('node:assert/strict'),vm=require('node:vm'),fs=require('node:fs');
class Element{value='';children=[];get childElementCount(){return this.children.length}get options(){return this.children}replaceChildren(...v){this.children=v;this.value=''}append(...v){this.children.push(...v)}add(v){this.append(v)}}
const els=new Map(),el=id=>{if(!els.has(id))els.set(id,new Element());return els.get(id)};
const rows=[
  ['X','x.com','社交帖子',['科学'],'x.com:account:one','Shared (@one)'],
  ['Paper','else.example','随笔',['科学','艺术'],'else.example:byline:Shared','Shared'],
  ['Older','x.com','随笔',[],'x.com:account:one','Shared (@one)'],
  ['Second','x.com','社交帖子',['艺术'],'x.com:account:two','Shared (@two)'],
  ['No byline','x.com','随笔',[],null],
].map(([title,site,category,tags,key,label],id)=>({title,site,category,tags,author:key?{key,label}:null,id,saved:'2025-01-01'}));
const markup=fs.readFileSync('ui/index.html','utf8');
for(const id of ['author','site','category','tag','find','clearFilters'])assert.ok(markup.includes(`id="${id}"`));
vm.runInNewContext(fs.readFileSync('ui/app.js','utf8'),{document:{getElementById:el,createElement:()=>new Element()},Option:class{constructor(text,value){this.text=text;this.value=value}},window:{__TAURI__:{core:{invoke:async()=>['library',rows]}}},clearTimeout,setTimeout});
setImmediate(()=>{
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
  console.log('Shelf: author ranking, counts, distinct accounts, combined filters, unknown, search and reset OK');
});
