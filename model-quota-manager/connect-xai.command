#!/bin/zsh
cd "$(dirname "$0")" || exit 1
print 'xAI API 账单接入：不是 Grok 网页 / X 订阅额度。'
print '请准备 Settings → Management Keys 创建的管理 Key，以及同一团队的 Team ID。'
print '两项均隐藏输入；不要粘贴普通推理 Key，也不要把认证发到聊天。'
node configure.mjs xai
if [[ $? -eq 0 ]]; then
  node verify-xai.mjs
fi
print '\n按回车关闭窗口。'
read
