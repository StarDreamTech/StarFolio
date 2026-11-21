const assert=require('node:assert/strict'), vm=require('node:vm'), fs=require('node:fs');
const script=fs.readFileSync('ui/updates.js','utf8'), markup=fs.readFileSync('ui/index.html','utf8');
const key='starfolio.autoUpdate';
const tick=()=>new Promise(setImmediate);
function boot({supported=true, saved, update='0.2.3', checkError}={}){
  const elements=new Map(), calls=[], storage=new Map(), toasts=[];
  if(saved)storage.set(key,saved);
  const $=id=>{
    assert.ok(markup.includes(`id="${id}"`),id);
    if(!elements.has(id))elements.set(id,{open:false,checked:false,listeners:{},addEventListener(e,fn){this.listeners[e]=fn},showModal(){this.open=true},close(){this.open=false}});
    return elements.get(id);
  };
  let releaseInstall;
  const ctx={$,status:m=>toasts.push(m),window:{localStorage:{getItem:k=>storage.get(k),setItem:(k,v)=>storage.set(k,v)}},async invoke(cmd,args){
    calls.push([cmd,args]);
    if(cmd==='app_info')return {version:'0.2.2',update_supported:supported};
    if(cmd==='check_update'){if(checkError)throw Error('模拟断网');return update}
    if(cmd==='install_update')return new Promise((_,no)=>{releaseInstall=no});
  }};
  vm.runInNewContext(script,ctx);
  return {$,calls,storage,failInstall:()=>releaseInstall(Error('模拟签名不匹配'))};
}
(async()=>{
  const a=boot(); await tick();
  assert.deepEqual(a.calls.map(x=>x[0]),['app_info']); // Preview and off-by-default do not visit GitHub.
  a.$('aboutBtn').onclick(); assert.equal(a.$('aboutPanel').open,true);
  assert.equal(a.$('appVersion').textContent,'版本 0.2.2');
  await a.$('githubBtn').onclick(); assert.equal(a.calls.at(-1)[1].url,'https://github.com/StarDreamTech/StarFolio');
  await a.$('checkUpdate').onclick();
  assert.match(a.$('updateState').textContent,/0\.2\.3/);
  assert.equal(a.$('installUpdate').hidden,false);
  assert.equal(a.calls.some(x=>x[0]==='install_update'),false);
  const installing=a.$('installUpdate').onclick();
  assert.equal(a.$('checkUpdate').disabled,true);
  a.failInstall(); await installing;
  assert.equal(a.calls.at(-1)[1].version,'0.2.3');
  assert.equal(a.$('installUpdate').hidden,true);
  assert.match(a.$('updateState').textContent,/签名不匹配/);
  a.$('autoUpdate').checked=true; a.$('autoUpdate').onchange();
  const b=boot({saved:a.storage.get(key)}); await tick();
  assert.deepEqual(b.calls.map(x=>x[0]),['app_info','check_update']);
  assert.equal(b.$('aboutPanel').open,true);
  assert.equal(b.$('installUpdate').hidden,false);
  b.$('dismissAbout').onclick(); assert.equal(b.$('aboutPanel').open,false);
  b.$('autoUpdate').checked=false; b.$('autoUpdate').onchange();
  assert.equal(b.storage.get(key),'off');
  const c=boot({saved:'on',update:null}); await tick();
  assert.equal(c.$('aboutPanel').open,false); assert.match(c.$('updateState').textContent,/已是最新/);
  const d=boot({saved:'on',checkError:true}); await tick();
  assert.equal(d.$('aboutPanel').open,false); assert.match(d.$('updateState').textContent,/模拟断网/);
  const e=boot({saved:'on',supported:false}); await tick();
  assert.deepEqual(e.calls.map(x=>x[0]),['app_info']); assert.equal(e.$('checkUpdate').hidden,true);
  console.log('About: GitHub URL, version, opt-in startup check, consent, offline and signature failure UI OK');
})().catch(e=>{console.error(e);process.exitCode=1});
