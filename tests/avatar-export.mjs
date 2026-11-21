import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';

// Small DOM fixture: no browser, credentials, network, or extra dependencies.
function fixture(page) {
  const location = new URL(page);
  const part = (node, selector) => {
    const plain = selector.replace(/\[([\w-]+)(?:="([^"]*)")?\]/g, (_, k, value) => {
      if (!(k in node.attrs) || (value !== undefined && node.attrs[k] !== value)) selector = null;
      return '';
    });
    if (selector === null) return false;
    const [tag, ...classes] = plain.split('.');
    return (!tag || node.tagName === tag.toUpperCase()) && classes.every(c => node.attrs.class?.split(' ').includes(c));
  };
  const matches = (node, selectors) => selectors.split(',').some(selector => {
    const chain = selector.trim().split(/\s+/);
    let item = node;
    if (!part(item, chain.pop())) return false;
    while (chain.length) {
      const previous = chain.pop();
      do { item = item.parentElement; } while (item && !part(item, previous));
      if (!item) return false;
    }
    return true;
  });
  let json;
  class Element {
    constructor(tag, attrs = {}, ...children) {
      this.tagName = tag.toUpperCase(); this.attrs = attrs; this.children = []; this.style = {};
      this.textContent = ''; this.innerHTML = '';
      this.append(...children);
    }
    get id() { return this.attrs.id || ''; }
    set id(id) { this.attrs.id = id; }
    get href() { return this.attrs.href ? new URL(this.attrs.href, location).href : ''; }
    get src() { return this.attrs.src ? new URL(this.attrs.src, location).href : ''; }
    get dateTime() { return this.attrs.datetime; }
    get innerText() { return [this.textContent, ...this.children.map(e => e.innerText)].filter(Boolean).join('\n'); }
    get dataset() {
      return Object.fromEntries(Object.entries(this.attrs).filter(([k]) => k.startsWith('data-'))
        .map(([k, v]) => [k.slice(5).replace(/-([a-z])/g, (_, ch) => ch.toUpperCase()), v]));
    }
    getAttribute(k) { return this.attrs[k] ?? null; }
    append(...children) { for (const child of children) { child.parentElement = this; this.children.push(child); } }
    remove() { this.parentElement.children = this.parentElement.children.filter(child => child !== this); this.parentElement = null; }
    closest(selector) { return matches(this, selector) ? this : this.parentElement?.closest(selector) ?? null; }
    contains(node) { return node === this || this.children.some(child => child.contains(node)); }
    querySelectorAll(selector) { return this.children.flatMap(e => [...(matches(e, selector) ? [e] : []), ...e.querySelectorAll(selector)]); }
    querySelector(selector) { return this.querySelectorAll(selector)[0] ?? null; }
    click() { this.onclick?.(); }
  }
  const el = (...args) => new Element(...args);
  const text = (tag, attrs, content) => Object.assign(el(tag, attrs), { textContent: content, innerHTML: content });
  const body = el('body');
  const document = { body, title: 'Fixture', createElement: el, querySelector: s => body.querySelector(s),
    getElementById: id => body.querySelector(`[id="${id}"]`) };
  class DownloadURL extends URL {
    static createObjectURL(blob) { json = JSON.parse(blob.parts[0]); return 'blob:fixture'; }
  }
  const ctx = { document, location, URL: DownloadURL, Blob: class { constructor(parts) { this.parts = parts; } },
    MutationObserver: class { observe() {} disconnect() {} }, setTimeout() {} };
  return { el, text, body, document, ctx, saved: () => json };
}

function bookmark(htmlName, jsName) {
  const read = name => readFileSync(new URL(`../tools/${name}`, import.meta.url), 'utf8');
  const code = decodeURIComponent(read(htmlName).match(/href="javascript:([^"]+)"/)[1]);
  assert.equal(code, read(jsName), `${htmlName} must match its source byte-for-byte`);
  return code;
}

