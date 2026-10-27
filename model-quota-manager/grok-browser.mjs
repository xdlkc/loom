const allowedPlan=/^(?:Super|SuperGrok(?: Heavy)?|Heavy|Premium\+?|Pro|Ultra|Free|Basic|X Premium\+?)$/;
export function normalizeGrokSnapshot(body,now=Date.now()){
 if(!body||typeof body!=='object'||!['https://grok.com/dashboard','https://grok.com/dashboard/','https://grok.com/dashboard/usage'].includes(body.pageUrl))throw Error('GROK_PAGE');
 if(typeof body.usedPercent!=='number'||!Number.isFinite(body.usedPercent)||body.usedPercent<0||body.usedPercent>100)throw Error('GROK_PAGE');
 const time=Date.parse(body.observedAt);if(!Number.isFinite(time)||Math.abs(time-now)>300000)throw Error('GROK_PAGE');
 const round=n=>Math.round(n*10000)/10000;
 const resetText=typeof body.resetText==='string'&&/^[A-Za-z]{3,9} \d{1,2}(?:, \d{4})?$/.test(body.resetText)?body.resetText:null;
 const plan=typeof body.plan==='string'&&allowedPlan.test(body.plan)?body.plan:'未显示档位';
 const subscription={kind:'subscription',plan:'Grok 网页订阅 · '+plan,source:'browser',sourceUrl:body.pageUrl,observedAt:new Date(time).toISOString(),metrics:[{label:'官方周包含额度（共享池）剩余',value:round(100-body.usedPercent),unit:'%',total:100,used:round(body.usedPercent),resetAt:null,resetText}],note:'来源：登录后的官方网页显示，不是公开查询 API。剩余由 100% 减已用计算；该额度池可能覆盖 Grok、Cursor、Grok Bot 等产品。重置只显示页面日期，具体时刻和时区未知。未读取 API 钱包或网页 Credits。'};
 const total=body.totalTokens,included=body.includedTokens;
 const valid=Number.isSafeInteger(total)&&total>=0&&Number.isSafeInteger(included)&&included>=0&&included<=total;
 const range=typeof body.rangeText==='string'&&body.rangeText.length<80&&/^[A-Za-z0-9 ,–—\-]+$/.test(body.rangeText)?body.rangeText:null;
 const usage={kind:'webUsage',plan:'Grok 官方仪表盘 · 网页用量汇总',source:'browser',sourceUrl:body.pageUrl,observedAt:new Date(time).toISOString(),metrics:valid?[{label:'页面总 Token 用量',value:total,unit:'Tokens'},{label:'其中包含额度 Token',value:included,unit:'Tokens'}]:[],note:valid?'页面日期范围：'+(range||'未显示')+'。这是官方订阅仪表盘的共享产品汇总，不是 xAI API 用量；不导入费用、Credits 或 API 余额。':'本次后台采集未取得可核实的 Token 汇总，保持未知，不视为零；可在官方用量页查看。'};
 return {subscription,usage};
}
