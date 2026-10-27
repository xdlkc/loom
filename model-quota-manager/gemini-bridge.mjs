import {appendHistory} from './history.mjs';
import {geminiExtensionId} from './gemini-extension-id.mjs';import {normalizeGeminiSnapshot} from './gemini-browser.mjs';
export async function handleGeminiBridge(req,res,{state,port,saveState,diagnostics={}}){
 const json=(status,data)=>{diagnostics.lastStatus=status;diagnostics.code=data.code||'OK';res.writeHead(status,{'Content-Type':'application/json; charset=utf-8'});res.end(JSON.stringify(data));};
 const origin='chrome-extension://'+geminiExtensionId;diagnostics.lastRequestAt=new Date().toISOString();diagnostics.originAccepted=req.headers.origin===origin;const actionName=req.url.slice('/api/gemini-browser/'.length);diagnostics.action=['control','bind','snapshot','failure'].includes(actionName)?actionName:'other';
 if(![`127.0.0.1:${port}`,`localhost:${port}`].includes(req.headers.host)||req.headers.origin!==origin)return json(403,{code:'EXTENSION_REQUIRED'});
 res.setHeader('Access-Control-Allow-Origin',origin);res.setHeader('Vary','Origin');
 if(req.method==='OPTIONS'){res.setHeader('Access-Control-Allow-Methods','POST, OPTIONS');res.setHeader('Access-Control-Allow-Headers','Content-Type, X-Gemini-Bridge');res.writeHead(204);res.end();return;}
 if(req.method!=='POST'||req.headers['x-gemini-bridge']!=='1'||!req.headers['content-type']?.startsWith('application/json'))return json(403,{code:'EXTENSION_REQUIRED'});
 const action=req.url.slice('/api/gemini-browser/'.length);try{let raw='',size=0;for await(const c of req){size+=c.length;if(size>8192)throw Error('GEMINI_PAGE');raw+=c;}const body=JSON.parse(raw||'{}');
  if(action==='control')return json(200,{enabled:!!state.settings.geminiBrowser,requested:!!state.settings.geminiBrowserRequestedAt});
  if(action==='failure'){if(!['ACCOUNT_CHANGED','PAGE','LOGIN_OR_PAGE'].includes(body.reason))throw Error('GEMINI_PAGE');state.attempts['gemini-web-quota']={at:new Date().toISOString(),status:'error',message:body.reason==='ACCOUNT_CHANGED'?'Gemini 网页账号发生变化，已暂停同步并保留旧值；请在目标账号用量页重新绑定。':'未能读取 Gemini 官方用量页；请确认 Chrome 运行且目标账号仍已登录。保留上次成功数据。'};if(body.reason==='ACCOUNT_CHANGED')state.settings.geminiBrowser=false;state.settings.geminiBrowserRequestedAt=0;saveState();return json(200,{ok:true});}
  if(!['bind','snapshot'].includes(action))return json(404,{code:'NOT_FOUND'});
  const record=normalizeGeminiSnapshot(body);
  if(action==='snapshot'&&(!state.settings.geminiBrowser||body.accountFingerprint!==state.settings.geminiBrowserFingerprint||body.pageUrl!==state.settings.geminiBrowserUrl)){state.attempts['gemini-web-quota']={at:new Date().toISOString(),status:'error',message:'Gemini 账号或用量页不匹配，已暂停同步并保留旧值；请重新绑定目标账号。'};state.settings.geminiBrowser=false;state.settings.geminiBrowserRequestedAt=0;saveState();return json(409,{code:'ACCOUNT_CHANGED'});}
  if(action==='bind'){if(state.settings.geminiBrowserFingerprint&&state.settings.geminiBrowserFingerprint!==body.accountFingerprint)state.history=(state.history||[]).filter(r=>r.id!=='gemini-web-quota');state.settings.geminiBrowser=true;state.settings.geminiBrowserFingerprint=body.accountFingerprint;state.settings.geminiBrowserUrl=body.pageUrl;}
  const id='gemini-web-quota';state.observations[id]={...record,id,provider:'gemini'};state.attempts[id]={at:record.observedAt,status:'ok'};appendHistory(state,state.observations[id]);state.settings.geminiBrowserRequestedAt=0;saveState();return json(200,{ok:true});
 }catch{return json(400,{code:'GEMINI_PAGE'});}
}
