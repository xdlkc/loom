# 本机 agent 默认权限

已按用户要求配置日常命令：

- `kimi`：启动时附加 `--auto`（Never Ask）。登录、升级、服务等管理子命令仍调用原程序。
- `opencode2` 和 `opencode2 run`：附加 `--auto`，自动批准未明确拒绝的操作。显式 deny 规则仍生效；mini、ACP、后台服务等子命令未改。
- `agy`：启动时附加 `--dangerously-skip-permissions`；管理子命令仍调用原程序。
- `pi`：本身没有内置逐次工具审批，未修改。项目信任与扩展自行实现的确认不在此次设置范围。

配置以 `~/.local/bin` 启动包装脚本实现，不改上游安装包或凭据。原目标及恢复链接在 `~/.local/libexec/agent-defaults/manifest.json`。agy 原二进制保存于同目录，其他工具仍调用原 npm 安装路径。包装脚本已做 shell 语法检查，原程序版本和帮助可正常运行；未执行安装或破坏性命令来验证放行。

重开 CLI 会话生效。升级工具可能重建 `~/.local/bin` 启动项，需要重新核对默认参数。`install.py` 只用于首次安装，发现已有 manifest 时停止，以免覆盖恢复信息。
