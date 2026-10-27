#!/usr/bin/env python3
"""Portable local setup. Never accepts secrets in argv or downloads account data."""
import argparse,getpass,json,os,pathlib,platform,secrets,shlex,shutil,subprocess,sys,time,urllib.request,plistlib
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from node_runtime import ensure_node
ROOT=pathlib.Path(__file__).resolve().parents[1]
PRESETS={
 'zenmux':('ZenMux','https://zenmux.ai/api/v1','ZenMux 订阅 / 钱包',['anthropic/claude-sonnet-5.5','anthropic/claude-opus-5.5','anthropic/claude-haiku-5.5']),
 'glm':('GLM Coding Plan','https://open.bigmodel.cn/api/coding/paas/v4','GLM 国内 Coding Plan',['glm-5.3','glm-5.3-flash','glm-5.3-flashx']),
 'minimax':('MiniMax 国内订阅','https://api.minimax.cn/v1','MiniMax Coding / Token Plan',['MiniMax-M3.1-Flash']),
 'deepseek':('DeepSeek API','https://api.deepseek.com','DeepSeek API 钱包',['deepseek-flash','deepseek-v4-pro']),
 'kimi':('Kimi Code','https://api.kimi.com/coding/v1','Kimi Code 订阅',['k3.1-pro','k3.1-pro-highspeed'])}
def read(p,fallback=None):
 return json.loads(p.read_text()) if p.exists() else (fallback if fallback is not None else {})
def write(p,v):
 p.parent.mkdir(parents=True,exist_ok=True,mode=0o700);t=p.with_name(p.name+'.setup-tmp')
 t.write_text(v if isinstance(v,str) else json.dumps(v,ensure_ascii=False,indent=2)+'\n');t.chmod(0o600);t.replace(p)
def backup(p,folder):
 if p.exists():
  folder.mkdir(parents=True,exist_ok=True,mode=0o700);target=folder/(p.parent.name+'-'+p.name);shutil.copy2(p,target);target.chmod(0o600)
def ask(prompt):return input(prompt+' [y/N] ').strip().lower() in ['y','yes']
def selected(prompt,allowed):
 s=input(prompt+'（逗号分隔，回车跳过）：').strip()
 values=[v.strip() for v in s.split(',') if v.strip()]
 if any(v not in allowed for v in values):raise ValueError('未知选项，未保存')
 return list(dict.fromkeys(values))
def hidden(label):
 if not sys.stdin.isatty():raise ValueError('Key 必须在本机交互终端隐藏输入')
 key=getpass.getpass(label+'：').strip()
 if not key or not key.isascii() or any(c.isspace() for c in key) or any(c in key for c in ['"','\\']):raise ValueError('Key 格式无效')
 return key

def configure(hub,quota,interactive,backups):
 regfile=hub/'.local/providers.json';registry=read(regfile,{'proxy':'','providers':{}})
 if interactive:
  print('\n模型入口：zenmux / glm / minimax / deepseek / kimi。ZenMux 只预设 Claude。')
  for prefix in selected('添加或轮换模型 provider',PRESETS):
   name,url,billing,ids=PRESETS[prefix];pid='local-'+prefix
   old=registry['providers'].get(pid)
   if old and not ask(name+' 已配置，是否轮换 Key？'):continue
   key=hidden(name+' 推理 API Key（不是 Management Key）');kp=hub/'.local/keys'/(pid+'.key');backup(kp,backups);write(kp,key+'\n')
   registry['providers'][pid]=old or {'prefix':prefix,'name':name,'baseURL':url,'billing':billing,'enabled':True,'keyFile':str(kp),'models':[{'id':mid,'name':mid.split('/')[-1],'contextWindow':262144 if prefix=='kimi' else 128000,'maxTokens':8192,'reasoning':prefix!='zenmux','input':['text']} for mid in ids]}
  print('\n额度 Key：zenmux=Management Key；glm/minimax=订阅 Key；deepseek=API Key。与推理 Key 独立保存。')
  cp=quota/'.local/credentials.json';credentials=read(cp)
  for provider in selected('配置额度查询：zenmux / glm / minimax / deepseek',['zenmux','glm','minimax','deepseek']):
   if provider in credentials and not ask(provider+' 已配置，是否轮换额度 Key？'):continue
   key=hidden(provider+' 额度查询 Key');credentials[provider]={'key':key,**({'region':'cn'} if provider=='minimax' else {})}
  backup(cp,backups);write(cp,credentials)
 backup(regfile,backups);write(regfile,registry)
 token=hub/'.local/hub.key'
 if not token.exists():write(token,'hub-'+secrets.token_hex(32)+'\n')
 return registry

