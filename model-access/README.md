# 本机统一模型入口

将客户端连接到本机入口后，日常直接运行 `pi` 或 `opencode2`。模型和默认值由本机中央配置决定，原生 OAuth 登录保持独立。

- 地址：`http://127.0.0.1:4319/v1`
- 可配置模型，来自 GLM 国内 Coding Plan、MiniMax 国内订阅、DeepSeek API 钱包及 ZenMux。
- 由 macOS 用户登录启动项自动运行，与额度工具进程独立。
- 上游密钥仅由本机入口读取；agent 配置只引用本机入口 Key。

## 网页管理

打开现有 [模型额度网页](http://127.0.0.1:4318/#hub)，选择侧栏“统一模型入口”。可查看入口运行状态、可用模型数量、pi / opencode2 连接状态，以及各 provider 的计费来源。

- provider：新增、编辑名称 / 地址 / 计费说明、启停、轮换 Key。
- 模型：新增、编辑 ID / 显示名称 / 本地上限 / 图片和推理能力、启停、移除。
- 新增或轮换 Key：网页生成一小时有效的待配置任务，打开本机 Terminal；Key 通过隐藏输入保存，不经过网页请求或页面状态。
- 如果 Terminal 未自动弹出，待配置卡片会显示 `modelhub finish <任务 ID>`，在本机 Terminal 执行即可继续。
- 待配置任务可取消；取消或过期后不会保存输入的 Key。provider 必须完成 Key 保存后才会出现并启用。
- 修改自动同步到 pi 和 opencode2；至少保留一个可用模型。启停或移除不会删除上游模型，也不会更改服务商套餐。

网页管理使用已有本机同源和 CSRF 保护；返回数据只包含普通配置和是否有 Key，不返回 Key 值或私密文件路径。Terminal 的自动打开依赖 macOS 允许本机应用控制 Terminal，失败时可用卡片中的手动命令。网页负责保存配置，不能保证新填 provider / Model ID 在上游拥有推理权限；需按服务商实际模型与账号权限填写。

## 以后新增 provider

在 Terminal 运行一条命令：

```sh
modelhub add
```

按提示填写名称、OpenAI 兼容 Base URL、计费说明和 Model ID，然后在**隐藏输入**中粘贴 API Key。Model ID 留空时尝试从 `/models` 自动发现。保存成功后，入口热加载，并自动更新 pi 与 opencode2 的模型目录，无需逐个 agent 再配置。正在运行的交互会话可能需要重开模型选择器或重启会话。

当前新增向导支持 HTTPS 的 **OpenAI Chat Completions 兼容 provider**。仅提供 Anthropic Messages、Responses 或其他专用协议的 provider，需先适配协议，不能只填地址就用。自动发现使用保守的文本、128K 上下文和 8K 输出设置；多模态和特殊推理参数可在中央配置中补充。

## 选择模型

在 agent 的模型选择器搜索 `hub`，选择“hub · 本机统一模型入口”，或使用：

```sh
pi --provider hub --model glm/glm-5.3
opencode2 run --model hub/minimax/MiniMax-M3.1-Flash "检查当前项目"
```

统一入口模型名按来源区分：

| 前缀 | 计费范围 | 示例 |
|---|---|---|
| `glm/` | 国内 GLM Coding Plan，固定 `/api/coding/paas/v4` | `glm/glm-5.3` |
| `minimax/` | MiniMax 国内订阅 Key | `minimax/MiniMax-M3.1-Flash` |
| `deepseek/` | DeepSeek API 钱包 | `deepseek/deepseek-flash` |
| `zenmux/` | ZenMux 订阅 / 钱包，以 ZenMux 规则为准 | `zenmux/moonshotai/kimi-k3.1-pro`、`zenmux/x-ai/grok-4.7` |

不会自动更换 provider、跨计费池回退或重试生成。Kimi/Grok 经 ZenMux 调用时消耗 **ZenMux** 额度，不是其网页会员额度。未把浏览器登录态、OAuth 或额度查询 Management Key 用作统一推理 Key。

## 新增其他 agent

只需首次连接一次上述地址和本机入口 Key，并选择 `/v1/models` 中的模型。无需复制每个上游 provider 的密钥。

若该 agent 支持 `OPENAI_BASE_URL`、`OPENAI_API_KEY` 环境变量，可这样启动：

```sh
modelhub run -- your-agent
```

此命令只向子进程传递本机入口凭据，不传上游 Key。模型选择参数仍遵循该 agent 的说明；不读取环境变量的 agent 需在其设置中连接一次本机入口。对会缓存模型清单的其他 agent，新增模型后可能需要刷新清单。

## 维护

```sh
modelhub status                         # 运行状态及模型数量
modelhub list                           # 中央 provider 信息，不显示密钥
modelhub check                          # Key 是否存在及文件权限
modelhub set-key local-glm-plan          # 隐藏输入轮换对应 Key，入口下次请求即生效
modelhub sync                           # 手动重建客户端目录，通常不需要
opencode2 reload                        # 手动重载客户端配置，通常由入口自动完成
```

真正运行的文件和私密数据位于：

```text
~/Library/Application Support/ModelHub/
  hub.mjs / hub-config.mjs / model-access.mjs
  .local/
    providers.json          中央 provider、模型、计费说明和 Key 文件路径
    hub.key                 本机入口专用 Key
    keys/                   上游 API Key
    backups/                迁移与后续修改前的客户端配置备份
    hub.log / hub-error.log  仅服务状态，不记录请求正文、响应正文或 Key
```

目录权限 0700、密钥及配置文件 0600。不要上传 `.local`、备份或整个 Application Support 目录。它们仍可被同一 macOS 用户的进程读取，属于本机文件权限保护，不是硬件密钥库。客户端自身可能按其设置保存聊天记录。

客户端配置：`~/.pi/agent/models.json`、`~/.pi/agent/settings.json` 和 `~/.config/opencode/opencode.json`。迁移已备份；原有 OAuth 凭据未修改。原来由本任务生成的直连 provider 已合并为 `hub`；与本次迁移匹配的旧 ZenMux / DeepSeek 自定义条目也已移除，避免重复入口。现有会话的模型选择可能覆盖新默认值。

启动项：`~/Library/LaunchAgents/local.model-access.hub.plist`，标签 `local.model-access.hub`。停用可在 Terminal 执行 `launchctl bootout gui/$(id -u)/local.model-access.hub`；重新启用执行 `launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/local.model-access.hub.plist`。

本目录是可查看的源码副本，不包含私密数据；运行服务使用 Application Support 中的副本。不要只修改源码就认为运行版本已更新。

## 已验证


仅列出的代表模型做了实际推理测试，其余模型经过目录核实。入口支持 `/v1/models`、`/v1/chat/completions`、`/v1/messages`；不支持 `/v1/responses`、文件上传、语音或图片生成专用接口。支持在 Chat Completions 消息中透传模型本身支持的图片输入与工具调用。请求上限 20 MiB、并发 8、单次上游最长 10 分钟，输出受中央配置本地上限约束。

[OpenCode v2 Provider 配置](https://dev.opencode.ai/v2/docs/providers/) · [配置文件变量](https://dev.opencode.ai/v2/docs/config/#files) · [MiniMax 订阅工具接入](https://platform.minimax.cn/docs/m-plan/other-tools) · [GLM 订阅入口](https://zcode.z.ai/en/docs/configuration)


## Anthropic Messages

已增加原生 `POST /v1/messages`，包括普通JSON、SSE、工具调用/结果、system、图片、思考签名及缓存参数透传。客户端Base URL使用 `http://127.0.0.1:4319`（不附加 `/v1`）；使用现有本机入口Key，支持 `x-api-key` 或 `Authorization: Bearer`。版本为 `anthropic-version: 2023-06-01`；beta头会经过格式检查后透传。

已按官方地址接入ZenMux、GLM Coding Plan、MiniMax国内订阅。DeepSeek直连使用官方 `https://api.deepseek.com/anthropic`，消耗DeepSeek API钱包；不会换成ZenMux等其他计费来源。新provider如有独立Anthropic入口，可在中央provider配置里填写HTTPS的 `anthropicBaseURL`，以供应商实际支持为准。

`modelhub run -- <agent>` 现在也向子进程注入本机 `ANTHROPIC_BASE_URL`、`ANTHROPIC_API_KEY`、`ANTHROPIC_AUTH_TOKEN`，不暴露上游Key、不修改原生OAuth配置。例如已安装Claude Code时：

```sh
modelhub run -- claude --model zenmux/anthropic/claude-sonnet-5.5
```

HTTP请求的model使用 `zenmux/anthropic/claude-sonnet-5.5`；pi/opencode中的 `hub/` 是客户端provider前缀。OpenAI协议与现有默认模型保持不变。

上游模型功能限制仍生效；不声明支持整个Anthropic平台，例如 `messages/count_tokens`、批处理、Files、OAuth登录或托管服务器工具。11项统一入口测试通过，包括原OpenAI回归、原生错误格式、认证冲突、工具/思考块原样透传及流式事件顺序。


## Claude Code

- `ANTHROPIC_BASE_URL=http://127.0.0.1:4319`。
- `apiKeyHelper` 从本机 `ModelHub/.local/hub.key` 读取入口凭据；没有复制上游 Key。
- 默认 `zenmux/anthropic/claude-sonnet-5.5`，模型菜单标签显示 `hub/`，HTTP 请求使用中央模型 ID。
- 原配置备份在 `~/.claude/backups/settings.pre-modelhub-*.json`；原生 OAuth 凭据未修改。

重开 Claude Code 会话后执行 `/model` 可切换模型。实际 Claude Code 最小调用已通过，返回 OK；计费来源为 ZenMux。入口兼容 Claude Code 消息列表里的 system 消息，将其移入 Anthropic 顶层 system，保留文本块和缓存标记；12 项入口测试通过。

pi 与 opencode2 的模型显示名统一为 `hub/短模型名 · 来源`，例如 `hub/Sonnet 5.5 · zenmux`、`hub/GLM 5.3 · glm`。中央同步时自动生成；实际模型 ID 和计费路由不变。

Anthropic Messages 使用入口 `http://127.0.0.1:4319`，路由 `/v1/messages`。支持 ZenMux、GLM、MiniMax、DeepSeek、Kimi Code 官方原生兼容接口；各 provider 独立计费，不跨来源回退。凭据只在本机保存。
