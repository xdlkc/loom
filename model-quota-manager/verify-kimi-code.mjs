import fs from 'node:fs';
import {dataDir} from './storage.mjs';
const base='http://127.0.0.1:'+(process.env.PORT||4318);
try{
 const session=await(await fetch(base+'/api/session')).json();let ok=true;
 for(const id of ['kimi-code-quota','kimi-code-extra']){
  const res=await fetch(base+'/api/refresh',{method:'POST',headers:{Origin:base,'Content-Type':'application/json','X-CSRF-Token':session.csrf},body:JSON.stringify({id})});if(!res.ok)throw new Error();
  const d=await res.json(),r=d.observations[id],a=d.attempts[id];
  if(!r||a?.status!=='ok'){console.log('查询未完成：'+(a?.message||'尚未配置'));ok=false;continue;}
  console.log('\n'+r.plan);for(const m of r.metrics){console.log('  '+m.label+'：'+m.value+' '+m.unit);if(m.resetAt)console.log('  重置：'+new Date(m.resetAt).toLocaleString('zh-CN',{timeZone:'Asia/Shanghai',hour12:false})+'（北京时间）');}if(!r.metrics.length)console.log('  官方未提供钱包，不视为零余额。');
  console.log('  成功获取：'+new Date(r.observedAt).toLocaleString('zh-CN',{timeZone:'Asia/Shanghai',hour12:false}));
 }
 console.log('\n凭据文件权限：'+(fs.statSync(dataDir+'/credentials.json').mode&0o777).toString(8)+'；未显示认证。');
 console.log('本次仅验证 Kimi Code / 会员，未查询 Moonshot API 钱包。');if(!ok)process.exitCode=1;
}catch{console.error('本机验证未完成，请确认额度工具正在运行。');process.exitCode=1;}
