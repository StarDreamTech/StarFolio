const assert=require('node:assert/strict'),fs=require('node:fs'),vm=require('node:vm');
const nodes=new Map(),calls=[];
const node=()=>({options:[],add(){},hidden:false,children:[],append(...items){this.children.push(...items)},replaceChildren(...items){this.children=items}});
function el(id){if(!nodes.has(id))nodes.set(id,node());return nodes.get(id)}
let jobs=[],fail=false;
const url='https://x.com/example/status/123';
const pending={id:'job',url,state:'pending'};
const window={__TAURI__:{core:{invoke:async(cmd,args)=>{
  calls.push({cmd,args});
  if(cmd==='catalog')return ['/example',[]];
  if(cmd==='archive_queue')return jobs;
  if(cmd==='enqueue_url'){if(fail)throw Error('无效');assert.equal(args.url,url);jobs=[pending];return pending}
  if(cmd==='retry_archive'){assert.equal(args.id,'job');jobs=[pending];return}
  throw Error('unexpected '+cmd);
}}}};
vm.runInNewContext(fs.readFileSync('ui/app.js','utf8'),{document:{getElementById:el,createElement:node},Option:class{},window,setTimeout:()=>0,clearTimeout(){}});
(async()=>{
  el('url').value=url;
  await el('capture').onsubmit({preventDefault(){}});
  assert.equal(el('url').value,'');
  assert.equal(el('queuePanel').open,true);
  assert.match(el('status').textContent,/等待 Agent/);
  assert.match(el('queueItems').children[0].children[0].textContent,/待处理/);
  jobs=[{...pending,state:'blocked',reason:'需要正常登录'}];
  await el('queuePanel').ontoggle(); // handler starts a local listing
  assert.match(el('queueItems').children[0].children[0].textContent,/需要正常登录/);
  await el('queueItems').children[0].children[1].onclick();
  assert.match(el('queueItems').children[0].children[0].textContent,/待处理/);
  fail=true;el('url').value=url;
  await el('capture').onsubmit({preventDefault(){}});
  assert.equal(el('url').value,url);
  assert.match(el('status').textContent,/无效/);
  assert(!calls.some(x=>['save','import_file','read','open_link'].includes(x.cmd)));
  console.log('Local queue form, visible statuses, retry and error handling OK');
})().catch(e=>{console.error(e);process.exitCode=1});
