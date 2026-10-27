// Native Anthropic endpoints only. Never switch providers or billing pools.
const known=new Map([
 ['https://api.kimi.com/coding/v1','https://api.kimi.com/coding'],
 ['https://api.deepseek.com','https://api.deepseek.com/anthropic'],
 ['https://zenmux.ai/api/v1','https://zenmux.ai/api/anthropic'],
 ['https://open.bigmodel.cn/api/coding/paas/v4','https://open.bigmodel.cn/api/anthropic'],
 ['https://api.minimax.cn/v1','https://api.minimax.cn/anthropic'],
 ['https://api.minimaxi.com/v1','https://api.minimaxi.com/anthropic'],
 ['https://api.minimax.io/v1','https://api.minimax.io/anthropic'],
 ['https://api.z.ai/api/coding/paas/v4','https://api.z.ai/api/anthropic']
]);
export function anthropicEndpoint(provider){
 const base=provider.anthropicBaseURL||known.get(provider.baseURL.replace(/\/$/,''));if(!base)return null;
 const u=new URL(base);if(u.protocol!=='https:'||u.username||u.password||u.search||u.hash)throw Error('ANTHROPIC_ENDPOINT_INVALID');
 return base.replace(/\/$/,'')+(u.pathname.replace(/\/$/,'').endsWith('/v1')?'/messages':'/v1/messages');
}
export function anthropicHeaders(headers){
 const version=headers['anthropic-version']||'2023-06-01',beta=headers['anthropic-beta'];
 if(version!=='2023-06-01'||beta!==undefined&&(typeof beta!=='string'||! /^[a-zA-Z0-9_,.-]{1,1024}$/.test(beta)))throw Error('INVALID_ANTHROPIC_HEADERS');
 return {'anthropic-version':version,...(beta?{'anthropic-beta':beta}:{})};
}
export function validateAnthropic(body){
 if(!Number.isInteger(body.max_tokens)||body.max_tokens<1||!Array.isArray(body.messages)||!body.messages.length)throw Error('INVALID_MESSAGES_REQUEST');
 const extra=body.messages.filter(m=>m?.role==='system');
 if(extra.length){
  const blocks=v=>typeof v==='string'?[{type:'text',text:v}]:Array.isArray(v)?v:null;
  if(extra.some(m=>!blocks(m.content))||body.system!==undefined&&!blocks(body.system))throw Error('INVALID_MESSAGES_REQUEST');
  body.system=[...blocks(body.system??[]),...extra.flatMap(m=>blocks(m.content))];
  body.messages=body.messages.filter(m=>m?.role!=='system');
  if(!body.messages.length)throw Error('INVALID_MESSAGES_REQUEST');
 }
 for(const m of body.messages)if(!m||!['user','assistant'].includes(m.role)||!(typeof m.content==='string'||Array.isArray(m.content)&&m.content.length))throw Error('INVALID_MESSAGES_REQUEST');
 if(body.system!==undefined&&typeof body.system!=='string'&&!Array.isArray(body.system))throw Error('INVALID_MESSAGES_REQUEST');
}
export function anthropicError(status,code){const type=status===401?'authentication_error':status===403?'permission_error':status===429?'rate_limit_error':status===413?'request_too_large':status===404?'not_found_error':status<500?'invalid_request_error':'api_error';return {type:'error',error:{type,message:code}};}
