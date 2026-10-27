import http from 'node:http';import test from 'node:test';import assert from 'node:assert/strict';import fs from 'node:fs';import os from 'node:os';import path from 'node:path';
const dir=fs.mkdtempSync(path.join(os.tmpdir(),'quota-test-'));process.env.QUOTA_DATA_DIR=dir;
const {createApp,validateManual}=await import('../server.mjs');
const {save}=await import('../storage.mjs');
const sample={provider:'chatgpt',kind:'subscription',plan:'Plus 网页',label:'剩余次数',value:'0',total:'80',used:'80',unit:'次',note:'手动核对'};
test('validation distinguishes blank from zero and prevents invalid records',()=>{assert.equal(validateManual(sample).metrics[0].value,0);assert.equal(validateManual({...sample,value:''}).metrics[0].value,null);for(const edit of [{value:'abc'},{total:'-1'},{value:'81'},{provider:'bad'},{plan:'sk-abcdefghijklmnopqrst'},{resetAt:'oops'}])assert.throws(()=>validateManual({...sample,...edit}));});
test('server local-only security, persistence, CRUD, safe export and secret isolation',async t=>{
 save('credentials',{zenmux:{key:'fixture-credential-super-secret'},xai:{key:'fixture-xai-key',teamId:'private-team'}});const {server}=createApp();await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const base='http://127.0.0.1:'+server.address().port;
 t.after(()=>new Promise(resolve=>server.close(resolve)));
 const get=async p=>fetch(base+p);const session=await (await get('/api/session')).json();const post=(p,data,headers={})=>fetch(base+p,{method:'POST',headers:{'Content-Type':'application/json',Origin:base,'X-CSRF-Token':session.csrf,...headers},body:JSON.stringify(data)});
 const icon=await get('/icons/gemini.svg');assert.equal(icon.status,200);assert.equal(icon.headers.get('content-type'),'image/svg+xml');assert.match(await icon.text(),/<svg/);assert.equal((await get('/icons/unknown.svg')).status,404);
 assert.equal((await get('/')).status,200);assert.match((await get('/')).headers.get('content-security-policy'),/frame-ancestors 'none'/);
 const badHost=await new Promise(resolve=>{http.get(base+'/api/state',{headers:{Host:'evil.example'}},r=>{r.resume();resolve(r.statusCode);});});assert.equal(badHost,403);
 assert.equal((await fetch(base+'/api/state',{headers:{Origin:'https://evil.example'}})).status,403);
 const alias='http://quota.localhost:'+server.address().port;const hostStatus=h=>new Promise(resolve=>http.get(base+'/',{headers:{Host:h}},r=>{r.resume();resolve(r.statusCode);}));assert.equal(await hostStatus('quota.localhost:'+server.address().port),200);assert.equal((await post('/api/settings',{autoRefresh:false,proxy:''},{Host:'quota.localhost:'+server.address().port,Origin:alias})).status,200);assert.equal(await hostStatus('quota.localhost.evil:'+server.address().port),403);
 assert.equal((await post('/api/manual',sample,{'X-CSRF-Token':'bad'})).status,403);
 assert.equal((await post('/api/manual',sample,{Origin:'https://evil.example'})).status,403);
 assert.equal((await get('/.local/credentials.json')).status,404);assert.equal((await get('/api/history?provider=invalid')).status,400);assert.equal((await get('/api/history?provider=deepseek')).status,200);assert.equal((await fetch(base+'/api/history?provider=deepseek',{headers:{Origin:'https://evil.example'}})).status,403);
 assert.equal((await post('/api/manual',{...sample,note:'fixture-credential-super-secret'})).status,400);
 assert.equal((await post('/api/manual',sample)).status,200);
 let d=await (await get('/api/state')).json();assert.equal(d.manual.length,1);const id=d.manual[0].id;assert.equal(d.configured['zenmux-balance'],true);
 assert.equal((await post('/api/manual',{...sample,id,value:4,used:76})).status,200);d=await (await get('/api/state')).json();assert.equal(d.manual.length,1);assert.equal(d.manual[0].metrics[0].value,4);
 assert.equal((await post('/api/settings',{autoRefresh:false,proxy:'http://evil.example:123'})).status,400);assert.equal((await post('/api/settings',{autoRefresh:false,proxy:''})).status,200);
 const exported=await (await get('/api/export')).text();assert.ok(!exported.includes('fixture-credential-super-secret'));assert.ok(!exported.includes('private-team'));assert.ok(!exported.includes('fixture-xai-key'));
 assert.equal(fs.statSync(path.join(dir,'credentials.json')).mode&0o777,0o600);assert.equal(fs.statSync(dir).mode&0o777,0o700);
 assert.equal((await post('/api/manual/delete',{id})).status,200);assert.equal((await (await get('/api/state')).json()).manual.length,0);
 const app2=createApp();assert.equal(app2.view().settings.autoRefresh,false);app2.server.emit('close');
});
test.after(()=>fs.rmSync(dir,{recursive:true,force:true}));
test('refresh failure preserves last success and secret echo is never stored',async()=>{
 const old={id:'zenmux-balance',provider:'zenmux',source:'official',kind:'balance',observedAt:'2026-10-01T00:00:00Z',plan:'PAYG',metrics:[{label:'可用余额',value:12,unit:'USD'}]};
 save('credentials',{zenmux:{key:'fixture-secret-for-refresh'}});save('state',{manual:[],observations:{'zenmux-balance':old},attempts:{},history:[],settings:{autoRefresh:false,proxy:''}});
 const app=createApp({queryFn:async()=>{throw new Error('upstream fixture-secret-for-refresh');}});await app.refresh('zenmux-balance');assert.equal(app.view().observations['zenmux-balance'].observedAt,old.observedAt);assert.equal(app.view().observations['zenmux-balance'].metrics[0].value,12);assert.equal(app.view().attempts['zenmux-balance'].status,'error');assert.ok(!JSON.stringify(app.view()).includes('fixture-secret-for-refresh'));app.server.emit('close');
 save('state',{manual:[],observations:{},attempts:{},history:[],settings:{autoRefresh:false,proxy:''}});const echo=createApp({queryFn:async()=>({kind:'subscription',plan:'fixture-secret-for-refresh',metrics:[]})});await echo.refresh('zenmux-subscription');assert.equal(echo.view().observations['zenmux-subscription'],undefined);assert.ok(!fs.readFileSync(path.join(dir,'state.json'),'utf8').includes('fixture-secret-for-refresh'));echo.server.emit('close');
});

