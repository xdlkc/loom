// Only rendered official usage DOM. No cookies, storage, page JS state or network reads.
function collectGeminiQuota(doc,href){const u=new URL(href);if(u.origin!=='https://gemini.google.com'||!/^\/(?:u\/\d+\/)?usage\/?$/.test(u.pathname))return null;
 const root=doc.querySelector('.usage-metrics-container');if(!root)return null;
 const windowData=selector=>{const e=root.querySelector(selector);if(!e)return null;const t=e.innerText||'';const m=t.match(/(?:已使用\s*(\d+(?:\.\d+)?)\s*%|(\d+(?:\.\d+)?)\s*%\s*used)/i);if(!m)return null;const usedPercent=Number(m[1]||m[2]);if(!Number.isFinite(usedPercent)||usedPercent<0||usedPercent>100)return null;const r=t.match(/(?:重置时间[：:]|Resets?(?: at)?[:：]?)\s*([^\n]+)/i);return {usedPercent,resetText:r?.[1]?.trim()||null};};
 const current=windowData('.gxu-currently'),weekly=windowData('.gxu-weekly');if(!current||!weekly)return null;
 const account=[...doc.querySelectorAll('a[aria-label],button[aria-label]')].filter(e=>/Google (?:账号|帐号|Account)/i.test(e.getAttribute('aria-label')||'')).find(e=>{const r=e.getBoundingClientRect();return r.width>0&&r.height>0;});
 const email=(account?.getAttribute('aria-label')||'').match(/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/i)?.[0]?.toLowerCase();if(!email)return null;
 const plan=(root.innerText||'').match(/(?:^|\n)\s*(ULTRA|PRO|PLUS|FREE)\s*(?:\n|$)/i)?.[1]?.toUpperCase()||null;
 return {pageUrl:u.origin+u.pathname.replace(/\/$/,''),plan,current,weekly,accountEmail:email,observedAt:new Date().toISOString()};}
if(typeof module!=='undefined')module.exports={collectGeminiQuota};
