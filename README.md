# ModelHub · Rust 本机统一模型网关

只包含模型统一网关。**不包含额度面板、浏览器扩展、账号用量采集或默认放行脚本。** 不需要 Node.js。

统一访问 OpenAI Chat Completions 与 Anthropic Messages；上游 Key 保存在本机，agent 只引用入口 Key。固定回环监听，拒绝浏览器跨站请求，不跨 provider 回退、不自动重试生成。

## 安装与启动

需要 [Rust 工具链](https://rustup.rs/)。macOS / Linux 的用户服务安装器额外需要 Python 3：

```sh
./setup.sh
modelhub add          # 交互添加 provider，API Key 隐藏输入
modelhub sync         # 导出 pi / opencode2，及已有 Claude Code 设置
modelhub status
```

无自启动需求时，直接运行 Rust 二进制：

```sh
cargo build --release --locked
./target/release/modelhub init
./target/release/modelhub add
./target/release/modelhub serve
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

`proxy` 留空直连；需要代理时在本机配置 URL。不要把 Key 放进 URL。`modelhub add` 默认文本、128K 上下文、8K 输出，按厂商实际能力修改。不携带任何账号的预填 Key、真实套餐、消费记录或模型权限保证。Management Key 不能用于模型推理。

`modelhub set-key local-demo` 隐藏输入轮换 Key。`modelhub check` 检查配置及 Key 文件是否存在，不验证推理权限。`sync` 保留无关客户端配置、OAuth 和权限模式，修改前私密备份；不强制改变既有默认模型或接管其他 provider。

## 验证与发布范围

```sh
cargo test --locked
cargo fmt --check
```

本仓库早期历史曾包含额度工具；当前分支只分发 Rust 网关。旧源码可从历史提交恢复，未重写远端历史；本机额度服务保持独立。Rust 版可在独立端口/配置目录验证后，再用安装器迁移运行服务。
