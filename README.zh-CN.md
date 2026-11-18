# Loom

[English](./README.md) | 简体中文

**本机专用的 Rust 模型统一网关与回环可观测管控台。**

Loom 通过单一本机回环守护进程（`127.0.0.1:4319`）统一代理 OpenAI Chat Completions 与 Anthropic Messages 请求；上游供应商 API Key 仅保存在本机私有目录（`0700` 目录下的 `0600` 文件），各 agent 客户端只引用本机入口密钥（`hub.key`）。Loom 使用纯 Rust（`edition = "2024"`，Rust `1.89+`）构建与运行，安装、自启动与日常服务均不依赖 Python 或 Node.js。

---

## 概览与明确边界

Loom 的职责严格限定在**模型统一路由、本机客户端同步与回环请求/会话可观测性**：

- **仅限本机回环**：推理网关仅监听 `127.0.0.1:4319`，管控台仅监听 `127.0.0.1:4320`。
- **不混用通道与静默重试**：不同计费来源分开配置，不跨 provider 回退，也不自动重试生成请求。
- **明确非目标**：**不包含**厂商额度面板、账号用量采集脚本、浏览器扩展或默认放行/审批跳过启动器。
- **兼容命名**：项目、仓库（`xdlkc/loom`）与主命令为 **Loom**（`loom`），同时保留 `modelhub` 命令别名、`hub/` 客户端 provider 前缀、`4319` 默认端口与历史 `ModelHub` 数据目录，确保既有 agent 与密钥路径平滑兼容。

---

## 快速开始（3 分钟上手）

