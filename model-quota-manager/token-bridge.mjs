import {grokExtensionId} from './grok-extension-id.mjs';
import {normalizeBrowserConsumption} from './browser-consumption.mjs';
export async function handleTokenBridge(req,res,{port,state,loadSnapshot,saveSnapshot,saveState,invalidate}){
 const json=(status,data)=>{res.writeHead(status,{'Content-Type':'application/json; charset=utf-8'});res.end(JSON.stringify(data));},origin='chrome-extension://'+grokExtensionId;
 if(![`127.0.0.1:${port}`,`localhost:${port}`].includes(req.headers.host)||req.headers.origin!==origin)return json(403,{code:'EXTENSION_REQUIRED'});
 res.setHeader('Access-Control-Allow-Origin',origin);res.setHeader('Vary','Origin');if(req.method==='OPTIONS'){res.setHeader('Access-Control-Allow-Methods','POST, OPTIONS');res.setHeader('Access-Control-Allow-Headers','Content-Type, X-Token-Bridge');res.writeHead(204);res.end();return;}
 if(req.method!=='POST'||req.headers['x-token-bridge']!=='1'||!req.headers['content-type']?.startsWith('application/json'))return json(403,{code:'EXTENSION_REQUIRED'});
 const match=req.url.match(/^\/api\/token-browser\/(grok|deepseek)\/(control|connect|snapshot|failure)$/);if(!match)return json(404,{code:'NOT_FOUND'});const [,provider,action]=match;
 try{let raw='',size=0;for await(const c of req){size+=c.length;if(size>1048576)throw Error();raw+=c;}const body=JSON.parse(raw||'{}');state.settings.tokenBrowser??={};const status=state.settings.tokenBrowser[provider]??={enabled:false};
 if(action==='control')return json(200,{enabled:!!status.enabled,requested:!!status.requestedAt,bind:!!status.bind});
 if(action==='connect'){status.enabled=true;status.bind=true;status.requestedAt=Date.now();saveState();return json(200,{ok:true});}
 if(!status.enabled)return json(409,{code:'NOT_CONNECTED'});
 if(action==='failure'){if(!['PAGE','ACCOUNT_CHANGED','TIMEOUT'].includes(body.reason))throw Error();status.error=body.reason;if(!(body.stage==='validation'&&status.stage?.startsWith('validate-')))status.stage=/^[a-z-]{1,30}$/.test(body.stage||'')?body.stage:null;status.requestedAt=0;if(body.reason==='ACCOUNT_CHANGED')status.enabled=false;saveState();invalidate(provider);return json(200,{ok:true});}
 const snapshot=normalizeBrowserConsumption(provider,body),old=loadSnapshot(provider);if(!status.bind&&(!old||snapshot.accountFingerprint!==old.accountFingerprint)){status.enabled=false;status.error='ACCOUNT_CHANGED';status.requestedAt=0;saveState();invalidate(provider);return json(409,{code:'ACCOUNT_CHANGED'});}
 saveSnapshot(provider,snapshot);status.enabled=true;status.bind=false;status.error=null;status.stage=null;status.requestedAt=0;saveState();invalidate(provider);return json(200,{ok:true});
 }catch(e){if(action==='snapshot'&&/^validate-[a-z-]{1,20}$/.test(e.stage||'')){const status=state.settings.tokenBrowser[provider];status.error='PAGE';status.stage=e.stage;status.requestedAt=0;saveState();invalidate(provider);}return json(400,{code:'TOKEN_PAGE'});}
}
