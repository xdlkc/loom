export const providers = [
 {id:'zenmux',name:'ZenMux',mark:'Z',color:'#7755d9',description:'Builder 订阅与 PAYG 钱包分别计量。模型调用 Key 不能替代 Management Key。',docs:'https://zenmux.ai/docs/api/platform/subscription-detail.html',console:'https://zenmux.ai',auto:['zenmux-subscription','zenmux-balance']},
 {id:'kimi',name:'Kimi',mark:'K',color:'#222832',description:'Kimi Code / 会员通过官方本地 OAuth 服务查询订阅窗口；会员额外用量钱包和 Moonshot API 钱包独立管理。',docs:'https://www.kimi.com/code/docs/en/kimi-code-cli/reference/server-api.html',console:'https://www.kimi.com/membership/subscription?tab=quota',auto:['kimi-code-quota','kimi-code-extra','kimi-balance']},
 {id:'glm',name:'GLM · 智谱',mark:'G',color:'#3264d9',description:'依据智谱官方用量插件查询 Coding Plan 已用百分比；剩余由 100% 减去已用计算，API 钱包另行记录。',docs:'https://docs.bigmodel.cn/cn/coding-plan/extension/coding-tool-helper',console:'https://bigmodel.cn/console/overview',auto:['glm-quota']},
 {id:'minimax',name:'MiniMax',mark:'M',color:'#db7041',description:'Token Plan 官方查询入口。响应未提供明确剩余百分比时拒绝推算；API 钱包单独手动记录。',docs:'https://platform.minimax.cn/docs/m-plan/faq',console:'https://platform.minimax.cn/console/usage',auto:['minimax-quota']},
 {id:'deepseek',name:'DeepSeek',mark:'D',color:'#4779dc',description:'API Key 可查询钱包余额；赠金与充值分项显示。不是聊天网页用量。',docs:'https://api-docs.deepseek.com/api/get-user-balance/',console:'https://platform.deepseek.com',auto:['deepseek-balance']},
 {id:'grok',name:'Grok · xAI',mark:'𝕏',color:'#333b40',description:'网页订阅通过本机 Chrome 扩展读取官方仪表盘；与可选 xAI API 管理账单分别展示，不混用余额。',docs:'https://docs.x.ai/developers/rest-api-reference/management/billing',console:'https://grok.com/dashboard/usage',auto:['grok-web-subscription','grok-web-usage','xai-balance','xai-usage']},
 {id:'chatgpt',name:'ChatGPT / Codex',mark:'O',color:'#32836b',description:'通过官方 Codex 本机登录读取实际额度池与窗口；不代表 ChatGPT 网页全部模型的消息额度。OpenAI API 费用独立展示。',docs:'https://learn.chatgpt.com/docs/app-server',console:'https://chatgpt.com/codex',auto:['codex-quota','openai-costs']},
 {id:'gemini',name:'Gemini',mark:'✧',color:'#6788d7',description:'网页会员通过本机 Chrome 扩展绑定指定 Gemini 账号，只读取官方用量页；与 Gemini API、CLI 和 Antigravity 独立。',docs:'https://support.google.com/gemini/answer/16275805',console:'https://gemini.google.com',auto:['gemini-web-quota','gemini-requests']}
];
export const connectors={
 'gemini-web-quota':{provider:'gemini',label:'Gemini 网页会员额度（指定账号）',key:'geminiBrowser',auth:'本机 Chrome 目标账号登录态，仅读取官方用量页（无需 API Key）'},
 'codex-quota':{provider:'chatgpt',label:'Codex · ChatGPT 订阅额度',key:'codexLocal',auth:'官方 Codex 本机登录（无需 API Key）'},
 'grok-web-subscription':{provider:'grok',label:'Grok 网页订阅周额度',key:'grokBrowser',auth:'本机 Chrome 登录态，仅读取官方可见页面（无需 API Key）'},
 'grok-web-usage':{provider:'grok',label:'Grok 网页用量汇总',key:'grokBrowser',auth:'本机 Chrome 登录态，仅读取官方可见页面（无需 API Key）'},
 'kimi-code-quota':{provider:'kimi',label:'Kimi Code / 会员订阅配额',key:'kimiCode',auth:'官方 Kimi Code 本地 OAuth 登录（无需 Moonshot Key）'},
 'kimi-code-extra':{provider:'kimi',label:'Kimi 会员额外用量钱包',key:'kimiCode',auth:'官方 Kimi Code 本地 OAuth 登录（无需 Moonshot Key）'},
 'glm-quota':{provider:'glm',label:'GLM Coding Plan 配额',key:'glm',auth:'智谱 Coding Plan API Key（原始 Authorization）'},
 'gemini-requests':{provider:'gemini',label:'Gemini API 请求统计（24 小时）',key:'gemini',auth:'Google Cloud OAuth Access Token + Project ID（Monitoring Read）'},
 'zenmux-subscription':{provider:'zenmux',label:'Builder 订阅',key:'zenmux',auth:'ZenMux Management API Key'},
 'zenmux-balance':{provider:'zenmux',label:'PAYG 钱包',key:'zenmux',auth:'ZenMux Management API Key'},
 'kimi-balance':{provider:'kimi',label:'Moonshot API 钱包（国内）',key:'kimi',auth:'国内 Moonshot 开放平台 API Key'},
 'deepseek-balance':{provider:'deepseek',label:'DeepSeek API 钱包',key:'deepseek',auth:'DeepSeek API Key'},
 'minimax-quota':{provider:'minimax',label:'Token Plan',key:'minimax',auth:'MiniMax Token Plan API Key'},
 'xai-balance':{provider:'grok',label:'API 预付账本',key:'xai',auth:'xAI Management Key + Team ID'},
 'xai-usage':{provider:'grok',label:'API 当月费用',key:'xai',auth:'xAI Management Key + Team ID'},
 'openai-costs':{provider:'chatgpt',label:'OpenAI API 当月费用（组织）',key:'openai',auth:'OpenAI Admin Key（Usage Read）'}
};
export const kinds={balance:'余额',subscription:'订阅配额',usage:'API 用量 / 费用',webUsage:'网页订阅用量',estimate:'估算值'};
