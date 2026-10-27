import fs from 'node:fs';
import {dataDir} from './storage.mjs';
const base='http://127.0.0.1:'+(process.env.PORT||4318);
try{
 const session=await(await fetch(base+'/api/session')).json();
 console.log('正在查询 Moonshot 国内开放平台 API 钱包；这不查询 Kimi Code 订阅。');
 const res=await fetch(base+'/api/refresh',{method:'POST',headers:{Origin:base,'Content-Type':'application/json','X-CSRF-Token':session.csrf},body:JSON.stringify({id:'kimi-balance'})});
 if(!res.ok)throw new Error();
 const state=await res.json(),r=state.observations['kimi-balance'],attempt=state.attempts['kimi-balance'];
 if(!state.configured['kimi-balance']){console.log('尚未配置 Moonshot 国内开放平台 API Key。');process.exitCode=1;}
 else if(attempt?.status!=='ok'||!r){console.log('查询未成功：'+(attempt?.message||'尚无成功结果'));console.log('请核对是否为国内开放平台 Key；Kimi Code 和国际站 Key 不通用。');process.exitCode=1;}
 else{
  console.log('\nMoonshot API 钱包（国内，人民币）：');
  for(const m of r.metrics)console.log('  '+m.label+'：'+m.value+' '+m.unit);
  console.log('成功获取：'+new Date(r.observedAt).toLocaleString('zh-CN',{timeZone:'Asia/Shanghai',hour12:false})+'（北京时间）');
  console.log('Kimi Code 订阅配额：未通过此次钱包查询获取，独立管理。');
 }
 console.log('本机凭据文件权限：'+(fs.statSync(dataDir+'/credentials.json').mode&0o777).toString(8)+'（应为 600）');
 console.log('查看网页：'+base);
}catch{console.error('本机验证未完成，请确认工具已启动，再运行 node verify-kimi.mjs。');process.exitCode=1;}
