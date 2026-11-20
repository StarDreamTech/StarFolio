/* Optional browser bookmarklet. Operates ONLY on already displayed posts. No HTTP, credentials or hidden state. */
(() => {
  if (document.getElementById('sf-save-visible')) return;
  const source = location.origin + location.pathname.match(/^\/t\/(?:[^/]+\/)?\d+(?=\/|$)/)?.[0];
  if (!document.querySelector('.post-stream') || !source.includes('/t/')) return;
  const posts = new Map();
  const save = document.createElement('button');
  save.id = 'sf-save-visible'; save.type = 'button';
  Object.assign(save.style,{position:'fixed',zIndex:2147483647,bottom:'2rem',right:'1rem',padding:'0.8em',background:'#164f40',color:'white'});
  function collect() {
    for (const a of document.querySelectorAll('article[data-post-id][data-user-id]')) {
      const c = a.querySelector('.cooked'), user = a.querySelector('a[data-user-card]');
      const n = Number(a.id.replace(/^post_/, ''));
      if (!c || !user || !Number.isInteger(n) || n < 1) continue;
      const time = a.querySelector('.post-date [data-time]')?.dataset.time;
      const reply = a.querySelector('a.reply-to-tab, .reply-to-tab a')?.getAttribute('href')?.match(/\/t\/[^/]+\/\d+\/(\d+)\/?$/)?.[1];
      posts.set(n, {id:Number(a.dataset.postId),post_number:n,user_id:Number(a.dataset.userId),username:user.dataset.userCard,name:a.querySelector('.names .full-name')?.textContent.trim()||null,created_at:time?new Date(Number(time)).toISOString():'',cooked:c.innerHTML,reply_to_post_number:reply?Number(reply):null});
    }
    const label = `星藏 · 已读 ${posts.size} 楼 / 点击导出`;
    if(save.textContent!==label)save.textContent=label;
  }
  save.onclick = () => {
    collect(); if (!posts.has(1)) {save.textContent='请先回到主楼，再点导出';return;}
    const title = document.querySelector('#topic-title h1')?.textContent.trim() || document.title;
    const count = Number(document.querySelector('.timeline-replies')?.textContent.split('/').pop()?.trim());
    const data={format:'starfolio.discourse-dom.v1',url:source,title,expected:Math.max(count||0,posts.size),posts:[...posts.values()].sort((a,b)=>a.post_number-b.post_number)};
    const a=document.createElement('a'), u=URL.createObjectURL(new Blob([JSON.stringify(data)],{type:'application/json'})); a.href=u; a.download='starfolio-private.json'; a.click();setTimeout(()=>URL.revokeObjectURL(u),1000);observer.disconnect();save.remove();
  };
  document.body.append(save);collect();const observer=new MutationObserver(collect);observer.observe(document.querySelector('.post-stream'),{childList:true,subtree:true});
})();