def connect_agents(node,hub,home,registry,backups):
 if not any(p.get('enabled') and p.get('models') for p in registry['providers'].values()):
  print('入口尚无 provider；稍后在网页或 modelhub add 配置，模型配置暂不导出。');return
 env=dict(os.environ,HOME=str(home),MODEL_HUB_DATA_DIR=str(hub/'.local'))
 subprocess.run([node,str(hub/'model-access.mjs'),'sync'],env=env,check=True)
 if not shutil.which('claude'):return
 p=home/'.claude/settings.json';c=read(p);backup(p,backups)
 js="import {catalog,read,registryPath,tokenPath} from './hub-config.mjs';import {anthropicEndpoint} from './anthropic.mjs';console.log(JSON.stringify({tokenPath,models:catalog(read(registryPath)).filter(x=>anthropicEndpoint(x.p)).map(x=>({id:x.id,name:x.m.name}))}));"
 d=json.loads(subprocess.check_output([node,'--input-type=module','-e',js],cwd=hub,env=env,text=True))
 if not d['models']:return
 c['apiKeyHelper']='cat '+shlex.quote(d['tokenPath']);e=c.setdefault('env',{});e.update({'ANTHROPIC_BASE_URL':'http://127.0.0.1:4319','ANTHROPIC_API_KEY':'','ANTHROPIC_AUTH_TOKEN':''})
 e['NO_PROXY']=','.join(dict.fromkeys((e.get('NO_PROXY','')+',127.0.0.1,localhost,::1').split(',')))
 ids={m['id'] for m in d['models']}
 if c.get('model') not in ids:c['model']=next((i for i in ids if 'claude-sonnet' in i),d['models'][0]['id'])
 picker=c.setdefault('modelPicker',{});old=picker.get('options',[])
 # Keep unrelated custom entries and compatibility metadata, replace only hub entries.
 byid={o['model']:o for o in old};prefixes={i.split('/')[0] for i in ids}
 picker['options']=[o for o in old if o['model'].split('/')[0] not in prefixes]+[{**byid.get(m['id'],{}),'model':m['id'],'label':'hub/'+m['name'].split(':')[-1].strip().replace('claude-',''),'description':m['id'].split('/')[0]+'/'+m['id'].split('/')[-1]} for m in d['models']]
 picker['replaceBuiltInOptions']=True;write(p,c)
 print('已配置 pi / opencode2；检测到 Claude Code 时同步其菜单。原生登录与权限模式不变。')

def service_config(node,hub,quota,home):
 bindir=home/'.local/bin';path=str(pathlib.Path(node).parent)+os.pathsep+str(bindir)+os.pathsep+os.environ.get('PATH','/usr/bin:/bin')
 return [('local.model-access.hub',hub,'hub.mjs',{'MODEL_HUB_DATA_DIR':str(hub/'.local'),'PATH':path}),('local.model-quota.manager',quota,'server.mjs',{'QUOTA_DATA_DIR':str(quota/'.local'),'MODEL_HUB_DATA_DIR':str(hub/'.local'),'PORT':'4318','PATH':path})]
