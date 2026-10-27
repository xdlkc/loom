import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
export const dataDir=path.resolve(process.env.QUOTA_DATA_DIR || fileURLToPath(new URL('./.local/',import.meta.url)));
export function load(name,fallback){try{return JSON.parse(fs.readFileSync(path.join(dataDir,name+'.json'),'utf8'));}catch(e){if(e.code==='ENOENT')return fallback;throw new Error('本地数据文件无法读取，请检查 JSON 与文件权限');}}
export function save(name,data){fs.mkdirSync(dataDir,{recursive:true,mode:0o700});fs.chmodSync(dataDir,0o700);const p=path.join(dataDir,name+'.json'),tmp=p+'.tmp';fs.writeFileSync(tmp,JSON.stringify(data,null,2),{mode:0o600});fs.chmodSync(tmp,0o600);fs.renameSync(tmp,p);}
