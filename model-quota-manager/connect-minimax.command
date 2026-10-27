#!/bin/zsh
cd "$(dirname "$0")" || exit 1
print 'MiniMax Coding Plan / Token Plan 订阅额度接入（不是普通 API 钱包）。'
print '请使用订阅套餐页面提供的 Subscription / Token Plan Key。'
print '密钥输入隐藏；回车后选择地区：国内输入 cn（或直接回车），国际输入 global。'
node configure.mjs minimax
if [[ $? -eq 0 ]]; then
  node verify-minimax.mjs
fi
print '\n按回车关闭窗口。'
read
