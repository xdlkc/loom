#!/bin/zsh
cd "$(dirname "$0")" || exit 1
print 'GLM Coding Plan 订阅配额接入（不是 API 按量钱包余额）。'
print '请在智谱个人编程套餐页面创建或复制套餐 API Key。团队订阅请先告知。'
print '原始 Key 隐藏输入，不加 Bearer，不要发到聊天或放进命令参数。'
node configure.mjs glm
if [[ $? -eq 0 ]]; then
  node verify-glm.mjs
fi
print '\n按回车关闭窗口。'
read
