import shutil
import os,pty,select,time,json,tempfile,uuid
from pathlib import Path
folder=Path(tempfile.mkdtemp(prefix='hub-hidden-')).resolve()
script=Path(__file__).resolve().parents[2]/'model-access'/'configure-provider.mjs'
(folder/'pending').mkdir(mode=0o700)
(folder/'providers.json').write_text(json.dumps({'providers':{}}))
id=str(uuid.uuid4())
draft={'id':id,'status':'waiting','createdAt':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'type':'add','prefix':'fixture','name':'Fixture','baseURL':'https://fixture.invalid/v1','billing':'test only','models':[{'id':'coder','name':'Coder','contextWindow':4096,'maxTokens':128,'enabled':True}]}
(folder/'pending'/f'{id}.json').write_text(json.dumps(draft))
pid,fd=pty.fork()
if pid==0:
 os.execve(shutil.which('node'),['node',str(script),id],dict(os.environ,MODEL_HUB_DATA_DIR=str(folder)))
output=b'';sent=False;deadline=time.time()+15
while time.time()<deadline:
 ready,_,_=select.select([fd],[],[],0.2)
 if ready:
  try:data=os.read(fd,8192)
  except OSError:break
  if not data:break
  output+=data
 if not sent and 'API Key（隐藏输入）：'.encode() in output:
  os.write(fd,b'FAKE-HIDDEN-KEY-ONLY\n');sent=True
try:os.close(fd)
except OSError:pass
assert sent,'No hidden input prompt'
assert b'FAKE-HIDDEN-KEY-ONLY' not in output,'Key was echoed'
assert (folder/'keys/local-fixture.key').read_text().strip()=='FAKE-HIDDEN-KEY-ONLY'
assert json.loads((folder/'pending'/f'{id}.json').read_text())['status']=='complete'
assert 'FAKE-HIDDEN-KEY-ONLY' not in (folder/'providers.json').read_text()
print('Hidden Terminal input verified: no echo, private local save, no key in provider metadata.')
for p in sorted(folder.rglob('*'),reverse=True):
 if p.is_file():p.unlink()
 elif p.is_dir():p.rmdir()
folder.rmdir()
