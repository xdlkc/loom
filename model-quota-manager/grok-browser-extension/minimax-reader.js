// Rendered DOM only; account identifier is hashed before any message is sent.
function collectMiniMaxConsumption(doc,href){
 if(href.split('?')[0]!=='https://platform.minimax.cn/console/usage')return null;const start=doc.querySelector('input[date-range="start"]')?.value,end=doc.querySelector('input[date-range="end"]')?.value,next=doc.querySelector('[title="下一页"]');if(!start||!end||!next)return null;
 const rows=[...doc.querySelectorAll('tbody tr')].map(r=>[...r.querySelectorAll('td')].map(e=>e.innerText.trim())).filter(c=>c.length===10&&/^\d{4}\//.test(c[0]));if(!rows.length)return null;
 return {pageUrl:'https://platform.minimax.cn/console/usage',rangeStart:start,rangeEnd:end,complete:next.getAttribute('aria-disabled')==='true',observedAt:new Date().toISOString(),rows:rows.map(c=>({time:c[0],model:c[1],type:c[2],inputTokens:c[4],outputTokens:c[5],cacheReadTokens:c[6],cacheWriteTokens:c[7]}))};
}
if(typeof module!=='undefined')module.exports={collectMiniMaxConsumption};
