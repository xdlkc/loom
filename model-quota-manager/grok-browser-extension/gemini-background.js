(()=>{
const store={
 async get(keys){const names=Array.isArray(keys)?keys:[keys];const data=await chrome.storage.local.get(names.map(k=>'gemini:'+k));return Object.fromEntries(names.map(k=>[k,data['gemini:'+k]]));},
 set(values){return chrome.storage.local.set(Object.fromEntries(Object.entries(values).map(([k,v])=>['gemini:'+k,v])));},
 remove(keys){return chrome.storage.local.remove((Array.isArray(keys)?keys:[keys]).map(k=>'gemini:'+k));}
};
const BASE='http://127.0.0.1:4318/api/gemini-browser/';
async function api(route,data={}){const r=await fetch(BASE+route,{method:'POST',headers:{'Content-Type':'application/json','X-Gemini-Bridge':'1'},body:JSON.stringify(data)});const d=await r.json();if(!r.ok)throw Error(d.code||'LOCAL');return d;}
async function fail(reason){await api('failure',{reason}).catch(()=>{});await store.set({lastStatus:reason==='ACCOUNT_CHANGED'?'账号变化，已暂停，请重新绑定目标用量页':'未能读取官方用量页，保留上次数据'});}
async function expire(){const s=await store.get(['pendingTab','deadline']);if(!s.pendingTab||Date.now()<s.deadline)return;await store.remove(['pendingTab','deadline']);await chrome.tabs.remove(s.pendingTab).catch(()=>{});await fail('LOGIN_OR_PAGE');}
async function run(){await expire();const s=await store.get(['pendingTab','targetUrl','enabled']);if(s.pendingTab||!s.enabled||!s.targetUrl)return;const tab=await chrome.tabs.create({url:s.targetUrl,active:false});await store.set({pendingTab:tab.id,deadline:Date.now()+75000});await chrome.alarms.create('gemini-timeout',{when:Date.now()+75000});}
const alarm=()=>chrome.alarms.create('gemini-control',{periodInMinutes:1});chrome.runtime.onInstalled.addListener(alarm);chrome.runtime.onStartup.addListener(alarm);
chrome.alarms.onAlarm.addListener(async a=>{if(a.name==='gemini-timeout'){await expire();return;}if(a.name!=='gemini-control')return;await expire();const s=await store.get('enabled');if(!s.enabled)return;try{const c=await api('control');if(c.enabled&&c.requested)await run();}catch{}});
const messageListener=(m,sender,reply)=>{
 if(m.type==='bind-current'){
  (async()=>{try{const previous=await store.get('pendingTab');if(previous.pendingTab){await store.remove(['pendingTab','deadline']);await chrome.tabs.remove(previous.pendingTab).catch(()=>{});}const [tab]=await chrome.tabs.query({active:true,lastFocusedWindow:true});if(!tab?.url||!/^https:\/\/gemini\.google\.com\/(?:u\/\d+\/)?usage\/?(?:\?.*)?$/.test(tab.url))throw Error('OPEN_USAGE');let d;try{d=await chrome.tabs.sendMessage(tab.id,{type:'gemini-capture'});}catch{throw Error('RELOAD');}if(!d?.snapshot)throw Error('PAGE');await api('bind',d.snapshot);await store.set({enabled:true,targetUrl:d.snapshot.pageUrl,expectedFingerprint:d.snapshot.accountFingerprint,lastStatus:'已绑定目标账号并同步',lastSync:new Date().toISOString()});await alarm();reply({ok:true});}catch(e){reply({ok:false,code:['OPEN_USAGE','RELOAD','PAGE','GEMINI_PAGE','EXTENSION_REQUIRED','ACCOUNT_CHANGED'].includes(e.message)?e.message:'LOCAL'});}})();return true;
 }
 if(m.type==='sync-now'){(async()=>{try{const s=await store.get('enabled');if(!s.enabled)throw Error();await run();reply({ok:true});}catch{reply({ok:false});}})();return true;}
 if(m.type==='gemini-quota'&&sender.url?.startsWith('https://gemini.google.com/')){
  (async()=>{const s=await store.get(['pendingTab','expectedFingerprint']);if(sender.tab?.id!==s.pendingTab){reply({ok:false});return;}const id=s.pendingTab;await store.remove(['pendingTab','deadline']);try{if(m.snapshot.accountFingerprint!==s.expectedFingerprint)throw Error('ACCOUNT_CHANGED');await api('snapshot',m.snapshot);await store.set({lastStatus:'已同步目标账号',lastSync:new Date().toISOString()});}catch(e){if(e.message==='ACCOUNT_CHANGED')await store.set({enabled:false});await fail(e.message==='ACCOUNT_CHANGED'?'ACCOUNT_CHANGED':'PAGE');}finally{await chrome.tabs.remove(id).catch(()=>{});}reply({ok:true});})();return true;
 }
};
if(typeof quotaHandlers!=='undefined')quotaHandlers.gemini=messageListener;else chrome.runtime.onMessage.addListener(messageListener);

})();
