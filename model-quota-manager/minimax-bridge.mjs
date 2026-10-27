import {grokExtensionId} from './grok-extension-id.mjs';
import {normalizeMiniMaxConsumption} from './minimax-consumption.mjs';
export async function handleMiniMaxBridge(req,res,{state,port,loadSnapshot,saveSnapshot,saveState,invalidate}){
 const json=(status,data)=>{res.writeHead(status,{'Content-Type':'application/json; charset=utf-8'});res.end(JSON.stringify(data));},origin='chrome-extension://'+grokExtensionId;
 if(![`127.0.0.1:${port}`,`localhost:${port}`].includes(req.headers.host)||req.headers.origin!==origin)return json(403,{code:'EXTENSION_REQUIRED'});
 res.setHeader('Access-Control-Allow-Origin',origin);res.setHeader('Vary','Origin');if(req.method==='OPTIONS'){res.setHeader('Access-Control-Allow-Methods','POST, OPTIONS');res.setHeader('Access-Control-Allow-Headers','Content-Type, X-MiniMax-Bridge');res.writeHead(204);res.end();return;}
 if(req.method!=='POST'||req.headers['x-minimax-bridge']!=='1'||!req.headers['content-type']?.startsWith('application/json'))return json(403,{code:'EXTENSION_REQUIRED'});
 try{let raw='',size=0;for await(const c of req){size+=c.length;if(size>1048576)throw Error('PAGE');raw+=c;}const body=JSON.parse(raw||'{}'),action=req.url.slice('/api/minimax-browser/'.length);
 if(action==='control')return json(200,{enabled:!!state.settings.minimaxBrowser,requested:!!state.settings.minimaxBrowserRequestedAt});
 if(action==='failure'){if(!['ACCOUNT_CHANGED','PAGE','LOGIN_OR_PAGE'].includes(body.reason))throw Error('PAGE');state.settings.minimaxBrowserError=body.reason;state.settings.minimaxBrowserRequestedAt=0;if(body.reason==='ACCOUNT_CHANGED')state.settings.minimaxBrowser=false;saveState();invalidate();return json(200,{ok:true});}
 if(!['bind','snapshot'].includes(action))return json(404,{code:'NOT_FOUND'});const next=normalizeMiniMaxConsumption(body),old=loadSnapshot();if(action==='snapshot'&&(!state.settings.minimaxBrowser||!old||next.accountFingerprint!==old.accountFingerprint)){state.settings.minimaxBrowser=false;state.settings.minimaxBrowserError='ACCOUNT_CHANGED';saveState();invalidate();return json(409,{code:'ACCOUNT_CHANGED'});}
 saveSnapshot(next);state.settings.minimaxBrowser=true;state.settings.minimaxBrowserError=null;state.settings.minimaxBrowserRequestedAt=0;saveState();invalidate();return json(200,{ok:true});
 }catch{return json(400,{code:'MINIMAX_PAGE'});}
}
