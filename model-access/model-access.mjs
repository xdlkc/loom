#!/usr/bin/env node
import {syncAgents,ensureToken,baseURL,tokenPath,catalog,dataDir as hubDataDir} from './hub-config.mjs';
import {createInterface} from 'node:readline/promises';
import fs from 'node:fs';import path from 'node:path';import os from 'node:os';import {fileURLToPath} from 'node:url';import {spawn,spawnSync} from 'node:child_process';
const root=path.dirname(fileURLToPath(import.meta.url));
export const dataDir=hubDataDir;
const registryPath=path.join(dataDir,'providers.json');
function write(p,v){fs.mkdirSync(path.dirname(p),{recursive:true,mode:0o700});fs.writeFileSync(p+'.tmp',typeof v==='string'?v:JSON.stringify(v,null,2)+'\n',{mode:0o600});fs.chmodSync(p+'.tmp',0o600);fs.renameSync(p+'.tmp',p);}
function read(p,f={}){return fs.existsSync(p)?JSON.parse(fs.readFileSync(p,'utf8')):f;}
export function sync(home=os.homedir()){ensureToken();return syncAgents({home});}

async function addProvider(registry){
 if(!process.stdin.isTTY)throw Error('请在本机交互终端运行 modelhub add');
 const rl=createInterface({input:process.stdin,output:process.stdout});let id,name,url,billing,modelText;
 try{id=(await rl.question('Provider 标识（例如 my-provider）：')).trim();if(!/^[a-z][a-z0-9-]{1,40}$/.test(id))throw Error('标识仅支持小写字母、数字、横线');if(registry.providers['local-'+id]||catalog(registry).some(m=>m.id.startsWith(id+'/')))throw Error('Provider 已存在；轮换 Key 请用 set-key');name=(await rl.question('显示名称：')).trim()||id;url=(await rl.question('OpenAI 兼容 Base URL（https）：')).trim().replace(/\/$/,'');const u=new URL(url);if(u.protocol!=='https:'||u.username||u.password||u.search||u.hash)throw Error('仅支持无凭据和查询参数的 HTTPS 地址');billing=(await rl.question('计费说明（订阅 / API 钱包等）：')).trim();modelText=(await rl.question('Model ID（逗号分隔；回车尝试自动发现）：')).trim();}finally{rl.close();}
 const keyFile=path.join(dataDir,'keys','local-'+id+'.key');if(fs.existsSync(keyFile))throw Error('同名 Key 文件已存在；未覆盖');fs.mkdirSync(path.dirname(keyFile),{recursive:true,mode:0o700});
 const py="import getpass,os,sys\nif not sys.stdin.isatty(): sys.exit('请在本机交互终端运行')\nk=getpass.getpass('API Key（隐藏输入）：').strip()\nif not k or any(c.isspace() for c in k): sys.exit('无效 Key')\nfd=os.open(sys.argv[1],os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)\nwith os.fdopen(fd,'w') as f: f.write(k)";
 const result=spawnSync('python3',['-c',py,keyFile],{stdio:'inherit'});if(result.status)throw Error('Key 未保存');
 try{let models=modelText?modelText.split(',').map(x=>x.trim()).filter(Boolean):[];
 if(!models.length){const key=fs.readFileSync(keyFile,'utf8').trim();if(!/^[\x21-\x7e]+$/.test(key)||/["\\]/.test(key))throw Error('Key 格式不受支持');const quote=x=>String(x).replace(/\\/g,'\\\\').replace(/"/g,'\\"');const conf=`url = "${quote(url+'/models')}"\nheader = "Authorization: Bearer ${key}"\nproxy = "${quote(registry.proxy??'http://127.0.0.1:7892')}"\n`;const c=spawnSync('/usr/bin/curl',['--silent','--fail','--max-time','20','--proto','=https','--config','-'],{input:conf,encoding:'utf8',maxBuffer:4*1024*1024});if(c.status)throw Error('模型发现未成功，请重新添加并显式填写 Model ID');models=JSON.parse(c.stdout).data?.filter(m=>!m.output_modalities||m.output_modalities.includes('text')).map(m=>m.id)||[];}
 if(!models.length||models.some(m=>typeof m!=='string'||!m||/[\s?#]/.test(m)))throw Error('未取得有效 Model ID');
 registry.providers['local-'+id]={prefix:id,name,baseURL:url,billing,enabled:true,keyFile,models:[...new Set(models)].map(id=>({id,name:id,reasoning:false,input:['text'],contextWindow:128000,maxTokens:8192}))};catalog(registry);write(registryPath,registry);console.log('已添加；入口热加载并自动更新 pi / opencode2 模型目录。现有交互会话请重新打开模型选择器或重启会话。');
 }catch(e){fs.unlinkSync(keyFile);throw e;}
}

async function main(){const [command,id,...rest]=process.argv.slice(2);const registry=read(registryPath,{providers:{}});
 if(command==='sync'){sync();return;}
 if(command==='migrate'){ensureToken();syncAgents({firstMigration:true});return;}
 if(command==='status'){try{const r=await fetch(baseURL+'/models',{headers:{Authorization:'Bearer '+ensureToken()}});const j=await r.json();console.log(JSON.stringify({running:r.ok,baseURL,models:j.data?.length??0},null,2));}catch{console.log('本机统一入口未运行');process.exitCode=1;}return;}
 if(command==='finish'){const {hiddenComplete}=await import('./hub-admin.mjs');hiddenComplete(id);return;}
 if(command==='add'){await addProvider(registry);return;}

 if(command==='check'){const results=Object.entries(registry.providers).filter(([,p])=>p.enabled).map(([id,p])=>({id,keyPresent:fs.existsSync(p.keyFile),private:fs.existsSync(p.keyFile)&&(fs.statSync(p.keyFile).mode&0o077)===0,modelCount:p.models.length}));console.log(JSON.stringify(results,null,2));if(results.some(r=>!r.keyPresent||!r.private))process.exitCode=1;return;}
 if(command==='list'){console.log(JSON.stringify({providers:Object.entries(registry.providers).map(([id,p])=>({id,name:p.name,billing:p.billing,baseURL:p.baseURL,models:p.models.map(m=>m.id),keyPresent:fs.existsSync(p.keyFile)})),unavailable:registry.unavailable},null,2));return;}
 const p=registry.providers[id];if(command!=='run'&&!p)throw Error('未知 provider；先运行 list');
 if(command==='set-key'){const python="import getpass,os,sys,tempfile\nif not sys.stdin.isatty(): sys.exit('请在本机交互终端运行，禁止回显输入')\nk=getpass.getpass('API Key（隐藏输入）：').strip()\nif not k or any(c.isspace() for c in k): sys.exit('无效 Key，未保存')\np=sys.argv[1]\nfd,t=tempfile.mkstemp(dir=os.path.dirname(p))\nos.fchmod(fd,0o600)\nwith os.fdopen(fd,'w') as f: f.write(k)\nos.replace(t,p)\nprint('已保存到本机；未输出密钥。')";fs.mkdirSync(path.dirname(p.keyFile),{recursive:true,mode:0o700});const r=spawnSync('python3',['-c',python,p.keyFile],{stdio:'inherit'});if(r.status)process.exitCode=1;return;}
 if(command==='run'){const args=id==='--'?rest:[id,...rest];if(!args[0])throw Error('用法：run -- command args');const key=ensureToken();const env={...process.env,OPENAI_API_KEY:key,OPENAI_BASE_URL:baseURL,ANTHROPIC_BASE_URL:baseURL.replace(/\/v1$/,''),ANTHROPIC_API_KEY:key,ANTHROPIC_AUTH_TOKEN:key,OPENAI_MODEL:'glm/glm-5.3',MODEL_ACCESS_DEFAULT_MODEL:'glm/glm-5.3'};const child=spawn(args[0],args.slice(1),{stdio:'inherit',env});child.on('exit',code=>{process.exitCode=code??1;});child.on('error',()=>{console.error('无法启动命令');process.exitCode=1;});return;}

 throw Error('用法：status | add | finish task-id | list | check | sync | set-key provider | run -- command');
}
if(process.argv[1]&&fs.realpathSync(process.argv[1])===fileURLToPath(import.meta.url))main().catch(e=>{console.error(e.code==='ENOENT'?'配置或 Key 文件尚未建立':e.message);process.exitCode=1;});
