(()=>{
const BASE='http://127.0.0.1:4318';
async function api(path,data){const r=await fetch(BASE+'/api/grok-browser/'+path,{method:data===undefined?'GET':'POST',headers:data===undefined?{}:{'Content-Type':'application/json','X-Grok-Bridge':'1'},body:data===undefined?undefined:JSON.stringify(data)});if(!r.ok)throw Error('LOCAL');return r.json();}
async function expire(){const {pendingTab,deadline}=await chrome.storage.local.get(['pendingTab','deadline']);if(!pendingTab||Date.now()<deadline)return;await chrome.storage.local.remove(['pendingTab','deadline']);await chrome.tabs.remove(pendingTab).catch(()=>{});await api('failure',{reason:'LOGIN_OR_PAGE'}).catch(()=>{});}
async function run(){await expire();const {pendingTab}=await chrome.storage.local.get('pendingTab');if(pendingTab)return;try{const tab=await chrome.tabs.create({url:'https://grok.com/dashboard/usage',active:false});await chrome.storage.local.set({pendingTab:tab.id,deadline:Date.now()+60000});await chrome.alarms.create('grok-timeout',{when:Date.now()+60000});}catch{await chrome.storage.local.set({lastStatus:'本机工具未连接'});}}
chrome.runtime.onInstalled.addListener(()=>chrome.alarms.create('grok-control',{periodInMinutes:1}));
chrome.runtime.onStartup.addListener(()=>chrome.alarms.create('grok-control',{periodInMinutes:1}));
chrome.alarms.onAlarm.addListener(async a=>{if(a.name==='grok-timeout'){await expire();return;}if(a.name!=='grok-control')return;await expire();const {enabled}=await chrome.storage.local.get('enabled');if(!enabled)return;try{const control=await api('control',{});if(control.enabled&&control.requested)await run();}catch{}});
const messageListener=(message,sender,reply)=>{
 if(message.type==='connect'){
  (async()=>{try{await api('connect',{});await chrome.storage.local.set({enabled:true});await chrome.alarms.create('grok-control',{periodInMinutes:1});await run();reply({ok:true});}catch{reply({ok:false});}})();return true;
 }
 if(message.type==='quota-page'&&sender.url?.startsWith('https://grok.com/dashboard/')){
  (async()=>{const {pendingTab}=await chrome.storage.local.get('pendingTab');const id=sender.tab?.id;if(!id||id!==pendingTab){reply({ok:false});return;}await chrome.storage.local.remove(['pendingTab','deadline']);try{await api('snapshot',message.snapshot);await chrome.storage.local.set({lastStatus:'已同步',lastSync:new Date().toISOString()});}catch{await api('failure',{reason:'PAGE'}).catch(()=>{});}finally{await chrome.tabs.remove(id).catch(()=>{});}reply({ok:true});})();return true;
 }
};
if(typeof quotaHandlers!=='undefined')quotaHandlers.grok=messageListener;else chrome.runtime.onMessage.addListener(messageListener);

})();
