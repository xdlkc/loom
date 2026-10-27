import fs from 'node:fs';import {dataDir,load} from './storage.mjs';
const base='http://127.0.0.1:'+(process.env.PORT||4318);
try{
 const session=await(await fetch(base+'/api/session')).json();console.log('正在查询 DeepSeek API 钱包，不涉及网页聊天用量。');
 const res=await fetch(base+'/api/refresh',{method:'POST',headers:{Origin:base,'Content-Type':'application/json','X-CSRF-Token':session.csrf},body:JSON.stringify({id:'deepseek-balance'})});if(!res.ok)throw new Error();
 const d=await res.json(),a=d.attempts['deepseek-balance'],r=d.observations['deepseek-balance'];
 if(!d.configured['deepseek-balance']||a?.status!=='ok'||!r){console.log('查询未成功：'+(a?.message||'尚未配置 API Key'));process.exitCode=1;}
 else{
  for(const m of r.metrics)console.log('  '+m.label+'：'+m.value+' '+m.unit);
  for(let i=0;i<r.metrics.length;i+=3){const [total,grant,topup]=r.metrics.slice(i,i+3);if(!total||!grant||!topup||Math.abs(total.value-grant.value-topup.value)>.0001){console.log('分项与总额不一致，请以官方控制台核对。');process.exitCode=1;}}
  console.log('成功获取：'+new Date(r.observedAt).toLocaleString('zh-CN',{timeZone:'Asia/Shanghai',hour12:false})+'（北京时间）');console.log(r.note||'');
 }
 const c=load('credentials',{}).deepseek;if(c?.key&&JSON.stringify(d).includes(c.key))throw new Error();
 console.log('凭据文件权限：'+(fs.statSync(dataDir+'/credentials.json').mode&0o777).toString(8)+'（应为 600）；未显示密钥。');
}catch{console.error('本机验证未完成，请检查工具服务和数据状态；未输出认证。');process.exitCode=1;}
