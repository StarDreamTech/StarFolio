/* Optional browser bookmarklet. Operates ONLY on already displayed posts. No HTTP, credentials or hidden state. */
(() => {
  if (document.getElementById('sf-save-visible')) return;
  const source = location.origin + location.pathname.match(/^\/t\/(?:[^/]+\/)?\d+(?=\/|$)/)?.[0];
  const stream = document.querySelector('.post-stream');
  if (!stream || !source.includes('/t/')) return;
  const nested = 'article,.cooked,.embedded-posts,.quote,blockquote';
  const posts = new Map();
  const save = document.createElement('button');
  save.id = 'sf-save-visible'; save.type = 'button';
  Object.assign(save.style,{position:'fixed',zIndex:2147483647,bottom:'2rem',right:'1rem',padding:'0.8em',background:'#164f40',color:'white'});
  function collect() {
    for (const a of stream.querySelectorAll('article[data-post-id][data-user-id]')) {
      if (a.parentElement.closest(nested)) continue;
      const owns = el => el.parentElement.closest(nested) === a;
      const find = selector => [...a.querySelectorAll(selector)].find(owns);
      const c = find('.cooked'), user = find('.topic-meta-data .names .first a[data-user-card], .topic-meta-data .names .username a[data-user-card]');
      const n = Number(a.id.replace(/^post_/, ''));
      const username = user?.dataset.userCard;
      if (!c || !username || !Number.isInteger(n) || n < 1) continue;
      const id = Number(a.dataset.postId), user_id = Number(a.dataset.userId), earlier = posts.get(n);
      const avatars = new Set();
      for (const img of a.querySelectorAll('.topic-avatar a[data-user-card] img.avatar')) {
        if (!owns(img) || img.closest('a[data-user-card]').dataset.userCard.toLowerCase() !== username.toLowerCase()) continue;
        const url = img.currentSrc || img.src;
        if (/^https:\/\/[^/?#@]+\/[^?#]*$/.test(url)) avatars.add(url);
      }
      const avatar_template = avatars.size === 1 ? [...avatars][0] :
        earlier?.id === id && earlier.user_id === user_id && earlier.username.toLowerCase() === username.toLowerCase() ? earlier.avatar_template : undefined;
      const time = find('.post-date [data-time]')?.dataset.time;
      const reply = find('a.reply-to-tab, .reply-to-tab a')?.getAttribute('href')?.match(/\/t\/[^/]+\/\d+\/(\d+)\/?$/)?.[1];
      posts.set(n, {id,post_number:n,user_id,username,avatar_template,name:find('.names .full-name')?.textContent.trim()||null,created_at:time?new Date(Number(time)).toISOString():'',cooked:c.innerHTML,reply_to_post_number:reply?Number(reply):null});
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
  document.body.append(save);collect();const observer=new MutationObserver(collect);observer.observe(stream,{childList:true,subtree:true});
})();
