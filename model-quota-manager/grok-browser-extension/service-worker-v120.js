const quotaHandlers={};
importScripts('grok-background.js','gemini-background.js','minimax-background.js','token-background.js');
chrome.runtime.onMessage.addListener((m,sender,reply)=>{
 if(m?.type?.startsWith('token-'))return quotaHandlers.tokens(m,sender,reply);
 if(m?.type==='bridge-ping'){reply({ok:true,version:chrome.runtime.getManifest().version,tokensReady:typeof quotaHandlers.tokens==='function',geminiReady:typeof quotaHandlers.gemini==='function',minimaxReady:typeof quotaHandlers.minimax==='function'});return false;}
 if(['connect','quota-page'].includes(m?.type))return quotaHandlers.grok(m,sender,reply);
 if(['bind-current','sync-now','gemini-quota'].includes(m?.type))return quotaHandlers.gemini(m,sender,reply);
 if(['minimax-bind','minimax-sync','minimax-task','minimax-usage','minimax-page-failure'].includes(m?.type))return quotaHandlers.minimax(m,sender,reply);
});
