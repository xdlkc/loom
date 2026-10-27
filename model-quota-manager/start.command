#!/bin/zsh
cd "$(dirname "$0")"
if ! command -v node >/dev/null 2>&1; then
  print '需要 Node.js 24+。请从已配置 Node 的终端执行 npm start。'
  read
  exit 1
fi
(sleep 1; open http://127.0.0.1:4318) &
node server.mjs
