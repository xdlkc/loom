# 官方接口核实与 MVP 边界

核实日期：2026-10-24。下列“接入”表示已写适配器，**不代表已用真实账户验证**。不把社区反向分析当作公开官方合约。没有查询凭据的服务初始为空。

| 服务 / 产品 | MVP 自动查询与认证 | 准确性边界与官方资料 |
| --- | --- | --- |
| ZenMux PAYG | `GET https://zenmux.ai/api/v1/management/payg/balance`，Bearer Management API Key | 展示 USD total / top_up / bonus credits。[官方钱包文档](https://zenmux.ai/docs/api/platform/payg-balance.html) |
| ZenMux Builder | `GET https://zenmux.ai/api/v1/management/subscription/detail`，Bearer Management API Key | 模型调用 Key 不被该接口接受。5h / 7d 为独立窗口；monthly 仅上限。重置只用服务端 `resets_at`。[官方订阅查询](https://zenmux.ai/docs/api/platform/subscription-detail.html) |
| Kimi / Moonshot API 国内 | `GET https://api.moonshot.cn/v1/users/me/balance`，Bearer 国内开放平台 API Key | CNY available / voucher / cash；不包含 Kimi Code 或会员额度。国内与国际 Key 不互通，MVP 仅实现国内。[官方余额查询](https://platform.kimi.com/docs/api/balance) |
| Kimi Code / 会员订阅 | 已接入官方 Kimi Code 本地 OAuth 服务 `GET /api/v1/oauth/usage`；使用本机服务 bearer token，与 Moonshot Key 独立 | 依据当前 CLI 2.3.1 的实时 OpenAPI 验证。支持实际返回的 limit5h / limit7d / monthTotal / monthCode，使用 usedRatio 计算剩余百分比，resetAt 按原值显示。extraUsage 为会员额外用量钱包，单独展示；null 不视为零。不使用网页 Cookie 或社区反向接口。可选 userinfo 只选套餐等级字段。[官方本地接口与响应](https://www.kimi.com/code/docs/en/kimi-code-cli/reference/server-api.html)、[官方安装与登录](https://www.kimi.com/code/docs/en/kimi-code-cli/guides/getting-started) |
| GLM / 智谱 Coding Plan | `GET https://open.bigmodel.cn/api/monitor/usage/quota/limit`，原始 API Key 放入 Authorization，不自行加 Bearer | 依据智谱官方插件源码而非独立 REST 合约。支持官方插件已有 TOKENS_LIMIT / TIME_LIMIT 的 percentage（已用），按 100−已用显示剩余。不能解释的新类型报未识别；不拿控制台业务 Cookie 取余额。已用个人 Key 实测成功，套餐等级使用 level 原值；多个 Token 窗口按响应顺序区分，周期枚举无已核实官方说明时不命名。nextResetTime 仅按响应中的毫秒时间戳展示，缺失则未知。[官方插件源码](https://github.com/zai-org/zai-coding-plugins/blob/main/plugins/glm-plan-usage/skills/usage-query-skill/scripts/query-usage.mjs)、[官方插件入口](https://docs.bigmodel.cn/cn/coding-plan/extension/coding-tool-helper) |
| MiniMax Token Plan | `GET https://www.minimax.cn/v1/token_plan/remains`（国内）或 `https://www.minimax.io/v1/token_plan/remains`（国际），Bearer Token Plan API Key | 官方 FAQ 提供入口，已用国内订阅 Key 实测成功。MVP 只识别显式 interval / weekly remaining_percent，不把含 usage 的计数猜成剩余；不支持的响应需手动记录。[官方国际页](https://platform.minimax.io/subscribe/token-plan)、[官方国内 FAQ](https://platform.minimax.cn/docs/m-plan/faq) |
| DeepSeek API | `GET https://api.deepseek.com/user/balance`，Bearer API Key | 按响应 CNY / USD 分项展示 total / granted / topped_up。[官方余额接口](https://api-docs.deepseek.com/api/get-user-balance/) |
| Grok / xAI API | `GET https://management-api.x.ai/v1/billing/teams/{team_id}/prepaid/balance`；`POST .../usage` 为只读查询；Bearer Management Key + Team ID | 与推理 Key 分开。预付账本美分转 USD，但不解释正负号为可消费余额。费用查询明确 UTC 月初至当前、usd SUM，拒绝截断响应。不执行 top-up 或更改支付方式。[管理认证](https://docs.x.ai/developers/management-api-guide)、[账单接口](https://docs.x.ai/developers/rest-api-reference/management/billing) |
| Grok 网页订阅 | 本机 Chrome 扩展读取官方 dashboard/usage 可见页面 | 周包含额度为共享池；仅读取套餐、百分比、重置日期与 Token 汇总，排除 Credits 和 xAI API 余额。登录态留在 Chrome，日期没有时区时不推测重置时刻。 |
| OpenAI API | `GET https://api.openai.com/v1/organization/costs`，Bearer Admin Key | 查组织 UTC 当月费用，固定 end_time 并遍历全部 next_page；不是余额查询。普通推理 Key 不能假定拥有组织管理权限。[官方费用查询](https://developers.openai.com/api/reference/resources/admin/subresources/organization/subresources/usage/methods/costs)、[Admin Key](https://developers.openai.com/api/reference/resources/admin/subresources/organization/subresources/admin_api_keys) |
| ChatGPT 网页订阅 | 手动记录 | 未发现本次可采用的公开个人网页订阅剩余额度接口。不要以 OpenAI API 费用抵扣或推断 ChatGPT 套餐额度。 |
| Gemini API / Cloud Monitoring | `GET https://monitoring.googleapis.com/v3/projects/{project_id}/timeSeries`，OAuth Access Token + Project ID | 查询 `serviceruntime.googleapis.com/api/request_count`，筛选 consumed_api / generativelanguage.googleapis.com；对最近24小时 DELTA 样本求和并翻页，包含成功与失败请求。无样本不报0。不是 Token、计费费用或配额余量。指标可能有约30分钟延迟，部分 API 不一定提供样本。需要 Monitoring Read IAM 权限和 OAuth scope，模型调用 Key 不适用。[查询与认证](https://cloud.google.com/monitoring/api/ref_v3/rest/v3/projects.timeSeries/list)、[官方指标语义](https://cloud.google.com/monitoring/api/metrics_gcp_p_z#serviceruntime) |
| Gemini 网页会员 / API 余额和额度 | 手动记录；MVP 不做 Cloud Billing BigQuery 导出集成 | Gemini API 可在 AI Studio 查看项目计费、用量与限额；不能将 Google AI 网页订阅当成 Gemini API 钱包。Cloud Billing / Quota 等其他接口存在，但无通用的“给 Gemini 推理 Key 即返回个人订阅余额”能力。[Gemini API 计费](https://ai.google.dev/gemini-api/docs/billing) |

重置、金额、额度上限均以实际官方响应或人工观察为准，不写死套餐价格、模型数量或每月固定配额。接口迁移和字段变化会使适配器安全失败，原数据与成功时间保留。

补充：MiniMax 官方 CLI 在 `src/client/endpoints.ts` 中区分订阅用量 `/v1/token_plan/remains` 与普通 API 钱包 `/account/query_balance`。本次仅实现并验证订阅；钱包未接入，不代表厂商没有查询能力。[官方 CLI 源码](https://github.com/MiniMax-AI/cli/blob/main/src/client/endpoints.ts)。重置采用接口绝对时间，倒计时换算仅作明确标注的回退。


### Codex 订阅额度

采用本机官方 `codex app-server --stdio` 的 `initialize`、`account/read` 和 `account/rateLimits/read`，不读取或复制 OAuth 文件，不创建线程或模型调用。优先解释 `rateLimitsByLimitId`；窗口长度取官方 `windowDurationMins`，剩余为 100 − usedPercent，重置取 Unix 秒时间戳。仅展示实际返回窗口；重置券仅显示可用次数，不调用 consume。忽略账号 ID、邮箱、完整券明细和未核实币种的 credits。该结果不代表所有 ChatGPT 网页模型的消息额度，也不是 OpenAI API 钱包。

[官方 App Server 文档](https://learn.chatgpt.com/docs/app-server)。


### Gemini 网页用量页

依据 [Gemini 官方用量说明](https://support.google.com/gemini/answer/16275805)，在 Gemini 网页“设置 → 用量限额”读取目标 Google 账号当前窗口和周窗口；官方帮助说明为 5 小时与每周额度，页面卡片按实际标签显示。用户已确认目标账号，实测 /u/1/usage 为 Google AI Ultra，当前与周窗口均显示已使用 1%。页面重置文字为局部时间，不补造年份或时区。

采集仅使用渲染 DOM，通过与 Grok 合并的统一扩展绑定可见账号的 SHA-256 指纹；后台账号或路径不符时暂停更新，不回退到默认 Google 账号。不查询其他产品或计划价格。
