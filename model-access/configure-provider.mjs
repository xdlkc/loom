import {hiddenComplete} from './hub-admin.mjs';
try{hiddenComplete(process.argv[2]);}catch{console.error('未完成保存。请返回网页检查任务状态或重新创建任务；不要把 Key 发到聊天。');process.exitCode=1;}
