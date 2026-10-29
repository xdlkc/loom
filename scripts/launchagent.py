"""Reload a user LaunchAgent after its previous instance has finished unloading."""
import subprocess,time

def reload_agent(domain,label,plist):
 target=domain+'/'+label
 subprocess.run(['launchctl','bootout',target],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 for _ in range(40):
  state=subprocess.run(['launchctl','print',target],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  if state.returncode:break
  time.sleep(.1)
 else:raise RuntimeError('旧网关仍在运行，系统未允许卸载启动项；请在本机终端重试安装。')
 for _ in range(8):
  result=subprocess.run(['launchctl','bootstrap',domain,str(plist)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  if result.returncode==0:return
  time.sleep(.25)
 raise RuntimeError('Loom 启动项加载失败；原启动项已在本机备份，请检查 launchctl 状态。')
