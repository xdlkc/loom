# 本机模型额度与统一入口

本机运行的模型额度面板、浏览器同步扩展和统一模型网关。支持 OpenAI Chat Completions 与 Anthropic Messages，通过中央 provider 配置连接各 agent。

- `model-quota-manager/`：网页、定时查询、Token 曲线、统一浏览器扩展、macOS 自启动安装器。
- `model-access/`：本机统一入口、中央 provider 管理、pi / opencode2 配置生成与协议测试。
- `agent-defaults/`：可选的 CLI 默认审批模式启动器；按需使用。

运行需要 Node.js 24+，部分安装及隐藏输入流程需要 Python 3。进入 `model-quota-manager` 后执行 `npm test`、`npm start`；入口运行 `node model-access/hub.mjs`。首次使用须在本机另行配置账号和 Key，仓库不携带可用凭据或个人额度。

各子目录 README 说明配置与使用方式。macOS 安装器会根据当前用户生成 LaunchAgent，无需提交机器专用 plist。默认仅监听本机回环地址。

## 数据边界

凭据、OAuth 状态、provider 实例配置、额度快照、浏览器账号绑定、日志、截图与备份均不纳入版本控制。测试中的邮箱、Key、账号标识均为虚构夹具。不要提交运行时 `.local` 目录或用户 agent 配置。

## 新机器一键初始化

支持 **macOS / Linux（systemd 用户会话）**；Windows 可在 WSL2 的 Linux 环境使用。先安装 Git 和 Python 3。已有 Node.js 24+ 会直接复用；缺失或版本过低时，向导从 nodejs.org 下载官方 Node 24（含 npm），校验 SHA-256 后安装到工具专用目录，不替换系统 Node。无需 Rust、数据库或额外 npm 项目依赖。

```sh
git clone https://github.com/xdlkc/model-quota-manager.git
cd model-quota-manager
./setup.sh
```

macOS 也可双击 `setup.command`。私有仓库拉取需要自己的 GitHub 授权。

向导会：

1. 检查依赖，将源码部署到当前用户私有目录；不从仓库复制其他机器的运行数据。
2. 选择模型 provider：ZenMux（仅 Claude）、GLM Coding Plan、MiniMax 国内订阅、DeepSeek API、Kimi Code；在本机隐藏输入对应推理 Key。可跳过，以后从网页新增。
3. 分别配置额度查询 Key：ZenMux Management Key、GLM/MiniMax 订阅 Key、DeepSeek API Key。即使同一服务商，两类 Key 仍明确分开输入；不自动拿 Management Key 推理。
4. 生成本机入口 Key，导出 pi / opencode2 配置，检测到 Claude Code 时连接本机 Anthropic 入口并更新模型菜单；保留原生登录与权限偏好。客户端软件本身需预先安装，向导不会安装或修改它们的权限模式。
5. 配置两个用户自启动服务，并检查额度网页与入口 HTTP 是否就绪。macOS 使用 LaunchAgent，Linux 使用 systemd --user；无 systemd 用户会话时可用 `--no-start` 仅部署，之后手动启动服务。

账号登录不能复制：Kimi Code OAuth 按终端给出的命令重新授权；Codex 先在官方 CLI 登录，再从网页设置连接。Grok/Gemini/MiniMax/DeepSeek 网页同步需在新机器 Chrome 加载统一扩展，并在目标账号页面绑定一次。浏览器扩展安装与站点授权由用户完成，之后后台自动同步。Gemini 目标账号无需等同 Chrome 个人资料账号。

额度页面：<http://127.0.0.1:4318/>；模型入口：`http://127.0.0.1:4319/v1`。将 `~/.local/bin` 加到 PATH 后可用 `modelhub add` / `modelhub sync` / `modelhub status`。HTTP 就绪不等于上游账号授权或模型推理验证；首次保存 Key 后，从网页检查额度结果，并在 agent 中做一次小请求确认订阅权限。

重复运行 `./setup.sh` 可更新源码并补充配置：运行数据保留，已配置 Key 轮换前确认并备份。备份也包含私密信息，只保存在该机器上。

```sh
./setup.sh --dry-run                    # 只说明计划，不改文件、不读 Key
./setup.sh --non-interactive            # 部署空服务，不询问 Key、不改 agent 配置
./setup.sh --no-start                   # 生成配置但不启动服务
python3 -m unittest discover -s tests -v
```

本向导已通过隔离目录的首次安装、重复运行、私有权限、两种启动项生成与 agent 配置保留测试；Linux systemd 真机启动尚需在目标机器验证。它不安装浏览器扩展、不自动审批、不绕过登录，也不自动发起付费推理。

专用 Node 运行时位于 `~/.local/share/model-quota-manager/node/`，服务启动项和 `modelhub` 使用其绝对路径；Kimi OAuth 的命令也使用已验证的 Node 路径。下载支持系统代理与 `HTTPS_PROXY`，不关闭 TLS 校验。离线环境可自行安装 Node 24+ 后运行，或用 `--no-install-node` 禁用自动下载。

### Node 官方源连接失败

自动模式会在官方连接失败后改用 npmmirror，并使用仓库内固定的 **Node 官方 SHA-256** 校验 v24.22.1，镜像站的校验清单不会作为信任依据。可直接选择备用源，避免等待官方连接超时：

```sh
git pull --ff-only
./setup.sh --node-source npmmirror
```

`--node-source official` 可禁止备用源。两个源都无法访问时，先配置可用 `HTTPS_PROXY`，或自行安装 Node.js 24+ 再初始化。下载继续使用 HTTPS 证书验证；校验不一致时不会解压或执行安装包。
