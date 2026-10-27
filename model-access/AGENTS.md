本目录是本机统一模型入口的可查看源码；运行版本和私密数据在 ~/Library/Application Support/ModelHub。

- 先读 README.md。所有 provider 只在中央 .local/providers.json 管理；上游 Key 在 .local/keys，本机入口 Key 在 .local/hub.key。
- 日常新增 provider 用 modelhub add，隐藏输入 API Key；入口热加载并自动更新 pi / opencode2。
- 不打印 Key、OAuth 或 Cookie；不把额度查询 Management Key 配成模型调用 Key；公开打包不得包含 .local 或客户端私密备份。
- 导出用 modelhub sync，不再生成逐个上游 provider 的客户端直连配置。
- 不改原生 OAuth 凭据。默认及新增 API 模型通过 hub；明确保留每条路由计费来源，不做跨 provider 回退或自动重试生成。
- 修改源码后，需显式同步所改运行文件到 Application Support 并重启 local.model-access.hub，不能只改本目录就宣称已部署。
- 验证使用必要的小请求，记录状态，不记录完整上游响应或凭据。
