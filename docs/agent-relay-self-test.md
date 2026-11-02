# Agent Relay 自动分派验证

本文档用于 Agent Relay 的自动分派端到端自检，确认 Issue / PR 分派链路可用。

服务在本机 agent 上执行任务，将进度与结果回写到 GitHub 状态，并生成草稿 PR 供审阅；PR 是否合并由维护者决定，控制器不会自行合并到 main。

自检完成后会关闭本次 PR 与对应 Issue，不在仓库中留下变更。
PR head SHA 已固定并核对。