def install_services(node,hub,quota,home,start,system):
 for label,cwd,script,env in service_config(node,hub,quota,home):
  data=cwd/'.local';data.mkdir(mode=0o700,exist_ok=True)
  if system=='Darwin':
   p=home/'Library/LaunchAgents'/(label+'.plist');p.parent.mkdir(parents=True,exist_ok=True)
   config={'Label':label,'ProgramArguments':[node,str(cwd/script)],'WorkingDirectory':str(cwd),'RunAtLoad':True,'KeepAlive':True,'ThrottleInterval':10,'EnvironmentVariables':env,'StandardOutPath':str(data/'service.log'),'StandardErrorPath':str(data/'service-error.log')}
   p.write_bytes(plistlib.dumps(config));p.chmod(0o600)
   if start:
    domain='gui/'+str(os.getuid());subprocess.run(['launchctl','bootout',domain+'/'+label],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL);subprocess.run(['launchctl','bootstrap',domain,str(p)],check=True)
  else:
   # systemd quoting supports whitespace in clone/home paths, without shell evaluation.
   quote=lambda v:'"'+v.replace('\\','\\\\').replace('"','\\"').replace('%','%%')+'"'
   p=home/'.config/systemd/user'/(label+'.service')
   unit='[Unit]\nDescription=Local model service\n[Service]\nType=simple\nWorkingDirectory='+quote(str(cwd))+'\nExecStart='+quote(node)+' '+quote(str(cwd/script))+'\nRestart=on-failure\nUMask=0077\n'+''.join('Environment='+quote(k+'='+v)+'\n' for k,v in env.items())+'\n[Install]\nWantedBy=default.target\n'
   write(p,unit)
   if start:subprocess.run(['systemctl','--user','daemon-reload'],check=True);subprocess.run(['systemctl','--user','enable','--now',label+'.service'],check=True);subprocess.run(['systemctl','--user','restart',label+'.service'],check=True)

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--non-interactive',action='store_true',help='安装空服务；不读取凭据、不改 agent 配置');parser.add_argument('--no-start',action='store_true',help='部署并生成启动项，但不启动服务');parser.add_argument('--home',type=pathlib.Path,default=pathlib.Path.home(),help='测试/自定义本机目录，不能与启动服务同时使用');parser.add_argument('--dry-run',action='store_true');parser.add_argument('--no-install-node',action='store_true',help='禁止自动下载专用 Node 运行时');args=parser.parse_args()
 system=platform.system()
 if system not in ['Darwin','Linux']:raise ValueError('当前支持 macOS / Linux；Windows 请使用 WSL2')
 home=args.home.expanduser().resolve()
 if home!=pathlib.Path.home().resolve() and not args.no_start:raise ValueError('--home 必须同时使用 --no-start')
 node=ensure_node(home,system,dry_run=args.dry_run,auto=not args.no_install_node)
 if not args.non_interactive and not sys.stdin.isatty():raise ValueError('请在交互终端运行，或使用 --non-interactive 部署空服务')
 base=home/('Library/Application Support' if system=='Darwin' else '.local/share');hub=base/'ModelHub';quota=base/'ModelQuotaManager/model-quota-manager'
 if args.dry_run:print('将部署额度面板、模型入口、启动项和本机配置；不读取旧机器数据。');return
 os.environ['PATH']=str(pathlib.Path(node).parent)+os.pathsep+os.environ.get('PATH','')
 os.umask(0o077);backups=hub/'.local/backups'/('setup-'+time.strftime('%Y%m%d-%H%M%S'))
 for src,dst in [(ROOT/'model-access',hub),(ROOT/'model-quota-manager',quota),(ROOT/'model-access',quota.parent/'model-access')]:
  shutil.copytree(src,dst,dirs_exist_ok=True,ignore=shutil.ignore_patterns('.local','.git','node_modules','__pycache__','*.log','*.plist'))
  dst.chmod(0o700)
 registry=configure(hub,quota,not args.non_interactive,backups)
 bin=home/'.local/bin';bin.mkdir(parents=True,exist_ok=True)
 p=bin/'modelhub'
 marker='# model-quota-manager managed launcher'
 if p.exists() or p.is_symlink():
  ours=p.is_symlink() and p.resolve()==hub/'model-access.mjs'
  if not ours and not p.is_symlink():ours=marker in p.read_text(errors='replace')[:150]
  if not ours:raise ValueError('modelhub 已被其他程序占用，未覆盖')
  backup(p,backups)
  if p.is_symlink():p.unlink()
 write(p,'#!/bin/sh\n'+marker+'\nexport PATH='+shlex.quote(str(pathlib.Path(node).parent))+':"$PATH"\nexec '+shlex.quote(node)+' '+shlex.quote(str(hub/'model-access.mjs'))+' "$@"\n');p.chmod(0o700)
 (hub/'model-access.mjs').chmod(0o700)
 if not args.non_interactive:connect_agents(node,hub,home,registry,backups)
 install_services(node,hub,quota,home,not args.no_start,system)
 if not args.no_start:
  for port,route in [(4318,'/'),(4319,'/health')]:
   for attempt in range(30):
    try:
     with urllib.request.build_opener(urllib.request.ProxyHandler({})).open('http://127.0.0.1:'+str(port)+route,timeout=2) as response:
      if response.status==200:break
    except Exception:time.sleep(.3)
   else:raise ValueError('服务 '+str(port)+' 未就绪，请检查本机 .local/service-error.log；不要分享原始日志')
  print('两个服务已启动并通过本机 HTTP 检查。')
 print('额度页面：http://127.0.0.1:4318/；模型入口：http://127.0.0.1:4319/v1')
 print('浏览器扩展目录：'+str(quota/'grok-browser-extension'))
 print('首次需要手动加载 Chrome 扩展并绑定 Grok/Gemini/MiniMax/DeepSeek 目标账号。')
 print('Kimi OAuth：'+shlex.quote(node)+' '+shlex.quote(str(quota/'setup-kimi-code.mjs')))
 print('Codex：先在官方 CLI 登录，再在网页设置连接本机账号。')
 print('将 ~/.local/bin 加入 PATH，可使用 modelhub add / sync / status。')
 print('重复运行会保留已配置数据；轮换 Key 会确认并备份。初始化不自动发起付费推理。')
if __name__=='__main__':
 try:main()
 except (ValueError,OSError,subprocess.SubprocessError) as exc:print('初始化未完成：'+str(exc) if isinstance(exc,ValueError) else '初始化未完成：依赖、文件或服务操作失败，请检查本机环境。',file=sys.stderr);sys.exit(1)
 except KeyboardInterrupt:print('\n已取消；已保存的本机配置保留。',file=sys.stderr);sys.exit(130)