test('consumption route is same-origin, scoped, cached and never publishes an echoed key',async t=>{
 save('credentials',{zenmux:{key:'fixture-consumption-secret'}});let calls=0;const app=createApp({consumptionFn:async(provider,range)=>{calls++;if(provider==='glm')return {model:'fixture-consumption-secret'};return {available:true,provider,range,series:[]};}});await new Promise(r=>app.server.listen(0,'127.0.0.1',r));t.after(()=>new Promise(r=>app.server.close(r)));const base='http://127.0.0.1:'+app.server.address().port;
 assert.equal((await fetch(base+'/api/consumption?provider=zenmux&range=7d',{headers:{Origin:'https://evil.example'}})).status,403);assert.equal(calls,0);assert.equal((await fetch(base+'/api/consumption?provider=invalid')).status,400);assert.equal((await fetch(base+'/api/consumption?provider=zenmux&range=invalid')).status,400);
 const a=await (await fetch(base+'/api/consumption?provider=zenmux&range=7d')).json();assert.equal(a.provider,'zenmux');await fetch(base+'/api/consumption?provider=zenmux&range=7d');assert.equal(calls,1);
 const leaked=await fetch(base+'/api/consumption?provider=glm&range=7d');assert.equal(leaked.status,502);assert.ok(!(await leaked.text()).includes('fixture-consumption-secret'));
});

test('background scheduler refreshes configured services and token buckets at startup without a browser click',async t=>{save('credentials',{glm:{key:'fixture-background-key'}});save('state',{manual:[],observations:{},attempts:{},history:[],settings:{autoRefresh:true,minimaxBrowser:true,tokenBrowser:{grok:{enabled:true},deepseek:{enabled:false,error:'ACCOUNT_CHANGED'}}}});const calls=[];const app=createApp({startScheduler:true,queryFn:async id=>{calls.push(id);return {kind:'subscription',plan:'fixture',metrics:[{label:'剩余',value:50,unit:'%',total:100}]};},consumptionFn:async(p,r)=>{calls.push(p+'|'+r);return {available:true,series:[]};}});await new Promise(r=>app.server.listen(0,'127.0.0.1',r));t.after(()=>new Promise(r=>app.server.close(r)));for(let i=0;i<50&&!app.view().backgroundSync.lastCompletedAt;i++)await new Promise(r=>setTimeout(r,10));const v=app.view();assert.ok(calls.includes('glm-quota'));for(const r of ['today','7d','30d'])assert.ok(calls.includes('glm|'+r));assert.ok(v.settings.tokenBrowser.grok.requestedAt);assert.equal(v.settings.tokenBrowser.deepseek.enabled,false);assert.ok(!v.settings.tokenBrowser.deepseek.requestedAt);assert.ok(v.settings.minimaxBrowserRequestedAt);assert.equal(v.backgroundSync.running,false);assert.ok(v.backgroundSync.lastCompletedAt);assert.ok(!JSON.stringify(v).includes('fixture-background-key'));});
