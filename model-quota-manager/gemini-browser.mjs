export function normalizeGeminiSnapshot(body,now=Date.now()){
 if(!body||typeof body!=='object'||typeof body.pageUrl!=='string'||!/^https:\/\/gemini\.google\.com\/(?:u\/\d+\/)?usage$/.test(body.pageUrl)||!/^([a-f0-9]{64})$/.test(body.accountFingerprint||''))throw Error('GEMINI_PAGE');
 const time=Date.parse(body.observedAt);if(!Number.isFinite(time)||Math.abs(time-now)>300000)throw Error('GEMINI_PAGE');
 const metric=(q,label)=>{if(!q||typeof q.usedPercent!=='number'||!Number.isFinite(q.usedPercent)||q.usedPercent<0||q.usedPercent>100)throw Error('GEMINI_PAGE');const round=n=>Math.round(n*10000)/10000;const resetText=typeof q.resetText==='string'&&/^[0-9年月日:： /.,APMapm-]{1,48}$/.test(q.resetText)?q.resetText:null;return {label,value:round(100-q.usedPercent),unit:'%',total:100,used:round(q.usedPercent),resetAt:null,resetText};};
 const plans={ULTRA:'Google AI Ultra',PRO:'Google AI Pro',PLUS:'Google AI Plus',FREE:'免费方案'};
 return {kind:'subscription',plan:'Gemini 网页 · '+(plans[body.plan]||'未显示方案'),source:'browser',sourceUrl:body.pageUrl,observedAt:new Date(time).toISOString(),metrics:[metric(body.current,'当前窗口剩余'),metric(body.weekly,'周窗口剩余')],note:'来源：已绑定目标账号的 Gemini 官方用量页，按网页显示的已用百分比计算剩余；百分比可能已四舍五入，不代表剩余 Token 或固定消息次数。重置文字按页面原样保存，不补造年份、时区或完整时间。账号指纹不匹配时暂停同步并保留旧值；不读取 Gemini API、CLI 或 Antigravity 额度。'};
}
