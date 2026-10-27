#!/usr/bin/env python3
"""Deploy the local app and register a per-user macOS LaunchAgent; never print secrets."""
from pathlib import Path
import os, plistlib, shutil, subprocess
source=Path(__file__).resolve().parent
home=Path.home()
base=home/'Library/Application Support/ModelQuotaManager'
runtime=base/'model-quota-manager'
label='local.model-quota.manager'
plist=home/'Library/LaunchAgents'/f'{label}.plist'
domain=f'gui/{os.getuid()}'
node_path=shutil.which('node')
if not node_path: raise RuntimeError('Node.js 24+ is required')
node=Path(node_path).resolve(strict=True)
subprocess.run(['launchctl','bootout',f'{domain}/{label}'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
base.mkdir(parents=True,exist_ok=True,mode=0o700)
base.chmod(0o700)
for name in ['model-quota-manager','model-access']:
    origin=source if name=='model-quota-manager' else source.parent/name
    target=base/name
    shutil.copytree(origin,target,dirs_exist_ok=True,symlinks=True,ignore=shutil.ignore_patterns('.local','.git','node_modules','__pycache__'))
data=runtime/'.local'
original=source/'.local'
if not data.exists():
    if original.is_symlink():
        raise RuntimeError('Existing data link must be checked before deployment')
    if original.exists(): shutil.move(str(original),str(data))
    else: data.mkdir(mode=0o700)
data.chmod(0o700)
if not original.exists(): original.symlink_to(data,target_is_directory=True)
for name in ['service.log','service-error.log']:
    p=data/name;p.touch(mode=0o600,exist_ok=True);p.chmod(0o600)
config={'Label':label,'ProgramArguments':[str(node),str(runtime/'server.mjs')],'WorkingDirectory':str(runtime),'RunAtLoad':True,'KeepAlive':True,'ThrottleInterval':10,'ProcessType':'Background','StandardOutPath':str(data/'service.log'),'StandardErrorPath':str(data/'service-error.log'),'EnvironmentVariables':{'PATH':str(home/'.local/bin')+':/usr/bin:/bin:/usr/sbin:/sbin','PORT':'4318','QUOTA_DATA_DIR':str(data),'MODEL_HUB_DATA_DIR':str(home/'Library/Application Support/ModelHub/.local')}}
plist.parent.mkdir(parents=True,exist_ok=True)
plist.write_bytes(plistlib.dumps(config));plist.chmod(0o600)
# Machine-specific LaunchAgent is kept outside the source checkout.
subprocess.run(['plutil','-lint',str(plist)],check=True)
subprocess.run(['launchctl','bootstrap',domain,str(plist)],check=True)
print('已部署并启用登录自启动：http://127.0.0.1:4318/')
