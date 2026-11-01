# Loom

包含 Rust 模型统一网关和本机可观测管控台。**不包含额度面板、浏览器扩展、账号用量采集或默认放行脚本。** 不需要 Node.js。

统一访问 OpenAI Chat Completions 与 Anthropic Messages；上游 Key 保存在本机，agent 只引用入口 Key。固定回环监听，拒绝浏览器跨站请求，不跨 provider 回退、不自动重试生成。

## 安装与启动

从源码构建需要 [Rust 工具链](https://rustup.rs/)。安装、自启动和运行均由 Rust 完成，**不需要 Python 或 Node.js**：

```sh
./setup.sh        # 首次交互安装自动进入供应商向导，然后安装并启用自启动
loom status
```

首次安装只需回车选择全部，再逐项粘贴 Key（隐藏输入）；没有 Key 就回车跳过。名称、Base URL、计费来源和模型列表自动填好，随后自动同步 pi / opencode2 及已有 Claude Code 设置。已有供应商配置时，安装与更新不再询问 Key，也不会套用预设覆盖原配置。非交互安装不启动向导，安装后在本机终端运行 `loom add`。

## 添加供应商 / 更换 Key

```sh
loom add                       # 默认全部；也可输入 1,4 等编号多选
loom add zenmux deepseek        # 只询问这两家的 Key
loom add kimi                  # 已配置时，输入新 Key 更换；回车保留
```

`loom configure` 是 `loom add` 的别名。不用填写名称、地址、模型 ID；输入的 Key 不显示、不放进命令参数，不接受管道传入。每家保存后立即落盘，未填 Key 的新供应商不会创建；已存在的供应商只轮换 Key，保留模型、地址、名称、启用状态及其他设置。旧 Key 仅在本机私密备份。重新运行向导可以继续未完成的配置。

| 编号 | 供应商 | 需要的 Key | 自动配置的模型 |
| --- | --- | --- | --- |
| 1 | ZenMux | 推理 API Key，非 Management Key | Claude 5.5 系列 + 公共目录中的 Muse Wonder、Ling 3.1 Flash、MiMo V2.6 Flash / Pro |
| 2 | GLM 国内 Coding Plan | 该套餐的 API Key | GLM 5.3 / Flash / FlashX |
| 3 | MiniMax 国内订阅 | Subscription Key | MiniMax M3.1 Flash |
| 4 | DeepSeek API 钱包 | 开放平台 API Key | DeepSeek Flash / V4 Pro |
| 5 | Kimi Code 国内订阅 | Kimi Code API Key，非 Moonshot Key | K3.1 Pro / Highspeed |

预设是公共接入信息，不代表账号已获所有模型权限；模型权限仍由上游套餐决定，不自动发送付费测试请求。上下文 128K、最大输出 8K、文本输入是保守初始设置，可在中央配置中调整。不同套餐不混用、不跨供应商回退。预设来源：[ZenMux](https://zenmux.ai/docs/guide/quickstart.html)、[GLM Coding Plan](https://docs.bigmodel.cn/cn/coding-plan/quick-start)、[MiniMax](https://platform.minimax.cn/docs/m-plan/quickstart)、[DeepSeek](https://api-docs.deepseek.com/quick_start/pricing-details-cny/)、[Kimi Code](https://www.kimi.com/code/docs/)。

其他供应商、海外站点或 API 钱包独立接入仍可手动配置：

```sh
loom add --custom              # 高级配置；显示名称默认采用供应商标识
loom set-key local-zenmux       # 按准确的供应商 ID 隐藏输入更换 Key
```

无自启动需求时，直接运行 Rust 二进制：

```sh
cargo build --release --locked
./target/release/loom init
./target/release/loom add
./target/release/loom serve
```

服务安装器只操作 `local.model-access.hub`；不会启停额度工具。已有网关配置直接沿用：macOS `~/Library/Application Support/ModelHub/.local`；Linux `~/.local/share/ModelHub/.local`。可通过 `--data-dir` 或 `MODEL_HUB_DATA_DIR` 指定独立目录。升级前备份二进制和启动项。Key 0600、私有目录 0700；不要上传此目录或备份。

## API

- OpenAI Base URL：`http://127.0.0.1:4319/v1`，`POST /chat/completions`。
- Anthropic Base URL：`http://127.0.0.1:4319`，`POST /v1/messages`。
- `GET /v1/models`：模型目录；Anthropic 请求头返回其原生列表格式。
- `GET /health`：无需 Key 的本机健康检查，包含 `runtime: rust`。
- 请求凭据：入口 Key 的 `Authorization: Bearer ...`；Messages 也支持 `x-api-key`，冲突凭据拒绝。
- 请求模型使用 `zenmux/anthropic/claude-sonnet-5.5` 等中央 ID；agent 的 `hub/` 是客户端 provider 前缀。

SSE 增量透传；思考、工具调用、图片、缓存参数和厂商字段原样透传，不转换思考强度。`max_tokens` 受中央模型输出上限约束。上游不支持的参数可能报错。仅支持上述推理接口，不声明支持整个 Anthropic 平台或自动审批服务。

原生 Anthropic 路由已映射 ZenMux、GLM Coding Plan、MiniMax、DeepSeek、Kimi Code；自定义 provider 可配置 `anthropicBaseURL`。只接受 HTTPS 上游、不跟随重定向；请求正文可按采集设置保存于本机私有数据库；Key 与认证头不进入观测数据，也不写程序输出。每次请求重新读取配置；本机配置变化时自动更新客户端目录。运行中的客户端可能需重启或刷新列表。

## 中央配置

`providers.json` 格式沿用此前网关：

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
      "models": [{"id": "model-id", "name": "Model", "contextWindow": 128000, "maxTokens": 8192, "input": ["text"]}]
    }
  }
}
```

`proxy` 留空直连；需要代理时在本机配置 URL。不要把 Key 放进 URL。供应商向导只使用公共预设，不携带任何账号的预填 Key、真实套餐、消费记录或模型权限保证。Management Key 不能用于模型推理。

`loom set-key local-demo` 隐藏输入轮换 Key。`loom check` 检查配置及 Key 文件是否存在，不验证推理权限。`sync` 保留无关客户端配置、OAuth 和权限模式，修改前私密备份；不强制改变既有默认模型或接管其他 provider。

## 多台机器的模型目录

从 0.6.0 起，[catalog/zenmux.json](catalog/zenmux.json) 随 Loom 一起发布公共模型目录。已有 ZenMux provider（`https://zenmux.ai/api/v1`）在运行时自动补充这些模型；没有配置 ZenMux 的机器不会自动创建 provider。私有 `providers.json` 和 Key 文件仍留在各台机器，不进入仓库，也不会因为更新公共目录而被改写。

