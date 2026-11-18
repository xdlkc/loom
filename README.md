# Loom

English | [简体中文](./README.zh-CN.md)

**A local-only unified Rust model gateway and loopback observability console.**

Loom routes OpenAI Chat Completions and Anthropic Messages traffic through a single loopback daemon (`127.0.0.1:4319`) while keeping upstream provider API keys on local disk (`0600` files inside a `0700` data directory). Agents only reference a local hub entry key (`hub.key`). Built entirely in Rust (`edition = "2024"`, Rust `1.89+`), Loom requires no Python or Node.js to install, run, or auto-start.

---

## Overview & Explicit Non-Goals

Loom is strictly scoped to **model routing, local client synchronization, and loopback request/session observability**:

- **Loopback-only surface**: The inference gateway binds to `127.0.0.1:4319` and the management console binds to `127.0.0.1:4320`.
- **No silent fallbacks or retries**: Different billing channels are never mixed, requests never fall back across providers, and generation is never retried automatically.
- **Explicit non-goals**: Loom **does not** include a vendor quota dashboard, account usage scraping, browser extensions, or agent auto-approval/permission-bypass launchers.
- **Compatibility naming**: The project, repository (`xdlkc/loom`), and primary binary are named **Loom** (`loom`), while `modelhub` is retained as a command symlink alias alongside the `hub/` client provider prefix, default port `4319`, and default `ModelHub` data directories so existing agents and key paths continue working without migration breakage.

---

## Quick Start (3-Minute Setup)

