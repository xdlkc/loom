#!/bin/zsh
cd "$(dirname "$0")" || exit 1
print 'ZenMux 本机接入：请在 Chrome 创建 Management API Key，然后复制到这里。'
print '密钥输入隐藏；不要把密钥发到聊天，也不要将其放入命令。'
node configure.mjs zenmux
if [[ $? -eq 0 ]]; then
  node verify-zenmux.mjs
fi
print '\n窗口可保留查看查询结果。按回车关闭。'
read
