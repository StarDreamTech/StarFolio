"""No network: one profile lookup per identity and no work on the second pass."""
import contextlib, hashlib, importlib.util, io, json, tempfile
from pathlib import Path
from unittest.mock import patch
script = Path(__file__).resolve().parents[1] / 'tools/backfill-x-avatars.py'
spec = importlib.util.spec_from_file_location('backfill',script)
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)
with tempfile.TemporaryDirectory() as tmp:
    base=Path(tmp); root=base/'public'; rel='x/alice/posts/1.json'; f=root/rel; f.parent.mkdir(parents=True)
    def row(user, n):
        profile='https://x.com/'+user
        return dict(id=str(n),url=f'{profile}/status/{n}',author=dict(handle=user,profileUrl=profile))
    bad=row('bob',3); bad['author']['profileUrl']='https://x.com/alice'
    data=row('alice',1)|{'comments':[row('alice',2),bad]}; f.write_text(json.dumps(data)); m.save_json(root/'catalog.json',{'records':[{'path':rel,'sha256':hashlib.sha256(f.read_bytes()).hexdigest()}]})
    url='https://pbs.twimg.com/profile_images/1/photo.png';png=b'\x89PNG\r\n\x1a\nexample'; calls=[]
    def get(u,head=False):
        calls.append(u);return {'og:url':u,'og:image':url} if head else png
    with patch.object(m,'get',get),patch.object(m.time,'sleep'),contextlib.redirect_stdout(io.StringIO()):
        assert m.run(root,base/'progress') is None
        assert calls==['https://x.com/alice',url];idx=root/'avatars/index.json';stamp=idx.stat().st_mtime_ns
        calls.clear();assert m.run(root,base/'progress') is None
        assert calls==[] and idx.stat().st_mtime_ns==stamp
        obj=next((root/'avatars/objects').iterdir());obj.write_bytes(b'damaged');m.run(root,base/'progress');assert obj.read_bytes()==png
print('X backfill: identity match, reuse, zero requests on rerun, corrupt-object repair OK')
