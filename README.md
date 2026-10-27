# 本机模型额度与统一入口

本机运行的模型额度面板、浏览器同步扩展和统一模型网关。支持 OpenAI Chat Completions 与 Anthropic Messages，通过中央 provider 配置连接各 agent。

- `model-quota-manager/`：网页、定时查询、Token 曲线、统一浏览器扩展、macOS 自启动安装器。
- `model-access/`：本机统一入口、中央 provider 管理、pi / opencode2 配置生成与协议测试。
- `agent-defaults/`：可选的 CLI 默认审批模式启动器；按需使用。

运行需要 Node.js 24+，部分安装及隐藏输入流程需要 Python 3。进入 `model-quota-manager` 后执行 `npm test`、`npm start`；入口运行 `node model-access/hub.mjs`。首次使用须在本机另行配置账号和 Key，仓库不携带可用凭据或个人额度。

各子目录 README 说明配置与使用方式。macOS 安装器会根据当前用户生成 LaunchAgent，无需提交机器专用 plist。默认仅监听本机回环地址。

## 数据边界

凭据、OAuth 状态、provider 实例配置、额度快照、浏览器账号绑定、日志、截图与备份均不纳入版本控制。测试中的邮箱、Key、账号标识均为虚构夹具。不要提交运行时 `.local` 目录或用户 agent 配置。
