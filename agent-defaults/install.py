"""Install user-requested default permission flags; no credentials are read."""
import pathlib,os,datetime,json,shlex,subprocess
h=pathlib.Path.home();bin=h/'.local/bin';root=h/'.local/libexec/agent-defaults';root.mkdir(parents=True,exist_ok=True,mode=0o700)
if (root/'manifest.json').exists():raise SystemExit('Already installed; inspect ~/.local/libexec/agent-defaults/manifest.json before changing launchers.')
backup=root/('backup-'+datetime.datetime.now().strftime('%Y%m%d-%H%M%S'));backup.mkdir(mode=0o700)
manifest={}
for name in ['kimi','opencode2','agy']:
 p=bin/name
 if p.is_symlink():
  target=str(p.resolve());os.symlink(target,backup/name)
 else:
  target=str(root/(name+'-original'))
  if pathlib.Path(target).exists():raise SystemExit('Original already exists; inspect before reinstalling: '+name)
  p.rename(target);os.symlink(target,backup/name)
 manifest[name]={'original':target,'backup':str(backup/name)}
 q=shlex.quote(target)
 if name=='kimi':
  body=f'''case "${{1-}}" in
  export|fork|provider|session|acp|web|server|rc|remote|login|doctor|vis|install-desktop|app|migrate|upgrade|update|help|--help|-h|--version|-V) exec {q} "$@" ;;
esac
exec {q} --auto "$@"
'''
 elif name=='opencode2':
  body=f'''case "${{1-}}" in
  run) shift; exec {q} run --auto "$@" ;;
  upgrade|update|uninstall|acp|api|debug|auth|mcp|plugin|models|stats|mini|session|service|reload|pair|serve|help|--help|-h|--version|-v) exec {q} "$@" ;;
esac
exec {q} --auto "$@"
'''
 else:
  body=f'''case "${{1-}}" in
  agent|agents|changelog|help|install|mcp|mic-serve|models|plugin|plugins|remote-control|update|--help|-h|--version) exec {q} "$@" ;;
esac
exec {q} --dangerously-skip-permissions "$@"
'''
 if p.is_symlink():p.unlink()
 p.write_text('#!/bin/sh\n# User-authorized default permissions; originals and rollback links in ~/.local/libexec/agent-defaults.\n'+body);p.chmod(0o755)
 subprocess.run(['/bin/sh','-n',str(p)],check=True)
(root/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');(root/'manifest.json').chmod(0o600)
print('Installed: kimi --auto; opencode2 TUI/run --auto; agy --dangerously-skip-permissions. Pi already runs tools without built-in approvals.')
