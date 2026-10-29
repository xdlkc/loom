"""Install only the Rust model gateway, preserving local registry and secrets."""
import os,pathlib,platform,plistlib,shlex,shutil,subprocess,sys,time,urllib.request
home=pathlib.Path.home();system=platform.system()
if system not in ['Darwin','Linux']:sys.exit('当前服务安装器支持 macOS / Linux')
base=home/('Library/Application Support/ModelHub' if system=='Darwin' else '.local/share/ModelHub');base.mkdir(parents=True,exist_ok=True,mode=0o700)
source=pathlib.Path(sys.argv[1]).resolve();target=base/'modelhub';data=base/'.local';data.mkdir(exist_ok=True,mode=0o700)
backup=data/'backups'/('rust-install-'+str(time.time_ns()));backup.mkdir(parents=True,mode=0o700)
if target.exists():shutil.copy2(target,backup/'modelhub')
tmp=base/'modelhub.install-tmp';shutil.copy2(source,tmp);tmp.chmod(0o700);tmp.replace(target)
launcher=home/'.local/bin/modelhub';launcher.parent.mkdir(parents=True,exist_ok=True)
if launcher.exists() or launcher.is_symlink():
 shutil.copy2(launcher,backup/'previous-launcher',follow_symlinks=True);launcher.unlink()
launcher.symlink_to(target)
env=dict(os.environ,MODEL_HUB_DATA_DIR=str(data));subprocess.run([str(target),'init'],env=env,check=True)
label='local.model-access.hub'
if system=='Darwin':
 p=home/'Library/LaunchAgents'/(label+'.plist');p.parent.mkdir(parents=True,exist_ok=True)
 if p.exists():shutil.copy2(p,backup/p.name)
 domain='gui/'+str(os.getuid());subprocess.run(['launchctl','bootout',domain+'/'+label],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 v={'Label':label,'ProgramArguments':[str(target),'serve'],'WorkingDirectory':str(base),'RunAtLoad':True,'KeepAlive':True,'ThrottleInterval':10,'EnvironmentVariables':{'MODEL_HUB_DATA_DIR':str(data),'PATH':str(home/'.local/bin')+':/usr/bin:/bin'},'StandardOutPath':str(data/'rust-service.log'),'StandardErrorPath':str(data/'rust-service-error.log')}
 p.write_bytes(plistlib.dumps(v));p.chmod(0o600);subprocess.run(['launchctl','bootstrap',domain,str(p)],check=True)
else:
 p=home/'.config/systemd/user'/(label+'.service');p.parent.mkdir(parents=True,exist_ok=True)
 if p.exists():shutil.copy2(p,backup/p.name)
 q=lambda v:'"'+str(v).replace('\\','\\\\').replace('"','\\"').replace('%','%%')+'"'
 p.write_text('[Unit]\nDescription=Local Rust model gateway\n[Service]\nExecStart='+q(target)+' serve\nEnvironment='+q('MODEL_HUB_DATA_DIR='+str(data))+'\nRestart=on-failure\nUMask=0077\n[Install]\nWantedBy=default.target\n');p.chmod(0o600)
 subprocess.run(['systemctl','--user','daemon-reload'],check=True);subprocess.run(['systemctl','--user','enable','--now',label],check=True);subprocess.run(['systemctl','--user','restart',label],check=True)
op=urllib.request.build_opener(urllib.request.ProxyHandler({}))
for _ in range(30):
 try:
  with op.open('http://127.0.0.1:4319/health',timeout=2) as r:
   import json
   if json.load(r).get('runtime')=='rust':break
 except Exception:pass
 time.sleep(.3)
else:sys.exit('Rust 网关未就绪，请在本机检查 rust-service-error.log。额度工具未修改。')
print('Rust 网关已启动：http://127.0.0.1:4319/v1；无额度面板、无需 Node.js。')
print('将 ~/.local/bin 加入 PATH。使用 modelhub add / sync / check / status。')
