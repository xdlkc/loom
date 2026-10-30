#!/bin/sh
set -eu
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null 2>&1 || { echo '需要 Rust 工具链：https://rustup.rs/ 。安装后重新执行 ./setup.sh'; exit 1; }
cargo build --release --locked
exec ./target/release/loom install "$@"
