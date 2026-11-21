// Manual export of publicly displayed originals. No requests, browser storage or account actions.
((captureOnly=false)=>{
 const path=location.pathname.match(/^\/([\w]+)\/status\/(\d+)\/?$/);
 if(!['x.com','twitter.com'].includes(location.hostname)||!path||document.getElementById('sf-x-export'))return;
 const source=location.href.split(/[?#]/)[0],author=path[1],mainId=path[2],rows=new Map(),blocked=new Set();
 const nested='article,blockquote,div[role="link"],[data-testid="quoteTweet"],[data-testid="tweetText"],[data-testid="card.wrapper"]';
 const spam=/固炮|炮友|破处|约炮|找炮|寻男|点主页|免费破/;
 const ui=document.createElement('aside');ui.id='sf-x-export';ui.style.cssText='position:fixed;bottom:1rem;right:1rem;z-index:2147483647;background:white;color:#183e30;padding:.8rem;border:1px solid #567;max-width:26em;font:14px system-ui';
 const message=document.createElement('p');ui.append(message);function button(label,fn){const b=document.createElement('button');b.textContent=label;b.onclick=fn;ui.append(b)}
 function avatar(el,owns,profileUrl){
  const urls=new Set();
  for(const img of el.querySelectorAll('[data-testid="Tweet-User-Avatar"] img')){
   const link=img.closest('a[href]'), component=img.closest('[data-testid="Tweet-User-Avatar"]');
   if(!owns(img)||!link||!component.contains(link)||link.href.replace(/\/$/,'').toLowerCase()!==profileUrl.toLowerCase())continue;
   const url=img.currentSrc||img.src;
   if(/^https:\/\/pbs\.twimg\.com\/(?:profile_images|sticky\/default_profile_images)\/[^?#]+$/.test(url))urls.add(url);
  }
  return urls.size===1?[...urls][0]:undefined;
 }
 const media=i=>{const url=i.currentSrc||i.src;return /^https:\/\/pbs\.twimg\.com\/media\/[\w.-]+(?:\?[^#]*)?$/.test(url)?url:''};
 function longForm(view){
  const title=view.querySelector('[data-testid="twitter-article-title"]')?.innerText.trim();
  const rich=view.querySelector('[data-testid="twitterArticleRichTextView"]');
  if(!title||!rich?.innerText.trim())return;
  const foreign='blockquote,[data-testid="quoteTweet"],[data-testid="card.wrapper"],article';
  const owned=i=>!i.closest('blockquote,[data-testid="quoteTweet"],[data-testid="card.wrapper"]')&&i.closest('article')===view;
  const covers=[...new Set([...view.querySelectorAll('[data-testid="tweetPhoto"] img')].filter(i=>owned(i)&&!rich.contains(i)).map(media).filter(Boolean))];
  const clone=rich.cloneNode(true), images=[];
  const src=[...rich.querySelectorAll('img')];
  [...clone.querySelectorAll('img')].forEach((i,n)=>{
   const url=owned(src[n])&&media(src[n]);
   if(!url){i.remove();return}
   images.push({alt:i.alt,url});
   for(const attr of [...i.attributes])i.removeAttribute(attr.name);
   i.setAttribute('src',url);i.setAttribute('alt',src[n].alt);
  });
  for(const n of clone.querySelectorAll(foreign))n.remove();
  return {format:'article',title,articleHtml:clone.innerHTML,text:rich.innerText,images,...(covers.length===1?{coverUrl:covers[0]}:{})};
 }
 function quoteLink(el,owns,id){
  const urls=new Set();
  for(const card of el.querySelectorAll('[data-testid="quoteTweet"],div[role="link"]')){
   if(!owns(card))continue;
   const time=card.querySelector('time[datetime]'),a=time?.closest('a[href]');
   if(!a||!card.contains(a))continue;
   const url=new URL(a.href);
   const target=url.pathname.match(/^\/\w+\/status\/(\d+)$/);
   if(url.origin===location.origin&&target&&target[1]!==id)urls.add(url.origin+url.pathname);
  }
  return urls.size===1?[...urls][0]:undefined;
 }
 function collect(){if(location.href.split(/[?#]/)[0]!==source){message.textContent='页面已改变，请关闭后在目标原帖重新开始。';return false}
 let translated=0;const previous=rows.has(mainId);let afterMain=previous,ended=false;
 const region=document.querySelector('[data-testid="primaryColumn"]');if(!region)return false;
 for(const el of region.querySelectorAll('article[data-testid="tweet"],h2,[role="heading"]')){
  if(el.tagName!=='ARTICLE'){if(!el.closest('article')&&/^(discover more|more posts|you might like|发现更多|更多帖子|你可能喜欢)$/i.test(el.textContent.trim()))ended=true;continue}if(ended||el.closest('[data-testid="placementTracking"]')||el.querySelector('[data-testid="placementTracking"]'))continue;
  if(el.parentElement.closest(nested))continue;
  const owns=n=>n.parentElement.closest(nested)===el, find=s=>[...el.querySelectorAll(s)].find(owns);
  const time=find('time[datetime]'),link=time?.closest('a[href]');if(!link)continue;const l=new URL(link.href);const p=l.pathname.match(/^\/(\w+)\/status\/(\d+)$/);if(!p||l.hostname!==location.hostname)continue;const [,handle,id]=p;
  if(id===mainId){if(handle.toLowerCase()!==author.toLowerCase())continue;afterMain=true;}if(!afterMain||(!rows.has(mainId)&&id!==mainId))continue;
  if(/^(?:translated from .+|show original|显示原文|查看原文)$/im.test(el.innerText)){translated++;continue}
  const view=id===mainId&&el.querySelector('article[data-testid="twitterArticleReadView"]');
  const long=view&&view.parentElement.closest(nested)===el?longForm(view):undefined;
  if(view&&!long)continue;
  const body=find('[data-testid="tweetText"]'),text=body?.innerText||'';const name=find('[data-testid="User-Name"]')?.innerText.split('\n')[0]||handle;
  if(id!==mainId&&spam.test([name,handle,text].join(' '))){blocked.add(id);continue}
  const images=[...el.querySelectorAll('[data-testid="tweetPhoto"] img')].filter(owns).map(i=>({alt:i.alt,url:media(i)})).filter(i=>i.url);
  if(!long&&!text&&!images.length)continue;const links=[...(body?.querySelectorAll('a[href]')||[])].filter(a=>/^https?:/.test(a.href)).map(a=>({text:a.innerText,url:a.href}));
  const profileUrl=l.origin+'/'+handle, earlier=rows.get(id)?.author;
  const avatarUrl=avatar(el,owns,profileUrl)||(earlier?.profileUrl.toLowerCase()===profileUrl.toLowerCase()?earlier.avatarUrl:undefined);
  rows.set(id,{id,url:l.origin+l.pathname,author:{handle,name,profileUrl,avatarUrl},createdAt:time.dateTime,visibleTime:time.textContent,text,images,links,...long,quoteUrl:quoteLink(el,owns,id),translated:false,...(id===mainId?{}:{relation:handle.toLowerCase()===author.toLowerCase()?'author-visible-reply':'visible-reply'})});
 }message.textContent=`${rows.has(mainId)?'主帖已记录':'先滚到主帖并显示原文'} · 已选可见回复 ${Math.max(0,rows.size-1)} · 本屏译文跳过 ${translated} · 已过滤 ${blocked.size}。只在原帖回复区点「记录此屏」，勿收录推荐流。`;return rows.has(mainId)}
 function record(){return {...rows.get(mainId),source:'x',schemaVersion:3,capturedAt:new Date().toISOString(),capturedFromUrl:source,blockedCommentCount:blocked.size,comments:[...rows.values()].filter(r=>r.id!==mainId)}}
 if(captureOnly)return collect()?record():null;
 button('记录此屏',collect);button('保存 JSON',()=>{if(!collect())return;const href=URL.createObjectURL(new Blob([JSON.stringify(record())],{type:'application/json'})),a=document.createElement('a');a.href=href;a.download=`x-${mainId}.json`;a.click();setTimeout(()=>URL.revokeObjectURL(href),1000)});button('关闭',()=>ui.remove());document.body.append(ui);collect();
})();
