import test from 'node:test';import assert from 'node:assert/strict';
import {appendHistory} from '../history.mjs';import {buildSeries,chartPoints} from '../public/trend-data.mjs';
const now=Date.parse('2026-10-24T12:00:00Z'),at=n=>new Date(now+n*60000).toISOString();
const row=(id,t,value,extra={})=>({id,at:at(t),kind:'subscription',metrics:[{label:'剩余',value,unit:'%',total:100}],...extra});
test('trend series keeps connectors, currencies and products separate, deduplicates latest observation and preserves null/zero',()=>{
 const h=[row('a',-20,0),row('a',-10,null),row('b',-20,80),{...row('a',-10,12),metrics:[{label:'剩余',value:12,unit:'USD'}]}];
 const ss=buildSeries(h,{a:{...row('a',-20,0),observedAt:at(-20),source:'official'}},{a:{provider:'p'},b:{provider:'other'}},'p');assert.equal(ss.length,2);assert.deepEqual(ss.find(s=>s.unit==='%').points.map(p=>p.value),[0,null]);assert.equal(ss[1].unit,'USD');
});
test('curves break on replenishment, known reset, changed plan, cumulative rollover, missing values and long gaps',()=>{
 const s={kind:'subscription',points:[{at:now-60e3,value:20,total:100},{at:now,value:100,total:100}]};assert.equal(chartPoints(s,24,now)[1].breakBefore,true);
 for(const change of [{resetAt:at(-.5)},{plan:'old'},{value:null}]){const points=[{at:now-60e3,value:20,total:100,...change},{at:now,value:10,total:100,plan:'new'}];assert.equal(chartPoints({...s,points},24,now)[1].breakBefore,true);}
 assert.equal(chartPoints({kind:'usage',points:[{at:now-60e3,value:20},{at:now,value:10}]},24,now)[1].breakBefore,true);
 assert.equal(chartPoints({kind:'balance',points:[{at:now-60e3,value:20},{at:now,value:10}]},24,now)[1].breakBefore,false);
 assert.equal(chartPoints({...s,points:[{at:now-7200000,value:20},{at:now,value:19}]},24,now)[1].breakBefore,true);
 assert.equal(chartPoints(s,.001,now).length,1);
});
test('history retains independent per-connector quotas, removes old/duplicate samples and never stores credentials or account fields',()=>{
 const state={history:[row('old',-31*1440,2),row('b',-1,30)]};for(let i=0;i<5002;i++)appendHistory(state,{id:'a',observedAt:new Date(now-5002+i).toISOString(),kind:'balance',metrics:[],key:'never',accountFingerprint:'never'},now);
 assert.equal(state.history.filter(r=>r.id==='a').length,5000);assert.equal(state.history.filter(r=>r.id==='b').length,1);assert.equal(state.history.some(r=>r.id==='old'),false);assert.equal(JSON.stringify(state.history).includes('never'),false);
 appendHistory(state,{id:'a',observedAt:state.history.at(-1).at,kind:'balance',metrics:[]},now);assert.equal(state.history.filter(r=>r.id==='a').length,5000);
});
