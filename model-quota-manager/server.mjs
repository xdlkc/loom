import {handleTokenBridge} from './token-bridge.mjs';
import {handleMiniMaxBridge} from './minimax-bridge.mjs';
import {queryConsumption} from './consumption.mjs';
import {appendHistory} from './history.mjs';
import {handleGeminiBridge} from './gemini-bridge.mjs';
import {hubView,startPending,launchPending,cancelPending,mutateHub} from '../model-access/hub-admin.mjs';
import {grokExtensionId} from './grok-extension-id.mjs';
import {normalizeGrokSnapshot} from './grok-browser.mjs';
import {spawn} from 'node:child_process';
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {randomBytes,randomUUID,timingSafeEqual} from 'node:crypto';
import {providers,connectors,kinds} from './catalog.mjs';
import {load,save} from './storage.mjs';
import {query,safeError} from './adapters.mjs';
const root=path.dirname(fileURLToPath(import.meta.url));
export function validateManual(body,credentials={}){
 const str=(v,max=120)=>{if(typeof v!=='string'||v.length>max)throw new Error('字段过长或格式错误');if(/(?:sk-|xai-)[a-zA-Z0-9_-]{12,}|Bearer\s+\S{12,}/i.test(v)||Object.values(credentials).some(c=>c.key&&v.includes(c.key)))throw new Error('请勿在记录中填写密钥');return v.trim();};
 if(!providers.some(p=>p.id===body.provider)||!Object.hasOwn(kinds,body.kind))throw new Error('请选择有效服务与数据类型');
 const number=(v,name)=>{if(v===''||v===null||v===undefined)return null;const n=Number(v);if(!Number.isFinite(n)||Math.abs(n)>1e15)throw new Error(name+'必须为有效数字');return n;};
 const date=v=>{if(!v)return null;if(typeof v!=='string'||!Number.isFinite(Date.parse(v)))throw new Error('时间格式错误');return new Date(v).toISOString();};
 const value=number(body.value,'数值'),total=number(body.total,'总量'),used=number(body.used,'已用量');
 if(total!==null&&total<0||used!==null&&used<0)throw new Error('总量与已用量不能为负');
 if(body.kind==='subscription'&&((value!==null&&value<0)||(total!==null&&value!==null&&value>total)||(total!==null&&used!==null&&used>total)))throw new Error('订阅剩余或已用量应位于 0 与总量之间');
 const unit=str(body.unit,30),plan=str(body.plan),label=str(body.label,80);if(!plan||!label||!unit)throw new Error('请填写名称、指标和单位');
 if(body.kind==='subscription'&&value!==null&&used!==null&&total!==null&&Math.abs(value+used-total)>Math.max(1e-8,total*1e-8))throw new Error('订阅剩余加已用应等于窗口总量');
 const observedAt=date(body.observedAt)||new Date().toISOString();if(Date.parse(observedAt)>Date.now()+60000)throw new Error('记录时间不能晚于当前时间');
 return {id:typeof body.id==='string'?str(body.id,50):randomUUID(),provider:body.provider,kind:body.kind,source:'manual',plan,note:str(body.note||'',500),observedAt,expiresAt:date(body.expiresAt),metrics:[{label,value,unit,total,used,resetAt:date(body.resetAt)}]};
}
export function createApp({queryFn=query,terminalLauncher=launchPending,consumptionFn=queryConsumption,startScheduler=false}={}){
 let state=load('state',{manual:[],observations:{},attempts:{},history:[],settings:{autoRefresh:true,proxy:'http://127.0.0.1:7892'}});
 state.settings={autoRefresh:true,proxy:'http://127.0.0.1:7892',...state.settings};
 const csrf=randomBytes(32).toString('hex'),active=new Map(),grokBridgeStatus={},geminiBridgeStatus={};
 const creds=()=>load('credentials',{});const consumptionCache=new Map(),consumptionActive=new Map();
 function view(){const c=creds();return {providers,connectors,kinds,manual:state.manual,observations:state.observations,attempts:state.attempts,history:state.history.slice(-300),settings:{...state.settings,geminiBrowserFingerprint:undefined},configured:Object.fromEntries(Object.entries(connectors).map(([id,x])=>[id,x.key==='geminiBrowser'?!!state.settings.geminiBrowser:x.key==='codexLocal'?!!state.settings.codexLocal:x.key==='grokBrowser'?!!state.settings.grokBrowser:!!c[x.key]?.key&&(x.key!=='xai'||!!c.xai.teamId)&&(x.key!=='gemini'||!!c.gemini.projectId)&&(x.key!=='kimiCode'||Number.isInteger(c.kimiCode.port))])),grokBridgeStatus,geminiBridgeStatus,backgroundSync:syncStatus,checkedAt:'2026-10-24',serverTime:new Date().toISOString()};}
 async function refresh(id){
  if(!Object.hasOwn(connectors,id))throw new Error('未知查询项');if(active.has(id))return active.get(id);
  if(state.attempts[id]?.at&&Date.now()-Date.parse(state.attempts[id].at)<30000)return;
  if(connectors[id].key==='geminiBrowser'){if(state.settings.geminiBrowser){state.settings.geminiBrowserRequestedAt=Date.now();state.attempts[id]={at:new Date().toISOString(),status:'pending',message:'等待 Chrome 读取已绑定 Gemini 账号（通常一至两分钟）'};save('state',state);}return;}
  if(connectors[id].key==='grokBrowser'){if(state.settings.grokBrowser){state.settings.grokBrowserRequestedAt=Date.now();state.attempts[id]={at:new Date().toISOString(),status:'pending',message:'等待 Chrome 网页读取（通常一分钟内）'};save('state',state);}return;}
  const local=connectors[id].key==='codexLocal';const c=local?{}:creds()[connectors[id].key];if(local?!state.settings.codexLocal:!c?.key)return;
  const task=(async()=>{const at=new Date().toISOString();try{const result=await queryFn(id,c,state.settings.proxy);if(c.key&&JSON.stringify(result).includes(c.key))throw new Error('SCHEMA');const observedAt=new Date().toISOString();state.observations[id]={...result,id,provider:connectors[id].provider,source:'official',observedAt};state.attempts[id]={at,status:'ok'};appendHistory(state,state.observations[id]);}catch(e){state.attempts[id]={at,status:'error',message:safeError(e)};}save('state',state);})();active.set(id,task);try{await task;}finally{active.delete(id);}
 }
 async function all(){await Promise.allSettled(Object.keys(connectors).map(refresh));}
 const syncStatus={running:false,lastStartedAt:null,lastCompletedAt:null,nextAt:null};
 async function consumption(provider,range){
  const key=provider+'|'+range,previous=consumptionCache.get(key);if(previous&&Date.now()-previous.savedAt<300000)return previous.data;
  if(!consumptionActive.has(key))consumptionActive.set(key,(async()=>{try{const credentials=creds(),data=await consumptionFn(provider,range,{credentials,settings:state.settings});if(Object.values(credentials).some(c=>c.key&&JSON.stringify(data).includes(c.key)))throw Error('SCHEMA');consumptionCache.set(key,{savedAt:Date.now(),data});return data;}catch(e){if(previous)return {...previous.data,warning:'更新失败，显示上次成功获取的消耗统计。'};throw e;}finally{consumptionActive.delete(key);}})());return consumptionActive.get(key);
 }
 async function scheduledSync(){
  if(!state.settings.autoRefresh||syncStatus.running)return;syncStatus.running=true;syncStatus.lastStartedAt=new Date().toISOString();
  try{for(const p of ['grok','deepseek']){const status=state.settings.tokenBrowser?.[p];if(status?.enabled&&!status.requestedAt)status.requestedAt=Date.now();}if(state.settings.minimaxBrowser&&!state.settings.minimaxBrowserRequestedAt)state.settings.minimaxBrowserRequestedAt=Date.now();save('state',state);
   const c=creds(),sources=[...(c.kimiCode?.key?['kimi']:[]),...(c.zenmux?.key?['zenmux']:[]),...(c.glm?.key?['glm']:[]),...(state.settings.codexLocal?['chatgpt']:[])];
   await Promise.allSettled([all(),...sources.flatMap(p=>(p==='kimi'?['7d']:['today','7d','30d']).map(r=>consumption(p,r)))]);
  }finally{syncStatus.running=false;syncStatus.lastCompletedAt=new Date().toISOString();syncStatus.nextAt=new Date(Date.now()+15*60000).toISOString();}
 }
 const interval=setInterval(()=>void scheduledSync(),15*60*1000);interval.unref();
 const json=(res,status,data)=>{res.writeHead(status,{'Content-Type':'application/json; charset=utf-8'});res.end(JSON.stringify(data));};
 const server=http.createServer(async(req,res)=>{
  res.setHeader('Cache-Control','no-store');res.setHeader('X-Content-Type-Options','nosniff');res.setHeader('Referrer-Policy','no-referrer');res.setHeader('Content-Security-Policy',"default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'");
  const host=req.headers.host,port=server.address()?.port;const origins=[`http://127.0.0.1:${port}`,`http://localhost:${port}`,`http://quota.localhost:${port}`];
  if(req.url?.startsWith('/api/token-browser/')){await handleTokenBridge(req,res,{state,port,loadSnapshot:p=>load(p+'-consumption',null),saveSnapshot:(p,d)=>save(p+'-consumption',d),saveState:()=>save('state',state),invalidate:p=>{for(const k of consumptionCache.keys())if(k.startsWith(p+'|'))consumptionCache.delete(k);}});return;}
  if(req.url?.startsWith('/api/minimax-browser/')){await handleMiniMaxBridge(req,res,{state,port,loadSnapshot:()=>load('minimax-consumption',null),saveSnapshot:d=>save('minimax-consumption',d),saveState:()=>save('state',state),invalidate:()=>{for(const k of consumptionCache.keys())if(k.startsWith('minimax|'))consumptionCache.delete(k);}});return;}
  if(req.url?.startsWith('/api/gemini-browser/')){await handleGeminiBridge(req,res,{state,port,diagnostics:geminiBridgeStatus,saveState:()=>save('state',state)});return;}
  if(req.url?.startsWith('/api/grok-browser/')){
   grokBridgeStatus.lastRequestAt=new Date().toISOString();grokBridgeStatus.method=req.method;grokBridgeStatus.route=['control','connect','snapshot','failure'].includes(req.url.split('/').at(-1))?req.url.split('/').at(-1):'other';grokBridgeStatus.originAccepted=req.headers.origin===`chrome-extension://${grokExtensionId}`;
   // Legacy privileged extension GETs omit Origin. Expose only task flags;
   // snapshots, failures and connection mutations still require the fixed Origin.
   if(req.method==='GET'&&req.url==='/api/grok-browser/control'&&!req.headers.origin&&req.headers['sec-fetch-site']!=='cross-site'&&[`127.0.0.1:${port}`,`localhost:${port}`].includes(host))return json(res,200,{enabled:!!state.settings.grokBrowser,requested:!!state.settings.grokBrowserRequestedAt});
   if(![`127.0.0.1:${port}`,`localhost:${port}`].includes(host)||req.headers.origin!==`chrome-extension://${grokExtensionId}`)return json(res,403,{error:'只允许指定本机同步扩展'});
   res.setHeader('Access-Control-Allow-Origin',`chrome-extension://${grokExtensionId}`);res.setHeader('Vary','Origin');
   if(req.method==='OPTIONS'){res.setHeader('Access-Control-Allow-Methods','GET, POST, OPTIONS');res.setHeader('Access-Control-Allow-Headers','Content-Type, X-Grok-Bridge');res.writeHead(204);res.end();return;}
   const route=req.url.slice('/api/grok-browser/'.length);
   if(req.method==='GET'&&route==='control')return json(res,200,{enabled:!!state.settings.grokBrowser,requested:!!state.settings.grokBrowserRequestedAt});
   if(req.method!=='POST'||req.headers['x-grok-bridge']!=='1'||!req.headers['content-type']?.startsWith('application/json'))return json(res,403,{error:'无效同步请求'});
   try{let raw='',size=0;for await(const chunk of req){size+=chunk.length;if(size>8192)throw Error();raw+=chunk;}const body=JSON.parse(raw||'{}');
    if(route==='control')return json(res,200,{enabled:!!state.settings.grokBrowser,requested:!!state.settings.grokBrowserRequestedAt});
    if(route==='connect'){state.settings.grokBrowser=true;state.settings.grokBrowserRequestedAt=Date.now();save('state',state);return json(res,200,{ok:true});}
    if(!state.settings.grokBrowser)return json(res,403,{error:'尚未连接扩展'});
    if(route==='snapshot'){
     const records=normalizeGrokSnapshot(body);for(const [id,r] of [['grok-web-subscription',records.subscription],['grok-web-usage',records.usage]]){state.observations[id]={...r,id,provider:'grok'};state.attempts[id]={at:r.observedAt,status:'ok'};appendHistory(state,state.observations[id]);}state.settings.grokBrowserRequestedAt=0;save('state',state);return json(res,200,{ok:true});
    }
    if(route==='failure'){if(!['LOGIN_OR_PAGE','PAGE','NETWORK'].includes(body.reason))throw Error();for(const id of ['grok-web-subscription','grok-web-usage'])state.attempts[id]={at:new Date().toISOString(),status:'error',message:'浏览器未能读取官方用量页。请确认 Chrome 运行且已登录 Grok；页面结构变化时保留上次成功数据。'};state.settings.grokBrowserRequestedAt=0;save('state',state);return json(res,200,{ok:true});}
    return json(res,404,{error:'未找到'});
   }catch{return json(res,400,{error:'网页数据未通过核验，保留上次结果'});}
  }
  if(![`127.0.0.1:${port}`,`localhost:${port}`,`quota.localhost:${port}`].includes(host)||req.headers['sec-fetch-site']==='cross-site')return json(res,403,{error:'仅允许本机同源访问'});
  if(req.headers.origin&&!origins.includes(req.headers.origin))return json(res,403,{error:'不允许跨站访问'});
  try{
   const url=new URL(req.url,'http://127.0.0.1');
   if(req.method==='GET'){
    if(url.pathname==='/api/hub')return json(res,200,await hubView());
    if(url.pathname==='/api/consumption'){const p=url.searchParams.get('provider'),r=url.searchParams.get('range')||'7d';if(!providers.some(x=>x.id===p)||!['today','7d','30d'].includes(r))return json(res,400,{error:'服务或时间范围无效'});try{return json(res,200,await consumption(p,r));}catch(e){return json(res,502,{error:safeError(e)});}}
    if(url.pathname==='/api/history'){const provider=url.searchParams.get('provider');if(!providers.some(p=>p.id===provider))return json(res,400,{error:'请选择有效服务'});return json(res,200,{history:state.history.filter(r=>connectors[r.id]?.provider===provider)});}
    if(url.pathname==='/api/state')return json(res,200,view());
    if(url.pathname==='/api/session')return json(res,200,{csrf});
    if(url.pathname==='/api/export')return json(res,200,{exportedAt:new Date().toISOString(),...view(),history:state.history});
    const files={'/':'index.html','/app.js':'app.js','/style.css':'style.css','/hub-ui.js':'hub-ui.js','/trends.js':'trends.js','/trend-data.mjs':'trend-data.mjs','/consumption.js':'consumption.js',...Object.fromEntries(['gemini','deepseek','grid','models','settings'].map(id=>['/icons/'+id+'.svg','icons/'+id+'.svg']))};const f=files[url.pathname];if(!f)return json(res,404,{error:'未找到'});
    res.writeHead(200,{'Content-Type':f.endsWith('html')?'text/html; charset=utf-8':f.endsWith('js')?'text/javascript; charset=utf-8':f.endsWith('svg')?'image/svg+xml':'text/css; charset=utf-8'});res.end(fs.readFileSync(path.join(root,'public',f)));return;
   }
   if(req.method!=='POST')return json(res,405,{error:'不支持的方法'});
   const token=Buffer.from(req.headers['x-csrf-token']||'');if(!origins.includes(req.headers.origin)||token.length!==csrf.length||!timingSafeEqual(token,Buffer.from(csrf)))return json(res,403,{error:'请刷新页面后重试'});
   if(!req.headers['content-type']?.startsWith('application/json'))return json(res,415,{error:'需要 JSON'});
   let raw='',bytes=0;for await(const chunk of req){bytes+=chunk.length;if(bytes>16384)return json(res,413,{error:'请求过大'});raw+=chunk;}
   let body;try{body=JSON.parse(raw||'{}');}catch{return json(res,400,{error:'JSON 格式错误'});}
   if(url.pathname==='/api/token/request'){const p=url.searchParams.get('provider');if(!['deepseek','grok'].includes(p)||!state.settings.tokenBrowser?.[p]?.enabled)return json(res,409,{error:'请先在统一扩展连接该服务的 Token 同步'});state.settings.tokenBrowser[p].requestedAt=Date.now();save('state',state);return json(res,200,{ok:true});}
   if(url.pathname==='/api/minimax/request'){if(!state.settings.minimaxBrowser)return json(res,409,{error:'请先绑定MiniMax用量页'});state.settings.minimaxBrowserRequestedAt=Date.now();save('state',state);return json(res,200,{ok:true});}
   if(url.pathname==='/api/codex/connect'){state.settings.codexLocal=true;delete state.attempts['codex-quota'];save('state',state);await refresh('codex-quota');return json(res,200,view());}
   if(url.pathname.startsWith('/api/hub/')){try{const action=url.pathname.slice('/api/hub/'.length);if(action==='add'||action==='rotate-key'){const draft=startPending(body,action==='rotate-key');const terminalOpened=await terminalLauncher(draft.id);return json(res,200,{...draft,terminalOpened});}if(action==='open-terminal')return json(res,200,{terminalOpened:await terminalLauncher(body.id)});if(action==='cancel'){cancelPending(body.id);return json(res,200,{ok:true});}return json(res,200,mutateHub(action,body));}catch(e){const message=typeof e.message==='string'&&/[一-鿿]/.test(e.message)?e.message:'本机模型配置操作失败，原配置已保留';return json(res,400,{error:message});}}
   if(url.pathname==='/api/manual'){
    const record=validateManual(body,creds());const index=state.manual.findIndex(x=>x.id===record.id);if(index>=0)state.manual[index]=record;else state.manual.push(record);save('state',state);return json(res,200,{ok:true});
   }
   if(url.pathname==='/api/manual/delete'){if(typeof body.id!=='string')throw new Error('缺少记录 ID');state.manual=state.manual.filter(x=>x.id!==body.id);save('state',state);return json(res,200,{ok:true});}
   if(url.pathname==='/api/refresh'){if(body.id)await refresh(body.id);else await all();return json(res,200,view());}
   if(url.pathname==='/api/settings'){
    if(typeof body.autoRefresh!=='boolean')throw new Error('刷新设置错误');const proxy=body.proxy;if(typeof proxy!=='string'||(proxy!==''&&!/^http:\/\/(127\.0\.0\.1|localhost):\d{1,5}$/.test(proxy)))throw new Error('代理仅支持本机 HTTP 地址，留空可直连');
    state.settings={...state.settings,autoRefresh:body.autoRefresh,proxy};save('state',state);return json(res,200,{ok:true});
   }
   return json(res,404,{error:'未找到'});
  }catch(e){const allowed=['字段过长或格式错误','请勿在记录中填写密钥','请选择有效服务与数据类型','总量与已用量不能为负','订阅剩余或已用量应位于 0 与总量之间','订阅剩余加已用应等于窗口总量','请填写名称、指标和单位','时间格式错误','记录时间不能晚于当前时间','未知查询项','刷新设置错误','代理仅支持本机 HTTP 地址，留空可直连','缺少记录 ID'];const message=allowed.includes(e.message)||/^(数值|总量|已用量)必须为有效数字$/.test(e.message)?e.message:'本地操作失败，请检查数据文件权限';json(res,400,{error:message});}
 });
 server.on('listening',()=>{if(startScheduler)void scheduledSync();});server.on('close',()=>clearInterval(interval));server.requestTimeout=30000;server.headersTimeout=10000;
 return {server,refresh,view};
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){
 const port=Number(process.env.PORT||4318);if(!Number.isInteger(port)||port<1024||port>65535){console.error('PORT 必须为 1024–65535');process.exit(1);}
 try{if(load('credentials',{}).kimiCode?.key){const service=spawn(process.execPath,[path.join(root,'kimi-service.mjs')],{cwd:root,detached:true,stdio:'ignore'});service.unref();}const {server}=createApp({startScheduler:true});server.on('error',()=>{console.error('服务启动失败：端口可能被占用');process.exitCode=1;});server.listen(port,'127.0.0.1',()=>console.log(`Model Quota 本机服务：http://127.0.0.1:${port}`));}catch{console.error('本地数据读取失败，请检查 .local 文件');process.exitCode=1;}
}
