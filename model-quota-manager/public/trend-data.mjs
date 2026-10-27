export function buildSeries(history,observations,connectors,provider){
 const rows=[...(history||[]),...Object.values(observations||{}).filter(r=>r.source!=='manual').map(r=>({...r,at:r.observedAt}))].filter(r=>connectors[r.id]?.provider===provider).sort((a,b)=>Date.parse(a.at)-Date.parse(b.at));
 const series=new Map();
 for(const r of rows){if(!Number.isFinite(Date.parse(r.at)))continue;for(const m of r.metrics||[]){const key=JSON.stringify([r.id,r.kind,m.label,m.unit]);if(!series.has(key))series.set(key,{key,id:r.id,kind:r.kind,label:m.label,unit:m.unit,points:[]});const points=series.get(key).points;const point={at:Date.parse(r.at),value:typeof m.value==='number'&&Number.isFinite(m.value)?m.value:null,total:m.total,resetAt:m.resetAt,period:r.periodStart,plan:r.plan};if(points.at(-1)?.at===point.at)points[points.length-1]=point;else points.push(point);}}
 return [...series.values()];
}
export function chartPoints(series,hours,now=Date.now()){
 const points=series.points.filter(p=>p.at>=now-hours*3600000&&p.at<=now),subscription=series.kind==='subscription';
 return points.map((p,i)=>{const prev=points[i-1];let reason='';if(prev){if(p.value===null||prev.value===null)reason='缺失值';else if(p.at-prev.at>3600000)reason='采样间隔超过一小时';else if(p.plan&&prev.plan&&p.plan!==prev.plan)reason='套餐变化';else if(p.period&&prev.period&&p.period!==prev.period)reason='统计周期变化';else if(subscription&&(p.value>prev.value||p.total!==prev.total))reason='额度回升或窗口总量变化';else if(subscription&&prev.resetAt&&Date.parse(prev.resetAt)>prev.at&&Date.parse(prev.resetAt)<=p.at)reason='已跨过重置时间';else if(!subscription&&series.kind!=='balance'&&p.value<prev.value)reason='累计值回落';}return {...p,breakBefore:!!reason,reason};});
}
