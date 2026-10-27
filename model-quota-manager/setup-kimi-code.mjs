import {spawn} from 'node:child_process';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import fs from 'node:fs';
import {load,dataDir} from './storage.mjs';
const root=path.dirname(fileURLToPath(import.meta.url)),cli=path.join(dataDir,'kimi-runtime/node_modules/@moonshot-ai/kimi-code/dist/main.mjs');
const wait=ms=>new Promise(r=>setTimeout(r,ms));
try{
 if(!fs.existsSync(cli)){console.log('正在安装官方 Kimi Code CLI 到工具的本机目录…');const code=await new Promise(resolve=>{const child=spawn('npm',['install','--prefix',path.join(dataDir,'kimi-runtime'),'--no-audit','--no-fund','--save-exact','@moonshot-ai/kimi-code@2.3.1'],{stdio:'ignore'});child.on('error',()=>resolve(1));child.on('exit',resolve);});if(code!==0)throw new Error();}
 let c=load('credentials',{}).kimiCode,alive=false;
 if(c?.key&&c?.port){try{alive=(await fetch(`http://127.0.0.1:${c.port}/api/v1/healthz`,{signal:AbortSignal.timeout(2000)})).ok;}catch{}}
 if(!alive){const child=spawn(process.execPath,[path.join(root,'kimi-service.mjs')],{cwd:root,detached:true,stdio:'ignore'});child.unref();for(let i=0;i<30;i++){await wait(1000);c=load('credentials',{}).kimiCode;if(c?.key&&c?.port){try{if((await fetch(`http://127.0.0.1:${c.port}/api/v1/healthz`,{signal:AbortSignal.timeout(1000)})).ok){alive=true;break;}}catch{}}}if(!alive)throw new Error();}
 const base='http://127.0.0.1:'+c.port,headers={Authorization:'Bearer '+c.key,'Content-Type':'application/json'};
 const d=await(await fetch(base+'/api/v1/oauth/login',{method:'POST',headers,body:JSON.stringify({region:'mainland-cn'})})).json();if(d.code!==0)throw new Error();
 if(d.data?.status!=='authenticated'){
  if(!/^https:\/\/(www\.)?kimi\.(com|ai)\//.test(d.data?.verification_uri_complete||''))throw new Error();
  spawn('/usr/bin/open',[d.data.verification_uri_complete],{stdio:'ignore'}).unref();console.log('已打开官方 Kimi 授权页面。请自行登录和确认；无需粘贴 API Key。');
  const until=Date.now()+Math.min(d.data.expires_in||300,900)*1000;let success=false;
  while(Date.now()<until){await wait(5000);const flow=await(await fetch(base+'/api/v1/oauth/login',{headers})).json();if(flow.data?.status==='authenticated'){success=true;break;}if(['expired','denied','cancelled'].includes(flow.data?.status))break;}
  if(!success){console.error('授权未完成或已过期，请重新运行此接入脚本。');process.exit(1);}
 }
 console.log('Kimi Code 官方登录已完成，连接令牌仅存本机。');
 const child=spawn(process.execPath,[path.join(root,'verify-kimi-code.mjs')],{cwd:root,stdio:'inherit'});child.on('exit',code=>process.exitCode=code||0);
}catch{console.error('Kimi Code 接入未完成，请检查网络代理与官方服务状态。未输出任何认证。');process.exitCode=1;}
