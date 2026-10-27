// Only read the two documented account endpoints on the authenticated loopback service.
export async function requestKimiLocal(endpoint,c){
 if(!['/api/v1/oauth/usage','/api/v1/oauth/userinfo'].includes(endpoint)||!Number.isInteger(c.port)||c.port<1024||c.port>65535||typeof c.key!=='string'||!c.key)throw new Error('KIMI_LOCAL');
 const r=await fetch(`http://127.0.0.1:${c.port}${endpoint}`,{headers:{Authorization:'Bearer '+c.key},redirect:'error',signal:AbortSignal.timeout(25000)});
 if(!r.ok)throw new Error('HTTP_'+r.status);
 const chunks=[];let length=0;for await(const chunk of r.body){length+=chunk.length;if(length>1048576)throw new Error('SCHEMA');chunks.push(Buffer.from(chunk));}
 let d;try{d=JSON.parse(Buffer.concat(chunks).toString('utf8'));}catch{throw new Error('SCHEMA');}
 if(d.code!==0)throw new Error('SCHEMA');
 if(d.data?.kind==='error')throw new Error(d.data.status===401||/login|oauth|auth|credential/i.test(d.data.message||'')?'KIMI_LOGIN':'SCHEMA');
 if(d.data?.kind!=='ok')throw new Error('SCHEMA');return d;
}
export function normalizeKimiQuota(d,plan='Kimi Code / 会员'){
 const usages=d.data?.quota?.usages;if(!usages||typeof usages!=='object'||Array.isArray(usages))throw new Error('SCHEMA');
 const metrics=[];
 for(const [field,label] of [['limit5h','Code · 5 小时剩余'],['limit7d','Code · 7 天剩余'],['monthTotal','会员 · 月度总额度剩余'],['monthCode','Code · 月度额度剩余']]){
  const q=usages[field];if(q===undefined)continue;
  if(typeof q.usedRatio!=='number'||!Number.isFinite(q.usedRatio)||q.usedRatio<0||q.usedRatio>1)throw new Error('SCHEMA');
  const round=n=>Math.round(n*10000)/10000;
  metrics.push({label,value:round((1-q.usedRatio)*100),unit:'%',total:100,used:round(q.usedRatio*100),resetAt:typeof q.resetAt==='string'&&Number.isFinite(Date.parse(q.resetAt))?new Date(q.resetAt).toISOString():null});
 }
 if(!metrics.length)throw new Error('SCHEMA');
 return {kind:'subscription',plan,metrics,note:'来源：官方 Kimi Code 本地 OAuth 服务。剩余百分比 = (1 − usedRatio) × 100；只显示该账户实际返回的窗口，未返回的窗口不代表零。与 Moonshot API 钱包独立。'};
}
export function normalizeKimiExtra(d){
 const extra=d.data?.quota?.extraUsage;
 if(extra===null)return {kind:'balance',plan:'Kimi 会员 · 额外用量钱包',metrics:[],note:'官方返回未提供额外用量钱包；不视为零余额。这不是 Moonshot 开放平台 API 钱包。'};
 if(!extra||!['CNY','USD'].includes(extra.currency?.toUpperCase())||!Number.isSafeInteger(extra.balanceCents))throw new Error('SCHEMA');
 const unit=extra.currency.toUpperCase();return {kind:'balance',plan:'Kimi 会员 · 额外用量钱包',metrics:[{label:'额外用量可用余额',value:extra.balanceCents/100,unit}],note:'官方 balanceCents 按 100 换算为元 / 美元；属于 Kimi 会员额外用量，与 Moonshot API 钱包独立。'};
}
