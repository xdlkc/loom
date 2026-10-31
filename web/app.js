'use strict';
const $=s=>document.querySelector(s), ns='http://www.w3.org/2000/svg';
const state={page:'overview',offset:0,sessionOffset:0,search:'',generation:0,groups:[],detail:null};
const names={overview:'调用概览',requests:'请求记录',sessions:'会话轨迹',settings:'采集设置'};
const num=n=>n==null?'—':new Intl.NumberFormat('zh-CN',{notation:n>999999?'compact':'standard',maximumFractionDigits:1}).format(n);
const pct=n=>n==null?'—':(n*100).toFixed(1)+'%';
const ms=n=>n==null?'—':n>=1000?(n/1000).toFixed(2)+' s':Math.round(n)+' ms';
const date=n=>n?new Date(n).toLocaleString('zh-CN',{hour12:false}):'—';
const el=(tag,cls,text)=>{const e=document.createElement(tag);if(cls)e.className=cls;if(text!=null)e.textContent=String(text);return e;};
const add=(parent,...children)=>{for(const c of children)if(c)parent.append(c);return parent;};
const btn=(label,fn)=>{const e=el('button','',label);e.addEventListener('click',fn);return e;};
const panel=(title)=>add(el('section','panel'),el('h2','',title));
const empty=(text)=>el('div','empty',text);
const tag=s=>el('span','pill '+(['failed','partial','cancelled','interrupted'].includes(s)?'bad':s==='inflight'?'pending':''),({success:'成功',failed:'失败',partial:'不完整',cancelled:'已取消',interrupted:'服务中断',inflight:'进行中'}[s]||s));
function filters(){const to=Date.now();return {from:to-Number($('#range').value)*3600000,to,provider:$('#provider').value,model:$('#model').value,harness:$('#harness').value};}
function qs(v){return new URLSearchParams(Object.entries(v).filter(([,v])=>v!==''&&v!=null)).toString();}
async function api(path,options={}){
 const response=await fetch(path,{credentials:'same-origin',...options,headers:{'Content-Type':'application/json',...(options.headers||{})}});
 if(response.status===401){$('#login').hidden=false;$('#filters').hidden=true;$('#content').replaceChildren();throw new Error('请运行 loom console 安全连接本机管控台。');}
 const data=await response.json();if(!response.ok)throw new Error(data.error||'请求失败');
 $('#login').hidden=true;return data;
}
function options(groups){
 const key=new Map(state.groups.map(x=>[x.provider+'|'+x.model+'|'+x.harness,x]));for(const g of groups)key.set(g.provider+'|'+g.model+'|'+g.harness,g);state.groups=[...key.values()];
 for(const [selector,field,title] of [['#provider','provider','全部供应商'],['#model','model','全部模型']]){
  const select=$(selector),current=select.value;select.replaceChildren(new Option(title,''));
  [...new Set(state.groups.map(x=>x[field]).filter(Boolean))].sort().forEach(v=>select.add(new Option(v,v)));
  if([...select.options].some(o=>o.value===current))select.value=current;
 }
}
function table(headers,rows){const t=el('table'),head=el('thead'),tr=el('tr');for(const h of headers)add(tr,el('th','',h));add(head,tr);const body=el('tbody');for(const row of rows){const tr=el('tr');for(const cell of row)add(tr,add(el('td'),cell instanceof Node?cell:document.createTextNode(String(cell??'—'))));add(body,tr);}return add(t,head,body);}
function pagination(offset,total,limit,onPage){const div=el('div','pagination');const prev=btn('上一页',()=>onPage(Math.max(0,offset-limit))),next=btn('下一页',()=>onPage(offset+limit));prev.disabled=offset===0;next.disabled=offset+limit>=total;return add(div,el('span','',`${num(total)} 条 · ${Math.floor(offset/limit)+1} 页`),prev,next);}
function card(title,value,note){return add(el('div','card'),el('small','',title),el('div','number',value),el('div','muted',note));}
function chart(series,bucket,f){
 if(!series.length||series.every(s=>s.input_tokens==null&&s.output_tokens==null))return empty('暂无已返回 usage 的调用。请求完成后自动显示 Token 曲线。');
 const map=new Map(series.map(s=>[s.timestamp,s])),points=[];for(let t=Math.floor(f.from/bucket)*bucket;t<f.to;t+=bucket)points.push(map.get(t)||{timestamp:t,input_tokens:0,output_tokens:0});
 const svg=document.createElementNS(ns,'svg');svg.setAttribute('viewBox','0 0 800 260');svg.setAttribute('role','img');svg.setAttribute('aria-label','输入与输出 Token 随时间的变化曲线');
 const node=(name,attrs,text)=>{const n=document.createElementNS(ns,name);for(const [k,v] of Object.entries(attrs))n.setAttribute(k,v);if(text)n.textContent=text;svg.append(n);return n;};
 const max=Math.max(1,...points.map(s=>Math.max(s.input_tokens||0,s.output_tokens||0)));
 for(let i=0;i<=4;i++){const y=20+i*49;node('line',{x1:58,x2:780,y1:y,y2:y,class:'chart-grid'});node('text',{x:4,y:y+4,class:'chart-label'},num(max*(1-i/4)));}
 const x=i=>58+i*720/Math.max(1,points.length-1),y=n=>216-(n||0)*196/max;
 for(const [field,cls] of [['input_tokens','chart-line'],['output_tokens','chart-line chart-output']]){
  node('path',{d:points.map((s,i)=>(i?'L':'M')+x(i).toFixed(1)+' '+y(s[field]).toFixed(1)).join(' '),class:cls});
  points.forEach((s,i)=>{const c=node('circle',{cx:x(i),cy:y(s[field]),r:3,fill:field==='input_tokens'?'#3b7c62':'#b6a274'});const title=document.createElementNS(ns,'title');title.textContent=`${date(s.timestamp)} · ${field==='input_tokens'?'输入':'输出'} ${num(s[field])} Tokens`;c.append(title);});
 }
 for(const i of [0,Math.floor((points.length-1)/2),points.length-1])node('text',{x:x(i),y:248,'text-anchor':i===0?'start':i===points.length-1?'end':'middle',class:'chart-label'},new Date(points[i].timestamp).toLocaleString('zh-CN',{month:'short',day:'numeric',hour:'2-digit'}));
 return svg;
}
async function overview(f){
 const v=await api('/api/overview?'+qs(f));options(v.groups);const s=v.summary,root=el('div');
 add(root,add(el('div','cards'),card('Token 用量',s.input_tokens==null&&s.output_tokens==null?'—':num((s.input_tokens||0)+(s.output_tokens||0)),`输入 ${num(s.input_tokens)} / 输出 ${num(s.output_tokens)} · usage ${num(s.usage_observed)}/${num(s.requests)} 次`),card('请求',num(s.requests),`成功 ${num(s.success)} · 异常 ${num(s.errors)}`),card('缓存命中率',pct(s.cache_hit_rate),`读 ${num(s.cache_read_tokens)} / 写 ${num(s.cache_write_tokens)} Tokens · 已观测 ${num(s.cache_observed)} 次`),card('首 Token 延迟',ms(s.avg_ttft_ms),`请求耗时 P50 ${ms(v.p50_duration_ms)} · P95 ${ms(v.p95_duration_ms)}`)));
 const charts=el('div','panels'),usage=panel('Token 消耗趋势'),health=panel('使用分布');add(usage,chart(v.series,v.bucket_ms,f),el('div','legend','● 输入 Tokens　 ● 输出 Tokens'));
 const provider=new Map;for(const g of v.groups)provider.set(g.provider||'未路由',(provider.get(g.provider||'未路由')||0)+(g.input_tokens||0)+(g.output_tokens||0));
 add(health,provider.size?table(['供应商','Tokens'],[...provider].map(([p,n])=>[p,num(n)])):empty('尚无网关调用'),el('p','muted','仅统计经过 Loom 的真实请求。缺失 usage 显示为未知；厂商余额、订阅积分和估算美元不混入此处。'));
 add(charts,usage,health);add(root,charts);
 const models=panel('模型与 Harness');add(models,v.groups.length?table(['模型','供应商 / Harness','输入 / 输出','缓存读 / 写','命中率','成功 / 请求','平均首 Token'],v.groups.map(g=>[btn(g.model||'未识别',()=>{$('#model').value=g.model||'';navigate('requests');}),`${g.provider||'未路由'} / ${g.harness}`,`${num(g.input_tokens)} / ${num(g.output_tokens)}`,`${num(g.cache_read_tokens)} / ${num(g.cache_write_tokens)}`,pct(g.cache_hit_rate),`${num(g.success)} / ${num(g.requests)}`,ms(g.avg_ttft_ms)])):empty('等待首次调用'));add(root,models);return root;
}
async function requests(f){
 const v=await api('/api/requests?'+qs({...f,offset:state.offset}));const root=panel('请求 · '+num(v.total));
 add(root,v.items.length?table(['时间','模型','供应商','Harness','状态','输入 / 输出','耗时','轨迹'],v.items.map(r=>[date(r.started),btn(r.model||r.id,()=>requestDetail(r.id)),r.provider||'未路由',r.harness,tag(r.status),`${num(r.input)} / ${num(r.output)}`,ms(r.duration_ms),r.session?btn('会话 →',()=>sessionDetail(r.session)):el('span','muted','未关联')])):empty('当前筛选下暂无请求'),pagination(state.offset,v.total,50,n=>{state.offset=n;refresh();}));return root;
}
function showDetail(title,content){$('#detail-title').textContent=title;$('#detail-content').replaceChildren(content);if(!$('#detail').open)$('#detail').showModal();}
function rawDetails(title,data,open=false){const d=el('details');d.open=open;return add(d,el('summary','',title),el('pre','',JSON.stringify(data,null,2)));}
async function requestDetail(id){
 try{const r=await api('/api/requests/'+encodeURIComponent(id)),root=el('div');
 add(root,add(el('div','kv'),el('span','','供应商 / 模型'),el('span','',`${r.provider||'—'} / ${r.model||'—'}`),el('span','','Harness / 协议'),el('span','',`${r.harness} / ${r.protocol}${r.streaming?' · SSE':''}`),el('span','','状态 / HTTP'),el('span','',`${r.status} / ${r.http}`),el('span','','时间 / 耗时'),el('span','',`${date(r.started)} / ${ms(r.duration_ms)}`),el('span','','输入 / 输出 Tokens'),el('span','',`${num(r.usage.input)} / ${num(r.usage.output)}`),el('span','','缓存读 / 写'),el('span','',`${num(r.usage.cache_read)} / ${num(r.usage.cache_write)}`),el('span','','首字节 / 首 Token'),el('span','',`${ms(r.first_byte_ms)} / ${ms(r.ttft_ms)}`)));
 if(r.session)add(root,btn('查看完整会话 →',()=>sessionDetail(r.session)));if(r.error)add(root,el('p','muted',r.error));
 if(r.truncated||r.parse_errors)add(root,el('p','muted',`观测内容${r.truncated?'超过保留上限，已标记截断':''} · 解析异常 ${r.parse_errors}，推理字节仍原样透传。`));
 const stages=el('div','stages');for(const s of r.spans)add(stages,el('span','',`${s.stage} · ${ms(s.ms)}`));add(root,stages,rawDetails('客户端 → Loom',r.request,true),rawDetails('Loom → 上游（凭据已排除）',r.forwarded),rawDetails('上游 → 客户端 · 完整响应 / SSE 事件',r.response));showDetail(r.id,root);
 }catch(e){report(e);}
}
async function sessions(){
 const v=await api('/api/sessions?'+qs({harness:$('#harness').value,q:state.search,offset:state.sessionOffset}));const root=panel('会话 · '+num(v.total)),bar=el('div','toolbar'),search=el('input');search.placeholder='搜索标题或 Session ID';search.value=state.search;search.setAttribute('aria-label','搜索会话');let timeout;search.oninput=()=>{clearTimeout(timeout);timeout=setTimeout(()=>{state.search=search.value;state.sessionOffset=0;refresh();},350);};
 add(bar,search,btn('立即导入',async()=>{try{await api('/api/import',{method:'POST'});refresh();}catch(e){report(e);}}));add(root,bar,v.items.length?table(['会话','Harness','最后更新','事件','网关请求','数据范围'],v.items.map(s=>[btn(s.title||s.native_id,()=>sessionDetail(s.id)),s.harness,date(s.updated),num(s.events),num(s.requests),s.coverage==='native'?'原生轨迹':s.coverage==='partial'?'原生轨迹（有解析异常）':'仅网关请求'])):empty('尚无已保存会话。三类 harness 的本机轨迹每 15 秒检查一次。'),pagination(state.sessionOffset,v.total,50,n=>{state.sessionOffset=n;refresh();}));return root;
}
function readable(v){
 const m=v.message||v;
 if(typeof m.content==='string')return m.content;
 if(typeof v.text==='string')return v.text;
 const parts=m.content||m.parts||v.parts;
 if(Array.isArray(parts))return parts.map(p=>{
  if(p.type==='text')return p.text||'';
  if(p.type==='thinking'||p.type==='reasoning')return '思考\n'+(p.thinking||p.text||'');
  if(p.type==='tool'&&p.state){const s=p.state;const output=Array.isArray(s.content)?s.content.map(x=>x.text||JSON.stringify(x)).join('\n'):typeof s.content==='string'?s.content:JSON.stringify(s.content??s.error??null,null,2);return '工具 '+(p.name||'')+' · '+(s.status||'')+'\n输入\n'+JSON.stringify(s.input??{},null,2)+'\n\n工具结果\n'+output;}
  if(['tool_use','toolCall','tool','tool-call'].includes(p.type))return '工具 '+(p.name||p.toolName||p.tool||'')+'\n'+JSON.stringify(p.input||p.arguments||p.state||p,null,2);
  if(['tool_result','toolResult','tool-result'].includes(p.type))return '工具结果\n'+JSON.stringify(p.content??p.output??p,null,2);
  return JSON.stringify(p,null,2);
 }).join('\n\n');
 return JSON.stringify(v,null,2);
}
async function sessionDetail(id,offset=0){
 try{const v=await api('/api/sessions/'+encodeURI(id)+'?'+qs({offset})),s=v.session,root=el('div');
 add(root,el('p','muted',`${s.harness} · ${s.native_id} · ${s.cwd||''}`));
 if(s.coverage==='gateway_only')add(root,el('p','muted','仅有经过网关的请求；客户端未保留原生 transcript 时，工具执行和其他本地事件不可恢复。'));if(s.coverage==='partial')add(root,el('p','muted','部分原生行解析失败，时间线保留了对应的字节位置与错误标记。'));
 if(s.parent)add(root,el('p','muted','父会话：'+s.parent));
 const linked=panel('关联的网关请求');add(linked,v.requests.length?table(['时间','模型','状态','输入 / 输出'],v.requests.map(r=>[date(r.started),btn(r.model||r.id,()=>requestDetail(r.id)),tag(r.status),`${num(r.input)} / ${num(r.output)}`])):el('p','muted','历史会话没有可匹配的网关请求 ID；不猜测归属，也不将导入 usage 重复加入全局统计。'));add(root,linked);
 const timeline=el('div','timeline');for(const e of v.events){
  const item=el('article','event'),role=e.payload.message?.role||e.kind;
  add(item,add(el('div','event-head'),tag(role),el('time','muted',date(e.timestamp)),el('span','muted',e.parent?'父事件 '+e.parent:'根事件')),el('div','event-body',readable(e.payload)),rawDetails('完整原生事件（已脱敏）',e.payload));add(timeline,item);
 }
 add(root,timeline,pagination(offset,v.total_events,100,n=>sessionDetail(id,n)));showDetail(s.title||s.native_id,root);
 }catch(e){report(e);}
}
async function settings(){
 const v=await api('/api/settings');$('#version').textContent='Loom '+v.version;const root=panel('采集与保留');root.classList.add('settings');
 const body=el('input');body.type='checkbox';body.checked=v.capture_bodies;
 const native=el('input');native.type='checkbox';native.checked=v.import_sessions;
 const days=el('input');days.type='number';days.min='1';days.max='3650';days.value=v.retention_days;days.setAttribute('aria-label','保留天数');
 add(root,add(el('div','setting-row'),add(el('div'),el('div','','保存完整请求 / 响应'),el('p','muted','仅落盘本机私有数据库，先移除已知凭据。关闭后只采集指标。')),body),add(el('div','setting-row'),add(el('div'),el('div','','自动导入原生会话'),el('p','muted','只读 pi、opencode2、Claude Code 的本机轨迹，每 15 秒检查。')),native),add(el('div','setting-row'),add(el('div'),el('div','','保留天数'),el('p','muted','到期清理本机观测副本，不删除 harness 原文件。')),days));
 const save=btn('保存设置',async()=>{try{await api('/api/settings',{method:'POST',body:JSON.stringify({capture_bodies:body.checked,import_sessions:native.checked,retention_days:Number(days.value)})});save.textContent='已保存';}catch(e){report(e);}});
 add(root,add(el('div','setting-row'),el('p','muted',`数据库 ${num(v.database_bytes/1048576)} MB · 最近导入 ${date(v.last_import?.checked_at)} · 失败来源 ${v.last_import?.failed_sources??0}`),save),btn('按保留策略立即清理',async()=>{if(!confirm('仅清理已到期的本机观测副本，继续？'))return;try{await api('/api/prune',{method:'POST'});refresh();}catch(e){report(e);}}),el('p','muted','旧会话中的未经过 Loom 的请求不会冒充已实测网关调用。缺失 Token、缓存或计费数据均显示未知。'));return root;
}
function report(e){$('#error').hidden=false;$('#error').textContent=e.message||'请求未完成';}
async function refresh(){const generation=++state.generation;$('#error').hidden=true;$('#filters').hidden=state.page==='settings';for(const id of ['range','provider','model'])$('#'+id).closest('label').hidden=state.page==='sessions';try{
 if(!state.groups.length&&state.page!=='settings'){const v=await api('/api/overview?'+qs(filters()));options(v.groups);}
 const content=await ({overview:()=>overview(filters()),requests:()=>requests(filters()),sessions,settings}[state.page]());
 if(generation===state.generation){$('#content').replaceChildren(content);$('#updated').textContent='更新于 '+new Date().toLocaleTimeString('zh-CN');}
 }catch(e){if(generation===state.generation)report(e);}}
function navigate(page){state.page=names[page]?page:'overview';state.offset=0;state.sessionOffset=0;$('#title').textContent=names[state.page];document.querySelectorAll('[data-page]').forEach(b=>b.classList.toggle('active',b.dataset.page===state.page));history.replaceState(null,'','#'+state.page);refresh();}
document.querySelectorAll('[data-page]').forEach(b=>b.onclick=()=>navigate(b.dataset.page));$('#refresh').onclick=refresh;$('#close-detail').onclick=()=>$('#detail').close();
for(const id of ['range','provider','model','harness'])$('#'+id).onchange=()=>{state.offset=0;state.sessionOffset=0;refresh();};
(async()=>{const params=new URLSearchParams(location.hash.slice(1)),ticket=params.get('ticket');if(ticket){history.replaceState(null,'',location.pathname);try{await api('/api/login',{method:'POST',body:JSON.stringify({ticket})});}catch(e){report(e);return;}}navigate(location.hash.slice(1)||'overview');})();
setInterval(()=>{if(!document.hidden&&!$('#detail').open&&!document.activeElement?.matches('input,select'))refresh();},15000);
