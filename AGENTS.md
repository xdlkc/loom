# Repository scope

This repository distributes the Rust model gateway and its local observability / session management console. Do not add external quota dashboards, browser extensions, committed personal usage data, or agent approval-mode launchers.

- Keep upstream credentials, local hub tokens, OAuth, provider instances and backups outside Git.
- Preserve the existing local providers.json format and separate billing routes. Do not automatically retry generation or fall back across providers.
- Bind only to loopback. Validate local Host and credentials; reject browser Origin requests. HTTPS upstreams only, without credential-bearing URLs or redirects.
- Preserve client preferences, native OAuth, and approval modes during sync. Never export upstream keys to clients.
- Run cargo test --locked, cargo fmt --check, and cargo clippy --locked -- -D warnings for gateway changes.
- Do not replace a user's running Node gateway or modify a quota service merely by editing this checkout. Rust service migration is explicit via setup.sh.

- The user authorized full local session traces for pi, opencode2 and Claude Code. Redact credentials before persistence; keep the private SQLite database, transcript imports and backups outside Git. Never upload trace bodies.
- Preserve byte-for-byte inference streaming. Metrics must distinguish unknown usage, partial streams, cache read/write and transport failures; do not double-count imported harness events as gateway usage.
