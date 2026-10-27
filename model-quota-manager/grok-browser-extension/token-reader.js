// Only exact visible chart tooltip values; never infer values from SVG geometry.
function parseGrokTokenTooltip(label,expectedLabel){
 if(typeof label!=='string')throw Error('PAGE');const lines=label.split('\n');if(lines.shift()!==expectedLabel)throw Error('PAGE');const values={};let reported=null;for(const line of lines){const m=line.match(/^([A-Za-z0-9_. /:@+-]+): ([\d,]+) tokens$/);if(!m)throw Error('PAGE');const n=Number(m[2].replaceAll(',',''));if(!Number.isSafeInteger(n)||n<0)throw Error('PAGE');if(m[1]==='Total'){if(reported!==null)throw Error('PAGE');reported=n;}else {if(Object.hasOwn(values,m[1]))throw Error('PAGE');values[m[1]]=n;}}
 const total=Object.values(values).reduce((a,b)=>a+b,0);if(!Object.keys(values).length||!Number.isSafeInteger(total)||reported!==null&&reported!==total)throw Error('PAGE');return {values,total};
}
function parseDeepSeekTokenTooltip(text,day){
 const lines=text.trim().split('\n').map(s=>s.trim());if(lines.length!==8||lines[0]!==day||lines[1]!=='输入（命中缓存）'||lines[2]!=='输入（未命中缓存）'||lines[3]!=='输出')throw Error('PAGE');const ns=lines.slice(4).map(s=>{if(!/^\d[\d,]*$/.test(s))throw Error('PAGE');const n=Number(s.replaceAll(',',''));if(!Number.isSafeInteger(n))throw Error('PAGE');return n;});if(ns[0]!==ns[1]+ns[2]+ns[3])throw Error('PAGE');return {tokens:ns[0],cacheReadTokens:ns[1],inputTokens:ns[2],outputTokens:ns[3]};
}
if(typeof module!=='undefined')module.exports={parseGrokTokenTooltip,parseDeepSeekTokenTooltip};
function readDeepSeekAccount(doc){
 const labels=[...doc.querySelectorAll('label')];const field=name=>labels.find(e=>e.textContent===name)?.parentElement.querySelector('.ds-form-item__content')?.textContent.trim()||'';
 const phone=field('手机号码').match(/\d{3}\*{3,8}\d{2,4}/)?.[0];if(phone)return 'phone:'+phone;
 const email=field('Email').match(/[A-Za-z0-9_.*+-]+@[A-Za-z0-9*.-]+\.[A-Za-z*]{2,}/)?.[0];if(email&&/[A-Za-z0-9]/.test(email.split('@')[0])&&/[A-Za-z0-9]/.test(email.split('@')[1]))return 'email:'+email.toLowerCase();return null;
}
if(typeof module!=='undefined')module.exports.readDeepSeekAccount=readDeepSeekAccount;
