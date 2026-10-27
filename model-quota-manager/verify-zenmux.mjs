import fs from 'node:fs';
import {dataDir} from './storage.mjs';
const base='http://127.0.0.1:'+(process.env.PORT||4318);
try{
 const session=await (await fetch(base+'/api/session')).json();
 console.log('正在通过本机服务查询 ZenMux 套餐与 PAYG 钱包…');
 for(const id of ['zenmux-subscription','zenmux-balance']){
  const res=await fetch(base+'/api/refresh',{method:'POST',headers:{Origin:base,'Content-Type':'application/json','X-CSRF-Token':session.csrf},body:JSON.stringify({id})});
  if(!res.ok)throw new Error();
  const state=await res.json(),attempt=state.attempts[id],r=state.observations[id];
  if(!state.configured[id]){console.log('尚未配置 Management Key。');continue;}
  if(attempt?.status!=='ok'||!r){console.log((id==='zenmux-subscription'?'订阅':'PAYG')+'查询未成功：'+(attempt?.message||'尚无成功结果'));continue;}
  console.log('\n'+(id==='zenmux-subscription'?'订阅套餐':'PAYG 钱包')+'：'+r.plan);
  for(const m of r.metrics){console.log('  '+m.label+'：'+m.value+' '+m.unit+(m.total==null?'':' / 总量 '+m.total)+(m.used==null?'':'；已用 '+m.used));if(m.resetAt)console.log('  重置：'+new Date(m.resetAt).toLocaleString('zh-CN',{timeZone:'Asia/Shanghai',hour12:false})+'（北京时间）');}
  if(r.expiresAt)console.log('  订阅到期：'+new Date(r.expiresAt).toLocaleString('zh-CN',{timeZone:'Asia/Shanghai',hour12:false})+'（北京时间）');
  console.log('  成功获取：'+new Date(r.observedAt).toLocaleString('zh-CN',{timeZone:'Asia/Shanghai',hour12:false})+'（北京时间）');
 }
 console.log('\n凭据文件权限：'+(fs.statSync(dataDir+'/credentials.json').mode&0o777).toString(8)+'（应为 600）');
 console.log('查询结果已保存。打开 '+base+'，刷新网页即可查看。');
}catch{console.error('本机验证未完成：请确认工具已启动，然后重新运行 node verify-zenmux.mjs。');process.exitCode=1;}
