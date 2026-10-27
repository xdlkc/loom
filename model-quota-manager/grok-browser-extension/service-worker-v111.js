// One synchronous listener owns message routing for both quota sources.
const quotaHandlers={};
importScripts('grok-background.js','gemini-background.js');
chrome.runtime.onMessage.addListener((m,sender,reply)=>{
 if(m?.type==='bridge-ping'){reply({ok:true,version:chrome.runtime.getManifest().version,geminiReady:typeof quotaHandlers.gemini==='function'});return false;}
 if(['connect','quota-page'].includes(m?.type))return quotaHandlers.grok(m,sender,reply);
 if(['bind-current','sync-now','gemini-quota'].includes(m?.type))return quotaHandlers.gemini(m,sender,reply);
});
