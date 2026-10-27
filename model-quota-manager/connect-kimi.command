#!/bin/zsh
cd "$(dirname "$0")" || exit 1
print 'Moonshot 国内开放平台 API 钱包接入（不是 Kimi Code 订阅）。'
print '请输入 platform.kimi.com 创建的开放平台 API Key；不要使用 Kimi Code Key。'
print '输入隐藏；不要把密钥发到聊天或放进命令参数。'
node configure.mjs kimi
if [[ $? -eq 0 ]]; then
  node verify-kimi.mjs
fi
print '\n按回车关闭窗口。'
read
