'use strict';
const $=s=>document.getElementById(s), invoke=(cmd,args)=>window.__TAURI__.core.invoke(cmd,args);
const aiLabel=s=>({reviewed:"AI 已复核",needs_review:"AI 建议人工确认",pending:"AI 待复核"})[s]||"AI 未复核";
const pageSize=20;
let catalog=[], active=null, siteCss='', generation=0, observer, timer, page=1, shelfScroll=0;
function status(x,persist=false){clearTimeout(timer);$('status').textContent=x;if(!persist) timer=setTimeout(()=>$('status').textContent='',5500)}
const zoneKey='starfolio.displayTimeZone';
function makeTimeFormatter(zone){
  const options={timeZone:zone||undefined,year:'numeric',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',second:'2-digit',hourCycle:'h23'};
  try{return new Intl.DateTimeFormat('en-GB',{...options,timeZoneName:'longOffset'})}
  catch{return new Intl.DateTimeFormat('en-GB',{...options,timeZoneName:'short'})}
}
let displayZone='', timeFormatter=makeTimeFormatter('');
try{
  const zone=window.localStorage.getItem(zoneKey)||'';
  timeFormatter=makeTimeFormatter(zone);displayZone=zone;
}catch{/* The reader can still follow the system timezone without a saved preference. */}
function displayTime(raw){
  const stamp=raw?.match(/^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(?:\.\d+)?(Z|[+-]\d{2}:\d{2})$/);
  if(!stamp)return raw;
  const wall=new Date(stamp[1]+'Z'),date=new Date(stamp[1]+stamp[2]);
  if(!Number.isFinite(+date)||!Number.isFinite(+wall)||wall.toISOString().slice(0,19)!==stamp[1])return raw;
  const p=Object.fromEntries(timeFormatter.formatToParts(date).map(({type,value})=>[type,value]));
  const zone=/^(GMT|UTC)$/.test(p.timeZoneName)?'UTC+00:00':p.timeZoneName.replace(/^GMT/,'UTC');
  return `${p.year}-${p.month}-${p.day} ${p.hour}:${p.minute}:${p.second} ${zone}`;
}
function showReaderTimes(doc){
  for(const el of doc?.querySelectorAll('time[data-archive-time]')||[]){
    const original=el.dateTime;
    el.textContent=displayTime(original);
    el.title=`原始时间：${original}`;
  }
}
function openZone(){
  const select=$('timeZone');
  const zones=Intl.supportedValuesOf?.('timeZone')||['Asia/Tokyo','America/New_York','America/Los_Angeles','Europe/London','Australia/Sydney'];
  const added=new Set([...select.options].map(o=>o.value));
  for(const zone of new Set([displayZone,...zones])){
    if(zone&&!added.has(zone))select.add(new Option(zone,zone));
  }
  select.value=displayZone;
  $('zonePanel').showModal();
}
$('zoneBtn').onclick=openZone;
$('saveZone').onclick=()=>{
  const zone=$('timeZone').value;
  timeFormatter=makeTimeFormatter(zone);displayZone=zone;
  $('zonePanel').close();cards();
  if(active)showReaderTimes($('frame').contentDocument);
  try{window.localStorage.setItem(zoneKey,zone)}catch{status('已应用时区，但本机无法保存设置。')}
};
const languageKey='starfolio.readingLanguage';
let readingLanguage='system', readerTranslated=false;
try{
  const saved=window.localStorage.getItem(languageKey);
  if(saved==='zh-Hans'||saved==='original')readingLanguage=saved;
}catch{/* Still usable without saved preferences. */}
const prefersChinese=()=>readingLanguage==='zh-Hans'||(readingLanguage==='system'&&/^zh(?:-|$)/i.test(window.navigator?.language||''));
$('languageBtn').onclick=()=>{$('readingLanguage').value=readingLanguage;$('languagePanel').showModal()};
$('saveLanguage').onclick=()=>{
  readingLanguage=$('readingLanguage').value;
  $('languagePanel').close();cards();
  if(active)read(active,true).catch(e=>status(String(e),true));
  try{window.localStorage.setItem(languageKey,readingLanguage)}catch{status('已切换阅读语言，但本机无法保存设置。')}
};
const facets=['category','site','tag'];
function facetOptions(){for(const [i,k] of facets.entries()){const el=$(k),old=el.value;const values=[...new Set(catalog.flatMap(x=>k==='tag'?(x.tags||[]):[x[k]]).filter(Boolean))].sort((a,b)=>a.localeCompare(b));el.replaceChildren(new Option(['全部分类','全部域名','全部标签'][i],''));if(k==='tag')el.add(new Option('暂无 AI 标签','__untagged'));for(const v of values)el.add(new Option(v,v));if([...el.options].some(o=>o.value===old))el.value=old}}
function matches(s,q,c,d,t){const tags=s.tags||[];return (!c||c===s.category)&&(!d||d===s.site)&&(!t||(t==='__untagged'?!tags.length:tags.includes(t)))&&[s.title,s.chinese_title,s.author?.label,s.site,s.category,s.image_text,...tags].join(' ').toLocaleLowerCase().includes(q.trim().toLocaleLowerCase())}
const authorKey=s=>s.author?.key||'__unknown_author';
function authorOptions(rows){
  const el=$('author'),old=el.value,counts=new Map();
  for(const s of rows){
    const key=authorKey(s);
    if(!counts.has(key))counts.set(key,{key,label:s.author?.label||'未记录作者',count:0});
    counts.get(key).count++;
  }
  if(old&&!counts.has(old)){
    const s=catalog.find(s=>authorKey(s)===old);
    if(s)counts.set(old,{key:old,label:s.author?.label||'未记录作者',count:0});
  }
  const options=[...counts.values()].sort((a,b)=>Number(a.key==='__unknown_author')-Number(b.key==='__unknown_author')||b.count-a.count||a.label.localeCompare(b.label,'zh-CN'));
  el.replaceChildren(new Option('全部作者',''),...options.map(a=>new Option(`${a.label} · ${a.count} 篇`,a.key)));
  if(counts.has(old))el.value=old;
}
function cards(){
  const items=$('items');items.replaceChildren();
  const candidates=catalog.filter(s=>matches(s,$('find').value,...facets.map(k=>$(k).value)));
  authorOptions(candidates);
  const author=$('author').value,rows=author?candidates.filter(s=>authorKey(s)===author):candidates;
  const pages=Math.max(1,Math.ceil(rows.length/pageSize));
  page=Math.min(page,pages);
  const start=(page-1)*pageSize,end=Math.min(start+pageSize,rows.length);
  $('count').textContent=rows.length?`显示第 ${start+1}–${end} 篇 · 匹配 ${rows.length} 篇 / 全部 ${catalog.length} 篇`:`匹配 0 篇 / 全部 ${catalog.length} 篇`;
  $('pager').hidden=pages===1;
  $('pageNumber').textContent=`第 ${page} / ${pages} 页`;
  $('prevPage').disabled=page===1;
  $('nextPage').disabled=page===pages;
  for(const s of rows.slice(start,end)){
    const b=document.createElement('button'),t=document.createElement('strong'),m=document.createElement('span'),l=document.createElement('span');
    b.className='card';t.textContent=(prefersChinese()&&s.chinese_title)||s.title;m.className='by';
    m.textContent=`${s.site}${s.author?' · '+s.author.label:''} · 归档于 ${displayTime(s.saved)} · ${s.posts} 条 · ${s.images} 张图片 · ${aiLabel(s.ai_state)}${s.missing_images+s.missing_posts?' · 部分未保存':''}`;
    l.className='labels';l.textContent=[s.category,...(s.tags||[]).map(x=>'#'+x)].filter(Boolean).join(' · ');
    b.append(t,m,l);b.onclick=()=>read(s.id).catch(e=>status(String(e)));items.append(b);
  }
  if(!rows.length)items.textContent=catalog.length?'没有符合这些条件的归档。':'书架还空着。粘贴第一篇文章的链接开始。';
}
function searchShelf(){page=1;cards()}
function turnPage(id,delta){
  if($(id).disabled)return;
  page+=delta;cards();$('shelfTitle').focus();$('shelfTitle').scrollIntoView();
}
$('prevPage').onclick=()=>turnPage('prevPage',-1);
$('nextPage').onclick=()=>turnPage('nextPage',1);
for(const k of [...facets,'author'])$(k).onchange=()=>{if(k==='site')$('author').value='';searchShelf()};
$('clearFilters').onclick=()=>{for(const k of [...facets,'author','find'])$(k).value='';searchShelf()};
async function refresh(){const [path,entries]=await invoke('catalog');catalog=entries;$('location').textContent=path;facetOptions();cards()}
$('capture').onsubmit=async e=>{e.preventDefault();$('save').disabled=true;status('正在保存正文、评论和图片…',true);try{const s=await invoke('save',{url:$('url').value});await refresh();$('url').value='';await read(s.id);status(s.missing_posts+s.missing_images?`已保存；缺 ${s.missing_posts} 楼、${s.missing_images} 个图片来源。`:'归档完成，可以离线阅读。')}catch(err){status(String(err),true)}finally{$('save').disabled=false}};
async function read(id, preserveReplies=false){const gen=++generation;const v=await invoke('read',{id,preferChinese:prefersChinese()});if(gen!==generation)return;active=id;readerTranslated=!!v.translated;
  const original=$('openOriginal');
  original.hidden=!v.source_url;
  original.onclick=v.source_url ? ()=>invoke('open_link',{url:v.source_url}).catch(e=>status('无法打开原文：'+e,true)) : null;
  if(!preserveReplies)$('copyReplies').checked=false;$('copyReplies').disabled=!(catalog.find(s=>s.id===id)?.posts>1);siteCss=v.site_css;setAudit(v.audit);if(!$('library').hidden)shelfScroll=window.scrollY;$('library').hidden=true;$('reader').hidden=false;observer?.disconnect();const f=$('frame');f.onload=()=>{if(gen!==generation)return;const d=f.contentDocument;if(!d)return;const s=d.createElement('style');s.textContent=v.css+'\n'+siteCss;d.head.append(s);const box=d.querySelector('main');box.innerHTML=v.html;showReaderTimes(d);d.addEventListener('click',ev=>{const a=ev.target.closest?.('a[href]');if(!a)return;ev.preventDefault();const href=a.getAttribute('href');if(/^#p\d+$/.test(href))d.getElementById(href.slice(1))?.scrollIntoView();else if(/^https?:\/\//i.test(href))invoke('open_link',{url:href}).catch(e=>status(String(e)))});const jobs=new Map();observer=new IntersectionObserver(changes=>{for(const c of changes){if(!c.isIntersecting)continue;observer.unobserve(c.target);const el=c.target,name=el.dataset.avatar||el.dataset.local,key=(el.dataset.avatar?'avatar:':'image:')+name;if(!jobs.has(key))jobs.set(key,el.dataset.avatar?invoke('avatar_image',{name}):invoke('image',{id,name}));jobs.get(key).then(data=>{if(gen===generation)el.src=data}).catch(()=>{el.alt='本地图片无法读取'})}},{root:d,rootMargin:'280px'});box.querySelectorAll('img[data-local],img[data-avatar]').forEach(el=>observer.observe(el))};f.srcdoc='<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src \'none\'; img-src data:; style-src \'unsafe-inline\'; form-action \'none\'; base-uri \'none\'"></head><body><main></main></body></html>'}
$('back').onclick=()=>{++generation;active=null;observer?.disconnect();const f=$('frame');f.onload=null;f.srcdoc='';$('reader').hidden=true;$('library').hidden=false;window.scrollTo(0,shelfScroll)};$('find').oninput=searchShelf;$('edit').onclick=()=>{$('css').value=siteCss;$('styler').showModal()};$('apply').onclick=async()=>{if(!active)return;try{await invoke('style',{id:active,css:$('css').value});$('styler').close();await read(active);status('已对该站点的全部归档应用样式。')}catch(e){status(String(e))}};refresh().catch(e=>status('无法打开书架：'+e,true));
$('file').onchange=async()=>{const file=$('file').files[0];if(!file)return;try{if(file.size>16*1024*1024)throw Error('文件超过 16 MiB');const data=await file.text();let url=$('url').value.trim();if(!url&&data.trimStart().startsWith('{'))url=JSON.parse(data).url;if(!url)throw Error('请先填写原文链接，然后重新选择文件。');status('正在导入正文和图片…',true);$('save').disabled=true;const s=await invoke('import_file',{url,content:data});await refresh();await read(s.id);status('导入完成。')}catch(e){status(String(e),true)}finally{$('file').value='';$('save').disabled=false}};

function setAudit(a){$('auditState').textContent=`${aiLabel(a?.state)}。已复核 ${a?.reviewed_entries||0} 条摘录。${a?.model?'模型：'+a.model+'。':''}${a?.flagged?.length?'建议查看楼层：'+a.flagged.join('、')+'。':''}${(a?.notes||[]).join('；')}`}
$('auditBtn').onclick=()=>$('auditPanel').showModal();$('dismissAudit').onclick=()=>$('auditPanel').close();$('again').onclick=async()=>{if(!active)return;$('again').disabled=true;try{setAudit(await invoke('review_again',{id:active}));await refresh()}catch(e){status(String(e))}finally{$('again').disabled=false}};

$('categoryBtn').onclick=()=>{if(!active)return;$('categoryName').value=catalog.find(s=>s.id===active)?.category||'';$('organize').showModal()};
$('saveCategory').onclick=async()=>{if(!active)return;try{await invoke('categorize',{id:active,name:$('categoryName').value});await refresh();$('organize').close();status('已保存分类。')}catch(e){status(String(e))}};

$('copyArticle').onclick=async()=>{
  const id=active, button=$('copyArticle'); if(!id||button.disabled)return;
  const replies=$('copyReplies').checked; button.disabled=true; $('back').disabled=true;
  status('正在从本地归档准备图文…',true);
  try {
    const [images,missing]=await invoke('copy_article',{id,replies,preferChinese:readerTranslated});
    status(`已复制${replies?'正文和评论':'正文'}${images?`及 ${images} 张图片`:''}。${missing?`另有 ${missing} 处缺图，已保留文字提示。`:''}`);
  } catch(e){status('复制失败：'+e,true)}
  finally{button.disabled=false;$('back').disabled=false}
};
