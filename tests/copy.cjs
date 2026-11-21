// Exercise the app's actual button handler; native rich-text import is tested separately.
const assert=require('node:assert/strict'), vm=require('node:vm'), fs=require('node:fs');
const nodes=new Map();
function el(id){if(!nodes.has(id))nodes.set(id,{value:'',children:[],options:[],append(){},add(){},replaceChildren(){}});return nodes.get(id)}
let calls=[],fail=false;
async function invoke(cmd,args){if(cmd==='catalog')return ['/example',[]];if(cmd==='read')return {html:'',css:'',site_css:''};if(cmd==='copy_article'){calls.push(args);if(fail)throw Error('测试失败');return [2,1]}}
const ctx={document:{getElementById:el}, Option:class{},window:{__TAURI__:{core:{invoke}}},setTimeout:()=>0,clearTimeout(){}};
vm.runInNewContext(fs.readFileSync('ui/app.js','utf8'),ctx);
(async()=>{await el('copyArticle').onclick();assert.equal(calls.length,0);
  await vm.runInNewContext("read('sample')",ctx);await el('copyArticle').onclick();assert.equal(calls[0].id,'sample');assert.equal(calls[0].replies,false);assert.match(el('status').textContent,/2 张图片.*1 处缺图/);
  el('copyReplies').checked=true;await el('copyArticle').onclick();assert.equal(calls[1].replies,true);
  fail=true;await el('copyArticle').onclick();assert.match(el('status').textContent,/复制失败/);assert.equal(el('copyArticle').disabled,false);assert.equal(el('back').disabled,false);
  console.log('Copy button: current archive, main vs replies, missing pictures, failure and recovery OK');
})().catch(e=>{console.error(e);process.exitCode=1});
