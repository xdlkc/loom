import {anthropicEndpoint,anthropicHeaders,validateAnthropic,anthropicError} from './anthropic.mjs';
import http from 'node:http';import fs from 'node:fs';import {spawn} from 'node:child_process';import {timingSafeEqual} from 'node:crypto';import {fileURLToPath} from 'node:url';import path from 'node:path';import {registryPath,catalog,read,ensureToken,syncAgents} from './hub-config.mjs';
const json=(res,status,data)=>{res.writeHead(status,{'Content-Type':'application/json; charset=utf-8','Cache-Control':'no-store'});res.end(JSON.stringify(data));};
const error=(res,status,code)=>json(res,status,res.anthropic?anthropicError(status,code):{error:{message:code,type:'local_hub_error',code}});
const quote=x=>String(x).replace(/\\/g,'\\\\').replace(/"/g,'\\"').replace(/\r/g,'\\r').replace(/\n/g,'\\n');
export function curlForward({url,key,body,proxy,signal,res,protocol='openai',headers:extraHeaders={}}){return new Promise(resolve=>{
 if(!/^[\x21-\x7e]+$/.test(key)||/["\\]/.test(key)){error(res,503,'PROVIDER_KEY_INVALID');resolve();return;}
 const child=spawn('/usr/bin/curl',['--silent','--no-buffer','--include','--suppress-connect-headers','--connect-timeout','15','--max-time','600','--proto','=https','--config','-'],{stdio:['pipe','pipe','pipe'],env:{PATH:process.env.PATH}});
 let pending=Buffer.alloc(0),headers=false,failed=false,bytes=0,settled=false;
 const finish=()=>{if(settled)return;settled=true;signal.removeEventListener('abort',abort);resolve();};
 const abort=()=>{child.kill('SIGTERM');finish();};signal.addEventListener('abort',abort,{once:true});if(signal.aborted){abort();return;}
 child.stderr.resume();child.stdin.on('error',()=>{});
 const auth=protocol==='anthropic'?`header = "x-api-key: ${key}"\n`:`header = "Authorization: Bearer ${key}"\n`;
 const extras=Object.entries(extraHeaders).map(([k,v])=>`header = "${quote(k)}: ${quote(v)}"\n`).join('');
 child.stdin.end(`url = "${quote(url)}"\nrequest = "POST"\n${auth}${extras}header = "Content-Type: application/json"\nheader = "Accept: ${body.stream?'text/event-stream':'application/json'}"\nproxy = "${quote(proxy||'')}"\ndata-binary = "${quote(JSON.stringify(body))}"\n`);
 function output(data){bytes+=data.length;if(bytes>64*1024*1024){failed=true;child.kill();if(!res.headersSent)error(res,502,'UPSTREAM_RESPONSE_TOO_LARGE');else res.destroy();return;}if(!res.write(data)){child.stdout.pause();res.once('drain',()=>child.stdout.resume());}}
 child.stdout.on('data',chunk=>{if(failed||res.destroyed)return;if(headers){output(chunk);return;}pending=Buffer.concat([pending,chunk]);if(pending.length>65536&&!pending.includes('\r\n\r\n')){failed=true;child.kill();error(res,502,'UPSTREAM_HEADERS_INVALID');return;}while(!headers){const end=pending.indexOf('\r\n\r\n');if(end<0)return;const raw=pending.subarray(0,end).toString('latin1');pending=pending.subarray(end+4);const status=Number(raw.match(/^HTTP\/\S+ (\d{3})/)?.[1]);if(status>=100&&status<200)continue;if(!Number.isInteger(status)||status<200||status>=300){failed=true;const safe=[400,401,403,404,408,413,429,500,502,503,504].includes(status)?status:502;error(res,safe,'UPSTREAM_'+(status||'INVALID'));child.kill();return;}headers=true;res.writeHead(status,{'Content-Type':body.stream?'text/event-stream; charset=utf-8':'application/json; charset=utf-8','Cache-Control':'no-store','X-Accel-Buffering':'no'});res.flushHeaders();if(pending.length)output(pending);pending=Buffer.alloc(0);}});
 child.on('error',()=>{if(!res.headersSent)error(res,502,'UPSTREAM_UNAVAILABLE');else res.destroy();finish();});
 child.on('close',code=>{if(!failed&&!res.destroyed){if(!headers)error(res,502,'UPSTREAM_UNAVAILABLE');else if(code!==0)res.destroy();else res.end();}finish();});
});}
export function createHub({getRegistry=()=>read(registryPath),getToken=ensureToken,forward=curlForward}={}){
 let active=0;const server=http.createServer(async(req,res)=>{
  res.setHeader('Cache-Control','no-store');res.setHeader('X-Content-Type-Options','nosniff');
  const port=server.address().port;const requestPath=new URL(req.url,'http://localhost').pathname;res.anthropic=requestPath==='/v1/messages'||typeof req.headers['x-api-key']==='string'||!!req.headers['anthropic-version'];
  if(![`127.0.0.1:${port}`,`localhost:${port}`].includes(req.headers.host)||req.headers.origin||req.headers['sec-fetch-site']==='cross-site')return error(res,403,'LOCAL_CLIENTS_ONLY');
  if(req.url==='/health'&&req.method==='GET')return json(res,200,{ok:true,service:'local-model-hub'});
  const expected=getToken(),equal=v=>{const a=Buffer.from(v||''),b=Buffer.from(expected);return a.length===b.length&&timingSafeEqual(a,b);};
  const bearer=typeof req.headers.authorization==='string'&&req.headers.authorization.startsWith('Bearer ')?req.headers.authorization.slice(7):null,apiKey=res.anthropic&&typeof req.headers['x-api-key']==='string'?req.headers['x-api-key']:null;
  if(req.headers.authorization!==undefined&&!bearer)return error(res,401,'LOCAL_KEY_REQUIRED');
  if(!(bearer&&equal(bearer)||apiKey&&equal(apiKey))||bearer&&!equal(bearer)||apiKey&&!equal(apiKey))return error(res,401,'LOCAL_KEY_REQUIRED');
  let models;try{models=catalog(getRegistry());}catch{return error(res,503,'REGISTRY_INVALID');}
  if(requestPath==='/v1/models'&&req.method==='GET')return json(res,200,res.anthropic?{data:models.filter(({p})=>anthropicEndpoint(p)).map(({id,m})=>({id,type:'model',display_name:m.name,created_at:'1970-01-01T00:00:00Z'})),has_more:false,first_id:null,last_id:null}:{object:'list',data:models.map(({id,p,m})=>({id,object:'model',owned_by:p.name,name:p.name+' · '+m.name,billing:p.billing,context_window:m.contextWindow,max_output_tokens:m.maxTokens}))});
  const messagesAPI=requestPath==='/v1/messages';
  if(!['/v1/chat/completions','/v1/messages'].includes(requestPath)||req.method!=='POST')return error(res,404,'ENDPOINT_NOT_SUPPORTED');
  if(active>=8)return error(res,429,'LOCAL_CONCURRENCY_LIMIT');
  if(!req.headers['content-type']?.startsWith('application/json'))return error(res,415,'JSON_REQUIRED');
  active++;const controller=new AbortController();res.once('close',()=>controller.abort());req.once('aborted',()=>controller.abort());
  try{let size=0;const chunks=[];for await(const c of req){size+=c.length;if(size>20*1024*1024){error(res,413,'REQUEST_TOO_LARGE');return;}chunks.push(c);}let body;try{body=JSON.parse(Buffer.concat(chunks).toString());}catch{return error(res,400,'INVALID_JSON');}const route=models.find(x=>x.id===body.model);if(!route)return error(res,400,'UNKNOWN_MODEL');if(!Array.isArray(body.messages)||!body.messages.length||body.stream!==undefined&&typeof body.stream!=='boolean')return error(res,400,'INVALID_CHAT_REQUEST');
   let endpoint,headers={};if(messagesAPI){try{validateAnthropic(body);headers=anthropicHeaders(req.headers);endpoint=anthropicEndpoint(route.p);}catch(e){return error(res,400,e.message);}if(!endpoint)return error(res,400,'ANTHROPIC_NOT_SUPPORTED_BY_PROVIDER');}
   const key=fs.readFileSync(route.p.keyFile,'utf8').trim();const upstream={...body,model:route.m.id};delete upstream.store;
   const max=route.m.maxTokens||8192;const requested=messagesAPI?upstream.max_tokens:upstream.max_completion_tokens??upstream.max_tokens;
   if(requested!==undefined&&(!Number.isInteger(requested)||requested<1))return error(res,400,'INVALID_MAX_TOKENS');
   upstream.max_tokens=Math.min(requested||max,max);delete upstream.max_completion_tokens;
   // Never retry or switch providers automatically: each request retains its chosen billing pool.
   await forward({url:messagesAPI?endpoint:route.p.baseURL.replace(/\/$/,'')+'/chat/completions',key,body:upstream,proxy:getRegistry().proxy??'http://127.0.0.1:7892',signal:controller.signal,res,protocol:messagesAPI?'anthropic':'openai',headers});
  }catch{if(!res.headersSent)error(res,502,'REQUEST_FAILED');else res.destroy();}finally{active--;}
 });server.requestTimeout=30000;server.headersTimeout=10000;server.timeout=660000;return server;
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){
 ensureToken();let timer;const reload=()=>{clearTimeout(timer);timer=setTimeout(()=>{try{if(syncAgents({quiet:true})){const child=spawn(path.join(process.env.HOME||'', '.local/bin/opencode2'),['reload'],{stdio:'ignore'});child.on('error',()=>{});}}catch{console.error('模型目录同步失败，现有客户端配置保留。');}},300);};
 fs.watchFile(registryPath,{interval:1000},reload);reload();const server=createHub();server.on('error',()=>{console.error('本机统一入口启动失败，请检查端口。');process.exit(1);});server.listen(4319,'127.0.0.1',()=>console.log('本机统一模型入口：http://127.0.0.1:4319/v1'));
 const stop=()=>{fs.unwatchFile(registryPath);clearTimeout(timer);server.close();setTimeout(()=>process.exit(0),3000).unref();};process.on('SIGTERM',stop);process.on('SIGINT',stop);
}