从源码构建需要 [Rust 工具链](https://rustup.rs/)（因启用 `rusqlite` bundled 构建，还需 C 编译器：macOS 安装 Xcode Command Line Tools，Linux 安装 `cc`）。交互运行 `./setup.sh` 时若缺少 Rust 工具链，脚本会自动引导通过官方 `rustup` 安装。

### 1. 一键安装与桌面端部署

```sh
./setup.sh        # 检查编译环境、编译 release 二进制、引导初始配置、启用自启动并构建 macOS Loom.app
loom              # 在终端打开交互式新手控制中心（查看状态、配置 Key、自检与打开工作台）
```

安装完成后，小白用户可任选以下三种前端之一完成全流程配置：

- **macOS 桌面端（`~/Applications/Loom.app`）**：双击打开即可使用，菜单栏常驻 `⌘` 图标支持一键启停后台网关、直达工作台、运行连通性自检，免终端命令自动安全登录。
- **Web 可视化工作台（`loom console`）**：内置「新手 3 步向导」，平铺各大模型厂商卡片、官网 Key 获取入口与防踩坑提示，粘贴 API Key 后一键验证连通性并自动同步客户端。
- **终端交互菜单（`loom`）**：无参数运行直接进入控制中心，或通过 `loom add`（隐藏输入）与 `loom doctor --ping` 完成配置与实测。

首次交互安装时，若直接回车跳过终端输入，也可随后打开 `Loom.app` 或 `loom console` 在可视化界面完成所有厂商配置。再次安装或更新时，既有配置与密钥原封保留。

### 手动编译与前台运行

```sh
cargo build --release --locked
./target/release/loom init
./target/release/loom add
./target/release/loom serve
```

### 用户级安装器、桌面端与一条命令更新

```sh
loom install              # 安装二进制至 ~/.local/bin/{loom,modelhub} 并注册用户自启动服务
loom install --no-start   # 仅部署二进制与服务配置，不启停正在运行的服务
loom app                  # 编译并打开 macOS 原生应用 ~/Applications/Loom.app（支持 --build-only / --rebuild / --app-dir）
loom doctor --ping        # 一键检查本机权限、客户端同步状态并实测各厂商单 Token 连通性
loom update               # 从 github.com/xdlkc/loom 拉取 main，执行 --release --locked 编译，校验配置/Key 不变后自动重启
```

安装器使用 macOS `launchctl`（`~/Library/LaunchAgents/local.model-access.hub.plist`）或 Linux `systemctl --user`（`~/.config/systemd/user/local.model-access.hub.service`），服务标签为 `local.model-access.hub`，无需管理员权限。覆盖前会将二进制、启动项和配置备份至本机私有目录；若健康检查失败，将尝试回滚旧版本。`loom update` 在 `<data-dir>/updates/` 内加独占锁（`update.lock`）隔离构建，不会改动你的本地 Git 工作副本。

---

## 支持的供应商与公共目录

使用 `loom add`（别名 `loom configure`）通过隐藏输入添加预设供应商或轮换 Key，使用 `--custom` 配置自定义 HTTPS 供应商：

```sh
loom add                       # 交互选择（默认全选）
loom add zenmux deepseek        # 仅询问指定供应商的 Key
loom add kimi                  # 已有供应商输入新 Key 更换（回车保留原 Key）
loom add longcat               # 美团 LongCat，隐藏输入开放平台 Key
loom add --custom              # 手动配置列表外的 HTTPS OpenAI/Anthropic 供应商
```

| 编号 | 预设 ID | 供应商 | 所需凭据 | 默认 / 公共目录模型 |
| --- | --- | --- | --- | --- |
| 1 | `zenmux` | ZenMux | 推理 API Key（非 Management Key） | Claude 5.5 系列（`claude-sonnet-5.5`、`claude-opus-5.5`、`claude-haiku-5.5`）+ 公共目录 `meta/muse-wonder-1.0`、`inclusionai/ling-3.1-flash`、`xiaomi/mimo-v2.6-flash`、`xiaomi/mimo-v2.6-pro` |
| 2 | `glm` | GLM 国内 Coding Plan | 套餐 API Key | `glm-5.3`、`glm-5.3-flash`、`glm-5.3-flashx`、`glm-5.2`、`glm-5.1`、`glm-5-turbo`、`glm-5`、`glm-4.7`、`glm-4.6`、`glm-4.5`、`glm-4.5-air` |
| 3 | `minimax` | MiniMax 国内订阅 | Subscription Key | `MiniMax-M3.1-Flash` |
| 4 | `deepseek` | DeepSeek API 钱包 | 开放平台 API Key | `deepseek-flash`、`deepseek-v4-pro` |
| 5 | `kimi` | Kimi Code 国内订阅 | Kimi Code API Key（非 Moonshot 开放平台 Key） | `k3.1-pro`、`k3.1-pro-highspeed`、`k3`、`k3-256k`、`kimi-for-coding`、`kimi-for-coding-highspeed` |
| 6 | `longcat` | LongCat 美团开放平台 | LongCat API Key（非网页登录令牌） | `LongCat-2.5` |

预设接入信息参考：[ZenMux](https://zenmux.ai/docs/guide/quickstart.html)、[GLM Coding Plan](https://docs.bigmodel.cn/cn/coding-plan/quick-start)、[MiniMax](https://platform.minimax.cn/docs/m-plan/quickstart)、[DeepSeek](https://api-docs.deepseek.com/quick_start/pricing-details-cny/)、[Kimi Code](https://www.kimi.com/code/docs/)、[LongCat](https://longcat.chat/platform/docs/zh/)。除公共目录另有声明外，默认初始值为 `128000` 上下文窗口与 `8192` 最大输出。

### 多台机器的公共模型目录

Loom 在编译期嵌入 [catalog/zenmux.json](catalog/zenmux.json)、[catalog/glm.json](catalog/glm.json) 与 [catalog/kimi.json](catalog/kimi.json)：

- 本机已配置对应官方 `baseURL` 的供应商会在运行时自动补充缺失模型，不会改写私有 `providers.json` 或密钥文件；未配置对应供应商的机器不会凭空创建 provider。
- 本机 `models` 中同 ID 条目始终优先（保留自定义名称、上下文/输出上限与 `"enabled": false` 停用项）。
- 若需完全自行维护模型列表，可在对应 provider 上设置 `"includeBundledModels": false`。

---

## OpenAI 与 Anthropic 网关 API

- **OpenAI Base URL**：`http://127.0.0.1:4319/v1`（`POST /v1/chat/completions`）
- **Anthropic Base URL**：`http://127.0.0.1:4319`（`POST /v1/messages`）
- **模型目录**：`GET /v1/models`（携带 `anthropic-version` 或 `x-api-key` 等 Anthropic 请求头时返回原生列表格式，否则返回 OpenAI 列表格式）
- **健康检查**：`GET /health`（免 Key 本机探针，返回 `{"ok":true,"service":"local-model-hub","runtime":"rust"}`）
- **请求鉴权**：使用本机入口 Key（`hub.key`）传入 `Authorization: Bearer <hub-key>`；`/v1/messages` 同时支持 `x-api-key`，若同时存在且冲突则拒绝（`401`）。
- **模型路由**：使用中央模型 ID（如 `zenmux/anthropic/claude-sonnet-5.5`）；客户端通过 `hub` provider 前缀引用。

### 协议与诊断行为

- **原样流式透传**：SSE 增量分片、思考字段、工具调用、图片、缓存参数与厂商字段均原样透传；`max_tokens`（或 `max_completion_tokens`）受中央配置 `maxTokens` 上限约束。网关限制请求体最大 `20 MiB`（`30s` 读取超时）、响应体最大 `64 MiB`、并发推理信号量上限 `8`（超出返回 `429 LOCAL_CONCURRENCY_LIMIT`）。
- **原生 Anthropic 路由**：自动映射 ZenMux（`https://zenmux.ai/api/anthropic`）、GLM（`https://open.bigmodel.cn/api/anthropic`、`https://api.z.ai/api/anthropic`）、MiniMax（`https://api.minimax.cn/anthropic`、`.minimaxi.com`、`.minimax.io`）、DeepSeek（`https://api.deepseek.com/anthropic`）、Kimi Code（`https://api.kimi.com/coding`）与 LongCat（`https://api.longcat.chat/anthropic`，按上游要求使用 Bearer 认证），自定义供应商可配置 `anthropicBaseURL`。
- **上游 402 账单分类**：保留 HTTP `402` 与 `UPSTREAM_402` 机器码，仅按白名单错误码输出固定中文说明（`subscription_quota_exhausted`、`insufficient_credit` 或 `payment_required`），不回显上游任意错误文本。
- **上游 TLS 校验**使用系统信任库和内置 Mozilla WebPKI 根证书，适用于推理请求及供应商连通性检测。系统信任库无法加载时，仍使用内置根证书校验证书与主机名。企业网络使用自有 CA 时，将根证书加入系统信任库后重启 Loom 服务。`loom doctor` 会显示命令行进程载入的系统根证书数量。Loom 进程设置 `SSL_CERT_FILE` 或 `SSL_CERT_DIR` 时，这些路径会替代默认的系统根证书来源。
- **网络传输 502 分类**：保留 HTTP `502` 与 `UPSTREAM_UNAVAILABLE`，按 `timeout`、`request_invalid`、`tls_error`、`dns_error`、`connect_failed`、`transport_failed` 输出固定诊断分类，不暴露上游 URL 或凭据。

---

## 本机可观测与全功能工作台

```sh
loom console                             # 以一次性登录票据安全打开 http://loom.localhost:4320/
loom console --page providers            # 直接打开指定分区（onboarding / providers / integrations / overview / requests / sessions / settings）
loom console --host 127.0.0.1            # 若浏览器不支持 *.localhost 可切换至回环 IP
```

管控台独立监听 `127.0.0.1:4320`（可通过 `loom serve --admin-port <端口>` 指定）。`loom console`（及 macOS `Loom.app`）使用 `hub.key` 调用 `POST /api/launch`（携带 `Origin` 头会被拒绝）换取 60 秒一次性票据，兑换为 8 小时 `HttpOnly; SameSite=Strict` 会话 Cookie（`loom_console`）。网页工作台内含七个视图：

- **新手向导（Onboarding）**：首次无可用模型时自动打开，提供 6 大预设厂商与自定义 HTTPS 厂商的分步引导、Key 申请外链、避坑说明与一键保存测通。
- **模型厂商（Providers）**：集中启停供应商、无回显轮换 Key、开关公共目录补充、启用/停用或增删单个模型，并支持指定模型执行单 Token 连通性探测与白名单错误诊断。
- **客户端接入与网络（Integrations）**：查看并一键同步 `pi`、`OpenCode V2`、`Claude Code`，复制通用 OpenAI/Anthropic 入口地址（绝不暴露 `hub.key` 明文），以及设置或关闭本机持久 HTTP/HTTPS 出站代理。
- **概览（Overview）**：支持按时间窗口（`24h` / `7d` / `30d`）、供应商、模型与 harness 筛选；展示输入/输出 Token 曲线、缓存读写分解、Token 加权缓存命中率、平均首 Token 延迟（TTFT）、请求耗时 P50 / P95 与成功/异常数。缺失 usage 保持未知（`—`），reasoning 细分不二次叠加到输出中，不将厂商余额/订阅积分混入 Token 统计。
- **请求（Requests）**：展示分阶段耗时（`authenticated`、`routed`、`dispatch`、响应头、首 Token、完成）、脱敏后的客户端请求、转发参数（已剥离凭据）、完整响应 / SSE 事件及关联会话跳转。
- **会话轨迹（Sessions）**：三栏式会话工作台，后台每 15 秒只读增量导入 `~/.pi/agent/sessions`、`~/.claude/projects` 与 `~/.local/share/opencode/opencode.db`（同时兼容 OpenCode V2 `session_v2`/`session_message` 与 V1 `message`/`part` 表结构）。支持对话回放、时间轴瀑布视图与右侧常驻步骤检查器。基于 [integrations/pi.ts](integrations/pi.ts) 与 [integrations/opencode2.ts](integrations/opencode2.ts) 注入的 `x-loom-harness` / `x-loom-session-id` 请求头或 Claude Code 原生 metadata / `response_id` 精确关联，绝不按提示词相似度猜测归属，也不把历史导入重算进网关计量。
- **采集设置（Settings）**：独立开关请求/响应正文采集与原生会话导入，配置保留期限（`1–3650` 天，默认 `30` 天）。

观测数据落盘在 `<data-dir>/observability/traces.sqlite3`（`0700` 目录下的 `0600` 文件），入库前脱敏已知密钥与常见 token 模式，单条响应采集上限为 `16 MiB`（超出后标记截断但不影响原始推理透传）。完整规范见 [docs/observability.md](docs/observability.md)；独立前端会话工作台原型见 [prototypes/session-workspace/README.md](prototypes/session-workspace/README.md)。

---

## 中央配置与 CLI 命令

默认数据目录（可通过 `--data-dir <路径>` 或环境变量 `MODEL_HUB_DATA_DIR` 覆盖）：

- **macOS**：`~/Library/Application Support/ModelHub/.local`
- **Linux**：`~/.local/share/ModelHub/.local`

`providers.json` 示例：

```json
{
  "proxy": "",
  "providers": {
    "local-demo": {
      "prefix": "demo",
      "name": "My provider",
      "baseURL": "https://provider.example/v1",
      "anthropicBaseURL": "https://provider.example/anthropic",
      "billing": "独立 API 钱包",
      "enabled": true,
      "keyFile": "/absolute/path/to/local/key",
      "models": [
        {
          "id": "model-id",
          "name": "Model",
          "contextWindow": 128000,
          "maxTokens": 8192,
          "input": ["text"],
          "reasoning": true
        }
      ]
    }
  }
}
```

### 代理与密钥管理命令

```sh
loom proxy                                # 查看当前出站模式（直连或已配置持久代理）
loom proxy http://127.0.0.1:7890          # 设置持久 HTTP/HTTPS 代理（下一条请求即生效）
loom proxy off                            # 恢复直连
loom set-key local-demo                   # 按准确供应商 ID 隐藏输入轮换 Key（自动备份旧 Key）
loom check                                # 校验 providers.json 并确认所有 keyFile 存在
```

Loom 在构建上游 HTTP 客户端时显式调用 `.no_proxy()`，**不读取**终端 `HTTP_PROXY` / `HTTPS_PROXY` 环境变量，确保自启动守护进程不依赖当前 Shell 环境。`loom proxy` 会拒绝携带用户名、密码、查询参数、fragment、自定义路径或非 HTTP(S) 协议的地址，且不在终端回显完整代理 URL。

---

## 客户端同步与思考档位兼容

运行 `loom sync`（或在 `loom serve` 运行期间修改 `providers.json`，后台每 2 秒自动触发同步）会在备份旧配置至 `<data-dir>/backups/` 后更新以下客户端：

- **pi**（`~/.pi/agent/models.json`、`~/.pi/agent/settings.json`）与会话关联扩展 `~/.pi/agent/extensions/loom-observability.ts`（[integrations/pi.ts](integrations/pi.ts)，基于 `before_provider_headers`）。
- **OpenCode V2**（`~/.config/opencode/opencode.json` 或无注释的 `~/.config/opencode/opencode.jsonc`；若 `opencode.jsonc` 含注释或与 `opencode.json` 并存则拒绝覆盖）与插件 `~/.config/opencode/plugins/loom-observability/index.ts`（[integrations/opencode2.ts](integrations/opencode2.ts)，基于 `ctx.session.hook("http.request")`）。
- **Claude Code**（仅当 `~/.claude/settings.json` 已存在时增量更新，保留主题、权限模式与其他无关配置）。

### DeepSeek 与 LongCat 思考控制适配

客户端连接本机回环地址时，其内置的 URL 厂商识别无法直接感知真实上游。`loom sync` 会根据 `providers.json` 中的实际 `baseURL` 写入准确能力声明：

- **DeepSeek（`api.deepseek.com`）**：为 `pi` 写入 `compat.thinkingFormat: "deepseek"` 与 `thinkingLevelMap`（`minimal -> low`、`low -> low`、`medium/high/xhigh -> high`、`max -> max`）；为 OpenCode V2 写入 `capabilities.reasoning` 对象及 `none` / `low` / `high` / `max` 四档 `variants`（显式切换 `thinking.type` 为 `"disabled"` / `"enabled"`），不覆盖用户默认档位。自定义 `piCompat` 与模型级 `compat` / `thinkingLevelMap` 继续生效。
- **LongCat（`api.longcat.chat`）**：预设写入 `piCompat: {"thinkingFormat": "deepseek", "supportsReasoningEffort": false}`，使 `pi` 仅切换 `thinking.type`（`enabled`/`disabled`），不发送未声明支持的 `reasoning_effort`。

可使用本机模拟服务验证真实 `pi-ai` SDK 的出站思考参数（不接触真实 Key 与上游）：

```sh
node tests/pi-thinking.mjs target/debug/loom /path/to/@earendil-works/pi-ai
```

---

## 安全模型

- **目录与文件权限**：私有数据目录强制为 `0700`，`hub.key`、`providers.json`、上游 Key 文件、备份与 `observability/traces.sqlite3` 均以原子写入创建为 `0600`（SQLite 使用 `PRAGMA journal_mode=DELETE; PRAGMA secure_delete=ON`）。
- **Host 与 Origin 隔离**：
  - 推理网关（`4319`）仅允许回环 `Host`（`127.0.0.1:4319` / `localhost:4319`），拒绝普通浏览器 `Origin` 与 `sec-fetch-site: cross-site` 请求，仅允许携带本机 `hub.key` 的 `chrome-extension://` 与 `moz-extension://` 扩展来源。
  - 管控台（`4320`）强制精确 `Host`（`loom.localhost:4320`、`localhost:4320`、`127.0.0.1:4320`）与严格同源（`Origin == http://<Host>`），不开放 CORS，启用严格 CSP（`default-src 'self'; script-src 'self'; ...`）与 `X-Frame-Options: DENY`。
- **上游安全约束**：仅接受无凭据、无查询参数与无 fragment 的 `https://` 上游地址，禁用 HTTP 重定向（`Policy::none()`）。上游 Key 永不写入客户端配置、遥测数据库或终端标准输出。

---

## 验证与生态集成

```sh
cargo test --locked
cargo fmt --check
cargo clippy --locked -- -D warnings
```

在周边工具生态中，Loom 作为统一的本机模型网关，为 [`verba`](https://github.com/xdlkc/verba)、[`gloss`](https://github.com/xdlkc/gloss) 与 [`llm-texas-holdem`](https://github.com/xdlkc/llm-texas-holdem) 等应用提供本机推理入口。
