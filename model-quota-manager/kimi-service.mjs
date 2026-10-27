// Suppress the official startup banner: it includes a local bearer credential.
import {spawn} from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {load,save,dataDir} from './storage.mjs';
const root=path.dirname(fileURLToPath(import.meta.url));
const cli=path.join(dataDir,'kimi-runtime/node_modules/@moonshot-ai/kimi-code/dist/main.mjs');
if(!fs.existsSync(cli)){console.error('Kimi Code 官方运行时尚未安装。');process.exit(1);}
const existing=load('credentials',{}).kimiCode;
if(existing?.port&&existing?.key){try{const r=await fetch(`http://127.0.0.1:${existing.port}/api/v1/healthz`,{signal:AbortSignal.timeout(2000)});if(r.ok){console.log('Kimi Code 本地服务已在运行。');process.exit(0);}}catch{}}
const proxy=load('state',{}).settings?.proxy||'';
const child=spawn(process.execPath,[cli,'web','--port','58627','--no-open','--log-level','silent'],{cwd:root,env:{...process.env,NODE_USE_ENV_PROXY:'1',HTTP_PROXY:proxy,HTTPS_PROXY:proxy,NO_PROXY:'127.0.0.1,localhost'},stdio:['ignore','pipe','pipe']});
let buffer='',ready=false;
const onData=b=>{if(ready)return;buffer+=b.toString().replace(/\x1b\[[0-9;]*m/g,'');if(buffer.length>65536)buffer=buffer.slice(-32768);
 const local=buffer.match(/http:\/\/127\.0\.0\.1:(\d+)\/[^\s]*#token=([^\s]+)[\r\n]/),token=buffer.match(/Token:\s*([^\s]+)[\r\n]/),port=buffer.match(/http:\/\/127\.0\.0\.1:(\d+)/);
 const key=local?.[2]||token?.[1],n=Number(local?.[1]||port?.[1]);if(!key||!n)return;
 const credentials=load('credentials',{});credentials.kimiCode={key,port:n};save('credentials',credentials);ready=true;buffer='';save('kimi-service',{pid:child.pid,port:n,startedAt:new Date().toISOString()});console.log('Kimi Code 官方本地服务就绪；连接令牌已写入 0600 文件，未显示或记录。');
};
child.stdout.on('data',onData);child.stderr.on('data',onData);
const timer=setTimeout(()=>{if(!ready){console.error('Kimi Code 本地服务未就绪（启动输出已隐藏，避免泄漏令牌）。');child.kill();}},30000);timer.unref();
child.on('exit',code=>{clearTimeout(timer);if(!ready)console.error('Kimi Code 启动未完成；退出码 '+code);process.exitCode=code||0;});
for(const signal of ['SIGINT','SIGTERM'])process.on(signal,()=>child.kill(signal));
