"""Prepare a verified, user-local official Node runtime without changing system Node."""
import hashlib,os,pathlib,platform,posixpath,re,shutil,subprocess,tarfile,tempfile,urllib.error,urllib.request
ORIGIN='https://nodejs.org/download/release'
def major(node):
 try:return int(subprocess.check_output([str(node),'-p','process.versions.node.split(".")[0]'],text=True,stderr=subprocess.DEVNULL).strip())
 except (OSError,ValueError,subprocess.SubprocessError):return 0
def download(url,path):
 if not url.startswith(ORIGIN+'/'):raise ValueError('只允许从 Node.js 官方源下载')
 try:
  with urllib.request.urlopen(url,timeout=90) as response,open(path,'wb') as out:shutil.copyfileobj(response,out)
 except (OSError,urllib.error.URLError):
  # Some macOS Python installations do not ship a CA bundle; curl uses OS trust.
  # TLS verification remains enabled; never use -k or an untrusted mirror.
  curl=shutil.which('curl')
  if not curl:raise ValueError('Node 官方下载失败，请检查网络、系统证书或 HTTPS_PROXY')
  result=subprocess.run([curl,'--fail','--silent','--show-error','--location','--proto','=https','--proto-redir','=https','--connect-timeout','20','--max-time','300','--output',str(path),url],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  if result.returncode:raise ValueError('Node 官方下载失败，请检查网络或 HTTPS_PROXY；也可自行安装 Node.js 24+')
def artifact(checksums,system,machine):
 arch={'arm64':'arm64','aarch64':'arm64','x86_64':'x64','AMD64':'x64'}.get(machine)
 osname={'Darwin':'darwin','Linux':'linux'}.get(system)
 if not arch or not osname:raise ValueError('此架构暂不支持自动安装 Node，请自行安装 Node.js 24+')
 matches=[]
 for line in checksums.splitlines():
  parts=line.split()
  if len(parts)!=2:continue
  digest,name=parts
  if re.fullmatch(r'[a-f0-9]{64}',digest) and re.fullmatch(r'node-v24\.\d+\.\d+-'+osname+'-'+arch+r'\.tar\.gz',name):matches.append((name,digest))
 if len(matches)!=1:raise ValueError('Node 官方校验清单不符合预期，停止安装')
 return matches[0]
def unpack(archive,destination,top):
 with tarfile.open(archive,'r:gz') as tar:
  members=tar.getmembers()
  for m in members:
   name=posixpath.normpath(m.name)
   if name!=top and not name.startswith(top+'/'):raise ValueError('Node 安装包包含非法路径')
   if m.isdev() or m.isfifo():raise ValueError('Node 安装包包含非法文件类型')
   if m.issym() or m.islnk():
    target=posixpath.normpath(posixpath.join(posixpath.dirname(name),m.linkname) if m.issym() else m.linkname)
    if target!=top and not target.startswith(top+'/'):raise ValueError('Node 安装包包含越界链接')
  tar.extractall(destination,members=members)
def ensure_node(home,system,dry_run=False,auto=True):
 current=shutil.which('node')
 if current and major(current)>=24:return str(pathlib.Path(current).resolve())
 root=home/'.local/share/model-quota-manager/node'
 for p in sorted(root.glob('node-v24.*/bin/node'),reverse=True):
  if major(p)>=24:return str(p.resolve())
 if not auto:raise ValueError('需要 Node.js 24+；已关闭自动准备运行时')
 if dry_run:
  print('当前 Node 缺失或版本低于 24；正式运行会下载官方 Node 24，校验 SHA-256 后安装到工具私有目录。');return None
 print('当前 Node 缺失或版本低于 24，正在准备工具专用 Node.js 24（不修改系统 Node）…',flush=True)
 root.mkdir(parents=True,exist_ok=True,mode=0o700)
 with tempfile.TemporaryDirectory(prefix='node-setup-',dir=root) as temp:
  work=pathlib.Path(temp);manifest=work/'SHASUMS256.txt';download(ORIGIN+'/latest-v24.x/SHASUMS256.txt',manifest)
  name,digest=artifact(manifest.read_text(),system,platform.machine());version=name.split('-')[1];archive=work/name
  download(ORIGIN+'/'+version+'/'+name,archive)
  checksum=hashlib.sha256()
  with archive.open('rb') as f:
   for chunk in iter(lambda:f.read(1024*1024),b''):checksum.update(chunk)
  if checksum.hexdigest()!=digest:raise ValueError('Node 安装包 SHA-256 校验失败，未安装')
  top=name.removesuffix('.tar.gz');unpack(archive,work,top);candidate=work/top/'bin/node'
  if major(candidate)!=24:raise ValueError('下载的 Node 运行验证失败；Linux 可能需要较新的 glibc')
  target=root/top
  if target.exists():raise ValueError('专用 Node 目录已存在但不可用，请检查本机目录，未覆盖')
  (work/top).rename(target)
 print('工具专用 Node.js 24 已准备并通过校验。',flush=True)
 return str((target/'bin/node').resolve())
