// Reads rendered quota DOM only. No cookies, session stores, page JS state or network access.
function collectGrokQuota(doc,href){
 const url=new URL(href);if(url.origin!=='https://grok.com'||!['/dashboard','/dashboard/','/dashboard/usage'].includes(url.pathname))return null;
 const meter=doc.querySelector('[aria-label="Weekly included usage"][aria-valuenow]');
 if(!meter)return null;
 const used=Number(meter.getAttribute('aria-valuenow'));if(!Number.isFinite(used)||used<0||used>100||Number(meter.getAttribute('aria-valuemax'))!==100)return null;
 let resetText=null,node=meter;for(let i=0;node&&i<9;i++,node=node.parentElement){const t=node.innerText||'';const match=t.match(/Resets\s+([A-Za-z]{3,9}\s+\d{1,2}(?:,\s+\d{4})?)/);if(match){resetText=match[1];break;}}
 const planNames=/^(?:Super|SuperGrok(?: Heavy)?|Heavy|Premium\+?|Pro|Ultra|Free|Basic|X Premium\+?)$/;
 const plan=[...doc.querySelectorAll('h2,button span')].map(e=>e.innerText?.trim()).find(t=>planNames.test(t||''))||null;
 const readTokens=label=>{const leaf=[...doc.querySelectorAll('span,div')].find(e=>e.children.length===0&&(e.textContent||e.innerText)?.trim()===label);const matches=[...(leaf?.parentElement?.textContent||leaf?.parentElement?.innerText||'').matchAll(/(\d[\d,]*)\s+tokens/g)];return matches.length?Number(matches.at(-1)[1].replaceAll(',','')):null;};
 const range=[...doc.querySelectorAll('button')].map(e=>e.innerText?.trim()).find(t=>/^Date range\s+/.test(t||''))?.replace(/^Date range\s+/,'')||null;
 return {pageUrl:url.origin+url.pathname,plan,usedPercent:used,resetText,rangeText:range,totalTokens:readTokens('Total'),includedTokens:readTokens('Included'),observedAt:new Date().toISOString()};
}
if(typeof module!=='undefined')module.exports={collectGrokQuota};
