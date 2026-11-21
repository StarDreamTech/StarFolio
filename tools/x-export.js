// Manual export of publicly displayed originals. No requests, browser storage or account actions.
(()=>{
 const path=location.pathname.match(/^\/([\w]+)\/status\/(\d+)\/?$/);
 if(!['x.com','twitter.com'].includes(location.hostname)||!path||document.getElementById('sf-x-export'))return;
 const source=location.href.split(/[?#]/)[0],author=path[1],mainId=path[2],rows=new Map(),blocked=new Set();
 const spam=/固炮|炮友|破处|约炮|找炮|寻男|点主页|免费破/;
 const ui=document.createElement('aside');ui.id='sf-x-export';ui.style.cssText='position:fixed;bottom:1rem;right:1rem;z-index:2147483647;background:white;color:#183e30;padding:.8rem;border:1px solid #567;max-width:26em;font:14px system-ui';
 const message=document.createElement('p');ui.append(message);function button(label,fn){const b=document.createElement('button');b.textContent=label;b.onclick=fn;ui.append(b)}
 function collect(){if(location.href.split(/[?#]/)[0]!==source){message.textContent='页面已改变，请关闭后在目标原帖重新开始。';return false}
 let translated=0;const previous=rows.has(mainId);let afterMain=previous,ended=false;
 const region=document.querySelector('[data-testid="primaryColumn"]');if(!region)return false;
 for(const el of region.querySelectorAll('article[data-testid="tweet"],h2,[role="heading"]')){
  if(el.tagName!=='ARTICLE'){if(/^(discover more|more posts|you might like|发现更多|更多帖子|你可能喜欢)$/i.test(el.textContent.trim()))ended=true;continue}if(ended||el.closest('[data-testid="placementTracking"]')||el.querySelector('[data-testid="placementTracking"]'))continue;
  const time=el.querySelector('time[datetime]'),link=time?.closest('a[href]');if(!link)continue;const l=new URL(link.href);const p=l.pathname.match(/^\/(\w+)\/status\/(\d+)$/);if(!p||l.hostname!==location.hostname)continue;const [,handle,id]=p;
  if(id===mainId)afterMain=true;if(!afterMain||(!rows.has(mainId)&&id!==mainId))continue;
  if(/^(?:translated from .+|show original|显示原文|查看原文)$/im.test(el.innerText)){translated++;continue}
  const body=el.querySelector('[data-testid="tweetText"]'),text=body?.innerText||'';const name=el.querySelector('[data-testid="User-Name"]')?.innerText.split('\n')[0]||handle;
  if(id!==mainId&&spam.test([name,handle,text].join(' '))){blocked.add(id);continue}
  const images=[...el.querySelectorAll('[data-testid="tweetPhoto"] img')].map(i=>({alt:i.alt,url:i.currentSrc||i.src})).filter(i=>/^https:\/\/pbs\.twimg\.com\/media\//.test(i.url));
  if(!text&&!images.length)continue;const links=[...(body?.querySelectorAll('a[href]')||[])].filter(a=>/^https?:/.test(a.href)).map(a=>({text:a.innerText,url:a.href}));
  rows.set(id,{id,url:l.origin+l.pathname,author:{handle,name,profileUrl:l.origin+'/'+handle},createdAt:time.dateTime,visibleTime:time.textContent,text,images,links,translated:false,...(id===mainId?{}:{relation:handle.toLowerCase()===author.toLowerCase()?'author-visible-reply':'visible-reply'})});
 }message.textContent=`${rows.has(mainId)?'主帖已记录':'先滚到主帖并显示原文'} · 已选可见回复 ${Math.max(0,rows.size-1)} · 本屏译文跳过 ${translated} · 已过滤 ${blocked.size}。只在原帖回复区点「记录此屏」，勿收录推荐流。`;return rows.has(mainId)}
 button('记录此屏',collect);button('保存 JSON',()=>{if(!collect())return;const doc={...rows.get(mainId),source:'x',schemaVersion:3,capturedAt:new Date().toISOString(),capturedFromUrl:source,blockedCommentCount:blocked.size,comments:[...rows.values()].filter(r=>r.id!==mainId)};const href=URL.createObjectURL(new Blob([JSON.stringify(doc)],{type:'application/json'})),a=document.createElement('a');a.href=href;a.download=`x-${mainId}.json`;a.click();setTimeout(()=>URL.revokeObjectURL(href),1000)});button('关闭',()=>ui.remove());document.body.append(ui);collect();
})();
