import {spawn} from 'node:child_process';import os from 'node:os';import path from 'node:path';
// Read-only official app-server protocol. Never read or copy OAuth files.
export function readCodexLimits({spawnFn=spawn,timeoutMs=25000,usage=false}={}){return new Promise((resolve,reject)=>{
 const env={...process.env};delete env.OPENAI_API_KEY;delete env.CODEX_API_KEY;
 const child=spawnFn(path.join(os.homedir(),'.local/bin/codex'),['app-server','--stdio'],{stdio:['pipe','pipe','pipe'],env});
 let buffer='',size=0,done=false;const pending=new Map();let next=1;
 const stop=(error,result)=>{if(done)return;done=true;clearTimeout(timer);child.stdin.end();child.kill('SIGTERM');setTimeout(()=>{if(child.exitCode===null)child.kill('SIGKILL');},1000).unref();if(error)reject(error);else resolve(result);};
 const timer=setTimeout(()=>stop(Error('CODEX_LOCAL')),timeoutMs);child.stderr.resume();child.stdin.on('error',()=>{});child.on('error',()=>stop(Error('CODEX_LOCAL')));child.on('close',()=>{if(!done)stop(Error('CODEX_LOCAL'));});
 child.stdout.on('data',chunk=>{size+=chunk.length;if(size>2*1024*1024){stop(Error('SCHEMA'));return;}buffer+=chunk;let index;while((index=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,index);buffer=buffer.slice(index+1);let msg;try{msg=JSON.parse(line);}catch{continue;}const p=pending.get(msg.id);if(!p)continue;pending.delete(msg.id);if(msg.error)p.reject(Error('CODEX_LOGIN'));else p.resolve(msg.result);}});
 function rpc(method,params){return new Promise((resolve,reject)=>{const id=next++;pending.set(id,{resolve,reject});child.stdin.write(JSON.stringify({id,method,...(params===undefined?{}:{params})})+'\n');});}
 (async()=>{try{await rpc('initialize',{clientInfo:{name:'local_quota_manager',title:'Local Quota Manager',version:'1.0.0'}});child.stdin.write(JSON.stringify({method:'initialized'})+'\n');const account=await rpc('account/read',{refreshToken:false});if(account.account?.type!=='chatgpt')throw Error('CODEX_LOGIN');const data=usage?await rpc('account/usage/read'):await rpc('account/rateLimits/read',{excludeResetCreditDetails:true});stop(null,{data,planType:account.account.planType});}catch(e){stop(Error(e.message==='CODEX_LOGIN'?'CODEX_LOGIN':'CODEX_LOCAL'));}})();
});}
const safePlan=p=>typeof p==='string'&&/^[a-z_0-9-]{1,50}$/i.test(p)?p:'未知方案';
export function normalizeCodexQuota(data,planType){if(!data||typeof data!=='object')throw Error('SCHEMA');const buckets=data.rateLimitsByLimitId&&Object.keys(data.rateLimitsByLimitId).length?Object.entries(data.rateLimitsByLimitId):data.rateLimits?[['codex',data.rateLimits]]:[];const metrics=[];
 const duration=n=>typeof n!=='number'||!Number.isFinite(n)||n<=0?null:n%1440===0?(n/1440)+' 天':n%60===0?(n/60)+' 小时':n+' 分钟';
 for(const [id,bucket]of buckets){const name=/^[a-zA-Z0-9_-]{1,80}$/.test(id)?id:'额度池';for(const [field,index]of [['primary',1],['secondary',2]]){const q=bucket?.[field];if(q==null)continue;if(typeof q.usedPercent!=='number'||!Number.isFinite(q.usedPercent)||q.usedPercent<0||q.usedPercent>100)throw Error('SCHEMA');const label=duration(q.windowDurationMins)||'窗口 '+index;const reset=typeof q.resetsAt==='number'&&q.resetsAt>0&&q.resetsAt<8640000000000?new Date(q.resetsAt*1000).toISOString():null;const round=n=>Math.round(n*10000)/10000;metrics.push({label:name+' · '+label+'剩余',value:round(100-q.usedPercent),unit:'%',total:100,used:round(q.usedPercent),resetAt:reset});}}
 if(!metrics.length)throw Error('SCHEMA');const count=data.rateLimitResetCredits?.availableCount;if(Number.isSafeInteger(count)&&count>=0)metrics.push({label:'可用额度重置券（仅查询）',value:count,unit:'次',total:null,used:null,resetAt:null});
 const rawPlan=planType||buckets.find(([,b])=>b.planType)?.[1]?.planType;const plan=({pro:'Pro',plus:'Plus',free:'Free',team:'Team',business:'Business'})[rawPlan]||rawPlan;return {kind:'subscription',plan:'Codex · ChatGPT '+safePlan(plan),metrics,note:'来源：本机官方 Codex app-server 的 account/rateLimits/read。仅显示实际返回的额度池与窗口；剩余 = 100% − 已用。不会消耗重置券。不是 ChatGPT 网页所有模型的剩余消息次数，也不是 OpenAI API 钱包；未折算 workspace credits 为现金。'};
}

export const readCodexUsage=options=>readCodexLimits({...options,usage:true});
