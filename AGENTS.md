# Repository scope

This repository distributes only the standalone Rust model gateway. Do not add quota dashboards, browser extensions, personal usage data, or agent approval-mode launchers.

- Keep upstream credentials, local hub tokens, OAuth, provider instances and backups outside Git.
- Preserve the existing local providers.json format and separate billing routes. Do not automatically retry generation or fall back across providers.
- Bind only to loopback. Validate local Host and credentials; reject browser Origin requests. HTTPS upstreams only, without credential-bearing URLs or redirects.
- Preserve client preferences, native OAuth, and approval modes during sync. Never export upstream keys to clients.
- Run cargo test --locked, cargo fmt --check, and cargo clippy --locked -- -D warnings for gateway changes.
- Do not replace a user's running Node gateway or modify a quota service merely by editing this checkout. Rust service migration is explicit via setup.sh.
