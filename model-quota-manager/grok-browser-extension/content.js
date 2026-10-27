let sent=false,last=null,stable=0;
const start=Date.now();
const timer=setInterval(()=>{
 if(sent){clearInterval(timer);return;}
 const elapsed=Date.now()-start;
 const snapshot=collectGrokQuota(document,location.href);
 if(!snapshot){if(elapsed>45000)clearInterval(timer);return;}
 const signature=JSON.stringify({...snapshot,observedAt:null});
 stable=signature===last?stable+1:0;last=signature;
 const complete=snapshot.plan&&snapshot.resetText&&snapshot.rangeText&&Number.isSafeInteger(snapshot.totalTokens)&&Number.isSafeInteger(snapshot.includedTokens);
 // Hydration can expose the meter before the reset label and token totals.
 // Wait for stable complete values, or a bounded partial result after 40 seconds.
 if(document.readyState!=='complete'||elapsed<8000||stable<3||(!complete&&elapsed<40000))return;
 sent=true;clearInterval(timer);chrome.runtime.sendMessage({type:'quota-page',snapshot}).catch(()=>{});
},1200);
