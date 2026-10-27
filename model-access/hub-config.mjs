import {anthropicEndpoint} from './anthropic.mjs';
import fs from 'node:fs';import path from 'node:path';import os from 'node:os';import {randomBytes} from 'node:crypto';import {fileURLToPath} from 'node:url';
export const root=path.dirname(fileURLToPath(import.meta.url));
export const dataDir=process.env.MODEL_HUB_DATA_DIR||path.join(os.homedir(),'Library/Application Support/ModelHub/.local');
export const registryPath=path.join(dataDir,'providers.json');
export const tokenPath=path.join(dataDir,'hub.key');
export const baseURL='http://127.0.0.1:4319/v1';
export const read=(p,f={})=>fs.existsSync(p)?JSON.parse(fs.readFileSync(p,'utf8')):f;
export function write(p,value){fs.mkdirSync(path.dirname(p),{recursive:true,mode:0o700});const t=p+'.'+randomBytes(6).toString('hex')+'.tmp';fs.writeFileSync(t,typeof value==='string'?value:JSON.stringify(value,null,2)+'\n',{mode:0o600});fs.renameSync(t,p);}
export function backup(p){if(!fs.existsSync(p))return;const dir=path.join(dataDir,'backups',new Date().toISOString().replaceAll(':','-'));fs.mkdirSync(dir,{recursive:true,mode:0o700});const dest=path.join(dir,path.basename(path.dirname(p))+'-'+path.basename(p));fs.copyFileSync(p,dest);fs.chmodSync(dest,0o600);}
export function ensureToken(){fs.mkdirSync(dataDir,{recursive:true,mode:0o700});fs.chmodSync(dataDir,0o700);if(!fs.existsSync(tokenPath))write(tokenPath,'hub-'+randomBytes(32).toString('hex')+'\n');fs.chmodSync(tokenPath,0o600);return fs.readFileSync(tokenPath,'utf8').trim();}
export function catalog(registry){const out=[];const seen=new Set();for(const [provider,p] of Object.entries(registry.providers||{})){if(!p.enabled)continue;anthropicEndpoint(p);const prefix=p.prefix||provider.replace(/^local-/,'').replace(/-plan$/,'');if(!/^[a-z][a-z0-9-]*$/.test(prefix))throw Error('PROVIDER');const u=new URL(p.baseURL);if(u.protocol!=='https:'||u.username||u.password||u.search||u.hash)throw Error('ENDPOINT');if(!Array.isArray(p.models)||!p.keyFile)throw Error('PROVIDER');for(const m of p.models){if(m.enabled===false)continue;if(typeof m.id!=='string'||!m.id||/[\s?#]/.test(m.id))throw Error('MODEL');const id=prefix+'/'+m.id;if(seen.has(id))throw Error('DUPLICATE');seen.add(id);out.push({id,provider,p,m});}}return out;}
export function modelLabel(id,m){
 const source=id.split('/')[0];
 let name=(m.name||id.split('/').at(-1)).replace(/^[^:]+:\s*/, '').replace(/^Claude\s+/i,'');
 name=name.replace(/^claude-/i,'').replace(/^glm-/i,'GLM ').replace(/^DeepSeek-/i,'DeepSeek ').replace(/-/g,' ');
 return 'hub/'+name+' · '+source;
}
export function buildHubConfigs(registry,pi={},settings={},oc={},firstMigration=false){
 pi=structuredClone(pi);settings=structuredClone(settings);oc=structuredClone(oc);pi.providers||={};oc.providers||={};const models=catalog(registry);if(!models.length)throw Error('NO_MODELS');
 // Migrate the previous client prefix while retaining the selected model.
 if(pi.providers['local-hub']?.baseUrl===baseURL)delete pi.providers['local-hub'];
 if(oc.providers['local-hub']?.settings?.baseURL===baseURL)delete oc.providers['local-hub'];
 if(settings.defaultProvider==='local-hub')settings.defaultProvider='hub';
 if(oc.model?.startsWith('local-hub/'))oc.model='hub/'+oc.model.slice('local-hub/'.length);
 if(Array.isArray(settings.enabledModels))settings.enabledModels=settings.enabledModels.map(x=>x.startsWith('local-hub/')?'hub/'+x.slice('local-hub/'.length):x);
 const managed=new Set(Object.keys(registry.providers||{}));
 if(firstMigration){for(const [id,url] of [['zenmux','https://zenmux.ai/api/v1'],['deepseek','https://api.deepseek.com']])if(pi.providers[id]?.baseUrl===url&&registry.providers['local-'+id]){delete pi.providers[id];if(Array.isArray(settings.enabledModels))settings.enabledModels=settings.enabledModels.filter(x=>!x.startsWith(id+'/'));}}for(const id of managed){delete pi.providers[id];delete oc.providers[id];}
 pi.providers['hub']={name:'hub · 本机统一模型入口',baseUrl:baseURL,api:'openai-completions',apiKey:'!cat '+"'"+tokenPath.replaceAll("'","'\\''")+"'",authHeader:true,models:models.map(({id,p,m})=>({...Object.fromEntries(Object.entries(m).filter(([k])=>k!=='enabled')),id,name:modelLabel(id,m),compat:{supportsStore:false,supportsDeveloperRole:false,supportsStrictMode:false,maxTokensField:'max_tokens',requiresReasoningContentOnAssistantMessages:true,...p.piCompat}}))};
 oc.providers['hub']={name:'hub · 本机统一模型入口',package:'@opencode/ai/providers/openai-compatible',settings:{baseURL,apiKey:'{file:'+tokenPath+'}'},models:Object.fromEntries(models.map(({id,p,m})=>[id,{name:modelLabel(id,m),capabilities:{tools:true,input:m.input||['text'],output:['text']},limit:{context:m.contextWindow||128000,output:m.maxTokens||8192}}]))};
 const valid=new Set(models.map(m=>'hub/'+m.id));const oldManaged=x=>[...managed].some(id=>x.startsWith(id+'/'))||x.startsWith('hub/');
 // Keep non-managed OAuth and custom models selectable, but use the hub by default after migration.
 if(Array.isArray(settings.enabledModels))settings.enabledModels=[...new Set([...settings.enabledModels.filter(x=>!oldManaged(x)),...valid])];
 const fallback=models.find(x=>x.id==='glm/glm-5.3')?.id||models[0].id;
 if(firstMigration||settings.defaultProvider==='hub'&&!valid.has('hub/'+settings.defaultModel)){settings.defaultProvider='hub';settings.defaultModel=fallback;}
 if(firstMigration||oc.model?.startsWith('hub/')&&!valid.has(oc.model))oc.model='hub/'+fallback;
 return {pi,settings,oc};
}
export function syncAgents({home=os.homedir(),firstMigration=false,quiet=false}={}){const registry=read(registryPath);const pp=path.join(home,'.pi/agent/models.json'),sp=path.join(home,'.pi/agent/settings.json'),op=path.join(home,'.config/opencode/opencode.json');if(fs.existsSync(op+'c'))throw Error('JSONC_EXISTS');const result=buildHubConfigs(registry,read(pp),read(sp),read(op),firstMigration);let changed=false;for(const [p,v] of [[pp,result.pi],[sp,result.settings],[op,result.oc]]){if(JSON.stringify(read(p))===JSON.stringify(v))continue;backup(p);write(p,v);changed=true;}if(!quiet)console.log(changed?'已同步统一入口到 pi 和 opencode2。':'模型目录已是最新。');return changed;}
