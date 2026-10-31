# Loom

只包含模型统一网关。**不包含额度面板、浏览器扩展、账号用量采集或默认放行脚本。** 不需要 Node.js。

统一访问 OpenAI Chat Completions 与 Anthropic Messages；上游 Key 保存在本机，agent 只引用入口 Key。固定回环监听，拒绝浏览器跨站请求，不跨 provider 回退、不自动重试生成。

## 安装与启动

从源码构建需要 [Rust 工具链](https://rustup.rs/)。安装、自启动和运行均由 Rust 完成，**不需要 Python 或 Node.js**：

```sh
./setup.sh           # cargo build --release --locked，再执行 loom install
loom add          # 交互添加 provider，API Key 隐藏输入
loom sync         # 导出 pi / opencode2，及已有 Claude Code 设置
loom status
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

原生 Anthropic 路由已映射 ZenMux、GLM Coding Plan、MiniMax、DeepSeek、Kimi Code；自定义 provider 可配置 `anthropicBaseURL`。只接受 HTTPS 上游、不跟随重定向；请求正文和 Key 不写日志。每次请求重新读取配置；本机配置变化时自动更新客户端目录。运行中的客户端可能需重启或刷新列表。

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

`proxy` 留空直连；需要代理时在本机配置 URL。不要把 Key 放进 URL。`loom add` 默认文本、128K 上下文、8K 输出，按厂商实际能力修改。不携带任何账号的预填 Key、真实套餐、消费记录或模型权限保证。Management Key 不能用于模型推理。

`loom set-key local-demo` 隐藏输入轮换 Key。`loom check` 检查配置及 Key 文件是否存在，不验证推理权限。`sync` 保留无关客户端配置、OAuth 和权限模式，修改前私密备份；不强制改变既有默认模型或接管其他 provider。

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