Building from source requires the [Rust toolchain](https://rustup.rs/) (and a C compiler such as Xcode Command Line Tools on macOS or `cc` on Linux for bundled SQLite). When running `./setup.sh` interactively without `cargo`, the script offers guided installation via official `rustup`.

### 1. Guided Install & Desktop App

```sh
./setup.sh        # Checks compiler/Rust, builds release binary, configures autostart, and builds macOS Loom.app
loom              # Launches the interactive terminal control center (status, keys, diagnostics, web/app launch)
```

Once installed, beginners can configure providers and inspect traffic from any of three unified frontends:

- **macOS Desktop App (`~/Applications/Loom.app`)**: Native menu bar `⌘` status icon and standalone `WKWebView` console window—start/stop the daemon, jump to provider setup, or run diagnostics without typing terminal commands.
- **Web Workspace (`loom console`)**: Guided 3-step onboarding wizard with preset vendor cards, official API key links, pitfall tips, one-click connectivity tests, and automatic client synchronization.
- **Interactive CLI (`loom`)**: Running `loom` with no arguments opens a menu-driven control center alongside `loom add` (hidden prompt) and `loom doctor --ping`.

Existing provider entries and key files are always preserved when reinstalling or upgrading.

### Manual Build & Foreground Run

```sh
cargo build --release --locked
./target/release/loom init
./target/release/loom add
./target/release/loom serve
```

### User Service Installer, Desktop App & One-Command Update

```sh
loom install              # Installs binary to ~/.local/bin/{loom,modelhub} and registers user autostart
loom install --no-start   # Deploys files and service unit without touching a running service
loom app                  # Builds and opens native macOS ~/Applications/Loom.app (--build-only / --rebuild / --app-dir)
loom doctor --ping        # Checks permissions, client sync state, and runs safe 1-token upstream connectivity tests
loom update               # Clones main from github.com/xdlkc/loom, builds --release --locked, verifies keys unchanged, and restarts
```

The installer uses macOS `launchctl` (`~/Library/LaunchAgents/local.model-access.hub.plist`) or Linux `systemctl --user` (`~/.config/systemd/user/local.model-access.hub.service`) under label `local.model-access.hub` without root privileges. Before overwriting, binaries, unit files, and configs are backed up privately; if health checks fail, Loom attempts rollback to the prior version. `loom update` builds inside `<data-dir>/updates/` under an exclusive lockfile (`update.lock`) without modifying your local Git checkout.

---

## Supported Providers & Bundled Catalogs

Use `loom add` (alias `loom configure`) to add preset providers or rotate keys via hidden prompt input (`--custom` adds unlisted HTTPS endpoints):

```sh
loom add                       # Interactive selection (all presets by default)
loom add zenmux deepseek        # Prompt only for selected presets
loom add kimi                  # Rotate key on an existing preset (Enter to keep current key)
loom add longcat               # Configure Meituan LongCat with hidden Open Platform key input
loom add --custom              # Configure a custom HTTPS OpenAI/Anthropic provider
```

| # | Preset ID | Provider | Required Credential | Default / Bundled Models |
| --- | --- | --- | --- | --- |
| 1 | `zenmux` | ZenMux | Inference API Key (not Management Key) | Claude 5.5 series (`claude-sonnet-5.5`, `claude-opus-5.5`, `claude-haiku-5.5`) + bundled `meta/muse-wonder-1.0`, `inclusionai/ling-3.1-flash`, `xiaomi/mimo-v2.6-flash`, `xiaomi/mimo-v2.6-pro` |
| 2 | `glm` | GLM Coding Plan | Coding Plan API Key | `glm-5.3`, `glm-5.3-flash`, `glm-5.3-flashx`, `glm-5.2`, `glm-5.1`, `glm-5-turbo`, `glm-5`, `glm-4.7`, `glm-4.6`, `glm-4.5`, `glm-4.5-air` |
| 3 | `minimax` | MiniMax Subscription | Subscription Key | `MiniMax-M3.1-Flash` |
| 4 | `deepseek` | DeepSeek API Wallet | Open Platform API Key | `deepseek-flash`, `deepseek-v4-pro` |
| 5 | `kimi` | Kimi Code Subscription | Kimi Code API Key (not Moonshot Open Platform Key) | `k3.1-pro`, `k3.1-pro-highspeed`, `k3`, `k3-256k`, `kimi-for-coding`, `kimi-for-coding-highspeed` |
| 6 | `longcat` | LongCat Open Platform | LongCat API Key (not web session token) | `LongCat-2.5` |

Preset references: [ZenMux](https://zenmux.ai/docs/guide/quickstart.html), [GLM Coding Plan](https://docs.bigmodel.cn/cn/coding-plan/quick-start), [MiniMax](https://platform.minimax.cn/docs/m-plan/quickstart), [DeepSeek](https://api-docs.deepseek.com/quick_start/pricing-details-cny/), [Kimi Code](https://www.kimi.com/code/docs/), and [LongCat](https://longcat.chat/platform/docs/zh/). Preset defaults use `128000` context window and `8192` max output tokens unless overridden in `providers.json` or the bundled catalogs.

### Bundled Multi-Machine Catalogs

Loom embeds [catalog/zenmux.json](catalog/zenmux.json), [catalog/glm.json](catalog/glm.json), and [catalog/kimi.json](catalog/kimi.json) at compile time:

- Existing providers matching those official `baseURL` endpoints automatically receive newly bundled models at runtime without modifying your private `providers.json` or key files.
- Machines without a configured provider will not auto-create one.
- Local entries with the same `id` always take precedence (preserving custom names, context/output limits, and `"enabled": false` overrides).
- Set `"includeBundledModels": false` on a provider to opt out of runtime catalog supplementation completely.

---

## OpenAI & Anthropic Gateway API

- **OpenAI Base URL**: `http://127.0.0.1:4319/v1` (`POST /v1/chat/completions`)
- **Anthropic Base URL**: `http://127.0.0.1:4319` (`POST /v1/messages`)
- **Model Directory**: `GET /v1/models` (returns Anthropic-native model list format when Anthropic headers such as `anthropic-version` or `x-api-key` are present; OpenAI list format otherwise)
- **Health Check**: `GET /health` (unauthenticated loopback probe returning `{"ok":true,"service":"local-model-hub","runtime":"rust"}`)
- **Gateway Authentication**: Local hub key (`hub.key`) via `Authorization: Bearer <hub-key>`; `/v1/messages` also accepts `x-api-key`, rejecting conflicting credentials (`401`).
- **Model Routing**: Request models by central ID such as `zenmux/anthropic/claude-sonnet-5.5` (clients under `hub` resolve `hub/<provider>/<model>`).

### Protocol & Diagnostic Behavior

- **Byte-for-byte streaming passthrough**: SSE chunks, reasoning fields, tool calls, images, cache hints, and vendor fields are forwarded untouched; `max_tokens` (or `max_completion_tokens`) is clamped to the model's configured `maxTokens` ceiling. Request bodies are capped at `20 MiB` (`30s` read timeout), responses at `64 MiB`, and concurrent in-flight inference requests at `8` semaphore permits (`429 LOCAL_CONCURRENCY_LIMIT`).
- **Native Anthropic routes**: Mapped automatically for ZenMux (`https://zenmux.ai/api/anthropic`), GLM (`https://open.bigmodel.cn/api/anthropic`, `https://api.z.ai/api/anthropic`), MiniMax (`https://api.minimax.cn/anthropic`, `.minimaxi.com`, `.minimax.io`), DeepSeek (`https://api.deepseek.com/anthropic`), Kimi Code (`https://api.kimi.com/coding`), and LongCat (`https://api.longcat.chat/anthropic`, using Bearer authentication per upstream requirement), or custom `anthropicBaseURL`.
- **Upstream `402` explanations**: Returns HTTP `402` (`UPSTREAM_402`) with safe, whitelisted category hints (`subscription_quota_exhausted`, `insufficient_credit`, or `payment_required`) without leaking raw upstream prose.
- **Upstream TLS** uses the system trust store and the bundled Mozilla WebPKI roots for inference and provider connectivity checks. If the system store cannot be loaded, Loom still uses the bundled roots to verify certificates and hostnames. If your network uses a corporate CA, add it to the system trust store and restart the Loom service. `loom doctor` reports how many system roots the CLI loaded. When set in the Loom process, `SSL_CERT_FILE` or `SSL_CERT_DIR` overrides the default source of system roots.
- **Transport `502` diagnostics**: Returns HTTP `502` (`UPSTREAM_UNAVAILABLE`) classified into fixed categories (`timeout`, `request_invalid`, `tls_error`, `dns_error`, `connect_failed`, `transport_failed`) without exposing upstream URLs or credentials.

---

## Local Observability & Management Workspace

```sh
loom console                             # Opens http://loom.localhost:4320/ with a one-use login ticket
loom console --page providers            # Opens a specific section (onboarding / providers / integrations / overview / requests / sessions / settings)
loom console --host 127.0.0.1            # Fallback host if browser does not resolve *.localhost
```

The management console listens separately on `127.0.0.1:4320` (configurable via `--admin-port` on `loom serve`). `loom console` (and macOS `Loom.app`) exchanges `hub.key` over `POST /api/launch` (blocked if `Origin` is present) for a 60-second single-use ticket, exchanged for an 8-hour `HttpOnly; SameSite=Strict` session cookie (`loom_console`). Seven integrated views cover the full lifecycle:

- **Onboarding Wizard**: Automatically opens on fresh installs when no active models are configured; guides users through 6 preset vendors and custom HTTPS endpoints with official key links, pitfall tips, and live connectivity tests.
- **Providers Manager**: Enable/disable providers, rotate API keys without echoing old credentials, toggle bundled public model catalogs, enable/disable or add/remove individual models, and run safe 1-token upstream probes with whitelisted error diagnostics.
- **Integrations & Network**: Inspect and trigger sync for `pi`, `OpenCode V2`, and `Claude Code`, copy generic OpenAI/Anthropic base URLs (never displaying `hub.key` contents), and configure or disable the persistent loopback outbound HTTP/HTTPS proxy.
- **Overview**: Filter by time window (`24h` / `7d` / `30d`), provider, model, and harness; view input/output token curves, cache read/write breakdown, token-weighted cache hit rate, average TTFT, P50/P95 latency, and success/error counts. Unknown usage stays unknown (`—`), reasoning tokens are never double-counted into output totals, and vendor wallet balances are never fabricated.
- **Requests**: Inspect stage timings (`authenticated`, `routed`, `dispatch`, headers, TTFT, completion), sanitized client request payloads, forwarded payloads (credentials excluded), full response/SSE logs, and jump to linked sessions.
- **Sessions Workspace**: Three-column replay & waterfall workspace backed by a read-only background worker polling every 15 seconds for native transcripts in `~/.pi/agent/sessions`, `~/.claude/projects`, and `~/.local/share/opencode/opencode.db` (supporting both OpenCode V2 `session_v2`/`session_message` and V1 `message`/`part` schemas). Native imports are linked deterministically via session headers (`x-loom-harness`, `x-loom-session-id` from [integrations/pi.ts](integrations/pi.ts) and [integrations/opencode2.ts](integrations/opencode2.ts)) or Claude Code metadata/response IDs—never guessed by prompt similarity and never double-counted into gateway metrics.
- **Capture Settings**: Independently toggle request/response body capture and native session import, and configure retention (`1–3650` days, default `30` days).

Observations are stored locally in `<data-dir>/observability/traces.sqlite3` (`0600` inside `0700`) with secret/token redaction applied prior to persistence and a `16 MiB` per-response capture cap (`truncated: true` if exceeded). See [docs/observability.md](docs/observability.md) for the specification and metrics rules, and [prototypes/session-workspace/README.md](prototypes/session-workspace/README.md) for the synthetic frontend session-workspace prototype.

---

## Central Configuration & CLI Reference

Default data directory locations (override via `--data-dir <path>` or `MODEL_HUB_DATA_DIR`):

- **macOS**: `~/Library/Application Support/ModelHub/.local`
- **Linux**: `~/.local/share/ModelHub/.local`

Example `providers.json`:

```json
{
  "proxy": "",
  "providers": {
    "local-demo": {
      "prefix": "demo",
      "name": "My provider",
      "baseURL": "https://provider.example/v1",
      "anthropicBaseURL": "https://provider.example/anthropic",
      "billing": "API Wallet",
      "enabled": true,
      "keyFile": "/absolute/path/to/local/key",
      "models": [
        {
          "id": "model-id",
          "name": "Model",
          "contextWindow": 128000,
          "maxTokens": 8192,
          "input": ["text"],
          "reasoning": true
        }
      ]
    }
  }
}
```

### Proxy & Key Commands

```sh
loom proxy                                # Show current mode (direct or persistent proxy configured)
loom proxy http://127.0.0.1:7890          # Set persistent HTTP/HTTPS proxy (takes effect on next request)
loom proxy off                            # Restore direct upstream connections
loom set-key local-demo                   # Rotate upstream API key via hidden input (backs up old key)
loom check                                # Validate providers.json and confirm every keyFile exists
```

Loom explicitly builds upstream clients with `.no_proxy()` and **ignores** terminal `HTTP_PROXY` / `HTTPS_PROXY` environment variables so background autostart services behave identically regardless of shell environment. `loom proxy` rejects URLs containing embedded usernames, passwords, query strings, fragments, non-root paths, or non-HTTP(S) schemes, and never echoes the full proxy URL.

---

## Client Sync & Reasoning Compatibility

Running `loom sync` (or updating `providers.json` while `loom serve` watches it every 2 seconds) safely updates client configs with private backups under `<data-dir>/backups/`:

- **pi** (`~/.pi/agent/models.json`, `~/.pi/agent/settings.json`) and observability extension `~/.pi/agent/extensions/loom-observability.ts` ([integrations/pi.ts](integrations/pi.ts), using `before_provider_headers`).
- **OpenCode V2** (`~/.config/opencode/opencode.json`, or comment-free `~/.config/opencode/opencode.jsonc`; refuses to overwrite if `opencode.jsonc` has comments or both files exist) and plugin `~/.config/opencode/plugins/loom-observability/index.ts` ([integrations/opencode2.ts](integrations/opencode2.ts), using `ctx.session.hook("http.request")`).
- **Claude Code** (`~/.claude/settings.json`, updated only if it already exists, preserving themes, permissions, and unrelated OAuth/providers).

### DeepSeek & LongCat Thinking Controls

Because clients connect only to `127.0.0.1`, URL-based vendor detection inside client SDKs cannot recognize DeepSeek or LongCat behind the gateway. `loom sync` detects the actual upstream host from `providers.json` and emits explicit client capabilities:

- **DeepSeek (`api.deepseek.com`)**: Sets `compat.thinkingFormat: "deepseek"` and `thinkingLevelMap` (`minimal -> low`, `low -> low`, `medium/high/xhigh -> high`, `max -> max`) for `pi`, and emits `capabilities.reasoning` object plus `none` / `low` / `high` / `max` `variants` (`thinking.type: "disabled"` vs `"enabled"`) for OpenCode V2 without overriding user defaults. Custom `piCompat` and model-level `compat` / `thinkingLevelMap` remain supported.
- **LongCat (`api.longcat.chat`)**: Preset configures `piCompat: {"thinkingFormat": "deepseek", "supportsReasoningEffort": false}` so `pi` toggles `thinking.type` (`enabled`/`disabled`) without sending unsupported `reasoning_effort`.

Run the optional real `pi-ai` SDK loopback verification (uses a local mock server, no real keys or upstream calls):

```sh
node tests/pi-thinking.mjs target/debug/loom /path/to/@earendil-works/pi-ai
```

---

## Security Model

- **Permissions**: Private data directories are enforced at `0700`; `hub.key`, `providers.json`, upstream key files, backups, and `observability/traces.sqlite3` are written atomically with `0600` (`PRAGMA journal_mode=DELETE; PRAGMA secure_delete=ON`).
- **Host & Origin isolation**:
  - Gateway (`4319`) enforces loopback `Host` (`127.0.0.1:4319` / `localhost:4319`) and rejects browser `Origin` and `sec-fetch-site: cross-site` requests except `chrome-extension://` and `moz-extension://` origins carrying the local `hub.key`.
  - Console (`4320`) enforces exact `Host` (`loom.localhost:4320`, `localhost:4320`, `127.0.0.1:4320`), strict same-origin `Origin == http://<Host>`, no CORS headers, strict CSP (`default-src 'self'; script-src 'self'; ...`), and `X-Frame-Options: DENY`.
- **Upstream validation**: Only `https://` URLs without credentials, query parameters, or fragments are accepted; HTTP redirects are disabled (`Policy::none()`). Upstream keys are never written to client configs, telemetry databases, or terminal output.

---

## Verification & Ecosystem

```sh
cargo test --locked
cargo fmt --check
cargo clippy --locked -- -D warnings
```

In the local tool ecosystem, Loom serves as the unified loopback model gateway for applications such as [`verba`](https://github.com/xdlkc/verba), [`gloss`](https://github.com/xdlkc/gloss), and [`llm-texas-holdem`](https://github.com/xdlkc/llm-texas-holdem).
