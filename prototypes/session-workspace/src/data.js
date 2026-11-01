export const configText='# Project configuration\nname = "loom-demo"\nversion = "0.1.0"\ndebug = true\nlog_level = "info"\nfeatures = ["agent", "trace", "local"]';
export const initialSessions=[
 {id:'config',title:'读取项目配置',agent:'pi',model:'glm-5.3',time:'今天 00:48',duration:3.86,icon:'flask',status:'success',tokens:[1571,20],cache:48.9},
 {id:'logs',title:'分析错误日志',agent:'claude-code',model:'sonnet-5.5',time:'今天 00:32',duration:12.4,icon:'file',status:'success',tokens:[4231,186],cache:64.2},
 {id:'logic',title:'修复构建报错',agent:'codex',model:'gpt-6',time:'今天 00:18',duration:28.1,icon:'terminal',status:'success',tokens:[6870,420],cache:72.1},
 {id:'search',title:'搜索代码定义',agent:'pi',model:'glm-5.3',time:'昨天 23:51',duration:7.6,icon:'search',status:'success',tokens:[2180,92],cache:39.5},
 {id:'login',title:'实现登录接口',agent:'claude-code',model:'sonnet-5.5',time:'昨天 23:20',duration:41.3,icon:'list',status:'success',tokens:[8942,773],cache:68.6},
 {id:'docs',title:'阅读设计文档',agent:'opencode2',model:'deepseek-v4-pro',time:'昨天 22:11',duration:5.8,icon:'file',status:'success',tokens:[3201,170],cache:51.8},
 {id:'refactor',title:'重构工具函数',agent:'codex',model:'gpt-6',time:'昨天 21:03',duration:19.2,icon:'gear',status:'error',tokens:[3120,42],cache:23.6},
 {id:'tests',title:'生成单元测试',agent:'pi',model:'glm-5.3',time:'昨天 20:16',duration:33.7,icon:'branch',status:'success',tokens:[5720,682],cache:43.9},
];
export const seconds=v=>v==null?'未采集':`${v.toFixed(v>=10?1:2)} s`;
export const number=n=>new Intl.NumberFormat('en-US').format(n);
export function stepsFor(s){
 const exact=s.id==='config',first=exact?3.83:+(s.duration*.72).toFixed(2),last=exact?.02:+(s.duration-first-.06).toFixed(2),input=exact?774:Math.round(s.tokens[0]*.46);
 const cachedTotal=Math.round(s.tokens[0]*s.cache/100),cachedFirst=exact?0:Math.round(cachedTotal*input/s.tokens[0]);
 return [
  {id:'model-1',kind:'model',title:'模型调用',model:s.model,time:'00:48:02',duration:first,ttft:exact?3.82:+(first*.9).toFixed(2),offset:0,status:'success',input,output:exact?12:Math.round(s.tokens[1]*.25),cached:cachedFirst,user:exact?'请读取当前目录下的 fixture.txt 文件，告诉我其中的主要配置项。':`请帮我${s.title}，检查相关文件并说明结果。`,summary:'分析请求并调用文件读取工具。',outputText:'需要先读取项目文件，确认配置内容。',params:{tool:'read',path:exact?'fixture.txt':'src/config.ts'}},
  {id:'tool-read',kind:'tool',title:'工具调用',tool:'read',file:exact?'fixture.txt':'src/config.ts',time:'00:48:06',duration:null,offset:exact?3.835:first+.02,status:'success',params:{path:exact?'fixture.txt':'src/config.ts'},result:configText},
  {id:'model-2',kind:'model',title:'模型调用',model:s.model,time:'00:48:07',duration:last,ttft:exact?.01:+(last*.64).toFixed(2),offset:exact?3.84:first+.06,status:s.status,input:s.tokens[0]-input,output:s.tokens[1]-(exact?12:Math.round(s.tokens[1]*.25)),cached:cachedTotal-cachedFirst,summary:s.status==='error'?'等待上游响应时发生超时。':'根据读取结果，整理主要配置项。',outputText:s.status==='error'?'上游响应超时，未收到完整的模型回复。':`已完成${s.title}，主要配置项已确认。`,params:{model:s.model,stream:true},user:'根据文件内容，返回主要配置项。',error:s.status==='error'?'UPSTREAM_UNAVAILABLE [timeout]':null},
 ];
}
