#!/bin/sh
set -eu
cd "$(dirname "$0")"
command -v python3 >/dev/null 2>&1 || { echo '需要 Python 3，请先安装。'; exit 1; }
exec python3 scripts/setup.py "$@"
