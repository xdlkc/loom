# Loom 本机可观测与会话管控

## 目标与数据边界

提供供应商 / 模型 / harness / session 四个维度的请求追踪、完整会话时间线与全局统计。支持 pi、OpenCode V2（opencode2）、Claude Code。网页由 Rust 直接提供，SQLite 和会话正文只存于本机私有数据目录；上游凭据不进入数据库或网页。此能力属于模型网关，不引入旧额度服务或浏览器登录态。

## 调研记录（2026-11-01）

- [ZenMux 请求日志](https://zenmux.ai/docs/guide/observability/logs.html)：按供应商、模型、时间和完成原因筛选；请求 / 转发 / 响应详情、首 Token 延迟、吞吐、缓存读写分解。Loom 沿用可下钻的请求详情与独立统计概览。
- [ZenMux 用量统计](https://zenmux.ai/docs/guide/observability/usage.html)：输入 / 输出 Token、请求数、按模型和供应商分布、性能对比。文档明确该用量页面不包含订阅，不能混作用户全部用量；Loom 统计经过本机网关的实际调用，不读取厂商余额推算。
- [ZenMux generation](https://zenmux.ai/docs/api/platform/get-generation.html)：usage 同步返回，账单费用可能延迟 3–5 分钟。Loom 将实测 Token 与估算金额 / 官方账单分开，未配置价格时金额显示未知，不用 0 冒充免费。
- [DeepSeek Chat Completions](https://api-docs.deepseek.com/api/create-chat-completion/) 与 [缓存](https://api-docs.deepseek.com/guides/kv_cache/)：prompt_tokens 已包含缓存；prompt_cache_hit_tokens 与 miss 互补。Loom 采用 Token 加权命中率，不平均每次请求的百分比，不把 reasoning_tokens 再加到输出中。
- [Kimi Code](https://www.kimi.com/code/docs/) / [配置](https://www.kimi.com/code/docs/en/kimi-code-cli/configuration/config-files.html)：国内订阅与 Moonshot 开放平台为不同产品，会员频控、积分与钱包计费不能混为美元消费。[官方会员权益](https://www.kimi.com/code/docs/en/kimi-code/membership.html) 的界面按剩余额度 / 速率窗口 / Extra Usage 展示，并提供 CLI `/usage`；所有设备和 Key 共享额度。[官方缓存实践](https://www.kimi.ai/academy/best-practices-for-context-caching) 则单独说明 API 缓存。Loom 将订阅窗口、缓存 Token、实际推理用量分开，不把订阅剩余百分比当作 Token 曲线。受登录限制的面板详情不作臆测。
- [Claude Code hooks](https://code.claude.com/docs/en/hooks)：session_id、transcript_path、工具生命周期；请求 metadata 与 transcript message ID 可提供确定性关联。
- pi 本机安装包 `@earendil-works/pi-coding-agent` 的 extensions 类型：提供 before_provider_headers，ctx.sessionManager.getSessionId，message_end、tool_execution_*；JSONL 具有 parentId、工具结果、压缩事件、responseId。
- [OpenCode V2 插件迁移](https://dev.opencode.ai/v2/docs/build/plugins/migrate-v1)：V1 chat.headers 不能直接用于 V2，应使用 ctx.session.hook("model.request"/"http.request")。实机数据库为 session_v2 + session_message，而非 V1 message / part；两种格式需检测后读取，不能写原数据库。

## 实施与验收

1. SQLite 请求存储：开始 / 路由 / 转发 / 首字节 / 首 Token / 完成；HTTP 错误、断流、客户端取消和服务重启可区分。完整正文只在本机，先脱敏再落盘；缺失 / 截断显式标识。
2. 流式旁路解析：原始字节即时透传；SSE 分片、UTF-8、CRLF、多行 data 与末尾 usage 正确处理。OpenAI / DeepSeek / Anthropic / Kimi 字段归一化，缺失保持未知。
3. 三类 harness：本机原生轨迹增量导入；精确 session / response ID 关联，工具调用、工具结果、分支、压缩、错误按原事件保留。不凭提示词相似度猜测归属。事件与网关 Token 统计分开，避免导入重放造成双重计费。
4. 管理网页：概览（曲线、维度排行、缓存分解、性能、错误）、请求列表 / 详情、会话列表 / 完整轨迹、采集与保留设置。支持筛选、分页、时间窗口、跳转关联请求。
5. 安全：独立 loopback 管理端口；仅同源访问、Host 校验、HttpOnly SameSite 会话，启动登录 ticket 一次性兑换；不把入口 Key 放在页面源码 / URL；无 CORS 开放、无通用文件读取接口。正文 textContent 渲染，不能执行轨迹中的 HTML / 指令。
6. 真实验收：三类 harness 各执行一个可控多步工具会话，确认请求、工具、最终响应与 Token 进入同一轨迹；浏览器实际交互验证筛选与下钻；原配置 / 凭据内容不变；升级 / 重启 / 留存 / 删除与认证边界测试。

此前 AGENTS 中禁止请求正文日志针对旧版无管控网关。用户本次明确授权完整轨迹，现更新范围为本机私有脱敏采集；不扩展到公开发布个人数据。