{
  const { el, text, body, document, ctx, saved } = fixture('https://x.com/Alice/status/1');
  const avatarUrl = id => `https://pbs.twimg.com/profile_images/${id}/pic_normal.jpg`;
  const userAvatar = (handle, src) => el('div', { 'data-testid': 'Tweet-User-Avatar' }, el('a', { href: `/${handle}` }, el('img', { src })));
  const tweet = (handle, id, ...content) => el('article', { 'data-testid': 'tweet' }, ...content,
    text('div', { 'data-testid': 'User-Name' }, handle),
    el('div', { 'data-testid': 'User-Name' }, el('a', { href: `/${handle}/status/${id}` }, el('time', { datetime: '2026-11-21' }))),
    text('div', { 'data-testid': 'tweetText' }, `post ${id}`));
  const mainAvatar = userAvatar('alice', avatarUrl(1));
  const quoted = tweet('Alice', 91, userAvatar('Alice', avatarUrl(91)));
  const inlineQuote = el('div', { role: 'link' }, userAvatar('Alice', avatarUrl(92)));
  const main = tweet('Alice', 1, quoted, inlineQuote, mainAvatar);
  const miss = tweet('Carol', 3, userAvatar('Mallory', avatarUrl(3)),
    el('blockquote', {}, userAvatar('Carol', avatarUrl(93))),
    el('a', { href: '/Carol' }, el('img', { src: avatarUrl(95) })),
    userAvatar('Carol', 'https://pbs.twimg.com/media/photo.jpg'));
  miss.querySelector('[data-testid="tweetText"]').append(userAvatar('Carol', avatarUrl(94)));
  const region = el('main', { 'data-testid': 'primaryColumn' }, main, miss);
  body.append(region);
  runInNewContext(bookmark('x-bookmark.html', 'x-export.js'), ctx);
  mainAvatar.remove(); // a later render of the same post may not show its avatar
  region.append(tweet('Bob', 2, userAvatar('Bob/', avatarUrl(2))),
    tweet('Dan', 4, userAvatar('Dan', avatarUrl(4)), userAvatar('Dan', avatarUrl(5))));
  document.getElementById('sf-x-export').querySelectorAll('button').find(b => b.textContent === '保存 JSON').click();
  const json = saved();
  assert.equal(json.author.avatarUrl, avatarUrl(1));
  assert.equal(json.author.handle, 'Alice');
  assert.deepEqual(json.comments.map(r => [r.id, r.author.avatarUrl]), [['3', undefined], ['2', avatarUrl(2)], ['4', undefined]]);
}

{
  const { el, text, body, document, ctx, saved } = fixture('https://forum.example/t/example/12');
  const avatarUrl = id => `https://cdn.example/user_avatar/forum.example/${id}/48/1.png`;
  const userAvatar = (handle, src) => el('div', { class: 'topic-avatar' },
    el('a', { 'data-user-card': handle }, el('img', { class: 'avatar', src })));
  const names = handle => el('div', { class: 'topic-meta-data' }, el('div', { class: 'names' },
    el('span', { class: handle === 'Bob' ? 'first full-name' : 'username' }, text('a', { 'data-user-card': handle }, handle))));
  const post = (handle, id, ...content) => el('article', { id: `post_${id}`, 'data-post-id': `${100 + id}`, 'data-user-id': `${id}` },
    ...content, names(handle), text('div', { class: 'cooked' }, `post ${id}`));
  const mainAvatar = userAvatar('alice', avatarUrl(1));
  mainAvatar.querySelector('img').currentSrc = avatarUrl('alice-retina');
  const nested = el('div', { class: 'embedded-posts' }, names('Mallory'), userAvatar('Alice', avatarUrl(91)), post('Quoter', 91));
  const cooked = () => el('div', { class: 'cooked' }, names('Mallory'), userAvatar('Carol', avatarUrl(92)));
  const stream = el('div', { class: 'post-stream' }, post('Alice', 1, nested, mainAvatar),
    post('Carol', 3, userAvatar('Mallory', avatarUrl(3)), userAvatar('Carol', 'data:image/png;base64,abc')));
  stream.querySelector('[id="post_3"]').querySelector('.cooked').append(cooked());
  body.append(stream);
  runInNewContext(bookmark('bookmark.html', 'discourse-export.js'), ctx);
  mainAvatar.remove();
  stream.append(post('Bob', 2, userAvatar('Bob', '/user_avatar/forum.example/bob/48/2.png')),
    post('Dan', 4, userAvatar('Dan', avatarUrl(4)), userAvatar('Dan', avatarUrl(5))),
    el('article', { id: 'post_5', 'data-post-id': '105', 'data-user-id': '5' }, userAvatar('Mallory', avatarUrl(5)), cooked()));
  document.getElementById('sf-save-visible').click();
  assert.deepEqual(saved().posts.map(r => [r.post_number, r.username, r.avatar_template]), [
    [1, 'Alice', avatarUrl('alice-retina')], [2, 'Bob', 'https://forum.example/user_avatar/forum.example/bob/48/2.png'],
    [3, 'Carol', undefined], [4, 'Dan', undefined],
  ]);
}

console.log('Avatar export: both bookmarks match source; matched authors only, with retained evidence across renders.');
