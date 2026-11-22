// The real frontend, driven with a two-entry fixture and no network.
const assert=require('node:assert/strict'), vm=require('node:vm'), fs=require('node:fs');
const script=fs.readFileSync('ui/app.js','utf8'), storage=new Map();
const rows=[{id:'one',title:'Original English',chinese_title:'中文标题'},{id:'two',title:'Untranslated'}];
class Element {
  value=''; children=[]; hidden=true;
  get options(){return this.children}
  add(value){this.children.push(value)}
  append(...children){this.children.push(...children)}
  replaceChildren(...children){this.children=children;this.value=''}
  showModal(){this.open=true}
  close(){this.open=false}
}
function app(language){
  const els=new Map(),el=id=>{if(!els.has(id))els.set(id,new Element());return els.get(id)};
  const calls=[];
  const invoke=async(cmd,args)=>{
    if(cmd==='catalog')return ['example',rows];
    calls.push([cmd,args]);
    if(cmd==='read')return {html:'',css:'',translated:args.id==='one'&&args.preferChinese};
    if(cmd==='copy_article')return [0,0];
  };
  const sandbox={document:{getElementById:el,createElement:()=>new Element()},Option:class{},window:{navigator:{language},localStorage:{getItem:k=>storage.get(k),setItem:(k,v)=>storage.set(k,v)},__TAURI__:{core:{invoke}}},clearTimeout(){},setTimeout(){return 0}};
  vm.runInNewContext(script,sandbox);
  return {el,calls,titles:()=>el('items').children.map(b=>b.children[0].textContent)};
}
(async()=>{
  const wait=()=>new Promise(setImmediate);
  const a=app('en-US'); await wait();
  assert.deepEqual(a.titles(),['Original English','Untranslated']);
  await a.el('items').children[0].onclick();
  assert.equal(a.calls.at(-1)[1].preferChinese,false);
  a.el('languageBtn').onclick(); assert.equal(a.el('readingLanguage').value,'system');
  a.el('readingLanguage').value='zh-Hans'; a.el('saveLanguage').onclick(); await wait();
  assert.deepEqual(a.titles(),['中文标题','Untranslated']);
  assert.equal(a.calls.at(-1)[1].preferChinese,true);
  await a.el('copyArticle').onclick(); assert.equal(a.calls.at(-1)[1].preferChinese,true);
  a.el('find').value='Original English'; a.el('find').oninput();
  assert.deepEqual(a.titles(),['中文标题']);
  const b=app('en-US'); await wait(); assert.deepEqual(b.titles(),['中文标题','Untranslated']);
  await b.el('items').children[1].onclick(); await b.el('copyArticle').onclick();
  assert.equal(b.calls.at(-1)[1].preferChinese,false);
  b.el('readingLanguage').value='original'; b.el('saveLanguage').onclick();
  assert.deepEqual(b.titles(),['Original English','Untranslated']);
  b.el('find').value='中文标题'; b.el('find').oninput();
  assert.deepEqual(b.titles(),['Original English']);
  storage.clear(); const c=app('zh-CN'); await wait(); assert.deepEqual(c.titles(),['中文标题','Untranslated']);
  console.log('Reading language: system default, persistence, both title searches, original fallback and matching copy OK');
})().catch(e=>{console.error(e);process.exitCode=1});
