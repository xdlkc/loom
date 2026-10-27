import fs from 'node:fs';import {dataDir,load} from './storage.mjs';import {request,safeError} from './adapters.mjs';
const base='http://127.0.0.1:'+(process.env.PORT||4318);
try{
 const c=load('credentials',{}).xai;if(!c?.key||!c.teamId){console.error('请先隐藏配置 Management Key 和 Team ID。');process.exit(1);}
 try{const validation=await request('https://management-api.x.ai/auth/management-keys/validation',c.key,load('state',{}).settings?.proxy||'');const team=validation.scope==='SCOPE_ORGANIZATION'?null:validation.scope==='SCOPE_TEAM'?validation.scopeId:validation.teamId;if(team&&team!==c.teamId){console.error('Management Key 与输入的 Team ID 不匹配，请在本机终端重新配置。');process.exit(1);}console.log('Management Key 官方验证接口已通过；'+(team?'团队归属一致。':'后续以账单接口验证团队访问权限。'));}catch(e){console.error('Management Key 验证未通过：'+safeError(e));process.exit(1);}
 const session=await(await fetch(base+'/api/session')).json();let ok=true;
 for(const id of ['xai-balance','xai-usage']){
  const res=await fetch(base+'/api/refresh',{method:'POST',headers:{Origin:base,'Content-Type':'application/json','X-CSRF-Token':session.csrf},body:JSON.stringify({id})});if(!res.ok)throw new Error();const d=await res.json(),a=d.attempts[id],r=d.observations[id];
  if(JSON.stringify(d).includes(c.key)||JSON.stringify(d).includes(c.teamId))throw new Error();
  if(!r||a?.status!=='ok'){console.log((id==='xai-balance'?'预付账本':'API 用量')+'查询未成功：'+(a?.message||'尚无成功结果'));ok=false;continue;}
  console.log('\n'+r.plan);for(const m of r.metrics)console.log('  '+m.label+'：'+m.value+' '+m.unit);console.log(r.note||'');console.log('成功获取：'+new Date(r.observedAt).toLocaleString('zh-CN',{timeZone:'Asia/Shanghai',hour12:false})+'（北京时间）');
 }
 console.log('\n凭据文件权限：'+(fs.statSync(dataDir+'/credentials.json').mode&0o777).toString(8)+'（应为 600）；未显示 Key 或 Team ID。');console.log('本次未查询 Grok 网页订阅额度。');if(!ok)process.exitCode=1;
}catch{console.error('本机验证未完成，请检查工具服务；未输出认证。');process.exitCode=1;}
