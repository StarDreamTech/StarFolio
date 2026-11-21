#!/usr/bin/env python3
"""Enrich an existing X collection. Reads public profile head metadata, never post bodies or auth.
Usage: python3 tools/backfill-x-avatars.py COLLECTION PRIVATE_CHECKPOINT_DIRECTORY
"""
import collections, datetime, hashlib, json, os, re, subprocess, sys, tempfile, time
from html.parser import HTMLParser
from pathlib import Path
from urllib import error

class Head(HTMLParser):
    def __init__(self): super().__init__(); self.meta={}
    def handle_starttag(self, tag, attrs):
        a=dict(attrs)
        if tag=='meta' and a.get('property') in ('og:url','og:image'): self.meta[a['property']]=a.get('content','')
def today(): return datetime.datetime.now(datetime.timezone.utc).date().isoformat()
def save(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=path.parent,delete=False) as f: f.write(data); tmp=f.name
    os.replace(tmp,path)
def save_json(p,v): save(p,(json.dumps(v,ensure_ascii=False,sort_keys=True,indent=2)+'\n').encode())
def load(p): return json.loads(p.read_bytes()) if p.exists() else {}
def picurl(u): return bool(re.fullmatch(r'https://pbs\.twimg\.com/(?:profile_images|sticky/default_profile_images)/[A-Za-z0-9_./%-]+',u)) and '..' not in u
def image_ext(b):
    if b.startswith(b'\x89PNG\r\n\x1a\n'): return 'png'
    if b.startswith(b'\xff\xd8\xff'): return 'jpg'
    if b[:6] in (b'GIF89a',b'GIF87a'): return 'gif'
    if b[:4]==b'RIFF' and b[8:12]==b'WEBP': return 'webp'
    raise ValueError('invalid raster')
def get(url,head=False):
    # Use the system trust store. Never follow redirects or disable TLS checks; do not retain profiles.
    cmd=['curl','-q','--silent','--show-error','--proto','=https','--max-time','14','--max-filesize',str(2<<20),'--write-out','\n%{http_code}',url]
    for attempt in range(2):
        proc=subprocess.run(cmd,capture_output=True)
        if not proc.returncode: break
        if proc.returncode in (51,58,60,77,83,90,91): raise RuntimeError(f'TLS certificate/trust check failed (curl {proc.returncode}); stopped')
        if proc.returncode==35:
            if not attempt and any(x in proc.stderr.lower() for x in (b'eof',b'syscall',b'reset by peer')):
                time.sleep(2);continue
            raise RuntimeError('TLS handshake failed (curl 35); stopped')
        raise OSError(f'transport failure (curl {proc.returncode})')
    data,code=proc.stdout.rsplit(b'\n',1);code=int(code)
    if code!=200:raise error.HTTPError(url,code,'HTTP response',{},None)
    if head:
        p=Head();p.feed(data[:384*1024].split(b'</head>')[0].decode('utf-8','replace'));return p.meta
    return data

def run(collection, checkpoint):
    root=collection.resolve(); store=root/'avatars'; ix=store/'index.json'; cp=checkpoint/'x-avatar-retry.json'; people=collections.Counter()
    if checkpoint.resolve().is_relative_to(root):raise ValueError('checkpoints must be outside the public collection')
    for r in load(root/'catalog.json')['records']:
        if not re.fullmatch(r'x/[A-Za-z0-9_]+/posts/[0-9]+\.json',r['path']):continue
        f=(root/r['path']).resolve()
        if f!=root/r['path'] or f.stat().st_size>16<<20:raise ValueError('invalid record path or size')
        data=f.read_bytes()
        if hashlib.sha256(data).hexdigest()!=r['sha256']:raise ValueError('record checksum mismatch')
        v=json.loads(data)
        for row in [v,*v.get('comments',[])]:
            h=row['author']['handle'].lower();p=row['author'].get('profileUrl','').rstrip('/').lower();url=row['url'].lower()
            if re.fullmatch('[a-z0-9_]{1,20}',h) and p=='https://x.com/'+h and url==f'{p}/status/{row["id"]}':people[p]+=1
    idx=load(ix);retry=load(cp);by_url={};calls=0;downloads=0;added=0;stopped=None;transient=0
    for path in (store,store/'objects',ix):
        if path.resolve()!=path:raise ValueError('symlinks not allowed in avatar store')
    for p,row in list(idx.items()):
        n=row['file']
        if not re.fullmatch(r'[0-9a-f]{64}\.(png|jpg|gif|webp)',n) or not picurl(row['url']):raise ValueError('invalid avatar entry')
        f=store/'objects'/n
        if f.is_symlink():raise ValueError('symlinked avatar')
        if f.is_file() and f.stat().st_size<=2<<20 and hashlib.sha256(f.read_bytes()).hexdigest()==n.split('.')[0]:by_url[row['url']]=n
        else:del idx[p]
    for profile,_ in people.most_common():
        if profile in idx or retry.get(profile,{}).get('date')==today():continue
        try:
            calls+=1; meta=get(profile,True);u=meta.get('og:image','');canonical=meta.get('og:url','').lower().rstrip('/')
            if canonical!=profile or not picurl(u):raise ValueError('no verified profile photo')
            if u not in by_url:
                b=get(u);downloads+=1;n=hashlib.sha256(b).hexdigest()+'.'+image_ext(b);f=store/'objects'/n
                if not f.exists() or hashlib.sha256(f.read_bytes()).digest()!=hashlib.sha256(b).digest():save(f,b)
                by_url[u]=n
            idx[profile]={'url':u,'file':by_url[u],'observed':today()};save_json(ix,idx);added+=1;transient=0
        except error.HTTPError as e:
            if e.code in (401,403,429) or e.code>=500:
                stopped=f'HTTP {e.code}, retry-after={e.headers.get("Retry-After")}';break
            retry[profile]={'date':today(),'reason':f'HTTP {e.code}'};save_json(cp,retry)
        except ValueError as e:
            retry[profile]={'date':today(),'reason':str(e)};save_json(cp,retry)
        except RuntimeError as e:
            stopped=str(e);break
        except (error.URLError,OSError) as e:
            reason=getattr(e,'reason',e)

            transient+=1;retry[profile]={'date':today(),'reason':type(reason).__name__};save_json(cp,retry)
            if transient>=3:stopped=f'transport unavailable: {type(reason).__name__}';break
        if calls%50==0:print('lookups',calls,'resolved',len(idx),flush=True)
        time.sleep(.35)
    print(json.dumps({'wanted':len(people),'resolved':len(idx),'covered':sum(people[p] for p in idx),'lookups':calls,'downloads':downloads,'added':added,'stop':stopped},ensure_ascii=False),flush=True)
    return stopped
if __name__=='__main__':sys.exit(2 if run(*(Path(x) for x in sys.argv[1:])) else 0)
