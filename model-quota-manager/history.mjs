// Successful public quota observations only; no credentials or account identifiers.
export function appendHistory(state,record,now=Date.now()){
 const row={id:record.id,at:record.observedAt,kind:record.kind,metrics:record.metrics};
 for(const k of ['plan','periodStart','periodEnd'])if(record[k])row[k]=record[k];
 const cutoff=now-30*86400000,seen=new Set(),counts=new Map();
 state.history=[...(state.history||[]),row].filter(r=>Number.isFinite(Date.parse(r.at))&&Date.parse(r.at)>=cutoff).sort((a,b)=>Date.parse(b.at)-Date.parse(a.at)).filter(r=>{const key=r.id+'|'+r.at;if(seen.has(key))return false;seen.add(key);const n=(counts.get(r.id)||0)+1;counts.set(r.id,n);return n<=5000;}).reverse();
}
