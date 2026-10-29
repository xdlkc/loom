#!/bin/sh
set -eu
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo '需要 Rust 工具链：https://rustup.rs/ 。安装后重新执行 ./setup.sh'; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo '服务安装器需要 Python 3；也可直接 cargo run --release -- init / serve。'; exit 1; }
cargo build --release --locked
exec python3 scripts/install.py target/release/modelhub
