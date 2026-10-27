# 模型额度面板

本机网页管理 ZenMux、Kimi Code、GLM Coding Plan、MiniMax、DeepSeek、Grok、Codex 和 Gemini 的额度及官方用量。订阅额度、API 钱包及网页会员数据分别展示，不混算。

## 运行

需要 Node.js 24+ 与 Python 3。执行 `npm test` 验证，`npm start` 启动，访问 http://127.0.0.1:4318/ 。相邻的 `../model-access` 目录供统一入口管理使用。

Key 通过各 `connect-*.command` 或 `npm run configure -- <provider>` 在本机隐藏输入。具体 Key 类型见 `catalog.mjs` 与 `OFFICIAL-APIS.md`；管理 Key 不可用作模型推理 Key。Kimi Code 与 Codex 使用官方本机登录，首次使用需要自行授权。

浏览器额度和曲线同步使用 `grok-browser-extension/` 统一扩展。Chrome 加载该已解压目录，按弹窗连接目标账号页面；按需授权对应站点。账号变化会暂停同步，登录态保留浏览器，不读取 Cookie 或令牌。后台每 15 分钟检查，校验失败保留上次成功快照。网页展示最后更新时间及需要关注状态。

## 自启动

macOS 执行 `python3 install-autostart.py`，按当前用户生成 LaunchAgent，并将运行源码放入用户 Application Support。安装器不包含其他用户的登录态或 Key。卸载或停用请根据生成的 LaunchAgent 标签操作。

## 隐私

`.local/` 是运行时私有目录，保存凭据、额度快照、账号散列与日志，不提交仓库、不共享压缩包。服务只绑定回环地址，管理写操作检查 Origin 与 CSRF。不要上传个人客户端配置、运行截图或导出数据。

额度重置券只查询，不自动兑换；用量缺失不伪造为零。各平台页面和接口可能变化，请以官方控制台为准。
