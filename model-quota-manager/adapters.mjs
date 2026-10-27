import {readCodexLimits,normalizeCodexQuota} from './codex-local.mjs';
import {requestKimiLocal,normalizeKimiQuota,normalizeKimiExtra} from './kimi-local.mjs';
import {spawn} from 'node:child_process';
const num=v=>v!==null&&v!==undefined&&v!==''&&Number.isFinite(Number(v))?Number(v):null;
const need=v=>{const n=num(v);if(n===null)throw new Error('SCHEMA');return n;};
const iso=v=>typeof v==='string'&&Number.isFinite(Date.parse(v))?new Date(v).toISOString():null;
const metric=(label,value,unit,extra={})=>({label,value:need(value),unit,...extra});
const text=v=>typeof v==='string'&&/^[a-zA-Z0-9_ ./-]{1,64}$/.test(v)?v:'未提供';
export function request(url,key,proxy='http://127.0.0.1:7892',body,auth='bearer'){
 return new Promise((resolve,reject)=>{
  if(!/^[\x21-\x7e]+$/.test(key)||/["\\]/.test(key))return reject(new Error('CREDENTIAL'));
  const args=['--silent','--max-time','25','--connect-timeout','8','--max-filesize','2097152','--proto','=https','--config','-','--write-out','\n%{http_code}',url];
  if(body)args.push('--request','POST','--data-binary',JSON.stringify(body));
  const child=spawn('/usr/bin/curl',args,{stdio:['pipe','pipe','pipe'],env:{PATH:process.env.PATH}});let out='',size=0;
  child.stdout.on('data',b=>{size+=b.length;if(size>2097152)child.kill();else out+=b;});child.stderr.resume();
  child.on('error',()=>reject(new Error('NETWORK')));
  child.on('close',code=>{if(code!==0)return reject(new Error('NETWORK'));const pos=out.lastIndexOf('\n'),status=Number(out.slice(pos+1));if(status<200||status>=300)return reject(new Error('HTTP_'+status));try{const d=JSON.parse(out.slice(0,pos));if(d.error||d.success===false||d.status===false|| (d.base_resp?.status_code!=null&&d.base_resp.status_code!==0))throw 0;resolve(d);}catch{reject(new Error('SCHEMA'));}});
  const quoted=s=>String(s).replace(/\\/g,'\\\\').replace(/"/g,'\\"');
  child.stdin.on('error',()=>{});child.stdin.end(`header = "Authorization: ${auth==='raw'?'':'Bearer '}${key}"\nheader = "Content-Type: application/json"\nproxy = "${quoted(proxy)}"\n`);
 });
}
export async function query(id,c,proxy,get=request,now=new Date()){
 if(id==='codex-quota'){const r=await readCodexLimits();return normalizeCodexQuota(r.data,r.planType);}
 const call=(url,body)=>get(url,c.key,proxy,body,id==='glm-quota'?'raw':'bearer');
 if(id==='kimi-code-quota'||id==='kimi-code-extra'){
  const local=(endpoint)=>get===request?requestKimiLocal(endpoint,c):get(endpoint,c.key,'');
  const d=await local('/api/v1/oauth/usage');if(d.code!==0||d.data?.kind!=='ok')throw new Error(d.data?.status===401?'KIMI_LOGIN':'SCHEMA');
  if(id==='kimi-code-extra')return normalizeKimiExtra(d);
  let plan='Kimi Code / 会员';try{const p=await local('/api/v1/oauth/userinfo'),name=p.data?.userInfo?.userLevelName;if(p.code===0&&p.data?.kind==='ok'&&typeof name==='string'&&name.length>0&&name.length<64&&!/[\x00-\x1f]/.test(name))plan='Kimi Code / 会员 · '+name;}catch{}
  return normalizeKimiQuota(d,plan);
 }
 if(id==='glm-quota'){
  const d=await call('https://open.bigmodel.cn/api/monitor/usage/quota/limit');if(d.success===false||d.code!==undefined&&![0,200].includes(d.code)||!Array.isArray(d.data?.limits))throw new Error('SCHEMA');
  const metrics=[];for(const q of d.data.limits){if(!['TOKENS_LIMIT','CREDIT_LIMIT','TIME_LIMIT'].includes(q.type))continue;const used=num(q.percentage);if(used===null||used<0||used>100)continue;
   const legacy=q.type==='TOKENS_LIMIT'&&q.unit===undefined&&q.number===undefined;
   const window=legacy||q.unit===3&&q.number===5?'5h':q.unit===6&&q.number===1?'7d':null;
   const label=q.type==='TIME_LIMIT'?'MCP 配额剩余':window==='5h'?'5 小时剩余':window==='7d'?'7 天剩余':`额度剩余（未知周期 ${text(String(q.unit))}/${text(String(q.number))}）`;
   const ms=num(q.nextResetTime),resetAt=ms!==null&&ms>=1e12&&ms<8640000000000000?new Date(ms).toISOString():null;
   metrics.push(metric(label,100-used,'%',{total:100,used,resetAt,...(q.type!=='TIME_LIMIT'?{window,windowUnit:q.unit??null,windowNumber:q.number??null}:{}),...(window==='5h'&&q.type!=='TIME_LIMIT'?{resetLabel:'下次额度恢复',resetDescription:'按请求消耗时间，满 5 小时后动态恢复'}:{})}));
  }if(!metrics.length)throw new Error('SCHEMA');metrics.sort((a,b)=>({ '5h':0,'7d':1 }[a.window]??2)-({ '5h':0,'7d':1 }[b.window]??2));
  return {kind:'subscription',plan:'GLM Coding Plan'+(d.data.level?' · '+text(d.data.level):''),metrics,note:'按 unit/number 识别：3/5 为 5 小时动态窗口，6/1 为 7 天窗口，与接口顺序无关。5 小时额度按每次请求消耗满 5 小时后恢复；周额度从套餐下单时起每 7 天刷新。剩余 = 100 − 官方已用 percentage；仅使用接口返回的 nextResetTime，不补造固定重置时间。订阅积分比例不是原始 Token 数，与 API 钱包独立。'};
 }
 if(id==='gemini-requests'){
  if(!/^[a-z][a-z0-9-]{4,61}[a-z0-9]$/.test(c.projectId||''))throw new Error('PROJECT');const start=new Date(now.getTime()-86400000).toISOString();let page=null,seen=new Set(),sum=0,found=false;
  for(let i=0;i<100;i++){const u=new URL(`https://monitoring.googleapis.com/v3/projects/${c.projectId}/timeSeries`);u.search=new URLSearchParams({filter:'metric.type="serviceruntime.googleapis.com/api/request_count" AND resource.type="consumed_api" AND resource.labels.service="generativelanguage.googleapis.com"','interval.startTime':start,'interval.endTime':now.toISOString(),view:'FULL',pageSize:'10000',...(page?{pageToken:page}:{})});
   const d=await call(u.href);if(d.executionErrors?.length)throw new Error('SCHEMA');if(d.timeSeries!==undefined&&!Array.isArray(d.timeSeries))throw new Error('SCHEMA');
   for(const series of d.timeSeries||[]){if(series.metricKind!=='DELTA'||series.valueType!=='INT64'||!Array.isArray(series.points))throw new Error('SCHEMA');for(const p of series.points){const n=need(p.value?.int64Value);if(n<0)throw new Error('SCHEMA');sum+=n;found=true;}}
   if(!d.nextPageToken)return {kind:'usage',plan:'Gemini API · Cloud Monitoring',periodStart:start,periodEnd:now.toISOString(),metrics:found?[metric('24 小时完成请求数（含失败）',sum,'次')]:[],note:found?'DELTA 请求计数；非 Token、非费用、非会员配额。官方统计可延迟约 30 分钟；区间边界受采样影响。':'该项目没有返回监控样本；不等于请求数为零。检查 API 监控是否可用与项目权限。'};
   if(seen.has(d.nextPageToken))throw new Error('SCHEMA');page=d.nextPageToken;seen.add(page);
  }throw new Error('SCHEMA');
 }
 if(id==='deepseek-balance'){
  const d=await call('https://api.deepseek.com/user/balance');if(!Array.isArray(d.balance_infos)||!d.balance_infos.length)throw new Error('SCHEMA');
  return {kind:'balance',plan:'API 按量付费',metrics:d.balance_infos.flatMap(b=>{if(!['CNY','USD'].includes(b.currency))throw new Error('SCHEMA');return [metric('可用余额',b.total_balance,b.currency),metric('赠送余额',b.granted_balance,b.currency),metric('充值余额',b.topped_up_balance,b.currency)];}),note:d.is_available===false?'官方标记当前余额不可用于调用':'余额不代表历史累计用量'};
 }
 if(id==='kimi-balance'){
  const d=await call('https://api.moonshot.cn/v1/users/me/balance');if(d.code!==0)throw new Error('SCHEMA');
  return {kind:'balance',plan:'Moonshot API · 国内',metrics:[metric('可用余额',d.data?.available_balance,'CNY'),metric('代金券',d.data?.voucher_balance,'CNY'),metric('现金',d.data?.cash_balance,'CNY')],note:'不包含 Kimi 网页会员或 Kimi Code 额度'};
 }
 if(id==='zenmux-balance'){
  const d=(await call('https://zenmux.ai/api/v1/management/payg/balance')).data;if(d?.currency!=='usd')throw new Error('SCHEMA');
  return {kind:'balance',plan:'PAYG',metrics:[metric('可用余额',d.total_credits,'USD'),metric('充值',d.top_up_credits,'USD'),metric('赠金',d.bonus_credits,'USD')]};
 }
 if(id==='zenmux-subscription'){
  const d=(await call('https://zenmux.ai/api/v1/management/subscription/detail')).data;if(!d?.plan)throw new Error('SCHEMA');let metrics=[];
  for(const [k,label] of [['quota_5_hour','5 小时'],['quota_7_day','7 天']]){const q=d[k];if(q){metrics.push(metric(label+'剩余',q.remaining_flows,'Flows',{total:num(q.max_flows),used:num(q.used_flows),resetAt:iso(q.resets_at)}));}}
  if(d.quota_monthly)metrics.push(metric('月度上限（无实时消耗）',d.quota_monthly.max_flows,'Flows'));
  if(num(d.quota_5_hour?.remaining_resets)!==null)metrics.push(metric('提前重置次数',d.quota_5_hour.remaining_resets,'次'));
  if(!metrics.length)throw new Error('SCHEMA');return {kind:'subscription',plan:text(d.plan.tier),expiresAt:iso(d.plan.expires_at),metrics,note:'月度上限不是月度剩余；所有窗口同时约束。'};
 }
 if(id==='minimax-quota'){
  const region=c.region==='global'?'https://www.minimax.io':'https://www.minimax.cn';const d=await call(region+'/v1/token_plan/remains');
  if(!Array.isArray(d.model_remains))throw new Error('SCHEMA');let metrics=[];
  for(const m of d.model_remains){for(const [field,label,reset,end] of [['current_interval_remaining_percent','窗口剩余','remains_time','end_time'],['current_weekly_remaining_percent','周窗口剩余','weekly_remains_time','weekly_end_time']]){const n=num(m[field]);if(n!==null&&n>=0&&n<=100){const ms=num(m[reset]),absolute=num(m[end]);const absoluteValid=absolute!==null&&absolute>=1e12&&absolute<8640000000000000;const estimated=!absoluteValid&&ms!==null&&ms>=0&&ms<31536000000;const resetAt=absoluteValid?new Date(absolute).toISOString():estimated?new Date(now.getTime()+ms).toISOString():null;const duration=num(m.end_time)!==null&&num(m.start_time)!==null?m.end_time-m.start_time:null;const windowLabel=field==='current_interval_remaining_percent'&&duration!==null&&Math.abs(duration-18000000)<1000?'5 小时剩余':label;const model=m.model_name==='general'?'通用模型（general）':text(m.model_name);metrics.push(metric(model+' · '+windowLabel,n,'%',{total:100,used:100-n,resetAt,resetEstimated:estimated}));}}}
  if(!metrics.length)throw new Error('MINIMAX_SCHEMA');return {kind:'subscription',plan:'Token Plan',metrics,note:'额度来自订阅服务，与 API 钱包余额独立。重置优先采用官方时间；根据倒计时计算的时间会标为估算。未使用窗口的重置时间可能随查询更新。'};
 }
 if(id==='xai-balance'){
  if(!/^[a-zA-Z0-9-]{1,100}$/.test(c.teamId||''))throw new Error('TEAM');const d=await call(`https://management-api.x.ai/v1/billing/teams/${c.teamId}/prepaid/balance`);
  return {kind:'balance',plan:'xAI API 预付账本',metrics:[metric('账本 total（原始符号）',need(d.total?.val)/100,'USD')],note:'官方 total 单位为美分，示例可为负数。未证实符号含义，保留原始符号，不宣称可用余额。'};
 }
 if(id==='xai-usage'){
  if(!/^[a-zA-Z0-9-]{1,100}$/.test(c.teamId||''))throw new Error('TEAM');const start=new Date(Date.UTC(now.getUTCFullYear(),now.getUTCMonth(),1));
  const d=await call(`https://management-api.x.ai/v1/billing/teams/${c.teamId}/usage`,{analyticsRequest:{timeRange:{startTime:start.toISOString(),endTime:now.toISOString(),timezone:'Etc/UTC'},timeUnit:'TIME_UNIT_DAY',values:[{name:'usd',aggregation:'AGGREGATION_SUM'}],groupBy:[],filters:[]}});
  if(!Array.isArray(d.timeSeries)||d.limitReached)throw new Error('SCHEMA');let sum=0;for(const s of d.timeSeries){if(!Array.isArray(s.dataPoints))throw new Error('SCHEMA');for(const p of s.dataPoints)sum+=need(p.values?.[0]);}
  return {kind:'usage',plan:'API 当月费用 · UTC',periodStart:start.toISOString(),periodEnd:now.toISOString(),metrics:[metric('当月累计费用',sum,'USD')],note:'UTC 自然月截至查询时刻；不代表 Grok 网页订阅'};
 }
 if(id==='openai-costs'){
  const start=Date.UTC(now.getUTCFullYear(),now.getUTCMonth(),1)/1000,end=Math.floor(now.getTime()/1000);let page=null,sum=0,seen=new Set();
  for(let i=0;i<100;i++){const u=new URL('https://api.openai.com/v1/organization/costs');u.search=new URLSearchParams({start_time:String(start),end_time:String(end),limit:'31',...(page?{page}:{})});const d=await call(u.href);if(!Array.isArray(d.data))throw new Error('SCHEMA');
   for(const b of d.data){if(!Array.isArray(b.results))throw new Error('SCHEMA');for(const r of b.results){if(r.amount?.currency!=='usd')throw new Error('SCHEMA');sum+=need(r.amount.value);}}
   if(!d.has_more)return {kind:'usage',plan:'OpenAI API 组织费用 · UTC',periodStart:new Date(start*1000).toISOString(),periodEnd:now.toISOString(),metrics:[metric('当月累计费用',sum,'USD')],note:'官方费用聚合可能延迟；不是 API 余额，也不是 ChatGPT 订阅剩余。'};
   if(!d.next_page||seen.has(d.next_page))throw new Error('SCHEMA');page=d.next_page;seen.add(page);
  }throw new Error('SCHEMA');
 }
 throw new Error('SCHEMA');
}
export function safeError(e){const c=e.message;if(c==='CODEX_LOGIN')return 'Codex 本机账号未登录或需要重新授权，请在官方 Codex 完成登录';if(c==='CODEX_LOCAL')return 'Codex 本机读取服务未就绪，请确认已安装 Codex CLI 且可正常启动';if(c==='KIMI_LOGIN')return 'Kimi Code 登录未完成或已过期，请在官方页面重新授权';if(c==='KIMI_LOCAL')return 'Kimi Code 本地连接未就绪，请启动订阅接入服务';if(c==='PROJECT')return '缺少有效 Google Cloud Project ID，请在终端配置';if(c==='TEAM')return '缺少有效 Team ID，请在本机终端配置';if(c==='MINIMAX_SCHEMA')return '接口未返回明确剩余百分比；拒绝解释歧义字段，请手动记录';if(c==='CREDENTIAL')return '凭据格式无效，请在终端重新配置';if(c==='HTTP_401'||c==='HTTP_403')return '认证失败或查询权限不足，请核对 Key 类型与权限';if(c==='HTTP_429'||c==='HTTP_422')return '服务限流，请稍后重试';if(c?.startsWith('HTTP_'))return '官方接口请求失败（'+c.slice(5)+'）';if(c==='SCHEMA')return '官方响应结构或业务状态不符合预期，未覆盖上次数据';return '网络连接或代理失败，请检查代理与网络';}
