// Exercise the real reader handler without opening a browser or reading private archives.
const assert=require('node:assert/strict'), vm=require('node:vm'), fs=require('node:fs');
const nodes=new Map();
function el(id){if(!nodes.has(id))nodes.set(id,{options:[],add(){},replaceChildren(){},hidden:true});return nodes.get(id)}
const sources={first:'https://example.org/post',second:'https://example.net/thread',image:''};
const opened=[];
let fail=false;
async function invoke(cmd,args){
  if(cmd==='catalog')return ['/example',[]];
  if(cmd==='read')return {html:'',css:'',site_css:'',source_url:sources[args.id]};
  if(cmd==='open_link'){opened.push(args.url);if(fail)throw Error('测试失败')}
}
const ctx={document:{getElementById:el},Option:class{},window:{__TAURI__:{core:{invoke}}},setTimeout:()=>0,clearTimeout(){}};
vm.runInNewContext(fs.readFileSync('ui/app.js','utf8'),ctx);
const read=id=>vm.runInNewContext(`read(${JSON.stringify(id)})`,ctx);
(async()=>{
  const markup=fs.readFileSync('ui/index.html','utf8');
  assert.match(markup,/<div class="tools">[^]*?<button[^>]*id="openOriginal"[^>]*hidden>打开原文[^<]*<\/button><\/div><iframe/);
  const button=el('openOriginal');
  await read('first');
  assert.equal(button.hidden,false);
  assert.deepEqual(opened,[]);
  await button.onclick();
  assert.deepEqual(opened,[sources.first]);
  await read('image');
  assert.equal(button.hidden,true);
  assert.equal(button.onclick,null);
  await read('second');
  assert.equal(button.hidden,false);
  assert.deepEqual(opened,[sources.first]);
  fail=true;
  await button.onclick();
  assert.deepEqual(opened,[sources.first,sources.second]);
  assert.match(el('status').textContent,/无法打开原文/);
  console.log('Original link: toolbar, explicit click, correct source, no-link fallback and failure feedback OK');
})().catch(e=>{console.error(e);process.exitCode=1});
