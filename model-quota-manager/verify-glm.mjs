import fs from 'node:fs';
import {dataDir} from './storage.mjs';
const base='http://127.0.0.1:'+(process.env.PORT||4318);
try{
 const session=await(await fetch(base+'/api/session')).json();
 console.log('正在查询智谱 GLM Coding Plan 订阅配额；本次不查询 API 钱包。');
 const res=await fetch(base+'/api/refresh',{method:'POST',headers:{Origin:base,'Content-Type':'application/json','X-CSRF-Token':session.csrf},body:JSON.stringify({id:'glm-quota'})});
 if(!res.ok)throw new Error();const d=await res.json(),a=d.attempts['glm-quota'],r=d.observations['glm-quota'];
 if(!d.configured['glm-quota']){console.log('尚未配置套餐 Key。');process.exitCode=1;}
 else if(a?.status!=='ok'||!r){console.log('查询未成功：'+(a?.message||'尚无结果'));console.log('请核对 Key 是否属于对应的个人 / 团队编程套餐。');process.exitCode=1;}
 else{console.log('\n'+r.plan);for(const m of r.metrics){console.log('  '+m.label+'：'+m.value+' '+m.unit+(m.used==null?'':'；已用 '+m.used+' '+m.unit));if(m.resetAt)console.log('  重置：'+new Date(m.resetAt).toLocaleString('zh-CN',{timeZone:'Asia/Shanghai',hour12:false})+'（北京时间）');}console.log('成功获取：'+new Date(r.observedAt).toLocaleString('zh-CN',{timeZone:'Asia/Shanghai',hour12:false})+'（北京时间）');console.log('未返回的套餐等级、窗口或重置时间保持未知，不补造。');}
 console.log('凭据文件权限：'+(fs.statSync(dataDir+'/credentials.json').mode&0o777).toString(8)+'（应为 600）；未显示密钥。');
}catch{console.error('本机验证未完成，请确认工具正在运行。');process.exitCode=1;}
