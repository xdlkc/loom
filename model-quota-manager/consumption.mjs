import {requestKimiLocal,normalizeKimiQuota} from './kimi-local.mjs';
import {browserConsumptionSeries} from './browser-consumption.mjs';
import {load} from './storage.mjs';
import {miniMaxSeries} from './minimax-consumption.mjs';
import {request} from './adapters.mjs';
import {readCodexUsage} from './codex-local.mjs';
export const consumptionSources={
 zenmux:{label:'ZenMux · Builder 订阅',url:'https://zenmux.ai/platform/usage',docs:'https://zenmux.ai/docs/api/platform/account-usage.html'},
 glm:{label:'GLM Coding Plan',url:'https://bigmodel.cn/console/overview',docs:'https://github.com/zai-org/zai-coding-plugins/blob/main/plugins/glm-plan-usage/skills/usage-query-skill/scripts/query-usage.mjs'},
 chatgpt:{label:'ChatGPT / Codex 账号',url:'https://chatgpt.com/codex/settings/usage',docs:'https://learn.chatgpt.com/docs/app-server'},
 deepseek:{label:'DeepSeek API',url:'https://platform.deepseek.com/usage',reason:'官方用量页提供按时间、模型的 Token 消耗图；当前只接入了余额查询，尚未接入这份消耗序列。'},
 grok:{label:'Grok 网页订阅',url:'https://grok.com/dashboard/usage',reason:'官方页面提供 Tokens per day 和产品分组；当前扩展只同步汇总，尚未同步逐日消耗序列。累计汇总不能替代逐日消耗。'},
 kimi:{label:'Kimi Code / 会员',url:'https://www.kimi.com/settings/subscription?tab=quota',reason:'已查看本账号使用明细：按时间记录额度百分比，未提供 Token 数；不能将额度百分比换算为 Token。'},
 gemini:{label:'Gemini 网页会员',url:'https://gemini.google.com/u/1/usage',reason:'已查看目标会员账号：页面提供当前和每周限额百分比，未提供按时段 Token 消耗。Gemini API 的统计不代表网页会员。'},
 minimax:{label:'MiniMax 国内 Token Plan',url:'https://platform.minimax.cn/console/usage',reason:'官方套餐页说明可查看套餐用量；已确认本账号套餐页提供按小时、按模型的输入/输出/缓存消耗明细；首次同步后可显示曲线。remains 仍只用于额度。'}
};
const numeric=v=>{if(v===null||v===undefined||v==='')return null;const n=typeof v==='number'?v:typeof v==='string'&&/^\d+(\.\d+)?$/.test(v)?Number(v):NaN;if(!Number.isSafeInteger(n)||n<0)throw Error('SCHEMA');return n;};
const sum=(a,b)=>{if(a===null||b===null)return null;const n=a+b;if(!Number.isSafeInteger(n))throw Error('SCHEMA');return n;};
const china=d=>d.toLocaleString('sv-SE',{timeZone:'Asia/Shanghai'});
export function bounds(range,now=new Date(),zone='UTC'){
 if(!['today','7d','30d'].includes(range))throw Error('RANGE');const day=(zone==='Asia/Shanghai'?china(now):now.toISOString()).slice(0,10),days=range==='today'?1:range==='7d'?7:30,from=new Date(day+'T00:00:00Z');from.setUTCDate(from.getUTCDate()-days+1);return {startDay:from.toISOString().slice(0,10),endDay:day};
}
function base(provider,range,zone,granularity){return {available:true,provider,range,unit:'Tokens',scope:consumptionSources[provider].label,officialUrl:consumptionSources[provider].url,sourceUrl:consumptionSources[provider].docs,timeZone:zone,granularity,updatedAt:new Date().toISOString(),series:[]};}
export function normalizeZenmuxUsage(responses,range,now=new Date()){
 const {startDay,endDay}=bounds(range,now),result=base('zenmux',range,'UTC',range==='today'?'hour':'day'),groups=new Map();
 for(const response of responses){const d=response?.data;if(response?.success!==true||!d||!Array.isArray(d.tokensByModel))throw Error('SCHEMA');for(const r of d.tokensByModel){if(typeof r.modelSlug!=='string'||!/^[a-zA-Z0-9_./:@-]{1,200}$/.test(r.modelSlug))throw Error('SCHEMA');const t=r.bizTime;if(typeof t!=='string'||!/^\d{8}(\d{2})?$/.test(t)||t.length!==(range==='today'?10:8))throw Error('SCHEMA');const day=t.slice(0,4)+'-'+t.slice(4,6)+'-'+t.slice(6,8);if(day<startDay||day>endDay)continue;const at=day+(t.length===10?'T'+t.slice(8,10)+':00:00Z':'T00:00:00Z');if(!Number.isFinite(Date.parse(at))||new Date(at).toISOString().slice(0,10)!==day)throw Error('SCHEMA');if(Date.parse(at)>now.getTime())continue;const key=r.modelSlug+'|'+at;if(groups.has(key))throw Error('SCHEMA');groups.set(key,{model:r.modelSlug,at,label:day+(t.length===10?' '+t.slice(8,10)+':00':''),tokens:numeric(r.tokens)});}}
 result.series=Object.entries(Object.groupBy([...groups.values()],p=>p.model)).map(([model,points])=>({model,points:points.sort((a,b)=>a.at.localeCompare(b.at)).map(({model,...p})=>p)}));return result;
}
export function normalizeGlmUsage(response,range,now=new Date()){
 const d=response?.data;if(response?.success===false||response?.code!==200||!Array.isArray(d?.x_time)||!Array.isArray(d.tokensUsage))throw Error('SCHEMA');const {startDay,endDay}=bounds(range,now,'Asia/Shanghai'),result=base('glm',range,'Asia/Shanghai',range==='today'?'hour':'day');
 if(range==='today'&&d.x_time.length&&d.x_time.every(t=>typeof t==='string'&&t.length===10))result.granularity='day';const sources=[{model:'全部模型',tokensUsage:d.tokensUsage},...(Array.isArray(d.modelDataList)?d.modelDataList:[])];
 for(const s of sources){if(typeof s.model!=='string'&&typeof s.modelName!=='string'||!Array.isArray(s.tokensUsage)||s.tokensUsage.length!==d.x_time.length)throw Error('SCHEMA');const model=s.model||s.modelName;if(!/^[a-zA-Z0-9_ ./:-]{1,120}$/.test(model)&&model!=='全部模型')throw Error('SCHEMA');const groups=new Map();d.x_time.forEach((t,i)=>{if(typeof t!=='string'||!/^\d{4}-\d{2}-\d{2}( \d{2}:\d{2})?$/.test(t))throw Error('SCHEMA');const day=t.slice(0,10);if(day<startDay||day>endDay)return;const rawAt=day+'T'+(t.length>10?t.slice(11)+':00':'00:00:00')+'+08:00';if(!Number.isFinite(Date.parse(rawAt))||china(new Date(rawAt)).slice(0,10)!==day)throw Error('SCHEMA');if(Date.parse(rawAt)>now.getTime())return;const label=range==='today'?t:day,at=label.length===10?label+'T00:00:00+08:00':label.replace(' ','T')+':00+08:00';if(!Number.isFinite(Date.parse(at)))throw Error('SCHEMA');const n=numeric(s.tokensUsage[i]);if(groups.has(at))groups.get(at).tokens=sum(groups.get(at).tokens,n);else groups.set(at,{at:new Date(at).toISOString(),label,tokens:n});});result.series.push({model,points:[...groups.values()].sort((a,b)=>a.at.localeCompare(b.at))});}
 return result;
}
export function normalizeCodexUsage(data,range,now=new Date()){
 if(!data||!Object.hasOwn(data,'dailyUsageBuckets'))throw Error('SCHEMA');const result=base('chatgpt',range,'官方日历日期（未注明时区）','day');if(data.dailyUsageBuckets===null){result.note='官方尚未返回每日 Token 数据。';return result;}if(!Array.isArray(data.dailyUsageBuckets))throw Error('SCHEMA');const {startDay,endDay}=bounds(range,now),seen=new Set(),points=[];for(const b of data.dailyUsageBuckets){if(typeof b.startDate!=='string'||!/^\d{4}-\d{2}-\d{2}$/.test(b.startDate)||seen.has(b.startDate))throw Error('SCHEMA');seen.add(b.startDate);if(b.startDate<startDay||b.startDate>endDay)continue;const at=b.startDate+'T00:00:00Z';if(!Number.isFinite(Date.parse(at))||new Date(at).toISOString().slice(0,10)!==b.startDate)throw Error('SCHEMA');points.push({at,label:b.startDate,tokens:numeric(b.tokens)});}result.series=[{model:'全部模型',points:points.sort((a,b)=>a.label.localeCompare(b.label))}];return result;
}
export async function queryConsumption(provider,range,{credentials={},settings={},get=request,readUsage=readCodexUsage,getKimi=requestKimiLocal,readMiniMax=()=>load('minimax-consumption',null),readBrowser=p=>load(p+'-consumption',null),now=new Date()}={}){
 const source=consumptionSources[provider];if(!source||!['today','7d','30d'].includes(range))throw Error('RANGE');if(provider==='kimi'){
  const c=credentials.kimiCode;if(!c?.key)return {available:false,provider,officialUrl:'https://www.kimi.com/code/console',scope:'Kimi Code / 会员',reason:'请先连接本机 Kimi Code 账号。'};
  const q=normalizeKimiQuota(await getKimi('/api/v1/oauth/usage',c));return {available:false,provider,scope:'Kimi Code / 会员 · 官方用量',officialUrl:'https://www.kimi.com/code/console',updatedAt:now.toISOString(),quotaSummary:q.metrics.map(m=>({label:m.label.replace('剩余','已用'),used:m.used,remaining:m.value,resetAt:m.resetAt,unit:'%'})),reason:'官方当前提供额度已用比例和请求记录，未提供可核实的逐日 Token 消耗序列。下方显示真实额度用量，不将百分比换算为 Token，不使用 Moonshot API 钱包替代。'};
 }if(['deepseek','grok'].includes(provider)){const d=browserConsumptionSeries(provider,readBrowser(provider),range,now),status=settings.tokenBrowser?.[provider];if(d.available){d.note+=status?.enabled?' 已启用每15分钟后台检查。':' 自动同步未启用。';if(status?.error)d.warning='浏览器更新失败，保留上次完整数据'+(status.error==='ACCOUNT_CHANGED'?'；账号已变化，请确认后重新连接。':'。');}return d;}if(provider==='minimax'){const d=miniMaxSeries(readMiniMax(),range,now);if(d.available){d.note+=(settings.minimaxBrowser?' 自动同步已启用，每15分钟检查一次。':' 当前为已采集网页快照，自动同步尚未启用。');if(settings.minimaxBrowserError)d.warning='浏览器未能更新，保留上次消耗数据。';}return d;}if(!['zenmux','glm','chatgpt'].includes(provider))return {available:false,provider,officialUrl:source.url,scope:source.label,reason:source.reason};
 if(provider==='chatgpt'){if(!settings.codexLocal)return {available:false,provider,officialUrl:source.url,reason:'请先在设置连接本机 Codex 账号。'};return normalizeCodexUsage((await readUsage()).data,range,now);}
 const key=credentials[provider]?.key;if(!key)return {available:false,provider,officialUrl:source.url,reason:'请先通过本机隐藏输入配置该服务的查询密钥。'};
 const {startDay,endDay}=bounds(range,now,provider==='glm'?'Asia/Shanghai':'UTC');
 if(provider==='glm'){const params=new URLSearchParams({startTime:startDay+' 00:00:00',endTime:china(now)});return normalizeGlmUsage(await get('https://open.bigmodel.cn/api/monitor/usage/model-usage?'+params,key,settings.proxy,undefined,'raw'),range,now);}
 const periods=range==='today'?[endDay.replaceAll('-','')]:[...new Set([startDay.slice(0,7),endDay.slice(0,7)])].map(s=>s.replace('-',''));const responses=[];for(const period of periods){const params=new URLSearchParams({type:'usage',query_dimension:range==='today'?'BIZ_DT':'BIZ_MTH',query_time:period,bill_types:'subscription'});responses.push(await get('https://zenmux.ai/api/v1/management/usage?'+params,key,settings.proxy));}return normalizeZenmuxUsage(responses,range,now);
}
