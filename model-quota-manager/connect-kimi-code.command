#!/bin/zsh
cd "$(dirname "$0")" || exit 1
print 'Kimi Code / 会员订阅接入：官方 OAuth 登录，无需 Moonshot API Key。'
node setup-kimi-code.mjs
print '\n按回车关闭窗口。'
read
