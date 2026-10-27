#!/bin/zsh
cd "$(dirname "$0")" || exit 1
print 'DeepSeek API 钱包接入：仅查询可用总余额、未过期赠金和充值余额。'
print '请在 platform.deepseek.com 的 API Keys 页面创建或复制 API Key。'
print '在下方隐藏输入原始 Key，不加 Bearer；不要发到聊天或放入命令参数。'
node configure.mjs deepseek
if [[ $? -eq 0 ]]; then
  node verify-deepseek.mjs
fi
print '\n按回车关闭窗口。'
read