其他机器执行 `loom update` 后，新网关会自动把有效目录同步到 pi / opencode2 和已有 Claude Code 设置。不必重新输入 Key 或重新添加 provider；已经打开的 agent 需要退出重开，以加载更新后的模型列表。手动运行 `loom sync` 也使用同一目录，旧版 `loom sync` 只同步本机旧配置，不能获取新版目录。

目前公共目录补充 `meta/muse-wonder-1.0`、`inclusionai/ling-3.1-flash`、`xiaomi/mimo-v2.6-flash` 和 `xiaomi/mimo-v2.6-pro`，均已核对 ZenMux 的 Chat Completions / Messages 适配。来源：[ZenMux 公共目录](https://zenmux.ai/api/v1/models)、[模型及协议目录](https://zenmux.ai/api/frontend/model/available/list)。Clef Omni 只支持 Decisions API，当前不作为聊天模型发布。

本机 `models` 中同 ID 的条目始终优先，名称、上下文、输出上限、停用状态和默认模型均保留。隐藏一个公共模型时，在该 provider 的 `models` 中加入相应 ID 和 `"enabled": false`；只从本机数组删除条目会重新显示公共默认值。若希望完全自行维护该供应商的模型列表，在 provider 上设置 `"includeBundledModels": false`。自定义端点不自动套用 ZenMux 公共目录。

## 验证与发布范围

```sh
cargo test --locked
cargo fmt --check
cargo clippy --locked -- -D warnings
```

本仓库早期历史曾包含额度工具；当前分支只分发 Rust 网关。旧源码可从历史提交恢复，未重写远端历史；本机额度服务保持独立。Rust 版可在独立端口/配置目录验证后，再用安装器迁移运行服务。

项目与命令名称为 **Loom**，仓库为 `xdlkc/loom`。已有 `modelhub` 命令作为兼容别名保留；`hub/` 模型前缀、4319 端口和历史 ModelHub 配置目录不改名，避免现有 agent 和密钥路径失效。

## Rust 安装器

```sh
loom install                       # 当前二进制安装到用户目录并配置自启动
loom install --no-start            # 只部署及生成启动项，不操作正在运行的服务
```

旧版 Loom 没有 `install` 子命令时，先在源码目录 `git pull --ff-only`，再运行 `./setup.sh`。已构建的独立二进制无需 Rust 工具链即可执行 `install`。安装器使用操作系统自带的 macOS `launchctl` 或 Linux `systemctl --user`，不需要管理员权限；Linux 要有可用的 systemd 用户会话。

安装器保留现有 `providers.json`、入口 Key、上游 Key、客户端设置和 OAuth；配置有误时在停止旧服务前退出。覆盖前将程序、启动项、命令链接信息备份到本机私有目录；启动项加载或健康检查失败会尝试恢复旧文件及原服务，并报告备份位置。首次安装失败会撤销新服务。恢复也失败时需根据该私密备份手动处理，不能承诺系统服务管理器故障时自动恢复一定成功。

macOS 安装会等待旧任务卸载再重试加载，避免错误 5 的清理竞态。只操作网关启动项 `local.model-access.hub`，不操作额度服务。

0.3.0 起安装器为纯 Rust。测试包含 macOS/Linux 启动项生成、卸载等待与重试、安装失败恢复、空 PATH 安装、对已安装二进制自身升级以及原配置/Key 字节不变。Linux systemd 真机启动仍需在目标机器确认。

## 一条命令更新

从 0.4.0 开始：

```sh
loom update
```

自动从 `https://github.com/xdlkc/loom.git` 的 `main` 获取源码，使用 `cargo build --release --locked` 构建，然后由新二进制执行 `loom install`，备份旧版本并重启服务。保留 provider 配置、入口 Key、上游 Key、客户端配置和 OAuth；安装器的健康检查失败时尝试恢复旧版本。更新前后对配置和 Key 做本机内容比对，不打印凭据。

需要本机已安装 Git 和 Rust 1.89+，并具有私有仓库访问权限（GitHub CLI 用户可先执行 `gh auth login`、`gh auth setup-git`）。构建需要访问 crates.io；自动更新不会安装系统依赖，也不需要 Python / Node.js。依赖缺失或网络/构建失败时旧服务保持运行，终端会给出失败阶段。

源码临时目录和编译缓存位于私有配置目录的 `updates/` 中，不使用或修改原来的 Git 工作副本。临时源码在成功或失败后清理，编译缓存保留以加快后续更新；独占文件锁防止同时更新，进程结束后自动释放。只有构建完成且配置/Key 未发生变化时才开始替换；本机设置正在编辑时可能中止更新，重试即可。

0.3.x 或更早版本还没有 `update` 命令，需要最后手动升级一次：

```sh
cd /path/to/your/loom-checkout
git remote set-url origin https://github.com/xdlkc/loom.git
git pull --ff-only
./setup.sh
```

此后可在任意目录直接运行 `loom update`。


## 本机可观测与会话管控（0.7.0）

```sh
loom update                 # 更新并重启；已有配置与密钥保留
loom console                # 安全打开本机管控台，不输出入口 Key 或登录票据
```

网关仍在 `127.0.0.1:4319`，管控台使用独立的 `127.0.0.1:4320`，浏览器固定入口为 **http://loom.localhost:4320/**。运行 `loom console` 会打开该域名并安全登录；Chrome 中无需修改 hosts、系统 DNS 或申请域名，服务仍仅监听本机。首次访问、登录过期或服务重启后，请重新运行 `loom console`；浏览器以一次性票据换取 HttpOnly / SameSite 会话，刷新页面不必重新输入 Key。若浏览器不支持该本地域名，可运行 `loom console --host 127.0.0.1` 使用原地址。不开放 CORS，不支持公网监听，不改变现有 agent 审批模式。

- **概览**：供应商 / 模型 / harness / 时间筛选，输入与输出 Token 曲线、缓存读写、Token 加权命中率、首 Token 延迟、请求耗时 P50 / P95、成功与异常。缺失 usage 显示未知；reasoning 是输出细分，不二次相加。厂商订阅剩余百分比、API 钱包余额与真实 Token 消耗不混用，也不将未知美元费用显示为零。
- **请求**：每个调用的接收、认证、路由、请求准备、上游响应、首字节、首 Token、完成阶段；客户端请求、上游转发参数、响应 / SSE、错误、取消与不完整状态。点击关联会话可继续下钻。
- **会话轨迹**：只读导入 pi / Claude Code 的 JSONL，以及 opencode2 V2 的 SQLite（兼容 V1 message / part）；保留消息、工具输入、工具结果、分支 parent ID、压缩与其他原生事件。支持搜索与分页；完整事件 JSON 可展开。原生文件未保留的会话只能展示网关请求，不能恢复已丢失的工具记录。历史导入不会混入全局网关计量，避免重复计数。
- **采集设置**：分别控制请求 / 响应正文和原生会话导入，保留期默认为 30 天（1–3650 天），后台每 15 秒检查原生轨迹并清理到期副本。清理不修改 harness 原始文件。关闭正文采集只影响之后的请求，已有副本按保留期清理。

`loom sync` / 网关启动会部署仅用于关联的本机 pi 扩展和 opencode2 V2 插件，并保留原有插件、OAuth、默认模型及权限设置。pi 使用官方 `before_provider_headers`；OpenCode V2 使用 `ctx.session.hook("http.request")`；只向本机 hub 请求附加 session 标识，网关不再转发这些标识。Claude Code 从原生请求 metadata / transcript 响应 ID 精确关联。导入默认路径为 `~/.pi/agent/sessions`、`~/.claude/projects`、`~/.local/share/opencode/opencode.db`（或 `opencode2`）；当前用户自定义的其他存储根目录不自动扫描。

观测数据位于原私有数据目录下 `observability/traces.sqlite3`（文件 0600、目录 0700）。完整轨迹可能包含代码、提示词与工具输出；只保存在本机，先移除已知入口 / 上游凭据、认证字段及常见 token 模式，不上传仓库或厂商。程序输出只记录安全状态。正文的观测副本上限为每响应 16 MiB；超过上限显式标注截断，不影响原始推理字节透传。会话和请求不会凭提示词相似度猜测关联。

从源码构建 SQLite bundled 依赖需要 C 编译器（macOS Xcode Command Line Tools，Linux `cc`）；Loom 安装与运行仍不需要 Python / Node.js。内置 TS 适配器由对应 harness 自己加载。调研、指标口径和验收范围见 [docs/observability.md](docs/observability.md)。
